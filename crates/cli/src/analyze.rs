//! 分析流水线：git → 日志证据 → 归因 → 存活代码 → credits.json。增量：提交按 base..head，日志按文件指纹。

use crate::detect_config;
use creditia_adapters as ad;
use creditia_core::config::Config;
use creditia_core::engine::{self, Engine};
use creditia_core::model::{area, vendor, BlameFile, Commit, CommitAttr, FileGroup, Index, Level, PathMap, PrMeta, Session, Src, Store, STORE_VERSION, UNATTR};
use creditia_core::util::{date_at, date_local, now_iso, week_at};
use creditia_store::{cache_dir, load_json, save_json, Library, Project};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct Opts {
    pub fresh: bool,
    pub github: bool,
    pub out: Option<PathBuf>,
    pub blame: Option<(String, usize)>,
    /// 用仓库外的配置文件（测试、不想往仓库里写的时候）
    pub config: Option<PathBuf>,
    /// 覆盖配置里的口径：strict / standard / loose
    pub level: Option<String>,
    /// 命令是在哪个目录里运行的（worktree、临时克隆），登记成本项目的路径
    pub invoked: Option<PathBuf>,
}

pub struct Report {
    pub name: String,
    pub head: String,
    pub commits: usize,
    pub new_commits: usize,
    pub units: usize,
    pub lines: usize,
    pub secs: f64,
    pub out: PathBuf,
    pub tier: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub blame: Option<Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CommitCache {
    /// 提交缓存的格式版本：读法变了（比如开始记改动块）就整份重读
    #[serde(default)]
    version: u32,
    head: Option<String>,
    commits: Vec<Commit>,
    #[serde(default)]
    stamps: HashMap<String, (String, Option<String>)>,
}

const COMMIT_CACHE_VERSION: u32 = 7;

/// 读配置：指定了就用指定的；仓库里有就用仓库的；都没有就现场探测（不落盘）。空着的字段自动补
pub fn load_config(repo: &Path, override_path: Option<&Path>) -> Config {
    let p = override_path.map(|x| x.to_path_buf()).unwrap_or(repo.join(".creditia/config.json"));
    let mut c = Config::load(&p).unwrap_or_else(|_| detect_config(repo));
    c.repo = repo.display().to_string();
    if c.name.is_empty() {
        c.name = crate::project_name(repo);
    }
    if c.remotes.is_empty() {
        c.remotes = ad::git::identity(repo).1;
    }
    if c.markers.is_empty() {
        c.markers = crate::derive_markers(repo, &c.remotes);
    }
    if c.exclude_markers.is_empty() {
        c.exclude_markers = crate::detect_excludes(repo, &c.remotes);
    }
    c
}

fn analysis_head(repo: &Path, revision: &str) -> Result<String, String> {
    ad::git::resolve_commit(repo, revision).ok_or_else(|| {
        if ad::git::resolve_commit(repo, "HEAD").is_none() {
            creditia_core::tr!("仓库没有提交", "the repository has no commits")
        } else {
            creditia_core::tr!("无法解析分析版本：{revision}；请检查 branch 配置", "cannot resolve analysis revision: {revision}; check the branch configuration")
        }
    })
}

/// 项目身份：根提交 + 远程地址。同一仓库的不同克隆、worktree、搬过家的目录认作同一个项目；
/// 根提交相同但远程地址完全不同的（fork）分开记
pub fn project_of(repo: &Path, cfg: &Config, lib: Option<&Library>) -> Project {
    let (root, remotes) = ad::git::identity(repo);
    let base = if root.is_empty() { cfg.name.clone() } else { root[..12.min(root.len())].to_string() };
    let mut id = base.clone();
    if !root.is_empty() {
        let same: Vec<Project> = lib.and_then(|l| l.projects().ok()).unwrap_or_default().into_iter().filter(|p| p.root_commit == root).collect();
        let fits = |p: &Project| p.remotes.is_empty() || remotes.is_empty() || p.remotes.iter().any(|r| remotes.contains(r));
        if let Some(p) = same.iter().find(|p| fits(p)) {
            id = p.id.clone();
        } else if !same.is_empty() {
            // fork：根提交一样，远程不一样
            let h = remotes.first().map(|r| r.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))).unwrap_or(0);
            id = format!("{base}-{:06x}", h & 0xffffff);
        }
    }
    Project { id, name: cfg.name.clone(), path: repo.display().to_string(), root_commit: root, remotes, last_head: None, last_analyzed: None }
}

/// 本项目所有已知的根路径：当前目录、各 worktree、作品库登记过的路径、配置里手写的路径
pub fn known_roots(repo: &Path, cfg: &Config, lib: Option<&Library>, project: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![repo.display().to_string()];
    v.extend(ad::git::worktrees(repo));
    if let Some(l) = lib {
        v.extend(l.paths(project).unwrap_or_default().into_iter().map(|(p, _)| p));
    }
    v.extend(cfg.paths.iter().cloned());
    let mut seen = HashSet::new();
    v.retain(|p| !p.is_empty() && seen.insert(p.trim_end_matches('/').to_string()));
    v
}

fn path_map(repo: &Path, head: &str, cfg: &Config, roots: &[String]) -> PathMap {
    let mut markers: Vec<String> = cfg.markers.iter().map(|m| m.to_lowercase()).collect();
    for r in roots {
        let m = format!("{}/", r.replace('\\', "/").trim_end_matches('/').to_lowercase());
        if m.len() > 4 && !markers.contains(&m) {
            markers.push(m);
        }
    }
    PathMap { known: ad::git::known_paths(repo, head), markers, excludes: cfg.exclude_markers.iter().map(|m| m.to_lowercase()).collect() }
}

fn scan_all(store: &mut Store, pm: &PathMap, roots: &[String], st: &mut ad::Stats) {
    ad::codex::scan(store, pm, st);
    ad::claude::scan(store, pm, st);
    ad::cursor::scan(store, pm, st);
    ad::grok::scan(store, pm, st);
    ad::kimi::scan(store, pm, st);
    ad::gemini::scan(store, pm, roots, st);
    ad::qwen::scan(store, pm, st);
    ad::opencode::scan(store, pm, st);
    ad::cline::scan(store, pm, st);
    ad::aider::scan(store, pm, roots, st);
    ad::copilot::scan(store, pm, st);
    ad::amp::scan(store, pm, st);
    ad::trae::scan(store, pm, st);
    ad::crush::scan(store, pm, roots, st);
    ad::goose::scan(store, pm, st);
    ad::cont::scan(store, pm, st);
    ad::zed::scan(store, pm, st);
    ad::amazonq::scan(store, pm, st);
    ad::antigravity::scan(store, pm, st);
}

/// 把这次见到的路径登记进作品库：以后这些目录删了、日志里还留着它们，也能认出是本项目
fn register_paths(lib: &Library, project: &str, repo: &Path, invoked: Option<&Path>) {
    let _ = lib.add_path(project, &repo.display().to_string(), "main");
    for w in ad::git::worktrees(repo) {
        let _ = lib.add_path(project, &w, "worktree");
    }
    if let Some(p) = invoked {
        if p != repo {
            let _ = lib.add_path(project, &p.display().to_string(), "clone");
        }
    }
}

/// collect：只采集证据进证据库
pub fn collect(repo: &Path, lib: &mut Library, config: Option<&Path>) -> Result<(usize, Vec<String>, String), String> {
    let cfg = load_config(repo, config);
    let proj = project_of(repo, &cfg, Some(lib));
    let head = analysis_head(repo, &cfg.branch)?;
    lib.upsert_project(&proj)?;
    register_paths(lib, &proj.id, repo, None);
    let roots = known_roots(repo, &cfg, Some(lib), &proj.id);
    let pm = path_map(repo, &head, &cfg, &roots);
    let cache = cache_dir(&proj.id);
    let mut store: Store = load_json(&cache.join("store.json"));
    if store.version != STORE_VERSION {
        store = Store { version: STORE_VERSION, ..Default::default() };
    }
    store.rescan_if_markers_changed(&pm.markers);
    let mut st = ad::Stats::default();
    scan_all(&mut store, &pm, &roots, &mut st);
    save_json(&cache.join("store.json"), &store)?;
    let n = lib.save_evidence(&proj.id, &store)?;
    Ok((n, st.warnings, proj.id))
}

/// 证据：日志（增量扫描）+ 证据库（日志被清理后仍在）
fn evidence(repo: &Path, cfg: &Config, lib: &mut Library, project: &str, head: &str) -> Result<(Store, ad::Stats), String> {
    let cache = cache_dir(project);
    let roots = known_roots(repo, cfg, Some(lib), project);
    let pm = path_map(repo, head, cfg, &roots);
    let mut store: Store = load_json(&cache.join("store.json"));
    if store.version != STORE_VERSION {
        store = Store { version: STORE_VERSION, ..Default::default() };
    }
    store.rescan_if_markers_changed(&pm.markers);
    let mut st = ad::Stats::default();
    creditia_core::progress::tick("logs", 0, 0);
    scan_all(&mut store, &pm, &roots, &mut st);
    save_json(&cache.join("store.json"), &store)?;
    lib.save_evidence(project, &store)?;
    let (db_evs, db_sessions) = lib.load_evidence(project)?;
    // 合并：证据库里有、当前日志里没有的（已清理）补进来
    // 日志还在的会话以这次解析为准（读法可能改进过，比如不再把编辑的上下文行记成证据），只补日志已经没了的会话；
    // Cursor 的行哈希表会单行清理，仍按行补
    let mut have: HashSet<(String, u32, String, String)> = HashSet::new();
    let mut live: HashSet<(String, String)> = HashSet::new();
    for g in store.groups.values() {
        for e in &g.evs {
            have.insert((e.rel.clone(), e.key, e.src.harness().into(), e.session.clone()));
        }
        for s in &g.sessions {
            if s.has_content && s.src != Src::Cursor {
                live.insert((s.src.harness().into(), s.id.clone()));
            }
        }
    }
    let mut extra = FileGroup::default();
    for e in db_evs {
        if live.contains(&(e.src.harness().to_string(), e.session.clone())) {
            continue;
        }
        if !have.contains(&(e.rel.clone(), e.key, e.src.harness().into(), e.session.clone())) {
            extra.evs.push(e);
        }
    }
    let have_s: HashSet<(String, String)> = store.groups.values().flat_map(|g| g.sessions.iter().map(|s| (s.src.harness().to_string(), s.id.clone()))).collect();
    for s in db_sessions {
        if !have_s.contains(&(s.src.harness().to_string(), s.id.clone())) {
            extra.sessions.push(s);
        }
    }
    let restored = extra.evs.len();
    if restored > 0 {
        st.notes.push(creditia_core::tr!("证据库补回 {restored} 条已不在日志里的证据", "restored {restored} pieces of evidence from the library that are no longer in the logs"));
        store.groups.insert("library".into(), extra);
    }
    Ok((store, st))
}

/// 暂存区里各工具写了几行（提交盖章 hook 用）：返回 (贡献者 id, 行数) 和暂存区新增的总行数
pub fn staged(repo: &Path) -> Result<(Vec<(String, usize)>, usize), String> {
    let cfg = load_config(repo, None);
    let mut lib = Library::open()?;
    let proj = project_of(repo, &cfg, Some(&lib));
    lib.upsert_project(&proj)?;
    let head = ad::git::git(repo, &["rev-parse", "HEAD"]).trim().to_string();
    // 这次提交的作者和时间：和 git 自己用的一样（认 GIT_AUTHOR_DATE、--date）
    let ident = ad::git::git(repo, &["var", "GIT_AUTHOR_IDENT"]);
    let email = ident.split_once('<').and_then(|(_, r)| r.split_once('>')).map(|(e, _)| e.trim().to_lowercase()).unwrap_or_default();
    // 没配身份时 git var 会报错：只为拿时间再问一次
    let ident = if ident.trim().is_empty() { ad::git::git(repo, &["-c", "user.name=-", "-c", "user.email=-", "var", "GIT_AUTHOR_IDENT"]) } else { ident };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let ts = ident.rsplit_once('>').and_then(|(_, r)| r.split_whitespace().next()?.parse().ok()).unwrap_or(now);
    let c = ad::git::staged_commit(repo, &email, ts);
    let total: usize = c.files.iter().filter(|f| !f.binary).map(|f| f.keys.len()).sum();
    if total == 0 {
        return Ok((vec![], 0));
    }
    let (store, _) = evidence(repo, &cfg, &mut lib, &proj.id, &head)?;
    let idx = Index::build(&store);
    let sessions: Vec<Session> = store.groups.values().flat_map(|g| g.sessions.clone()).collect();
    let mut eng = Engine::new(&cfg, &idx, &sessions, &[], HashMap::new());
    let a = eng.attribute(&c);
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for (f, lines) in c.files.iter().zip(&a.lines) {
        if f.binary {
            continue;
        }
        for x in lines {
            let w = &eng.reg.list[x.who];
            if w.kind == "agent" {
                *by.entry(w.id.trim_start_matches("a:").to_string()).or_default() += 1;
            }
        }
    }
    let mut v: Vec<(String, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok((v, total))
}

pub fn run(repo: &Path, opts: &Opts) -> Result<Report, String> {
    let t0 = Instant::now();
    let mut cfg = load_config(repo, opts.config.as_deref());
    if let Some(lv) = &opts.level {
        cfg.level = lv.clone();
    }
    let mut lib = Library::open()?;
    let proj = project_of(repo, &cfg, Some(&lib));
    lib.upsert_project(&proj)?;
    register_paths(&lib, &proj.id, repo, opts.invoked.as_deref());
    let cache = cache_dir(&proj.id);
    if opts.fresh {
        let _ = std::fs::remove_dir_all(&cache);
    }
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let mut warnings = vec![];

    // ---- git：提交（增量）
    let head = analysis_head(repo, &cfg.branch)?;
    let mut cc: CommitCache = load_json(&cache.join("commits.json"));
    if cc.version != COMMIT_CACHE_VERSION {
        cc = CommitCache { version: COMMIT_CACHE_VERSION, ..Default::default() };
    }
    let base = cc.head.clone().filter(|h| ad::git::git_ok(repo, &["merge-base", "--is-ancestor", h, &head]));
    let (new_commits, new_stamps) = if let Some(b) = base {
        if b == head { (vec![], HashMap::new()) } else { ad::git::read_commits(repo, &head, Some(&format!("{b}..{head}"))) }
    } else {
        cc.commits.clear();
        cc.stamps.clear();
        ad::git::read_commits(repo, &head, None)
    };
    let n_new = new_commits.len();
    cc.commits.extend(new_commits);
    // .gitattributes 里标成生成物、第三方的文件不算谁写的（linguist-generated / linguist-vendored / -diff）
    let gen = ad::git::generated_patterns(repo, &head);
    let gen_hit = |rel: &str| gen.iter().any(|re| re.is_match(rel));
    let mut skipped_generated = 0usize;
    if !gen.is_empty() {
        for c in cc.commits.iter_mut() {
            let before = c.files.len();
            c.files.retain(|f| !gen_hit(&f.rel));
            skipped_generated += before - c.files.len();
        }
    }
    cc.stamps.extend(new_stamps);
    cc.head = Some(head.clone());
    // 时区：你自己的提交里最常见的那个（不看这台电脑：CI 上多半是 UTC）；你没有提交就看全部提交
    let forced_tz = creditia_core::util::forced_offset();
    {
        let mine = cfg.director_emails();
        let mut n: HashMap<i64, usize> = HashMap::new();
        for c in cc.commits.iter().filter(|c| mine.contains(&c.email)) {
            *n.entry(c.tz).or_default() += 1;
        }
        if n.is_empty() {
            for c in &cc.commits {
                *n.entry(c.tz).or_default() += 1;
            }
        }
        if let Some((tz, _)) = n.into_iter().max_by_key(|&(tz, k)| (k, std::cmp::Reverse(tz))) {
            creditia_core::util::set_offset(tz);
        }
    }
    // 读历史是最慢的几步之一：读完先存下，后面被取消也不用重读
    if n_new > 0 {
        save_json(&cache.join("commits.json"), &cc)?;
    }

    // ---- PR 元数据（gh 只读；缓存）
    let prs_path = cache.join("prs.json");
    let mut prs: Vec<PrMeta> = load_json::<Vec<Value>>(&prs_path).into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
    if opts.github {
        let fresh = ad::git::github_prs(repo);
        if fresh.is_empty() {
            warnings.push(creditia_core::tr!("gh 没有返回 PR 列表（未安装、未登录或仓库不在 GitHub）", "gh returned no pull requests (not installed, not signed in, or the repository is not on GitHub)"));
        } else {
            prs = fresh;
            let v: Vec<Value> = prs.iter().map(|p| json!({ "source": p.source, "number": p.number, "title": p.title, "branch": p.branch, "author": p.author, "reviews": p.reviews })).collect();
            save_json(&prs_path, &v)?;
        }
    }
    let (units, parents) = ad::git::units(repo, &head, &prs);
    let (tags, ver_of) = ad::git::versions(repo, &head, &parents, &cfg.ignore_tags)?;

    // ---- 证据：日志（增量）+ 证据库（日志被清理后仍在）
    let (store, mut st) = evidence(repo, &cfg, &mut lib, &proj.id, &head)?;
    warnings.extend(st.warnings.clone());
    let idx = Index::build(&store);
    let sessions: Vec<Session> = store.groups.values().flat_map(|g| g.sessions.clone()).collect();

    // ---- 归因
    let mut eng = Engine::new(&cfg, &idx, &sessions, &units, cc.stamps.clone());
    let mut attrs: HashMap<String, CommitAttr> = HashMap::new();
    // 按时间先后归因：一条证据只认领一次，先提交的先用
    let mut order: Vec<&Commit> = cc.commits.iter().collect();
    order.sort_by_key(|c| (c.commit_ts, c.author_ts));
    let n_commits = order.len();
    for (i, c) in order.into_iter().enumerate() {
        attrs.insert(c.hash.clone(), eng.attribute(c));
        creditia_core::progress::tick("attribute", i + 1, n_commits);
    }
    // 你的提交里有未确认的行、提交前两周这台电脑上又没有这个项目的任何 AI 记录：多半是在别的电脑上写的
    {
        use creditia_core::engine::{WINDOW_AFTER, WINDOW_BEFORE};
        let mine = cfg.director_emails();
        let (mut n, mut lines) = (0usize, 0usize);
        for c in cc.commits.iter().filter(|c| mine.contains(&c.email)) {
            let un = attrs[&c.hash].lines.iter().flatten().filter(|a| a.level == Level::Unattributed).count();
            let (lo, hi) = (c.author_ts.min(c.commit_ts) - WINDOW_BEFORE, c.commit_ts.max(c.author_ts) + WINDOW_AFTER);
            if un > 0 && !sessions.iter().any(|s| !s.reviewer && s.end >= lo && s.start <= hi) {
                n += 1;
                lines += un;
            }
        }
        if n > 0 {
            st.notes.push(creditia_core::tr!(
                "你的 {n} 个提交（{lines} 行未确认）在提交前两周里这台电脑上没有这个项目的 AI 记录。如果是在别的电脑上写的：在那台电脑上运行 creditia evidence export，再在这里 creditia evidence import <文件>",
                "{n} of your commits ({lines} unattributed lines) have no AI records for this project on this computer in the two weeks before them. If they were written on another computer, run creditia evidence export there, then creditia evidence import <file> here"
            ));
        }
    }

    // ---- 存活代码
    // blame.json、blame2.json 是旧格式的缓存
    for old in ["blame.json", "blame2.json"] {
        let _ = std::fs::remove_file(cache.join(old));
    }
    let mut bc: HashMap<String, BlameFile> = load_json(&cache.join("blame3.json"));
    let blame_path = cache.join("blame3.json");
    let mut blame = ad::git::blame_head(repo, &head, &mut bc, &mut |c| {
        let _ = save_json(&blame_path, c);
    });
    if !gen.is_empty() {
        blame.retain(|(p, _)| !gen_hit(p));
    }
    if skipped_generated > 0 {
        st.notes.push(creditia_core::tr!(".gitattributes 标为生成物、第三方的文件改动 {skipped_generated} 处不计入", "{skipped_generated} changes to files marked generated or vendored in .gitattributes are not counted"));
    }
    let commit_by_hash: HashMap<&str, &Commit> = cc.commits.iter().map(|c| (c.hash.as_str(), c)).collect();
    let unattr = eng.reg.unattributed();
    let surv = engine::survival(&blame, &commit_by_hash, &attrs, unattr);

    // 准确率基准用：逐行归因写到 CREDITIA_DUMP_LINES 指定的文件（含行内容哈希之外的一切，不含代码原文）
    if let Some(p) = std::env::var_os("CREDITIA_DUMP_LINES") {
        let lv = |l: Level| serde_json::to_value(l).unwrap_or_default();
        let hist: Vec<Value> = cc
            .commits
            .iter()
            .map(|c| {
                let a = &attrs[&c.hash];
                let files: Vec<Value> = c.files.iter().zip(&a.lines).filter(|(f, _)| !f.binary).map(|(f, fl)| json!({ "rel": f.rel, "who": fl.iter().map(|x| &eng.reg.list[x.who].id).collect::<Vec<_>>(), "lv": fl.iter().map(|x| lv(x.level)).collect::<Vec<_>>() })).collect();
                json!({ "commit": c.hash, "files": files })
            })
            .collect();
        let mut i = 0;
        let mut sv = vec![];
        for (path, lines) in &blame {
            let s = &surv[i..i + lines.len()];
            i += lines.len();
            sv.push(json!({ "path": path, "who": s.iter().map(|x| &eng.reg.list[x.who].id).collect::<Vec<_>>(), "lv": s.iter().map(|x| lv(x.level)).collect::<Vec<_>>() }));
        }
        let _ = std::fs::write(p, json!({ "history": hist, "survival": sv }).to_string());
    }

    // ---- 逐行溯源
    let mut blame_out = None;
    if let Some((fpath, ln)) = &opts.blame {
        let mut i = 0;
        for (path, lines) in &blame {
            if path == fpath {
                if let Some(b) = lines.get(ln.saturating_sub(1)) {
                    let a = surv[i + ln - 1];
                    let w = &eng.reg.list[a.who];
                    let c = commit_by_hash.get(b.commit);
                    let ev = idx.map.get(&(b.rel.to_string(), b.key)).and_then(|v| {
                        let ts = c.map(|c| c.author_ts).unwrap_or(0);
                        v.iter().filter(|e| e.ts <= ts + 3600).max_by_key(|e| e.ts)
                    });
                    let unit = units.iter().find(|u| u.commits.iter().any(|x| x == b.commit));
                    blame_out = Some(json!({
                        "file": fpath, "line": ln, "contributor": w.id, "harness": w.harness, "model": w.model, "name": w.name,
                        "evidence": a.level, "commit": &b.commit[..10.min(b.commit.len())], "date": c.map(|c| date_local(c.author_ts)),
                        "subject": c.map(|c| if cfg.privacy.hide_titles { String::new() } else { c.subject.clone() }),
                        "unit": unit.map(|u| u.id.clone()), "unitTitle": unit.map(|u| if cfg.privacy.hide_titles { String::new() } else { u.title.clone() }),
                        "session": ev.map(|e| e.session.clone()), "sessionTime": ev.map(|e| e.ts), "tzOffset": creditia_core::util::offset(),
                    }));
                }
                break;
            }
            i += lines.len();
        }
    }

    // ---- 汇总
    let reg = &eng.reg;
    let seg_of = |ts: i64| -> String {
        let d = date_local(ts);
        if cfg.segments.is_empty() {
            return d[..7].to_string();
        }
        cfg.segments.iter().find(|(_, a, b)| d >= *a && d <= *b).map(|s| s.0.clone()).unwrap_or("-".into())
    };
    #[derive(Default, Clone)]
    struct Agg {
        long: usize,
        verified: usize,
        /// 这段里最强的证据等级（多少行是逐行证据看 verifiedShare）
        best: Option<Level>,
        from: String,
        to: String,
    }
    let mut by_who: BTreeMap<usize, BTreeMap<Level, usize>> = BTreeMap::new();
    let mut by_area: BTreeMap<usize, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut by_ver: BTreeMap<usize, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_week: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_day: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut seg: BTreeMap<String, Agg> = BTreeMap::new();
    let mut binary_files: BTreeMap<String, usize> = BTreeMap::new();
    let (mut conflicts, mut tab_total) = (0usize, 0usize);
    let mut doc_lines_director = 0usize;
    let mut clock = vec![vec![0usize; 24]; 7];
    // 按贡献者看：提交者 id → (git 作者名, 是不是你, 归因 id → 行数)
    let mut authors: BTreeMap<String, (String, bool, BTreeMap<String, usize>)> = BTreeMap::new();
    for c in &cc.commits {
        let a = &attrs[&c.hash];
        let (aid, aname, me) = eng.author_of(c);
        let au = &mut authors.entry(aid).or_insert_with(|| (aname, me, BTreeMap::new())).2;
        // 按天、按周、作息：提交者当地的时间（`CREDITIA_TZ` 指定了就都按它）
        let off = forced_tz.unwrap_or(c.tz);
        let sg = seg_of(c.author_ts);
        let d = date_at(c.author_ts, off);
        let wk = week_at(c.author_ts, off);
        let ver = ver_of.get(&c.hash).cloned().unwrap_or("unreleased".into());
        conflicts += a.conflicts;
        tab_total += a.tab_lines;
        let t = c.author_ts + off;
        let wd = ((t.div_euclid(86400) + 3).rem_euclid(7)) as usize;
        clock[wd][(t.rem_euclid(86400) / 3600) as usize] += 1;
        let e = seg.entry(sg.clone()).or_default();
        if e.from.is_empty() || d < e.from {
            e.from = d.clone();
        }
        if d > e.to {
            e.to = d.clone();
        }
        for (f, fl) in c.files.iter().zip(&a.lines) {
            if f.binary {
                let g = a.main.map(|w| reg.group(w)).unwrap_or(UNATTR.into());
                *binary_files.entry(g).or_default() += 1;
                continue;
            }
            let ar = area(&f.rel, false);
            for (&k, at) in f.keys.iter().zip(fl) {
                *by_who.entry(at.who).or_default().entry(at.level).or_default() += 1;
                *au.entry(reg.list[at.who].id.clone()).or_default() += 1;
                *by_area.entry(at.who).or_default().entry(ar).or_default() += 1;
                *by_ver.entry(at.who).or_default().entry(ver.clone()).or_default() += 1;
                *by_week.entry(wk.clone()).or_default().entry(reg.list[at.who].id.clone()).or_default() += 1;
                *by_day.entry(d.clone()).or_default().entry(reg.fine(at.who)).or_default() += 1;
                let e = seg.entry(sg.clone()).or_default();
                if k != 0 {
                    e.long += 1;
                    if at.level == Level::Verified {
                        e.verified += 1;
                    }
                    if at.level != Level::Unattributed {
                        e.best = Some(e.best.map_or(at.level, |b| b.min(at.level)));
                    }
                }
                if reg.list[at.who].kind == "human" && ar == "docs" {
                    doc_lines_director += 1;
                }
            }
        }
    }
    let mut surv_by: BTreeMap<usize, usize> = BTreeMap::new();
    for a in &surv {
        *surv_by.entry(a.who).or_default() += 1;
    }
    let mut files_surv: Vec<Value> = vec![];
    {
        let mut i = 0;
        for (path, lines) in &blame {
            let mut g: BTreeMap<String, usize> = BTreeMap::new();
            // 按行顺序的分段：[[贡献方 id, 连续行数], …]，代码地图用（只有归属，没有代码；id 带模型）
            let mut runs: Vec<(&str, usize)> = vec![];
            for _ in lines.iter() {
                *g.entry(reg.fine(surv[i].who)).or_default() += 1;
                let id = reg.list[surv[i].who].id.as_str();
                match runs.last_mut() {
                    Some((last, n)) if *last == id => *n += 1,
                    _ => runs.push((id, 1)),
                }
                i += 1;
            }
            if !lines.is_empty() {
                let mut v = json!({ "lines": lines.len(), "by": g, "runs": runs });
                if !cfg.privacy.hide_paths {
                    v["path"] = json!(path);
                }
                files_surv.push(v);
            }
        }
    }

    // 项
    let hide_t = cfg.privacy.hide_titles;
    let mut unit_json = vec![];
    for u in &units {
        let mut cnt: BTreeMap<usize, (usize, Level)> = BTreeMap::new();
        let mut areas: HashSet<&str> = HashSet::new();
        let mut total = 0usize;
        for h in &u.commits {
            let (Some(c), Some(a)) = (commit_by_hash.get(h.as_str()), attrs.get(h)) else { continue };
            for (f, fl) in c.files.iter().zip(&a.lines) {
                areas.insert(area(&f.rel, f.binary));
                for at in fl {
                    let e = cnt.entry(at.who).or_insert((0, at.level));
                    e.0 += 1;
                    e.1 = e.1.min(at.level);
                    total += 1;
                }
            }
        }
        if total == 0 {
            continue;
        }
        let mut shares: Vec<Value> = cnt
            .iter()
            .map(|(w, (n, lv))| json!({ "contributor": reg.list[*w].id, "share": (*n as f64 / total as f64 * 1000.0).round() / 1000.0, "evidence": lv }))
            .collect();
        shares.sort_by(|a, b| b["share"].as_f64().partial_cmp(&a["share"].as_f64()).unwrap());
        let mut areas: Vec<&str> = areas.into_iter().collect();
        areas.sort();
        let ver = u.commits.iter().find_map(|h| ver_of.get(h)).cloned();
        let title = if hide_t { if u.id.starts_with("pr:") { format!("PR #{}", &u.id[3..]) } else { u.commits.first().map(|h| h.chars().take(7).collect()).unwrap_or_default() } } else { u.title.clone() };
        unit_json.push(json!({ "id": u.id, "title": title, "date": date_local(u.ts), "version": ver, "commits": u.commits, "areas": areas, "shares": shares, "lines": total }));
    }

    // 贡献方
    let hide_names = cfg.display.hide_names;
    let reviews = sessions.iter().filter(|s| s.reviewer).count();
    let mut contributors = vec![];
    // 每个 (工具, 模型) 最常见的渠道
    let mut prov: HashMap<(String, String), BTreeMap<String, usize>> = HashMap::new();
    for s2 in &sessions {
        if let Some(p) = &s2.provider {
            *prov.entry((s2.src.harness().to_string(), creditia_core::model::normalize_model(&s2.model))).or_default().entry(p.clone()).or_default() += 1;
        }
    }
    for (i, w) in reg.list.iter().enumerate() {
        if !by_who.contains_key(&i) {
            continue;
        }
        let mut roles = vec![];
        if w.kind != "unattributed" {
            roles.push("author");
        }
        let mut c = json!({ "id": w.id, "kind": w.kind, "roles": roles });
        if let Some(h) = &w.harness {
            c["harness"] = json!(h);
            c["model"] = json!(w.model);
            c["family"] = json!(w.model.as_deref().map(vendor));
            c["where"] = json!("local");
            if let Some(p) = w.model.as_ref().and_then(|m| prov.get(&(h.clone(), m.clone()))).and_then(|m| m.iter().max_by_key(|(_, n)| **n)).map(|(p, _)| p.clone()) {
                c["provider"] = json!(p);
            }
        }
        // 不写名字时档案里也不留：credits.json 会公开
        if let Some(n) = w.name.as_ref().filter(|_| !hide_names) {
            c["name"] = json!(cfg.display_name(n));
        }
        contributors.push(c);
    }
    if reviews > 0 {
        let by_model: BTreeMap<String, usize> = sessions.iter().filter(|s| s.reviewer).fold(BTreeMap::new(), |mut m, s| {
            *m.entry(format!("{}/{}", s.src.harness(), creditia_core::model::normalize_model(&s.model))).or_default() += 1;
            m
        });
        for (k, n) in by_model {
            let (h, m) = k.split_once('/').unwrap();
            contributors.push(json!({ "id": format!("a:{h}/{m}"), "kind": "agent", "harness": h, "model": m, "roles": ["reviewer"], "reviews": n }));
        }
    }
    let breakdown: Vec<Value> = by_who
        .iter()
        .map(|(w, lv)| {
            let ev = |l: Level| lv.get(&l).copied().unwrap_or(0);
            json!({
                "contributor": reg.list[*w].id,
                "linesAdded": lv.values().sum::<usize>(),
                "linesSurviving": surv_by.get(w).copied().unwrap_or(0),
                "byEvidence": { "verified": { "linesAdded": ev(Level::Verified) }, "manual": { "linesAdded": ev(Level::Manual) }, "declared": { "linesAdded": ev(Level::Declared) }, "inferred": { "linesAdded": ev(Level::Inferred) }, "unattributed": { "linesAdded": ev(Level::Unattributed) } },
                "byArea": by_area.get(w), "byVersion": by_ver.get(w),
            })
        })
        .collect();
    let mut authors_json: Vec<(usize, Value)> = authors
        .iter()
        .map(|(id, (name, me, by))| {
            let n: usize = by.values().sum();
            let mut v = json!({ "id": id, "me": me, "linesAdded": n, "by": by });
            if !hide_names {
                v["name"] = json!(name);
            }
            (n, v)
        })
        .filter(|(n, _)| *n > 0)
        .collect();
    authors_json.sort_by(|a, b| b.0.cmp(&a.0).then(a.1["id"].as_str().cmp(&b.1["id"].as_str())));
    let timeline: Vec<Value> = by_week.iter().map(|(w, m)| json!({ "week": w, "byContributor": m })).collect();
    let sess_json: Vec<Value> = {
        let mut v: Vec<&Session> = sessions.iter().collect();
        v.sort_by_key(|s| s.start);
        v.iter()
            .map(|s| {
                let id = if s.model.is_empty() { format!("a:{}", s.src.harness()) } else { format!("a:{}/{}", s.src.harness(), creditia_core::model::normalize_model(&s.model)) };
                json!({ "contributor": id, "start": s.start, "end": s.end, "tokens": s.tokens, "review": s.reviewer, "prompts": s.prompts })
            })
            .collect()
    };
    let total_lines: usize = by_who.values().flat_map(|m| m.values()).sum();
    let tokens: i64 = sessions.iter().map(|s| s.tokens).sum();
    let days: HashSet<String> = cc.commits.iter().map(|c| date_at(c.author_ts, forced_tz.unwrap_or(c.tz))).collect();
    let first = cc.commits.iter().map(|c| c.author_ts).min().unwrap_or(0);
    let last = cc.commits.iter().map(|c| c.author_ts).max().unwrap_or(0);
    let prompts: i64 = sessions.iter().filter(|s| !s.reviewer).map(|s| s.prompts).sum();
    // GitHub PR review：bot 看账号形式（GitHub 的 bot 账号以 [bot] 结尾）和已知的 review 应用，其余都是人
    const REVIEW_APPS: [&str; 7] = ["copilot-pull-request-reviewer", "chatgpt-codex-connector", "coderabbitai", "gemini-code-assist", "cursor", "sourcery-ai", "greptile-apps"];
    let (human_reviews, bot_reviews) = prs.iter().flat_map(|p| p.reviews.iter()).fold((0usize, 0usize), |(h, b), r| {
        let r = r.to_lowercase();
        let is_bot = r.ends_with("[bot]") || REVIEW_APPS.contains(&r.as_str()) || r.starts_with("app/");
        if is_bot { (h, b + 1) } else { (h + 1, b) }
    });
    let lines_written: usize = store.groups.values().flat_map(|g| g.evs.iter()).filter(|e| !e.tab).map(|e| (e.rel.as_str(), e.key, e.session.as_str())).collect::<HashSet<_>>().len();
    let coverage: Vec<Value> = seg
        .iter()
        .map(|(n, s)| {
            let best = s.best.unwrap_or(Level::Unattributed);
            json!({ "name": n, "from": s.from, "to": s.to, "best": best, "verifiedShare": if s.long == 0 { 0.0 } else { (s.verified as f64 / s.long as f64 * 1000.0).round() / 1000.0 } })
        })
        .collect();
    let generated_at = now_iso();
    let credits = json!({
        "$schema": "https://creditia.dev/schema/v0.json",
        "schemaVersion": 0,
        "project": { "name": cfg.name, "repos": cfg.remotes, "range": { "from": date_local(first), "to": date_local(last) } },
        "generatedAt": generated_at,
        // 按天、会话时间用的时区（秒）；按天、按周、作息里提交按提交者当地时间
        "tzOffset": creditia_core::util::offset(),
        "level": cfg.level,
        "head": head,
        "versions": tags.iter().map(|(t, ts)| json!({ "tag": t, "date": date_local(*ts) })).collect::<Vec<_>>(),
        "coverage": coverage,
        "contributors": contributors,
        "totals": { "commits": cc.commits.len(), "units": unit_json.len(), "linesAdded": total_lines, "linesSurviving": surv.len(), "linesWritten": lines_written, "tokens": tokens, "sessions": sessions.iter().filter(|s| !s.reviewer).count(), "days": days.len(), "binaryFiles": binary_files.values().sum::<usize>() },
        "breakdown": breakdown,
        "authors": authors_json.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        "human": { "prompts": prompts, "plans": doc_lines_director, "reviews": human_reviews, "tabAssistedLines": tab_total, "agentReviews": reviews + bot_reviews },
        "timeline": timeline,
        "days": by_day.iter().map(|(d, m)| json!({ "date": d, "by": m })).collect::<Vec<_>>(),
        "clock": clock,
        "files": files_surv,
        "sessions": sess_json,
        "units": unit_json,
        "conflictLines": conflicts,
    });

    // ---- 写出
    let out = opts.out.clone().unwrap_or(repo.join(".creditia/credits.json"));
    if opts.blame.is_none() {
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&out, serde_json::to_string(&credits).unwrap()).map_err(|e| creditia_core::tr!("写入 {} 失败：{e}", "couldn't write {}: {e}", out.display()))?;
        lib.save_snapshot(&proj.id, &head, &generated_at, &credits.to_string())?;
    }
    save_json(&cache.join("commits.json"), &cc)?;
    save_json(&cache.join("blame3.json"), &bc)?;

    // 终端里的一行 tier
    // 分母是全部新增行：工具、贡献者（别人的提交）、未确认加起来 100%（同作品里的份额口径）
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for (w, lv) in &by_who {
        let who = &reg.list[*w];
        let label = match who.kind {
            "agent" => reg.group(*w),
            "human" if !hide_names => cfg.display_name(who.name.as_deref().unwrap_or(&who.id)),
            "human" => creditia_core::tr!("其他贡献者", "other contributors"),
            _ => creditia_core::tr!("未确认", "unattributed"),
        };
        *groups.entry(label).or_default() += lv.values().sum::<usize>();
    }
    let mut gl: Vec<(String, usize)> = groups.into_iter().collect();
    gl.sort_by_key(|g| std::cmp::Reverse(g.1));
    let all: usize = gl.iter().map(|(_, n)| *n).sum();
    let tier = gl.iter().map(|(k, n)| format!("{k} {:.1}%", *n as f64 / all.max(1) as f64 * 100.0)).collect::<Vec<_>>().join(" · ");
    Ok(Report { name: cfg.name.clone(), head, commits: cc.commits.len(), new_commits: n_new, units: unit_json.len(), lines: total_lines, secs: t0.elapsed().as_secs_f64(), out, tier, notes: st.notes, warnings, blame: blame_out })
}
