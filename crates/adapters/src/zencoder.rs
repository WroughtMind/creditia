//! Zencoder's bundled CLI SessionStorage and IDE FileSystemChatStore.
//! Read post-execution diffs, never the IDE's pre-permission codeHunk previews.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    std::env::var_os("CREDITIA_ZENCODER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".zencoder"))
}
fn files() -> Vec<PathBuf> {
    let root = root();
    let mut out = n::files(&root.join("sessions"), |p| {
        p.extension().is_some_and(|s| s == "jsonl")
    });
    if root.is_file() {
        out.push(root.clone());
    }
    let ide_roots = if std::env::var_os("CREDITIA_ZENCODER_DIR").is_some() {
        vec![root]
    } else {
        crate::copilot::user_dirs()
    };
    for dir in ide_roots {
        out.extend(n::files(&dir, |p| {
            p.extension().is_some_and(|s| s == "json")
                && p.parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .is_some_and(|s| s == "zencoder-chat")
        }));
    }
    out.sort();
    out.dedup();
    out
}
fn absolute(path: &str) -> bool {
    Path::new(path).is_absolute() || path.get(1..2) == Some(":")
}
fn actual_diff(path: &str, output: &Value) -> String {
    if !absolute(path) || output["type"] != "diff" || output["path"] != path {
        return String::new();
    }
    // jsdiff headers may contain only the basename. The authenticated tool result
    // supplies the absolute destination; headers never override it.
    let mut hunk = false;
    let mut text = String::new();
    for line in n::text(&output["diff"]).lines() {
        if line.starts_with("@@ ") {
            hunk = true;
        } else if hunk {
            if let Some(added) = line.strip_prefix('+') {
                text.push_str(added);
                text.push('\n');
            }
        }
    }
    text
}
fn native(
    rows: &[Value],
    pm: &PathMap,
    owned: Option<&HashSet<String>>,
) -> Result<FileGroup, String> {
    let header = rows.first().ok_or("empty Zencoder session")?;
    if header["version"] != "1.0" {
        return Err("unsupported Zencoder session version".into());
    }
    let id = header["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Zencoder session has no id")?;
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let (start, mut end, mut prompts) = (crate::any_ts(&header["createdAt"]).unwrap_or(0), 0, 0);
    for message in &rows[1..] {
        let ts = crate::any_ts(&message["createdAt"]).unwrap_or(0);
        end = end.max(ts);
        if message["role"] == "user"
            && message["content"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| p["tag"] == "user-input")
        {
            prompts += 1;
        }
        for part in message["content"].as_array().into_iter().flatten() {
            let cid = n::text(&part["toolCallId"]);
            if cid.is_empty() {
                continue;
            }
            if message["role"] == "assistant" && part["type"] == "tool-call" {
                calls.insert(
                    cid,
                    (part, n::text(&message["modelDetails"]["model"]["id"]), ts),
                );
            } else if message["role"] == "tool" && part["type"] == "tool-result" {
                let Some((call, model, ts)) = calls.remove(cid) else {
                    continue;
                };
                if part["isError"] != false
                    || call["toolName"] != part["toolName"]
                    || !matches!(call["toolName"].as_str(), Some("Write" | "Edit"))
                    || !seen.insert(cid)
                    || owned.is_some_and(|ids| !ids.contains(cid))
                {
                    continue;
                }
                let path = n::text(&call["input"]["file_path"]);
                for output in part["content"].as_array().into_iter().flatten() {
                    let text = actual_diff(path, output);
                    imp::push_scoped(&mut g, pm, "", path, &text, Src::Zencoder, model, id, ts);
                }
            }
        }
    }
    n::session(&mut g, pm, Src::Zencoder, id, "", start, end, prompts, 0);
    Ok(g)
}
fn ide(
    doc: &Value,
    pm: &PathMap,
    native_calls: &HashSet<(String, String)>,
) -> Result<FileGroup, String> {
    let id = doc["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Zencoder chat has no id")?;
    let messages = doc["messages"]
        .as_array()
        .ok_or("Zencoder chat has no messages")?;
    let mut g = FileGroup::default();
    let (start, mut end) = (crate::any_ts(&doc["createdAt"]).unwrap_or(0), 0);
    let mut seen = HashSet::new();
    // Other runtimes have their own authoritative native readers.
    if !matches!(
        doc["agentsRuntime"].as_str(),
        Some("zen-cli" | "zencoder-cloud")
    ) {
        return Ok(g);
    }
    for message in messages
        .iter()
        .filter(|m| m["role"] == "assistant" && m["isGhost"] != true && m["isInternal"] != true)
    {
        let ts = crate::any_ts(&message["createdAt"]).unwrap_or(0);
        end = end.max(ts);
        for call in message["content"].as_array().into_iter().flatten() {
            let cid = n::text(&call["toolCallId"]);
            if native_calls.contains(&(
                n::text(&message["context"]["cliSessionId"]).into(),
                cid.into(),
            )) {
                continue;
            }
            if call["type"] != "cliToolCall"
                || call["status"] != "SUCCESS"
                || !matches!(call["toolName"].as_str(), Some("write" | "edit"))
                || cid.is_empty()
                || !seen.insert(cid)
            {
                continue;
            }
            let path = n::text(&call["input"]["path"]);
            let text = actual_diff(path, &call["output"]);
            // IDE serialization drops per-message model details. The mutable
            // chat.model selection is not evidence of the model that wrote this.
            imp::push_scoped(&mut g, pm, "", path, &text, Src::Zencoder, "", id, ts);
        }
    }
    n::session(&mut g, pm, Src::Zencoder, id, "", start, end, 0, 0);
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let files = files();
    let mut logs = HashMap::new();
    let mut ids = HashSet::new();
    let mut owners = HashMap::new();
    let mut ordered = files.clone();
    ordered.sort_by_key(|p| {
        (
            std::fs::metadata(p).and_then(|m| m.modified()).ok(),
            p.clone(),
        )
    });
    for p in ordered
        .iter()
        .filter(|p| p.extension().is_some_and(|s| s == "jsonl"))
    {
        let rows = n::jsonl(p);
        if let Ok(rows) = &rows {
            // Invalid or incomplete sessions must not suppress their IDE record.
            if native(rows, pm, None).is_ok() {
                let calls: HashSet<_> = rows[1..]
                    .iter()
                    .filter(|m| m["role"] == "assistant")
                    .flat_map(|m| m["content"].as_array().into_iter().flatten())
                    .filter(|p| p["type"] == "tool-call")
                    .map(|p| n::text(&p["toolCallId"]))
                    .collect();
                for m in &rows[1..] {
                    for part in m["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|v| v["type"] == "tool-result")
                    {
                        let cid = n::text(&part["toolCallId"]);
                        if calls.contains(cid) {
                            ids.insert((n::text(&rows[0]["id"]).into(), cid.into()));
                            owners.insert(cid.to_string(), p.clone());
                        }
                    }
                }
            }
        }
        logs.insert(p.clone(), rows);
    }
    n::scan(Src::Zencoder, store, pm, st, files, |p, pm| {
        if let Some(rows) = logs.get(p) {
            let owned = owners
                .iter()
                .filter(|(_, owner)| owner.as_path() == p)
                .map(|(id, _)| id.clone())
                .collect();
            native(rows.as_ref().map_err(Clone::clone)?, pm, Some(&owned))
        } else {
            ide(&n::json(p)?, pm, &ids)
        }
    });
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Zencoder, &root(), "sessions/*.jsonl + IDE zencoder-chat/sessions/*.json", files().len(), "Paired native Write/Edit results with actual post-write diffs and per-message model; shared calls across copied/forked sessions count once. IDE SUCCESS output diffs are a fallback with unknown model; preview hunks, encrypted history and hosted Claude/Codex are excluded.", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_uses_actual_result_not_proposed_or_mutable_model() {
        let mut rows = vec![
            json!({"id":"s","version":"1.0","modelId":"mutable","createdAt":"2026-10-23T00:00:00Z"}),
            json!({"role":"assistant","modelDetails":{"model":{"id":"actual"}},"createdAt":"2026-10-23T00:00:01Z","content":[{"type":"tool-call","toolCallId":"c","toolName":"Edit","input":{"file_path":"/demo/src/main.rs","new_string":"PROPOSAL"}}]}),
            json!({"role":"tool","content":[{"type":"tool-result","toolCallId":"c","toolName":"Edit","isError":false,"content":[{"type":"diff","path":"/demo/src/main.rs","diff":"--- main.rs\n+++ main.rs\n@@ -1 +1 @@\n-old\n+fn saved() {}"}]}]}),
        ];
        let g = native(&rows, &n::project(), None).unwrap();
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "actual");
        rows[2]["content"][0]["isError"] = json!(true);
        assert!(native(&rows, &n::project(), None).unwrap().evs.is_empty());
    }
    #[test]
    fn ide_requires_final_diff_and_does_not_repeat_native_session() {
        let mut doc = json!({"id":"ide","agentsRuntime":"zen-cli","model":"mutable","messages":[{"role":"assistant","context":{"cliSessionId":"raw"},"content":[{"type":"codeHunk","lineBasedDiff":"+preview"},{"type":"cliToolCall","toolCallId":"c","toolName":"write","status":"IN_PROGRESS","input":{"path":"/demo/src/main.rs"},"output":{"type":"diff","path":"/demo/src/main.rs","diff":"@@ -0,0 +1 @@\n+fn saved() {}"}}]}]});
        assert!(ide(&doc, &n::project(), &HashSet::new())
            .unwrap()
            .evs
            .is_empty());
        doc["messages"][0]["content"][1]["status"] = json!("SUCCESS");
        let g = ide(&doc, &n::project(), &HashSet::new()).unwrap();
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert!(ide(
            &doc,
            &n::project(),
            &HashSet::from([("raw".into(), "c".into())])
        )
        .unwrap()
        .evs
        .is_empty());
        // A header-only/pending native call cannot hide another completed IDE edit.
        assert_eq!(
            ide(
                &doc,
                &n::project(),
                &HashSet::from([("raw".into(), "another-call".into())])
            )
            .unwrap()
            .evs
            .len(),
            1
        );
    }
}
