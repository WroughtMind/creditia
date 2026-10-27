//! Tinycloud 0.3.25's native SessionStore JSONL, inspected in its official release.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    std::env::var_os("CREDITIA_TINYCLOUD_DIR")
        .or_else(|| std::env::var_os("TINYCLOUD_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".tinycloud"))
}
fn files() -> Vec<PathBuf> {
    let root = root();
    if root.is_file() {
        return vec![root];
    }
    let mut out = n::files(&root.join("projects"), |p| {
        p.extension().is_some_and(|s| s == "jsonl")
            && p.parent()
                .and_then(Path::file_name)
                .is_some_and(|s| s == "sessions")
    });
    out.extend(n::files(&root.join("sessions"), |p| {
        p.extension().is_some_and(|s| s == "jsonl")
    }));
    out.sort();
    out.dedup();
    out
}
fn brand_text(s: &str) -> String {
    // The native tool changes these spellings before writing, including in code.
    s.replace("TinyCloud", "Tinycloud")
        .replace("tinyCloud", "tinycloud")
        .replace("CloudGlue", "Cloudglue")
        .replace("cloudGlue", "cloudglue")
}
fn redacted(s: &str) -> bool {
    s.contains("[redacted]") || s.contains("[REDACTED ****]")
}
fn parse(id: &str, rows: &[Value], pm: &PathMap) -> Result<FileGroup, String> {
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen_rows = HashSet::new();
    let mut seen_calls = HashSet::new();
    let (mut start, mut end, mut prompts) = (i64::MAX, 0, 0);
    for row in rows {
        if row["type"] == "compaction" {
            continue;
        } // Compaction leaves real earlier writes in the log.
        let m = &row["message"];
        if !m.is_object() {
            continue;
        }
        let rid = row["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Tinycloud entry has no id")?;
        if !seen_rows.insert(rid) {
            continue;
        }
        let ts = crate::any_ts(&m["timestamp"])
            .or_else(|| crate::any_ts(&row["timestamp"]))
            .unwrap_or(0);
        start = start.min(ts);
        end = end.max(ts);
        if m["role"] == "user" && m["hidden"] != true {
            prompts += 1;
        }
        if m["role"] == "assistant" {
            for c in m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["type"] == "toolCall")
            {
                let cid = n::text(&c["id"]);
                if !cid.is_empty() {
                    calls.insert(cid, (c, n::text(&m["model"]), ts));
                }
            }
        }
        if m["role"] != "toolResult" {
            continue;
        }
        let cid = n::text(&m["toolCallId"]);
        let Some((call, model, call_time)) = calls.remove(cid) else {
            continue;
        };
        if m["isError"] != false || m["toolName"] != call["name"] || !seen_calls.insert(cid) {
            continue;
        }
        let a = &call["arguments"];
        let path = n::text(&a["path"]);
        // SessionStore hashes the project root, not the execution cwd. A relative
        // tool path may originate in a subdirectory; never guess it from that hash.
        if (!Path::new(path).is_absolute() && path.get(1..2) != Some(":"))
            || m["details"]["path"] != path
        {
            continue;
        }
        let receipt = m["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["type"] == "text")
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let text = match n::text(&call["name"]) {
            "write" if a["content"].is_string() => {
                let content = brand_text(n::text(&a["content"]));
                // The vendor labels JS UTF-16 length as bytes; mirror its actual contract.
                let len = content.encode_utf16().count();
                if redacted(&content)
                    || m["details"]["bytes"].as_u64() != Some(len as u64)
                    || receipt != format!("Wrote {len} bytes to {path}")
                {
                    continue;
                }
                content
            }
            "edit"
                if a["old_string"].is_string()
                    && a["new_string"].is_string()
                    && receipt == format!("Edited {path}") =>
            {
                let new = brand_text(n::text(&a["new_string"]));
                let old = n::text(&a["old_string"]);
                // JS String.replace expands $ patterns against the complete file.
                if old.is_empty() || new.contains('$') || redacted(&new) || redacted(old) {
                    continue;
                }
                crate::added_lines(old, &new)
            }
            _ => continue,
        };
        imp::push_scoped(
            &mut g,
            pm,
            "",
            path,
            &text,
            Src::Tinycloud,
            model,
            id,
            call_time,
        );
    }
    if start == i64::MAX {
        start = 0;
    }
    n::session(&mut g, pm, Src::Tinycloud, id, "", start, end, prompts, 0);
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Tinycloud, store, pm, st, files(), |p, pm| {
        parse(
            p.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
            &n::jsonl(p)?,
            pm,
        )
    });
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Tinycloud, &root(), "native projects/*/sessions/*.jsonl", files().len(), "Successful native write/edit pairs, absolute file paths, per-message model. Applies native trademark spelling changes; rejects redacted text and JS replacement patterns. Native history has no execution cwd; relative paths, media output and shell writes remain outside line attribution.", false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actual_written_text_receipts_and_utf16_length() {
        let mut rows = vec![
            json!({"id":"a","timestamp":1700000000000i64,"message":{"role":"assistant","model":"m","content":[{"type":"toolCall","id":"c","name":"write","arguments":{"path":"/demo/src/main.rs","content":"fn TinyCloud() {} // 🦀"}}]}}),
            json!({"id":"b","message":{"role":"toolResult","toolCallId":"c","toolName":"write","isError":false,"details":{"path":"/demo/src/main.rs","bytes":23},"content":[{"type":"text","text":"Wrote 23 bytes to /demo/src/main.rs"}]}}),
        ];
        let actual = brand_text("fn TinyCloud() {} // 🦀");
        let len = actual.encode_utf16().count();
        rows[1]["message"]["details"]["bytes"] = json!(len);
        rows[1]["message"]["content"][0]["text"] =
            json!(format!("Wrote {len} bytes to /demo/src/main.rs"));
        assert_eq!(parse("s", &rows, &n::project()).unwrap().evs.len(), 1);
        rows[1]["message"]["isError"] = json!(true);
        assert!(parse("s", &rows, &n::project()).unwrap().evs.is_empty());
    }
    #[test]
    fn cwd_is_not_inferred_from_a_project_slug() {
        let rows = vec![
            json!({"id":"a","message":{"role":"assistant","content":[{"type":"toolCall","id":"c","name":"edit","arguments":{"path":"src/main.rs","old_string":"old","new_string":"fn saved() {}"}}]}}),
            json!({"id":"b","message":{"role":"toolResult","toolCallId":"c","toolName":"edit","isError":false,"details":{"path":"src/main.rs"},"content":[{"type":"text","text":"Edited src/main.rs"}]}}),
        ];
        assert!(parse("s", &rows, &n::project()).unwrap().evs.is_empty());
    }
}
