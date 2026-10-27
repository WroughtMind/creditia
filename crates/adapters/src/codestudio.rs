//! Code Studio 2.0.5's persisted editing timeline and accepted text snapshots.
//! Tool transcripts alone are insufficient: hooks/fuzzy edits can alter input.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn roots() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("CREDITIA_CODESTUDIO_DIR") {
        return vec![p.into()];
    }
    #[cfg(target_os = "macos")]
    let base = crate::home().join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".config"));
    vec![
        base.join("Syncfusion Code Studio/User"),
        crate::home().join(".syncfusion-ai-server-code-studio/data/User"),
    ]
}
fn files() -> Vec<PathBuf> {
    roots()
        .iter()
        .flat_map(|root| {
            n::files(root, |p| {
                p.file_name().is_some_and(|s| s == "state.json")
                    && p.parent()
                        .and_then(Path::parent)
                        .and_then(Path::file_name)
                        .is_some_and(|s| s == "chatEditingSessions")
            })
        })
        .collect()
}
fn path(v: &Value) -> String {
    if let Some(s) = v.as_str().filter(|s| s.starts_with("file://")) {
        return crate::file_uri_path(s);
    }
    if v["scheme"] == "file" && v["authority"].as_str().unwrap_or("").is_empty() {
        return n::text(&v["path"]).into();
    }
    String::new()
}
fn content(dir: &Path, hash: &Value) -> Result<String, String> {
    let hash = hash
        .as_str()
        .filter(|s| s.len() == 7 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or("invalid Code Studio content hash")?;
    std::fs::read_to_string(dir.join("contents").join(hash))
        .map(|s| s.replace("\r\n", "\n"))
        .map_err(|e| format!("missing Code Studio snapshot {hash}: {e}"))
}
fn offset(text: &str, line: u64, col: u64) -> Option<usize> {
    let (line, col) = (line.checked_sub(1)? as usize, col.checked_sub(1)? as usize);
    let mut start = 0;
    for _ in 0..line {
        start += text.get(start..)?.find('\n')? + 1;
    }
    let current = text.get(start..)?.split('\n').next()?;
    let mut units = 0;
    for (byte, ch) in current.char_indices() {
        if units == col {
            return Some(start + byte);
        }
        units += ch.len_utf16();
    }
    (units == col).then_some(start + current.len())
}
fn apply(text: &str, edits: &[Value]) -> Result<String, String> {
    let mut ranges = vec![];
    for edit in edits {
        let r = &edit["range"];
        let pos = |l, c| {
            offset(text, r[l].as_u64().unwrap_or(0), r[c].as_u64().unwrap_or(0))
                .ok_or("invalid Code Studio UTF-16 edit range")
        };
        let (a, b) = (
            pos("startLineNumber", "startColumn")?,
            pos("endLineNumber", "endColumn")?,
        );
        if a > b {
            return Err("reversed Code Studio edit range".into());
        }
        ranges.push((
            a,
            b,
            edit["text"]
                .as_str()
                .ok_or("Code Studio edit has no text")?,
        ));
    }
    ranges.sort_by_key(|(a, b, _)| (*a, *b));
    if ranges.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err("overlapping Code Studio edits".into());
    }
    let mut out = text.to_string();
    for (a, b, new) in ranges.into_iter().rev() {
        out.replace_range(a..b, &new.replace("\r\n", "\n"));
    }
    Ok(out)
}
fn read(p: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let doc = n::json(p)?;
    if !matches!(doc["version"].as_u64(), Some(1 | 2)) {
        return Err("unsupported Code Studio editing schema".into());
    }
    let dir = p.parent().ok_or("Code Studio snapshot has no parent")?;
    let id = dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Code Studio snapshot has no session id")?;
    let ws = dir
        .parent()
        .and_then(Path::parent)
        .ok_or("Code Studio snapshot has no workspace")?;
    let cwd = n::json(&ws.join("workspace.json"))
        .ok()
        .map(|v| path(&v["folder"]))
        .unwrap_or_default();
    let chat_path = ws.join("chatSessions").join(id);
    let chat = if chat_path.with_extension("jsonl").is_file() {
        let s = std::fs::read_to_string(chat_path.with_extension("jsonl"))
            .map_err(|e| e.to_string())?;
        crate::copilot::replay(&s).ok_or("incomplete Code Studio chat snapshot")?
    } else if chat_path.with_extension("json").is_file() {
        n::json(&chat_path.with_extension("json"))?
    } else {
        Value::Null
    };
    let requests: HashMap<_, _> = chat["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["requestId"].as_str().map(|id| (id, r)))
        .collect();
    parse(&doc, dir, id, &cwd, &requests, pm)
}
fn parse(
    doc: &Value,
    dir: &Path,
    id: &str,
    cwd: &str,
    requests: &HashMap<&str, &Value>,
    pm: &PathMap,
) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    let epoch = doc["timeline"]["currentEpoch"]
        .as_u64()
        .ok_or("missing Code Studio timeline epoch")?;
    let ops = doc["timeline"]["operations"]
        .as_array()
        .ok_or("missing Code Studio operations")?;
    let initial = doc["initialFileContents"]
        .as_array()
        .ok_or("missing Code Studio initial contents")?;
    let entries = doc["recentSnapshot"]["entries"]
        .as_array()
        .ok_or("missing Code Studio final snapshot")?;
    let mut start = i64::MAX;
    let mut end = 0;
    for r in requests.values() {
        if let Some(ts) = crate::any_ts(&r["timestamp"]) {
            start = start.min(ts);
            end = end.max(ts);
        }
    }
    for entry in entries {
        if entry["state"] != 1 || entry["isDeleted"] == true {
            continue;
        }
        let dest = path(&entry["resource"]);
        if dest.is_empty()
            || !pm.in_project(&dest)
            || n::text(&entry["snapshotUri"]).starts_with("chat-editing-notebook")
        {
            continue;
        }
        // Renames/notebooks require separate reconstruction; never merge their histories.
        if ops.iter().any(|op| {
            op["type"] == "rename" && (path(&op["oldUri"]) == dest || path(&op["newUri"]) == dest)
        }) {
            continue;
        }
        let baseline = initial
            .iter()
            .find(|v| path(&v[0]) == dest)
            .ok_or("missing Code Studio file baseline")?;
        let mut text = content(dir, &baseline[1])?;
        let expected = content(dir, &entry["currentHash"])?;
        let mut events = vec![];
        let mut last_epoch = None;
        let mut unsupported = false;
        for op in ops.iter().filter(|op| path(&op["uri"]) == dest) {
            let e = op["epoch"]
                .as_u64()
                .ok_or("invalid Code Studio operation epoch")?;
            if e >= epoch {
                continue;
            }
            if last_epoch.is_some_and(|last| e <= last) {
                return Err("unordered Code Studio timeline".into());
            }
            last_epoch = Some(e);
            if !op["cellIndex"].is_null() {
                unsupported = true;
                break;
            }
            let after = match n::text(&op["type"]) {
                "create" => op["initialContent"]
                    .as_str()
                    .ok_or("Code Studio create has no content")?
                    .replace("\r\n", "\n"),
                "textEdit" => apply(
                    &text,
                    op["edits"]
                        .as_array()
                        .ok_or("Code Studio operation has no edits")?,
                )?,
                _ => {
                    unsupported = true;
                    break;
                }
            };
            let added = crate::added_lines(&text, &after);
            let rid = n::text(&op["requestId"]);
            let r = requests.get(rid).copied().unwrap_or(&Value::Null);
            let model = n::text(&r["modelId"]).to_string();
            let ts = crate::any_ts(&r["timestamp"]).unwrap_or(0);
            if !rid.is_empty() {
                events.push((added, model, ts));
            }
            text = after;
        }
        if unsupported {
            continue;
        }
        // Operations are recorded before application. Only an exact accepted final
        // snapshot proves their replay; manual/conflicting edits cannot be credited.
        if text != expected {
            return Err(format!(
                "Code Studio timeline does not match accepted snapshot: {dest}"
            ));
        }
        for (added, model, ts) in events {
            imp::push_scoped(
                &mut g,
                pm,
                cwd,
                &dest,
                &added,
                Src::Codestudio,
                &model,
                id,
                ts,
            );
        }
    }
    if start == i64::MAX {
        start = 0;
    }
    n::session(
        &mut g,
        pm,
        Src::Codestudio,
        id,
        cwd,
        start,
        end,
        requests.len() as i64,
        0,
    );
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Codestudio, store, pm, st, files(), read);
}
pub fn discover() -> Vec<Activity> {
    files()
        .into_iter()
        .filter_map(|p| {
            let ws = p.parent()?.parent()?.parent()?;
            let cwd = path(&n::json(&ws.join("workspace.json")).ok()?["folder"]);
            (!cwd.is_empty()).then(|| imp::activity(Src::Codestudio, cwd, None, 0))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Codestudio,&roots()[0],"chatEditingSessions/*/state.json + contents + chatSessions",files().len(),"Accepted text editing timelines reconstructed and checked against the final stored snapshot; model/time from each request. Pending/rejected edits, mismatches, notebooks and renamed files are excluded. Tool arguments/Chronicle file lists alone are not write evidence.",false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn vscode_positions_are_utf16_and_invalid_ranges_fail() {
        let edit = json!({"range":{"startLineNumber":1,"startColumn":3,"endLineNumber":1,"endColumn":4},"text":"B"});
        assert_eq!(apply("🦀a", &[edit]).unwrap(), "🦀B");
        assert!(apply("🦀a",&[json!({"range":{"startLineNumber":1,"startColumn":2,"endLineNumber":1,"endColumn":4},"text":"B"})]).is_err());
    }
}
