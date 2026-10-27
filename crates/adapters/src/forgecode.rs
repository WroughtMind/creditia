//! ForgeCode conversation.context JSON inside .forge.db. Never read API credentials.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn paths() -> Vec<PathBuf> {
    if let Some(p) =
        std::env::var_os("CREDITIA_FORGECODE_DIR").or_else(|| std::env::var_os("FORGE_CONFIG"))
    {
        let p = PathBuf::from(p);
        return vec![if p.is_file() { p } else { p.join(".forge.db") }];
    }
    let legacy = crate::home().join("forge");
    vec![if legacy.exists() {
        legacy.join(".forge.db")
    } else {
        crate::home().join(".forge/.forge.db")
    }]
}
fn load(path: &Path) -> Result<Vec<(String, i64, Value)>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    let mut q=db.prepare("select conversation_id, cast(strftime('%s',created_at) as integer), context from conversations where context is not null").map_err(|e|e.to_string())?;
    let out = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .map(|r| {
            r.map_err(|e| e.to_string()).and_then(|(id, ts, s)| {
                serde_json::from_str(&s)
                    .map(|v| (id, ts, v))
                    .map_err(|e| e.to_string())
            })
        })
        .collect();
    out
}
/// ForgeDisplay diffs contain two line-number columns, then |+ and exact new bytes.
fn formatted_additions(text: &str) -> String {
    text.lines()
        .filter_map(|line| {
            let (numbers, body) = line.split_once('|')?;
            if !numbers.chars().all(|c| c.is_ascii_digit() || c == ' ') {
                return None;
            }
            body.strip_prefix('+')
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn parse(id: &str, ts: i64, context: &Value, pm: &PathMap, g: &mut FileGroup) {
    let before = g.evs.len();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut prompts = 0;
    for row in context["messages"].as_array().into_iter().flatten() {
        let message = row.get("message").unwrap_or(row);
        let text = &message["text"];
        if text["role"] == "user" || text["role"] == "User" {
            prompts += 1;
        }
        if text["role"] == "assistant" || text["role"] == "Assistant" {
            for c in text["tool_calls"].as_array().into_iter().flatten() {
                let cid = n::text(&c["call_id"]);
                if !cid.is_empty() {
                    calls.insert(cid, (c, n::text(&text["model"])));
                }
            }
        }
        let tool = &message["tool"];
        let cid = n::text(&tool["call_id"]);
        let Some((call, model)) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) || tool["output"]["is_error"] != false || tool["name"] != call["name"]
        {
            continue;
        }
        let args = if let Some(s) = call["arguments"].as_str() {
            serde_json::from_str(s).unwrap_or(Value::Null)
        } else {
            call["arguments"].clone()
        };
        let name = n::text(&call["name"]);
        if !matches!(name, "fs_write" | "fs_patch" | "fs_multi_patch") {
            continue;
        }
        let path = n::text(&args["file_path"]);
        for value in tool["output"]["values"].as_array().into_iter().flatten() {
            if let Some(d) = value.get("fileDiff") {
                let added = crate::added_lines(n::text(&d["old_text"]), n::text(&d["new_text"]));
                imp::push_scoped(
                    g,
                    pm,
                    "",
                    n::text(&d["path"]),
                    &added,
                    Src::Forgecode,
                    model,
                    id,
                    ts,
                );
                continue;
            }
            let receipt = n::text(&value["text"]);
            let added = if name == "fs_write" && receipt.starts_with("<file_created ") {
                n::text(&args["content"]).to_string()
            } else if receipt.starts_with("<file_diff ")
                || receipt.starts_with("<file_overwritten ")
            {
                receipt
                    .split("<![CDATA[")
                    .skip(1)
                    .filter_map(|p| p.split_once("]]>").map(|(d, _)| formatted_additions(d)))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                continue;
            };
            imp::push_scoped(g, pm, "", path, &added, Src::Forgecode, model, id, ts);
        }
    }
    n::session(g, pm, Src::Forgecode, id, "", ts, ts, prompts, before);
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    for (id, ts, v) in load(path)? {
        parse(&id, ts, &v, pm, &mut g);
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(
        Src::Forgecode,
        store,
        pm,
        st,
        paths().into_iter().filter(|p| p.is_file()).collect(),
        read,
    );
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    let ps = paths();
    imp::health(Src::Forgecode,&ps[0],"native conversations.context SQLite",ps.iter().filter(|p|p.is_file()).count(),"Successful fs_write/fs_patch/fs_multi_patch; actual rendered diffs. Per-message model; conversation timestamp (native messages have no timestamp). Workspace hashes alone do not identify a repository.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actual_numbered_diff_only() {
        let doc = json!({"messages":[{"message":{"text":{"role":"assistant","model":"m","tool_calls":[{"call_id":"c","name":"fs_patch","arguments":{"file_path":"/demo/src/main.rs"}}]}}},{"message":{"tool":{"call_id":"c","name":"fs_patch","output":{"is_error":false,"values":[{"text":"<file_diff path=\"/demo/src/main.rs\"><![CDATA[1   |-fn old() {}\n  1 |+fn saved() {}\n2 2 | fn context() {}\n]]></file_diff>"}]}}}}]});
        let mut g = FileGroup::default();
        parse("s", 1, &doc, &n::project(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(
            formatted_additions("1 1 | context\n  2 |+  exact\n3   |-old"),
            "  exact"
        );
    }
}
