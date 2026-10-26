//! Lingma 2.6.10 local.db and workingSpace plaintext snapshots.
//! Native struct tags, provider and acceptance path: SUPPORT_AGENT6.md.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use md5::{Digest, Md5};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn roots() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("CREDITIA_LINGMA_DIR") {
        return vec![p.into()];
    }
    let mut roots = vec![crate::home().join(".lingma")];
    if let Some(p) = std::env::var_os("COSY_HOME") {
        roots.push(p.into());
    }
    #[cfg(target_os = "macos")]
    roots.push(crate::home().join("Library/Caches/.lingma"));
    #[cfg(target_os = "windows")]
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(p).join(".lingma"));
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    roots.push(
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::home().join(".cache"))
            .join(".lingma"),
    );
    roots.sort();
    roots.dedup();
    roots
}
fn paths() -> Vec<PathBuf> {
    roots()
        .iter()
        .flat_map(|r| n::files(r, |p| p.file_name().is_some_and(|n| n == "local.db")))
        .collect()
}
fn hash(text: &str) -> String {
    format!("{:x}", Md5::digest(text.as_bytes()))
}
fn absolute(p: &str) -> bool {
    p.starts_with('/')
        || (p.as_bytes().get(1) == Some(&b':') && matches!(p.as_bytes().get(2), Some(b'/' | b'\\')))
}
struct File {
    id: String,
    session: String,
    path: String,
    content_type: String,
    kind: String,
    key: String,
    status: String,
    version: String,
    local_id: String,
    extra: String,
    start: i64,
    end: i64,
}
fn cached(db_path: &Path, file: &File) -> Result<String, String> {
    let portable_key = file.key.replace('\\', "/");
    let key = Path::new(&portable_key);
    let name = file.path.rsplit(['/', '\\']).next().unwrap_or("");
    let expected = format!("{}__{name}", file.id);
    // GenerateKey creates this exact filename under workingSpace. Never read
    // the mutable destination file as a substitute for a missing snapshot.
    if !absolute(&file.key)
        || key.file_name().is_none_or(|n| n != expected.as_str())
        || key
            .parent()
            .and_then(Path::file_name)
            .is_none_or(|n| n != "workingSpace")
        || file.id.contains(['/', '\\'])
    {
        return Err("unexpected native content key".into());
    }
    // Native cache/db/local.db and cache/workingSpace travel together. The
    // absolute key records the original machine, including Windows paths.
    let snapshot = db_path
        .parent()
        .and_then(Path::parent)
        .ok_or("missing native cache directory")?
        .join("workingSpace")
        .join(expected);
    if std::fs::symlink_metadata(&snapshot).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("symlinked native snapshot".into());
    }
    std::fs::read_to_string(snapshot).map_err(|e| format!("unreadable native snapshot: {e}"))
}
fn read(
    path: &Path,
    pm: &PathMap,
    warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<FileGroup, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    db.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    let mut q = db.prepare("SELECT item_id,session_id,file_id,content_type,type,key,status,version,local_id,extra,gmt_create,gmt_modified FROM chat_working_space_file").map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok(File {
                id: r.get(0)?,
                session: r.get(1)?,
                path: r.get(2)?,
                content_type: r.get(3)?,
                kind: r.get(4)?,
                key: r.get(5)?,
                status: r.get(6)?,
                version: r.get(7)?,
                local_id: r.get(8)?,
                extra: r.get(9)?,
                start: r.get(10)?,
                end: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut files = vec![];
    for row in rows {
        files.push(row.map_err(|e| e.to_string())?);
    }
    files.retain(|f| absolute(&f.path) && pm.to_rel(&f.path).is_some());
    let mut contents = HashMap::new();
    let mut versions = HashMap::new();
    for f in &files {
        if f.content_type != "FULL" || f.kind != "LOCAL_FILE" {
            continue;
        }
        if let Ok(text) = cached(path, f) {
            versions.insert(
                (f.session.clone(), f.path.clone(), hash(&text)),
                text.clone(),
            );
            contents.insert(f.id.clone(), text);
        }
    }
    let mut g = FileGroup::default();
    let mut sessions = HashMap::<String, (i64, i64, bool)>::new();
    for f in &files {
        // Initial local baselines are also ACCEPTED. They have version 0 and
        // no local_id; they must never become agent-created code.
        if f.id.is_empty()
            || f.session.is_empty()
            || f.local_id.is_empty()
            || f.version.is_empty()
            || f.version == "0"
        {
            continue;
        }
        let sid = format!("item:{}", f.id);
        if matches!(
            f.status.as_str(),
            "REJECTED" | "GENERATING_CANCELLED" | "APPLYING_CANCELLED"
        ) {
            g.reconciled_sessions.push((Src::Lingma, sid));
            continue;
        }
        if f.status != "ACCEPTED" || f.content_type != "FULL" || f.kind != "LOCAL_FILE" {
            continue;
        }
        let result = (|| -> Result<String, String> {
            let extra: Value =
                serde_json::from_str(&f.extra).map_err(|e| format!("invalid native extra: {e}"))?;
            // edit and use-local resolve to a mutable local row in the native
            // provider. Only independently saved full snapshots are replayed.
            if extra["CHAT_MODE"] == "edit"
                || extra["LINGMA_TAG::CONTENT_USE_WORKING_SPACE_LOCAL"] == true
            {
                return Err("mutable local-content reference".into());
            }
            let diff = &extra["diffInfo"];
            let target = diff["targetMd5"].as_str().ok_or("missing targetMd5")?;
            let source = diff["sourceMd5"].as_str().ok_or("missing sourceMd5")?;
            let after = contents.get(&f.id).ok_or("missing native full snapshot")?;
            if target != hash(after) {
                return Err("targetMd5 does not match native snapshot".into());
            }
            let before = if source == hash("") {
                ""
            } else {
                versions
                    .get(&(f.session.clone(), f.path.clone(), source.into()))
                    .map(String::as_str)
                    .ok_or("sourceMd5 has no matching native snapshot")?
            };
            Ok(crate::added_lines(before, after))
        })();
        match result {
            Ok(text) => {
                let start = crate::any_ts(&Value::from(f.start)).unwrap_or(0);
                let end = crate::any_ts(&Value::from(f.end)).unwrap_or(0);
                let before = g.evs.len();
                imp::push_scoped(&mut g, pm, "", &f.path, &text, Src::Lingma, "", &sid, end);
                g.reconciled_sessions.push((Src::Lingma, sid));
                let s = sessions
                    .entry(f.session.clone())
                    .or_insert((start, end, false));
                s.0 = s.0.min(start);
                s.1 = s.1.max(end);
                s.2 |= g.evs.len() > before;
            }
            Err(reason) => warnings.borrow_mut().push(format!(
                "lingma: {} item {}: {reason}; prior evidence retained",
                path.display(),
                f.id
            )),
        }
    }
    for (id, (start, end, content)) in sessions {
        g.sessions
            .push(imp::session(Src::Lingma, &id, "", start, end, 0, content));
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let warnings = std::cell::RefCell::new(vec![]);
    n::scan(Src::Lingma, store, pm, st, paths(), |p, pm| {
        read(p, pm, &warnings)
    });
    st.warnings.extend(warnings.into_inner());
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for path in paths() {
        let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            continue;
        };
        let Ok(mut q) = db.prepare("SELECT project_uri,gmt_modified FROM chat_session") else {
            continue;
        };
        let Ok(rows) = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        else {
            continue;
        };
        for (uri, last) in rows.flatten() {
            if absolute(&uri) {
                out.push(imp::activity(
                    Src::Lingma,
                    uri,
                    None,
                    crate::any_ts(&Value::from(last)).unwrap_or(0),
                ));
            }
        }
    }
    out
}
pub fn doctor() -> Health {
    imp::health(Src::Lingma,&roots()[0],"local.db + workingSpace snapshots",paths().len(),"2.6.10 ACCEPTED full snapshots with native sourceMd5/targetMd5 verification. Model remains unknown. Preview, initial baselines, mutable local aliases, missing or mismatched content are excluded; explicit rejection retracts evidence.",false)
}
