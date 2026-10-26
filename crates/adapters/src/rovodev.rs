//! Rovo Dev session_context.json, checked against native samples and the
//! bundled Nautilus file tools. See SUPPORT_AGENT6.md for the exact sources.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    std::env::var_os("CREDITIA_ROVODEV_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".rovodev/sessions"))
}
fn paths(projects: &[String]) -> Vec<PathBuf> {
    let mut roots = vec![root()];
    if std::env::var_os("CREDITIA_ROVODEV_DIR").is_none() {
        roots.extend(
            projects
                .iter()
                .map(|p| Path::new(p).join(".rovodev/sessions")),
        );
    }
    let mut files: Vec<_> = roots
        .iter()
        .flat_map(|r| {
            n::files(r, |p| {
                p.file_name().is_some_and(|s| s == "session_context.json")
            })
        })
        .collect();
    files.sort();
    files.dedup();
    files
}
fn absolute(path: &str) -> bool {
    path.starts_with('/')
        || (path.as_bytes().get(1) == Some(&b':')
            && matches!(path.as_bytes().get(2), Some(b'/' | b'\\')))
}
fn read(path: &Path) -> Result<Value, String> {
    let v = n::json(path)?;
    if n::text(&v["id"]).is_empty()
        || !absolute(n::text(&v["workspace_path"]))
        || !v["message_history"].is_array()
    {
        return Err(format!(
            "{}: expected native id, absolute workspace_path and message_history",
            path.display()
        ));
    }
    for row in v["message_history"].as_array().unwrap() {
        if !matches!(n::text(&row["kind"]), "request" | "response") || !row["parts"].is_array() {
            return Err(format!("{}: invalid native message", path.display()));
        }
        for part in row["parts"].as_array().unwrap() {
            if !matches!(
                n::text(&part["tool_name"]),
                "create_file" | "find_and_replace_code"
            ) {
                continue;
            }
            if row["kind"] == "response" && part["part_kind"] == "tool-call" {
                let args = arguments(part)
                    .ok_or_else(|| format!("{}: malformed tool arguments", path.display()))?;
                if !args["file_path"].is_string()
                    || (part["tool_name"] == "create_file"
                        && !args["initial_content"].is_null()
                        && !args["initial_content"].is_string())
                {
                    return Err(format!("{}: invalid file tool arguments", path.display()));
                }
            } else if row["kind"] == "request"
                && part["part_kind"] == "tool-return"
                && !part["content"].is_string()
            {
                return Err(format!("{}: malformed file tool receipt", path.display()));
            }
        }
    }
    Ok(v)
}

#[derive(Clone)]
struct Receipt {
    call: Value,
    result: Value,
    model: String,
    ts: i64,
}
fn arguments(call: &Value) -> Option<Value> {
    let args = if let Some(s) = call["args"].as_str() {
        serde_json::from_str(s).ok()?
    } else {
        call["args"].clone()
    };
    args.is_object().then_some(args)
}
fn receipts(v: &Value) -> HashMap<String, Receipt> {
    let mut calls = HashMap::new();
    let mut results = HashMap::new();
    for row in v["message_history"].as_array().into_iter().flatten() {
        for part in row["parts"].as_array().into_iter().flatten() {
            let id = n::text(&part["tool_call_id"]);
            if id.is_empty() {
                continue;
            }
            if row["kind"] == "response" && part["part_kind"] == "tool-call" {
                calls.insert(
                    id.to_string(),
                    (part.clone(), n::text(&row["model_name"]).to_string()),
                );
                results.remove(id);
            } else if row["kind"] == "request"
                && matches!(n::text(&part["part_kind"]), "tool-return" | "retry-prompt")
            {
                if let Some((call, model)) = calls.get(id) {
                    if matches!(
                        n::text(&call["tool_name"]),
                        "create_file" | "find_and_replace_code"
                    ) && part["tool_name"] == call["tool_name"]
                    {
                        results.insert(
                            id.to_string(),
                            Receipt {
                                call: call.clone(),
                                result: part.clone(),
                                model: model.clone(),
                                ts: crate::any_ts(&part["timestamp"]).unwrap_or(0),
                            },
                        );
                    }
                }
            }
        }
    }
    results
}
fn edit(r: &Receipt) -> Option<(String, String)> {
    if r.result["part_kind"] != "tool-return" || r.result["tool_name"] != r.call["tool_name"] {
        return None;
    }
    let args = arguments(&r.call)?;
    let path = args["file_path"].as_str()?;
    if path.is_empty() || path.contains(['\n', '\r', '\0']) {
        return None;
    }
    let content = r.result["content"].as_str()?;
    let text = match n::text(&r.call["tool_name"]) {
        "create_file" => {
            if content != format!("Successfully created {path}.")
                && !(args["overwrite"] == true
                    && content == format!("Successfully updated {path}."))
            {
                return None;
            }
            // Nautilus strips this sentinel before writing the content.
            let text = args["initial_content"].as_str().unwrap_or("");
            text.strip_prefix("!U!SHALL!NOT!PARSE!")
                .unwrap_or(text)
                .to_string()
        }
        "find_and_replace_code" => {
            // Nautilus may fix indentation/entities. Only its post-write diff
            // describes the actual edit; the proposed replacement is not used.
            let diff =
                content.strip_prefix(&format!("Successfully replaced code in {path}:\n\n"))?;
            if !diff.starts_with(&format!("--- {path}\n+++ {path}\n@@ ")) {
                return None;
            }
            crate::added_from_diff(diff)
        }
        _ => return None,
    };
    Some((path.to_string(), text))
}

fn group(v: &Value, pm: &PathMap, owned: impl Fn(&str) -> bool) -> FileGroup {
    let mut g = FileGroup::default();
    let cwd = n::text(&v["workspace_path"]);
    if !pm.in_project(cwd) {
        return g;
    }
    let mut start = crate::any_ts(&v["timestamp"]).unwrap_or(0);
    let mut end = start;
    for (cid, r) in receipts(v) {
        if !owned(&cid) {
            continue;
        }
        // Call IDs survive forks and pruning. Reconcile only an observed
        // terminal call, never the absence of history after native /prune.
        // User session metadata keeps the native session ID separately.
        let evidence_id = format!("tool:{cid}");
        if r.ts > 0 {
            start = if start > 0 { start.min(r.ts) } else { r.ts };
            end = end.max(r.ts);
        }
        if let Some((path, text)) = edit(&r) {
            imp::push_scoped(
                &mut g,
                pm,
                cwd,
                &path,
                &text,
                Src::Rovodev,
                &r.model,
                &evidence_id,
                r.ts,
            );
        }
        g.reconciled_sessions.push((Src::Rovodev, evidence_id));
    }
    let prompts = v["prompts"].as_array().map(|a| a.len()).unwrap_or_else(|| {
        v["message_history"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| m["parts"].as_array().into_iter().flatten())
            .filter(|p| p["part_kind"] == "user-prompt")
            .count()
    });
    g.sessions.push(imp::session(
        Src::Rovodev,
        n::text(&v["id"]),
        "",
        start,
        end,
        prompts as i64,
        !g.evs.is_empty(),
    ));
    g
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    let paths = paths(projects);
    let parsed: Vec<_> = paths.iter().map(|p| read(p)).collect();
    let mut session_owners = HashMap::new();
    let mut owners = HashMap::new();
    for (i, result) in parsed.iter().enumerate() {
        if let Ok(v) = result {
            let rank = (
                std::fs::metadata(&paths[i]).and_then(|m| m.modified()).ok(),
                i,
            );
            let best = session_owners
                .entry(n::text(&v["id"]).to_string())
                .or_insert(rank);
            if rank > *best {
                *best = rank;
            }
        }
    }
    for (i, result) in parsed.iter().enumerate() {
        if let Ok(v) = result {
            if session_owners[n::text(&v["id"])].1 != i
                || !pm.in_project(n::text(&v["workspace_path"]))
            {
                continue;
            }
            for cid in receipts(v).into_keys() {
                let rank = session_owners[n::text(&v["id"])];
                let best = owners.entry(cid).or_insert(rank);
                if rank > *best {
                    *best = rank;
                }
            }
        }
    }
    n::scan(Src::Rovodev, store, pm, st, paths.clone(), |p, pm| {
        let i = paths
            .iter()
            .position(|v| v == p)
            .ok_or("missing Rovo Dev history")?;
        let v = parsed[i].as_ref().map_err(Clone::clone)?;
        if session_owners[n::text(&v["id"])].1 != i {
            return Ok(FileGroup::default());
        }
        Ok(group(v, pm, |id| owners.get(id).is_some_and(|o| o.1 == i)))
    });
}
pub fn discover() -> Vec<Activity> {
    paths(&[])
        .iter()
        .filter_map(|p| {
            let v = read(p).ok()?;
            Some(imp::activity(
                Src::Rovodev,
                n::text(&v["workspace_path"]).into(),
                None,
                crate::any_ts(&v["timestamp"]).unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Rovodev, &root(), "native session_context.json + Nautilus receipts", paths(&[]).len(), "Completed create_file and actual find_and_replace_code result diffs; paired calls, per-response model, fork call-ID deduplication. Pruned/missing calls retain prior evidence. Shell, unpaired calls and proposed replacements are excluded.", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actual_diff_and_sentinel_are_used() {
        let mut r = Receipt {
            call: json!({"tool_name":"create_file","args":{"file_path":"src/a.rs","initial_content":"!U!SHALL!NOT!PARSE!fn actual() {}"}}),
            result: json!({"part_kind":"tool-return","tool_name":"create_file","content":"Successfully created src/a.rs."}),
            model: "".into(),
            ts: 1,
        };
        assert_eq!(edit(&r).unwrap().1, "fn actual() {}");
        r.call = json!({"tool_name":"find_and_replace_code","args":{"file_path":"src/a.rs","replace":"proposal"}});
        r.result = json!({"part_kind":"tool-return","tool_name":"find_and_replace_code","content":"Successfully replaced code in src/a.rs:\n\n--- src/a.rs\n+++ src/a.rs\n@@ -1 +1 @@\n-old\n+actual\n"});
        assert_eq!(edit(&r).unwrap().1, "actual\n");
        r.result["content"] = json!("Error: replacement failed");
        assert!(edit(&r).is_none());
    }
}
