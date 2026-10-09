//! git adapter：提交、逐行新增内容（只取哈希）、PR 划分、版本、存活代码

use crate::hash::{ext_of, line_key, normalize};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(["-c", "core.quotepath=false"]).args(args).output().expect("git");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FileAdd {
    pub rel: String,
    /// 每个新增行的哈希键；0 表示过短或空行（不作证据）
    pub keys: Vec<u32>,
    pub binary: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Commit {
    pub hash: String,
    pub email: String,
    #[serde(default)]
    pub name: String,
    pub author_ts: i64,
    pub commit_ts: i64,
    pub subject: String,
    pub coauthors: Vec<String>,
    pub files: Vec<FileAdd>,
}

/// 默认排除：lockfile、构建产物、压缩产物
pub fn excluded(rel: &str) -> bool {
    let f = rel.rsplit('/').next().unwrap_or(rel);
    matches!(f, "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "Package.resolved" | "Cargo.lock" | "bun.lockb")
        || rel.contains("/dist/")
        || rel.starts_with("dist/")
        || rel.contains("/build/")
        || rel.contains("node_modules/")
        || f.contains(".min.")
}

const FMT: &str = "@@@C%x00%H%x00%ae%x00%an%x00%at%x00%ct%x00%s%x00%(trailers:key=Co-authored-by,valueonly,separator=|)";

/// 解析 `git log -p` 输出。`range` 为空表示全量
pub fn read_commits(repo: &Path, branch: &str, range: Option<&str>) -> Vec<Commit> {
    let spec = range.map(|r| r.to_string()).unwrap_or_else(|| branch.to_string());
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false", "log", "--no-merges", "-p", "-M", "--unified=0", "--no-color", "--no-ext-diff"])
        .arg(format!("--format={FMT}"))
        .arg(&spec)
        .stdout(Stdio::piped())
        .spawn()
        .expect("git log");
    let rd = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
    let mut out: Vec<Commit> = vec![];
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
            out.push(Commit {
                hash: p[0].into(),
                email: p[1].to_lowercase(),
                name: p[2].into(),
                author_ts: p[3].parse().unwrap_or(0),
                commit_ts: p[4].parse().unwrap_or(0),
                subject: p[5].into(),
                coauthors: p.get(6).map(|s| s.split('|').filter(|x| !x.is_empty()).map(|x| x.to_lowercase()).collect()).unwrap_or_default(),
                ..Default::default()
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
            // 形如 Binary files /dev/null and b/x.png differ
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
    out
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Unit {
    pub id: String,
    pub title: String,
    pub branch: Option<String>,
    pub ts: i64,
    pub commits: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct PrMeta {
    pub source: String,
    pub number: u64,
    pub title: String,
    pub branch: String,
    pub author: Option<String>,
    #[serde(default)]
    pub reviews: Vec<String>,
}

/// squash 提交的标题是「PR 标题 (#N)」：先按编号 + 标题匹配（GitHub、Codeberg 编号会重叠），再退回只按 GitHub 编号
fn find_pr<'a>(prs: &'a [PrMeta], num: u64, title: &str) -> Option<&'a PrMeta> {
    prs.iter().find(|p| p.number == num && p.title.trim() == title.trim()).or_else(|| prs.iter().find(|p| p.number == num && p.source == "github"))
}

/// 按第一父链划分「项」：PR merge、squash 的 (#N) 提交、其余直接提交
pub fn units(repo: &Path, branch: &str, prs: &[PrMeta]) -> (Vec<Unit>, HashMap<String, Vec<String>>) {
    let all = git(repo, &["log", "--format=%H %P", branch]);
    let parents: HashMap<String, Vec<String>> = all
        .lines()
        .filter_map(|l| {
            let mut it = l.split(' ');
            Some((it.next()?.to_string(), it.map(String::from).collect()))
        })
        .collect();
    let fp = git(repo, &["log", "--first-parent", "--reverse", "--format=%H%x00%P%x00%ct%x00%s%x00%b%x1e", branch]);
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
            // 第二父及其祖先中尚未出现过的非 merge 提交
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

/// 每个提交首次出现在哪个 tag
pub fn versions(repo: &Path, parents: &HashMap<String, Vec<String>>) -> (Vec<(String, i64)>, HashMap<String, String>) {
    let tags = git(repo, &["for-each-ref", "--sort=creatordate", "--format=%(refname:short)%00%(objectname)%00%(*objectname)%00%(creatordate:unix)", "refs/tags"]);
    let mut seen: HashSet<String> = HashSet::new();
    let mut of: HashMap<String, String> = HashMap::new();
    let mut list = vec![];
    for l in tags.lines() {
        let p: Vec<&str> = l.split('\0').collect();
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

/// 存活代码：HEAD 上每一行来自哪个提交、该提交里的哪一行（路径 + 哈希键）
#[derive(Serialize, Deserialize, Clone)]
pub struct BlameLine {
    pub commit: String,
    pub rel: String,
    pub key: u32,
}

pub fn blame_head(repo: &Path, cache: &mut HashMap<String, Vec<BlameLine>>) -> Vec<(String, Vec<BlameLine>)> {
    // 只 blame 文本文件（git grep -I 跳过二进制）
    let text: HashSet<String> = git(repo, &["grep", "-I", "-l", "-e", "", "HEAD"]).lines().filter_map(|l| l.strip_prefix("HEAD:")).map(String::from).collect();
    let tree = git(repo, &["ls-tree", "-r", "HEAD"]);
    let files: Vec<(String, String)> = tree
        .lines()
        .filter_map(|l| {
            let (meta, path) = l.split_once('\t')?;
            let m: Vec<&str> = meta.split(' ').collect();
            (m.get(1) == Some(&"blob") && !excluded(path) && text.contains(path)).then(|| (m[2].to_string(), path.to_string()))
        })
        .collect();
    let todo: Vec<(String, String)> = files.iter().filter(|(b, _)| !cache.contains_key(b)).cloned().collect();
    let chunks: Vec<Vec<(String, String)>> = todo.chunks(todo.len().div_ceil(10).max(1)).map(|c| c.to_vec()).collect();
    let results: Vec<Vec<(String, Vec<BlameLine>)>> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|chunk| s.spawn(move || chunk.into_iter().map(|(blob, path)| (blob, blame_file(repo, &path))).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for r in results.into_iter().flatten() {
        cache.insert(r.0, r.1);
    }
    files.into_iter().map(|(b, p)| (p, cache.get(&b).cloned().unwrap_or_default())).collect()
}

fn blame_file(repo: &Path, path: &str) -> Vec<BlameLine> {
    let out = git(repo, &["blame", "--porcelain", "HEAD", "--", path]);
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
