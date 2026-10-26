//! Kiro classic CLI persisted conversations. Official stable 2.32.0 distribution
//! (prod.download.cli.kiro.dev/stable/latest/manifest.json) checked read-only:
//! sqlite SQL uses conversations_v2(key,conversation_id,value,created_at,updated_at).
//! It still contains the classic crates/chat-cli/src/cli/chat/conversation.rs
//! ConversationState and fs_write protocol from the public aws/amazon-q-developer-cli
//! source. Only that recognizable classic shape is read here. KAS V3 sessions
//! use kiro_sessions; the Rust V2 conversation log remains a separate codec.

use crate::{Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

fn db_dir() -> PathBuf {
    if let Some(p) = std::env::var("CREDITIA_KIRO_DB_DIR")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "macos")]
    return crate::home().join("Library/Application Support/kiro-cli");
    #[cfg(target_os = "windows")]
    return PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("kiro-cli");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    crate::xdg_data().join("kiro-cli")
}
fn home() -> PathBuf {
    std::env::var("CREDITIA_KIRO_DIR")
        .ok()
        .or_else(|| std::env::var("KIRO_HOME").ok())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".kiro"))
}
fn db_path() -> PathBuf {
    db_dir().join("data.sqlite3")
}
fn recognizable_db(c: &rusqlite::Connection) -> bool {
    c.prepare("SELECT key,value FROM conversations_v2 ORDER BY updated_at DESC")
        .is_ok()
        || c.prepare("SELECT key,value FROM conversations").is_ok()
}
fn conversations(c: &rusqlite::Connection) -> Vec<(String, Value)> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    // Classic multi-session table is current in 2.32.0. Earlier migration rows
    // can coexist, so prefer v2 and retain only distinct conversation IDs.
    for sql in [
        "SELECT key,value FROM conversations_v2 ORDER BY updated_at DESC",
        "SELECT key,value FROM conversations",
    ] {
        let Ok(mut q) = c.prepare(sql) else {
            continue;
        };
        for (cwd, text) in q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .into_iter()
            .flatten()
            .flatten()
        {
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            // Do not reinterpret V2/V3 events or arbitrary JSON as classic.
            if !v["history"].is_array() || !v["conversation_id"].is_string() {
                continue;
            }
            if seen.insert(v["conversation_id"].as_str().unwrap_or("").to_string()) {
                out.push((cwd, v));
            }
        }
    }
    out
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let native_ids = crate::kiro_sessions::session_ids();
    store.groups.retain(|key, group| {
        !key.starts_with("kiro-classic:")
            || !group.sessions.iter().any(|s| native_ids.contains(&s.id))
    });
    let db = db_path();
    if !db.is_file() {
        return;
    }
    let c = match crate::open_ro(&db) {
        Ok(c) => c,
        Err(e) => {
            st.warnings
                .push(format!("kiro classic: cannot read {}: {e}", db.display()));
            return;
        }
    };
    let a = fingerprint(&db);
    let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
    let fp = (a.0.max(b.0), a.1 + b.1);
    for (cwd, v) in conversations(&c).into_iter().filter(|(cwd, v)| {
        pm.in_project(cwd)
            && !v["conversation_id"]
                .as_str()
                .is_some_and(|id| native_ids.contains(id))
    }) {
        let p = crate::amazonq::parse_request_models(&v);
        let key = format!("kiro-classic:{}:{}", db.display(), p.id);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for (path, text, model, ts) in &p.edits {
            if path.is_empty() {
                for rel in crate::shell_targets(text, &cwd, pm) {
                    crate::push_rel(&mut g.evs, &rel, text, Src::Kiro, model, &p.id, *ts);
                }
                continue;
            }
            let path = path
                .strip_prefix("~/")
                .map(|p| crate::home().join(p).display().to_string())
                .unwrap_or_else(|| path.clone());
            let path = crate::harness_paths::target(&path, &cwd);
            crate::push_text(&mut g.evs, pm, &path, text, Src::Kiro, model, &p.id, *ts);
        }
        let model = p
            .models
            .iter()
            .max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone())))
            .map(|(k, _)| k.clone())
            .unwrap_or_default();
        g.sessions.push(Session {
            src: Src::Kiro,
            id: p.id,
            model,
            start: p.start,
            end: p.end,
            tokens: 0,
            branch: None,
            prompts: p.prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    let Ok(c) = crate::open_ro(&db_path()) else {
        return vec![];
    };
    let native_ids = crate::kiro_sessions::session_ids();
    conversations(&c)
        .into_iter()
        .filter(|(_, v)| {
            !v["conversation_id"]
                .as_str()
                .is_some_and(|id| native_ids.contains(id))
        })
        .map(|(cwd, v)| {
            let p = crate::amazonq::parse_request_models(&v);
            Activity {
                src: Src::Kiro,
                cwd,
                remote: None,
                sessions: 1,
                last: p.end,
            }
        })
        .collect()
}
pub fn doctor() -> Vec<Health> {
    let db = db_path();
    let parsed: Vec<_> = crate::open_ro(&db)
        .map(|c| {
            conversations(&c)
                .iter()
                .map(|(_, v)| crate::amazonq::parse_request_models(v))
                .collect()
        })
        .unwrap_or_default();
    let v2 = home().join("sessions");
    let records = walkdir::WalkDir::new(&v2)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name() == "conversation_log.jsonl")
        .count();
    vec![Health{source:"kiro-cli-classic".into(),path:db.display().to_string(),readable:crate::open_ro(&db).is_ok_and(|c|recognizable_db(&c)),format:"data.sqlite3 conversations_v2 / legacy conversations; classic ConversationState JSON".into(),records:parsed.len()as u64,earliest:parsed.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_cn),latest:parsed.iter().map(|p|p.end).filter(|&t|t>0).max().map(date_cn),note:creditia_core::tr!("已用官方 stable 2.32.0 发行包和公开前身源码核对 SQL 与 classic 格式。fs_write 必须匹配 Success 结果，模型取逐请求 model_id。实现已通过构造记录测试，尚未用真实会话验收。", "Official stable 2.32.0 SQL and classic schema verified against distribution and public predecessor source. Uses fs_write only after matching Success results and per-request model_id. Implementation validated with fixtures, not yet with real sessions."),..Default::default()},
    Health{source:"kiro-cli-v2".into(),path:v2.display().to_string(),readable:false,format:"Rust V2 session_metadata.json + conversation_log.jsonl (LogEntryV1)".into(),records:records as u64,note:creditia_core::tr!("官方 2.32.0 Rust 引擎确认使用 session_metadata.json 与 conversation_log.jsonl，独立于 classic SQLite 和已实现的 KAS V3 messages.jsonl。V2 的 LogEntryV1 成功回执序列化尚未完整复核，当前不解析；可用官方 ACP 采集后续运行。", "Official 2.32.0 Rust engine uses session_metadata.json and conversation_log.jsonl, separately from classic SQLite and the implemented KAS V3 messages.jsonl. V2 LogEntryV1 successful-result serialization is not fully verified and is not parsed; official ACP capture is available for future runs."),unsupported:true,..Default::default()}]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn classic_v2_sql_and_real_per_request_results() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE conversations_v2(key TEXT,conversation_id TEXT,value TEXT,created_at INTEGER,updated_at INTEGER);CREATE TABLE conversations(key TEXT,value TEXT);").unwrap();
        let v = json!({"conversation_id":"kiro-s","history":[{"request_metadata":{"model_id":"model-a","request_start_timestamp_ms":1790000000000i64},"assistant":{"ToolUse":{"tool_uses":[{"id":"a","name":"fs_write","args":{"command":"str_replace","path":"x.rs","old_str":"human context","new_str":"human context\ncreated line"}},{"id":"b","name":"fs_write","args":{"command":"create","path":"x.rs","file_text":"failed line"}},{"id":"overwrite","name":"fs_write","args":{"command":"create","path":"x.rs","file_text":"human context\ncreated line"}}]}}},{"request_metadata":{"model_id":"model-b"},"user":{"content":{"ToolUseResults":{"tool_use_results":[{"tool_use_id":"a","status":"Success"},{"tool_use_id":"b","status":"Error"},{"tool_use_id":"overwrite","status":"Success"}]}}}}]});
        c.execute(
            "INSERT INTO conversations_v2 VALUES('/repo','kiro-s',?1,1,2)",
            [v.to_string()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO conversations VALUES('/repo',?1)",
            [v.to_string()],
        )
        .unwrap();
        let rows = conversations(&c);
        assert_eq!(rows.len(), 1);
        let p = crate::amazonq::parse_request_models(&rows[0].1);
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].2, "model-a");
        let mut missing = rows[0].1.clone();
        missing["model"] = json!("current-model-must-not-backfill");
        missing["history"][0]["request_metadata"] = Value::Null;
        let p = crate::amazonq::parse_request_models(&missing);
        assert_eq!(p.edits[0].2, "");
        missing["history"][0]["assistant"]["ToolUse"]["tool_uses"][0]["id"] = Value::Null;
        missing["history"][1]["user"]["content"]["ToolUseResults"]["tool_use_results"][0]
            ["tool_use_id"] = Value::Null;
        assert!(crate::amazonq::parse_request_models(&missing)
            .edits
            .is_empty());
        c.execute(
            "INSERT INTO conversations_v2 VALUES('/repo','v3',?1,1,3)",
            [json!({"version":3,"messages":[]}).to_string()],
        )
        .unwrap();
        assert_eq!(conversations(&c).len(), 1);
    }
}
