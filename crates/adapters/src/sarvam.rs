//! Sarvam Code 0.55.0 rollout JSONL; native layout and event serde in its release.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_SARVAM_CODE_DIR")
        .or_else(|| std::env::var_os("SARVAM_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".sarvam"))
}
fn paths() -> Vec<PathBuf> {
    let r = root();
    if r.is_file() {
        return vec![r];
    }
    ["sessions", "archived_sessions"]
        .iter()
        .flat_map(|d| n::files(&r.join(d), |p| p.extension().is_some_and(|s| s == "jsonl")))
        .collect()
}
fn terminal(row: &Value) -> Option<(&str, &Value)> {
    let p = &row["payload"];
    (row["type"] == "event_msg" && p["type"] == "patch_apply_end")
        .then(|| (n::text(&p["call_id"]), p))
}
fn parse(
    rows: &[Value],
    pm: &PathMap,
    owned: Option<&HashSet<String>>,
) -> Result<FileGroup, String> {
    let meta = rows
        .first()
        .filter(|r| r["type"] == "session_meta")
        .map(|r| &r["payload"])
        .ok_or("Sarvam rollout has no session metadata")?;
    let id = meta["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Sarvam session has no id")?;
    let cwd = n::text(&meta["cwd"]);
    let mut g = FileGroup::default();
    let mut model = String::new();
    let mut edits = HashMap::new();
    let start = crate::any_ts(&meta["timestamp"]).unwrap_or(0);
    let mut end = start;
    let mut prompts = 0;
    for row in rows {
        let p = &row["payload"];
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(start);
        end = end.max(ts);
        if row["type"] == "turn_context" {
            model = n::text(&p["model"]).into();
        }
        if row["type"] == "event_msg" && p["type"] == "task_started" {
            prompts += 1;
        }
        if let Some((cid, event)) = terminal(row) {
            if !cid.is_empty() {
                edits.insert(cid, (event, model.clone(), ts));
            }
        }
    }
    for (cid, (event, model, ts)) in edits {
        if event["success"] != true || owned.is_some_and(|o| !o.contains(cid)) {
            continue;
        }
        let Some(changes) = event["changes"].as_object() else {
            continue;
        };
        for (file, change) in changes {
            let dest = change["move_path"].as_str().unwrap_or(file);
            let text = match n::text(&change["type"]) {
                "add" => n::text(&change["content"]).to_string(),
                "update" => {
                    let mut hunk = false;
                    let mut out = String::new();
                    for line in n::text(&change["unified_diff"]).lines() {
                        if line.starts_with("@@ ") {
                            hunk = true;
                        } else if hunk {
                            if let Some(s) = line.strip_prefix('+') {
                                out.push_str(s);
                                out.push('\n');
                            }
                        }
                    }
                    out
                }
                _ => continue,
            };
            imp::push_scoped(
                &mut g,
                pm,
                cwd,
                dest,
                &text,
                Src::SarvamCode,
                &model,
                id,
                ts,
            );
        }
    }
    n::session(&mut g, pm, Src::SarvamCode, id, cwd, start, end, prompts, 0);
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let paths = paths();
    let mut parsed = HashMap::new();
    let mut owners = HashMap::new();
    for p in &paths {
        let result = n::jsonl(p).and_then(|r| {
            parse(&r, pm, None)?;
            Ok(r)
        });
        if let Ok(rows) = &result {
            let rank = (
                std::fs::metadata(p).and_then(|m| m.modified()).ok(),
                p.clone(),
            );
            for row in rows {
                if let Some((cid, _)) = terminal(row) {
                    if !cid.is_empty() {
                        let owner = owners
                            .entry(cid.to_string())
                            .or_insert_with(|| rank.clone());
                        if rank > *owner {
                            *owner = rank.clone();
                        }
                    }
                }
            }
        }
        parsed.insert(p.clone(), result);
    }
    n::scan(Src::SarvamCode, store, pm, st, paths, |p, pm| {
        let rows = parsed
            .get(p)
            .ok_or("missing Sarvam rollout")?
            .as_ref()
            .map_err(Clone::clone)?;
        let owned = owners
            .iter()
            .filter(|(_, (_, path))| path == p)
            .map(|(id, _)| id.clone())
            .collect();
        parse(rows, pm, Some(&owned))
    });
}
pub fn discover() -> Vec<Activity> {
    paths()
        .into_iter()
        .filter_map(|p| {
            let r = n::jsonl(&p).ok()?;
            let meta = &r.first()?["payload"];
            let cwd = meta["cwd"].as_str()?;
            Some(imp::activity(
                Src::SarvamCode,
                cwd.into(),
                None,
                crate::any_ts(&meta["timestamp"]).unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::SarvamCode,&root(),"sessions / archived_sessions rollout JSONL",paths().len(),"Completed patch_apply_end with success=true, add/update/move text and per-turn model when recorded. Pending/failed patches and shell proposals are excluded; repeated calls across forks count once. No mutable index model inference.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn failed_final_receipt_wins_and_missing_turn_model_stays_unknown() {
        let mut rows = vec![
            json!({"type":"session_meta","payload":{"id":"s","cwd":"/demo"}}),
            json!({"type":"turn_context","payload":{"model":"m"}}),
            json!({"type":"event_msg","payload":{"type":"patch_apply_end","call_id":"c","success":true,"changes":{"src/main.rs":{"type":"add","content":"fn saved() {}"}}}}),
        ];
        assert_eq!(parse(&rows, &n::project(), None).unwrap().evs[0].model, "m");
        rows[1]["payload"] = json!({"turn_id":"new"});
        assert_eq!(parse(&rows, &n::project(), None).unwrap().evs[0].model, "");
        let mut failed = rows[2].clone();
        failed["payload"]["success"] = json!(false);
        rows.push(failed);
        assert!(parse(&rows, &n::project(), None).unwrap().evs.is_empty());
    }
}
