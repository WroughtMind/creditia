//! Pochi LiveStore materialized SQLite tasks/messages. No event-log or preview evidence.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    crate::source_dir(Src::Pochi)
}
fn paths() -> Vec<PathBuf> {
    n::sqlite_files(&root())
}
fn load(path: &Path) -> Result<Vec<(Value, Vec<Value>)>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut q=db.prepare("select json_object('id',id,'cwd',cwd,'parentId',parentId,'createdAt',createdAt,'updatedAt',updatedAt) from tasks").map_err(|e|e.to_string())?;
    let tasks = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for task in tasks {
        let task: Value =
            serde_json::from_str(&task.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut q = db
            .prepare("select data from messages where taskId=? order by rowid")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([n::text(&task["id"])], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let rows = rows
            .map(|r| {
                r.map_err(|e| e.to_string())
                    .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            })
            .collect::<Result<Vec<Value>, String>>()?;
        out.push((task, rows));
    }
    Ok(out)
}
fn cleaned(mut s: &str) -> String {
    for trim in ["\\\n", "\\"] {
        s = s.strip_prefix(trim).unwrap_or(s);
        s = s.strip_suffix(trim).unwrap_or(s);
    }
    for wrap in ["```", "'''", "\"\"\""] {
        if s.len() >= wrap.len() * 2 && s.starts_with(wrap) && s.ends_with(wrap) {
            s = &s[wrap.len()..s.len() - wrap.len()];
        }
    }
    s.into()
}
fn parse(task: &Value, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let id = n::text(&task["id"]);
    let cwd = n::text(&task["cwd"]);
    let start = crate::any_ts(&task["createdAt"]).unwrap_or(0);
    let before = g.evs.len();
    let mut seen = HashSet::new();
    let mut prompts = 0;
    let mut end = start;
    for row in rows {
        let ts = crate::any_ts(&row["metadata"]["finishedAt"])
            .or_else(|| crate::any_ts(&row["metadata"]["startedAt"]))
            .unwrap_or(0);
        end = end.max(ts);
        if row["role"] == "user" {
            prompts += 1;
        }
        if row["role"] != "assistant" {
            continue;
        }
        // Forked tasks can contain old messages; no original-time receipt means no new edit.
        if ts == 0 || ts < start {
            continue;
        }
        for part in row["parts"].as_array().into_iter().flatten() {
            if part["state"] != "output-available" || part["output"]["success"] != true {
                continue;
            }
            let cid = n::text(&part["toolCallId"]);
            if cid.is_empty() || !seen.insert(cid) {
                continue;
            }
            let name = n::text(&part["type"]);
            if !matches!(
                name,
                "tool-writeToFile" | "tool-applyDiff" | "tool-multiApplyDiff"
            ) {
                continue;
            }
            let path = n::text(&part["input"]["path"]);
            let result = &part["output"];
            if let Some(diff) = result["_meta"]["edit"].as_str() {
                for (_, added) in imp::diff_files(diff) {
                    imp::push_scoped(g, pm, cwd, path, &added, Src::Pochi, "", id, ts);
                }
            } else if result["autoFormattingEdits"].is_null() {
                let input = &part["input"];
                let added = match name {
                    "tool-writeToFile" => cleaned(n::text(&input["content"])),
                    "tool-applyDiff" => crate::added_lines(
                        n::text(&input["searchContent"]),
                        n::text(&input["replaceContent"]),
                    ),
                    _ => continue,
                };
                imp::push_scoped(g, pm, cwd, path, &added, Src::Pochi, "", id, ts);
            }
        }
    }
    // Current task.modelId is mutable; native message metadata has no model field.
    n::session(g, pm, Src::Pochi, id, cwd, start, end, prompts, before);
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    for (task, rows) in load(path)? {
        parse(&task, &rows, pm, &mut g);
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Pochi, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    paths()
        .iter()
        .flat_map(|p| load(p).unwrap_or_default())
        .filter(|(s, _)| s["cwd"].is_string())
        .map(|(s, _)| {
            imp::activity(
                Src::Pochi,
                n::text(&s["cwd"]).into(),
                None,
                crate::any_ts(&s["updatedAt"]).unwrap_or(0),
            )
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Pochi,&root(),"native LiveStore SQLite tasks/messages",paths().len(),"Persistent CLI storage or copied materialized SQLite; CLI without POCHI_LIVEKIT_SYNC_ON is in-memory. VS Code OPFS requires a database export. Historical model remains unknown.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn success_and_saved_diff_override_proposal() {
        let task = json!({"id":"p","cwd":"/demo","createdAt":1790000000000i64});
        let mut rows = vec![
            json!({"role":"assistant","metadata":{"finishedAt":1790000001000i64},"parts":[{"type":"tool-writeToFile","state":"output-available","toolCallId":"c","input":{"path":"src/main.rs","content":"fn proposal() {}"},"output":{"success":true,"_meta":{"edit":"--- src/main.rs\n+++ src/main.rs\n@@ -0,0 +1 @@\n+fn saved() {}\n"}}}]}),
        ];
        let mut g = FileGroup::default();
        parse(&task, &rows, &n::project(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert!(g.evs[0].model.is_empty());
        rows[0]["parts"][0]["state"] = json!("input-available");
        let mut g = FileGroup::default();
        parse(&task, &rows, &n::project(), &mut g);
        assert!(g.evs.is_empty());
        assert_eq!(cleaned("\\\n```fn saved() {}```"), "fn saved() {}");
    }
}
