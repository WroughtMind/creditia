//! Native editor history boundaries. Antigravity summary schema was inspected read-only on
//! macOS 2026-10-17. Summaries/FTS search text do not certify successful code writes.
//! TRAE CLI has a separate reader; no unverified VS Code ItemTable payload is treated as an edit.
use crate::trace_import as imp;
use crate::{any_ts, file_uri_path, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn antigravity_dir() -> PathBuf {
    std::env::var("CREDITIA_ANTIGRAVITY_IDE_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".gemini/antigravity"))
}
struct Summary {
    id: String,
    roots: Vec<String>,
    end: i64,
}
fn summaries(path: &Path) -> Result<Vec<Summary>, String> {
    let c = open_ro(path).map_err(|e| e.to_string())?;
    let mut q = c
        .prepare(
            "SELECT conversation_id,workspace_uris,last_modified_time FROM conversation_summaries",
        )
        .map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            let roots: Value = serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or_default();
            let roots = roots
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(file_uri_path)
                .collect();
            let end = r.get::<_, String>(2)?;
            Ok(Summary {
                id: r.get(0)?,
                roots,
                end: any_ts(&Value::String(end)).unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    crate::antigravity_ide::scan(store, pm, st);
    let native_ids: std::collections::HashSet<_> = store.groups.iter()
        .filter(|(key, _)| key.starts_with("antigravity-native:"))
        .flat_map(|(_, g)| g.sessions.iter().map(|s| s.id.clone())).collect();
    let p = antigravity_dir().join("conversation_summaries.db");
    let key = format!("antigravity-ide:{}", p.display());
    if !p.is_file() {
        store.groups.remove(&key);
        return;
    }
    let a = fingerprint(&p);
    let b = fingerprint(&PathBuf::from(format!("{}-wal", p.display())));
    let fp = (a.0.max(b.0), a.1 + b.1);
    if let Some(g) = store.groups.get_mut(&key) {
        g.sessions.retain(|s| !native_ids.contains(&s.id));
    }
    if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
        return;
    }
    let rows = match summaries(&p) {
        Ok(rows) => rows,
        Err(e) => {
            st.warnings
                .push(format!("antigravity-ide: {}: {e}", p.display()));
            return;
        }
    };
    let mut g = FileGroup {
        fp,
        ..Default::default()
    };
    for r in rows {
        if !native_ids.contains(&r.id) && r.roots.iter().any(|p| pm.in_project(p)) {
            // last_user_input_time is NOT conversation start: keep start unknown, end is observed modification time.
            g.sessions.push(imp::session(
                Src::AntigravityIde,
                &r.id,
                "",
                0,
                r.end,
                0,
                false,
            ));
        }
    }
    if !g.sessions.is_empty() {
        st.notes.push(format!(
            "antigravity-ide: {} project conversation summaries; edit bodies/model not decoded",
            g.sessions.len()
        ));
    }
    store.groups.insert(key, g);
}
pub fn discover() -> Vec<Activity> {
    let (mut activities, native_ids) = crate::antigravity_ide::discover();
    activities.extend(summaries(&antigravity_dir().join("conversation_summaries.db"))
        .unwrap_or_default()
        .into_iter()
        .filter(|r| !native_ids.contains(&r.id))
        .flat_map(|r| {
            r.roots
                .into_iter()
                .map(move |p| imp::activity(Src::AntigravityIde, p, None, r.end))
        })
        .collect::<Vec<_>>());
    activities
}
pub fn doctor() -> Vec<Health> {
    let root = antigravity_dir();
    let db = root.join("conversation_summaries.db");
    let result = summaries(&db);
    let count = result.as_ref().map(Vec::len).unwrap_or(0);
    let mut note="Reads conversation_id, workspace_uris and last_modified_time from observed native summary database. Model, true start time, prompt count and successful file edit content are not in the confirmed summary columns. Native conversation protobuf and FTS text are not treated as write evidence.".to_string();
    if db.exists() {
        if let Err(e) = result {
            note.push_str(&format!(" Database read failed: {e}"));
        }
    }
    let mut out = vec![imp::health(
        Src::AntigravityIde,
        &db,
        "conversation_summaries SQLite (metadata only)",
        count,
        &note,
        false,
    )];
    if let Some(native) = crate::antigravity_ide::doctor() { out[0] = native; }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_summary_preserves_only_observed_metadata() {
        let p =
            std::env::temp_dir().join(format!("creditia-agy-summary-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch("CREATE TABLE conversation_summaries(conversation_id TEXT,workspace_uris TEXT,last_modified_time TEXT,last_user_input_time TEXT); INSERT INTO conversation_summaries VALUES('s','[\"file:///demo\"]','2026-10-17 01:00:00+00:00','2026-10-17 00:59:00+00:00');").unwrap();
        drop(c);
        let s = summaries(&p).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].roots, vec!["/demo"]);
        assert!(s[0].end > 0);
        std::fs::remove_file(p).unwrap();
    }
}
