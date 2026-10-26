//! AiderDesk v2 context.json + settings.json. Source references: SUPPORT_AGENT31.md.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn roots(projects: &[String]) -> Vec<PathBuf> {
    if let Some(root) = std::env::var_os("CREDITIA_AIDER_DESK_DIR") {
        return vec![root.into()];
    }
    let dir = std::env::var("AIDER_DESK_DIR").unwrap_or_else(|_| ".aider-desk".into());
    projects
        .iter()
        .map(|p| PathBuf::from(p).join(&dir).join("tasks"))
        .collect()
}
fn paths(projects: &[String]) -> Vec<PathBuf> {
    let mut out: Vec<_> = roots(projects)
        .iter()
        .flat_map(|p| n::files(p, |f| f.file_name().is_some_and(|n| n == "context.json")))
        .collect();
    out.sort();
    out.dedup();
    out
}
fn location(path: &Path) -> Result<(String, String), String> {
    let task_dir = path.parent().ok_or("missing task directory")?;
    let settings = n::json(&task_dir.join("settings.json"))?;
    // settings are mutable: never use their current model for historical edits.
    let project = task_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("missing project directory")?;
    let cwd = settings["worktree"]["path"]
        .as_str()
        .unwrap_or_else(|| project.to_str().unwrap_or(""));
    let id = task_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    Ok((id.into(), cwd.into()))
}

fn parse(value: &Value, id: &str, cwd: &str, pm: &PathMap) -> Result<FileGroup, String> {
    if value["version"] != 2 {
        return Err("unsupported AiderDesk context version".into());
    }
    let rows = value["contextMessages"]
        .as_array()
        .ok_or("missing contextMessages")?;
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut start = i64::MAX;
    let mut end = 0;
    let mut prompts = 0;
    for row in rows {
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(0);
        if ts > 0 {
            start = start.min(ts);
            end = end.max(ts);
        }
        if row["role"] == "user" {
            prompts += 1;
        }
        let model = n::text(&row["usageReport"]["model"]);
        // Aider mode supplies a completed, actual file diff on its assistant message.
        if row["role"] == "assistant" && row["commitHash"].as_str().is_some_and(|x| !x.is_empty()) {
            for (path, added) in imp::diff_files(n::text(&row["diff"])) {
                imp::push_scoped(
                    &mut g,
                    pm,
                    cwd,
                    &path,
                    &added,
                    Src::AiderDesk,
                    model,
                    id,
                    ts,
                );
            }
        }
        for part in row["content"].as_array().into_iter().flatten() {
            let call_id = n::text(&part["toolCallId"]);
            if call_id.is_empty() {
                continue;
            }
            if row["role"] == "assistant" && part["type"] == "tool-call" {
                calls.insert(call_id, (part, model, ts));
            } else if row["role"] == "tool" && part["type"] == "tool-result" {
                let Some((call, model, call_ts)) = calls.remove(call_id) else {
                    continue;
                };
                if !seen.insert(call_id)
                    || part["toolName"] != call["toolName"]
                    || part["output"]["type"] != "text"
                {
                    continue;
                }
                let input = &call["input"];
                let path = n::text(&input["filePath"]);
                let receipt = n::text(&part["output"]["value"]);
                let added = match n::text(&call["toolName"]) {
                    "power---file_write"
                        if [
                            format!("Successfully created '{path}'."),
                            format!("Successfully appended to '{path}'."),
                            format!("Successfully written to '{path}' (overwritten)."),
                        ]
                        .contains(&receipt.to_string()) =>
                    {
                        n::text(&input["content"]).to_string()
                    }
                    "power---file_edit"
                        if receipt == format!("Successfully edited '{path}'.")
                            && input["isRegex"] != true =>
                    {
                        let old = n::text(&input["searchTerm"]);
                        let new = n::text(&input["replacementText"]);
                        // Escaped/regex replacement needs the original file to reproduce sanitization.
                        // The ordinary literal path is exact; ambiguous replacements remain sessions.
                        if old.contains('\\') || new.contains('\\') {
                            continue;
                        }
                        crate::added_lines(old, new)
                    }
                    _ => continue,
                };
                imp::push_scoped(
                    &mut g,
                    pm,
                    cwd,
                    path,
                    &added,
                    Src::AiderDesk,
                    model,
                    id,
                    if ts > 0 { ts } else { call_ts },
                );
            }
        }
    }
    n::session(
        &mut g,
        pm,
        Src::AiderDesk,
        id,
        cwd,
        if start == i64::MAX { 0 } else { start },
        end,
        prompts,
        0,
    );
    Ok(g)
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let (id, cwd) = location(path)?;
    parse(&n::json(path)?, &id, &cwd, pm)
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    n::scan(Src::AiderDesk, store, pm, st, paths(projects), read);
}
pub fn discover() -> Vec<Activity> {
    paths(&[])
        .iter()
        .filter_map(|p| {
            let (_, cwd) = location(p).ok()?;
            Some(imp::activity(
                Src::AiderDesk,
                cwd,
                None,
                i64::try_from(creditia_core::model::fingerprint(p).0).unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::AiderDesk, &crate::source_dir(Src::AiderDesk), "native context v2 + task settings", paths(&[]).len(), "Project-local .aider-desk/tasks are scanned with the selected repository; successful Power writes/literal edits and committed Aider diffs. Regex/escaped edits require actual diff evidence.", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_paired_successful_project_writes() {
        let mut doc = json!({"version":2,"contextMessages":[
          {"role":"assistant","timestamp":1790000000000i64,"usageReport":{"model":"claude-test"},"content":[{"type":"tool-call","toolCallId":"one","toolName":"power---file_write","input":{"filePath":"src/main.rs","content":"fn confirmed() {}"}}]},
          {"role":"tool","timestamp":1790000001000i64,"content":[{"type":"tool-result","toolCallId":"one","toolName":"power---file_write","output":{"type":"text","value":"Successfully created 'src/main.rs'."}}]}
        ]});
        let g = parse(&doc, "task", "/demo", &n::project()).unwrap();
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "claude-test");
        assert!(parse(&doc, "task", "/other", &n::project())
            .unwrap()
            .evs
            .is_empty());
        doc["contextMessages"][1]["content"][0]["output"]["value"] =
            json!("File write denied by user");
        assert!(parse(&doc, "task", "/demo", &n::project())
            .unwrap()
            .evs
            .is_empty());
        doc["version"] = json!(3);
        assert!(parse(&doc, "task", "/demo", &n::project()).is_err());
    }
}
