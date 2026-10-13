//! git adapter：提交、逐行新增内容（只取哈希）、PR 划分、版本、存活代码、盖章 trailer

use creditia_core::hash::{ext_of, line_key, normalize};
use creditia_core::model::{excluded, BlameLine, Commit, FileAdd, PrMeta, Unit};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(["-c", "core.quotepath=false"]).args(args).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

pub fn git_ok(repo: &Path, args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(repo).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

/// 仓库身份：根提交 + 远程地址（去协议、.git 后缀）
pub fn identity(repo: &Path) -> (String, Vec<String>) {
    let root = git(repo, &["rev-list", "--max-parents=0", "HEAD"]).lines().last().unwrap_or("").to_string();
    let remotes: HashSet<String> = git(repo, &["remote", "-v"])
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .map(normalize_remote)
        .collect();
    let mut r: Vec<String> = remotes.into_iter().collect();
    r.sort();
    (root, r)
}

pub fn normalize_remote(url: &str) -> String {
    let u = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let u = u.strip_prefix("https://").or(u.strip_prefix("http://")).or(u.strip_prefix("ssh://")).unwrap_or(u);
    let u = u.strip_prefix("git@").unwrap_or(u).replacen(':', "/", if u.starts_with("git@") || !u.contains("://") && u.contains(':') { 1 } else { 0 });
    u.to_lowercase()
}

/// 主 worktree 的 git 公共目录；worktree 内调用也能映射回主仓库
pub fn main_worktree(repo: &Path) -> Option<std::path::PathBuf> {
    let common = git(repo, &["rev-parse", "--git-common-dir"]).trim().to_string();
    if common.is_empty() {
        return None;
    }
    let p = if Path::new(&common).is_absolute() { std::path::PathBuf::from(common) } else { repo.join(common) };
    p.parent().map(|x| x.to_path_buf()).and_then(|x| x.canonicalize().ok())
}

/// `.gitattributes` 里标为生成物或第三方的路径（linguist-generated、linguist-vendored、-diff、binary），转成正则
pub fn generated_patterns(repo: &Path, rev: &str) -> Vec<regex::Regex> {
    let text = git(repo, &["show", &format!("{rev}:.gitattributes")]);
    let mut out = vec![];
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut it = l.split_whitespace();
        let Some(pat) = it.next() else { continue };
        let attrs: Vec<&str> = it.collect();
        let hit = attrs.iter().any(|a| matches!(*a, "linguist-generated" | "linguist-generated=true" | "linguist-vendored" | "linguist-vendored=true" | "-diff" | "binary"));
        if hit {
            if let Ok(re) = regex::Regex::new(&glob_re(pat)) {
                out.push(re);
            }
        }
    }
    out
}

/// gitignore 式通配 → 正则：不含 `/` 的模式匹配任意目录下的文件名
fn glob_re(pat: &str) -> String {
    let anchored = pat.starts_with('/') || pat.trim_end_matches('/').contains('/');
    let p = pat.trim_start_matches('/');
    let mut re = String::new();
    let cs: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '*' if i + 1 < cs.len() && cs[i + 1] == '*' => { re.push_str(".*"); i += 2; if i < cs.len() && cs[i] == '/' { i += 1; } continue; }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c if "\\.+()|[]{}^$".contains(c) => { re.push('\\'); re.push(c); }
            c => re.push(c),
        }
        i += 1;
    }
    let re = if p.ends_with('/') { format!("{re}.*") } else { re };
    if anchored { format!("^{re}$") } else { format!("(^|.*/){re}$") }
}

/// tag 名的通配：整个名字都要对上（`archive/**` 只排除 archive/ 开头的）
fn tag_re(pat: &str) -> String {
    glob_re(&format!("/{}", pat.trim_start_matches('/')))
}

pub fn worktrees(repo: &Path) -> Vec<String> {
    git(repo, &["worktree", "list", "--porcelain"]).lines().filter_map(|l| l.strip_prefix("worktree ")).map(String::from).collect()
}

// trailer 都指定 separator，这样 git 不会在值后面补换行，一条提交始终占一行
const FMT: &str = "@@@C%x00%H%x00%ae%x00%an%x00%at%x00%ct%x00%s%x00%(trailers:key=Co-authored-by,valueonly,separator=%x7C)%x00%(trailers:key=Creditia-Harness,valueonly,separator=%x7C)%x00%(trailers:key=Creditia-Model,valueonly,separator=%x7C)";

/// 盖章表：提交哈希 → (工具, 模型)
pub type Stamps = HashMap<String, (String, Option<String>)>;

/// 解析 `git log -p` 输出。`range` 为空表示全量到 `rev`。返回提交和盖章表
pub fn read_commits(repo: &Path, rev: &str, range: Option<&str>) -> (Vec<Commit>, Stamps) {
    let spec = range.map(|r| r.to_string()).unwrap_or_else(|| rev.to_string());
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false", "log", "--no-merges", "-p", "-M", "--unified=0", "--no-color", "--no-ext-diff"])
        .arg(format!("--format={FMT}"))
        .arg(&spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return (vec![], HashMap::new());
    };
    let rd = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
    let mut out: Vec<Commit> = vec![];
    let mut stamps = HashMap::new();
    let mut cur: Option<FileAdd> = None;
    let mut ext = String::new();
    let flush = |out: &mut Vec<Commit>, cur: &mut Option<FileAdd>| {
        if let (Some(f), Some(c)) = (cur.take(), out.last_mut()) {
            if !f.keys.is_empty() || f.binary {
                c.files.push(f);
            }
        }
    };
    for line in rd.split(b'\n').flatten() {
        let line = String::from_utf8_lossy(&line);
        if let Some(rest) = line.strip_prefix("@@@C\0") {
            flush(&mut out, &mut cur);
            let p: Vec<&str> = rest.split('\0').collect();
            if p.len() < 7 {
                continue;
            }
            let hash = p[0].to_string();
            // 盖章 trailer 若重复出现，取第一个
            let harness = p.get(7).and_then(|s| s.split('|').next()).map(|s| s.trim()).filter(|s| !s.is_empty());
            if let Some(h) = harness {
                let model = p.get(8).and_then(|s| s.split('|').next()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                stamps.insert(hash.clone(), (h.to_lowercase(), model));
            }
            out.push(Commit {
                hash,
                email: p[1].to_lowercase(),
                name: p[2].into(),
                author_ts: p[3].parse().unwrap_or(0),
                commit_ts: p[4].parse().unwrap_or(0),
                subject: p[5].into(),
                coauthors: p[6].split('|').filter(|x| !x.is_empty()).map(|x| x.to_lowercase()).collect(),
                files: vec![],
            });
        } else if line.starts_with("diff --git ") {
            flush(&mut out, &mut cur);
        } else if let Some(p) = line.strip_prefix("+++ ") {
            let rel = p.strip_prefix("b/").unwrap_or(p).trim_matches('"').to_string();
            if rel != "/dev/null" && !excluded(&rel) {
                ext = ext_of(&rel);
                cur = Some(FileAdd { rel, ..Default::default() });
            }
        } else if line.starts_with("Binary files ") {
            if let Some(b) = line.split(" and ").nth(1) {
                let rel = b.trim_end_matches(" differ").trim_start_matches("b/").to_string();
                if rel != "/dev/null" && !excluded(&rel) {
                    cur = Some(FileAdd { rel, binary: true, ..Default::default() });
                }
            }
        } else if let Some(a) = line.strip_prefix('+') {
            if let Some(f) = cur.as_mut() {
                let n = normalize(a, &ext);
                f.keys.push(if n.chars().count() < 4 { 0 } else { line_key(&f.rel, &n) });
            }
        }
    }
    flush(&mut out, &mut cur);
    let _ = child.wait();
    (out, stamps)
}

/// squash 提交的标题是「PR 标题 (#N)」：先按编号 + 标题匹配（不同托管平台编号会重叠），再退回只按 GitHub 编号
fn find_pr<'a>(prs: &'a [PrMeta], num: u64, title: &str) -> Option<&'a PrMeta> {
    prs.iter().find(|p| p.number == num && p.title.trim() == title.trim()).or_else(|| prs.iter().find(|p| p.number == num && p.source == "github"))
}

/// 按第一父链划分「项」：PR merge、squash 的 (#N) 提交、其余直接提交
pub fn units(repo: &Path, rev: &str, prs: &[PrMeta]) -> (Vec<Unit>, HashMap<String, Vec<String>>) {
    let all = git(repo, &["log", "--format=%H %P", rev]);
    let parents: HashMap<String, Vec<String>> = all
        .lines()
        .filter_map(|l| {
            let mut it = l.split(' ');
            Some((it.next()?.to_string(), it.map(String::from).collect()))
        })
        .collect();
    let fp = git(repo, &["log", "--first-parent", "--reverse", "--format=%H%x00%P%x00%ct%x00%s%x00%b%x1e", rev]);
    let mut seen: HashSet<String> = HashSet::new();
    let mut units = vec![];
    let pr_re = regex::Regex::new(r"(?:Merge pull request|Merge PR) #(\d+)(?: from \S+?/(\S+))?").unwrap();
    let squash_re = regex::Regex::new(r"\(#(\d+)\)\s*$").unwrap();
    let int_re = regex::Regex::new(r"\(#(\d+)\)").unwrap();
    for rec in fp.split('\x1e') {
        let p: Vec<&str> = rec.trim_start_matches('\n').splitn(5, '\0').collect();
        if p.len() < 4 {
            continue;
        }
        let (h, ps, ts, subj, body) = (p[0], p[1], p[2].parse().unwrap_or(0), p[3], p.get(4).copied().unwrap_or(""));
        let ps: Vec<&str> = ps.split(' ').filter(|s| !s.is_empty()).collect();
        seen.insert(h.to_string());
        if ps.len() >= 2 {
            let mut q: VecDeque<String> = ps[1..].iter().map(|s| s.to_string()).collect();
            let mut got = vec![];
            while let Some(c) = q.pop_front() {
                if !seen.insert(c.clone()) {
                    continue;
                }
                let pp = parents.get(&c).cloned().unwrap_or_default();
                if pp.len() < 2 {
                    got.push(c.clone());
                }
                q.extend(pp);
            }
            let (id, title, br) = if let Some(m) = pr_re.captures(subj) {
                let t = body.lines().find(|l| !l.trim().is_empty()).unwrap_or(subj).to_string();
                (format!("pr:{}", &m[1]), t, m.get(2).map(|b| b.as_str().trim_start_matches("codeberg/").to_string()))
            } else if let Some(m) = int_re.captures(subj) {
                (format!("pr:{}", &m[1]), subj.to_string(), None)
            } else {
                (format!("merge:{}", &h[..8]), subj.to_string(), None)
            };
            if !got.is_empty() {
                units.push(Unit { id, title, branch: br, ts, commits: got });
            }
        } else if let Some(m) = squash_re.captures(subj) {
            let title = squash_re.replace(subj, "").trim().to_string();
            let br = find_pr(prs, m[1].parse().unwrap_or(0), &title).map(|p| p.branch.trim_start_matches("codeberg/").to_string());
            units.push(Unit { id: format!("pr:{}", &m[1]), title, branch: br, ts, commits: vec![h.into()] });
        } else {
            units.push(Unit { id: format!("c:{}", &h[..8]), title: subj.to_string(), branch: None, ts, commits: vec![h.into()] });
        }
    }
    (units, parents)
}

/// 每个提交首次出现在哪个 tag。ignore 里的 tag（通配）不算版本，比如存档用的 `archive/**`
pub fn versions(repo: &Path, parents: &HashMap<String, Vec<String>>, ignore: &[String]) -> (Vec<(String, i64)>, HashMap<String, String>) {
    let skip: Vec<regex::Regex> = ignore.iter().filter_map(|p| regex::Regex::new(&tag_re(p)).ok()).collect();
    let tags = git(repo, &["for-each-ref", "--sort=creatordate", "--format=%(refname:short)%00%(objectname)%00%(*objectname)%00%(creatordate:unix)", "refs/tags"]);
    let mut seen: HashSet<String> = HashSet::new();
    let mut of: HashMap<String, String> = HashMap::new();
    let mut list = vec![];
    for l in tags.lines() {
        let p: Vec<&str> = l.split('\0').collect();
        if p.len() < 4 {
            continue;
        }
        if skip.iter().any(|r| r.is_match(p[0])) {
            continue;
        }
        let target = if p[2].is_empty() { p[1] } else { p[2] };
        list.push((p[0].to_string(), p[3].parse().unwrap_or(0)));
        let mut q = VecDeque::from([target.to_string()]);
        while let Some(c) = q.pop_front() {
            if !seen.insert(c.clone()) {
                continue;
            }
            of.insert(c.clone(), p[0].to_string());
            q.extend(parents.get(&c).cloned().unwrap_or_default());
        }
    }
    (list, of)
}

/// 存活代码：HEAD 上每一行来自哪个提交。只 blame 文本文件；按 blob 缓存
pub fn blame_head(repo: &Path, rev: &str, cache: &mut HashMap<String, Vec<BlameLine>>) -> Vec<(String, Vec<BlameLine>)> {
    let text: HashSet<String> = git(repo, &["grep", "-I", "-l", "-e", "", rev]).lines().filter_map(|l| l.split_once(':').map(|(_, p)| p)).map(String::from).collect();
    let tree = git(repo, &["ls-tree", "-r", rev]);
    let files: Vec<(String, String)> = tree
        .lines()
        .filter_map(|l| {
            let (meta, path) = l.split_once('\t')?;
            let m: Vec<&str> = meta.split(' ').collect();
            (m.get(1) == Some(&"blob") && !excluded(path) && text.contains(path)).then(|| (m[2].to_string(), path.to_string()))
        })
        .collect();
    let todo: Vec<(String, String)> = files.iter().filter(|(b, _)| !cache.contains_key(b)).cloned().collect();
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(10);
    let chunks: Vec<Vec<(String, String)>> = todo.chunks(todo.len().div_ceil(n).max(1)).map(|c| c.to_vec()).collect();
    let rev_s = rev.to_string();
    let results: Vec<Vec<(String, Vec<BlameLine>)>> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let rev_s = rev_s.clone();
                s.spawn(move || chunk.into_iter().map(|(blob, path)| (blob, blame_file(repo, &rev_s, &path))).collect::<Vec<_>>())
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for r in results.into_iter().flatten() {
        cache.insert(r.0, r.1);
    }
    files.into_iter().map(|(b, p)| (p, cache.get(&b).cloned().unwrap_or_default())).collect()
}

fn blame_file(repo: &Path, rev: &str, path: &str) -> Vec<BlameLine> {
    let out = git(repo, &["blame", "--porcelain", rev, "--", path]);
    let mut res = vec![];
    let (mut commit, mut file) = (String::new(), String::new());
    let mut files: HashMap<String, String> = HashMap::new();
    for l in out.lines() {
        if let Some(code) = l.strip_prefix('\t') {
            let rel = files.get(&commit).cloned().unwrap_or_else(|| file.clone());
            let n = normalize(code, &ext_of(&rel));
            let key = if n.chars().count() < 4 { 0 } else { line_key(&rel, &n) };
            res.push(BlameLine { commit: commit.clone(), rel, key });
        } else if let Some(f) = l.strip_prefix("filename ") {
            file = f.to_string();
            files.insert(commit.clone(), file.clone());
        } else {
            let mut it = l.split(' ');
            if let Some(h) = it.next() {
                if h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
                    commit = h.to_string();
                }
            }
        }
    }
    res
}

/// 历史里出现过的全部路径 + 当前树，供日志路径映射
pub fn known_paths(repo: &Path, rev: &str) -> HashSet<String> {
    let mut known: HashSet<String> = git(repo, &["log", "--format=", "--name-only", rev]).lines().filter(|l| !l.is_empty()).map(String::from).collect();
    known.extend(git(repo, &["ls-tree", "-r", "--name-only", rev]).lines().map(String::from));
    known
}

/// 用 gh 只读拉取 PR 元数据（没有 gh 或未登录就返回空）
pub fn github_prs(repo: &Path) -> Vec<PrMeta> {
    let out = Command::new("gh")
        .current_dir(repo)
        .args(["pr", "list", "--state", "all", "--limit", "1000", "--json", "number,headRefName,title,reviews,author"])
        .output();
    let Ok(o) = out else { return vec![] };
    if !o.status.success() {
        return vec![];
    }
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|x| PrMeta {
                    source: "github".into(),
                    number: x["number"].as_u64().unwrap_or(0),
                    title: x["title"].as_str().unwrap_or("").into(),
                    branch: x["headRefName"].as_str().unwrap_or("").into(),
                    author: x["author"]["login"].as_str().map(String::from),
                    reviews: x["reviews"].as_array().map(|r| r.iter().filter_map(|y| y["author"]["login"].as_str().map(String::from)).collect()).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `.mailmap` 支持交给 git 本身：`%ae`/`%an` 已经应用 mailmap 吗？没有，用 `%aE`/`%aN` 才会。
/// 这里提供一个读取函数，把 mailmap 的邮箱映射并进 config 的 director 邮箱。
pub fn mailmap_emails(repo: &Path, canonical: &str) -> Vec<String> {
    let s = std::fs::read_to_string(repo.join(".mailmap")).unwrap_or_default();
    let mut out = vec![];
    for l in s.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        // 形式：Proper Name <proper@email> [Commit Name] <commit@email>
        let emails: Vec<&str> = l.split('<').skip(1).filter_map(|x| x.split('>').next()).collect();
        if emails.first().map(|e| e.eq_ignore_ascii_case(canonical)).unwrap_or(false) {
            out.extend(emails.iter().skip(1).map(|e| e.to_lowercase()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{glob_re, tag_re};
    #[test]
    fn globs() {
        let m = |p: &str, f: &str| regex::Regex::new(&glob_re(p)).unwrap().is_match(f);
        assert!(m("*.pb.go", "api/v1/x.pb.go"));
        assert!(m("vendor/**", "vendor/a/b.c"));
        assert!(!m("vendor/**", "src/vendor.rs"));
        assert!(m("/dist/*.js", "dist/app.js"));
        assert!(!m("/dist/*.js", "web/dist/app.js"));
        assert!(m("package-lock.json", "web/package-lock.json"));
    }
    #[test]
    fn ignored_tags() {
        let m = |p: &str, t: &str| regex::Regex::new(&tag_re(p)).unwrap().is_match(t);
        assert!(m("archive/**", "archive/site-pre-mobile"));
        assert!(m("archive/**", "archive/a/b"));
        assert!(!m("archive/**", "v1.2.0"));
        assert!(!m("archive/**", "release/archive/x"));
        assert!(m("nightly-*", "nightly-2026-10-13"));
        assert!(!m("nightly-*", "v1-nightly-2"));
    }
}
