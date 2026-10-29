//! git adapter：提交、逐行新增内容（只取哈希）、PR 划分、版本、存活代码、盖章 trailer

use creditia_core::hash::{ext_of, line_key, normalize};
use creditia_core::model::{excluded, BlameFile, Commit, FileAdd, PrMeta, Unit, JOIN_WINDOWS};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(["-c", "core.quotepath=false"]).args(args).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

/// Resolve only an existing commit. Plain rev-parse can echo an invalid name
/// to stdout while exiting unsuccessfully; that text is not a commit hash.
pub fn resolve_commit(repo: &Path, rev: &str) -> Option<String> {
    let commit = format!("{rev}^{{commit}}");
    let out = Command::new("git").arg("-C").arg(repo)
        .args(["rev-parse", "--verify", "--end-of-options", &commit]).output().ok()?;
    if !out.status.success() { return None; }
    let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if head.is_empty() { None } else { Some(head) }
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
const FMT: &str = "@@@C%x00%H%x00%ae%x00%an%x00%at%x00%ct%x00%s%x00%(trailers:key=Co-authored-by,valueonly,separator=%x7C)%x00%(trailers:key=Creditia-Harness,valueonly,separator=%x7C)%x00%(trailers:key=Creditia-Model,valueonly,separator=%x7C)%x00%(trailers:key=Assisted-by,valueonly,separator=%x7C)%x00%(trailers:key=Creditia-Lines,valueonly,separator=%x7C)%x00%ce%x00%ai";

/// `Creditia-Lines: claude-code/claude-opus-5-5 30, codex 4 / 120` → ([(工具/模型, 行数)], 总行数)。没有 AI 行时写成 `/ 120`
pub fn parse_lines_stamp(v: &str) -> Option<(Vec<(String, usize)>, usize)> {
    let (parts, total) = v.trim().rsplit_once('/')?;
    let total: usize = total.trim().parse().ok()?;
    let mut by = vec![];
    for p in parts.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, n) = p.rsplit_once(char::is_whitespace)?;
        by.push((k.trim().to_string(), n.trim().parse().ok()?));
    }
    Some((by, total))
}

/// 盖章表：提交哈希 → (工具, 模型)
pub type Stamps = HashMap<String, (String, Option<String>)>;

/// 正文里找到的盖章和署名
#[derive(Default, Debug, PartialEq)]
pub struct BodyStamps {
    /// 各条 `Creditia-Lines` 加起来
    pub lines: Option<(Vec<(String, usize)>, usize)>,
    /// 出现最多的 `Creditia-Harness`（和它后面的 `Creditia-Model`）
    pub harness: Option<(String, Option<String>)>,
    pub coauthors: Vec<String>,
    pub assisted: Vec<String>,
}

/// GitHub 网页上 squash 合并时，原分支每个提交的说明连同它们的盖章、`Co-authored-by` 都被并进正文，
/// 只有最后一段还算 trailer。读整个正文（也认 `* ` / `- ` 开头的列表行），把它们都找回来
pub fn body_stamps(body: &str) -> BodyStamps {
    let mut out = BodyStamps::default();
    let mut by: Vec<(String, usize)> = vec![];
    let mut total = 0;
    let mut found = false;
    let mut hs: Vec<(String, Option<String>)> = vec![];
    for l in body.lines() {
        let l = l.trim().trim_start_matches(['*', '-']).trim();
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "creditia-lines" => {
                if let Some((parts, t)) = parse_lines_stamp(v) {
                    found = true;
                    total += t;
                    for (id, n) in parts {
                        match by.iter_mut().find(|(x, _)| *x == id) {
                            Some(e) => e.1 += n,
                            None => by.push((id, n)),
                        }
                    }
                }
            }
            "creditia-harness" if !v.is_empty() => hs.push((creditia_core::model::canonical_harness(v), None)),
            "creditia-model" if !v.is_empty() => {
                if let Some(h) = hs.last_mut().filter(|h| h.1.is_none()) {
                    h.1 = Some(v.to_string());
                }
            }
            "co-authored-by" if !v.is_empty() => {
                let v = v.to_lowercase();
                if !out.coauthors.contains(&v) {
                    out.coauthors.push(v);
                }
            }
            "assisted-by" if !v.is_empty() && !out.assisted.iter().any(|a| a == v) => out.assisted.push(v.to_string()),
            _ => {}
        }
    }
    if found {
        out.lines = Some((by, total));
    }
    let count = |h: &str| hs.iter().filter(|x| x.0 == h).count();
    out.harness = hs.iter().max_by_key(|x| (count(&x.0), x.1.is_some())).cloned();
    out
}

/// 网页合并的提交（提交者是 GitHub）：正文里原分支提交的盖章和署名补进来
fn web_bodies(repo: &Path, spec: &str, commits: &mut [Commit], stamps: &mut Stamps) {
    if !commits.iter().any(|c| c.web) {
        return;
    }
    let log = git(repo, &["log", "--no-merges", "--committer=noreply@github.com", "--format=%H%x00%B%x1e", spec]);
    let bodies: HashMap<&str, &str> = log.split('\x1e').filter_map(|r| r.trim_start().split_once('\0')).collect();
    for c in commits.iter_mut().filter(|c| c.web) {
        let Some(b) = bodies.get(c.hash.as_str()) else { continue };
        let bs = body_stamps(b);
        if bs.lines.is_some() {
            c.lines_stamp = bs.lines;
        }
        for co in bs.coauthors {
            if !c.coauthors.contains(&co) {
                c.coauthors.push(co);
            }
        }
        for a in bs.assisted {
            if !c.assisted.contains(&a) {
                c.assisted.push(a);
            }
        }
        if let Some(h) = bs.harness {
            stamps.entry(c.hash.clone()).or_insert(h);
        }
    }
}

/// 解析 `git log -p` 输出。`range` 为空表示全量到 `rev`。返回提交和盖章表
pub fn read_commits(repo: &Path, rev: &str, range: Option<&str>) -> (Vec<Commit>, Stamps) {
    read_commits_checked(repo, rev, range).unwrap_or_default()
}

/// Recovery caches must never persist a partially read Git history.
pub(crate) fn read_commits_checked(repo: &Path, rev: &str, range: Option<&str>) -> Result<(Vec<Commit>, Stamps), String> {
    let spec = range.map(|r| r.to_string()).unwrap_or_else(|| rev.to_string());
    let total: usize = git(repo, &["rev-list", "--count", "--no-merges", &spec]).trim().parse().unwrap_or(0);
    creditia_core::progress::tick("history", 0, total);
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false", "log", "--no-merges", "-p", "-M", "--unified=0", "--no-color", "--no-ext-diff", "--no-textconv"])
        .arg(format!("--format={FMT}"))
        .arg(&spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Err("cannot start Git history read".into());
    };
    let rd = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
    let (mut commits, mut stamps) = parse_log(rd, total);
    if !child.wait().map_err(|e| e.to_string())?.success() { return Err("Git history read failed".into()); }
    web_bodies(repo, &spec, &mut commits, &mut stamps);
    Ok((commits, stamps))
}

/// 暂存区（`git diff --cached`）当成一个还没提交的提交：提交盖章 hook 用它算这次提交里各工具写了几行
pub fn staged_commit(repo: &Path, email: &str, ts: i64) -> Commit {
    let diff = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false", "diff", "--cached", "-M", "--unified=0", "--no-color", "--no-ext-diff"])
        .stderr(Stdio::null())
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    let head = format!("@@@C\0staged\0{email}\0\0{ts}\0{ts}\0\0\0\0\0\n");
    let (mut out, _) = parse_log(head.as_bytes().chain(&diff[..]), 0);
    out.pop().unwrap_or(Commit { hash: "staged".into(), email: email.into(), author_ts: ts, commit_ts: ts, ..Default::default() })
}

fn parse_log(rd: impl BufRead, total: usize) -> (Vec<Commit>, Stamps) {
    let mut out: Vec<Commit> = vec![];
    let mut stamps = HashMap::new();
    let mut cur: Option<FileAdd> = None;
    let mut ext = String::new();
    let mut newline: u32 = 0;
    // 文件头（diff --git 到第一个 @@）里才认 `+++ ` / `Binary files`：新增的内容行本身以 `++ ` 开头时，diff 里也是 `+++ `
    let mut in_header = false;
    // 同一改动块里最近几行规范化后的文本，拆行识别用
    let mut win: Vec<String> = vec![];
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
                stamps.insert(hash.clone(), (creditia_core::model::canonical_harness(h), model));
            }
            let lines_stamp = p.get(10).and_then(|s| s.split('|').next()).and_then(parse_lines_stamp);
            creditia_core::progress::tick("history", out.len() + 1, total);
            out.push(Commit {
                hash,
                email: p[1].to_lowercase(),
                name: p[2].into(),
                author_ts: p[3].parse().unwrap_or(0),
                commit_ts: p[4].parse().unwrap_or(0),
                subject: p[5].into(),
                coauthors: p[6].split('|').filter(|x| !x.is_empty()).map(|x| x.to_lowercase()).collect(),
                assisted: p.get(9).map(|s| s.split('|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default(),
                lines_stamp,
                web: p.get(11).is_some_and(|e| e.trim().eq_ignore_ascii_case("noreply@github.com")),
                // `%ai`：2026-10-02 22:55:10 +0800
                tz: p.get(12).and_then(|s| s.trim().rsplit(' ').next()).and_then(creditia_core::util::parse_offset).unwrap_or(0),
                files: vec![],
            });
        } else if line.starts_with("diff --git ") {
            flush(&mut out, &mut cur);
            in_header = true;
        } else if let Some(p) = line.strip_prefix("+++ ").filter(|_| in_header) {
            let rel = p.strip_prefix("b/").unwrap_or(p).trim_matches('"').to_string();
            if rel != "/dev/null" && !excluded(&rel) {
                ext = ext_of(&rel);
                cur = Some(FileAdd { rel, ..Default::default() });
            }
        } else if line.starts_with("Binary files ") && in_header {
            if let Some(b) = line.split(" and ").nth(1) {
                let rel = b.trim_end_matches(" differ").trim_start_matches("b/").to_string();
                if rel != "/dev/null" && !excluded(&rel) {
                    cur = Some(FileAdd { rel, binary: true, ..Default::default() });
                }
            }
        } else if line.starts_with("@@ ") {
            in_header = false;
            win.clear();
            if let Some(f) = cur.as_mut() {
                f.hunks.push(f.keys.len() as u32);
                // `@@ -a,b +c,d @@`：新文件从第 c 行起
                newline = line.split(' ').nth(2).and_then(|p| p.trim_start_matches('+').split(',').next()?.parse().ok()).unwrap_or(0);
            }
        } else if let Some(a) = line.strip_prefix('+') {
            if let Some(f) = cur.as_mut() {
                let n = normalize(a, &ext);
                let long = n.chars().count() >= 4;
                f.keys.push(if long { line_key(&f.rel, &n) } else { 0 });
                f.short.push(if long { 0 } else { creditia_core::hash::short_hash(&n) });
                f.at.push(newline);
                newline += 1;
                // 以前面 1~3 行开头、到这一行结束的拼接：记在窗口第一行的位置上
                let i = f.keys.len() - 1;
                f.joins.extend([0u32; JOIN_WINDOWS]);
                for w in 1..=JOIN_WINDOWS.min(win.len()) {
                    let joined: String = win[win.len() - w..].concat() + &n;
                    f.joins[(i - w) * JOIN_WINDOWS + (w - 1)] = line_key(&f.rel, &joined);
                }
                win.push(n);
                if win.len() > JOIN_WINDOWS {
                    win.remove(0);
                }
            }
        }
    }
    flush(&mut out, &mut cur);
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

/// 每个提交首次出现在哪个版本节点。只认分析分支的祖先，按提交拓扑而非打标签时间划分。
/// ignore 里的 tag（通配）不算版本，比如存档用的 `archive/**`。
pub type VersionAttribution = (Vec<(String, i64)>, HashMap<String, String>);

pub fn versions(repo: &Path, rev: &str, parents: &HashMap<String, Vec<String>>, ignore: &[String]) -> Result<VersionAttribution, String> {
    let read = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git").arg("-C").arg(repo).args(args).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("cannot read version tags: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let skip: Vec<regex::Regex> = ignore.iter().filter_map(|p| regex::Regex::new(&tag_re(p)).ok()).collect();
    let tags = read(&["for-each-ref", &format!("--merged={rev}"), "--sort=creatordate", "--format=%(refname:strip=2)%00%(objectname)%00%(*objectname)%00%(creatordate:unix)%00%(*objecttype)", "refs/tags"])?;
    let mut eligible = vec![];
    for l in tags.lines() {
        let p: Vec<&str> = l.split('\0').collect();
        if p.len() < 5 || skip.iter().any(|r| r.is_match(p[0])) { continue; }
        let target = if p[4] == "tag" {
            read(&["rev-parse", "--verify", &format!("refs/tags/{}^{{commit}}", p[0])])?.trim().to_string()
        } else if p[2].is_empty() { p[1].to_string() } else { p[2].to_string() };
        if parents.contains_key(&target) {
            eligible.push((p[0].to_string(), p[3].parse::<i64>().unwrap_or(0), target));
        }
    }
    if eligible.is_empty() { return Ok((vec![], HashMap::new())); }
    // 旧版本后来补打标签，仍应先分到旧版本；同一提交的别名以创建时间、名字稳定排序。
    let history = read(&["log", "--topo-order", "--reverse", "--format=%H", rev])?;
    let order: HashMap<&str, usize> = history.lines().enumerate().map(|(i, h)| (h, i)).collect();
    eligible.sort_by(|a, b| order.get(a.2.as_str()).cmp(&order.get(b.2.as_str())).then(a.1.cmp(&b.1)).then(a.0.cmp(&b.0)));
    let mut seen: HashSet<String> = HashSet::new();
    let mut of: HashMap<String, String> = HashMap::new();
    let mut list = vec![];
    for (tag, date, target) in eligible {
        list.push((tag.clone(), date));
        let mut q = VecDeque::from([target]);
        while let Some(c) = q.pop_front() {
            if !seen.insert(c.clone()) {
                continue;
            }
            of.insert(c.clone(), tag.clone());
            q.extend(parents.get(&c).cloned().unwrap_or_default());
        }
    }
    Ok((list, of))
}

/// 存活代码：HEAD 上每一行来自哪个提交。只 blame 文本文件；按 blob 缓存
/// HEAD 上每个文本文件的 blame（按 blob 缓存）。几个线程从同一个队列里取文件；
/// 跑的过程中每隔一分钟调一次 `save`，第一次分析大仓库时中途取消，下次接着跑
pub fn blame_head(repo: &Path, rev: &str, cache: &mut HashMap<String, BlameFile>, save: &mut dyn FnMut(&HashMap<String, BlameFile>)) -> Vec<(String, BlameFile)> {
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
    // 缓存按「blob 路径」存：内容一样的两个文件路径不同，行哈希和 blame 也不同
    let ck = |b: &str, p: &str| format!("{b} {p}");
    // 只留 HEAD 上还在的文件，缓存不会越攒越大
    let live: HashSet<String> = files.iter().map(|(b, p)| ck(b, p)).collect();
    cache.retain(|k, _| live.contains(k));
    let mut todo: Vec<(String, String)> = files.iter().filter(|(b, p)| !cache.contains_key(&ck(b, p))).cloned().collect();
    // 创建以后再没改过的文件不用跑 blame
    if todo.len() > 50 {
        let fresh = unchanged_since_created(repo, rev, &todo);
        if !fresh.is_empty() {
            for (k, bf) in fresh {
                cache.insert(k, bf);
            }
            todo.retain(|(b, p)| !cache.contains_key(&ck(b, p)));
            save(cache);
        }
    }
    if !todo.is_empty() {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(10);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel::<(String, BlameFile)>();
        let ck = &ck;
        std::thread::scope(|s| {
            for _ in 0..n {
                let (tx, next, todo) = (tx.clone(), &next, &todo);
                s.spawn(move || loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((blob, path)) = todo.get(i) else { break };
                    if tx.send((ck(blob, path), blame_file(repo, rev, path))).is_err() {
                        break;
                    }
                });
            }
            drop(tx);
            let mut saved = std::time::Instant::now();
            let mut done = 0;
            creditia_core::progress::tick("blame", 0, todo.len());
            for (k, bf) in rx {
                cache.insert(k, bf);
                done += 1;
                creditia_core::progress::tick("blame", done, todo.len());
                if saved.elapsed().as_secs() >= 60 {
                    save(cache);
                    saved = std::time::Instant::now();
                }
            }
        });
    }
    files.into_iter().map(|(b, p)| { let bf = cache.get(&ck(&b, &p)).cloned().unwrap_or_default(); (p, bf) }).collect()
}

/// 创建以后再没改过的文件：HEAD 上的 blob 正是某个提交新建这个文件时写下的（不是改名、复制来的），
/// 那每一行都出自那个提交、行号也一样，和 git blame 的结果相同，却只要一遍 `git log --raw` 加读一次文件内容。
/// 大仓库里这样的文件常常占一大半。返回 (「blob 路径」, blame)
fn unchanged_since_created(repo: &Path, rev: &str, todo: &[(String, String)]) -> Vec<(String, BlameFile)> {
    let want: HashSet<(&str, &str)> = todo.iter().map(|(b, p)| (b.as_str(), p.as_str())).collect();
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=false", "log", "--raw", "--no-abbrev", "--no-merges", "-M", "--format=@@%H", rev])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return vec![];
    };
    // (blob, 路径) → 新建它的提交。git log 从新到旧，每个路径只看最近的那一次改动：
    // 它是新建、而且写下的正是 HEAD 上的 blob 才算。最近一次是改名、复制过来的，或者改过又改回原样的，
    // git blame 会算到别的提交上，这些照常跑 blame
    // 新建它的那个提交里还删了、改名了别的文件时也不算：git blame 可能把它当成从那个文件改名来的
    let mut created: HashMap<(String, String), String> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut commit = String::new();
    let (mut pending, mut moved): (Vec<(String, String)>, bool) = (vec![], false);
    let mut flush = |commit: &str, pending: &mut Vec<(String, String)>, moved: bool| {
        for k in pending.drain(..) {
            if !moved {
                created.insert(k, commit.to_string());
            }
        }
    };
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(h) = line.strip_prefix("@@") {
            flush(&commit, &mut pending, moved);
            moved = false;
            commit = h.to_string();
        } else if let Some(meta) = line.strip_prefix(':') {
            let Some((m, paths)) = meta.split_once('\t') else { continue };
            // 改名、复制是「旧路径<TAB>新路径」，看新路径
            let path = paths.rsplit('\t').next().unwrap_or(paths);
            let f: Vec<&str> = m.split(' ').collect();
            if f.len() >= 5 && (f[4] == "D" || f[4].starts_with('R') || f[4].starts_with('C')) {
                moved = true;
            }
            if !seen.insert(path.to_string()) {
                continue;
            }
            if f.len() >= 5 && f[4] == "A" && want.contains(&(f[3], path)) {
                pending.push((f[3].to_string(), path.to_string()));
            }
        }
    }
    flush(&commit, &mut pending, moved);
    let _ = child.wait();
    if created.is_empty() {
        return vec![];
    }
    // 一次 cat-file --batch 读出这些文件的内容
    let Ok(mut cat) = Command::new("git").arg("-C").arg(repo).args(["cat-file", "--batch"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
        return vec![];
    };
    let list: Vec<(String, String, String)> = created.into_iter().map(|((b, p), c)| (b, p, c)).collect();
    let mut stdin = cat.stdin.take().unwrap();
    let ids: Vec<String> = list.iter().map(|(b, _, _)| b.clone()).collect();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        for id in ids {
            if writeln!(stdin, "{id}").is_err() {
                break;
            }
        }
    });
    let mut r = BufReader::new(cat.stdout.take().unwrap());
    let mut out = vec![];
    for (blob, path, commit) in &list {
        let mut head = String::new();
        if r.read_line(&mut head).unwrap_or(0) == 0 {
            break;
        }
        // `<id> missing`：没有内容可读，跳过（这个文件照常跑 blame）
        let Some(size) = head.split(' ').nth(2).and_then(|x| x.trim().parse::<usize>().ok()) else { continue };
        let mut buf = vec![0u8; size + 1];
        if std::io::Read::read_exact(&mut r, &mut buf).is_err() {
            break;
        }
        buf.pop();
        let text = String::from_utf8_lossy(&buf);
        let ext = ext_of(path);
        let mut bf = BlameFile::default();
        for (i, l) in text.lines().enumerate() {
            let n = normalize(l, &ext);
            let key = if n.chars().count() < 4 { 0 } else { line_key(path, &n) };
            bf.push(commit, path, key, i as u32 + 1);
        }
        out.push((format!("{blob} {path}"), bf));
    }
    let _ = writer.join();
    let _ = cat.wait();
    out
}

fn blame_file(repo: &Path, rev: &str, path: &str) -> BlameFile {
    let out = git(repo, &["blame", "--porcelain", rev, "--", path]);
    let mut res = BlameFile::default();
    let (mut commit, mut file, mut orig) = (String::new(), String::new(), 0u32);
    let mut files: HashMap<String, String> = HashMap::new();
    for l in out.lines() {
        if let Some(code) = l.strip_prefix('\t') {
            let rel = files.get(&commit).cloned().unwrap_or_else(|| file.clone());
            let n = normalize(code, &ext_of(&rel));
            let key = if n.chars().count() < 4 { 0 } else { line_key(&rel, &n) };
            res.push(&commit, &rel, key, orig);
        } else if let Some(f) = l.strip_prefix("filename ") {
            file = f.to_string();
            files.insert(commit.clone(), file.clone());
        } else {
            let mut it = l.split(' ');
            if let Some(h) = it.next() {
                if h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
                    commit = h.to_string();
                    orig = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                }
            }
        }
    }
    res
}

/// 历史里出现过的全部路径 + 当前树 + 暂存区和工作区里还没提交的文件，供日志路径映射。
/// 只认历史的话，agent 新建的文件在提交之前读到的日志里映射不上，日志又按指纹缓存，提交以后也补不回来
pub fn known_paths(repo: &Path, rev: &str) -> HashSet<String> {
    let mut known: HashSet<String> = git(repo, &["log", "--format=", "--name-only", rev]).lines().filter(|l| !l.is_empty()).map(String::from).collect();
    known.extend(git(repo, &["ls-tree", "-r", "--name-only", rev]).lines().map(String::from));
    known.extend(git(repo, &["-c", "core.quotepath=false", "ls-files", "--cached", "--others", "--exclude-standard"]).lines().filter(|l| !l.is_empty()).map(String::from));
    known
}

/// 用 gh 只读拉取 PR 元数据（没有 gh 或未登录就返回空）
pub fn github_prs(repo: &Path) -> Vec<PrMeta> {
    let Some(repository) = crate::squash::github_repository(repo) else { return vec![]; };
    let out = crate::command::output(Command::new("gh")
        .current_dir(repo)
        .args(["pr", "list", "--repo", &format!("github.com/{repository}"), "--state", "all", "--limit", "1000", "--json", "number,headRefName,title,reviews,author,mergedAt,mergeCommit"]), std::time::Instant::now() + std::time::Duration::from_secs(15));
    let Ok(o) = out else { return vec![] };
    let v: serde_json::Value = serde_json::from_str(&o).unwrap_or_default();
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
                    merge_commit: x["mergedAt"].as_str().and_then(|_| x["mergeCommit"]["oid"].as_str()).map(String::from),
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
    use super::*;
    #[test]
    fn squash_body_stamps() {
        // GitHub 网页 squash 合并的正文：每个原提交一段，盖章在中间，最后一段才是 trailer
        let body = "* add parser\n\nCreditia-Harness: codex\nCreditia-Lines: codex/gpt-6 20, claude-code/claude-opus-5-5 5 / 40\n\n* fix\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* docs\n\nCreditia-Harness: Codex CLI\nCreditia-Model: gpt-6\nCreditia-Lines: codex/gpt-6 3 / 10\n\nCo-authored-by: me <me@users.noreply.github.com>";
        let b = body_stamps(body);
        assert_eq!(b.lines, Some((vec![("codex/gpt-6".into(), 23), ("claude-code/claude-opus-5-5".into(), 5)], 50)));
        assert_eq!(b.harness, Some(("codex".into(), Some("gpt-6".into()))));
        assert_eq!(b.coauthors, vec!["claude opus 5.5 <noreply@anthropic.com>".to_string(), "me <me@users.noreply.github.com>".to_string()]);
        assert_eq!(body_stamps("just a message"), BodyStamps::default());
    }
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


    /// 跳过 blame 的捷径和真跑 git blame 结果一致。CREDITIA_BLAME_CHECK=<仓库> 时拿真仓库对照（手动跑）
    #[test]
    #[ignore]
    fn unchanged_files_match_real_blame() {
        let Some(repo) = std::env::var_os("CREDITIA_BLAME_CHECK") else { return };
        let repo = Path::new(&repo);
        let tree = git(repo, &["ls-tree", "-r", "HEAD"]);
        let todo: Vec<(String, String)> = tree.lines().filter_map(|l| { let (m, p) = l.split_once('\t')?; let f: Vec<&str> = m.split(' ').collect(); (f[1] == "blob").then(|| (f[2].to_string(), p.to_string())) }).collect();
        let fast = unchanged_since_created(repo, "HEAD", &todo);
        let mut bad = 0;
        let n: usize = std::env::var("CREDITIA_BLAME_CHECK_N").ok().and_then(|x| x.parse().ok()).unwrap_or(400);
        for (k, bf) in fast.iter().take(n) {
            let path = k.split_once(' ').unwrap().1;
            let real = blame_file(repo, "HEAD", path);
            let a: Vec<_> = bf.iter().map(|l| (l.commit.to_string(), l.key, l.orig)).collect();
            let b: Vec<_> = real.iter().map(|l| (l.commit.to_string(), l.key, l.orig)).collect();
            if a != b {
                bad += 1;
                eprintln!("differs: {path} {:?}", a.iter().zip(&b).find(|(x, y)| x != y).map(|(x, y)| (x.clone(), y.clone())).or(Some(((String::new(), a.len() as u32, 0), (String::new(), b.len() as u32, 0)))));
            }
        }
        eprintln!("checked {} of {} unchanged files ({} in tree)", fast.len().min(n), fast.len(), todo.len());
        assert_eq!(bad, 0);
    }
}
