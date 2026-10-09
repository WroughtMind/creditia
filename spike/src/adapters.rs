//! 各 agent 日志 adapter：只读打开，输出统一的行级证据和会话元数据

use crate::ev::{fingerprint, Ev, FileGroup, Session, Src, Store};
use crate::hash::{ext_of, line_key, normalize};
use crate::paths::PathMap;
use crate::util::parse_iso;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn open_ro(p: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    Ok(c)
}

/// 把一段新文本拆成行，生成证据
fn push_text(out: &mut Vec<Ev>, pm: &PathMap, abs: &str, text: &str, src: Src, model: &str, session: &str, ts: i64) -> bool {
    let Some(rel) = pm.to_rel(abs) else { return false };
    let ext = ext_of(&rel);
    for line in text.lines() {
        let n = normalize(line, &ext);
        if n.chars().count() < 4 {
            continue;
        }
        out.push(Ev { rel: rel.clone(), key: line_key(&rel, &n), src, model: model.to_string(), session: session.to_string(), ts, tab: false });
    }
    true
}

pub struct Stats {
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------- Codex

pub fn codex(store: &mut Store, pm: &PathMap, markers: &[String], st: &mut Stats) {
    let base = std::env::var("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|_| home().join(".codex"));
    let db = base.join("state_5.sqlite");
    let conn = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => return st.warnings.push(format!("codex: 线程索引不可读 {e}")),
    };
    // 会话文件索引：文件名 → 实际路径（可能已压缩、已归档）
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    for dir in ["sessions", "archived_sessions"] {
        for e in walkdir::WalkDir::new(base.join(dir)).into_iter().flatten() {
            let name = e.file_name().to_string_lossy().replace(".zst", "");
            if name.ends_with(".jsonl") {
                files.insert(name, e.path().to_path_buf());
            }
        }
    }
    let cond: Vec<String> = markers
        .iter()
        .map(|m| format!("(lower(cwd) like '%{m}%' or lower(coalesce(git_origin_url,'')) like '%{m}%')"))
        .collect();
    let sql = format!(
        "select id, rollout_path, created_at, updated_at, tokens_used, coalesce(model,''), git_branch, cwd, coalesce(git_origin_url,'') from threads where {}",
        cond.join(" or ")
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows: Vec<(String, String, i64, i64, i64, String, Option<String>)> = stmt
        .query_map([], |r| {
            let cwd: String = r.get(7)?;
            let origin: String = r.get(8)?;
            Ok((pm.in_project(&cwd) || pm.in_project(&origin), (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
        })
        .unwrap()
        .flatten()
        .filter(|(ok, _)| *ok)
        .map(|(_, r)| r)
        .collect();
    let (mut missing, mut found) = (0, 0);
    for (id, rollout, start, end, tokens, model, branch) in rows {
        let name = Path::new(&rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default();
        let file = files.get(&name);
        let reviewer = model == "codex-auto-review";
        let gk = format!("codex:{id}");
        let fp = file.map(|f| fingerprint(f)).unwrap_or((0, 0));
        if file.is_some() { found += 1 } else { missing += 1 }
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let mut prompts = 0;
        if let Some(f) = file {
            let data = read_maybe_zst(f);
            let mut cur_model = model.clone();
            for line in data.lines() {
                if line.contains("\"task_started\"") {
                    prompts += 1;
                }
                let is_ctx = line.starts_with("{\"timestamp\"") && line.contains("\"turn_context\"");
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
                let ts = o["timestamp"].as_str().and_then(parse_iso).unwrap_or(start);
                if let Some(ch) = it["changes"].as_object() {
                    for (path, v) in ch {
                        let target = v["move_path"].as_str().unwrap_or(path);
                        match v["type"].as_str() {
                            Some("add") => {
                                push_text(&mut g.evs, pm, target, v["content"].as_str().unwrap_or(""), Src::Codex, &cur_model, &id, ts);
                            }
                            Some("update") => {
                                let added: String = v["unified_diff"]
                                    .as_str()
                                    .unwrap_or("")
                                    .lines()
                                    .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                                    .map(|l| format!("{}\n", &l[1..]))
                                    .collect();
                                push_text(&mut g.evs, pm, target, &added, Src::Codex, &cur_model, &id, ts);
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
        g.sessions.push(Session { src: Src::Codex, id: id.clone(), model, start, end, tokens, branch, prompts, reviewer, has_content: file.is_some() });
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("codex: 线程 {} 个，内容文件存在 {found}，缺失 {missing}", found + missing));
}

fn read_maybe_zst(p: &Path) -> String {
    let raw = std::fs::read(p).unwrap_or_default();
    let bytes = if p.extension().map(|e| e == "zst").unwrap_or(false) {
        zstd::stream::decode_all(&raw[..]).unwrap_or_default()
    } else {
        raw
    };
    String::from_utf8_lossy(&bytes).into_owned()
}

// ---------------------------------------------------------------- Claude Code

pub fn claude(store: &mut Store, pm: &PathMap, markers: &[String], st: &mut Stats) {
    let base = std::env::var("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|_| home().join(".claude"));
    let mut sessions = 0;
    for e in walkdir::WalkDir::new(base.join("projects")).into_iter().flatten() {
        let p = e.path();
        if !e.file_type().is_file() || p.extension().map(|x| x != "jsonl").unwrap_or(true) {
            continue;
        }
        let gk = format!("claude:{}", p.display());
        let fp = fingerprint(p);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            sessions += store.groups[&gk].sessions.len();
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let text = std::fs::read_to_string(p).unwrap_or_default();
        // 不靠目录名识别项目（中文路径会被编码成 ---），读每条记录的 cwd
        let lower = text.to_lowercase();
        if markers.iter().any(|m| lower.contains(m.as_str())) {
            parse_claude(&text, pm, &mut g);
        }
        sessions += g.sessions.len();
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("claude-code: 项目相关会话 {sessions} 个（按 cwd 识别）"));
}

fn parse_claude(text: &str, pm: &PathMap, g: &mut FileGroup) {
    let mut in_project = false;
    let (mut sid, mut start, mut end, mut prompts, mut tokens) = (String::new(), i64::MAX, 0i64, 0i64, 0i64);
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut seen_msg: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(cwd) = o["cwd"].as_str() {
            if pm.in_project(cwd) {
                in_project = true;
            }
        }
        let Some(ts) = o["timestamp"].as_str().and_then(parse_iso) else { continue };
        if sid.is_empty() {
            sid = o["sessionId"].as_str().unwrap_or("").to_string();
        }
        start = start.min(ts);
        end = end.max(ts);
        let m = &o["message"];
        if o["type"] == "user" && o["isMeta"] != true {
            let human = match &m["content"] {
                Value::String(_) => true,
                Value::Array(a) => a.iter().any(|b| b["type"] == "text") && !a.iter().any(|b| b["type"] == "tool_result"),
                _ => false,
            };
            if human {
                prompts += 1;
            }
        }
        if o["type"] != "assistant" {
            continue;
        }
        let model = m["model"].as_str().unwrap_or("");
        if model.is_empty() || model == "<synthetic>" {
            continue;
        }
        if let Some(id) = m["id"].as_str() {
            if seen_msg.insert(id.to_string()) {
                let u = &m["usage"];
                tokens += ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                    .iter()
                    .map(|k| u[*k].as_i64().unwrap_or(0))
                    .sum::<i64>();
            }
        }
        *models.entry(model.to_string()).or_default() += 1;
        for b in m["content"].as_array().into_iter().flatten() {
            if b["type"] != "tool_use" {
                continue;
            }
            let inp = &b["input"];
            let path = inp["file_path"].as_str().unwrap_or("");
            match b["name"].as_str() {
                Some("Write") => {
                    push_text(&mut g.evs, pm, path, inp["content"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("Edit") => {
                    push_text(&mut g.evs, pm, path, inp["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("MultiEdit") => {
                    for ed in inp["edits"].as_array().into_iter().flatten() {
                        push_text(&mut g.evs, pm, path, ed["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                    }
                }
                _ => {}
            }
        }
    }
    if !in_project {
        g.evs.clear();
        return;
    }
    let model = models.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or_default();
    if start <= end {
        g.sessions.push(Session { src: Src::Claude, id: sid, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true });
    }
}

// ---------------------------------------------------------------- Cursor

pub fn cursor(store: &mut Store, pm: &PathMap, markers: &[String], st: &mut Stats) {
    let db = home().join(".cursor/ai-tracking/ai-code-tracking.db");
    let fp = fingerprint(&db);
    let gk = "cursor:ai-code-tracking".to_string();
    if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
        return;
    }
    let conn = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => return st.warnings.push(format!("cursor: 行级记录不可读 {e}")),
    };
    let cond: Vec<String> = markers.iter().map(|m| format!("lower(fileName) like '%{m}%'")).collect();
    let sql = format!(
        "select hash, source, fileName, coalesce(conversationId,''), coalesce(timestamp,createdAt), coalesce(model,'') from ai_code_hashes where {}",
        cond.join(" or ")
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows: Vec<(String, String, String, String, i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .unwrap()
        .flatten()
        .collect();

    // Auto 模式记为 default：回退到对话库里的对话级模型
    let vsc = home().join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
    let vconn = open_ro(&vsc).ok();
    let mut conv_model: HashMap<String, String> = HashMap::new();
    let mut resolve = |conv: &str, m: &str| -> String {
        if m != "default" && !m.is_empty() {
            return m.to_string();
        }
        if let Some(x) = conv_model.get(conv) {
            return x.clone();
        }
        let got: Option<String> = vconn.as_ref().and_then(|c| {
            c.query_row(
                "select json_extract(value,'$.modelConfig.modelName') from cursorDiskKV where key = ?1",
                [format!("composerData:{conv}")],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
        });
        let x = match got.as_deref() {
            Some(v) if v != "default" && !v.is_empty() => v.to_string(),
            _ => "auto".to_string(),
        };
        conv_model.insert(conv.to_string(), x.clone());
        x
    };

    let mut g = FileGroup { fp, ..Default::default() };
    let mut conv: HashMap<String, (i64, i64, String)> = HashMap::new();
    let (mut unmapped, mut total) = (0, 0);
    for (hash, source, file, cid, ts_ms, model) in rows {
        total += 1;
        let Some(rel) = pm.to_rel(&file) else {
            unmapped += 1;
            continue;
        };
        let Ok(key) = u32::from_str_radix(&hash, 16) else { continue };
        let ts = ts_ms / 1000;
        let model = resolve(&cid, &model);
        let e = conv.entry(cid.clone()).or_insert((ts, ts, model.clone()));
        e.0 = e.0.min(ts);
        e.1 = e.1.max(ts);
        g.evs.push(Ev { rel, key, src: Src::Cursor, model, session: cid, ts, tab: source == "tab" });
    }
    for (id, (start, end, model)) in conv {
        g.sessions.push(Session { src: Src::Cursor, id, model, start, end, tokens: 0, branch: None, prompts: 0, reviewer: false, has_content: true });
    }
    st.notes.push(format!("cursor: 行级记录 {total} 条，无法映射到仓库路径 {unmapped} 条"));
    store.groups.insert(gk, g);
}
