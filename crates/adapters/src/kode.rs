//! Kode CLI's native projects/<cwd>/<session>.jsonl, with successful structured tool results.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_KODE_DIR")
        .or_else(|| std::env::var_os("KODE_CONFIG_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".kode"))
}
fn paths() -> Vec<PathBuf> {
    crate::pi::jsonl_files(&root().join("projects"))
}
fn parse(rows: &[Value], pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut ids = HashMap::new();
    for row in rows {
        let session = n::text(&row["sessionId"]);
        let cwd = n::text(&row["cwd"]);
        if session.is_empty() {
            continue;
        }
        let id = if row["isSidechain"] == true {
            format!("{session}/agent/{}", n::text(&row["agentId"]))
        } else {
            session.to_string()
        };
        // Sidechains share a parent sessionId but have independent native transcripts.
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(0);
        let entry = ids.entry(id.clone()).or_insert((cwd, ts, ts, 0));
        entry.1 = entry.1.min(ts);
        entry.2 = entry.2.max(ts);
        if row["type"] == "user" && row["message"]["content"].is_string() {
            entry.3 += 1;
        }
        for part in row["message"]["content"].as_array().into_iter().flatten() {
            if row["type"] == "assistant" && part["type"] == "tool_use" {
                let cid = n::text(&part["id"]);
                if !cid.is_empty() {
                    calls.insert(
                        cid,
                        (part, n::text(&row["message"]["model"]), id.clone(), cwd),
                    );
                }
            } else if row["type"] == "user" && part["type"] == "tool_result" {
                let cid = n::text(&part["tool_use_id"]);
                let Some((call, model, origin, cwd)) = calls.remove(cid) else {
                    continue;
                };
                if part["is_error"] == true
                    || !seen.insert((origin.clone(), cid))
                    || !matches!(n::text(&call["name"]), "Write" | "Edit")
                {
                    continue;
                }
                let result = &row["toolUseResult"];
                let path = n::text(&result["filePath"]);
                if path.is_empty() {
                    continue;
                }
                let added = if result["type"] == "create" {
                    n::text(&result["content"]).into()
                } else if result["structuredPatch"].is_array() {
                    crate::added_from_patch(&result["structuredPatch"])
                } else {
                    continue;
                };
                imp::push_scoped(&mut g, pm, cwd, path, &added, Src::Kode, model, &origin, ts);
            }
        }
    }
    for (id, (cwd, start, end, prompts)) in ids {
        let has = g.evs.iter().any(|e| e.session == id);
        if pm.in_project(cwd) || has {
            g.sessions
                .push(imp::session(Src::Kode, &id, "", start, end, prompts, has));
            g.reconciled_sessions.push((Src::Kode, id));
        }
    }
    g
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    Ok(parse(&n::jsonl(path)?, pm))
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Kode, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    paths()
        .iter()
        .filter_map(|p| {
            let rows = imp::read(p);
            let r = rows.iter().find(|r| r["cwd"].is_string())?;
            Some(imp::activity(
                Src::Kode,
                n::text(&r["cwd"]).into(),
                None,
                rows.iter()
                    .filter_map(|r| crate::any_ts(&r["timestamp"]))
                    .max()
                    .unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Kode,&root(),"native session JSONL + toolUseResult",paths().len(),"Confirmed Write/Edit structuredPatch or create receipts; snapshots, failed tools and unpaired requests do not supply edit evidence.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn paired_structured_receipt() {
        let mut rows = vec![
            json!({"sessionId":"s","cwd":"/demo","type":"assistant","message":{"model":"m","content":[{"type":"tool_use","id":"c","name":"Write","input":{"file_path":"src/main.rs","content":"fn proposal() {}"}}]}}),
            json!({"sessionId":"s","cwd":"/demo","type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"c","is_error":false}]},"toolUseResult":{"type":"create","filePath":"src/main.rs","content":"fn actual() {}"}}),
        ];
        assert_eq!(parse(&rows, &n::project()).evs.len(), 1);
        rows[1]["message"]["content"][0]["is_error"] = json!(true);
        assert!(parse(&rows, &n::project()).evs.is_empty());
    }
}
