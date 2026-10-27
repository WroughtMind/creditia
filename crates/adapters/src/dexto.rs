//! Dexto's native SQLite kv_store/list_store (database/<agent>.db).
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    crate::source_dir(Src::Dexto)
}
fn paths(projects: &[String]) -> Vec<PathBuf> {
    let mut paths = n::sqlite_files(&root().join("database"));
    if root().is_file() {
        paths.push(root());
    }
    for p in projects {
        paths.extend(n::sqlite_files(&PathBuf::from(p).join(".dexto/database")));
    }
    paths.sort();
    paths.dedup();
    paths
}
fn load(path: &Path) -> Result<Vec<(Value, String, Vec<Value>)>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut query = db
        .prepare("select value from kv_store where key like 'session:%'")
        .map_err(|e| e.to_string())?;
    let values = query
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for value in values {
        let s: Value =
            serde_json::from_str(&value.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let workspace = format!("workspace:item:{}", n::text(&s["workspaceId"]));
        let cwd = db
            .query_row("select value from kv_store where key=?", [workspace], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|x| serde_json::from_str::<Value>(&x).ok())
            .and_then(|x| x["path"].as_str().map(str::to_string))
            .unwrap_or_default();
        let mut q = db
            .prepare("select value from list_store where key=? order by sequence")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([format!("messages:{}", n::text(&s["id"]))], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|e| e.to_string())?;
        let rows = rows
            .map(|r| {
                r.map_err(|e| e.to_string())
                    .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            })
            .collect::<Result<Vec<Value>, String>>()?;
        out.push((s, cwd, rows));
    }
    Ok(out)
}
fn parse(s: &Value, cwd: &str, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let id = n::text(&s["id"]);
    if id.is_empty() {
        return;
    }
    let start = crate::any_ts(&s["createdAt"]).unwrap_or(0);
    let mut end = start;
    let mut prompts = 0;
    let before = g.evs.len();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    for row in rows {
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(0);
        end = end.max(ts);
        if s["parentSessionId"].is_string() && (ts == 0 || ts < start) {
            continue;
        }
        if row["role"] == "user" {
            prompts += 1;
        }
        if row["role"] == "assistant" {
            for call in row["toolCalls"].as_array().into_iter().flatten() {
                let cid = n::text(&call["id"]);
                if !cid.is_empty() {
                    calls.insert(cid, (call, n::text(&row["model"])));
                }
            }
        }
        if row["role"] != "tool" || row["success"] != true || row["approvalStatus"] == "rejected" {
            continue;
        }
        let cid = n::text(&row["toolCallId"]);
        let Some((call, model)) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) || call["function"]["name"] != row["name"] {
            continue;
        }
        let Ok(input) = serde_json::from_str::<Value>(n::text(&call["function"]["arguments"]))
        else {
            continue;
        };
        let result = row["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| v["type"] == "text")
            .find_map(|v| serde_json::from_str::<Value>(n::text(&v["text"])).ok());
        let Some(result) = result.filter(|v| v["success"] == true) else {
            continue;
        };
        let path = n::text(&result["path"]);
        if path.is_empty() || result["path"] != input["file_path"] {
            continue;
        }
        let text = match n::text(&row["name"]) {
            "write_file"
                if input["encoding"].is_null()
                    || input["encoding"] == "utf-8"
                    || input["encoding"] == "utf8" =>
            {
                n::text(&input["content"]).into()
            }
            "edit_file" if result["changes_count"].as_i64().is_some_and(|n| n > 0) => {
                let new = n::text(&input["new_string"]);
                // JS String.replace expands $ substitutions unless replace_all uses split/join.
                if input["replace_all"] != true && new.contains('$') {
                    continue;
                }
                crate::added_lines(n::text(&input["old_string"]), new)
            }
            _ => continue,
        };
        imp::push_scoped(g, pm, cwd, path, &text, Src::Dexto, model, id, ts);
    }
    n::session(g, pm, Src::Dexto, id, cwd, start, end, prompts, before);
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    for (s, cwd, rows) in load(path)? {
        parse(&s, &cwd, &rows, pm, &mut g);
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    n::scan(Src::Dexto, store, pm, st, paths(projects), read);
}
pub fn discover() -> Vec<Activity> {
    paths(&[])
        .iter()
        .flat_map(|p| load(p).unwrap_or_default())
        .filter(|(_, cwd, _)| !cwd.is_empty())
        .map(|(s, cwd, _)| {
            imp::activity(
                Src::Dexto,
                cwd,
                None,
                crate::any_ts(&s["lastActivity"]).unwrap_or(0),
            )
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Dexto,&root(),"native SQLite kv_store/list_store",paths(&[]).len(),"Paired successful write_file/edit_file results; workspaceId resolves relative paths. PostgreSQL/in-memory histories are not local SQLite.",false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn success_receipt_model_and_failed_fork() {
        let s = json!({"id":"s","createdAt":1790000000000i64});
        let mut rows = vec![
            json!({"role":"assistant","model":"gpt-test","toolCalls":[{"id":"c","function":{"name":"edit_file","arguments":"{\"file_path\":\"src/main.rs\",\"old_string\":\"fn old() {}\",\"new_string\":\"fn new() {}\"}"}}]}),
            json!({"role":"tool","name":"edit_file","toolCallId":"c","success":true,"timestamp":1790000001000i64,"content":[{"type":"text","text":"{\"success\":true,\"path\":\"src/main.rs\",\"changes_count\":1}"}]}),
        ];
        let mut g = FileGroup::default();
        parse(&s, "/demo", &rows, &n::project(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "gpt-test");
        rows[1]["success"] = json!(false);
        let mut g = FileGroup::default();
        parse(&s, "/demo", &rows, &n::project(), &mut g);
        assert!(g.evs.is_empty());
    }
}
