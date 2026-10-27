//! Posit Assistant's V2 trees and V3 tree + lm-messages.jsonl store.
//! Contract inspected in the official pa distribution; see SUPPORT_AGENT18.md.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn roots() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("CREDITIA_POSIT_ASSISTANT_DIR") {
        return vec![p.into()];
    }
    let mut roots = vec![
        crate::home().join(".posit/assistant"),
        crate::home().join(".positai"),
    ];
    for root in roots.clone() {
        if let Ok(settings) = n::json(&root.join("settings.json")) {
            if let Some(p) = settings["storage"]["path"]
                .as_str()
                .filter(|p| !p.is_empty())
            {
                roots.push(if let Some(rest) = p.strip_prefix("~/") {
                    crate::home().join(rest)
                } else {
                    p.into()
                });
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}
fn files(projects: &[String]) -> Vec<(PathBuf, String)> {
    let mut files = vec![];
    for root in roots() {
        files.extend(
            n::files(&root, |p| {
                p.file_name().is_some_and(|s| s == "conversation.json")
                    && !p
                        .components()
                        .any(|c| matches!(c.as_os_str().to_str(), Some("backups" | "quarantine")))
            })
            .into_iter()
            .map(|p| {
                let cwd = workspace(&p);
                (p, cwd)
            }),
        );
    }
    for project in projects {
        files.extend(
            n::files(
                &Path::new(project).join(".creditia/imports/posit-assistant"),
                |p| p.extension().is_some_and(|s| s == "json"),
            )
            .into_iter()
            .map(|p| (p, project.clone())),
        );
    }
    files.sort();
    files.dedup();
    files
}
fn workspace(path: &Path) -> String {
    // The vendor writes workspace.json beside the workspace's conversation directories.
    path.ancestors()
        .skip(1)
        .find_map(|p| {
            n::json(&p.join("workspace.json"))
                .ok()?
                .get("path")?
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn active_nodes(doc: &Value) -> Result<Vec<&Value>, String> {
    let rows = doc["messages"]
        .as_array()
        .ok_or("Posit tree has no messages")?;
    let mut by_id = HashMap::new();
    let mut next = HashMap::new();
    for node in rows {
        let id = node["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Posit node has no id")?;
        if by_id.insert(id, node).is_some() {
            return Err("duplicate Posit tree node".into());
        }
        if doc["schemaVersion"] == "3" && node["isActive"] == true {
            let parent = node["parentId"]
                .as_str()
                .ok_or("Posit node has no parent")?;
            if next.insert(parent, id).is_some() {
                return Err("multiple active Posit branches".into());
            }
        }
    }
    if doc["schemaVersion"] == "2" {
        for node in std::iter::once(&doc["root"]).chain(rows) {
            if let Some(child) = node["activeChildId"].as_str() {
                next.insert(n::text(&node["id"]), child);
            }
        }
    }
    let mut out = vec![];
    let mut id = n::text(&doc["root"]["id"]);
    let mut seen = HashSet::new();
    while let Some(child) = next.get(id) {
        if !seen.insert(*child) {
            return Err("cycle in Posit tree".into());
        }
        let node = *by_id.get(child).ok_or("missing active Posit tree node")?;
        out.push(node);
        id = child;
    }
    Ok(out)
}

fn edits(call: &Value, result: &Value) -> Vec<(String, String)> {
    let args = &call["input"];
    let output = &result["output"];
    let name = n::text(&call["toolName"]);
    if name == "apply_patch" {
        let ledger = &result["providerOptions"]["databot"]["affectedPaths"];
        if ledger["version"] != 1 {
            return vec![];
        }
        return crate::apply_patch_files(n::text(&args["patch"]))
            .into_iter()
            .filter(|(path, _)| {
                ledger["operations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|op| {
                        op["outcome"] == "applied"
                            && (op["path"] == *path
                                || op["kind"] == "move" && op["destination"] == *path)
                    })
            })
            .collect();
    }
    if output["type"] != "text" {
        return vec![];
    }
    let path = n::text(&args["file_path"]);
    let receipt = n::text(&output["value"]);
    if path.is_empty() {
        return vec![];
    }
    let text = match name {
        "write"
            if args["content"].is_string()
                && (receipt == format!("Created file: {path}")
                    || receipt == format!("Updated file: {path}")) =>
        {
            n::text(&args["content"]).to_string()
        }
        "edit"
            if args["old_string"].is_string()
                && args["new_string"].is_string()
                && receipt == format!("Updated {path}") =>
        {
            // The vendor's fuzzy matcher only adjusts whitespace; line fingerprints
            // already normalize that. Never use the pre-execution preview as a receipt.
            crate::added_lines(n::text(&args["old_string"]), n::text(&args["new_string"]))
        }
        _ => return vec![],
    };
    vec![(path.into(), text)]
}

fn lm_rows<'a>(doc: &'a Value, log: &'a [Value]) -> Result<Vec<(&'a Value, i64)>, String> {
    if !matches!(doc["schemaVersion"].as_str(), Some("2" | "3")) {
        return Err("unsupported Posit conversation schema".into());
    }
    let mut messages = HashMap::new();
    for row in log {
        if let Some(id) = row["id"].as_u64() {
            messages.insert(id, &row["message"]);
        }
    }
    let mut seen_messages = HashSet::new();
    let mut all = vec![];
    for node in active_nodes(doc)? {
        let node_time = crate::any_ts(&node["timestamp"]).unwrap_or(0);
        let rows: Vec<&Value> = if doc["schemaVersion"] == "3" {
            let mut out = vec![];
            for id in node["lmMessageIds"]
                .as_array()
                .ok_or("Posit node has no message references")?
            {
                let id = id.as_u64().ok_or("invalid Posit message reference")?;
                if !seen_messages.insert(id) {
                    return Err("repeated Posit message reference".into());
                }
                out.push(*messages.get(&id).ok_or("Posit message log is incomplete")?);
            }
            out
        } else {
            node["message"]["lmMessage"]
                .as_array()
                .ok_or("Posit export has no LM messages")?
                .iter()
                .collect()
        };
        // EngineTranscriptMirror.finish stamps only the final assistant row.
        // The whole node belongs to that external engine, including earlier calls.
        if rows
            .iter()
            .any(|r| r["providerOptions"]["providerMetadata"]["positai"]["engineId"].is_string())
        {
            continue;
        }
        all.extend(rows.into_iter().map(|r| (r, node_time)));
    }
    Ok(all)
}

fn parse(
    doc: &Value,
    log: &[Value],
    cwd: &str,
    pm: &PathMap,
    owned: Option<&HashSet<String>>,
) -> Result<(FileGroup, HashSet<String>), String> {
    let id = doc["root"]["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Posit tree has no root id")?;
    let rows = lm_rows(doc, log)?;
    let mut calls = HashMap::new();
    let mut seen_calls = HashSet::new();
    let mut g = FileGroup::default();
    let start = crate::any_ts(&doc["root"]["timestamp"]).unwrap_or(0);
    let (mut end, mut prompts) = (start, 0);
    for (row, node_time) in &rows {
        let meta = &row["providerOptions"]["providerMetadata"]["positai"];
        let ts = crate::any_ts(&meta["timestamp"]).unwrap_or(*node_time);
        end = end.max(ts);
        if row["role"] == "user" {
            prompts += 1;
        }
        for part in row["content"].as_array().into_iter().flatten() {
            let cid = n::text(&part["toolCallId"]);
            if cid.is_empty() {
                continue;
            }
            if row["role"] == "assistant" && part["type"] == "tool-call" {
                calls.insert(cid, (part, n::text(&meta["modelId"]), ts));
            } else if row["role"] == "tool" && part["type"] == "tool-result" {
                let Some((call, model, call_time)) = calls.remove(cid) else {
                    continue;
                };
                if call["toolName"] != part["toolName"]
                    || !seen_calls.insert(cid.to_string())
                    || owned.is_some_and(|o| !o.contains(cid))
                {
                    continue;
                }
                for (path, text) in edits(call, part) {
                    imp::push_scoped(
                        &mut g,
                        pm,
                        cwd,
                        &path,
                        &text,
                        Src::PositAssistant,
                        model,
                        id,
                        call_time,
                    );
                }
            }
        }
    }
    if rows.is_empty() {
        g.reconciled_sessions.push((Src::PositAssistant, id.into()));
    } else {
        n::session(
            &mut g,
            pm,
            Src::PositAssistant,
            id,
            cwd,
            start,
            end,
            prompts,
            0,
        );
    }
    Ok((g, seen_calls))
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    let files = files(projects);
    let cwd: HashMap<_, _> = files.iter().cloned().collect();
    let mut parsed = HashMap::new();
    let mut owners = HashMap::new();
    for (p, cwd) in &files {
        let result = n::json(p).and_then(|doc| {
            let log = if doc["schemaVersion"] == "3" {
                n::jsonl(&p.with_file_name("lm-messages.jsonl"))?
            } else {
                vec![]
            };
            let (_, calls) = parse(&doc, &log, cwd, pm, None)?;
            // Full exports / cloned trees can replace root and node IDs while
            // preserving tool-call IDs. A final failure must own the call too.
            let rank = (
                std::fs::metadata(p).and_then(|m| m.modified()).ok(),
                p.clone(),
            );
            for cid in calls {
                let best = owners.entry(cid).or_insert_with(|| rank.clone());
                if rank > *best {
                    *best = rank.clone();
                }
            }
            Ok((doc, log))
        });
        parsed.insert(p.clone(), result);
    }
    n::scan(
        Src::PositAssistant,
        store,
        pm,
        st,
        files.into_iter().map(|(p, _)| p).collect(),
        |p, pm| {
            let (doc, log) = parsed
                .get(p)
                .ok_or("missing Posit history")?
                .as_ref()
                .map_err(Clone::clone)?;
            let owned = owners
                .iter()
                .filter(|(_, (_, path))| path == p)
                .map(|(cid, _)| cid.clone())
                .collect();
            parse(
                doc,
                log,
                cwd.get(p).map(String::as_str).unwrap_or(""),
                pm,
                Some(&owned),
            )
            .map(|(g, _)| g)
        },
    );
}
pub fn discover() -> Vec<Activity> {
    files(&[])
        .into_iter()
        .filter_map(|(p, cwd)| {
            if cwd.is_empty() {
                return None;
            }
            let doc = n::json(&p).ok()?;
            Some(imp::activity(
                Src::PositAssistant,
                cwd,
                None,
                crate::any_ts(&doc["root"]["timestamp"]).unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::PositAssistant, &roots()[0], "V2 conversation tree / V3 tree + lm-messages.jsonl", files(&[]).len(), "Active branch; paired write/edit receipts and applied patch operations. Per-message model. Workspace path from workspace.json; full V2 exports also accepted in .creditia/imports/posit-assistant. Shell tools, engine-hosted chats and repeated replacements are not attributed.", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn active_tree_requires_complete_successful_pairs() {
        let mut doc = json!({"schemaVersion":"3","root":{"id":"s","timestamp":1700000000000i64},"messages":[{"id":"a","parentId":"s","isActive":true,"timestamp":1700000000000i64,"lmMessageIds":[0,1]},{"id":"b","parentId":"s","isActive":false,"lmMessageIds":[2]}]});
        let mut rows = vec![
            json!({"id":0,"message":{"role":"assistant","providerOptions":{"providerMetadata":{"positai":{"modelId":"actual-model"}}},"content":[{"type":"tool-call","toolCallId":"c","toolName":"write","input":{"file_path":"src/main.rs","content":"fn saved() {}"}}]}}),
            json!({"id":1,"message":{"role":"tool","content":[{"type":"tool-result","toolCallId":"c","toolName":"write","output":{"type":"text","value":"Created file: src/main.rs"}}]}}),
        ];
        let g = parse(&doc, &rows, "/demo", &n::project(), None).unwrap().0;
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "actual-model");
        // An external engine stamps the last assistant, not each tool call.
        rows.push(json!({"id":3,"message":{"role":"assistant","content":[],"providerOptions":{"providerMetadata":{"positai":{"engineId":"codex"}}}}}));
        doc["messages"][0]["lmMessageIds"] = json!([0, 1, 3]);
        let mirrored = parse(&doc, &rows, "/demo", &n::project(), None).unwrap().0;
        assert!(mirrored.evs.is_empty());
        assert!(mirrored.sessions.is_empty());
        doc["messages"][0]["lmMessageIds"] = json!([0, 1]);
        rows[1]["message"]["content"][0]["output"]["type"] = json!("error-text");
        assert!(parse(&doc, &rows, "/demo", &n::project(), None)
            .unwrap()
            .0
            .evs
            .is_empty());
        doc["messages"][0]["isActive"] = json!(false);
        doc["messages"][1]["isActive"] = json!(true);
        assert!(parse(&doc, &rows, "/demo", &n::project(), None).is_err());
    }
    #[test]
    fn partial_patch_counts_only_applied_files() {
        let call = json!({"toolName":"apply_patch","input":{"patch":"*** Begin Patch\n*** Add File: src/main.rs\n+fn saved() {}\n*** Add File: failed.rs\n+not written\n*** End Patch"}});
        let result = json!({"output":{"type":"error-text"},"providerOptions":{"databot":{"affectedPaths":{"version":1,"operations":[{"kind":"add","path":"src/main.rs","outcome":"applied"},{"kind":"add","path":"failed.rs","outcome":"failed"}]}}}});
        assert_eq!(
            edits(&call, &result),
            vec![("src/main.rs".into(), "fn saved() {}\n".into())]
        );
    }
}
