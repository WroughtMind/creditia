//! MiniMax Code, upstream 9a93560: v2/sqlite/runtime-state.sqlite and
//! canonical v2/sessions/**/messages.jsonl; earlier pi history rows / JSON fallback.
//! Canonical history wins over migrated SQL history. Pi wire messages preserve
//! actual model and paired isError results; this is not Claude/OpenCode guessing.
use crate::{any_ts, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Src, Store};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
pub fn data_dir() -> PathBuf {
    [
        "CREDITIA_MINIMAX_CODE_DIR",
        "MINIMAX_DATA_DIR",
        "MAVIS_DATA_DIR",
    ]
    .iter()
    .find_map(|k| {
        std::env::var(k)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    })
    .unwrap_or_else(|| crate::home().join(".minimax"))
}
fn roots() -> Vec<PathBuf> {
    let p = data_dir();
    let mut roots = vec![p.clone()];
    if p == crate::home().join(".minimax") {
        for e in std::fs::read_dir(crate::home())
            .into_iter()
            .flatten()
            .flatten()
        {
            if e.file_name().to_string_lossy().starts_with(".minimax-")
                && e.path().join("v2/sqlite/runtime-state.sqlite").is_file()
            {
                roots.push(e.path());
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}
fn has(c: &rusqlite::Connection, t: &str, col: &str) -> bool {
    c.prepare(&format!(
        "SELECT 1 FROM pragma_table_info('{t}') WHERE name=?1"
    ))
    .and_then(|mut q| q.exists([col]))
    .unwrap_or(false)
}
fn sessions(c: &rusqlite::Connection) -> Vec<Value> {
    let table = "local_runtime_sessions";
    let col = |s: &str| {
        if has(c, table, s) {
            s.to_string()
        } else {
            "NULL".into()
        }
    };
    let sql=format!("SELECT session_id,record_json,updated_at_ms,{},{},{} FROM local_runtime_sessions ORDER BY updated_at_ms,session_id",col("workspace_dir"),col("created_at_ms"),col("history_relative_dir"));
    let Ok(mut q) = c.prepare(&sql) else {
        return vec![];
    };
    q.query_map([], |r| {
        let mut s: Value = r
            .get::<_, String>(1)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .filter(Value::is_object)
            .unwrap_or(json!({}));
        s["sessionId"] = json!(r.get::<_, String>(0)?);
        s["updatedAtMs"] = json!(r.get::<_, i64>(2)?);
        for (i, k) in [(3, "workspaceDir"), (5, "historyRelativeDir")] {
            if let Ok(Some(v)) = r.get::<_, Option<String>>(i) {
                s[k] = json!(v);
            }
        }
        if let Ok(Some(v)) = r.get::<_, Option<i64>>(4) {
            s["createdAtMs"] = json!(v);
        }
        Ok(s)
    })
    .into_iter()
    .flatten()
    .flatten()
    .collect()
}
fn history(c: &rusqlite::Connection, root: &Path, s: &Value) -> Vec<Value> {
    if let Some(rel) = s["historyRelativeDir"].as_str() {
        let rel = Path::new(rel);
        if !rel.is_absolute()
            && !rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            let p = root.join("v2/sessions").join(rel).join("messages.jsonl");
            if p.is_file() {
                return crate::pi::read_jsonl(&p);
            }
        }
    }
    // Older dated canonical stores did not persist historyRelativeDir.
    let sid = s["sessionId"].as_str().unwrap_or("");
    for p in crate::pi::jsonl_files(&root.join("v2/sessions")) {
        if p.file_name().is_some_and(|n| n == "messages.jsonl") {
            let manifest = p.with_file_name("manifest.json");
            let v: Value = std::fs::read_to_string(manifest)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
            if v["sessionId"] == sid {
                return crate::pi::read_jsonl(&p);
            }
        }
    }
    if has(c, "local_runtime_pi_history_rows", "data_json") {
        if let Ok(mut q) = c.prepare(
            "SELECT data_json FROM local_runtime_pi_history_rows WHERE session_id=?1 ORDER BY id",
        ) {
            let rows: Vec<Value> = q
                .query_map([sid], |r| r.get::<_, String>(0))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|s| serde_json::from_str(&s).ok())
                .collect();
            if !rows.is_empty() {
                return rows;
            }
        }
    }
    c.query_row(
        "SELECT pi_history_json FROM local_runtime_messages WHERE session_id=?1",
        [sid],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    .and_then(|v| v.as_array().cloned())
    .unwrap_or_default()
}
fn normalized(s: &Value, rows: &[Value]) -> Vec<Value> {
    let mut out = vec![
        json!({"type":"session","id":s["sessionId"],"cwd":s["workspaceDir"],"timestamp":s["createdAtMs"]}),
    ];
    for r in rows {
        let mut m = if r["message"].is_object() {
            r["message"].clone()
        } else {
            r.clone()
        };
        if m["role"] == "assistant" {
            // Native MiniMax edit has old_string/new_string, unlike Pi oldText.
            if let Some(blocks) = m["content"].as_array_mut() {
                for b in blocks {
                    if b["type"] == "toolCall" && b["name"] == "edit" {
                        let a = &mut b["arguments"];
                        if a["new_string"].is_string() {
                            a["oldText"] = a["old_string"].clone();
                            a["newText"] = a["new_string"].clone();
                        }
                    }
                }
            }
        }
        let ts = m["timestamp"].clone();
        out.push(json!({"type":"message","timestamp":ts,"message":m}));
    }
    out
}
fn parse(s: &Value, rows: &[Value], pm: &PathMap) -> FileGroup {
    crate::pi::parse(Src::MiniMax, &normalized(s, rows), pm)
}
pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    let prefix = "minimax-code:db:";
    let mut keys = std::collections::HashSet::new();
    for root in roots() {
        let db = root.join("v2/sqlite/runtime-state.sqlite");
        if !db.is_file() {
            continue;
        }
        let key = format!("{prefix}{}", db.display());
        keys.insert(key.clone());
        // Canonical files can change before the SQL projection catches up.
        let mut fps = vec![
            fingerprint(&db),
            fingerprint(&PathBuf::from(format!("{}-wal", db.display()))),
        ];
        fps.extend(
            crate::pi::jsonl_files(&root.join("v2/sessions"))
                .iter()
                .map(|p| fingerprint(p)),
        );
        let fp = (
            fps.iter().map(|f| f.0).max().unwrap_or(0),
            fps.iter().map(|f| f.1).sum(),
        );
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let Ok(c) = open_ro(&db) else {
            continue;
        };
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for s in sessions(&c) {
            if !pm.in_project(s["workspaceDir"].as_str().unwrap_or("")) {
                continue;
            }
            let part = parse(&s, &history(&c, &root, &s), pm);
            g.evs.extend(part.evs);
            g.sessions.extend(part.sessions);
        }
        store.groups.insert(key, g);
    }
    store
        .groups
        .retain(|k, _| !k.starts_with(prefix) || keys.contains(k));
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    for root in roots() {
        let Ok(c) = open_ro(&root.join("v2/sqlite/runtime-state.sqlite")) else {
            continue;
        };
        for s in sessions(&c) {
            let Some(cwd) = s["workspaceDir"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            let e = out.entry(cwd.into()).or_insert(Activity {
                src: Src::MiniMax,
                cwd: cwd.into(),
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(any_ts(&s["updatedAtMs"]).unwrap_or(0));
        }
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    Health{source:"minimax-code".into(),path:data_dir().join("v2/sqlite/runtime-state.sqlite").display().to_string(),readable:roots().iter().any(|p|p.join("v2/sqlite/runtime-state.sqlite").is_file()),format:"runtime-state.sqlite + canonical messages.jsonl / legacy Pi history".into(),records:discover().iter().map(|a|a.sessions).sum(),note:creditia_core::tr!("原生 Pi 消息配对成功edit差分，保留每条回复真实模型；write无before或新建证明则排除；新文件历史优先于迁移 SQL 历史。尚未真实运行验收。", "Native Pi messages require successful edit deltas and preserve each response model; full writes without before/create proof are excluded. Canonical file history takes precedence over migrated SQL history. Runtime validation remains pending."),..Default::default()}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_full_write_cannot_claim_retained_human_line() {
        let human = "human_original_line = minimax_existing_work();";
        let new = "model_new_line = minimax_added_work();";
        let after = format!("{human}\n{new}");
        let s = json!({"sessionId":"mm-delta","workspaceDir":"/repo","createdAtMs":1});
        let rows = vec![
            json!({"message":{"role":"assistant","model":"actual","content":[{"type":"toolCall","id":"write","name":"write","arguments":{"path":"a.rs","content":after}},{"type":"toolCall","id":"edit","name":"edit","arguments":{"file_path":"a.rs","old_string":human,"new_string":after}},{"type":"toolCall","id":"bad","name":"edit","arguments":{"file_path":"a.rs","old_string":human,"new_string":"failed_only_line"}}]}}),
            json!({"message":{"role":"toolResult","toolCallId":"write","isError":false,"content":[{"type":"text","text":"Successfully wrote a.rs"}]}}),
            json!({"message":{"role":"toolResult","toolCallId":"edit","isError":false}}),
            json!({"message":{"role":"toolResult","toolCallId":"bad","isError":true}}),
        ];
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse(&s, &rows, &pm);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "actual");
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key("a.rs", &creditia_core::hash::normalize(new, ".rs"))
        );
        assert_ne!(
            g.evs[0].key,
            creditia_core::hash::line_key("a.rs", &creditia_core::hash::normalize(human, ".rs"))
        );
    }
    #[test]
    fn native_edit_failure_and_model_switch() {
        let s = json!({"sessionId":"mm","workspaceDir":"/repo","createdAtMs":1});
        let rows = vec![
            json!({"message_id":"1","message":{"role":"assistant","model":"first","content":[{"type":"toolCall","id":"a","name":"edit","arguments":{"file_path":"/repo/a.rs","old_string":"keep\nold","new_string":"keep\nlet first = 1;"}}]}}),
            json!({"message_id":"2","message":{"role":"toolResult","toolCallId":"a","isError":false}}),
            json!({"message_id":"3","message":{"role":"assistant","model":"second","content":[{"type":"toolCall","id":"b","name":"edit","arguments":{"path":"a.rs","old_string":"human_original_line","new_string":"human_original_line\nlet second = 2;"}},{"type":"toolCall","id":"bad","name":"write","arguments":{"path":"a.rs","content":"let bad = 3;"}}]}}),
            json!({"message":{"role":"toolResult","toolCallId":"bad","isError":true}}),
            json!({"message":{"role":"toolResult","toolCallId":"b","isError":false}}),
        ];
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse(&s, &rows, &pm);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "first");
        assert_eq!(g.evs[1].model, "second");
        assert_eq!(g.sessions[0].src, Src::MiniMax);
    }
}
