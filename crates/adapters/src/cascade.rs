//! Cascade's documented post-write hooks and completed JSONL transcript steps.
//! https://docs.devin.ai/desktop/cascade/hooks
//! Reads existing files only: it never installs hooks. Native *.pb storage is not decoded.
use crate::trace_import as imp;
use crate::{any_ts, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    std::env::var("CREDITIA_WINDSURF_DIR")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".windsurf"))
}

fn parse(path: &Path, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let fallback = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let mut sessions: std::collections::HashMap<String, (String, i64, i64, i64)> =
        std::collections::HashMap::new();
    for v in rows {
        let id = v["trajectory_id"].as_str().unwrap_or(fallback);
        let model = v["model_name"]
            .as_str()
            .filter(|m| !m.eq_ignore_ascii_case("unknown"))
            .unwrap_or("");
        let ts = any_ts(&v["timestamp"]).unwrap_or(0);
        let mut relevant = false;
        match v["agent_action_name"].as_str() {
            Some("post_write_code") => {
                let t = &v["tool_info"];
                let p = t["file_path"].as_str().unwrap_or("");
                relevant = pm.to_rel(p).is_some();
                for edit in t["edits"].as_array().into_iter().flatten() {
                    // Both strings are part of the published hook contract. A missing old value isn't an empty file.
                    if let (Some(old), Some(new)) =
                        (edit["old_string"].as_str(), edit["new_string"].as_str())
                    {
                        imp::push_scoped(
                            g,
                            pm,
                            "",
                            p,
                            &crate::added_lines(old, new),
                            Src::Windsurf,
                            model,
                            id,
                            ts,
                        );
                    }
                }
            }
            None if v["type"] == "code_action" && v["status"] == "done" => {
                let a = &v["code_action"];
                let p = a["path"].as_str().unwrap_or("");
                relevant = pm.to_rel(p).is_some();
                if let Some(text) = a["new_content"].as_str() {
                    imp::push_scoped(g, pm, "", p, text, Src::Windsurf, model, id, ts);
                }
            }
            _ => {}
        }
        if relevant {
            let entry = sessions
                .entry(id.into())
                .or_insert((model.into(), ts, ts, 0));
            if !model.is_empty() {
                entry.0 = model.into();
            }
            if ts > 0 {
                entry.1 = if entry.1 == 0 { ts } else { entry.1.min(ts) };
                entry.2 = entry.2.max(ts);
            }
        }
    }
    for (id, (model, start, end, prompts)) in sessions {
        g.sessions.push(imp::session(
            Src::Windsurf,
            &id,
            &model,
            start,
            end,
            prompts,
            !g.evs.is_empty(),
        ));
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    imp::scan_files(Src::Windsurf, &data_dir(), store, pm, st, parse);
}

pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for p in imp::files(&data_dir()) {
        let mut roots = std::collections::HashSet::new();
        for v in imp::read(&p) {
            let path = match v["agent_action_name"].as_str() {
                Some("post_write_code") => v["tool_info"]["file_path"].as_str(),
                None if v["type"] == "code_action" && v["status"] == "done" => {
                    v["code_action"]["path"].as_str()
                }
                _ => None,
            };
            if let Some(path) = path {
                // A file parent is an observed location, not an invented repository root.
                if let Some(parent) = Path::new(path).parent() {
                    if roots.insert(parent.to_path_buf()) {
                        out.push(imp::activity(
                            Src::Windsurf,
                            parent.display().to_string(),
                            None,
                            any_ts(&v["timestamp"]).unwrap_or(0),
                        ));
                    }
                }
            }
        }
    }
    out
}

pub fn doctor() -> Vec<Health> {
    let root = data_dir();
    let records = imp::files(&root)
        .iter()
        .flat_map(|p| imp::read(p))
        .filter(|v| {
            v["agent_action_name"] == "post_write_code"
                || v["type"] == "code_action" && v["status"] == "done"
        })
        .count();
    vec![imp::health(Src::Windsurf, &root, "Cascade hook JSON / transcript JSONL", records,
        "Reads existing post_write_code edits and done code_action.new_content. Hook model_name is a display label; transcripts may omit model and time. Requires previously enabled logging/transcript hook; native cascade/*.pb history is not decoded. Applies to Windsurf Cascade / Devin Desktop Cascade, not Devin Local agent.", false)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_completed_edits_with_text_and_repository_path() {
        let pm = PathMap {
            known: ["src/a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let rows = vec![
            json!({"agent_action_name":"pre_write_code","trajectory_id":"s","tool_info":{"file_path":"/demo/src/a.rs","edits":[{"old_string":"","new_string":"rejected_line"}]}}),
            json!({"agent_action_name":"post_write_code","trajectory_id":"s","model_name":"Claude Sonnet 4","timestamp":"2026-10-17T01:00:00Z","tool_info":{"file_path":"/demo/src/a.rs","edits":[{"old_string":"keep_line","new_string":"keep_line\naccepted_line"}]}}),
            json!({"type":"code_action","status":"error","code_action":{"path":"/demo/src/a.rs","new_content":"failed_line"}}),
            json!({"type":"code_action","status":"done","code_action":{"path":"/other/src/a.rs","new_content":"foreign_line"}}),
            json!({"type":"code_action","status":"done","code_action":{"path":"/demo/src/a.rs"}}),
        ];
        let mut g = FileGroup::default();
        parse(Path::new("s.jsonl"), &rows, &pm, &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "Claude Sonnet 4");
        assert_eq!(g.evs[0].session, "s");
        assert!(g.evs[0].ts > 0);
    }
    #[test]
    fn documented_transcript_without_model_stays_unknown() {
        let pm = PathMap {
            known: ["a.py".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let mut g = FileGroup::default();
        parse(
            Path::new("trajectory-1.jsonl"),
            &[
                json!({"status":"done","type":"code_action","code_action":{"new_content":"print('hello world')\n","path":"/demo/a.py"}}),
            ],
            &pm,
            &mut g,
        );
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.evs[0].session, "trajectory-1");
    }
}
