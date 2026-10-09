//! creditia 阶段 0 验证工程：分析一个仓库，产出 credits.json 草稿和验证报告数据

mod adapters;
mod engine;
mod ev;
mod git;
mod hash;
mod paths;
mod util;

use engine::{area, Attr, CommitAttr, Config, Engine, Level, UNATTR};
use ev::{Index, Session, Store, STORE_VERSION};
use git::{BlameLine, Commit};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use util::{date_cn, iso_week};

fn load<T: serde::de::DeserializeOwned + Default>(p: &Path) -> T {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save<T: serde::Serialize>(p: &Path, v: &T) {
    std::fs::write(p, serde_json::to_vec(v).unwrap()).unwrap();
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CommitCache {
    head: Option<String>,
    commits: Vec<Commit>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg_path = args.get(1).expect("用法：creditia-spike <project.json> [--rev REV] [--fresh] [--out DIR]");
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let fresh = args.iter().any(|a| a == "--fresh");
    let cfg: Config = serde_json::from_str(&std::fs::read_to_string(cfg_path).unwrap()).unwrap();
    let repo = PathBuf::from(&cfg.repo);
    let rev = flag("--rev").unwrap_or(cfg.branch.clone());
    let out = PathBuf::from(flag("--out").unwrap_or("out".into()));
    let cache = out.join("cache");
    if fresh {
        let _ = std::fs::remove_dir_all(&cache);
    }
    std::fs::create_dir_all(&cache).unwrap();
    let mut timing: Vec<(String, f64)> = vec![];
    let t0 = Instant::now();
    let mut lap = |name: &str, t: &mut Instant| {
        timing.push((name.to_string(), t.elapsed().as_secs_f64()));
        *t = Instant::now();
    };
    let mut t = Instant::now();

    // ---- git：提交（增量）
    let head = git::git(&repo, &["rev-parse", &rev]).trim().to_string();
    let mut cc: CommitCache = load(&cache.join("commits.json"));
    // 上次处理到的提交仍是祖先就只读新增部分；历史被改写则全量重读
    let base = cc.head.clone().filter(|h| {
        std::process::Command::new("git").arg("-C").arg(&repo).args(["merge-base", "--is-ancestor", h, &head]).status().map(|s| s.success()).unwrap_or(false)
    });
    let new_commits = if let Some(b) = base {
        let range = format!("{b}..{head}");
        git::read_commits(&repo, &cfg.branch, Some(&range))
    } else {
        cc.commits.clear();
        git::read_commits(&repo, &cfg.branch, Some(&head))
    };
    let n_new = new_commits.len();
    cc.commits.extend(new_commits);
    cc.head = Some(head.clone());
    lap("git log -p", &mut t);
    // PR 元数据（gh pr list + Codeberg 存档导出的 JSON，可选）
    let prs: Vec<git::PrMeta> = flag("--prs").map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).unwrap_or_default();
    let (units, parents) = git::units(&repo, &head, &prs);
    let (tags, ver_of) = git::versions(&repo, &parents);
    lap("PR 划分 / 版本", &mut t);

    // ---- 路径集合：仓库历史里出现过的所有路径
    let mut known: HashSet<String> = git::git(&repo, &["log", "--format=", "--name-only", &head]).lines().filter(|l| !l.is_empty()).map(String::from).collect();
    known.extend(git::git(&repo, &["ls-tree", "-r", "--name-only", &head]).lines().map(String::from));
    let pm = paths::PathMap { known, markers: cfg.markers.iter().map(|m| m.to_lowercase()).collect(), excludes: cfg.exclude_markers.iter().map(|m| m.to_lowercase()).collect() };

    // ---- 证据库（按来源文件增量）
    let mut store: Store = load(&cache.join("store.json"));
    if store.version != STORE_VERSION {
        store = Store { version: STORE_VERSION, ..Default::default() };
    }
    let mut st = adapters::Stats { warnings: vec![], notes: vec![] };
    adapters::codex(&mut store, &pm, &pm.markers, &mut st);
    lap("adapter: codex", &mut t);
    adapters::claude(&mut store, &pm, &pm.markers, &mut st);
    lap("adapter: claude-code", &mut t);
    adapters::cursor(&mut store, &pm, &pm.markers, &mut st);
    lap("adapter: cursor", &mut t);
    let mut idx = Index::build(&store);
    let sessions: Vec<Session> = store.groups.values().flat_map(|g| g.sessions.clone()).collect();
    lap("建索引", &mut t);

    // ---- 归因
    let mut eng = Engine::new(&cfg, &idx, &sessions, &units);
    let mut attrs: HashMap<String, CommitAttr> = HashMap::new();
    for c in &cc.commits {
        attrs.insert(c.hash.clone(), eng.attribute(c));
    }
    lap("逐行归因", &mut t);

    // ---- 存活代码
    let mut bc: HashMap<String, Vec<BlameLine>> = load(&cache.join("blame.json"));
    let blame = git::blame_head(&repo, &mut bc);
    lap("git blame（存活代码）", &mut t);
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
            let main = a.main.map(|w| Attr { who: w, level: Level::Declared });
            (h.clone(), (m, main))
        })
        .collect();
    let unattr = eng.reg.unattributed();
    let surv = engine::survival(&blame, &per_commit, unattr);
    // 逐行溯源：--blame 路径:行号，输出这一行的作者、模型、会话、提交，不输出代码内容
    if let Some(q) = flag("--blame") {
        let (fpath, ln) = q.rsplit_once(':').map(|(a, b)| (a.to_string(), b.parse::<usize>().unwrap_or(1))).unwrap_or((q.clone(), 1));
        let mut i = 0;
        for (path, lines) in &blame {
            if *path == fpath {
                let Some(b) = lines.get(ln.saturating_sub(1)) else { break };
                let a = surv[i + ln - 1];
                let w = &eng.reg.list[a.who];
                let c = commit_by_hash.get(b.commit.as_str());
                let ev = idx.map.get(&(b.rel.clone(), b.key)).and_then(|v| {
                    let ts = c.map(|c| c.author_ts).unwrap_or(0);
                    v.iter().filter(|e| e.ts <= ts + 3600).max_by_key(|e| e.ts)
                });
                let unit = units.iter().find(|u| u.commits.contains(&b.commit));
                let out = json!({
                    "file": fpath, "line": ln, "contributor": w.id, "harness": w.harness, "model": w.model, "name": w.name,
                    "evidence": a.level, "commit": &b.commit[..10.min(b.commit.len())], "date": c.map(|c| date_cn(c.author_ts)),
                    "subject": c.map(|c| c.subject.clone()), "unit": unit.map(|u| u.id.clone()), "unitTitle": unit.map(|u| u.title.clone()),
                    "session": ev.map(|e| e.session.clone()), "sessionTime": ev.map(|e| e.ts),
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                return;
            }
            i += lines.len();
        }
        eprintln!("没找到 {q}");
        std::process::exit(1);
    }
    lap("存活归因", &mut t);
    drop(per_commit);
    let mut files_surv: Vec<Value> = vec![];
    {
        let mut i = 0;
        for (path, lines) in &blame {
            let mut g: BTreeMap<String, usize> = BTreeMap::new();
            for _ in lines {
                *g.entry(eng.reg.group(surv[i].who)).or_default() += 1;
                i += 1;
            }
            if !lines.is_empty() {
                files_surv.push(json!({"path": path, "lines": lines.len(), "by": g}));
            }
        }
    }

    // ---- 汇总
    let reg = &eng.reg;
    let seg_of = |ts: i64| -> String {
        let d = date_cn(ts);
        cfg.segments.iter().find(|(_, a, b)| d >= *a && d <= *b).map(|s| s.0.clone()).unwrap_or("-".into())
    };
    #[derive(Default, Clone, serde::Serialize)]
    struct Agg {
        lines: usize,
        long: usize,
        verified: usize,
        declared: usize,
        inferred: usize,
        unattributed: usize,
    }
    let mut by_who: BTreeMap<usize, BTreeMap<Level, usize>> = BTreeMap::new();
    let mut by_area: BTreeMap<usize, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut by_ver: BTreeMap<usize, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_week: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_day: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut seg: BTreeMap<String, Agg> = BTreeMap::new();
    let mut seg_src: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut seg_group: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut binary_files: BTreeMap<String, usize> = BTreeMap::new();
    let (mut conflicts, mut tab_total, mut verified_lines) = (0usize, 0usize, 0usize);
    let mut human_lines_by_director = 0usize;
    let mut doc_lines_director = 0usize;
    for c in &cc.commits {
        let a = &attrs[&c.hash];
        let sg = seg_of(c.author_ts);
        let ver = ver_of.get(&c.hash).cloned().unwrap_or("unreleased".into());
        conflicts += a.conflicts;
        tab_total += a.tab_lines;
        for (s, n) in &a.hits {
            *seg_src.entry(sg.clone()).or_default().entry(s).or_default() += n;
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
                *by_day.entry(date_cn(c.author_ts)).or_default().entry(reg.group(at.who)).or_default() += 1;
                *seg_group.entry(sg.clone()).or_default().entry(reg.group(at.who)).or_default() += 1;
                let e = seg.entry(sg.clone()).or_default();
                e.lines += 1;
                if k != 0 {
                    e.long += 1;
                    if at.level == Level::Verified {
                        e.verified += 1;
                        verified_lines += 1;
                    }
                }
                match at.level {
                    Level::Verified => {}
                    Level::Declared => e.declared += 1,
                    Level::Inferred => e.inferred += 1,
                    Level::Unattributed => e.unattributed += 1,
                }
                if reg.list[at.who].kind == "human" && at.who == 0 {
                    human_lines_by_director += 1;
                    if ar == "docs" {
                        doc_lines_director += 1;
                    }
                }
            }
        }
    }
    let mut surv_by: BTreeMap<usize, usize> = BTreeMap::new();
    for a in &surv {
        *surv_by.entry(a.who).or_default() += 1;
    }

    // 项：每个项的各方占比，主要贡献者
    let mut unit_json = vec![];
    let mut unit_main_group: BTreeMap<String, usize> = BTreeMap::new();
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
        let mut g: BTreeMap<String, usize> = BTreeMap::new();
        for (w, (n, _)) in &cnt {
            *g.entry(reg.group(*w)).or_default() += n;
        }
        let top = g.iter().max_by_key(|(_, n)| **n).map(|(k, _)| k.clone()).unwrap();
        *unit_main_group.entry(top).or_default() += 1;
        let mut shares: Vec<Value> = cnt
            .iter()
            .map(|(w, (n, lv))| json!({"contributor": reg.list[*w].id, "share": (*n as f64 / total as f64 * 1000.0).round() / 1000.0, "evidence": lv}))
            .collect();
        shares.sort_by(|a, b| b["share"].as_f64().partial_cmp(&a["share"].as_f64()).unwrap());
        let mut areas: Vec<&str> = areas.into_iter().collect();
        areas.sort();
        let ver = u.commits.iter().find_map(|h| ver_of.get(h)).cloned();
        unit_json.push(json!({
            "id": u.id, "title": u.title, "date": date_cn(u.ts), "version": ver,
            "commits": u.commits, "areas": areas, "shares": shares, "lines": total,
        }));
    }

    // 贡献方和人的信号
    let mut contributors = vec![];
    for (i, w) in reg.list.iter().enumerate() {
        if !by_who.contains_key(&i) && i != 0 && !sessions.iter().any(|s| s.reviewer && w.harness.as_deref() == Some(s.src.harness()) && w.model.as_deref() == Some(&s.model)) {
            continue;
        }
        let mut roles = vec![];
        if i == 0 {
            roles.push("director");
        }
        if by_who.contains_key(&i) && w.kind != "unattributed" {
            roles.push("author");
        }
        let mut c = json!({"id": w.id, "kind": w.kind});
        if let Some(h) = &w.harness {
            c["harness"] = json!(h);
            c["model"] = json!(w.model);
            c["family"] = json!(w.model.as_deref().map(family));
            c["where"] = json!("local");
        }
        if let Some(n) = &w.name {
            c["name"] = json!(n);
        }
        c["roles"] = json!(roles);
        contributors.push(c);
    }
    // 审阅者（codex-auto-review）单独列出，不计代码行
    let reviews = sessions.iter().filter(|s| s.reviewer).count();
    if reviews > 0 {
        contributors.push(json!({"id": "a:codex/codex-auto-review", "kind": "agent", "harness": "codex", "model": "codex-auto-review", "roles": ["reviewer"], "reviews": reviews}));
    }
    let breakdown: Vec<Value> = by_who
        .iter()
        .map(|(w, lv)| {
            let ev = |l: Level| lv.get(&l).copied().unwrap_or(0);
            json!({
                "contributor": reg.list[*w].id,
                "linesAdded": lv.values().sum::<usize>(),
                "linesSurviving": surv_by.get(w).copied().unwrap_or(0),
                "byEvidence": {
                    "verified": {"linesAdded": ev(Level::Verified)},
                    "declared": {"linesAdded": ev(Level::Declared)},
                    "inferred": {"linesAdded": ev(Level::Inferred)},
                    "unattributed": {"linesAdded": ev(Level::Unattributed)},
                },
                "byArea": by_area.get(w),
                "byVersion": by_ver.get(w),
            })
        })
        .collect();
    let timeline: Vec<Value> = by_week.iter().map(|(w, m)| json!({"week": w, "byContributor": m})).collect();
    let sess_json: Vec<Value> = {
        let mut v: Vec<&Session> = sessions.iter().collect();
        v.sort_by_key(|s| s.start);
        v.iter()
            .map(|s| {
                let id = if s.model.is_empty() { format!("a:{}", s.src.harness()) } else { format!("a:{}/{}", s.src.harness(), s.model) };
                json!({"contributor": id, "start": s.start, "end": s.end, "tokens": s.tokens, "review": s.reviewer})
            })
            .collect()
    };
    // agent 在会话里写出的行（去重后的证据条数，不含 Tab 补全）
    let lines_written: usize = store.groups.values().flat_map(|g| g.evs.iter()).filter(|e| !e.tab).map(|e| (e.rel.as_str(), e.key, e.session.as_str())).collect::<HashSet<_>>().len();
    // 作息：星期（周一=0）× 小时（北京时间）的提交数
    let mut clock = vec![vec![0usize; 24]; 7];
    for c in &cc.commits {
        let t = c.author_ts + 8 * 3600;
        let wd = ((t.div_euclid(86400) + 3).rem_euclid(7)) as usize;
        clock[wd][(t.rem_euclid(86400) / 3600) as usize] += 1;
    }
    let total_lines: usize = by_who.values().flat_map(|m| m.values()).sum();
    let tokens: i64 = sessions.iter().map(|s| s.tokens).sum();
    let days: HashSet<String> = cc.commits.iter().map(|c| date_cn(c.author_ts)).collect();
    let first = cc.commits.iter().map(|c| c.author_ts).min().unwrap_or(0);
    let last = cc.commits.iter().map(|c| c.author_ts).max().unwrap_or(0);
    let prompts: i64 = sessions.iter().filter(|s| !s.reviewer).map(|s| s.prompts).sum();
    let coverage: Vec<Value> = cfg
        .segments
        .iter()
        .map(|(n, a, b)| {
            let s = seg.get(n).cloned().unwrap_or_default();
            let best = if s.long > 0 && s.verified * 10 >= s.long { "verified" } else { "declared" };
            json!({"from": a, "to": b, "best": best, "verifiedShare": ratio(s.verified, s.long)})
        })
        .collect();
    let credits = json!({
        "$schema": "https://creditia.dev/schema/v0.json",
        "project": {"name": cfg.name, "repos": cfg.remotes, "range": {"from": date_cn(first), "to": date_cn(last)}},
        "generatedAt": now_iso(),
        "level": "standard",
        "head": head,
        "versions": tags.iter().map(|(t, ts)| json!({"tag": t, "date": date_cn(*ts)})).collect::<Vec<_>>(),
        "coverage": coverage,
        "directors": cfg.directors.iter().map(|d| json!({"id": d.id, "name": d.name})).collect::<Vec<_>>(),
        "contributors": contributors,
        "totals": {
            "commits": cc.commits.len(), "units": unit_json.len(), "linesAdded": total_lines,
            "linesSurviving": surv.len(), "tokens": tokens, "sessions": sessions.iter().filter(|s| !s.reviewer).count(), "days": days.len(),
            "binaryFiles": binary_files.values().sum::<usize>(),
            "linesWritten": lines_written,
        },
        "breakdown": breakdown,
        "human": {"prompts": prompts, "plans": doc_lines_director, "reviews": 0, "linesRewritten": null, "tabAssistedLines": tab_total, "agentReviews": reviews},
        "timeline": timeline,
        "clock": clock,
        "days": by_day.iter().map(|(d, m)| json!({"date": d, "by": m})).collect::<Vec<_>>(),
        "files": files_surv,
        "sessions": sess_json,
        "units": unit_json,
    });
    let total_time = t0.elapsed().as_secs_f64();
    save(&out.join("credits.json"), &credits);
    std::fs::write(out.join("credits.pretty.json"), serde_json::to_string_pretty(&credits).unwrap()).unwrap();

    // ---- 验证报告数据
    let by_group_lines = group_lines(&by_who, reg);
    let by_group_surv: BTreeMap<String, usize> = surv_by.iter().fold(BTreeMap::new(), |mut m, (w, n)| {
        *m.entry(reg.group(*w)).or_default() += n;
        m
    });
    let models: BTreeMap<String, usize> = by_who.iter().map(|(w, m)| (reg.list[*w].id.clone(), m.values().sum())).collect();
    let report = json!({
        "head": head, "rev": rev, "newCommits": n_new, "commits": cc.commits.len(), "units": unit_json.len(),
        "timing": timing.iter().map(|(k, v)| json!({"step": k, "secs": (v * 100.0).round() / 100.0})).collect::<Vec<_>>(),
        "totalSecs": total_time,
        "segments": seg, "segmentSourceHits": seg_src, "segmentGroups": seg_group,
        "groupsByLines": by_group_lines, "groupsBySurviving": by_group_surv, "groupsByUnits": unit_main_group,
        "byContributor": models, "conflictLines": conflicts, "verifiedLines": verified_lines,
        "tabLines": tab_total, "directorLines": human_lines_by_director, "binaryFiles": binary_files,
        "notes": st.notes, "warnings": st.warnings, "panel": cfg.panel,
        "evidence": store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
        "evidenceBySource": count_by_src(&store),
        "sessions": sessions.len(), "reviewSessions": reviews,
    });
    std::fs::write(out.join("report.json"), serde_json::to_string_pretty(&report).unwrap()).unwrap();

    // ---- 抽样 40 个提交
    let sample = sample(&cc.commits, &attrs, &eng, &seg_of, &units);
    std::fs::write(out.join("sample.json"), serde_json::to_string_pretty(&sample).unwrap()).unwrap();

    // ---- 缓存
    let tc = Instant::now();
    save(&cache.join("commits.json"), &cc);
    save(&cache.join("store.json"), &store);
    save(&cache.join("blame.json"), &bc);
    idx.map.clear();
    eprintln!(
        "完成：{} 个提交（新增 {}），{} 项，{} 行，分析 {:.1}s，写缓存 {:.1}s",
        cc.commits.len(),
        n_new,
        unit_json.len(),
        total_lines,
        total_time,
        tc.elapsed().as_secs_f64()
    );
    for w in &st.warnings {
        eprintln!("警告：{w}");
    }
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { (a as f64 / b as f64 * 1000.0).round() / 1000.0 }
}

fn family(m: &str) -> &'static str {
    let m = m.to_lowercase();
    if m.starts_with("gpt") || m.contains("codex") { "gpt" }
    else if m.contains("claude") || m.contains("opus") || m.contains("sonnet") || m.contains("fable") || m.contains("haiku") { "claude" }
    else if m.contains("grok") { "grok" }
    else if m.contains("kimi") { "kimi" }
    else if m.contains("gemini") { "gemini" }
    else if m.contains("deepseek") { "deepseek" }
    else if m.contains("composer") { "cursor" }
    else { "unknown" }
}

fn now_iso() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (y, m, d) = util::civil_from_days((t + 8 * 3600).div_euclid(86400));
    let s = (t + 8 * 3600).rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+08:00", s / 3600, s / 60 % 60, s % 60)
}

fn group_lines(by_who: &BTreeMap<usize, BTreeMap<Level, usize>>, reg: &engine::Registry) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for (w, lv) in by_who {
        *m.entry(reg.group(*w)).or_default() += lv.values().sum::<usize>();
    }
    m
}

fn count_by_src(store: &Store) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for g in store.groups.values() {
        for e in &g.evs {
            *m.entry(if e.tab { "cursor-tab" } else { e.src.harness() }).or_default() += 1;
        }
    }
    m
}

/// 抽样：A、B、C 各 10 个，另 10 个取改动最大的前 100 个里的随机样本（固定种子，可复现）
fn sample(commits: &[Commit], attrs: &HashMap<String, CommitAttr>, eng: &Engine, seg_of: &dyn Fn(i64) -> String, units: &[git::Unit]) -> Vec<Value> {
    let mut rng = 0x2026_1008u64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let lines = |c: &Commit| c.files.iter().map(|f| f.keys.len()).sum::<usize>();
    let eligible: Vec<&Commit> = commits.iter().filter(|c| lines(c) >= 5).collect();
    let mut picked: Vec<(&str, &Commit)> = vec![];
    let mut used: HashSet<&str> = HashSet::new();
    for s in ["A", "B", "C"] {
        let mut pool: Vec<&Commit> = eligible.iter().copied().filter(|c| seg_of(c.author_ts) == s).collect();
        for _ in 0..10.min(pool.len()) {
            let i = (next() as usize) % pool.len();
            let c = pool.swap_remove(i);
            used.insert(&c.hash);
            picked.push((s, c));
        }
    }
    let mut big: Vec<&Commit> = eligible.iter().copied().filter(|c| !used.contains(c.hash.as_str())).collect();
    big.sort_by_key(|c| std::cmp::Reverse(lines(c)));
    big.truncate(100);
    for _ in 0..10.min(big.len()) {
        let i = (next() as usize) % big.len();
        picked.push(("大改动", big.swap_remove(i)));
    }
    let unit_of: HashMap<&str, &git::Unit> = units.iter().flat_map(|u| u.commits.iter().map(move |c| (c.as_str(), u))).collect();
    let reg = &eng.reg;
    picked
        .into_iter()
        .map(|(s, c)| {
            let a = &attrs[&c.hash];
            let mut g: BTreeMap<String, (usize, Level)> = BTreeMap::new();
            for at in a.lines.iter().flatten() {
                let w = &reg.list[at.who];
                let name = match (&w.harness, &w.model) {
                    (Some(h), Some(m)) => format!("{h}（{m}）"),
                    (Some(h), None) => h.clone(),
                    _ => w.name.clone().unwrap_or(UNATTR.into()),
                };
                let e = g.entry(name).or_insert((0, at.level));
                e.0 += 1;
                e.1 = e.1.min(at.level);
            }
            let total: usize = g.values().map(|x| x.0).sum();
            let mut shares: Vec<(String, usize, Level)> = g.into_iter().map(|(k, (n, l))| (k, n, l)).collect();
            shares.sort_by_key(|x| std::cmp::Reverse(x.1));
            let u = unit_of.get(c.hash.as_str());
            json!({
                "segment": s, "hash": &c.hash[..10], "date": date_cn(c.author_ts), "subject": c.subject,
                "unit": u.map(|u| &u.id), "branch": u.and_then(|u| u.branch.clone()).or_else(|| eng.unit_branch.get(&c.hash).cloned()),
                "email": c.email, "coauthors": c.coauthors,
                "files": c.files.iter().map(|f| json!({"path": f.rel, "added": f.keys.len()})).collect::<Vec<_>>(),
                "linesAdded": total, "hits": a.hits, "reason": a.reason,
                "shares": shares.iter().map(|(k, n, l)| json!({"who": k, "share": ratio(*n, total), "evidence": l})).collect::<Vec<_>>(),
            })
        })
        .collect()
}
