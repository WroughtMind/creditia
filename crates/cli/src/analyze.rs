//! 分析流水线：git → 日志证据 → 归因 → 存活代码 → credits.json。增量：提交按 base..head，日志按文件指纹。

use crate::detect_config;
use creditia_adapters as ad;
use creditia_core::config::Config;
use creditia_core::engine::{self, Engine};
use creditia_core::model::{area, vendor, Attr, BlameLine, Commit, CommitAttr, FileGroup, Index, Level, PathMap, PrMeta, Session, Store, STORE_VERSION, UNATTR};
use creditia_core::util::{date_cn, iso_week, now_iso};
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
    head: Option<String>,
    commits: Vec<Commit>,
    #[serde(default)]
    stamps: HashMap<String, (String, Option<String>)>,
}

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
    ad::opencode::scan(store, pm, st);
    ad::cline::scan(store, pm, st);
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
    let head = ad::git::git(repo, &["rev-parse", &cfg.branch]).trim().to_string();
    let head = if head.is_empty() { ad::git::git(repo, &["rev-parse", "HEAD"]).trim().to_string() } else { head };
    lib.upsert_project(&proj)?;
    register_paths(lib, &proj.id, repo, None);
    let roots = known_roots(repo, &cfg, Some(lib), &proj.id);
    let pm = path_map(repo, &head, &cfg, &roots);
    let cache = cache_dir(&proj.id);
    let mut store: Store = load_json(&cache.join("store.json"));
    if store.version != STORE_VERSION {
        store = Store { version: STORE_VERSION, ..Default::default() };
    }
    let mut st = ad::Stats::default();
    scan_all(&mut store, &pm, &roots, &mut st);
    save_json(&cache.join("store.json"), &store)?;
    let n = lib.save_evidence(&proj.id, &store)?;
    Ok((n, st.warnings, proj.id))
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
    let rev = cfg.branch.clone();
    let mut head = ad::git::git(repo, &["rev-parse", &rev]).trim().to_string();
    if head.is_empty() {
        head = ad::git::git(repo, &["rev-parse", "HEAD"]).trim().to_string();
    }
    if head.is_empty() {
        return Err("仓库没有提交".into());
    }
    let mut cc: CommitCache = load_json(&cache.join("commits.json"));
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

    // ---- PR 元数据（gh 只读；缓存）
    let prs_path = cache.join("prs.json");
    let mut prs: Vec<PrMeta> = load_json::<Vec<Value>>(&prs_path).into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
    if opts.github {
        let fresh = ad::git::github_prs(repo);
        if fresh.is_empty() {
            warnings.push("gh 没有返回 PR 列表（未安装、未登录或仓库不在 GitHub）".into());
        } else {
            prs = fresh;
            let v: Vec<Value> = prs.iter().map(|p| json!({ "source": p.source, "number": p.number, "title": p.title, "branch": p.branch, "author": p.author, "reviews": p.reviews })).collect();
            save_json(&prs_path, &v)?;
        }
    }
    let (units, parents) = ad::git::units(repo, &head, &prs);
    let (tags, ver_of) = ad::git::versions(repo, &parents);

    // ---- 证据：日志（增量）+ 证据库（日志被清理后仍在）
    let roots = known_roots(repo, &cfg, Some(&lib), &proj.id);
    let pm = path_map(repo, &head, &cfg, &roots);
    let mut store: Store = load_json(&cache.join("store.json"));
    if store.version != STORE_VERSION {
        store = Store { version: STORE_VERSION, ..Default::default() };
    }
    let mut st = ad::Stats::default();
    scan_all(&mut store, &pm, &roots, &mut st);
    save_json(&cache.join("store.json"), &store)?;
    warnings.extend(st.warnings.clone());
    lib.save_evidence(&proj.id, &store)?;
    let (db_evs, db_sessions) = lib.load_evidence(&proj.id)?;
    // 合并：证据库里有、当前日志里没有的（已清理）补进来
    let mut have: HashSet<(String, u32, String, String)> = HashSet::new();
    for g in store.groups.values() {
        for e in &g.evs {
            have.insert((e.rel.clone(), e.key, e.src.harness().into(), e.session.clone()));
        }
    }
    let mut extra = FileGroup::default();
    for e in db_evs {
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
        st.notes.push(format!("证据库补回 {restored} 条已不在日志里的证据"));
        store.groups.insert("library".into(), extra);
    }
    let idx = Index::build(&store);
    let sessions: Vec<Session> = store.groups.values().flat_map(|g| g.sessions.clone()).collect();

    // ---- 归因
    let mut eng = Engine::new(&cfg, &idx, &sessions, &units, cc.stamps.clone());
    let mut attrs: HashMap<String, CommitAttr> = HashMap::new();
    for c in &cc.commits {
        attrs.insert(c.hash.clone(), eng.attribute(c));
    }

    // ---- 存活代码
    let mut bc: HashMap<String, Vec<BlameLine>> = load_json(&cache.join("blame.json"));
    let mut blame = ad::git::blame_head(repo, &head, &mut bc);
    if !gen.is_empty() {
        blame.retain(|(p, _)| !gen_hit(p));
    }
    if skipped_generated > 0 {
        st.notes.push(format!(".gitattributes 标为生成物、第三方的文件改动 {skipped_generated} 处不计入"));
    }
    let commit_by_hash: HashMap<&str, &Commit> = cc.commits.iter().map(|c| (c.hash.as_str(), c)).collect();
    let per_commit: HashMap<String, (HashMap<(String, u32), Attr>, Option<Attr>)> = attrs
        .iter()
        .map(|(h, a)| {
            let c = commit_by_hash[h.as_str()];
            let mut m = HashMap::new();
            for (f, fl) in c.files.iter().zip(&a.lines) {
                for (&k, &at) in f.keys.iter().zip(fl) {
                    if k != 0 {
                        m.insert((f.rel.clone(), k), at);
                    }
                }
            }
            (h.clone(), (m, a.main.map(|w| Attr { who: w, level: Level::Declared })))
        })
        .collect();
    let unattr = eng.reg.unattributed();
    let surv = engine::survival(&blame, &per_commit, unattr);
    drop(per_commit);

    // ---- 逐行溯源
    let mut blame_out = None;
    if let Some((fpath, ln)) = &opts.blame {
        let mut i = 0;
        for (path, lines) in &blame {
            if path == fpath {
                if let Some(b) = lines.get(ln.saturating_sub(1)) {
                    let a = surv[i + ln - 1];
                    let w = &eng.reg.list[a.who];
                    let c = commit_by_hash.get(b.commit.as_str());
                    let ev = idx.map.get(&(b.rel.clone(), b.key)).and_then(|v| {
                        let ts = c.map(|c| c.author_ts).unwrap_or(0);
                        v.iter().filter(|e| e.ts <= ts + 3600).max_by_key(|e| e.ts)
                    });
                    let unit = units.iter().find(|u| u.commits.contains(&b.commit));
                    blame_out = Some(json!({
                        "file": fpath, "line": ln, "contributor": w.id, "harness": w.harness, "model": w.model, "name": w.name,
                        "evidence": a.level, "commit": &b.commit[..10.min(b.commit.len())], "date": c.map(|c| date_cn(c.author_ts)),
                        "subject": c.map(|c| if cfg.privacy.hide_titles { String::new() } else { c.subject.clone() }),
                        "unit": unit.map(|u| u.id.clone()), "unitTitle": unit.map(|u| if cfg.privacy.hide_titles { String::new() } else { u.title.clone() }),
                        "session": ev.map(|e| e.session.clone()), "sessionTime": ev.map(|e| e.ts),
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
        let d = date_cn(ts);
        if cfg.segments.is_empty() {
            return d[..7].to_string();
        }
        cfg.segments.iter().find(|(_, a, b)| d >= *a && d <= *b).map(|s| s.0.clone()).unwrap_or("-".into())
    };
    #[derive(Default, Clone)]
    struct Agg {
        long: usize,
        verified: usize,
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
    for c in &cc.commits {
        let a = &attrs[&c.hash];
        let sg = seg_of(c.author_ts);
        let d = date_cn(c.author_ts);
        let ver = ver_of.get(&c.hash).cloned().unwrap_or("unreleased".into());
        conflicts += a.conflicts;
        tab_total += a.tab_lines;
        let t = c.author_ts + 8 * 3600;
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
                *by_area.entry(at.who).or_default().entry(ar).or_default() += 1;
                *by_ver.entry(at.who).or_default().entry(ver.clone()).or_default() += 1;
                *by_week.entry(iso_week(c.author_ts)).or_default().entry(reg.list[at.who].id.clone()).or_default() += 1;
                *by_day.entry(d.clone()).or_default().entry(reg.group(at.who)).or_default() += 1;
                let e = seg.entry(sg.clone()).or_default();
                if k != 0 {
                    e.long += 1;
                    if at.level == Level::Verified {
                        e.verified += 1;
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
            for _ in lines {
                *g.entry(reg.group(surv[i].who)).or_default() += 1;
                i += 1;
            }
            if !lines.is_empty() {
                let mut v = json!({ "lines": lines.len(), "by": g });
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
        let title = if hide_t { if u.id.starts_with("pr:") { format!("PR #{}", &u.id[3..]) } else { "提交".into() } } else { u.title.clone() };
        unit_json.push(json!({ "id": u.id, "title": title, "date": date_cn(u.ts), "version": ver, "commits": u.commits, "areas": areas, "shares": shares, "lines": total }));
    }

    // 贡献方
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
        if let Some(n) = &w.name {
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
    let days: HashSet<String> = cc.commits.iter().map(|c| date_cn(c.author_ts)).collect();
    let first = cc.commits.iter().map(|c| c.author_ts).min().unwrap_or(0);
    let last = cc.commits.iter().map(|c| c.author_ts).max().unwrap_or(0);
    let prompts: i64 = sessions.iter().filter(|s| !s.reviewer).map(|s| s.prompts).sum();
    // GitHub PR review：主导者本人的 review 计入人的参与，bot（如 chatgpt-codex-connector）计入 agent 审阅
    let director_logins: HashSet<String> = cfg
        .directors
        .iter()
        .flat_map(|d| d.emails.iter())
        .filter_map(|e| {
            let local = e.split('@').next()?;
            Some(local.split_once('+').map(|(_, l)| l).unwrap_or(local).to_lowercase())
        })
        .chain(cfg.directors.iter().map(|d| d.name.to_lowercase()))
        .collect();
    let (human_reviews, bot_reviews) = prs.iter().flat_map(|p| p.reviews.iter()).fold((0usize, 0usize), |(h, b), r| {
        let r = r.to_lowercase();
        if director_logins.contains(&r) {
            (h + 1, b)
        } else if r.contains("codex") || r.contains("bot") || r.contains("copilot") {
            (h, b + 1)
        } else {
            (h, b)
        }
    });
    let lines_written: usize = store.groups.values().flat_map(|g| g.evs.iter()).filter(|e| !e.tab).map(|e| (e.rel.as_str(), e.key, e.session.as_str())).collect::<HashSet<_>>().len();
    let coverage: Vec<Value> = seg
        .iter()
        .map(|(n, s)| {
            let best = if s.long > 0 && s.verified * 10 >= s.long { "verified" } else { "declared" };
            json!({ "name": n, "from": s.from, "to": s.to, "best": best, "verifiedShare": if s.long == 0 { 0.0 } else { (s.verified as f64 / s.long as f64 * 1000.0).round() / 1000.0 } })
        })
        .collect();
    let generated_at = now_iso();
    let credits = json!({
        "$schema": "https://creditia.dev/schema/v0.json",
        "schemaVersion": 0,
        "project": { "name": cfg.name, "repos": cfg.remotes, "range": { "from": date_cn(first), "to": date_cn(last) } },
        "generatedAt": generated_at,
        "level": cfg.level,
        "head": head,
        "versions": tags.iter().map(|(t, ts)| json!({ "tag": t, "date": date_cn(*ts) })).collect::<Vec<_>>(),
        "coverage": coverage,
        "contributors": contributors,
        "totals": { "commits": cc.commits.len(), "units": unit_json.len(), "linesAdded": total_lines, "linesSurviving": surv.len(), "linesWritten": lines_written, "tokens": tokens, "sessions": sessions.iter().filter(|s| !s.reviewer).count(), "days": days.len(), "binaryFiles": binary_files.values().sum::<usize>() },
        "breakdown": breakdown,
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
        std::fs::write(&out, serde_json::to_string(&credits).unwrap()).map_err(|e| format!("写入 {} 失败：{e}", out.display()))?;
        lib.save_snapshot(&proj.id, &head, &generated_at, &credits.to_string())?;
    }
    save_json(&cache.join("commits.json"), &cc)?;
    save_json(&cache.join("blame.json"), &bc)?;

    // 终端里的一行 tier
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for (w, lv) in &by_who {
        *groups.entry(reg.group(*w)).or_default() += lv.values().sum::<usize>();
    }
    let mut gl: Vec<(String, usize)> = groups.into_iter().collect();
    gl.sort_by(|a, b| b.1.cmp(&a.1));
    let tier = gl
        .iter()
        .map(|(k, n)| format!("{} {:.1}%", if k == UNATTR { "未确认".to_string() } else if k.starts_with("h:") { reg.list.iter().find(|w| &w.id == k).and_then(|w| w.name.clone()).unwrap_or(k.clone()) } else { k.clone() }, *n as f64 / total_lines.max(1) as f64 * 100.0))
        .collect::<Vec<_>>()
        .join(" · ");
    Ok(Report { name: cfg.name.clone(), head, commits: cc.commits.len(), new_commits: n_new, units: unit_json.len(), lines: total_lines, secs: t0.elapsed().as_secs_f64(), out, tier, notes: st.notes, warnings, blame: blame_out })
}
