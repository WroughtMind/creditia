//! DeepAgents CLI's LangGraph SQLite checkpoints. Decode data, never Python objects/pickle.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn path() -> PathBuf {
    let expand = |p: std::ffi::OsString| {
        let s = p.to_string_lossy();
        s.strip_prefix("~/")
            .map(|s| crate::home().join(s))
            .unwrap_or_else(|| PathBuf::from(p))
    };
    if let Some(p) = std::env::var_os("CREDITIA_DEEPAGENTS_DIR") {
        let p = expand(p);
        return if p.is_file() {
            p
        } else {
            p.join(".state/sessions.db")
        };
    }
    if let Some(p) = std::env::var_os("DEEPAGENTS_SESSIONS_DB") {
        return expand(p);
    }
    let root = std::env::var_os("DEEPAGENTS_HOME")
        .map(expand)
        .unwrap_or_else(|| crate::home().join(".deepagents"));
    root.join(".state/sessions.db")
}
fn unpack(v: rmpv::Value, depth: usize) -> Result<Value, String> {
    if depth > 128 {
        return Err("MessagePack nesting exceeds 128".into());
    }
    Ok(match v {
        rmpv::Value::Nil => Value::Null,
        rmpv::Value::Boolean(v) => v.into(),
        rmpv::Value::Integer(v) => {
            if let Some(n) = v.as_i64() {
                n.into()
            } else {
                v.as_u64().unwrap_or(0).into()
            }
        }
        rmpv::Value::F32(v) => json!(v),
        rmpv::Value::F64(v) => json!(v),
        rmpv::Value::String(v) => v.as_str().map(Value::from).unwrap_or(Value::Null),
        rmpv::Value::Binary(_) => Value::Null,
        rmpv::Value::Array(a) => Value::Array(
            a.into_iter()
                .map(|v| unpack(v, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        rmpv::Value::Map(m) => {
            let mut out = serde_json::Map::new();
            for (k, v) in m {
                if let Some(k) = k.as_str() {
                    out.insert(k.into(), unpack(v, depth + 1)?);
                }
            }
            Value::Object(out)
        }
        rmpv::Value::Ext(code, bytes) => {
            let payload =
                rmpv::decode::read_value(&mut bytes.as_slice()).map_err(|e| e.to_string())?;
            let tuple = unpack(payload, depth + 1)?;
            if matches!(code, 4 | 5) && n::text(&tuple[0]).starts_with("langchain_core.messages.") {
                tuple[2].clone()
            } else if code == 2 && tuple[0] == "langgraph.types" && tuple[1] == "Overwrite" {
                json!({"__overwrite":tuple[2]["value"]})
            } else {
                Value::Null
            }
        }
    })
}
fn decode(kind: &str, bytes: &[u8]) -> Result<Value, String> {
    match kind {
        "json" => serde_json::from_slice(bytes).map_err(|e| e.to_string()),
        "msgpack" => unpack(
            rmpv::decode::read_value(&mut &*bytes).map_err(|e| e.to_string())?,
            0,
        ),
        _ => Err(format!(
            "unsupported checkpoint encoding {kind}; pickle is never executed"
        )),
    }
}
fn message(v: &Value) -> Value {
    if v["lc"] == 1
        && v["type"] == "constructor"
        && v["id"]
            .as_array()
            .is_some_and(|id| id.first() == Some(&json!("langchain")))
    {
        v["kwargs"].clone()
    } else {
        v.clone()
    }
}
fn apply(messages: &mut Vec<Value>, delta: &Value) {
    if let Some(a) = delta["__overwrite"].as_array() {
        *messages = a.iter().map(message).collect();
        return;
    }
    let one;
    let deltas = if let Some(a) = delta.as_array() {
        a
    } else {
        one = vec![delta.clone()];
        &one
    };
    for v in deltas {
        let v = message(v);
        let id = n::text(&v["id"]);
        if v["type"] == "remove" {
            if id == "__remove_all__" {
                messages.clear();
            } else {
                messages.retain(|m| m["id"] != id);
            }
            continue;
        }
        if !id.is_empty() {
            if let Some(old) = messages.iter_mut().find(|m| m["id"] == id) {
                *old = v;
                continue;
            }
        }
        messages.push(v);
    }
}
fn load(path: &Path) -> Result<Vec<(String, Value, Vec<Value>)>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut q=db.prepare("select thread_id, checkpoint_id, type, checkpoint, metadata from checkpoints c where checkpoint_ns='' and checkpoint_id=(select max(checkpoint_id) from checkpoints where thread_id=c.thread_id and checkpoint_ns='') order by thread_id").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get_ref(4)?.as_bytes()?.to_vec(),
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for row in rows {
        let (id, cid, kind, bytes, meta) = row.map_err(|e| e.to_string())?;
        let checkpoint = decode(&kind, &bytes)?;
        let metadata: Value = serde_json::from_slice(&meta).map_err(|e| e.to_string())?;
        let inline = checkpoint["channel_values"]["messages"].as_array();
        let mut messages = inline
            .map(|a| a.iter().map(message).collect())
            .unwrap_or_default();
        let mut writes=db.prepare("select type,value from writes where thread_id=? and checkpoint_ns='' and channel='messages' and (? is null or checkpoint_id=?) order by checkpoint_id,task_id,idx").map_err(|e|e.to_string())?;
        let filter = inline.map(|_| cid.as_str());
        let rows = writes
            .query_map(rusqlite::params![id, filter, filter], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (kind, bytes) = row.map_err(|e| e.to_string())?;
            apply(&mut messages, &decode(&kind, &bytes)?);
        }
        out.push((id, metadata, messages));
    }
    Ok(out)
}
fn parse(id: &str, metadata: &Value, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let cwd = n::text(&metadata["cwd"]);
    let ts = crate::any_ts(&metadata["updated_at"]).unwrap_or(0);
    let before = g.evs.len();
    let mut prompts = 0;
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    for r in rows {
        if r["type"] == "human" {
            prompts += 1;
        }
        if r["type"] == "ai" {
            for c in r["tool_calls"].as_array().into_iter().flatten() {
                let cid = n::text(&c["id"]);
                if !cid.is_empty() {
                    let model = r["response_metadata"]["model_name"]
                        .as_str()
                        .or_else(|| r["response_metadata"]["model"].as_str())
                        .unwrap_or("");
                    calls.insert(cid, (c, model));
                }
            }
        }
        if r["type"] != "tool" || r["status"] != "success" {
            continue;
        }
        let cid = n::text(&r["tool_call_id"]);
        let Some((call, model)) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) || r["name"] != call["name"] {
            continue;
        }
        let args = &call["args"];
        let path = n::text(&args["file_path"]);
        let output = n::text(&r["content"]);
        // CLI local filesystem paths only. Reserved virtual mounts are not repository files.
        if [
            "/memories/",
            "/conversation_history/",
            "/large_tool_results/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        {
            continue;
        }
        let added = match n::text(&call["name"]) {
            "write_file" if output == format!("Updated file {path}") => {
                n::text(&args["content"]).to_string()
            }
            "edit_file"
                if output
                    .strip_prefix("Successfully replaced ")
                    .and_then(|s| s.split_once(" instance(s) of the string in '"))
                    .is_some_and(|(n, p)| {
                        n.parse::<usize>().is_ok_and(|n| n > 0) && p == format!("{path}'")
                    }) =>
            {
                crate::added_lines(n::text(&args["old_string"]), n::text(&args["new_string"]))
            }
            _ => continue,
        };
        imp::push_scoped(g, pm, cwd, path, &added, Src::Deepagents, model, id, ts);
    }
    n::session(g, pm, Src::Deepagents, id, cwd, ts, ts, prompts, before);
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    for (id, meta, rows) in load(path)? {
        parse(&id, &meta, &rows, pm, &mut g);
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let p = path();
    n::scan(
        Src::Deepagents,
        store,
        pm,
        st,
        if p.is_file() { vec![p] } else { vec![] },
        read,
    );
}
pub fn discover() -> Vec<Activity> {
    load(&path())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, m, _)| {
            let cwd = n::text(&m["cwd"]);
            (!cwd.is_empty()).then(|| {
                imp::activity(
                    Src::Deepagents,
                    cwd.into(),
                    None,
                    crate::any_ts(&m["updated_at"]).unwrap_or(0),
                )
            })
        })
        .collect()
}
pub fn doctor() -> Health {
    let p = path();
    imp::health(Src::Deepagents,&p,"native LangGraph SQLite JSON/MessagePack",usize::from(p.is_file()),"CLI local-filesystem write/edit receipts, latest root checkpoint plus pending writes. Timestamp is checkpoint update time; model from response_metadata. Arbitrary framework backends are outside this reader.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn msgpack_pydantic_is_data_and_remove_retracts() {
        let inner = rmpv::Value::Array(vec![
            "langchain_core.messages.ai".into(),
            "AIMessage".into(),
            rmpv::Value::Map(vec![
                ("type".into(), "ai".into()),
                ("id".into(), "a".into()),
            ]),
            "model_validate_json".into(),
        ]);
        let mut bytes = vec![];
        rmpv::encode::write_value(&mut bytes, &inner).unwrap();
        let v = unpack(rmpv::Value::Ext(5, bytes), 0).unwrap();
        assert_eq!(v["type"], "ai");
        let mut messages = vec![v];
        apply(&mut messages, &json!({"type":"remove","id":"a"}));
        assert!(messages.is_empty());
        assert!(decode("pickle", b"whatever").is_err());
    }
    #[test]
    fn positive_receipt_and_model() {
        let mut g = FileGroup::default();
        let rows = vec![
            json!({"type":"ai","response_metadata":{"model_name":"m"},"tool_calls":[{"id":"c","name":"write_file","args":{"file_path":"src/main.rs","content":"fn saved() {}"}}]}),
            json!({"type":"tool","status":"success","name":"write_file","tool_call_id":"c","content":"Updated file src/main.rs"}),
        ];
        parse("s", &json!({"cwd":"/demo"}), &rows, &n::project(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "m");
    }
}
