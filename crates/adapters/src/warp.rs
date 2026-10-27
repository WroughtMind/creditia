//! Warp native agent_tasks Task protobufs and official ConversationData exports.
//! Contract: warpdotdev/warp f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0
//! app/src/persistence/agent.rs, crates/persistence/src/model.rs, warp_core/src/paths.rs;
//! GraphQL finalTaskList decoding: app/src/server/server_api/ai.rs and
//! crates/graphql/src/api/{ai.rs,queries/list_ai_conversations.rs} at the same client commit.
//! warpdotdev/warp-proto-apis 30d54b5 apis/multi_agent/v1/{task,conversation_data,file_content,input_context}.proto.
//! No generated vendor implementation is copied; the small decoder below reads only published wire fields.
use crate::trace_import as imp;
use crate::{any_ts, open_ro, Activity, Health, Stats};
use base64::Engine as _;
use creditia_core::model::{fingerprint, FileGroup, PathMap, Src, Store};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
enum Wire<'a> {
    Int(u64),
    Bytes(&'a [u8]),
    Other,
}
struct Fields<'a>(Vec<(u32, Wire<'a>)>);
fn varint(b: &[u8], at: &mut usize) -> Result<u64, String> {
    let mut n = 0;
    for shift in (0..70).step_by(7) {
        let x = *b.get(*at).ok_or("truncated protobuf varint")?;
        *at += 1;
        if shift == 63 && x > 1 {
            return Err("overflowed protobuf varint".into());
        }
        n |= ((x & 127) as u64) << shift;
        if x < 128 {
            return Ok(n);
        }
    }
    Err("overflowed protobuf varint".into())
}
fn wire(b: &[u8]) -> Result<Fields<'_>, String> {
    let mut out = vec![];
    let mut at = 0;
    while at < b.len() {
        let tag = varint(b, &mut at)?;
        let number = u32::try_from(tag >> 3).map_err(|_| "protobuf field number overflow")?;
        if number == 0 {
            return Err("zero protobuf field number".into());
        }
        let value = match tag & 7 {
            0 => Wire::Int(varint(b, &mut at)?),
            2 => {
                let len =
                    usize::try_from(varint(b, &mut at)?).map_err(|_| "protobuf length overflow")?;
                let end = at
                    .checked_add(len)
                    .filter(|e| *e <= b.len())
                    .ok_or("truncated protobuf bytes")?;
                let s = &b[at..end];
                at = end;
                Wire::Bytes(s)
            }
            1 | 5 => {
                at = at
                    .checked_add(if tag & 7 == 1 { 8 } else { 4 })
                    .filter(|i| *i <= b.len())
                    .ok_or("truncated protobuf fixed field")?;
                Wire::Other
            }
            _ => return Err("unsupported protobuf wire type".into()),
        };
        out.push((number, value));
    }
    Ok(Fields(out))
}
impl<'a> Fields<'a> {
    fn bytes(&self, n: u32) -> Option<&'a [u8]> {
        self.0.iter().rev().find_map(|(k, v)| {
            if *k == n {
                if let Wire::Bytes(b) = v {
                    Some(*b)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
    fn repeated(&self, n: u32) -> Vec<&'a [u8]> {
        self.0
            .iter()
            .filter_map(|(k, v)| {
                if *k == n {
                    if let Wire::Bytes(b) = v {
                        Some(*b)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
    fn number(&self, n: u32) -> u64 {
        self.0
            .iter()
            .rev()
            .find_map(|(k, v)| {
                if *k == n {
                    if let Wire::Int(i) = v {
                        Some(*i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }
    fn text(&self, n: u32) -> String {
        self.bytes(n)
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
            .into()
    }
}
fn proto_context(b: &[u8]) -> Result<Value, String> {
    let f = wire(b)?;
    let directory = f.bytes(1).map(wire).transpose()?;
    Ok(json!({"directory":{"pwd":directory.map(|d|d.text(1)).unwrap_or_default()}}))
}
fn proto_file(b: &[u8]) -> Result<Value, String> {
    let f = wire(b)?;
    Ok(json!({"file_path":f.text(1),"content":f.text(2)}))
}
fn proto_edit(b: &[u8]) -> Result<Value, String> {
    let f = wire(b)?;
    let mut diffs = vec![];
    for b in f.repeated(2) {
        let d = wire(b)?;
        diffs.push(json!({"file_path":d.text(1),"search":d.text(2),"replace":d.text(3)}));
    }
    let mut new_files = vec![];
    for b in f.repeated(3) {
        let n = wire(b)?;
        new_files.push(
            json!({"file_path":n.text(1),"content":n.text(2),"allow_overwrite":n.number(3)!=0}),
        );
    }
    let mut v4a = vec![];
    for b in f.repeated(5) {
        let u = wire(b)?;
        let mut hunks = vec![];
        for h in u.repeated(3) {
            let h = wire(h)?;
            hunks.push(json!({"old":h.text(3),"new":h.text(4)}));
        }
        v4a.push(json!({"file_path":u.text(1),"move_to":u.text(2),"hunks":hunks}));
    }
    Ok(json!({"diffs":diffs,"new_files":new_files,"v4a_updates":v4a}))
}
fn proto_result(b: &[u8]) -> Result<Value, String> {
    let f = wire(b)?;
    if f.bytes(2).is_some() {
        return Ok(json!({"error":{}}));
    }
    let Some(success) = f.bytes(1) else {
        return Ok(Value::Null);
    };
    let s = wire(success)?;
    let mut legacy = vec![];
    for f in s.repeated(1) {
        legacy.push(proto_file(f)?);
    }
    let mut updated = vec![];
    for b in s.repeated(2) {
        let u = wire(b)?;
        if let Some(f) = u.bytes(1) {
            updated.push(json!({"file":proto_file(f)?,"was_edited_by_user":u.number(2)!=0}));
        }
    }
    Ok(json!({"success":{"updated_files":legacy,"updated_files_v2":updated}}))
}
fn proto_message(b: &[u8]) -> Result<Value, String> {
    let f = wire(b)?;
    let mut m = json!({"id":f.text(1),"task_id":f.text(11),"request_id":f.text(13)});
    if let Some(t) = f.bytes(14) {
        m["timestamp"] = json!(wire(t)?.number(1));
    }
    if let Some(model) = f.bytes(25) {
        m["model_used"] = json!({"model_id":wire(model)?.text(1)});
    }
    for (n, key) in [(2, "user_query"), (9, "system_query")] {
        if let Some(q) = f.bytes(n) {
            let q = wire(q)?;
            if let Some(c) = q.bytes(2) {
                m[key] = json!({"context":proto_context(c)?});
            } else {
                m[key] = json!({});
            }
        }
    }
    if let Some(call) = f.bytes(4) {
        let c = wire(call)?;
        if let Some(edit) = c.bytes(6) {
            m["tool_call"] = json!({"tool_call_id":c.text(1),"apply_file_diffs":proto_edit(edit)?});
        }
    }
    if let Some(result) = f.bytes(5) {
        let r = wire(result)?;
        let mut v = json!({"tool_call_id":r.text(1)});
        if let Some(e) = r.bytes(6) {
            v["apply_file_diffs"] = proto_result(e)?;
        }
        if let Some(c) = r.bytes(11) {
            v["context"] = proto_context(c)?;
        }
        m["tool_call_result"] = v;
    }
    Ok(m)
}
fn proto_task(b: &[u8]) -> Result<Value, String> {
    let t = wire(b)?;
    let id = t.text(1);
    if id.is_empty() {
        return Err("Task protobuf lacks id".into());
    }
    let mut messages = vec![];
    for m in t.repeated(5) {
        messages.push(proto_message(m)?);
    }
    Ok(json!({"id":id,"messages":messages}))
}
fn proto_conversation(b: &[u8]) -> Result<Vec<Value>, String> {
    wire(b)?.repeated(1).into_iter().map(proto_task).collect()
}

/// Both spellings are canonical protobuf JSON field names: source snake_case and JSON lowerCamelCase.
fn field<'a>(v: &'a Value, name: &str) -> &'a Value {
    if let Some(v) = v.get(name) {
        return v;
    }
    let mut camel = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            camel.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            camel.push(c);
        }
    }
    &v[camel]
}
fn list<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    field(v, name).as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn string<'a>(v: &'a Value, name: &str) -> &'a str {
    field(v, name).as_str().unwrap_or("")
}
fn timestamp(v: &Value) -> i64 {
    if v.is_object() {
        any_ts(&v["seconds"]).unwrap_or(0)
    } else {
        any_ts(v).unwrap_or(0)
    }
}
fn cwd_of(c: &Value) -> &str {
    string(&c["directory"], "pwd")
}
fn absolute(cwd: &str, p: &str) -> String {
    let raw = if Path::new(p).is_absolute() || p.as_bytes().get(1) == Some(&b':') {
        p.replace('\\', "/")
    } else if !cwd.is_empty() {
        format!("{}/{}", cwd.trim_end_matches('/'), p).replace('\\', "/")
    } else {
        p.into()
    };
    let mut parts = vec![];
    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p: &&str| !p.ends_with(':')) => {
                parts.pop();
            }
            ".." => {}
            _ => parts.push(part),
        }
    }
    format!(
        "{}{}",
        if raw.starts_with('/') { "/" } else { "" },
        parts.join("/")
    )
}

fn parse(id: &str, tasks: &[Value], meta: &Value, pm: &PathMap, g: &mut FileGroup) {
    let initial_events = g.evs.len();
    let reverted: HashSet<&str> = meta["reverted_action_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut start = 0;
    let mut end = 0;
    let mut prompts = 0;
    let initial_cwd = string(meta, "working_directory");
    let mut relevant = !initial_cwd.is_empty() && pm.in_project(initial_cwd);
    let mut used_models = HashMap::<String, usize>::new();
    let mut observed_models = HashMap::<String, usize>::new();
    for task in tasks {
        let messages = list(task, "messages");
        let mut models: HashMap<&str, HashSet<&str>> = HashMap::new();
        for m in messages {
            let request = string(m, "request_id");
            let model = string(field(m, "model_used"), "model_id");
            if !request.is_empty() && !model.is_empty() {
                models.entry(request).or_default().insert(model);
            }
        }
        for request_models in models.values().filter(|s| s.len() == 1) {
            if let Some(model) = request_models.iter().next() {
                *observed_models.entry((*model).into()).or_default() += 1;
            }
        }
        let mut pending: HashMap<&str, (&Value, String, String, &str)> = HashMap::new();
        let mut current_cwd = initial_cwd.to_string();
        for m in messages {
            let ts = timestamp(&m["timestamp"]);
            if ts > 0 {
                start = if start == 0 { ts } else { start.min(ts) };
                end = end.max(ts);
            }
            for kind in ["user_query", "system_query"] {
                let q = field(m, kind);
                if !q.is_null() {
                    let p = cwd_of(field(q, "context"));
                    if !p.is_empty() {
                        current_cwd = p.into();
                        relevant |= pm.in_project(p);
                    }
                    if kind == "user_query" {
                        prompts += 1;
                    }
                }
            }
            let call = field(m, "tool_call");
            let cid = string(call, "tool_call_id");
            let edit = field(call, "apply_file_diffs");
            if !cid.is_empty() && !edit.is_null() {
                let request = string(m, "request_id");
                let model = models
                    .get(request)
                    .filter(|s| s.len() == 1)
                    .and_then(|s| s.iter().next())
                    .copied()
                    .unwrap_or("");
                pending.insert(
                    cid,
                    (edit, model.into(), current_cwd.clone(), string(m, "id")),
                );
            }
            let result = field(m, "tool_call_result");
            let cid = string(result, "tool_call_id");
            if cid.is_empty() {
                continue;
            }
            let Some((requested, model, call_cwd, message_id)) = pending.remove(cid) else {
                continue;
            };
            if reverted.contains(cid) || reverted.contains(message_id) {
                continue;
            }
            let r = field(result, "apply_file_diffs");
            let success = field(r, "success");
            if !success.is_object() || !field(r, "error").is_null() {
                continue;
            }
            let result_cwd = cwd_of(field(result, "context"));
            let cwd = if result_cwd.is_empty() {
                call_cwd.as_str()
            } else {
                result_cwd
            };
            let mut actual = HashMap::<String, &str>::new();
            let mut user_edited = HashSet::new();
            for u in list(success, "updated_files_v2") {
                let f = field(u, "file");
                let p = absolute(cwd, string(f, "file_path"));
                if field(u, "was_edited_by_user") == true {
                    user_edited.insert(p);
                } else if let Some(text) = field(f, "content").as_str() {
                    actual.insert(p, text);
                }
            }
            for f in list(success, "updated_files") {
                let p = absolute(cwd, string(f, "file_path"));
                if let Some(text) = field(f, "content").as_str() {
                    actual.entry(p).or_insert(text);
                }
            }
            let before = g.evs.len();
            let mut write = |path: &str, old: &str, new: &str, full: bool| {
                let p = absolute(cwd, path);
                if user_edited.contains(&p) {
                    return;
                }
                let Some(actual) = actual.get(&p) else {
                    return;
                };
                if new.is_empty()
                    || if full {
                        **actual != *new
                    } else {
                        !actual.contains(new)
                    }
                {
                    return;
                }
                let text = crate::added_lines(old, new);
                imp::push_scoped(g, pm, "", &p, &text, Src::Warp, &model, id, ts);
            };
            for d in list(requested, "diffs") {
                if let (Some(old), Some(new)) =
                    (field(d, "search").as_str(), field(d, "replace").as_str())
                {
                    write(string(d, "file_path"), old, new, false);
                }
            }
            for n in list(requested, "new_files") {
                // An overwrite has no before-image here; its unchanged human
                // context cannot safely become newly generated line evidence.
                if field(n, "allow_overwrite") == true {
                    continue;
                }
                if let Some(text) = field(n, "content").as_str() {
                    write(string(n, "file_path"), "", text, true);
                }
            }
            for u in list(requested, "v4a_updates") {
                let moved = string(u, "move_to");
                let p = if moved.is_empty() {
                    string(u, "file_path")
                } else {
                    moved
                };
                for h in list(u, "hunks") {
                    if let Some(new) = field(h, "new").as_str() {
                        let old = field(h, "old").as_str().unwrap_or("");
                        write(p, old, new, false);
                    }
                }
            }
            if g.evs.len() > before {
                relevant = true;
                *used_models.entry(model).or_default() += g.evs.len() - before;
            }
        }
    }
    if relevant {
        if used_models.is_empty() {
            used_models = observed_models;
        }
        let model = used_models
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(m, _)| m)
            .unwrap_or_default();
        g.sessions.push(imp::session(
            Src::Warp,
            id,
            &model,
            start,
            end,
            prompts,
            g.evs.len() > initial_events,
        ));
    }
}

fn app_roots() -> Vec<PathBuf> {
    if let Some(p) = std::env::var("CREDITIA_WARP_DIR")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return vec![p.into()];
    }
    let mut roots = vec![];
    #[cfg(target_os = "macos")]
    {
        let app = crate::home().join("Library/Application Support");
        roots.extend(
            std::fs::read_dir(&app)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("dev.warp."))
                }),
        );
        for group in std::fs::read_dir(crate::home().join("Library/Group Containers"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".dev.warp"))
            })
        {
            roots.extend(
                std::fs::read_dir(group.join("Library/Application Support"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("dev.warp."))
                    }),
            );
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(local).join("warp");
            roots.extend(
                std::fs::read_dir(base)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path().join("data")),
            );
        }
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let base = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| crate::home().join(".local/state"));
        roots.extend(
            std::fs::read_dir(base)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with("warp-terminal")
                            || n.to_string_lossy().starts_with("warp-oss")
                    })
                }),
        );
    }
    roots
}
fn input_files() -> Vec<PathBuf> {
    let mut files = vec![];
    for root in app_roots() {
        if root.is_file() {
            files.push(root);
            continue;
        }
        for e in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == "warp.sqlite")
                || matches!(p.extension().and_then(|s| s.to_str()), Some("pb" | "json"))
            {
                files.push(p.into());
            }
        }
    }
    // Native DB snapshots take precedence when a downloaded cloud conversation
    // or copied export describes that same session.
    files.sort_by_key(|p| (p.file_name().is_none_or(|n| n != "warp.sqlite"), p.clone()));
    files.dedup();
    files
}
fn database(path: &Path) -> Result<Vec<(String, Value, Vec<Value>)>, String> {
    let c = open_ro(path).map_err(|e| e.to_string())?;
    let mut q = c
        .prepare("SELECT conversation_id,conversation_data FROM agent_conversations")
        .map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for r in rows {
        let (id, meta) = r.map_err(|e| e.to_string())?;
        let meta: Value = serde_json::from_str(&meta).map_err(|e| e.to_string())?;
        let mut q = c
            .prepare("SELECT task FROM agent_tasks WHERE conversation_id=?1 ORDER BY id")
            .map_err(|e| e.to_string())?;
        let bytes = q
            .query_map([&id], |r| r.get::<_, Vec<u8>>(0))
            .map_err(|e| e.to_string())?;
        let mut tasks = vec![];
        for b in bytes {
            tasks.push(proto_task(&b.map_err(|e| e.to_string())?)?);
        }
        out.push((id, meta, tasks));
    }
    Ok(out)
}
fn imports(path: &Path) -> Result<Vec<(String, Value, Vec<Value>)>, String> {
    if path.file_name().is_some_and(|n| n == "warp.sqlite")
        || path.extension().is_some_and(|n| n == "sqlite")
    {
        return database(path);
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let tasks = if path.extension().is_some_and(|e| e == "json") {
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !v["tasks"].is_array() {
            return graphql_conversations(&v);
        }
        v["tasks"].as_array().cloned().unwrap_or_default()
    } else {
        proto_conversation(&bytes)?
    };
    if tasks.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![(id, Value::Null, tasks)])
}
fn graphql_conversations(v: &Value) -> Result<Vec<(String, Value, Vec<Value>)>, String> {
    // Official ListAIConversations HTTP response; native get_ai_conversation
    // decodes this exact finalTaskList as STANDARD base64 -> ConversationData.
    let output = &v["data"]["listAIConversations"];
    if output["__typename"] != "ListAIConversationsOutput" {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for c in list(output, "conversations") {
        let id = string(c, "conversation_id");
        let payload = string(c, "final_task_list");
        // Third-party CLI block snapshots are a different contract and must
        // not be relabeled as Warp's own agent.
        if id.is_empty() || payload.is_empty() || c["harness"] != "OZ" {
            continue;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| format!("invalid finalTaskList base64: {e}"))?;
        let tasks = proto_conversation(&bytes)?;
        let meta = json!({"server_conversation_token":id,
            "run_id":field(c, "ambient_agent_task_id"),
            "working_directory":field(c, "working_directory")});
        out.push((id.into(), meta, tasks));
    }
    Ok(out)
}
fn session_id<'a>(id: &'a str, meta: &'a Value) -> &'a str {
    [
        string(meta, "server_conversation_token"),
        string(meta, "run_id"),
        id,
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or(id)
}
fn aliases<'a>(id: &'a str, meta: &'a Value) -> impl Iterator<Item = String> + 'a {
    [
        id,
        string(meta, "run_id"),
        string(meta, "server_conversation_token"),
    ]
    .into_iter()
    .filter(|id| !id.is_empty())
    .map(str::to_owned)
}
/// Oz exports can identify the same run by its server token or run ID. Suppress
/// those metadata rows only if the native reader can actually retain the session.
pub(crate) fn native_session_aliases(pm: Option<&PathMap>) -> HashSet<String> {
    let mut out = HashSet::new();
    for p in input_files() {
        for (id, meta, tasks) in imports(&p).unwrap_or_default() {
            let readable = if let Some(pm) = pm {
                let mut g = FileGroup::default();
                parse(session_id(&id, &meta), &tasks, &meta, pm, &mut g);
                !g.sessions.is_empty()
            } else {
                !string(&meta, "working_directory").is_empty()
                    || tasks.iter().any(|t| {
                        list(t, "messages").iter().any(|m| {
                            ["user_query", "system_query", "tool_call_result"]
                                .iter()
                                .any(|key| !cwd_of(field(field(m, key), "context")).is_empty())
                        })
                    })
            };
            if readable {
                out.extend(aliases(&id, &meta));
            }
        }
    }
    out
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    scan_paths(input_files(), store, pm, st);
}
fn scan_paths(files: Vec<PathBuf>, store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let prefix = "warp:native:";
    let keys: HashSet<String> = files
        .iter()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(prefix) || keys.contains(k));
    let ordered_keys: Vec<_> = files
        .iter()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    for p in files {
        let a = fingerprint(&p);
        let b = fingerprint(&PathBuf::from(format!("{}-wal", p.display())));
        let fp = (a.0.max(b.0), a.1 + b.1);
        let key = format!("{prefix}{}", p.display());
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        match imports(&p) {
            Ok(rows) => {
                for (id, meta, tasks) in rows {
                    parse(session_id(&id, &meta), &tasks, &meta, pm, &mut g)
                }
            }
            Err(e) => st.warnings.push(format!("warp: {}: {e}", p.display())),
        };
        store.groups.insert(key, g);
    }
    let mut seen = HashSet::new();
    for key in ordered_keys {
        if let Some(g) = store.groups.get_mut(&key) {
            let previous = seen.clone();
            let before = (g.sessions.len(), g.evs.len());
            g.sessions.retain(|s| seen.insert(s.id.clone()));
            g.evs.retain(|e| !previous.contains(&e.session));
            // Re-read a suppressed copy next time: if the preferred native DB
            // is removed, this export must become usable without changing it.
            if before != (g.sessions.len(), g.evs.len()) {
                g.fp = (0, 0);
            }
        }
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    for p in input_files() {
        for (id, meta, tasks) in imports(&p).unwrap_or_default() {
            let mut roots = HashSet::new();
            let initial = string(&meta, "working_directory");
            if !initial.is_empty() {
                roots.insert(initial.to_string());
            }
            let mut last = 0;
            for t in &tasks {
                for m in list(t, "messages") {
                    last = last.max(timestamp(&m["timestamp"]));
                    for key in ["user_query", "system_query", "tool_call_result"] {
                        let cwd = cwd_of(field(field(m, key), "context"));
                        if !cwd.is_empty() {
                            roots.insert(cwd.to_string());
                        }
                    }
                }
            }
            for root in roots {
                if aliases(&id, &meta).any(|id| seen.contains(&(id, root.clone()))) {
                    continue;
                }
                seen.extend(aliases(&id, &meta).map(|id| (id, root.clone())));
                out.push(imp::activity(Src::Warp, root, None, last));
            }
        }
    }
    out
}
pub fn doctor() -> Vec<Health> {
    let files = input_files();
    let location = files
        .first()
        .cloned()
        .or_else(|| app_roots().into_iter().next())
        .unwrap_or_else(|| {
            #[cfg(target_os = "macos")]
            {
                crate::home().join("Library/Application Support")
            }
            #[cfg(target_os = "windows")]
            {
                std::env::var("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| crate::home())
                    .join("warp")
            }
            #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
            {
                std::env::var("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| crate::home().join(".local/state"))
            }
        });
    let mut n = 0;
    let mut errors = vec![];
    for p in &files {
        match imports(p) {
            Ok(rows) => n += rows.len(),
            Err(e) => errors.push(format!("{}: {e}", p.display())),
        }
    }
    let mut note="Reads native agent_conversations + protobuf agent_tasks, ConversationData .pb / protobuf-JSON, and official ListAIConversations GraphQL JSON (OZ finalTaskList base64 only). Only matched apply_file_diffs Success results with returned content are accepted; failed/unmatched/reverted calls, user-edited files and new_files overwrites without before-images are excluded. Uses request-correlated ModelUsed and message timestamps; ambiguous/missing models remain unknown. No installed local Warp/account sample was available; integration is verified with official-schema fixtures.".to_string();
    if !errors.is_empty() {
        note.push_str(&format!(" Read errors: {}", errors.join("; ")));
    }
    vec![imp::health(
        Src::Warp,
        &location,
        "Warp native SQLite + Task protobuf / ConversationData",
        n,
        &note,
        false,
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into(), "b.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    fn tasks() -> Vec<Value> {
        vec![
            json!({"id":"task","messages":[{"id":"q","requestId":"r","timestamp":"2026-10-17T00:00:00Z","userQuery":{"context":{"directory":{"pwd":"/demo"}}}},{"id":"model","requestId":"r","modelUsed":{"modelId":"claude-sonnet-4"}},{"id":"call","requestId":"r","toolCall":{"toolCallId":"c","applyFileDiffs":{"diffs":[{"filePath":"a.rs","search":"keep_line","replace":"keep_line\naccepted_line"}],"newFiles":[{"filePath":"b.rs","content":"human_line"}]}}},{"id":"result","requestId":"r","timestamp":"2026-10-17T00:01:00Z","toolCallResult":{"toolCallId":"c","applyFileDiffs":{"success":{"updatedFilesV2":[{"file":{"filePath":"/demo/a.rs","content":"keep_line\naccepted_line"},"wasEditedByUser":false},{"file":{"filePath":"/demo/b.rs","content":"human_line"},"wasEditedByUser":true}]}}}}]}),
        ]
    }
    #[test]
    fn matched_success_uses_request_model_and_excludes_human_edits() {
        let mut g = FileGroup::default();
        parse("session", &tasks(), &Value::Null, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "a.rs");
        assert_eq!(g.evs[0].model, "claude-sonnet-4");
        assert!(g.evs[0].ts > 0);
        assert_eq!(g.sessions.len(), 1);
    }
    #[test]
    fn errors_unmatched_unreturned_and_reverted_calls_are_not_writes() {
        let mut ts = tasks();
        ts[0]["messages"][3]["toolCallResult"]["applyFileDiffs"] =
            json!({"error":{"message":"permission denied"}});
        let mut g = FileGroup::default();
        parse("s", &ts, &Value::Null, &pm(), &mut g);
        assert!(g.evs.is_empty());
        let mut g = FileGroup::default();
        parse(
            "s",
            &tasks(),
            &json!({"reverted_action_ids":["c"]}),
            &pm(),
            &mut g,
        );
        assert!(g.evs.is_empty());
        let mut ts = tasks();
        ts[0]["messages"][3]["toolCallResult"]["toolCallId"] = json!("other");
        let mut g = FileGroup::default();
        parse("s", &ts, &Value::Null, &pm(), &mut g);
        assert!(g.evs.is_empty());
    }
    #[test]
    fn returned_content_must_match_and_session_content_stays_local() {
        let mut g = FileGroup::default();
        parse("edited", &tasks(), &Value::Null, &pm(), &mut g);
        let mut ts = tasks();
        ts[0]["messages"][3]["toolCallResult"]["applyFileDiffs"]["success"]["updatedFilesV2"][0]
            ["file"]["content"] = json!("different_line");
        parse("no-edit", &ts, &Value::Null, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert!(g.sessions[0].has_content);
        assert!(!g.sessions[1].has_content);
        assert_eq!(g.sessions[1].model, "claude-sonnet-4");
        assert_eq!(
            aliases(
                "local",
                &json!({"run_id":"run", "server_conversation_token":"server"})
            )
            .collect::<HashSet<_>>(),
            ["local".into(), "run".into(), "server".into()]
                .into_iter()
                .collect()
        );
    }
    #[test]
    fn new_file_overwrite_and_parent_escape_never_claim_human_context() {
        let mut ts = tasks();
        ts[0]["messages"][2]["toolCall"]["applyFileDiffs"] = json!({"newFiles":[
            {"filePath":"a.rs", "content":"retained_human_line", "allowOverwrite":true},
            {"filePath":"../other/a.rs", "content":"outside_line"}
        ]});
        ts[0]["messages"][3]["toolCallResult"]["applyFileDiffs"]["success"] = json!({"updatedFilesV2":[
            {"file":{"filePath":"a.rs", "content":"retained_human_line"}},
            {"file":{"filePath":"../other/a.rs", "content":"outside_line"}}
        ]});
        let mut g = FileGroup::default();
        parse("s", &ts, &Value::Null, &pm(), &mut g);
        assert!(g.evs.is_empty());
        assert_eq!(absolute("/demo/sub", "./../a.rs"), "/demo/a.rs");
    }
    #[test]
    fn ambiguous_model_is_not_backfilled_from_another_turn() {
        let mut ts = tasks();
        ts[0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"requestId":"r","modelUsed":{"modelId":"another-model"}}));
        let mut g = FileGroup::default();
        parse("s", &ts, &Value::Null, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
    }
    fn enc(n: u32, b: &[u8]) -> Vec<u8> {
        fn var(mut n: u64) -> Vec<u8> {
            let mut b = vec![];
            while n >= 128 {
                b.push((n as u8 & 127) | 128);
                n >>= 7;
            }
            b.push(n as u8);
            b
        }
        let mut out = var(((n as u64) << 3) | 2);
        out.extend(var(b.len() as u64));
        out.extend(b);
        out
    }
    fn text(n: u32, s: &str) -> Vec<u8> {
        enc(n, s.as_bytes())
    }
    fn binary_task() -> Vec<u8> {
        let mut call = text(1, "c");
        let mut new = text(1, "/demo/a.rs");
        new.extend(text(2, "actual_line"));
        call.extend(enc(6, &enc(3, &new)));
        let mut m = text(1, "call");
        m.extend(text(13, "request"));
        m.extend(enc(4, &call));
        let mut result = text(1, "c");
        let mut f = text(1, "/demo/a.rs");
        f.extend(text(2, "actual_line"));
        let u = enc(1, &f);
        result.extend(enc(6, &enc(1, &enc(2, &u))));
        let mut r = text(1, "result");
        r.extend(enc(5, &result));
        // Timestamp.seconds = 1_780_000_000 (protobuf varint); nanos don't affect line matching.
        r.extend(enc(14, &[8, 128, 202, 226, 208, 6]));
        let mut model = text(1, "model");
        model.extend(text(13, "request"));
        model.extend(enc(25, &text(1, "gpt-4.1")));
        let mut t = text(1, "task");
        for m in [model, m, r] {
            t.extend(enc(5, &m));
        }
        t
    }
    #[test]
    fn native_sqlite_task_blob_and_conversationdata_decode() {
        let p = std::env::temp_dir().join(format!("creditia-warp-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch("CREATE TABLE agent_conversations(id INTEGER,conversation_id TEXT,conversation_data TEXT);CREATE TABLE agent_tasks(id INTEGER,conversation_id TEXT,task BLOB);").unwrap();
        c.execute("INSERT INTO agent_conversations VALUES(1,'s','{}')", [])
            .unwrap();
        c.execute("INSERT INTO agent_tasks VALUES(1,'s',?1)", [binary_task()])
            .unwrap();
        drop(c);
        let rows = database(&p).unwrap();
        let mut g = FileGroup::default();
        parse(&rows[0].0, &rows[0].2, &rows[0].1, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "gpt-4.1");
        assert_eq!(g.evs[0].ts, 1_780_000_000);
        assert_eq!(
            proto_conversation(&enc(1, &binary_task())).unwrap().len(),
            1
        );
        assert!(proto_task(&[255, 255]).is_err());
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn official_graphql_final_task_list_decodes_and_excludes_external_harnesses() {
        let body = base64::engine::general_purpose::STANDARD.encode(enc(1, &binary_task()));
        let mut response = json!({"data":{"listAIConversations":{
            "__typename":"ListAIConversationsOutput",
            "conversations":[{"conversationId":"server-id", "harness":"OZ",
                "finalTaskList":body, "workingDirectory":"/demo", "ambientAgentTaskId":"run-id"}]
        }}});
        let rows = graphql_conversations(&response).unwrap();
        let (id, meta, tasks) = &rows[0];
        assert_eq!(session_id(id, meta), "server-id");
        assert!(aliases(id, meta).any(|s| s == "run-id"));
        let mut g = FileGroup::default();
        parse(id, tasks, meta, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "gpt-4.1");
        response["data"]["listAIConversations"]["conversations"][0]["harness"] =
            json!("CLAUDE_CODE");
        assert!(graphql_conversations(&response).unwrap().is_empty());
        response["data"]["listAIConversations"]["conversations"][0]["harness"] = json!("OZ");
        response["data"]["listAIConversations"]["conversations"][0]["finalTaskList"] =
            json!("!!invalid!!");
        assert!(graphql_conversations(&response).is_err());
    }
    #[test]
    fn native_and_cloud_export_are_one_session_and_removed_native_allows_export_again() {
        let root = std::env::temp_dir().join(format!("creditia-warp-dedup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("warp.sqlite");
        let export = root.join("download.json");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE agent_conversations(id INTEGER,conversation_id TEXT,conversation_data TEXT);CREATE TABLE agent_tasks(id INTEGER,conversation_id TEXT,task BLOB);").unwrap();
        c.execute(
            "INSERT INTO agent_conversations VALUES(1,'local-id',?1)",
            [json!({"server_conversation_token":"server-id", "run_id":"run-id"}).to_string()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO agent_tasks VALUES(1,'local-id',?1)",
            [binary_task()],
        )
        .unwrap();
        drop(c);
        let payload = base64::engine::general_purpose::STANDARD.encode(enc(1, &binary_task()));
        std::fs::write(
            &export,
            json!({"data":{"listAIConversations":{
                "__typename":"ListAIConversationsOutput", "conversations":[{
                    "conversationId":"server-id", "harness":"OZ", "finalTaskList":payload
                }]
            }}})
            .to_string(),
        )
        .unwrap();
        let mut store = Store::default();
        let mut st = Stats::default();
        for _ in 0..2 {
            scan_paths(vec![db.clone(), export.clone()], &mut store, &pm(), &mut st);
            assert_eq!(
                store
                    .groups
                    .values()
                    .map(|g| g.sessions.len())
                    .sum::<usize>(),
                1
            );
            assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        }
        scan_paths(vec![export], &mut store, &pm(), &mut st);
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        assert!(st.warnings.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
