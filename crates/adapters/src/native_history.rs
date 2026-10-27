//! Read-only helpers shared by the source-specific native history readers.
use crate::{trace_import as imp, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) fn json(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A reconstructed snapshot must be complete, including the final append row.
pub(crate) fn jsonl(path: &Path) -> Result<Vec<Value>, String> {
    let bytes = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = vec![];
    for (i, line) in bytes.split_inclusive('\n').enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(v) => rows.push(v),
            Err(e) => return Err(format!("{}:{}: {e}", path.display(), i + 1)),
        }
    }
    Ok(rows)
}

pub(crate) fn files(root: &Path, accept: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    if root.is_file() {
        return if accept(root) {
            vec![root.into()]
        } else {
            vec![]
        };
    }
    let mut out: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && accept(e.path()))
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}

pub(crate) fn sqlite_files(root: &Path) -> Vec<PathBuf> {
    files(root, |p| {
        matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("db" | "sqlite" | "sqlite3")
        )
    })
}

/// A current snapshot replaces prior evidence, including when records were removed.
/// Unreadable/missing history is reported; only a successfully reconstructed
/// session can retract persisted evidence (see reconciled_sessions).
pub(crate) fn scan(
    src: Src,
    store: &mut Store,
    pm: &PathMap,
    st: &mut Stats,
    paths: Vec<PathBuf>,
    parse: impl Fn(&Path, &PathMap) -> Result<FileGroup, String>,
) {
    let prefix = format!("{}:history:", src.harness());
    store.groups.retain(|k, _| !k.starts_with(&prefix));
    let mut groups = vec![];
    let mut unreadable = false;
    for path in paths {
        match parse(&path, pm) {
            Ok(group) => {
                groups.push((path, group));
            }
            Err(e) => {
                unreadable = true;
                st.warnings.push(format!("{}: {e}", src.harness()));
            }
        }
    }
    // An unreadable newer snapshot may share a session with any readable copy.
    // Without its identity/progress, that older copy cannot authorize retraction.
    // The analysis layer restores this source's existing durable evidence.
    if unreadable {
        return;
    }
    // A copied database / archived transcript can repeat a whole session.
    // Native progress orders snapshots; copying a file never advances it.
    let mut candidates: std::collections::HashMap<_, Vec<_>> = std::collections::HashMap::new();
    for (i, (_, g)) in groups.iter().enumerate() {
        for (source, id) in &g.reconciled_sessions {
            let revision = g
                .session_revisions
                .iter()
                .find(|(s, key, _)| s == source && key == id)
                .map(|(_, _, end)| *end)
                .or_else(|| {
                    g.sessions
                        .iter()
                        .find(|s| s.src == *source && s.id == *id)
                        .map(|s| s.end)
                })
                .filter(|end| *end > 0);
            candidates
                .entry((*source, id.clone()))
                .or_default()
                .push((i, revision));
        }
    }
    let mut owners = std::collections::HashMap::new();
    for ((source, id), mut copies) in candidates {
        if copies.iter().all(|(_, revision)| revision.is_some()) {
            let latest = copies.iter().filter_map(|(_, revision)| *revision).max();
            copies.retain(|(_, revision)| *revision == latest);
        }
        let first = copies[0].0;
        let snapshot = |i: usize| {
            let g = &groups[i].1;
            let mut events: Vec<_> = g
                .evs
                .iter()
                .filter(|e| e.src == source && e.session == id)
                .map(|e| serde_json::to_string(e).expect("event serialization"))
                .collect();
            let mut sessions: Vec<_> = g
                .sessions
                .iter()
                .filter(|s| s.src == source && s.id == id)
                .map(|s| serde_json::to_string(s).expect("session serialization"))
                .collect();
            events.sort();
            sessions.sort();
            (events, sessions)
        };
        let first_snapshot = snapshot(first);
        let owner = if copies
            .iter()
            .all(|(i, _)| *i == first || snapshot(*i) == first_snapshot)
        {
            Some(first)
        } else {
            st.warnings.push(format!("{}: conflicting snapshots for session {id} have no distinct native progress; preserving stored evidence", source.harness()));
            None
        };
        owners.insert((source, id), owner);
    }
    for (i, (path, mut g)) in groups.into_iter().enumerate() {
        let owned = |source, id: &str| {
            owners
                .get(&(source, id.to_string()))
                .is_none_or(|owner| *owner == Some(i))
        };
        g.evs.retain(|e| owned(e.src, &e.session));
        g.sessions.retain(|s| owned(s.src, &s.id));
        g.reconciled_sessions.retain(|(s, id)| owned(*s, id));
        g.session_revisions.retain(|(s, id, _)| owned(*s, id));
        store
            .groups
            .insert(format!("{prefix}{}", path.display()), g);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn session(
    g: &mut FileGroup,
    pm: &PathMap,
    src: Src,
    id: &str,
    cwd: &str,
    start: i64,
    end: i64,
    prompts: i64,
    before: usize,
) {
    if id.is_empty() {
        return;
    }
    g.reconciled_sessions.push((src, id.into()));
    if end > 0 {
        g.session_revisions.push((src, id.into(), end));
    }
    if pm.in_project(cwd) || g.evs.len() > before {
        g.sessions.push(imp::session(
            src,
            id,
            "",
            start,
            end,
            prompts,
            g.evs.len() > before,
        ));
    }
}

pub(crate) fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

#[cfg(test)]
pub(crate) fn project() -> PathMap {
    PathMap {
        known: ["src/main.rs".into()].into(),
        markers: vec!["/demo/".into()],
        excludes: vec![],
    }
}
