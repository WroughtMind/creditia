//! Jazz conversation logs. Tool receipts supply absolute paths; logs do not store a model per message.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_JAZZ_DIR")
        .or_else(|| std::env::var_os("JAZZ_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".jazz"))
}
fn paths() -> Vec<PathBuf> {
    n::files(&root().join("history"), |p| {
        p.to_str()
            .is_some_and(|p| p.ends_with(".jsonl") || p.ends_with(".jsonl.gz"))
    })
}
fn parse(rows: &[Value], pm: &PathMap) -> Result<FileGroup, String> {
    let header = rows.first().ok_or("empty Jazz history")?;
    if header["type"] != "conversation" || !matches!(header["version"].as_u64(), Some(1..=4)) {
        return Err("unsupported Jazz conversation version".into());
    }
    let id = format!(
        "{}:{}",
        n::text(&header["agentId"]),
        n::text(&header["conversationId"])
    );
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut message_ids = HashSet::new();
    let start = crate::any_ts(&header["startedAt"]).unwrap_or(0);
    let mut end = start;
    let mut prompts = 0;
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    for row in rows {
        if row["type"] == "rewrite" {
            calls.clear();
            continue;
        }
        if row["type"] != "message" {
            continue;
        }
        let mid = n::text(&row["id"]);
        if !mid.is_empty() && !message_ids.insert(mid) {
            continue;
        }
        let r = &row["message"];
        let ts = crate::any_ts(&row["at"]).unwrap_or(start);
        end = end.max(ts);
        if r["role"] == "user" {
            prompts += 1;
        }
        if r["role"] == "assistant" {
            for c in r["tool_calls"].as_array().into_iter().flatten() {
                let cid = n::text(&c["id"]);
                if !cid.is_empty() {
                    calls.insert(cid, c);
                }
            }
        }
        if r["role"] != "tool" || r["cleared"] == true {
            continue;
        }
        let cid = n::text(&r["tool_call_id"]);
        let Some(call) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) {
            continue;
        }
        let name = n::text(&call["function"]["name"]);
        if !matches!(
            name,
            "write_file" | "execute_write_file" | "edit_file" | "execute_edit_file"
        ) {
            continue;
        }
        let Ok(result) = serde_json::from_str::<Value>(n::text(&r["content"])) else {
            continue;
        };
        if !result["error"].is_null() {
            continue;
        }
        let path = n::text(&result["path"]);
        if !Path::new(path).is_absolute() {
            continue;
        }
        let diff = ansi.replace_all(n::text(&result["diff"]), "");
        let added = if matches!(name, "write_file" | "execute_write_file")
            && result["isNewFile"] == true
            && result["message"] == format!("File created: {path}")
        {
            let args: Value = serde_json::from_str(n::text(&call["function"]["arguments"]))
                .unwrap_or(Value::Null);
            n::text(&args["content"]).to_string()
        } else if diff.lines().any(|l| l.starts_with("@@ "))
            && !diff.contains("output truncated")
            && !diff.contains("[redacted:")
        {
            crate::added_from_diff(&diff)
        } else {
            continue;
        };
        imp::push_scoped(&mut g, pm, "", path, &added, Src::Jazz, "", &id, ts);
    }
    n::session(&mut g, pm, Src::Jazz, &id, "", start, end, prompts, 0);
    Ok(g)
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let rows = if path.extension().and_then(|s| s.to_str()) == Some("gz") {
        let mut s = String::new();
        flate2::read::GzDecoder::new(std::fs::File::open(path).map_err(|e| e.to_string())?)
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        s.lines()
            .filter(|s| !s.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    } else {
        n::jsonl(path)?
    };
    parse(&rows, pm)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Jazz, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Jazz,&root(),"native conversation JSONL + gzip archives",paths().len(),"Paired write_file/edit_file receipts with absolute paths. Truncated/redacted diffs are excluded. Native ChatMessage has no per-message model or project cwd.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_result_body_not_success_wrapper() {
        let mut rows = vec![
            json!({"type":"conversation","version":4,"agentId":"a","conversationId":"s"}),
            json!({"type":"message","id":"1","message":{"role":"assistant","tool_calls":[{"id":"c","function":{"name":"edit_file","arguments":"{}"}}]}}),
            json!({"type":"message","id":"2","message":{"role":"tool","tool_call_id":"c","content":json!({"path":"/demo/src/main.rs","diff":"@@ -1 +1 @@\n-old\n+fn actual() {}"}).to_string()}}),
        ];
        let g = parse(&rows, &n::project()).unwrap();
        assert_eq!(g.evs.len(), 1);
        rows[2]["message"]["content"] = json!({"error":"permission denied"}).to_string().into();
        assert!(parse(&rows, &n::project()).unwrap().evs.is_empty());
    }
}
