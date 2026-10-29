//! Amazon Q Developer CLI（aws/amazon-q-developer-cli，`q chat`）：`<本地数据目录>/amazon-q/data.sqlite3`
//! （Linux `~/.local/share`，macOS `~/Library/Application Support`，Windows `%LOCALAPPDATA%`）。
//! conversations 表按工作目录存一份对话（key 是目录，value 是 JSON）：同一目录里新开的对话会覆盖旧的，所以只读得到每个目录最近的一次，
//! 之前 analyze 过的留在证据库里。
//! history[] 每项是 user + assistant + request_metadata；assistant 的 ToolUse 里的工具调用，结果在下一项 user 的 ToolUseResults 里，
//! status 为 Success 才算。fs_write（command 为 create / str_replace / insert / append）写下的文本是证据，execute_bash 的命令原文是候选证据。
//! 模型只取历史 request_metadata.model_id（请求标识），不以对话当前选择回填；时间取 request_start_timestamp_ms，没有就取 user 的 timestamp。

use crate::{added_lines, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(p) = env("CREDITIA_AMAZONQ_DIR") {
        return p;
    }
    #[cfg(target_os = "macos")]
    return crate::home().join("Library/Application Support/amazon-q");
    #[cfg(target_os = "windows")]
    return env("LOCALAPPDATA").unwrap_or_default().join("amazon-q");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    crate::xdg_data().join("amazon-q")
}

fn db_path() -> PathBuf {
    data_dir().join("data.sqlite3")
}

/// (工作目录, 对话 JSON)
fn conversations(c: &rusqlite::Connection) -> Vec<(String, Value)> {
    let Ok(mut q) = c.prepare("SELECT key, value FROM conversations") else { return vec![] };
    let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())));
    rows.into_iter().flatten().flatten().map(|(k, v)| (k, serde_json::from_str(&v).unwrap_or_default())).collect()
}

/// 枚举的外部标签：{"ToolUse": {...}} → ("ToolUse", {...})
fn tagged(v: &Value) -> (&str, &Value) {
    v.as_object().and_then(|o| o.iter().next()).map(|(k, x)| (k.as_str(), x)).unwrap_or(("", &Value::Null))
}

fn entry_ts(e: &Value) -> i64 {
    e["request_metadata"]["request_start_timestamp_ms"].as_i64().filter(|&t| t > 0).map(|t| t / 1000).or_else(|| crate::any_ts(&e["user"]["timestamp"])).unwrap_or(0)
}

/// 一次工具调用写下的（路径, 新增的行）；命令路径为空
fn edits(name: &str, a: &Value) -> Vec<(String, String)> {
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    let out = match (name, a["command"].as_str()) {
        ("fs_write", Some("create")) => vec![(s("path"), a["file_text"].as_str().or(a["new_str"].as_str()).unwrap_or("").to_string())],
        ("fs_write", Some("str_replace")) => vec![(s("path"), added_lines(&s("old_str"), &s("new_str")))],
        ("fs_write", Some("insert" | "append")) => vec![(s("path"), s("new_str"))],
        ("execute_bash" | "execute_cmd", Some(cmd)) => vec![(String::new(), cmd.to_string())],
        _ => vec![],
    };
    out.into_iter().filter(|(_, t)| !t.trim().is_empty()).collect()
}

pub(crate) struct Parsed {
    pub(crate) id: String,
    pub(crate) edits: Vec<(String, String, String, i64)>,
    pub(crate) start: i64,
    pub(crate) end: i64,
    pub(crate) prompts: i64,
    pub(crate) models: HashMap<String, usize>,
}

pub(crate) fn parse(v: &Value) -> Parsed {
    parse_history(v, true)
}

/// Kiro shares the request history format but does not accept whole-file writes.
pub(crate) fn parse_request_models(v: &Value) -> Parsed {
    parse_history(v, false)
}

fn parse_history(v: &Value, allow_whole_write: bool) -> Parsed {
    let hist: Vec<&Value> = v["history"].as_array().map(|h| h.iter().collect()).unwrap_or_default();
    let mut p = Parsed { id: v["conversation_id"].as_str().unwrap_or("").to_string(), edits: vec![], start: 0, end: 0, prompts: 0, models: HashMap::new() };
    // 成功的工具结果：在下一项 user 里
    let mut ok: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for e in &hist {
        let (kind, c) = tagged(&e["user"]["content"]);
        match kind {
            "Prompt" => p.prompts += 1,
            "ToolUseResults" => {
                for r in c["tool_use_results"].as_array().into_iter().flatten().filter(|r| r["status"] == "Success") {
                    if let Some(id) = r["tool_use_id"].as_str().filter(|s| !s.is_empty()) { ok.insert(id); }
                }
            }
            _ => {}
        }
    }
    for e in &hist {
        let ts = entry_ts(e);
        if ts > 0 {
            p.start = if p.start == 0 { ts } else { p.start.min(ts) };
            p.end = p.end.max(ts);
        }
        let model = e["request_metadata"]["model_id"].as_str().unwrap_or("").trim().to_string();
        if !model.is_empty() {
            *p.models.entry(model.clone()).or_default() += 1;
        }
        let (kind, a) = tagged(&e["assistant"]);
        if kind != "ToolUse" {
            continue;
        }
        for u in a["tool_uses"].as_array().into_iter().flatten() {
            let Some(id) = u["id"].as_str().filter(|s| !s.is_empty()) else { continue };
            if !ok.contains(id) {
                continue;
            }
            let name = u["name"].as_str().unwrap_or("");
            let args = if u["args"].is_object() { &u["args"] } else { &u["orig_args"] };
            if !allow_whole_write && name == "fs_write" && args["command"] == "create" { continue; }
            for (path, text) in edits(name, args) {
                p.edits.push((path, text, model.clone(), ts));
            }
        }
    }
    p
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = db_path();
    if !db.is_file() {
        return;
    }
    let c = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => {
            st.warnings.push(creditia_core::tr!("amazon-q: 打不开 {}：{e}", "amazon-q: couldn't open {}: {e}", db.display()));
            return;
        }
    };
    let fp = {
        let a = fingerprint(&db);
        let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
        (a.0.max(b.0), a.1 + b.1)
    };
    let mut n = 0;
    for (cwd, v) in conversations(&c) {
        if !pm.in_project(&cwd) {
            continue;
        }
        let p = parse(&v);
        if p.id.is_empty() || p.models.is_empty() && p.edits.is_empty() {
            continue;
        }
        // 每个对话一组：同一目录换了新对话，旧对话那一组留在证据库里
        let gk = format!("amazon-q:{}", p.id);
        if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
            n += 1;
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        for (path, text, model, ts) in &p.edits {
            if path.is_empty() {
                for rel in crate::shell_targets(text, &cwd, pm) {
                    crate::push_rel(&mut g.evs, &rel, text, Src::AmazonQ, model, &p.id, *ts);
                }
                continue;
            }
            let path = path.strip_prefix("~/").map(|r| crate::home().join(r).display().to_string()).unwrap_or_else(|| path.clone());
            let abs = if Path::new(&path).is_absolute() { path } else { format!("{}/{path}", cwd.trim_end_matches('/')) };
            crate::push_text(&mut g.evs, pm, &abs, text, Src::AmazonQ, model, &p.id, *ts);
        }
        let model = p.models.iter().max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone()))).map(|(k, _)| k.clone()).unwrap_or_default();
        g.reconciled_events = g.evs.clone();
        g.sessions.push(Session { src: Src::AmazonQ, id: p.id.clone(), model, start: p.start, end: p.end, tokens: 0, branch: None, prompts: p.prompts, reviewer: false, has_content: true, provider: None });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("amazon-q: 项目相关对话 {n} 个（每个目录只留最近一次）", "amazon-q: {n} conversations for this project (only the latest per directory is kept)"));
    }
}

pub fn discover() -> Vec<Activity> {
    let Ok(c) = open_ro(&db_path()) else { return vec![] };
    conversations(&c)
        .into_iter()
        .map(|(cwd, v)| {
            let p = parse(&v);
            Activity { src: Src::AmazonQ, cwd, remote: None, sessions: 1, last: p.end }
        })
        .collect()
}

pub fn doctor() -> Health {
    let db = db_path();
    let cs: Vec<Parsed> = open_ro(&db).map(|c| conversations(&c).iter().map(|(_, v)| parse(v)).collect()).unwrap_or_default();
    Health {
        source: "amazon-q".into(),
        path: db.display().to_string(),
        readable: db.is_file(),
        format: "data.sqlite3 conversations (JSON per directory)".into(),
        records: cs.len() as u64,
        earliest: cs.iter().map(|p| p.start).filter(|&t| t > 0).min().map(date_local),
        latest: cs.iter().map(|p| p.end).max().filter(|&t| t > 0).map(date_local),
        note: creditia_core::tr!(
            "每个目录只存最近一次对话；历史请求模型字段已按官方源码核对，缺失时不以当前选择回填",
            "only the latest conversation per directory is kept; historical request model field checked against official source, never backfilled from the current selection"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conversation_to_edits() {
        let mut v = json!({ "conversation_id": "q1", "model": "claude-sonnet-4", "history": [
            { "user": { "content": { "Prompt": { "prompt": "go" } }, "timestamp": "2026-09-22T10:00:00+08:00" },
              "assistant": { "ToolUse": { "message_id": "m", "content": "", "tool_uses": [
                  { "id": "a", "name": "fs_write", "orig_name": "fs_write", "args": { "command": "create", "path": "src/x.py", "file_text": "x = 1" }, "orig_args": {} },
                  { "id": "b", "name": "fs_write", "orig_name": "fs_write", "args": { "command": "str_replace", "path": "src/x.py", "old_str": "x = 1", "new_str": "x = 1\ny = 2" }, "orig_args": {} } ] } },
              "request_metadata": { "model_id": "claude-sonnet-4.5", "request_start_timestamp_ms": 1_790_000_000_000i64 } },
            { "user": { "content": { "ToolUseResults": { "tool_use_results": [{ "tool_use_id": "a", "content": [], "status": "Success" }, { "tool_use_id": "b", "content": [], "status": "Error" }] } } },
              "assistant": { "Response": { "message_id": "n", "content": "done" } } }
        ] });
        let p = parse(&v);
        assert_eq!(p.id, "q1");
        assert_eq!(p.prompts, 1);
        // 报错的 str_replace 不算
        assert_eq!(p.edits, vec![("src/x.py".to_string(), "x = 1".to_string(), "claude-sonnet-4.5".to_string(), 1_790_000_000)]);
        assert_eq!((p.start, p.end), (1_790_000_000, 1_790_000_000));
        assert_eq!(edits("execute_bash", &json!({ "command": "ls" })), vec![(String::new(), "ls".to_string())]);
        assert_eq!(edits("fs_read", &json!({ "path": "a" })), vec![]);
        // /model changes selection, not the identity of earlier requests.
        v["history"][0]["request_metadata"]["model_id"] = Value::Null;
        v["model_info"] = json!({"model_id": "claude-opus-4.5"});
        let p = parse(&v);
        assert!(p.edits[0].2.is_empty(), "current selection must not relabel historical edits");
        assert!(p.models.is_empty());
    }
}
