//! Zenflow 2.5.3 persisted normalized LogMsg JSON patches (db.sqlite).
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    if let Some(p) =
        std::env::var_os("CREDITIA_ZENFLOW_DIR").or_else(|| std::env::var_os("ZENFLOW_DATA_DIR"))
    {
        return p.into();
    }
    #[cfg(target_os = "macos")]
    let p = crate::home().join("Library/Application Support/ai.forgoodai.zenflow");
    #[cfg(target_os = "windows")]
    let p = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
        .join("forgoodai/zenflow/data");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let p = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".local/share"))
        .join("zenflow");
    p
}
fn paths() -> Vec<PathBuf> {
    let r = root();
    let p = if r.is_file() { r } else { r.join("db.sqlite") };
    p.is_file().then_some(p).into_iter().collect()
}
fn patch(doc: &mut Value, p: &Value) -> Result<(), String> {
    let op = n::text(&p["op"]);
    let path = p["path"].as_str().ok_or("missing Zenflow patch path")?;
    if path.is_empty() && matches!(op, "add" | "replace") && p.get("value").is_some() {
        *doc = p["value"].clone();
        return Ok(());
    }
    let (parent, key) = path
        .rsplit_once('/')
        .ok_or("invalid Zenflow JSON pointer")?;
    let key = key.replace("~1", "/").replace("~0", "~");
    let target = doc
        .pointer_mut(parent)
        .ok_or("missing Zenflow patch parent")?;
    if !matches!(op, "add" | "replace" | "remove") || op != "remove" && p.get("value").is_none() {
        return Err("unsupported Zenflow patch".into());
    }
    match target {
        Value::Object(m) => {
            if op != "add" && !m.contains_key(&key) {
                return Err("missing Zenflow patch target".into());
            }
            if op == "remove" {
                m.remove(&key);
            } else {
                m.insert(key, p["value"].clone());
            }
        }
        Value::Array(a) => {
            let i = if key == "-" && op == "add" {
                a.len()
            } else {
                key.parse::<usize>().map_err(|e| e.to_string())?
            };
            if op == "add" && i <= a.len() {
                a.insert(i, p["value"].clone());
            } else if i < a.len() {
                if op == "remove" {
                    a.remove(i);
                } else {
                    a[i] = p["value"].clone();
                }
            } else {
                return Err("Zenflow patch index out of bounds".into());
            }
        }
        _ => return Err("invalid Zenflow patch target".into()),
    }
    Ok(())
}
fn replay(raw: &str) -> Result<Vec<Value>, String> {
    let mut doc = json!({"entries":[]});
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if row == "Reset" {
            doc = json!({"entries":[]});
        }
        if let Some(ops) = row.get("JsonPatch") {
            for op in ops.as_array().ok_or("invalid Zenflow patch list")? {
                patch(&mut doc, op)?;
            }
        }
    }
    doc["entries"]
        .as_array()
        .cloned()
        .ok_or_else(|| "invalid Zenflow entries".into())
}
fn added(change: &Value) -> String {
    match n::text(&change["action"]) {
        "write" => n::text(&change["content"]).into(),
        "edit" if change["has_line_numbers"] == false => {
            let mut hunk = false;
            let mut out = String::new();
            for l in n::text(&change["unified_diff"]).lines() {
                if l.starts_with("@@ ") {
                    hunk = true;
                } else if hunk {
                    if let Some(s) = l.strip_prefix('+') {
                        out.push_str(s);
                        out.push('\n');
                    }
                }
            }
            out
        }
        _ => String::new(),
    }
}
fn read(path: &Path, pm: &PathMap, native: &HashSet<String>) -> Result<FileGroup, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut q=db.prepare("select hex(e.id),l.logs,coalesce(t.container_ref,p.git_repo_path,''),e.started_at,e.completed_at,coalesce(s.session_id,''),e.dropped from execution_processes e join execution_process_normalized_logs l on l.execution_id=e.id join tasks t on t.id=e.task_id left join projects p on p.id=t.project_id left join executor_sessions s on s.execution_process_id=e.id where e.run_reason='codingagent' order by e.started_at,e.id").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, bool>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut runs = vec![];
    let mut owners = HashMap::new();
    for row in rows {
        let (id, raw, cwd, start, end, sid, dropped) = row.map_err(|e| e.to_string())?;
        let entries = replay(&raw)?;
        for entry in &entries {
            let cid = n::text(&entry["content"]["metadata"]["tool_call_id"]);
            if !sid.is_empty() && !cid.is_empty() {
                owners.insert((sid.clone(), cid.to_string()), id.clone());
            }
        }
        runs.push((id, entries, cwd, start, end, sid, dropped));
    }
    let mut g = FileGroup::default();
    for (id, entries, cwd, start, end, sid, dropped) in runs {
        let start = crate::any_ts(&json!(start)).unwrap_or(0);
        let end = crate::any_ts(&json!(end)).unwrap_or(start);
        let before = g.evs.len();
        if !dropped && !native.contains(&sid) {
            for entry in &entries {
                if entry["type"] != "NORMALIZED_ENTRY" {
                    continue;
                }
                let content = &entry["content"];
                let kind = &content["entry_type"];
                if kind["type"] != "tool_use"
                    || kind["status"]["status"] != "success"
                    || kind["action_type"]["action"] != "file_edit"
                {
                    continue;
                }
                let cid = n::text(&content["metadata"]["tool_call_id"]);
                if !sid.is_empty()
                    && !cid.is_empty()
                    && owners.get(&(sid.clone(), cid.into())) != Some(&id)
                {
                    continue;
                }
                let action = &kind["action_type"];
                let ts = crate::any_ts(&content["timestamp"]).unwrap_or(start);
                for change in action["changes"].as_array().into_iter().flatten() {
                    imp::push_scoped(
                        &mut g,
                        pm,
                        &cwd,
                        n::text(&action["path"]),
                        &added(change),
                        Src::Zenflow,
                        "",
                        &id,
                        ts,
                    );
                }
            }
        }
        let prompts = entries
            .iter()
            .filter(|e| e["content"]["entry_type"]["type"] == "user_message")
            .count() as i64;
        // Always reconcile a dropped/redundant execution, without inventing a second session.
        if dropped || native.contains(&sid) {
            g.reconciled_sessions.push((Src::Zenflow, id));
        } else {
            n::session(
                &mut g,
                pm,
                Src::Zenflow,
                &id,
                &cwd,
                start,
                end,
                prompts,
                before,
            );
        }
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let ids = store
        .groups
        .values()
        .flat_map(|g| g.sessions.iter())
        .filter(|s| {
            matches!(
                s.src,
                Src::Zencoder | Src::Codex | Src::Claude | Src::Gemini
            )
        })
        .map(|s| s.id.clone())
        .collect();
    n::scan(Src::Zenflow, store, pm, st, paths(), |p, pm| {
        read(p, pm, &ids)
    });
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Zenflow,&root(),"db.sqlite: execution_process_normalized_logs",paths().len(),"Replayed final normalized file-edit receipts with success status. Dropped executions, failed/pending edits, numbered diff renderings and sessions already read from their native executor are excluded. Model remains unknown; mutable profiles are not historical evidence.",false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_final_status_and_remove() {
        let raw = vec![
            json!({"JsonPatch":[{"op":"add","path":"/entries/-","value":{"status":"created"}}]}),
            json!({"JsonPatch":[{"op":"replace","path":"/entries/0/status","value":"failed"}]}),
        ]
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        assert_eq!(replay(&raw).unwrap()[0]["status"], "failed");
        assert!(replay(
            "{\"JsonPatch\":[{\"op\":\"replace\",\"path\":\"/entries/1\",\"value\":{}}]}"
        )
        .is_err());
    }
}
