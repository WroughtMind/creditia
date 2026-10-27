//! Read-only TRAE desktop history, verified against official 3.6.2/build 2.3.91093 and 3.6.4/build 2.3.93439.
//! Native SQLCipher tables and original tool results are used; the UI's computed
//! status and code-contribution telemetry are deliberately not edit receipts.
#[path = "trae_codec.rs"]
mod codec;
use crate::{Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use rusqlite::{types::ValueRef, Connection};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

fn dirs() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("CREDITIA_TRAE_IDE_DIR") {
        return vec![PathBuf::from(p)];
    }
    #[cfg(target_os = "macos")]
    let base = crate::home().join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var("APPDATA").unwrap_or_default());
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".config"));
    ["Trae", "Trae CN"].iter().map(|n| base.join(n)).collect()
}
fn databases(dir: &Path) -> Vec<PathBuf> {
    if dir.extension().is_some_and(|s| s == "db") {
        return vec![dir.into()];
    }
    let default = dir.join("ModularData/ai-agent");
    if let Some(configured) = std::fs::read_to_string(default.join(".last-db-dir"))
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_absolute())
    {
        return vec![configured.join("database.db")];
    }
    let current = default.join("database.db");
    if current.exists() {
        vec![current]
    } else {
        vec![current, dir.join("ModularData/ai-chat/database.db")]
    }
}
fn paths() -> Vec<PathBuf> {
    let mut p: Vec<_> = dirs().iter().flat_map(|p| databases(p)).collect();
    p.sort();
    p.dedup();
    p
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn json(v: &Value) -> Result<Value, String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => {
            serde_json::from_str(s).map_err(|e| e.to_string())
        }
        Value::String(_) | Value::Null => Ok(Value::Null),
        _ => Ok(v.clone()),
    }
}
fn deleted(v: &Value) -> bool {
    let tombstone = match &v["deleted_at"] {
        Value::Null => false,
        Value::Number(n) => n.as_f64().is_some_and(|n| n > 0.0),
        Value::String(s) => {
            let text = s.trim();
            !text.is_empty()
                && text.parse::<i64>().map_or_else(
                    |_| crate::any_ts(&v["deleted_at"]).is_none_or(|n| n > 0),
                    |n| n > 0,
                )
        }
        _ => true,
    };
    tombstone
        || matches!(&v["is_archived"], Value::Bool(true))
        || v["is_archived"] == 1
        || v["is_archived"] == "1"
        || v["is_archived"] == "true"
}
fn ts(v: &Value) -> i64 {
    crate::any_ts(v).unwrap_or(0)
}
fn rows(db: &Connection, table: &str) -> Result<Vec<Value>, String> {
    // Table names come exclusively from sqlite_master or fixed native names.
    let mut q = db
        .prepare(&format!("SELECT * FROM \"{}\"", table.replace('"', "\"\"")))
        .map_err(|e| e.to_string())?;
    let names: Vec<_> = q.column_names().iter().map(|s| s.to_string()).collect();
    let mut result = Vec::new();
    let mut r = q.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = r.next().map_err(|e| e.to_string())? {
        let mut obj = Map::new();
        for (i, k) in names.iter().enumerate() {
            let v = match row.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(n) => Value::from(n),
                ValueRef::Text(b) | ValueRef::Blob(b) => Value::String(
                    String::from_utf8(b.to_vec())
                        .map_err(|_| format!("TRAE {table}.{k} is not UTF-8"))?,
                ),
            };
            obj.insert(k.clone(), v);
        }
        result.push(Value::Object(obj));
    }
    Ok(result)
}
fn columns(db: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut q = db
        .prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(|e| e.to_string())?;
    let result = q
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string());
    result
}
fn require(db: &Connection, table: &str, fields: &[&str]) -> Result<(), String> {
    let c = columns(db, table)?;
    if fields.iter().all(|f| c.contains(*f)) {
        Ok(())
    } else {
        Err(format!(
            "unsupported TRAE {table} schema: missing required native columns"
        ))
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Edit {
    call: String,
    path: String,
    text: String,
    model: String,
    time: i64,
}
#[derive(Default, Debug)]
struct Parsed {
    id: String,
    cwd: String,
    start: i64,
    end: i64,
    prompts: i64,
    calls: Vec<(String, Vec<Edit>)>,
    call_messages: HashMap<String, String>,
    withdrawn_messages: HashSet<String>,
    user_messages: HashSet<String>,
    active: bool,
}
fn absolute(cwd: &str, p: &str) -> Option<String> {
    let p = crate::file_uri_path(p).replace('\\', "/");
    let rooted = p.starts_with('/') || p.as_bytes().get(1) == Some(&b':');
    let full = if rooted {
        p
    } else if cwd.is_empty() {
        return None;
    } else {
        format!("{}/{p}", cwd.replace('\\', "/").trim_end_matches('/'))
    };
    let unc = full.starts_with("//");
    let drive = full.as_bytes().get(1) == Some(&b':');
    let floor = if unc {
        2
    } else if drive {
        1
    } else {
        0
    };
    let mut parts = Vec::new();
    for part in full.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() <= floor {
                    return None;
                }
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let prefix = if unc {
        "//"
    } else if full.starts_with('/') {
        "/"
    } else {
        ""
    };
    Some(format!("{prefix}{}", parts.join("/")))
}
fn task_edits(content: &Value, cwd: &str) -> Result<Vec<(String, Vec<Edit>)>, String> {
    let content = json(content)?;
    let messages = content
        .as_array()
        .or_else(|| content["messages"].as_array())
        .or_else(|| {
            content
                .get("task_id")
                .and_then(|_| content["content"].as_array())
        })
        .ok_or("unsupported TRAE task content: expected native messages")?;
    let mut out = Vec::new();
    for item in messages {
        let native_kind = s(item, "message_type");
        let wire_kind = s(item, "type");
        if !native_kind.is_empty() && !wire_kind.is_empty() && native_kind != wire_kind {
            return Err("conflicting TRAE task-message discriminants".into());
        }
        if !matches!(
            (native_kind, wire_kind),
            ("plan_item", _) | ("", "plan_item")
        ) {
            continue;
        }
        let plan = &item["plan_item"];
        let tool = &plan["tool_call_info"];
        let result = &tool["result"];
        let call = s(tool, "id");
        if call.is_empty() {
            continue;
        }
        let mut edits = Vec::new();
        // Original persisted status is authoritative. The frontend treats some
        // old running items as displayed successes; that transformation is unsafe.
        if result["status"] != "success"
            || result["interrupt"] == true
            || result["is_truncated"] == true
            || !s(result, "error_message").is_empty()
        {
            out.push((call.into(), edits));
            continue;
        }
        let name = s(tool, "name");
        if !matches!(
            name,
            "create_file"
                | "write_to_file"
                | "Write"
                | "Edit"
                | "MultiEdit"
                | "SearchReplace"
                | "edit_file"
                | "edit_file_search_replace"
                | "edit_file_update"
                | "apply_patch"
        ) {
            out.push((call.into(), edits));
            continue;
        }
        let Some(changes) = result["data"]["changes"].as_array() else {
            out.push((call.into(), edits));
            continue;
        };
        for change in changes {
            if !matches!(s(change, "file_action"), "added" | "modified") {
                continue;
            }
            // Only the actual result's retained before/after text is evidence.
            // A missing/null before requires the native added-file action.
            let Some(after) = change["new_content"].as_str() else {
                continue;
            };
            let before = match change["old_content"].as_str() {
                Some(before) => before,
                None if change["old_content"].is_null() && s(change, "file_action") == "added" => {
                    ""
                }
                None => continue,
            };
            let renamed = s(change, "rename_file_path");
            let path = if renamed.is_empty() {
                s(change, "file_path")
            } else {
                renamed
            };
            if path.is_empty() {
                continue;
            }
            let Some(path) = absolute(cwd, path) else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            let text = crate::added_lines(before, after);
            if text.is_empty() {
                continue;
            }
            let timing = &plan["timing"];
            let finished = timing["tool_call_finished_at_ms"].as_i64().unwrap_or(0);
            edits.push(Edit {
                call: call.into(),
                path,
                text,
                model: String::new(),
                time: if finished > 0 { finished / 1000 } else { 0 },
            });
        }
        out.push((call.into(), edits));
    }
    Ok(out)
}
fn parse_db(db: &Connection) -> Result<Vec<Parsed>, String> {
    require(
        db,
        "chat_session",
        &[
            "session_id",
            "project_id",
            "created_at",
            "updated_at",
            "deleted_at",
        ],
    )?;
    require(
        db,
        "chat_message",
        &[
            "session_id",
            "message_id",
            "message_type",
            "message_role",
            "message_index",
            "reply_to_message_id",
            "is_archived",
            "user_message_context",
            "created_at",
            "deleted_at",
        ],
    )?;
    require(
        db,
        "chat_message_task",
        &["message_id", "content", "deleted_at"],
    )?;
    let mut table_q = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .map_err(|e| e.to_string())?;
    let tables = table_q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut projects: HashMap<String, HashSet<String>> = HashMap::new();
    for table in tables {
        let c = columns(db, &table)?;
        if !c.contains("project_id") || !c.contains("absolute_path") {
            continue;
        }
        for row in rows(db, &table)? {
            if deleted(&row) {
                continue;
            }
            let id = s(&row, "project_id");
            let p = s(&row, "absolute_path");
            if !id.is_empty() {
                if let Some(path) = absolute("", p) {
                    projects.entry(id.into()).or_default().insert(path);
                }
            }
        }
    }
    let bodies: HashMap<_, _> = rows(db, "chat_message_task")?
        .into_iter()
        .map(|r| (s(&r, "message_id").to_string(), r))
        .collect();
    let messages = rows(db, "chat_message")?;
    let by_id: HashMap<_, _> = messages.iter().map(|r| (s(r, "message_id"), r)).collect();
    let mut out = Vec::new();
    for row in rows(db, "chat_session")? {
        let id = s(&row, "session_id");
        if id.is_empty() {
            return Err("TRAE session has no ID".into());
        }
        let project = projects.get(s(&row, "project_id"));
        let cwd = project
            .filter(|p| p.len() == 1)
            .and_then(|p| p.iter().next())
            .cloned()
            .unwrap_or_default();
        let mut p = Parsed {
            id: id.into(),
            cwd,
            start: ts(&row["created_at"]),
            end: ts(&row["updated_at"]),
            active: !deleted(&row),
            ..Default::default()
        };
        if !p.active {
            out.push(p);
            continue;
        }
        for m in messages.iter().filter(|m| s(m, "session_id") == id) {
            if s(m, "message_role") == "user" {
                let id = s(m, "message_id");
                if deleted(m) {
                    p.withdrawn_messages.insert(id.into());
                } else {
                    p.user_messages.insert(id.into());
                    p.prompts += 1;
                }
                continue;
            }
            if s(m, "message_role") != "assistant" || s(m, "message_type") != "task" {
                continue;
            }
            let message_id = s(m, "message_id");
            let reply = s(m, "reply_to_message_id");
            let body = bodies.get(message_id);
            if deleted(m)
                || (!reply.is_empty() && by_id.get(reply).is_some_and(|r| deleted(r)))
                || body.is_some_and(deleted)
            {
                p.withdrawn_messages.insert(message_id.into());
                continue;
            }
            let body = body.ok_or("TRAE task message body is missing")?;
            let calls = task_edits(&body["content"], &p.cwd)?;
            for (call, _) in &calls {
                p.call_messages.insert(call.clone(), message_id.into());
            }
            p.calls.extend(calls);
        }
        out.push(p);
    }
    Ok(out)
}
fn load(path: &Path) -> Result<Vec<Parsed>, String> {
    let mut key = codec::raw_key();
    let db = crate::sqlcipher_pages::open(path, &key);
    key.fill(0);
    parse_db(&db?)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    scan_paths(&paths(), store, pm, st);
}
const GROUP_KEY: &str = "trae-native:all";
const GROUP_PREFIX: &str = "trae-native:";
fn scan_paths(paths: &[PathBuf], store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut parsed = Vec::new();
    let mut observed = false;
    for path in paths.iter().filter(|p| p.is_file()) {
        match load(path) {
            Ok(rows) => {
                observed = true;
                parsed.extend(rows);
            }
            Err(e) => st
                .warnings
                .push(format!("trae-ide: {}: {e}", path.display())),
        }
    }
    // No readable snapshot means missing history, never a negative observation.
    if observed {
        replace_group(store, pm, parsed);
    } else {
        // Cached evidence remains historical evidence, but an unsuccessful
        // refresh cannot assert a newly observed authoritative session set.
        for (key, group) in &mut store.groups {
            if key.starts_with(GROUP_PREFIX) {
                group.reconciled_sessions.clear();
            }
        }
    }
}
fn replace_group(store: &mut Store, pm: &PathMap, parsed: Vec<Parsed>) {
    let known_ids = store
        .groups
        .iter()
        .filter(|(key, _)| key.starts_with(GROUP_PREFIX))
        .flat_map(|(_, g)| {
            g.sessions
                .iter()
                .filter(|s| s.src == Src::TraeIde)
                .map(|s| s.id.clone())
                .chain(
                    g.evs
                        .iter()
                        .filter(|e| e.src == Src::TraeIde)
                        .map(|e| e.session.clone()),
                )
                .chain(
                    g.reconciled_sessions
                        .iter()
                        .filter(|(src, _)| *src == Src::TraeIde)
                        .map(|(_, id)| id.clone()),
                )
        })
        .collect();
    let group = build_group(parsed, pm, &known_ids);
    store.groups.retain(|key, _| !key.starts_with(GROUP_PREFIX));
    store.groups.insert(GROUP_KEY.into(), group);
}
fn session_copies(parsed: &[Parsed]) -> BTreeMap<&str, Vec<&Parsed>> {
    let mut copies: BTreeMap<&str, Vec<&Parsed>> = BTreeMap::new();
    for p in parsed {
        copies.entry(&p.id).or_default().push(p);
    }
    copies
}
fn session_metadata(copies: &[&Parsed]) -> Option<Parsed> {
    let roots: HashSet<_> = copies
        .iter()
        .map(|p| p.cwd.as_str())
        .filter(|p| !p.is_empty())
        .collect();
    let starts: HashSet<_> = copies.iter().map(|p| p.start).filter(|&t| t > 0).collect();
    if copies.iter().any(|p| !p.active) || roots.len() > 1 || starts.len() > 1 {
        return None;
    }
    let withdrawn: HashSet<_> = copies
        .iter()
        .flat_map(|p| p.withdrawn_messages.iter())
        .collect();
    let prompts: HashSet<_> = copies
        .iter()
        .flat_map(|p| p.user_messages.iter())
        .filter(|id| !withdrawn.contains(id))
        .collect();
    Some(Parsed {
        id: copies[0].id.clone(),
        cwd: roots.into_iter().next().unwrap_or("").into(),
        start: starts.into_iter().next().unwrap_or(0),
        end: copies.iter().map(|p| p.end).max().unwrap_or(0),
        prompts: prompts.len() as i64,
        active: true,
        ..Default::default()
    })
}
fn build_group(parsed: Vec<Parsed>, pm: &PathMap, known_ids: &HashSet<String>) -> FileGroup {
    let mut g = FileGroup::default();
    let sessions = session_copies(&parsed);
    let blocked: HashSet<_> = sessions
        .iter()
        .filter(|(_, copies)| session_metadata(copies).is_none())
        .map(|(id, _)| *id)
        .collect();
    let withdrawn: HashSet<_> = parsed
        .iter()
        .flat_map(|p| {
            p.withdrawn_messages
                .iter()
                .map(|id| (p.id.as_str(), id.as_str()))
        })
        .collect();
    let mut calls: BTreeMap<&str, Vec<(&Parsed, &Vec<Edit>)>> = BTreeMap::new();
    for p in &parsed {
        for (id, edits) in &p.calls {
            calls.entry(id).or_default().push((p, edits));
        }
    }
    for (call_id, items) in &calls {
        let first = items[0].1;
        if items.iter().any(|(p, edits)| {
            blocked.contains(p.id.as_str())
                || *edits != first
                || p.call_messages
                    .get(*call_id)
                    .is_some_and(|message| withdrawn.contains(&(p.id.as_str(), message.as_str())))
        }) {
            continue;
        }
        // An empty vector is an explicit non-success observation and therefore
        // conflicts with any successful copy of the same original call ID.
        let (p, edits) = items.iter().min_by_key(|(p, _)| (p.start, &p.id)).unwrap();
        for e in *edits {
            crate::push_text(
                &mut g.evs,
                pm,
                &e.path,
                &e.text,
                Src::TraeIde,
                &e.model,
                &p.id,
                e.time,
            );
        }
    }
    for (id, copies) in sessions {
        if !copies.iter().any(|p| pm.in_project(&p.cwd))
            && !g.evs.iter().any(|e| e.session == id)
            && !known_ids.contains(id)
        {
            continue;
        }
        g.reconciled_sessions.push((Src::TraeIde, id.into()));
        if let Some(p) = session_metadata(&copies) {
            g.sessions.push(Session {
                src: Src::TraeIde,
                id: p.id,
                model: String::new(),
                start: p.start,
                end: p.end,
                tokens: 0,
                branch: None,
                prompts: p.prompts,
                reviewer: false,
                has_content: true,
                provider: None,
            });
        }
    }
    g
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    let parsed: Vec<_> = paths()
        .iter()
        .filter(|p| p.is_file())
        .filter_map(|p| load(p).ok())
        .flatten()
        .collect();
    let merged: Vec<_> = session_copies(&parsed)
        .into_values()
        .filter_map(|copies| session_metadata(&copies))
        .collect();
    for p in merged.into_iter().filter(|p| !p.cwd.is_empty()) {
        let a = out.entry(p.cwd.clone()).or_insert(Activity {
            src: Src::TraeIde,
            cwd: p.cwd,
            remote: None,
            sessions: 0,
            last: 0,
        });
        a.sessions += 1;
        a.last = a.last.max(p.end);
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    let paths = paths();
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    let mut readable = false;
    for path in paths.iter().filter(|p| p.is_file()) {
        match load(path) {
            Ok(p) => {
                readable = true;
                parsed.extend(p);
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    let parsed: Vec<_> = session_copies(&parsed)
        .into_values()
        .filter_map(|copies| session_metadata(&copies))
        .collect();
    let mut note=creditia_core::tr!("只读解密 TRAE 原生历史；只认 task 中原始 success 工具回执及其实际前后文本，排除归档、删除、失败、未完成与预览。逐工具执行模型未核实，留空；缺少逐工具执行时间时留空。尚无真实账号验收。","Read-only TRAE native history; original successful task-tool receipts with actual before/after text only. Archived/deleted/failed/pending/preview records excluded. Per-tool execution models remain unknown; missing per-tool execution times remain unknown. No real-account acceptance yet.");
    if !errors.is_empty() {
        note.push_str(&format!(" {}", errors.join("; ")));
    }
    Health {
        source: "trae-ide".into(),
        path: paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("; "),
        readable,
        format: "TRAE SQLCipher 4: chat_session + chat_message + chat_message_task".into(),
        records: parsed.iter().filter(|p| p.active).count() as u64,
        earliest: parsed
            .iter()
            .map(|p| p.start)
            .filter(|&t| t > 0)
            .min()
            .map(date_local),
        latest: parsed
            .iter()
            .map(|p| p.end)
            .filter(|&t| t > 0)
            .max()
            .map(date_local),
        note,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn plan(id: &str, status: &str, name: &str, changes: Value) -> Value {
        json!({"type":"plan_item","plan_item":{"id":format!("p-{id}"),"timing":{"tool_call_finished_at_ms":1720000005000i64},"tool_call_info":{"id":id,"name":name,"params":{"file_path":"a.rs","content":"requested text must never count"},"result":{"status":status,"data":{"changes":changes}}}}})
    }
    fn change(path: &str, before: Value, after: &str, action: &str) -> Value {
        json!({"file_path":path,"old_content":before,"new_content":after,"file_action":action})
    }
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into(), "b.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    fn database(content: &Value) -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE local_project(project_id TEXT,absolute_path TEXT,deleted_at INTEGER); INSERT INTO local_project VALUES('p','/repo',0); CREATE TABLE chat_session(session_id TEXT,project_id TEXT,created_at INTEGER,updated_at INTEGER,deleted_at INTEGER); INSERT INTO chat_session VALUES('s','p',1720000000,1720000010,0); CREATE TABLE chat_message(session_id TEXT,message_id TEXT,message_type TEXT,message_role TEXT,message_index INTEGER,reply_to_message_id TEXT,is_archived INTEGER,user_message_context TEXT,created_at INTEGER,deleted_at INTEGER); INSERT INTO chat_message VALUES('s','u','general','user',0,'',0,'{}',1720000000,0); INSERT INTO chat_message VALUES('s','a','task','assistant',1,'u',0,'{}',1720000001,0); CREATE TABLE chat_message_task(message_id TEXT,content TEXT,deleted_at INTEGER); CREATE TABLE chat_turn(response_message_id TEXT,context TEXT,deleted_at INTEGER); INSERT INTO chat_turn VALUES('a','{\"model_smart_selection_meta\":{\"config_name\":\"recorded-model\"}}',0);").unwrap();
        db.execute(
            "INSERT INTO chat_message_task VALUES('a',?,0)",
            [content.to_string()],
        )
        .unwrap();
        db
    }
    #[test]
    fn original_receipts_only_and_actual_added_lines() {
        let c = change(
            "a.rs",
            json!("unchanged line\nold line"),
            "unchanged line\nnew actual line",
            "modified",
        );
        let content = json!({"messages":[plan("good","success","Edit",json!([c])),plan("failed","failed","Write",json!([change("a.rs",json!(""),"failed line","added")])),plan("running","running","Write",json!([change("a.rs",json!(""),"pending line","added")])),plan("preview","success","show_diff",json!([change("a.rs",json!(""),"preview line","added")])),plan("overwrite","success","Write",json!([change("a.rs",Value::Null,"unknown previous line","modified")])),plan("create","success","create_file",json!([change("b.rs",Value::Null,"new created line","added")]))]});
        let mut native_content = content.clone();
        native_content["messages"][0]["message_type"] = json!("plan_item");
        native_content["messages"][0]
            .as_object_mut()
            .unwrap()
            .remove("type");
        let db = database(&native_content);
        let parsed = parse_db(&db).unwrap();
        let g = build_group(parsed, &pm(), &HashSet::new());
        assert_eq!(g.evs.len(), 2);
        assert!(g.evs.iter().all(|e| e.model.is_empty()));
        assert!(g.evs.iter().all(|e| e.ts == 1720000005));
        assert_eq!(g.sessions[0].prompts, 1);
        assert_eq!(g.reconciled_sessions, vec![(Src::TraeIde, "s".into())]);
    }
    #[test]
    fn native_task_variant_and_rename_use_saved_destination() {
        let mut c = change(
            "/other/old.rs",
            json!("old line"),
            "actual renamed line",
            "modified",
        );
        c["rename_file_path"] = json!("/repo/b.rs");
        let content = json!({"task_id":"t","content":[plan("rename","success","edit_file",json!([c]))],"summary":""});
        let db = database(&content);
        let g = build_group(parse_db(&db).unwrap(), &pm(), &HashSet::new());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "b.rs");
    }
    #[test]
    fn historical_path_resolution_preserves_project_boundary() {
        assert_eq!(
            absolute("/repo", "src/../a.rs").as_deref(),
            Some("/repo/a.rs")
        );
        assert_eq!(
            absolute("C:\\repo", "a.rs").as_deref(),
            Some("C:/repo/a.rs")
        );
        let outside = absolute("/repo", "../other/a.rs").unwrap();
        assert!(!pm().in_project(&outside));
        assert!(absolute("", "a.rs").is_none());
    }
    #[test]
    fn missing_execution_time_and_selection_model_stay_unknown() {
        let mut item = plan(
            "untimed",
            "success",
            "Write",
            json!([change(
                "a.rs",
                json!(""),
                "actual unknown timing line",
                "added"
            )]),
        );
        item["plan_item"]["timing"] = Value::Null;
        let db = database(&json!([item]));
        let g = build_group(parse_db(&db).unwrap(), &pm(), &HashSet::new());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].ts, 0);
        assert!(g.evs[0].model.is_empty());
        db.execute(
            "UPDATE chat_message SET deleted_at='2026-10-19 12:30:00' WHERE message_id='a'",
            [],
        )
        .unwrap();
        assert!(build_group(parse_db(&db).unwrap(), &pm(), &HashSet::new())
            .evs
            .is_empty());
    }
    #[test]
    fn archive_delete_and_missing_content_do_not_keep_stale_success() {
        let content = json!([plan(
            "good",
            "success",
            "Write",
            json!([change("a.rs", json!(""), "actual written line", "added")])
        )]);
        let db = database(&content);
        assert_eq!(parse_db(&db).unwrap()[0].calls[0].1.len(), 1);
        db.execute(
            "UPDATE chat_message SET is_archived=1 WHERE message_id='u'",
            [],
        )
        .unwrap();
        let g = build_group(parse_db(&db).unwrap(), &pm(), &HashSet::new());
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions.len(), 1);
        db.execute("UPDATE chat_session SET deleted_at=1720000011", [])
            .unwrap();
        db.execute("UPDATE local_project SET deleted_at=1720000011", [])
            .unwrap();
        let g = build_group(
            parse_db(&db).unwrap(),
            &pm(),
            &["s".into()].into_iter().collect(),
        );
        assert!(g.sessions.is_empty());
        assert_eq!(g.reconciled_sessions.len(), 1);
        db.execute("UPDATE chat_session SET deleted_at=0", [])
            .unwrap();
        db.execute("UPDATE chat_message SET is_archived=0", [])
            .unwrap();
        db.execute("DELETE FROM chat_message_task", []).unwrap();
        assert!(parse_db(&db).unwrap_err().contains("body is missing"));
    }
    #[test]
    fn repeated_multifile_calls_count_once_and_conflicts_retract() {
        let content = json!({"messages":[plan("same","success","Write",json!([change("a.rs",json!(""),"first actual line","added"),change("b.rs",json!(""),"second actual line","added")]))]});
        let db = database(&content);
        let mut parsed = parse_db(&db).unwrap();
        let mut fork = parse_db(&db).unwrap().remove(0);
        fork.id = "fork".into();
        fork.start += 10;
        parsed.push(fork);
        let g = build_group(parsed, &pm(), &HashSet::new());
        assert_eq!(g.evs.len(), 2);
        assert!(g.evs.iter().all(|e| e.session == "s"));
        let mut parsed = parse_db(&db).unwrap();
        parsed[0].calls.push(("same".into(), vec![]));
        let g = build_group(parsed, &pm(), &HashSet::new());
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions.len(), 1);
    }
    #[test]
    fn multiple_databases_share_one_call_and_session_namespace() {
        let content = json!({"task_id":"t","user_message_id":"u","messages":[plan("same","success","Write",json!([change("a.rs",json!(""),"actual shared line","added")]))]});
        let success = database(&content);
        let duplicate = database(&content);
        let mut store = Store::default();
        store.groups.insert(
            "trae-native:/old/a.db".into(),
            build_group(parse_db(&success).unwrap(), &pm(), &HashSet::new()),
        );
        store
            .groups
            .insert("trae-native:/old/b.db".into(), FileGroup::default());
        store
            .groups
            .insert("unrelated".into(), FileGroup::default());
        let observations = parse_db(&success)
            .unwrap()
            .into_iter()
            .chain(parse_db(&duplicate).unwrap())
            .collect();
        replace_group(&mut store, &pm(), observations);
        assert_eq!(store.groups.len(), 2);
        assert!(!store.groups.contains_key("trae-native:/old/a.db"));
        assert!(store.groups.contains_key("unrelated"));
        let group = &store.groups[GROUP_KEY];
        assert_eq!(group.evs.len(), 1);
        assert_eq!(group.sessions.len(), 1);
        assert_eq!(group.sessions[0].prompts, 1);
        assert_eq!(group.reconciled_sessions, vec![(Src::TraeIde, "s".into())]);
        let mut failure = content;
        failure["messages"][0]["plan_item"]["tool_call_info"]["result"]["status"] = json!("failed");
        duplicate
            .execute(
                "UPDATE chat_message_task SET content=?",
                [failure.to_string()],
            )
            .unwrap();
        replace_group(
            &mut store,
            &pm(),
            parse_db(&success)
                .unwrap()
                .into_iter()
                .chain(parse_db(&duplicate).unwrap())
                .collect(),
        );
        assert!(store.groups[GROUP_KEY].evs.is_empty());
        assert_eq!(store.groups[GROUP_KEY].sessions.len(), 1);
        assert_eq!(store.groups[GROUP_KEY].reconciled_sessions.len(), 1);
    }
    #[test]
    fn deleted_or_conflicting_session_copy_vetoes_other_database() {
        let content = json!([plan(
            "same",
            "success",
            "Write",
            json!([change("a.rs", json!(""), "actual shared line", "added")])
        )]);
        for update in [
            "UPDATE chat_session SET deleted_at=1720000011",
            "UPDATE local_project SET absolute_path='/other'",
            "UPDATE chat_session SET created_at=1720000002",
        ] {
            let success = database(&content);
            let other = database(&content);
            other.execute(update, []).unwrap();
            let observations = parse_db(&success)
                .unwrap()
                .into_iter()
                .chain(parse_db(&other).unwrap())
                .collect();
            let group = build_group(observations, &pm(), &HashSet::new());
            assert!(group.evs.is_empty(), "{update}");
            assert!(group.sessions.is_empty(), "{update}");
            assert_eq!(group.reconciled_sessions, vec![(Src::TraeIde, "s".into())]);
        }
    }
    #[test]
    fn archived_or_deleted_message_copy_vetoes_other_database() {
        let content = json!([plan(
            "same",
            "success",
            "Write",
            json!([change("a.rs", json!(""), "actual shared line", "added")])
        )]);
        for update in [
            "UPDATE chat_message SET is_archived=1 WHERE message_id='u'",
            "UPDATE chat_message SET deleted_at=1720000011 WHERE message_id='a'",
            "UPDATE chat_message_task SET deleted_at=1720000011",
        ] {
            let success = database(&content);
            let other = database(&content);
            other.execute(update, []).unwrap();
            let observations = parse_db(&success)
                .unwrap()
                .into_iter()
                .chain(parse_db(&other).unwrap())
                .collect();
            let group = build_group(observations, &pm(), &HashSet::new());
            assert!(group.evs.is_empty(), "{update}");
            assert_eq!(group.sessions.len(), 1);
            if update.contains("message_id='u'") {
                assert_eq!(group.sessions[0].prompts, 0);
            }
            assert_eq!(group.reconciled_sessions.len(), 1);
        }
    }
    #[test]
    fn missing_and_unreadable_databases_do_not_assert_retraction() {
        let content = json!([plan(
            "same",
            "success",
            "Write",
            json!([change("a.rs", json!(""), "actual shared line", "added")])
        )]);
        let db = database(&content);
        let mut store = Store::default();
        replace_group(&mut store, &pm(), parse_db(&db).unwrap());
        let path = std::env::temp_dir().join(format!(
            "creditia-trae-missing-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut stats = Stats::default();
        scan_paths(std::slice::from_ref(&path), &mut store, &pm(), &mut stats);
        assert_eq!(store.groups[GROUP_KEY].evs.len(), 1);
        assert!(store.groups[GROUP_KEY].reconciled_sessions.is_empty());
        assert!(stats.warnings.is_empty());
        std::fs::write(&path, b"not a TRAE database").unwrap();
        scan_paths(std::slice::from_ref(&path), &mut store, &pm(), &mut stats);
        assert_eq!(store.groups[GROUP_KEY].evs.len(), 1);
        assert_eq!(stats.warnings.len(), 1);
        std::fs::remove_file(path).unwrap();
    }
}
