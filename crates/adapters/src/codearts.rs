//! CodeArts' documented opencode.db locations. Accept only the known v1 schema
//! and completed tools carrying a saved unified diff, never infer its private schema.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    std::env::var_os("CREDITIA_CODEARTS_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".codeartsdoer"))
}
fn paths() -> Vec<PathBuf> {
    let root = root();
    let mut paths = if root.is_file() {
        vec![root]
    } else {
        vec![
            root.join("codearts-data/opencode.db"),
            root.join("cli-data/opencode.db"),
        ]
    };
    paths.retain(|p| p.is_file());
    paths
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut query = db.prepare("select s.id,s.directory,s.time_created,s.time_updated,m.data,p.data from session s join message m on m.session_id=s.id join part p on p.session_id=s.id and p.message_id=m.id order by s.id,m.time_created,p.time_created,p.id")
        .map_err(|e| format!("unrecognized CodeArts OpenCode schema: {e}"))?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut g = FileGroup::default();
    let mut sessions = std::collections::HashMap::new();
    for row in rows {
        let (id, cwd, start, end, message, part) = row.map_err(|e| e.to_string())?;
        if !pm.in_project(&cwd) {
            continue;
        }
        let m: Value = serde_json::from_str(&message).map_err(|e| e.to_string())?;
        let p: Value = serde_json::from_str(&part).map_err(|e| e.to_string())?;
        sessions.insert(id.clone(), (cwd.clone(), start, end));
        if m["role"] != "assistant"
            || p["type"] != "tool"
            || p["state"]["status"] != "completed"
            || !matches!(n::text(&p["tool"]), "write" | "edit")
        {
            continue;
        }
        let diff = n::text(&p["state"]["metadata"]["diff"]);
        if !diff.lines().any(|l| l.starts_with("@@ ")) {
            continue;
        }
        let Some(path) = crate::path_arg(&p["state"]["input"]) else {
            continue;
        };
        let model = n::text(&m["modelID"]);
        let ts = crate::any_ts(&p["state"]["time"]["end"])
            .or_else(|| crate::any_ts(&m["time"]["created"]))
            .unwrap_or(0);
        imp::push_scoped(
            &mut g,
            pm,
            &cwd,
            &path,
            &crate::added_from_diff(diff),
            Src::CodeartsAgent,
            model,
            &id,
            ts,
        );
    }
    for (id, (cwd, start, end)) in sessions {
        let has = g.evs.iter().any(|e| e.session == id);
        let start = crate::any_ts(&start.into()).unwrap_or(0);
        let end = crate::any_ts(&end.into()).unwrap_or(0);
        if pm.in_project(&cwd) {
            g.sessions.push(imp::session(
                Src::CodeartsAgent,
                &id,
                "",
                start,
                end,
                0,
                has,
            ));
            g.reconciled_sessions.push((Src::CodeartsAgent, id));
        }
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::CodeartsAgent, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    paths()
        .iter()
        .flat_map(|p| {
            let Ok(db) = crate::open_ro(p) else {
                return vec![];
            };
            let Ok(mut q) = db.prepare(
                "select directory,max(time_updated),count(*) from session group by directory",
            ) else {
                return vec![];
            };
            q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, u64>(2)?,
                ))
            })
            .into_iter()
            .flatten()
            .flatten()
            .map(|(cwd, ts, sessions)| Activity {
                src: Src::CodeartsAgent,
                cwd,
                remote: None,
                sessions,
                last: crate::any_ts(&ts.into()).unwrap_or(0),
            })
            .collect()
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::CodeartsAgent, &root(), "conditional OpenCode v1 session/message/part SQLite", paths().len(), "Official IDE/CLI opencode.db locations; only matching schema with completed saved diffs is accepted. Vendor history not available on this machine; unknown schema warns and contributes no edits. Legacy IDE chat_sessions is not decoded.", false)
}
