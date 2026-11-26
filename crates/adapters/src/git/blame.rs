use super::{git, CommitGraph};
use creditia_core::hash::{ext_of, normalize_into, LineHasher};
use creditia_core::model::{excluded, BlameFile};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const BINARY_PREFIX: usize = 8_000;
const MAX_FAST_BLOB: usize = 1 << 20;
const MAX_REVISIONS: usize = 8;
const CHECKPOINT: Duration = Duration::from_secs(60);

pub struct HeadBlame {
    pub files: Vec<(String, BlameFile)>,
    pub cache_changed: bool,
}

/// 存活代码：HEAD 上每一行来自哪个提交。只 blame 文本文件；按 blob 缓存
/// HEAD 上每个文本文件的 blame（按 blob 缓存）。几个线程从同一个队列里取文件；
/// 跑的过程中每隔一分钟调一次 `save`，第一次分析大仓库时中途取消，下次接着跑
pub fn blame_head(
    repo: &Path,
    rev: &str,
    graph: &CommitGraph,
    generated: &[regex::Regex],
    cache: &mut HashMap<String, BlameFile>,
    save: &mut dyn FnMut(&HashMap<String, BlameFile>),
) -> HeadBlame {
    let timing = std::env::var_os("CREDITIA_TIMING").is_some_and(|v| v == "1");
    let mut last = Instant::now();
    let mut phase = |name| {
        if timing {
            let now = Instant::now();
            eprintln!(
                "@timing phase={name} seconds={:.6}",
                (now - last).as_secs_f64()
            );
            last = now;
        }
    };
    let tree = git(repo, &["ls-tree", "-r", rev]);
    let files: Vec<_> = tree
        .lines()
        .filter_map(|line| {
            let (meta, path) = line.split_once('\t')?;
            let (mode, blob) = meta.split_once(" blob ")?;
            if !regular(mode) || excluded(path) || generated.iter().any(|re| re.is_match(path)) {
                return None;
            }
            // 缓存按「blob 路径」存：内容一样的两个文件路径不同，行哈希和 blame 也不同
            Some((blob, path, format!("{blob} {path}")))
        })
        .collect();
    let config = git(
        repo,
        &[
            "config",
            "--name-only",
            "--get-regexp",
            r"^blame\.ignorerevsfile$|^diff(\..*)?\.(algorithm|textconv)$|^diff\.(default|unspecified)\.binary$",
        ],
    );
    let uses_custom_blame = config.lines().any(|k| !k.ends_with(".binary"));
    let uses_custom_binary = config.lines().any(|k| k.ends_with(".binary"));
    let overrides = text_overrides(repo, rev, &files, uses_custom_binary);
    phase("blame_select");

    // 创建以后再没改过的文件不用跑 blame；只有少量缓存未命中时直接跑 Git。
    let candidates: Vec<_> = files
        .iter()
        .filter(|(_, path, key)| {
            !cache.contains_key(key) && overrides.get(path).is_none_or(|&text| text)
        })
        .map(|&(blob, path, _)| (blob, path))
        .collect();
    let histories = if !uses_custom_blame && candidates.len() > 50 {
        linear_histories(repo, rev, graph, &candidates)
    } else {
        HashMap::new()
    };
    let sizes = removed_sizes(repo, &histories);

    let mut cat = BlobBatch::open(repo);
    let mut buffer = Vec::new();
    let total = files.len();
    let mut live = Vec::with_capacity(total);
    let mut todo = vec![];
    let mut fast_count = 0;
    let mut unchanged_count = 0;
    let mut cache_changed = false;
    let mut saved = Instant::now();
    creditia_core::progress::tick("blame", 0, total);
    for (i, (blob, path, key)) in files.into_iter().enumerate() {
        let history = histories.get(path);
        let explicit_text = overrides.get(path).copied();
        let size = if history.is_some() || explicit_text.is_none() {
            let limit = if history.is_some() {
                MAX_FAST_BLOB
            } else {
                BINARY_PREFIX
            };
            cat.as_mut()
                .and_then(|batch| batch.read(blob, limit, &mut buffer).ok().flatten())
        } else {
            None
        };
        let is_text = explicit_text.unwrap_or_else(|| match size {
            Some(size) => size != 0 && !buffer[..buffer.len().min(BINARY_PREFIX)].contains(&0),
            None => grep_selected(repo, rev, [path]).contains(path),
        });
        if !is_text {
            continue;
        }
        if !cache.contains_key(&key) {
            let fast = history
                .filter(|_| size.is_some_and(|n| n <= MAX_FAST_BLOB))
                .and_then(|h| fast_blame(path, h, cat.as_mut()?, &buffer, &sizes));
            if let Some(result) = fast {
                cache.insert(key.clone(), result);
                cache_changed = true;
                fast_count += 1;
                unchanged_count += usize::from(history.is_some_and(|h| h.revisions.len() == 1));
                creditia_core::progress::tick("blame", i + 1 - todo.len(), total);
                checkpoint(cache, save, &mut saved);
            } else {
                todo.push((key.clone(), path));
            }
        }
        live.push((key, path));
    }
    drop(cat);
    // 只留 HEAD 上还在的文件，缓存不会越攒越大
    let live_keys: HashSet<&str> = live.iter().map(|(key, _)| key.as_str()).collect();
    let old_len = cache.len();
    cache.retain(|key, _| live_keys.contains(key.as_str()));
    cache_changed |= cache.len() != old_len;
    phase("blame_fast");
    if timing {
        eprintln!("@timing unchanged_blame_files={unchanged_count} linear_blame_files={} git_blame_files={} cached_blame_files={}",
            fast_count - unchanged_count, todo.len(), live.len() - todo.len() - fast_count);
    }

    if !todo.is_empty() {
        let n = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(16)
            .min(todo.len());
        let next = AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..n {
                let (tx, next, todo) = (tx.clone(), &next, &todo);
                scope.spawn(move || loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((key, path)) = todo.get(i) else {
                        break;
                    };
                    if tx.send((key.clone(), blame_file(repo, rev, path))).is_err() {
                        break;
                    }
                });
            }
            drop(tx);
            let mut done = total - todo.len();
            creditia_core::progress::tick("blame", done, total);
            for (key, result) in rx {
                cache.insert(key, result);
                done += 1;
                creditia_core::progress::tick("blame", done, total);
                checkpoint(cache, save, &mut saved);
            }
        });
        cache_changed = true;
    }
    phase("blame_git");
    creditia_core::progress::tick("blame", total, total);
    let files = live
        .into_iter()
        .map(|(key, path)| (path.to_string(), cache[&key].clone()))
        .collect();
    HeadBlame {
        files,
        cache_changed,
    }
}

fn checkpoint(
    cache: &HashMap<String, BlameFile>,
    save: &mut dyn FnMut(&HashMap<String, BlameFile>),
    saved: &mut Instant,
) {
    if saved.elapsed() >= CHECKPOINT {
        save(cache);
        *saved = Instant::now();
    }
}

fn regular(mode: &str) -> bool {
    mode.starts_with("100")
}

fn git_with_input(repo: &Path, args: &[&str], input: Vec<u8>) -> Option<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    std::thread::scope(|s| {
        let writer = s.spawn(move || stdin.write_all(&input));
        let result = child.wait_with_output().ok()?;
        writer.join().ok()?.ok()?;
        result
            .status
            .success()
            .then(|| String::from_utf8_lossy(&result.stdout).into_owned())
    })
}

/// Git grep inspects the first 8,000 bytes of nonempty regular blobs unless
/// the path has a diff attribute or a configured binary driver overriding it.
/// Let Git interpret those overrides, using only the affected paths.
fn text_overrides<'a>(
    repo: &Path,
    rev: &str,
    files: &[(&str, &'a str, String)],
    custom_binary: bool,
) -> HashMap<&'a str, bool> {
    if files.is_empty() {
        return HashMap::new();
    }
    let input = files
        .iter()
        .flat_map(|(_, p, _)| p.bytes().chain(std::iter::once(b'\n')))
        .collect();
    let attrs = git_with_input(repo, &["check-attr", "--stdin", "diff"], input);
    let ordinary: HashSet<&str> = attrs
        .as_deref()
        .into_iter()
        .flat_map(str::lines)
        .filter_map(|line| line.strip_suffix(": diff: unspecified"))
        .collect();
    let special: Vec<&str> = files
        .iter()
        .map(|(_, path, _)| *path)
        .filter(|p| custom_binary || !ordinary.contains(p))
        .collect();
    let text = grep_selected(repo, rev, special.iter().copied());
    special.into_iter().map(|p| (p, text.contains(p))).collect()
}

fn grep_selected<'a>(
    repo: &Path,
    rev: &str,
    paths: impl IntoIterator<Item = &'a str>,
) -> HashSet<String> {
    let mut text = HashSet::new();
    let mut args = vec![];
    let mut bytes = 0;
    let mut run = |args: &[OsString]| {
        if args.is_empty() {
            return;
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "core.quotepath=false",
                "--literal-pathspecs",
                "grep",
                "-I",
                "-l",
                "-e",
                "",
                rev,
                "--",
            ])
            .args(args)
            .output();
        if let Ok(output) = output {
            text.extend(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| line.split_once(':').map(|(_, path)| path.to_string())),
            );
        }
    };
    for path in paths {
        let Some(path_arg) = git_path_arg(path) else {
            continue;
        };
        // Keep each invocation within the platform's argument size limit.
        if bytes + path.len() >= 16 * 1024 {
            run(&args);
            args.clear();
            bytes = 0;
        }
        bytes += path.len() + 1;
        args.push(path_arg);
    }
    run(&args);
    text
}

fn git_path_arg(path: &str) -> Option<OsString> {
    if !path.starts_with('"') {
        return Some(path.into());
    }
    let raw = crate::trace_import::git_quoted_path_bytes(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(OsString::from_vec(raw))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(raw).ok().map(OsString::from)
    }
}

struct BlobBatch {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    header: String,
}

impl BlobBatch {
    fn open(repo: &Path) -> Option<Self> {
        // 一次 cat-file --batch 读出这些文件的内容
        let mut child = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let input = child.stdin.take();
        let output = BufReader::with_capacity(64 * 1024, child.stdout.take()?);
        Some(Self {
            child,
            input,
            output,
            header: String::new(),
        })
    }

    fn read(
        &mut self,
        blob: &str,
        limit: usize,
        buffer: &mut Vec<u8>,
    ) -> io::Result<Option<usize>> {
        let result = self.read_blob(blob, limit, buffer);
        if result.is_err() {
            // An interrupted response leaves the stream out of sync.
            drop(self.input.take());
            let _ = self.child.kill();
        }
        result
    }

    fn read_blob(
        &mut self,
        blob: &str,
        limit: usize,
        buffer: &mut Vec<u8>,
    ) -> io::Result<Option<usize>> {
        let Some(input) = &mut self.input else {
            return Ok(None);
        };
        writeln!(input, "{blob}")?;
        self.header.clear();
        self.output.read_line(&mut self.header)?;
        // `<id> missing`：没有内容可读，跳过（这个文件照常跑 blame）
        let mut header = self.header.split_ascii_whitespace();
        let (Some(id), Some(kind), Some(size)) = (header.next(), header.next(), header.next())
        else {
            return Ok(None);
        };
        let size: usize = size.parse().map_err(io::Error::other)?;
        let take = size.min(limit);
        buffer.resize(take, 0);
        self.output.read_exact(buffer)?;
        if size > take {
            io::copy(
                &mut self.output.by_ref().take((size - take) as u64),
                &mut io::sink(),
            )?;
        }
        let mut end = [0];
        self.output.read_exact(&mut end)?;
        Ok((id == blob && kind == "blob" && end == *b"\n").then_some(size))
    }
}

impl Drop for BlobBatch {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.child.wait();
    }
}

struct Revision {
    blob: String,
    commit: String,
}

struct FileHistory {
    next: String,
    revisions: Vec<Revision>,
    removed_at_creation: Arc<[String]>,
    unchanged_only: bool,
}

/// 创建以后再没改过的文件：HEAD 上的 blob 正是某个提交新建这个文件时写下的（不是改名、复制来的），
/// 那每一行都出自那个提交、行号也一样，和 git blame 的结果相同，却只要一遍 `git log --raw` 加读一次文件内容。
/// 大仓库里这样的文件常常占一大半。有少量修改的文件也可复用这趟遍历。
fn linear_histories<'a>(
    repo: &Path,
    rev: &str,
    graph: &CommitGraph,
    todo: &[(&str, &'a str)],
) -> HashMap<&'a str, FileHistory> {
    let mut histories: HashMap<_, _> = todo
        .iter()
        .map(|&(blob, path)| {
            (
                path,
                FileHistory {
                    next: blob.to_string(),
                    revisions: vec![],
                    removed_at_creation: Arc::default(),
                    unchanged_only: false,
                },
            )
        })
        .collect();
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "core.quotepath=false",
            "log",
            "--raw",
            "--no-abbrev",
            "--topo-order",
            "--root",
            "--diff-merges=separate",
            "--ignore-submodules=none",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "-M",
            "--format=@@%H",
            rev,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return HashMap::new();
    };
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut bytes = Vec::new();
    let mut commit = String::new();
    let mut parents = None;
    let mut first_parent = false;
    let mut added = vec![];
    let mut removed = vec![];
    let mut merge_additions = HashMap::<String, usize>::new();
    let mut complete = true;
    let flush = |histories: &mut HashMap<&str, FileHistory>,
                 added: &mut Vec<String>,
                 removed: &mut Vec<String>| {
        // 新建它的那个提交里还删了、改名了别的文件时：git blame 可能把它当成从那个文件改名来的。
        // 记录这些来源，之后按 Git 的相似度门槛排除大小不可能匹配的文件。
        if !added.is_empty() && !removed.is_empty() {
            let sources: Arc<[String]> = std::mem::take(removed).into();
            for path in added.drain(..) {
                if let Some(history) = histories.get_mut(path.as_str()) {
                    history.removed_at_creation = Arc::clone(&sources);
                }
            }
        } else {
            added.clear();
            removed.clear();
        }
    };
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => {
                complete = false;
                break;
            }
        }
        let line = String::from_utf8_lossy(bytes.strip_suffix(b"\n").unwrap_or(&bytes));
        if let Some(hash) = line.strip_prefix("@@") {
            if hash == commit {
                continue;
            }
            flush(&mut histories, &mut added, &mut removed);
            drop_merge_creations(&mut histories, &mut merge_additions, parents);
            parents = graph
                .parents
                .get(hash)
                .filter(|_| hash.len() == 40)
                .map(Vec::len);
            first_parent = graph.first_parent.contains(hash);
            commit.clear();
            commit.push_str(hash);
            continue;
        }
        let Some((meta, paths)) = line.strip_prefix(':').and_then(|l| l.split_once('\t')) else {
            continue;
        };
        let mut fields = meta.split_ascii_whitespace();
        let (Some(old_mode), Some(new_mode), Some(old), Some(new), Some(status)) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };
        // 改名、复制是「旧路径<TAB>新路径」，两边都交给 Git。
        let path = if let Some((from, to)) = paths.split_once('\t') {
            histories.remove(from);
            to
        } else {
            paths
        };
        if parents.is_some_and(|n| n <= 1)
            && regular(old_mode)
            && (status == "D" || status.starts_with('R'))
        {
            removed.push(old.to_string());
        }
        let Some(history) = histories.get_mut(path) else {
            continue;
        };
        if parents.is_some_and(|n| n > 1) {
            // For an unchanged file, blame passes the entire blob to the first
            // parent with the same path and blob. Count additions across every
            // parent so a file created by the merge itself still goes to Git.
            if status == "A"
                && history.revisions.is_empty()
                && regular(new_mode)
                && history.next == new
            {
                history.unchanged_only = true;
                *merge_additions.entry(path.to_string()).or_default() += 1;
            } else {
                histories.remove(path);
            }
            continue;
        }
        // 最近一次是改名、复制过来的，或者历史不能连成一条单父链，都照常跑 blame。
        let content_edit =
            status == "M" && regular(old_mode) && first_parent && !history.unchanged_only;
        let creation = status == "A" && (first_parent || history.revisions.is_empty());
        if parents.is_none()
            || !regular(new_mode)
            || history.next != new
            || history.revisions.len() == MAX_REVISIONS
            || !(creation || content_edit)
        {
            histories.remove(path);
            continue;
        }
        if status == "M" && old == new {
            continue; // chmod only
        }
        if status == "A" {
            added.push(path.to_string());
        }
        let prior = if status == "A" {
            String::new()
        } else {
            old.to_string()
        };
        let blob = std::mem::replace(&mut history.next, prior);
        history.revisions.push(Revision {
            blob,
            commit: commit.clone(),
        });
    }
    flush(&mut histories, &mut added, &mut removed);
    drop_merge_creations(&mut histories, &mut merge_additions, parents);
    drop(reader);
    if !child.wait().is_ok_and(|s| s.success()) || !complete {
        return HashMap::new();
    }
    histories.retain(|_, h| h.next.is_empty());
    histories
}

fn drop_merge_creations(
    histories: &mut HashMap<&str, FileHistory>,
    additions: &mut HashMap<String, usize>,
    parents: Option<usize>,
) {
    for (path, n) in additions.drain() {
        if parents.is_none_or(|parents| n >= parents) {
            histories.remove(path.as_str());
        }
    }
}

fn removed_sizes(repo: &Path, histories: &HashMap<&str, FileHistory>) -> HashMap<String, usize> {
    let ids: HashSet<&str> = histories
        .values()
        .flat_map(|h| h.removed_at_creation.iter().map(String::as_str))
        .collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let input = ids
        .into_iter()
        .flat_map(|id| id.bytes().chain(std::iter::once(b'\n')))
        .collect();
    let output = git_with_input(
        repo,
        &["cat-file", "--batch-check=%(objectname) %(objectsize)"],
        input,
    )
    .unwrap_or_default();
    output
        .lines()
        .filter_map(|line| {
            let (id, size) = line.split_once(' ')?;
            Some((id.to_string(), size.parse().ok()?))
        })
        .collect()
}

fn fast_blame(
    path: &str,
    history: &FileHistory,
    cat: &mut BlobBatch,
    head: &[u8],
    sizes: &HashMap<String, usize>,
) -> Option<BlameFile> {
    let mut origin = vec![];
    let mut before = Vec::new();
    let mut buffer = Vec::new();
    for (i, rev) in history.revisions.iter().enumerate().rev() {
        let text = if i == 0 {
            head
        } else {
            let size = cat.read(&rev.blob, MAX_FAST_BLOB, &mut buffer).ok()??;
            if size > MAX_FAST_BLOB {
                return None;
            }
            &buffer
        };
        if text.contains(&0) {
            return None;
        }
        let new = blob_lines(text);
        if i + 1 == history.revisions.len() {
            if history.removed_at_creation.iter().any(|id| {
                sizes
                    .get(id)
                    .is_none_or(|&n| n.min(text.len()).saturating_mul(2) >= n.max(text.len()))
            }) {
                return None; // The per-path rename search can choose a source already paired with another destination.
            }
            origin.extend((1..=new.len()).map(|line| (i as u32, line as u32)));
        } else {
            let old = blob_lines(&before);
            origin = transfer_blame(&old, &new, &origin, i as u32)?;
        }
        std::mem::swap(&mut buffer, &mut before);
    }
    let ext = ext_of(path);
    let hasher = LineHasher::new(path);
    let mut normalized = String::new();
    let lines = String::from_utf8_lossy(head)
        .lines()
        .zip(origin)
        .map(|(line, (id, original))| {
            [
                id,
                0,
                blame_key(line, &ext, &hasher, &mut normalized),
                original,
            ]
        })
        .collect();
    Some(BlameFile {
        commits: history.revisions.iter().map(|r| r.commit.clone()).collect(),
        rels: vec![path.to_string()],
        lines,
    })
}

fn blob_lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split_inclusive(|b| *b == b'\n').collect()
}

fn transfer_blame(
    old: &[&[u8]],
    new: &[&[u8]],
    origin: &[(u32, u32)],
    commit: u32,
) -> Option<Vec<(u32, u32)>> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    // A single replacement is unambiguous only if none of its unmatched
    // lines could be aligned elsewhere. Compare raw lines, including LF.
    for (changed, other) in [(&old[prefix..old_end], new), (&new[prefix..new_end], old)] {
        if !changed.is_empty() {
            let other: HashSet<_> = other.iter().copied().collect();
            if changed.iter().any(|line| other.contains(line)) {
                return None;
            }
        }
    }
    let mut out = Vec::with_capacity(new.len());
    out.extend_from_slice(&origin[..prefix]);
    out.extend((prefix + 1..=new_end).map(|line| (commit, line as u32)));
    out.extend_from_slice(&origin[old_end..]);
    Some(out)
}

fn blame_key(line: &str, ext: &str, hasher: &LineHasher, normalized: &mut String) -> u32 {
    normalize_into(line, ext, normalized);
    if normalized.chars().nth(3).is_some() {
        hasher.hash([normalized.as_str()])
    } else {
        0
    }
}

fn blame_file(repo: &Path, rev: &str, path: &str) -> BlameFile {
    let Some(path) = git_path_arg(path) else {
        return BlameFile::default();
    };
    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "core.quotepath=false",
            "blame",
            "--porcelain",
            rev,
            "--",
        ])
        .arg(path)
        .output()
    else {
        return BlameFile::default();
    };
    let out = String::from_utf8_lossy(&output.stdout);
    let mut res = BlameFile::default();
    let (mut commit, mut file, mut orig) = ("", "", 0u32);
    let mut filenames = HashMap::new();
    let mut commits = HashMap::new();
    let mut paths = HashMap::new();
    let mut normalized = String::new();
    for line in out.lines() {
        if let Some(code) = line.strip_prefix('\t') {
            let rel = filenames.get(commit).copied().unwrap_or(file);
            let (ri, ext, hasher) = paths.entry(rel).or_insert_with(|| {
                let index = res.rels.len() as u32;
                res.rels.push(rel.to_string());
                (index, ext_of(rel), LineHasher::new(rel))
            });
            let ci = commits.entry(commit).or_insert_with(|| {
                let index = res.commits.len() as u32;
                res.commits.push(commit.to_string());
                index
            });
            res.lines.push([
                *ci,
                *ri,
                blame_key(code, ext, hasher, &mut normalized),
                orig,
            ]);
        } else if let Some(f) = line.strip_prefix("filename ") {
            file = f;
            filenames.insert(commit, file);
        } else if let Some((h, positions)) = line.split_once(' ') {
            if h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
                commit = h;
                orig = positions
                    .split(' ')
                    .next()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);
            }
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{generated_patterns, history_paths, read_history, units};
    use std::fs;
    use std::path::PathBuf;

    struct Repo(PathBuf);

    impl Repo {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "creditia-blame-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            let repo = Self(p);
            repo.git(&["init", "-q", "--object-format=sha1"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout)
                .unwrap()
                .trim_end()
                .to_string()
        }

        fn save(&self, parents: &[&str]) -> String {
            self.git(&["add", "-A"]);
            let tree = self.git(&["write-tree"]);
            let mut args = vec!["commit-tree", &tree, "-m", "test revision"];
            for parent in parents {
                args.extend(["-p", parent]);
            }
            let hash = self.git(&args);
            self.git(&["update-ref", "HEAD", &hash]);
            hash
        }

        fn write(&self, path: &str, bytes: impl AsRef<[u8]>) {
            let dest = self.0.join(path);
            fs::create_dir_all(dest.parent().unwrap()).unwrap();
            fs::write(dest, bytes).unwrap();
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn assert_blame_eq(repo: &Path, rev: &str, path: &str, fast: &BlameFile) {
        let git = blame_file(repo, rev, path);
        let lines = |bf: &BlameFile| {
            bf.iter()
                .map(|b| (b.commit.to_string(), b.rel.to_string(), b.key, b.orig))
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(fast), lines(&git), "{path}");
    }

    #[test]
    fn blob_selection_matches_grep_with_empty_files_and_attributes() {
        let repo = Repo::new();
        repo.write(".gitattributes", "generated.rs linguist-generated\n");
        repo.write(
            "attrs/.gitattributes",
            "excluded -diff\nforced diff\nexplicit diff=unset\nnamed diff=custom\n",
        );
        repo.git(&["config", "diff.custom.binary", "true"]);
        for (name, body) in [
            ("generated.rs", &b"generated\n"[..]),
            ("empty", b""),
            ("newline", b"\n"),
            ("no-newline", b"source"),
            ("nul", b"two\0words\n"),
            ("non-utf8", b"one\xffline\n"),
            ("attrs/excluded", b"looks text\n"),
            ("attrs/forced", b"a\0b\n"),
            ("attrs/explicit", b"looks text\n"),
            ("attrs/named", b"looks text\n"),
        ] {
            repo.write(name, body);
        }
        let mut boundary = vec![b'x'; BINARY_PREFIX];
        boundary.push(0);
        repo.write("nul-after-prefix", &boundary);
        boundary[BINARY_PREFIX - 1] = 0;
        repo.write("nul-in-prefix", &boundary);
        #[cfg(unix)]
        {
            repo.write("tab\tname", "source\n");
            std::os::unix::fs::symlink("no-newline", repo.0.join("symlink")).unwrap();
        }
        let rev = repo.save(&[]);
        let generated = generated_patterns(&repo.0, &rev);
        let (_, graph) = units(&repo.0, &rev, &[]);
        let mut cache = HashMap::new();
        let actual = blame_head(&repo.0, &rev, &graph, &generated, &mut cache, &mut |_| {});
        let reference = git(&repo.0, &["grep", "-I", "-l", "-e", "", &rev]);
        let want: HashSet<&str> = reference
            .lines()
            .filter_map(|l| l.split_once(':').map(|(_, p)| p))
            .filter(|p| !generated.iter().any(|r| r.is_match(p)))
            .collect();
        assert_eq!(
            actual
                .files
                .iter()
                .map(|(p, _)| p.as_str())
                .collect::<HashSet<_>>(),
            want
        );
        for (path, _) in &actual.files {
            assert!(cache
                .keys()
                .any(|k| k.split_once(' ').is_some_and(|(_, p)| p == path)));
        }
        assert!(want.contains("nul-after-prefix"));
        assert!(!want.contains("nul-in-prefix"));
        assert!(!want.contains("empty"));
        #[cfg(unix)]
        assert!(actual
            .files
            .iter()
            .any(|(path, blame)| path == r#""tab\tname""# && blame.len() == 1));
    }

    #[test]
    fn linear_blame_transfers_exact_coordinates_and_rejects_ambiguous_edits() {
        let repo = Repo::new();
        repo.write(
            "linear.rs",
            "outline\nold carefully spelled line\nlast line without newline",
        );
        repo.write("ambiguous.rs", "same line\nsame line\n");
        repo.write("rename-source.rs", "copied content\n".repeat(10));
        repo.write("gone.rs", "tiny\n");
        let root = repo.save(&[]);
        repo.write(
            "linear.rs",
            "outline\ninserted lines\nreplacement content\nlast line without newline",
        );
        repo.write("ambiguous.rs", "same line\n");
        repo.write("new.rs", "completely unrelated new content\n".repeat(30));
        repo.write("duplicate.rs", "copied content\n".repeat(10));
        fs::rename(repo.0.join("rename-source.rs"), repo.0.join("renamed.rs")).unwrap();
        fs::remove_file(repo.0.join("gone.rs")).unwrap();
        let modified = repo.save(&[&root]);
        let (_, graph) = units(&repo.0, &modified, &[]);
        let tree = git(&repo.0, &["ls-tree", "-r", &modified]);
        let todo = tree_blobs(&tree);
        let histories = linear_histories(&repo.0, &modified, &graph, &todo);
        let mut cat = BlobBatch::open(&repo.0).unwrap();
        let sizes = removed_sizes(&repo.0, &histories);
        let mut buffer = vec![];
        let mut accepted = HashSet::new();
        for (id, path) in todo {
            cat.read(id, MAX_FAST_BLOB, &mut buffer).unwrap().unwrap();
            if let Some(blame) = histories
                .get(path)
                .and_then(|h| fast_blame(path, h, &mut cat, &buffer, &sizes))
            {
                assert_blame_eq(&repo.0, &modified, path, &blame);
                accepted.insert(path);
            }
        }
        assert_eq!(accepted, HashSet::from(["linear.rs", "new.rs"]));
        let mut paths = HashSet::new();
        read_history(&repo.0, &modified, None, Some(&mut paths)).unwrap();
        assert_eq!(paths, history_paths(&repo.0, &modified).unwrap());
        assert!(paths.contains("gone.rs") && paths.contains("renamed.rs"));
    }

    #[test]
    fn unchanged_side_branch_file_is_distinct_from_a_merge_creation() {
        let repo = Repo::new();
        repo.write("linear.rs", "first line\n");
        let root = repo.save(&[]);
        repo.write("side.rs", "created on a side branch\n");
        repo.write("linear.rs", "first line\nadded on a side branch\n");
        let side = repo.save(&[&root]);
        repo.write("merge.rs", "created while merging\n");
        let merged = repo.save(&[&root, &side]);
        let (_, graph) = units(&repo.0, &merged, &[]);
        let tree = git(&repo.0, &["ls-tree", "-r", &merged]);
        let todo = tree_blobs(&tree);
        let histories = linear_histories(&repo.0, &merged, &graph, &todo);
        assert_eq!(histories.keys().copied().collect::<Vec<_>>(), ["side.rs"]);
        let history = &histories["side.rs"];
        assert_eq!(history.revisions.len(), 1);
        assert_eq!(history.revisions[0].commit, side);
        let mut cat = BlobBatch::open(&repo.0).unwrap();
        let result = fast_blame(
            "side.rs",
            history,
            &mut cat,
            b"created on a side branch\n",
            &HashMap::new(),
        )
        .unwrap();
        assert_blame_eq(&repo.0, &merged, "side.rs", &result);
    }

    fn tree_blobs(tree: &str) -> Vec<(&str, &str)> {
        tree.lines()
            .filter_map(|line| {
                let (meta, path) = line.split_once('\t')?;
                let (mode, blob) = meta.split_once(" blob ")?;
                regular(mode).then_some((blob, path))
            })
            .collect()
    }

    /// 跳过 blame 的捷径和真跑 git blame 结果一致。CREDITIA_BLAME_CHECK=<仓库> 时拿真仓库对照（手动跑）
    #[test]
    #[ignore]
    fn unchanged_files_match_real_blame() {
        let Some(repo) = std::env::var_os("CREDITIA_BLAME_CHECK") else {
            return;
        };
        let repo = Path::new(&repo);
        let (_, graph) = units(repo, "HEAD", &[]);
        let tree = git(repo, &["ls-tree", "-r", "HEAD"]);
        let todo = tree_blobs(&tree);
        let histories = linear_histories(repo, "HEAD", &graph, &todo);
        let sizes = removed_sizes(repo, &histories);
        let mut cat = BlobBatch::open(repo).unwrap();
        let mut buffer = vec![];
        let n: usize = std::env::var("CREDITIA_BLAME_CHECK_N")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(usize::MAX);
        let mut checked = 0;
        let mut edited = 0;
        for (id, path) in todo {
            let Some(history) = histories.get(path) else {
                continue;
            };
            let Some(size) = cat.read(id, MAX_FAST_BLOB, &mut buffer).unwrap() else {
                continue;
            };
            if size == 0 || size > MAX_FAST_BLOB {
                continue;
            }
            if let Some(blame) = fast_blame(path, history, &mut cat, &buffer, &sizes) {
                assert_blame_eq(repo, "HEAD", path, &blame);
                checked += 1;
                edited += usize::from(history.revisions.len() > 1);
                if checked == n {
                    break;
                }
            }
        }
        eprintln!("checked {checked} fast files ({edited} edited) against Git blame");
        assert!(checked > 0);
    }
}
