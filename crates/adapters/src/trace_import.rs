//! Shared import plumbing for vendor-owned formats. No generic JSON evidence fallback.
use crate::{Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) fn files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && matches!(
                    e.path().extension().and_then(|x| x.to_str()),
                    Some("json" | "jsonl" | "traj" | "har")
                )
        })
        .map(|e| e.into_path())
        .collect()
}

pub(crate) fn read(path: &Path) -> Vec<Value> {
    let Ok(s) = std::fs::read_to_string(path) else {
        return vec![];
    };
    if let Ok(v) = serde_json::from_str::<Value>(&s) {
        return if let Value::Array(a) = v { a } else { vec![v] };
    }
    s.lines()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect()
}

pub(crate) fn session(
    src: Src,
    id: &str,
    model: &str,
    start: i64,
    end: i64,
    prompts: i64,
    has_content: bool,
) -> Session {
    Session {
        src,
        id: id.into(),
        model: model.into(),
        start,
        end,
        tokens: 0,
        branch: None,
        prompts,
        reviewer: false,
        has_content,
        provider: None,
    }
}

pub(crate) fn scan_files(
    src: Src,
    root: &Path,
    store: &mut Store,
    pm: &PathMap,
    st: &mut Stats,
    parse: impl Fn(&Path, &[Value], &PathMap, &mut FileGroup),
) {
    let prefix = format!("{}:import:", src.harness());
    let paths = files(root);
    let keys: std::collections::HashSet<String> = paths
        .iter()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(&prefix) || keys.contains(k));
    let mut sessions = 0;
    for p in paths {
        let key = format!("{prefix}{}", p.display());
        let fp = fingerprint(&p);
        if let Some(g) = store.groups.get(&key).filter(|g| g.fp == fp) {
            sessions += g.sessions.len();
            continue;
        }
        let rows = read(&p);
        if rows.is_empty() && fp.1 > 0 {
            st.warnings.push(format!(
                "{}: unreadable or invalid import {}",
                src.harness(),
                p.display()
            ));
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        parse(&p, &rows, pm, &mut g);
        sessions += g.sessions.len();
        store.groups.insert(key, g);
    }
    if sessions > 0 {
        st.notes.push(format!(
            "{}: {sessions} project sessions from vendor exports",
            src.harness()
        ));
    }
}

pub(crate) fn activity(src: Src, cwd: String, remote: Option<String>, last: i64) -> Activity {
    Activity {
        src,
        cwd,
        remote,
        sessions: 1,
        last,
    }
}

pub(crate) fn health(
    src: Src,
    root: &Path,
    format: &str,
    records: usize,
    note: &str,
    unsupported: bool,
) -> Health {
    Health {
        source: src.harness().into(),
        path: root.display().to_string(),
        readable: records > 0,
        format: format.into(),
        records: records as u64,
        note: note.into(),
        unsupported,
        ..Default::default()
    }
}

/// Unified patch additions, grouped by the +++ destination. Hunk context and deletions never become evidence.
pub(crate) fn diff_files(diff: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    let mut current = None;
    let mut hunk = false;
    for l in diff.lines() {
        if l.starts_with("diff --git ") {
            current = None;
            hunk = false;
        } else if let Some(p) = l.strip_prefix("+++ ") {
            let p = p.split('\t').next().unwrap_or(p);
            let p = if p.starts_with('"') {
                match git_quoted_path(p) {
                    Some(p) => p,
                    None => {
                        current = None;
                        hunk = false;
                        continue;
                    }
                }
            } else {
                p.into()
            };
            if p == "/dev/null" {
                current = None;
                continue;
            }
            out.push((p.strip_prefix("b/").unwrap_or(&p).into(), String::new()));
            current = Some(out.len() - 1);
            hunk = false;
        } else if l.starts_with("@@ ") {
            hunk = true;
            if let Some(i) = current { out[i].1.push_str(l); out[i].1.push('\n'); }
        } else if hunk {
            if let Some(i) = current {
                out[i].1.push_str(l);
                out[i].1.push('\n');
            }
        }
    }
    out.into_iter().filter_map(|(p, diff)| { let added = crate::added_from_diff(&diff); (!added.is_empty()).then_some((p, added)) }).collect()
}

/// Git quotes paths with C escapes, including octal UTF-8 bytes; JSON strings
/// don't support those octal escapes. Decode only the documented Git form.
fn git_quoted_path(path: &str) -> Option<String> {
    let raw = path.strip_prefix('"')?.strip_suffix('"')?.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != b'\\' {
            out.push(raw[i]);
            i += 1;
            continue;
        }
        i += 1;
        let escaped = *raw.get(i)?;
        let byte = match escaped {
            b'0'..=b'7' => {
                let mut n = 0u16;
                let mut count = 0;
                while i < raw.len() && count < 3 && (b'0'..=b'7').contains(&raw[i]) {
                    n = n * 8 + u16::from(raw[i] - b'0');
                    i += 1;
                    count += 1;
                }
                out.push(u8::try_from(n).ok()?);
                continue;
            }
            b'a' => 7,
            b'b' => 8,
            b't' => b'\t',
            b'n' => b'\n',
            b'v' => 11,
            b'f' => 12,
            b'r' => b'\r',
            b'\\' => b'\\',
            b'"' => b'"',
            _ => return None,
        };
        out.push(byte);
        i += 1;
    }
    String::from_utf8(out).ok()
}

#[allow(clippy::too_many_arguments)] // Same evidence fields as push_text, with a verified cwd for relative paths.
pub(crate) fn push_scoped(
    g: &mut FileGroup,
    pm: &PathMap,
    cwd: &str,
    path: &str,
    text: &str,
    src: Src,
    model: &str,
    id: &str,
    ts: i64,
) {
    push_scoped_events(&mut g.evs, pm, cwd, path, text, src, model, id, ts);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_scoped_events(
    evs: &mut Vec<creditia_core::model::Ev>,
    pm: &PathMap,
    cwd: &str,
    path: &str,
    text: &str,
    src: Src,
    model: &str,
    id: &str,
    ts: i64,
) {
    if text.is_empty() {
        return;
    }
    let absolute = if Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':') {
        path.replace('\\', "/")
    } else if !cwd.is_empty() {
        format!(
            "{}/{}",
            cwd.replace('\\', "/").trim_end_matches('/'),
            path.replace('\\', "/")
        )
    } else {
        return;
    };
    // Clean traversal lexically: remote/deleted paths cannot be canonicalized
    // against this computer's filesystem. Do this before the project boundary check.
    let rooted = absolute.starts_with('/');
    let mut parts: Vec<&str> = vec![];
    for part in absolute.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| !p.ends_with(':')) {
                    parts.pop();
                }
            }
            _ => parts.push(part),
        }
    }
    let resolved = format!("{}{}", if rooted { "/" } else { "" }, parts.join("/"));
    crate::push_text(evs, pm, &resolved, text, src, model, id, ts);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_writes_resolve_against_subdirectory_and_cannot_escape_project() {
        let pm = PathMap {
            known: ["a.py".into(), "sub/a.py".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let mut g = FileGroup::default();
        push_scoped(
            &mut g,
            &pm,
            "/demo/sub",
            "a.py",
            "written_from_subdirectory",
            Src::MiniSweAgent,
            "",
            "s",
            1,
        );
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "sub/a.py");
        push_scoped(
            &mut g,
            &pm,
            "/demo/sub",
            "../../other/a.py",
            "outside_project",
            Src::MiniSweAgent,
            "",
            "s",
            1,
        );
        assert_eq!(g.evs.len(), 1);
        push_scoped(
            &mut g,
            &pm,
            "C:\\demo\\sub",
            "a.py",
            "windows_subdirectory",
            Src::MiniSweAgent,
            "",
            "s",
            1,
        );
        assert_eq!(g.evs[1].rel, "sub/a.py");
    }
    #[test]
    fn unified_patch_decodes_git_octal_unicode_paths() {
        let patch = r#"diff --git a/a.py "b/new/\344\270\255\346\226\207.py"
--- a/a.py
+++ "b/new/\344\270\255\346\226\207.py"
@@ -0,0 +1 @@
+added_line
"#;
        assert_eq!(
            diff_files(patch),
            vec![("new/中文.py".into(), "added_line\n".into())]
        );
        assert_eq!(
            git_quoted_path(r#""b/quote\"and\\slash.py""#).as_deref(),
            Some("b/quote\"and\\slash.py")
        );
    }
}
