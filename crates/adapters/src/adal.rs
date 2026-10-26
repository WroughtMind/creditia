//! AdaL's documented headless JSON / NDJSON exports, not its unpublished HTML store.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn paths(projects: &[String]) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    let accept = |p: &Path| {
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("json" | "jsonl" | "ndjson")
        )
    };
    if let Some(root) = std::env::var_os("CREDITIA_ADAL_DIR") {
        out.extend(
            n::files(Path::new(&root), accept)
                .into_iter()
                .map(|p| (p, String::new())),
        );
    }
    for root in projects {
        out.extend(
            n::files(&Path::new(root).join(".creditia/imports/adal"), accept)
                .into_iter()
                .map(|p| (p, root.clone())),
        );
    }
    out.sort();
    out.dedup();
    out
}
fn parse(rows: &[Value], cwd: &str, pm: &PathMap) -> Result<FileGroup, String> {
    let mut calls: HashMap<String, Value> = HashMap::new();
    let (id, finished) = if rows.len() == 1 && rows[0]["tool_calls"].is_array() {
        let doc = &rows[0];
        for c in doc["tool_calls"].as_array().unwrap() {
            calls.insert(n::text(&c["id"]).into(), c.clone());
        }
        (n::text(&doc["session_id"]), doc["exit_code"].is_number())
    } else {
        for row in rows {
            let id = n::text(&row["id"]);
            match n::text(&row["type"]) {
                "tool_call" => {
                    calls.insert(id.into(), row.clone());
                }
                "tool_result" => {
                    if let Some(c) = calls.get_mut(id).filter(|c| c["name"] == row["name"]) {
                        c["status"] = row["status"].clone();
                        c["output"] = row["output"].clone();
                    }
                }
                "permission_denied" => {
                    calls.remove(id);
                }
                _ => {}
            }
        }
        let end = rows.last().ok_or("empty AdaL export")?;
        (
            n::text(&end["session_id"]),
            end["type"] == "complete" && end["exit_code"].is_number(),
        )
    };
    if id.is_empty() || !finished {
        return Err("AdaL export has no complete session envelope".into());
    }
    let mut g = FileGroup::default();
    let mut seen = HashSet::new();
    for (cid, c) in calls {
        if cid.is_empty()
            || !seen.insert(cid)
            || c["status"] != "completed"
            || c["name"] != "replace_by_string"
        {
            continue;
        }
        let args = &c["args"];
        let path = n::text(&args["file_path"]);
        if path.is_empty()
            || c["output"] != format!("Edited {path}")
            || !args["old_string"].is_string()
            || !args["new_string"].is_string()
        {
            continue;
        }
        if cwd.is_empty() && !Path::new(path).is_absolute() && path.get(1..2) != Some(":") {
            continue;
        }
        let added = crate::added_lines(n::text(&args["old_string"]), n::text(&args["new_string"]));
        // The final run's model is not a per-call model; exports contain no edit timestamps.
        imp::push_scoped(&mut g, pm, cwd, path, &added, Src::Adal, "", id, 0);
    }
    n::session(&mut g, pm, Src::Adal, id, cwd, 0, 0, 0, 0);
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    let files = paths(projects);
    let cwd: HashMap<_, _> = files.iter().cloned().collect();
    n::scan(
        Src::Adal,
        store,
        pm,
        st,
        files.into_iter().map(|(p, _)| p).collect(),
        |p, pm| {
            let rows = if p.extension().is_some_and(|s| s == "json") {
                vec![n::json(p)?]
            } else {
                n::jsonl(p)?
            };
            parse(&rows, cwd.get(p).map(String::as_str).unwrap_or(""), pm)
        },
    );
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Adal, Path::new(".creditia/imports/adal"), "official headless JSON / stream-json exports", paths(&[]).len(), "Completed replace_by_string with exact Edited receipt. Put native exports under the target repository's .creditia/imports/adal, or use CREDITIA_ADAL_DIR with absolute paths. HTML history, other tools, per-call model and timestamps remain unverified.", false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn denied_and_incomplete_streams_are_not_edits() {
        let mut rows = vec![
            json!({"type":"tool_call","id":"c","name":"replace_by_string","args":{"file_path":"src/main.rs","old_string":"old","new_string":"fn saved() {}"}}),
            json!({"type":"tool_result","id":"c","name":"replace_by_string","status":"completed","output":"Edited src/main.rs"}),
            json!({"type":"complete","exit_code":0,"session_id":"s","model":"final-model"}),
        ];
        let g = parse(&rows, "/demo", &n::project()).unwrap();
        assert_eq!(g.evs.len(), 1);
        assert!(g.evs[0].model.is_empty());
        rows[1]["status"] = json!("denied");
        assert!(parse(&rows, "/demo", &n::project()).unwrap().evs.is_empty());
        rows.pop();
        assert!(parse(&rows, "/demo", &n::project()).is_err());
    }
}
