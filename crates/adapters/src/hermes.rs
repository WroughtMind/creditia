//! Hermes state.db, verified against NousResearch/hermes-agent hermes_state_common.py,
//! hermes_state_messages.py, tools/file_tools.py and tools/file_operations_common.py.
//! Only successful patch receipts containing the actual diff are edit evidence.
use crate::{trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn db_path() -> PathBuf {
    let root = std::env::var_os("CREDITIA_HERMES_AGENT_DIR")
        .or_else(|| std::env::var_os("HERMES_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".hermes"));
    if root.is_file() {
        root
    } else {
        root.join("state.db")
    }
}

fn columns(db: &Connection, table: &str) -> rusqlite::Result<HashSet<String>> {
    db.prepare(&format!("pragma table_info({table})"))?
        .query_map([], |r| r.get(1))?
        .collect()
}

fn load(path: &Path) -> Result<Vec<(Value, Vec<Value>)>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    // A transaction gives session metadata and message receipts one consistent snapshot.
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let sc = columns(&db, "sessions").map_err(|e| e.to_string())?;
    let mc = columns(&db, "messages").map_err(|e| e.to_string())?;
    if !["id", "started_at"].iter().all(|c| sc.contains(*c))
        || ![
            "session_id",
            "role",
            "content",
            "tool_calls",
            "tool_call_id",
            "timestamp",
        ]
        .iter()
        .all(|c| mc.contains(*c))
    {
        return Err("unrecognized Hermes sessions/messages schema".into());
    }
    let obj = |cols: &HashSet<String>, names: &[&str]| {
        names
            .iter()
            .map(|n| format!("'{n}',{}", if cols.contains(*n) { *n } else { "null" }))
            .collect::<Vec<_>>()
            .join(",")
    };
    let ss = obj(
        &sc,
        &[
            "id",
            "cwd",
            "started_at",
            "ended_at",
            "parent_session_id",
            "input_tokens",
            "output_tokens",
        ],
    );
    let ms = obj(
        &mc,
        &[
            "role",
            "content",
            "tool_calls",
            "tool_call_id",
            "timestamp",
            "effect_disposition",
        ],
    );
    let sql = format!("select json_object({ss}) from sessions order by started_at, id");
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let sessions = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let active = if mc.contains("active") {
        " and active=1"
    } else {
        ""
    };
    let sql = format!(
        "select json_object({ms}) from messages where session_id=?1{active} order by timestamp, id"
    );
    let mut msgs = db.prepare(&sql).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for raw in sessions {
        let s: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let rows = msgs
            .query_map([s["id"].as_str().unwrap_or("")], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut values = vec![];
        for row in rows {
            values.push(
                serde_json::from_str(&row.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            );
        }
        out.push((s, values));
    }
    Ok(out)
}

fn parse(session: &Value, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let Some(id) = session["id"].as_str().filter(|s| !s.is_empty()) else {
        return;
    };
    let cwd = session["cwd"].as_str().unwrap_or("");
    let start = crate::any_ts(&session["started_at"]).unwrap_or(0);
    let mut end = start;
    let mut pending: HashMap<String, String> = HashMap::new();
    let mut completed = HashSet::new();
    let before = g.evs.len();
    let mut prompts = 0;
    for row in rows {
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(0);
        // Fork/compression inherits old messages; their original timestamps precede the child.
        if !session["parent_session_id"].is_null() && (ts == 0 || ts < start) {
            continue;
        }
        end = end.max(ts);
        if row["role"] == "user" {
            prompts += 1;
        }
        if row["role"] == "assistant" {
            let calls: Value = row["tool_calls"]
                .as_str()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(Value::Null);
            for call in calls.as_array().into_iter().flatten() {
                if let (Some(id), Some(name)) =
                    (call["id"].as_str(), call["function"]["name"].as_str())
                {
                    pending.insert(id.into(), name.into());
                }
            }
        }
        if row["role"] != "tool"
            || row["effect_disposition"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        let Some(call_id) = row["tool_call_id"].as_str() else {
            continue;
        };
        if pending.remove(call_id).as_deref() != Some("patch") || !completed.insert(call_id) {
            continue;
        }
        let result: Value = row["content"]
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null);
        if result["success"] != true
            || result["no_change"] == true
            || result["error"].as_str().is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        let diff = result["diff"].as_str().unwrap_or("");
        let files = imp::diff_files(diff);
        // The receipt records actual resolved targets. Never substitute requested text/path.
        for (path, added) in &files {
            let path = if files.len() == 1 {
                result["resolved_path"].as_str().unwrap_or(path)
            } else {
                path
            };
            let confirmed = result["files_modified"]
                .as_array()
                .is_some_and(|xs| xs.iter().any(|v| v.as_str() == Some(path)))
                || result["files_created"]
                    .as_array()
                    .is_some_and(|xs| xs.iter().any(|v| v.as_str() == Some(path)));
            if confirmed {
                imp::push_scoped(g, pm, cwd, path, added, Src::HermesAgent, "", id, ts);
            }
        }
    }
    if pm.in_project(cwd) || g.evs.len() > before {
        g.reconciled_sessions.push((Src::HermesAgent, id.into()));
        let mut s = imp::session(
            Src::HermesAgent,
            id,
            "",
            start,
            end,
            prompts,
            g.evs.len() > before,
        );
        s.tokens = session["input_tokens"].as_i64().unwrap_or(0)
            + session["output_tokens"].as_i64().unwrap_or(0);
        g.sessions.push(s);
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let path = db_path();
    if !path.is_file() {
        return;
    }
    match load(&path) {
        Ok(sessions) => {
            let mut g = FileGroup::default();
            for (s, rows) in sessions {
                parse(&s, &rows, pm, &mut g);
            }
            store.groups.insert("hermes-agent:native".into(), g);
        }
        Err(e) => {
            if let Some(g) = store.groups.get_mut("hermes-agent:native") {
                g.reconciled_sessions.clear();
            }
            st.warnings
                .push(format!("Hermes Agent {}: {e}", path.display()));
        }
    }
}
pub fn discover() -> Vec<Activity> {
    load(&db_path())
        .unwrap_or_default()
        .iter()
        .filter_map(|(s, _)| {
            Some(imp::activity(
                Src::HermesAgent,
                s["cwd"].as_str()?.into(),
                None,
                crate::any_ts(&s["ended_at"])
                    .or_else(|| crate::any_ts(&s["started_at"]))
                    .unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    let path = db_path();
    let result = if path.is_file() {
        load(&path)
    } else {
        Ok(vec![])
    };
    imp::health(Src::HermesAgent, &path, "state.db sessions/messages; successful patch diff", result.as_ref().map_or(0, Vec::len),
        &result.err().unwrap_or_else(|| "Only paired successful patch receipts; no-change, inactive and inherited messages excluded. Full writes without before are excluded; execution model remains unknown. Contract-tested, no account runtime validation.".into()), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    fn rows() -> Vec<Value> {
        vec![
            json!({"role":"assistant","timestamp":1002,"tool_calls":json!([{"id":"c1","function":{"name":"patch","arguments":"unused"}}]).to_string()}),
            json!({"role":"tool","timestamp":1003,"tool_call_id":"c1","content":json!({"success":true,"resolved_path":"/repo/a.rs","files_modified":["/repo/a.rs"],"diff":"--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old_line(42);\n+new_line(73);\n"}).to_string()}),
        ]
    }
    #[test]
    fn receipt_not_request_determines_lines_and_mutable_model_is_not_used() {
        let s = json!({"id":"s","cwd":"/repo","started_at":1000,"model":"mutable-final-model"});
        let mut g = FileGroup::default();
        parse(&s, &rows(), &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.evs[0].ts, 1003);
        assert_eq!(g.reconciled_sessions, vec![(Src::HermesAgent, "s".into())]);
        for field in ["success", "no_change", "error"] {
            let mut r = rows();
            let mut v: Value = serde_json::from_str(r[1]["content"].as_str().unwrap()).unwrap();
            v[field] = match field {
                "success" => json!(false),
                "no_change" => json!(true),
                _ => json!("failed"),
            };
            r[1]["content"] = json!(v.to_string());
            let mut g = FileGroup::default();
            parse(&s, &r, &pm(), &mut g);
            assert!(g.evs.is_empty());
            assert!(!g.reconciled_sessions.is_empty());
        }
    }
    #[test]
    fn inherited_and_unpaired_results_do_not_attribute() {
        let s = json!({"id":"child","cwd":"/repo","parent_session_id":"parent","started_at":1100});
        let mut g = FileGroup::default();
        parse(&s, &rows(), &pm(), &mut g);
        assert!(g.evs.is_empty());
        let s = json!({"id":"s","cwd":"/repo","started_at":1000});
        parse(&s, &rows()[1..], &pm(), &mut g);
        assert!(g.evs.is_empty());
    }
}
