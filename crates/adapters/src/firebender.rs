//! Firebender 1.0.12's project-local SQLite history (schema 1/2).
//! SQL, MessageAdapter and Write's completed receipt were recovered from the
//! official JetBrains package. See SUPPORT_AGENT8.md for the pinned evidence.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const REMINDER: &str = ".\n\nWhen you're done with your current set of changes to this file, you should call the read_lints tool with the specific file path and fix any newly introduced errors.";

fn paths(projects: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<_> = projects
        .iter()
        .map(|p| Path::new(p).join(".idea/firebender/chat_history.db"))
        .filter(|p| p.is_file())
        .collect();
    if let Some(root) = std::env::var_os("CREDITIA_FIREBENDER_DIR") {
        paths.extend(n::files(Path::new(&root), |p| {
            p.file_name().is_some_and(|f| f == "chat_history.db")
        }));
    }
    paths.sort();
    paths.dedup();
    paths
}
fn workspace(path: &Path) -> String {
    path.parent()
        .filter(|p| p.file_name().is_some_and(|v| v == "firebender"))
        .and_then(Path::parent)
        .filter(|p| p.file_name().is_some_and(|v| v == ".idea"))
        .and_then(Path::parent)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
struct Session {
    id: String,
    cwd: String,
    start: i64,
    end: i64,
    updated_ms: i64,
    deleted: bool,
    messages: Vec<Value>,
}
fn messages(json: &str) -> Result<Vec<Value>, String> {
    serde_json::from_str(json).map_err(|e| format!("invalid Firebender message array: {e}"))
}
fn read(path: &Path) -> Result<Vec<Session>, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    db.execute_batch("BEGIN DEFERRED")
        .map_err(|e| e.to_string())?;
    let version: String = db
        .query_row(
            "SELECT value FROM schema_info WHERE key='version'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !matches!(version.as_str(), "1" | "2") {
        return Err(format!("unsupported Firebender schema {version}"));
    }
    let mut q = db.prepare("SELECT id, created_at, updated_at, deleted_at, messages_json, metadata_json FROM chat_sessions ORDER BY updated_at, id").map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut sessions = vec![];
    for row in rows {
        let (id, start, end, deleted, body, meta) = row.map_err(|e| e.to_string())?;
        if id.is_empty() {
            return Err("Firebender session has no id".into());
        }
        let meta: Value =
            serde_json::from_str(&meta).map_err(|e| format!("invalid Firebender metadata: {e}"))?;
        let cwd = meta["worktreePath"]
            .as_str()
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| workspace(path));
        sessions.push(Session {
            id,
            cwd,
            start: start / 1000,
            end: end / 1000,
            updated_ms: end,
            deleted: deleted.is_some(),
            messages: messages(&body)?,
        });
    }
    let has_subagents: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='subagent_conversations')", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if has_subagents {
        let mut q = db.prepare("SELECT session_id, tool_call_id, created_at, updated_at, messages_json FROM subagent_conversations ORDER BY updated_at, tool_call_id").map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut children = vec![];
        for row in rows {
            let (parent, call, start, end, body) = row.map_err(|e| e.to_string())?;
            let Some(parent) = sessions.iter().find(|s| s.id == parent) else {
                continue;
            };
            if call.is_empty() {
                return Err("Firebender subagent has no call id".into());
            }
            children.push(Session {
                id: format!("{}:subagent:{call}", parent.id),
                cwd: parent.cwd.clone(),
                start: start / 1000,
                end: end / 1000,
                updated_ms: end,
                deleted: parent.deleted,
                messages: messages(&body)?,
            });
        }
        sessions.extend(children);
    }
    Ok(sessions)
}

// The completed Write receipt contains the actual absolute destination. A
// relative argument is resolved by the vendor's project/worktree, not this host.
fn write(call: &Value, result: &Value) -> Option<(String, String)> {
    if call["type"] != "function"
        || call["function"]["name"] != "Write"
        || result["role"] != "tool"
        || result["name"] != "write"
        || result["failed"] == true
    {
        return None;
    }
    let args: Value = serde_json::from_str(call["function"]["arguments"].as_str()?).ok()?;
    let content = args["content"].as_str()?;
    let input_path = args["file_path"].as_str()?;
    let path = result["content"]
        .as_str()?
        .strip_prefix("Wrote contents to ")?
        .strip_suffix(REMINDER)?;
    // Auto-shortening adds another reminder and changes the content. Until a
    // post-edit snapshot is available, that result must not reuse the input.
    if path.is_empty()
        || input_path.is_empty()
        || path.contains('\n')
        || path.contains('\r')
        || !(Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':'))
    {
        return None;
    }
    let normalized = |p: &str| p.replace('\\', "/");
    if (Path::new(input_path).is_absolute() || input_path.as_bytes().get(1) == Some(&b':'))
        && normalized(input_path) != normalized(path)
    {
        return None;
    }
    Some((path.into(), content.into()))
}

fn pairs(s: &Session) -> HashMap<String, (Value, Value)> {
    let mut calls = HashMap::new();
    let mut pairs = HashMap::new();
    for row in &s.messages {
        if row["role"] == "assistant" {
            for call in row["tool_calls"].as_array().into_iter().flatten() {
                if let Some(id) = call["id"].as_str().filter(|s| !s.is_empty()) {
                    calls.insert(id.to_string(), call.clone());
                    pairs.remove(id);
                }
            }
        } else if row["role"] == "tool" {
            let id = n::text(&row["tool_call_id"]);
            if let Some(call) = calls.get(id) {
                // A final failure replaces a preceding result with the same ID.
                pairs.insert(id.into(), (call.clone(), row.clone()));
            }
        }
    }
    pairs
}
fn group(
    sessions: &[Session],
    pm: &PathMap,
    owners: &HashMap<String, (i64, usize, usize)>,
    file: usize,
) -> FileGroup {
    let mut g = FileGroup::default();
    for (index, s) in sessions.iter().enumerate() {
        let before = g.evs.len();
        if !s.deleted {
            for (cid, (call, result)) in pairs(s) {
                if owners.get(&cid).is_none_or(|o| o.1 != file || o.2 != index) {
                    continue;
                }
                if let Some((path, text)) = write(&call, &result) {
                    imp::push_scoped(
                        &mut g,
                        pm,
                        &s.cwd,
                        &path,
                        &text,
                        Src::Firebender,
                        "",
                        &s.id,
                        s.end,
                    );
                }
            }
            n::session(
                &mut g,
                pm,
                Src::Firebender,
                &s.id,
                &s.cwd,
                s.start,
                s.end,
                s.messages.iter().filter(|r| r["role"] == "user").count() as i64,
                before,
            );
        } else {
            g.reconciled_sessions.push((Src::Firebender, s.id.clone()));
        }
    }
    g
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    let paths = paths(projects);
    let parsed: Vec<_> = paths.iter().map(|p| read(p)).collect();
    let mut owners = HashMap::new();
    for (file, sessions) in parsed.iter().enumerate() {
        if let Ok(sessions) = sessions {
            for (index, s) in sessions.iter().enumerate().filter(|(_, s)| !s.deleted) {
                for cid in pairs(s).into_keys() {
                    let rank = (s.updated_ms, file, index);
                    let best = owners.entry(cid).or_insert(rank);
                    if rank > *best {
                        *best = rank;
                    }
                }
            }
        }
    }
    n::scan(Src::Firebender, store, pm, st, paths.clone(), |p, pm| {
        let i = paths
            .iter()
            .position(|v| v == p)
            .ok_or("missing Firebender database")?;
        Ok(group(
            parsed[i].as_ref().map_err(Clone::clone)?,
            pm,
            &owners,
            i,
        ))
    });
}
pub fn discover() -> Vec<Activity> {
    paths(&[])
        .iter()
        .filter_map(|p| read(p).ok())
        .flatten()
        .filter(|s| !s.deleted && !s.cwd.is_empty())
        .map(|s| imp::activity(Src::Firebender, s.cwd, None, s.end))
        .collect()
}
pub fn doctor() -> Health {
    let root = std::env::var_os("CREDITIA_FIREBENDER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".idea/firebender/chat_history.db"));
    imp::health(Src::Firebender,&root,"project-local chat_history.db (schema 1/2)",paths(&[]).len(),"Scanned inside each selected project. Completed native Write receipts, including subagent conversations. Per-message model/time are not stored: model stays unknown; time is the session snapshot. Edit/API replacements, IDE auto-shortening, shell and pending calls are excluded.",false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_complete_native_write_receipts_supply_text() {
        let call = json!({"id":"c","type":"function","function":{"name":"Write","arguments":json!({"file_path":"src/main.rs","content":"fn saved() {}"}).to_string()}});
        let mut result = json!({"role":"tool","name":"write","tool_call_id":"c","content":format!("Wrote contents to /demo/src/main.rs{REMINDER}")});
        assert_eq!(write(&call, &result).unwrap().0, "/demo/src/main.rs");
        result["content"]=json!(format!("Wrote contents to /demo/src/main.rs\n\n<system_reminder>IDE auto shortened FQ class names.</system_reminder>{REMINDER}"));
        assert!(write(&call, &result).is_none());
        result["content"] = json!("Write proposed, awaiting approval");
        assert!(write(&call, &result).is_none());
    }
    #[test]
    fn terminal_failure_and_deleted_sessions_retract_evidence() {
        let call = json!({"id":"c","type":"function","function":{"name":"Write","arguments":json!({"file_path":"/demo/src/main.rs","content":"fn saved() {}"}).to_string()}});
        let result = json!({"role":"tool","name":"write","tool_call_id":"c","content":format!("Wrote contents to /demo/src/main.rs{REMINDER}")});
        let mut s = Session {
            id: "s".into(),
            cwd: "/demo".into(),
            start: 1,
            end: 2,
            updated_ms: 2000,
            deleted: false,
            messages: vec![json!({"role":"assistant","tool_calls":[call]}), result],
        };
        let owners = HashMap::from([("c".into(), (2, 0, 0))]);
        assert_eq!(
            group(std::slice::from_ref(&s), &n::project(), &owners, 0)
                .evs
                .len(),
            1
        );
        s.messages.push(json!({"role":"tool","name":"write","tool_call_id":"c","content":"cancelled","failed":true}));
        assert!(group(std::slice::from_ref(&s), &n::project(), &owners, 0)
            .evs
            .is_empty());
        s.messages.pop();
        s.deleted = true;
        let g = group(&[s], &n::project(), &owners, 0);
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions.len(), 1);
    }
}
