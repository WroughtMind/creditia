//! Codex：`state_5.sqlite` 的 threads 表（模型、token、分支、cwd）+ rollout jsonl 里的 FileChange 事件

use crate::{open_ro, read_maybe_zst, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, parse_iso};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Thread {
    id: String,
    rollout: String,
    start: i64,
    end: i64,
    tokens: i64,
    model: String,
    branch: Option<String>,
    cwd: String,
    origin: String,
}

fn threads(base: &Path) -> Result<Vec<Thread>, String> {
    let db = base.join("state_5.sqlite");
    let conn = open_ro(&db).map_err(|e| format!("codex: 线程索引不可读 {e}"))?;
    let mut stmt = conn
        .prepare("select id, rollout_path, created_at, updated_at, tokens_used, coalesce(model,''), git_branch, cwd, coalesce(git_origin_url,'') from threads")
        .map_err(|e| format!("codex: threads 表结构不认识 {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Thread {
                id: r.get(0)?,
                rollout: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                tokens: r.get(4)?,
                model: r.get(5)?,
                branch: r.get(6)?,
                cwd: r.get(7)?,
                origin: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(rows)
}

/// 会话文件索引：文件名 → 实际路径（可能已压缩、已归档）
fn rollout_files(base: &Path) -> HashMap<String, PathBuf> {
    let mut files = HashMap::new();
    for dir in ["sessions", "archived_sessions"] {
        for e in walkdir::WalkDir::new(base.join(dir)).into_iter().flatten() {
            let name = e.file_name().to_string_lossy().replace(".zst", "");
            if name.ends_with(".jsonl") {
                files.insert(name, e.path().to_path_buf());
            }
        }
    }
    files
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let base = source_dir(Src::Codex);
    let rows = match threads(&base) {
        Ok(r) => r,
        Err(e) => return st.warnings.push(e),
    };
    let files = rollout_files(&base);
    let (mut missing, mut found) = (0, 0);
    for t in rows.into_iter().filter(|t| pm.in_project(&t.cwd) || pm.in_project(&t.origin)) {
        let name = Path::new(&t.rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default();
        let file = files.get(&name);
        let reviewer = t.model == "codex-auto-review";
        let gk = format!("codex:{}", t.id);
        let fp = file.map(|f| fingerprint(f)).unwrap_or((0, 0));
        if file.is_some() { found += 1 } else { missing += 1 }
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let mut prompts = 0;
        if let Some(f) = file {
            let data = read_maybe_zst(f);
            let mut cur_model = t.model.clone();
            for line in data.lines() {
                if line.contains("\"task_started\"") {
                    prompts += 1;
                }
                let is_ctx = line.contains("\"turn_context\"");
                if !line.contains("\"FileChange\"") && !is_ctx {
                    continue;
                }
                let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
                let p = &o["payload"];
                if o["type"] == "turn_context" {
                    if let Some(m) = p["model"].as_str() {
                        cur_model = m.to_string();
                    }
                    continue;
                }
                let it = &p["item"];
                if it["type"] != "FileChange" {
                    continue;
                }
                let ts = o["timestamp"].as_str().and_then(parse_iso).unwrap_or(t.start);
                if let Some(ch) = it["changes"].as_object() {
                    for (path, v) in ch {
                        let target = v["move_path"].as_str().unwrap_or(path);
                        match v["type"].as_str() {
                            Some("add") => {
                                crate::push_text(&mut g.evs, pm, target, v["content"].as_str().unwrap_or(""), Src::Codex, &cur_model, &t.id, ts);
                            }
                            Some("update") => {
                                let added: String = v["unified_diff"]
                                    .as_str()
                                    .unwrap_or("")
                                    .lines()
                                    .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                                    .map(|l| format!("{}\n", &l[1..]))
                                    .collect();
                                crate::push_text(&mut g.evs, pm, target, &added, Src::Codex, &cur_model, &t.id, ts);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if reviewer {
            g.evs.clear();
        }
        g.sessions.push(Session { src: Src::Codex, id: t.id.clone(), model: t.model, start: t.start, end: t.end, tokens: t.tokens, branch: t.branch, prompts, reviewer, has_content: file.is_some() });
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("codex: 线程 {} 个，内容文件存在 {found}，缺失 {missing}", found + missing));
}

pub fn discover() -> Vec<Activity> {
    let base = source_dir(Src::Codex);
    let Ok(rows) = threads(&base) else { return vec![] };
    let mut m: HashMap<String, Activity> = HashMap::new();
    for t in rows {
        let e = m.entry(t.cwd.clone()).or_insert(Activity { src: Src::Codex, cwd: t.cwd.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(t.end);
        if !t.origin.is_empty() {
            e.remote = Some(t.origin);
        }
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Codex);
    let mut h = Health { source: "codex".into(), path: base.display().to_string(), format: "state_5.sqlite + rollout jsonl".into(), ..Default::default() };
    match threads(&base) {
        Ok(rows) => {
            h.readable = true;
            h.records = rows.len() as u64;
            h.earliest = rows.iter().map(|t| t.start).min().map(date_cn);
            h.latest = rows.iter().map(|t| t.end).max().map(date_cn);
            let files = rollout_files(&base);
            let have = rows.iter().filter(|t| files.contains_key(&Path::new(&t.rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default())).count();
            h.note = format!("{} 个线程有内容文件", have);
        }
        Err(e) => h.note = e,
    }
    h
}
