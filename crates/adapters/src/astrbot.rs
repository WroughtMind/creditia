//! AstrBot data_v4.db conversations.content. Only native filesystem tool success receipts.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_ASTRBOT_DIR")
        .or_else(|| std::env::var_os("ASTRBOT_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".astrbot"))
}
fn paths(projects: &[String]) -> Vec<PathBuf> {
    let root = root();
    if root.is_file() {
        return vec![root];
    }
    let mut p = vec![root.join("data/data_v4.db")];
    for r in projects {
        p.push(Path::new(r).join("data/data_v4.db"));
    }
    p.retain(|p| p.is_file());
    p.sort();
    p.dedup();
    p
}
fn parse(id: &str, ts: i64, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let before = g.evs.len();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut prompts = 0;
    for r in rows {
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
        if r["role"] != "tool" {
            continue;
        }
        let cid = n::text(&r["tool_call_id"]);
        let Some(c) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) {
            continue;
        }
        let args: Value =
            serde_json::from_str(n::text(&c["function"]["arguments"])).unwrap_or(Value::Null);
        let output = n::text(&r["content"]);
        let (path, added) = match n::text(&c["function"]["name"]) {
            "astrbot_file_write_tool" => {
                let Some(path) = output.strip_prefix("File written successfully: ") else {
                    continue;
                };
                (path, n::text(&args["content"]).to_string())
            }
            "astrbot_file_edit_tool" => {
                let Some((path, rest)) = output
                    .strip_prefix("Edited ")
                    .and_then(|s| s.rsplit_once(". Replaced "))
                else {
                    continue;
                };
                if !rest
                    .split_once(" occurrence(s) using ")
                    .is_some_and(|(n, m)| {
                        n.parse::<usize>().is_ok_and(|n| n > 0)
                            && matches!(m, "all matches mode." | "first match mode.")
                    })
                {
                    continue;
                }
                let old = n::text(&args["old"]);
                let new = n::text(&args["new"]);
                if old.contains('\\') || new.contains('\\') {
                    continue;
                }
                (path, crate::added_lines(old, new))
            }
            _ => continue,
        };
        // Conversation rows contain no cwd. A relative/remote workspace cannot be guessed.
        if Path::new(path).is_absolute() || path.get(1..2) == Some(":") {
            imp::push_scoped(g, pm, "", path, &added, Src::Astrbot, "", id, ts);
        }
    }
    n::session(g, pm, Src::Astrbot, id, "", ts, ts, prompts, before);
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut q=db.prepare("select conversation_id,content,cast(strftime('%s',updated_at) as integer) from conversations where content is not null").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut g = FileGroup::default();
    for row in rows {
        let (id, s, ts) = row.map_err(|e| e.to_string())?;
        let rows: Vec<Value> = serde_json::from_str(&s).map_err(|e| e.to_string())?;
        parse(&id, ts, &rows, pm, &mut g);
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    n::scan(Src::Astrbot, store, pm, st, paths(projects), read);
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Astrbot,&root(),"native data_v4.db conversations",paths(&[]).len(),"Paired native file write/literal edit receipts with absolute repository paths. No cwd or per-message model in conversation rows; remote sandbox-relative paths and escaped edit fallbacks are excluded.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn successful_absolute_receipt_only() {
        let mut rows = vec![
            json!({"role":"assistant","tool_calls":[{"id":"c","function":{"name":"astrbot_file_write_tool","arguments":"{\"path\":\"src/main.rs\",\"content\":\"fn actual() {}\"}"}}]}),
            json!({"role":"tool","tool_call_id":"c","content":"File written successfully: /demo/src/main.rs"}),
        ];
        let mut g = FileGroup::default();
        parse("s", 1, &rows, &n::project(), &mut g);
        assert_eq!(g.evs.len(), 1);
        rows[1]["content"] = json!("Error writing file: denied");
        let mut g = FileGroup::default();
        parse("s", 1, &rows, &n::project(), &mut g);
        assert!(g.evs.is_empty());
    }
}
