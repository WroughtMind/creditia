//! Goose（block/goose）：`~/.local/share/goose/sessions/sessions.db`（Windows 是 `%APPDATA%\Block\goose\data`，`GOOSE_PATH_ROOT` 设了就是 `<它>/data`）。
//! sessions 表有 working_dir、provider_name、model_config_json（model_name）、累计 token；
//! messages 表的 content_json 是内容块数组：assistant 的 `toolRequest`（toolCall.value 是 name + arguments）配上之后 user 消息里同 id 的
//! `toolResponse`，toolResult.status 为 success 且 value.isError 不为真才算写进去了。
//! 编辑工具：developer 扩展的 write（path, content）、edit（path, before, after），老版本的 text_editor（command 为 write / str_replace / insert），
//! 名字可能带扩展前缀（`developer__write`）。shell 命令的原文作为命令里提到的文件的候选证据。
//! 模型优先取 usage_ledger 里那次调用之前最近的一条，没有就取会话的 model_config_json。

use crate::{added_lines, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use rusqlite::types::Value as Sql;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Goose 的数据目录（sessions/ 的上一级）
pub fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(p) = env("CREDITIA_GOOSE_DIR") {
        return p;
    }
    if let Some(p) = env("GOOSE_PATH_ROOT").filter(|p| p.is_absolute()) {
        return p.join("data");
    }
    #[cfg(target_os = "windows")]
    if let Some(p) = env("APPDATA") {
        return p.join("Block/goose/data");
    }
    crate::xdg_data().join("goose")
}

fn db_path() -> PathBuf {
    data_dir().join("sessions/sessions.db")
}

/// TIMESTAMP 列：sqlx 存成 `YYYY-MM-DD HH:MM:SS` 文本，也可能是数字
fn ts_of(v: &Sql) -> i64 {
    match v {
        Sql::Integer(n) => if *n > 10_000_000_000 { n / 1000 } else { *n },
        Sql::Real(f) => *f as i64,
        Sql::Text(s) => creditia_core::util::parse_iso(s).unwrap_or(0),
        _ => 0,
    }
}

fn has_table(c: &rusqlite::Connection, t: &str) -> bool {
    c.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |_| Ok(())).is_ok()
}
fn has_column(c: &rusqlite::Connection, table: &str, col: &str) -> bool {
    c.prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1")).and_then(|mut q| q.exists([col])).unwrap_or(false)
}

struct Sess {
    id: String,
    dir: String,
    created: i64,
    updated: i64,
    tokens: i64,
    model: String,
    provider: Option<String>,
}

fn sessions(c: &rusqlite::Connection) -> Vec<Sess> {
    let col = |name: &str, or: &str| if has_column(c, "sessions", name) { name.to_string() } else { or.to_string() };
    let sql = format!(
        "SELECT id, working_dir, created_at, updated_at, {}, {}, {}, {} FROM sessions",
        col("accumulated_total_tokens", "NULL"),
        col("total_tokens", "NULL"),
        col("model_config_json", "NULL"),
        col("provider_name", "NULL")
    );
    let Ok(mut q) = c.prepare(&sql) else { return vec![] };
    let rows = q.query_map([], |r| {
        let cfg: Value = r.get::<_, Option<String>>(6)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Ok(Sess {
            id: r.get(0)?,
            dir: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            created: ts_of(&r.get(2)?),
            updated: ts_of(&r.get(3)?),
            tokens: r.get::<_, Option<i64>>(4)?.or(r.get::<_, Option<i64>>(5)?).unwrap_or(0),
            model: cfg["model_name"].as_str().unwrap_or("").to_string(),
            provider: r.get::<_, Option<String>>(7)?.filter(|s| !s.is_empty()),
        })
    });
    rows.into_iter().flatten().flatten().collect()
}

/// 一次成功的工具调用写下的（路径, 新增的行），或者 shell 命令原文（路径为空）
fn edits(name: &str, a: &Value) -> Vec<(String, String)> {
    let tool = name.rsplit("__").next().unwrap_or(name);
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    let path = s("path");
    match (tool, a["command"].as_str()) {
        ("write", _) => vec![(path, s("content"))],
        ("edit", _) => vec![(path, added_lines(&s("before"), &s("after")))],
        ("text_editor", Some("write")) => vec![(path, s("file_text"))],
        ("text_editor", Some("str_replace")) => vec![(path, added_lines(&s("old_str"), &s("new_str")))],
        ("text_editor", Some("insert")) => vec![(path, s("new_str"))],
        ("shell", Some(cmd)) => vec![(String::new(), cmd.to_string())],
        _ => vec![],
    }
    .into_iter()
    .filter(|(_, t)| !t.trim().is_empty())
    .collect()
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = db_path();
    if !db.is_file() {
        return;
    }
    let gk = "goose:sessions.db".to_string();
    let fp = {
        let a = fingerprint(&db);
        let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
        (a.0.max(b.0), a.1 + b.1)
    };
    if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
        let n = g.sessions.len();
        if n > 0 {
            st.notes.push(creditia_core::tr!("goose: 项目相关会话 {n} 个", "goose: {n} sessions for this project"));
        }
        return;
    }
    let c = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => {
            st.warnings.push(creditia_core::tr!("goose: 打不开 {}：{e}", "goose: couldn't open {}: {e}", db.display()));
            return;
        }
    };
    let mut g = FileGroup { fp, ..Default::default() };
    // usage_ledger：每次调用的模型（后来加的表）
    let mut ledger: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    if has_table(&c, "usage_ledger") {
        if let Ok(mut q) = c.prepare("SELECT session_id, created_timestamp, model FROM usage_ledger WHERE model IS NOT NULL AND model != '' ORDER BY created_timestamp, id") {
            for (sid, ts, m) in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))).into_iter().flatten().flatten() {
                ledger.entry(sid).or_default().push((ts, m));
            }
        }
    }
    let mut msgs = c.prepare("SELECT role, content_json, created_timestamp FROM messages WHERE session_id = ?1 ORDER BY created_timestamp, id").ok();
    for s in sessions(&c).into_iter().filter(|s| pm.in_project(&s.dir)) {
        let rows: Vec<(String, String, i64)> = msgs
            .as_mut()
            .and_then(|q| q.query_map([&s.id], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<i64>>(2)?.unwrap_or(0)))).ok().map(|it| it.flatten().collect()))
            .unwrap_or_default();
        // 成功的工具结果
        let mut ok: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut prompts = 0;
        let mut parsed: Vec<(String, Value, i64)> = vec![];
        for (role, json, ts) in rows {
            let blocks: Value = serde_json::from_str(&json).unwrap_or_default();
            let mut typed = false;
            for b in blocks.as_array().into_iter().flatten() {
                match b["type"].as_str() {
                    Some("toolResponse") => {
                        let r = &b["toolResult"];
                        if r["status"] == "success" && r["value"]["isError"] != true {
                            ok.insert(b["id"].as_str().unwrap_or("").to_string());
                        }
                    }
                    Some("text") if role == "user" && b["text"].as_str().is_some_and(|t| !t.trim().is_empty()) => typed = true,
                    _ => {}
                }
            }
            if typed {
                prompts += 1;
            }
            parsed.push((role, blocks, if ts > 10_000_000_000 { ts / 1000 } else { ts }));
        }
        let turns = ledger.get(&s.id);
        let mut end = s.updated;
        let mut any = false;
        for (role, blocks, ts) in &parsed {
            end = end.max(*ts);
            if role != "assistant" {
                continue;
            }
            any = true;
            let model = turns.and_then(|t| t.iter().rev().find(|x| x.0 <= *ts + 1).or(t.first())).map(|x| x.1.clone()).unwrap_or_else(|| s.model.clone());
            for b in blocks.as_array().into_iter().flatten().filter(|b| b["type"] == "toolRequest") {
                let call = &b["toolCall"];
                if call["status"] != "success" || !ok.contains(b["id"].as_str().unwrap_or("")) {
                    continue;
                }
                let v = &call["value"];
                for (path, text) in edits(v["name"].as_str().unwrap_or(""), &v["arguments"]) {
                    if path.is_empty() {
                        for rel in crate::shell_targets(&text, &s.dir, pm) {
                            crate::push_rel(&mut g.evs, &rel, &text, Src::Goose, &model, &s.id, *ts);
                        }
                    } else {
                        let abs = if Path::new(&path).is_absolute() { path } else { format!("{}/{}", s.dir.trim_end_matches('/'), path) };
                        crate::push_text(&mut g.evs, pm, &abs, &text, Src::Goose, &model, &s.id, *ts);
                    }
                }
            }
        }
        if !any {
            continue;
        }
        // 会话的模型：账本里最多的，没有就是配置里的
        let model = turns
            .and_then(|t| {
                let mut m: HashMap<&str, usize> = HashMap::new();
                for x in t {
                    *m.entry(x.1.as_str()).or_default() += 1;
                }
                m.into_iter().max_by_key(|(k, n)| (*n, std::cmp::Reverse(*k))).map(|(k, _)| k.to_string())
            })
            .unwrap_or_else(|| s.model.clone());
        g.sessions.push(Session { src: Src::Goose, id: s.id.clone(), model, start: s.created, end, tokens: s.tokens, branch: None, prompts, reviewer: false, has_content: true, provider: s.provider.clone() });
    }
    let n = g.sessions.len();
    store.groups.insert(gk, g);
    if n > 0 {
        st.notes.push(creditia_core::tr!("goose: 项目相关会话 {n} 个", "goose: {n} sessions for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let Ok(c) = open_ro(&db_path()) else { return vec![] };
    let mut m: HashMap<String, Activity> = HashMap::new();
    for s in sessions(&c).into_iter().filter(|s| !s.dir.is_empty()) {
        let e = m.entry(s.dir.clone()).or_insert(Activity { src: Src::Goose, cwd: s.dir.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(s.updated);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let db = db_path();
    let ss = open_ro(&db).map(|c| sessions(&c)).unwrap_or_default();
    Health {
        source: "goose".into(),
        path: db.display().to_string(),
        readable: db.is_file(),
        format: "sessions.db (sessions + messages + usage_ledger)".into(),
        records: ss.len() as u64,
        earliest: ss.iter().map(|s| s.created).filter(|&t| t > 0).min().map(date_local),
        latest: ss.iter().map(|s| s.updated).max().filter(|&t| t > 0).map(date_local),
        note: creditia_core::tr!(
            "会话含工作目录、模型与渠道；工具调用逐条配结果。尚未用真实日志核对",
            "sessions carry working dir, model and provider; each tool call is paired with its result. Not yet checked against real logs"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_calls_to_edits() {
        assert_eq!(edits("developer__write", &json!({ "path": "/r/a.rs", "content": "fn a() {}" })), vec![("/r/a.rs".into(), "fn a() {}".into())]);
        assert_eq!(edits("edit", &json!({ "path": "a.rs", "before": "keep\nold", "after": "keep\nnew" })), vec![("a.rs".into(), "new\n".into())]);
        assert_eq!(edits("developer__text_editor", &json!({ "command": "str_replace", "path": "/r/b.rs", "old_str": "x", "new_str": "y" })), vec![("/r/b.rs".into(), "y\n".into())]);
        assert_eq!(edits("developer__text_editor", &json!({ "command": "view", "path": "/r/b.rs" })), vec![]);
        assert_eq!(edits("shell", &json!({ "command": "cat > a.rs <<EOF\nx\nEOF" })), vec![(String::new(), "cat > a.rs <<EOF\nx\nEOF".into())]);
        assert_eq!(edits("tree", &json!({ "path": "/r" })), vec![]);
    }

    #[test]
    fn timestamps() {
        assert_eq!(ts_of(&Sql::Text("2026-09-22 10:00:00".into())), 1_790_071_200);
        assert_eq!(ts_of(&Sql::Integer(1_790_071_200_000)), 1_790_071_200);
        assert_eq!(ts_of(&Sql::Null), 0);
    }
}
