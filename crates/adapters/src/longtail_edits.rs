//! Success receipts and actual results verified against vendor-owned code.
//! Parent module owns path discovery and session metadata.
use super::{text, Record};
use creditia_core::model::{FileGroup, PathMap, Src};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn parsed(v: &Value) -> Value {
    v.as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| v.clone())
}
fn role(v: &Value, want: &str) -> bool {
    v["role"]
        .as_str()
        .is_some_and(|r| r.eq_ignore_ascii_case(want))
}
fn push(r: &Record, pm: &PathMap, g: &mut FileGroup, p: &str, added: &str, model: &str, at: i64) {
    // Remote MCP paths cannot prove a write inside this local repository.
    if p.contains(":/") && !p.starts_with("file://") && !Path::new(p).is_absolute() {
        return;
    }
    let path = crate::file_uri_path(p);
    crate::push_text(
        &mut g.evs,
        pm,
        &crate::pi::abs(&r.cwd, &path),
        added,
        r.src,
        model,
        &r.id,
        at,
    );
}
fn explicit_error(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.contains_key("success") && v["success"] != true)
        || !v["error"].is_null()
        || !v["error_type"].is_null()
        || !v["failure_kind"].is_null()
        || v["success"] == false
        || v["blocked"] == true
        || v["isError"] == true
        || v["status"]
            .as_str()
            .is_some_and(|s| !matches!(s, "success" | "completed" | "ok"))
}

// Minion's full text observations include an exact total and every numbered
// line. Partial or output-truncated reads cannot establish a before snapshot.
fn minion_before(r: &Record, args: &Value, receipt: &str) -> Option<String> {
    if !args["offset"].is_null() || !args["limit"].is_null() { return None; }
    let path = crate::pi::abs(&r.cwd, args["file_path"].as_str()?);
    let prefix = format!("File: {path}\n");
    let rest = receipt.strip_prefix(&prefix)?;
    if rest == "(empty file)" { return Some(String::new()); }
    let (total, body) = rest.strip_prefix("Total lines: ")?.split_once("\n\n")?;
    let total: usize = total.parse().ok()?;
    let mut content = String::new(); let mut count = 0;
    for line in body.split_inclusive('\n') {
        let (number, raw) = line.split_once('→')?;
        count += 1;
        if number.trim().parse::<usize>().ok()? != count { return None; }
        content.push_str(raw);
    }
    (count == total).then_some(content)
}
fn dirac_hash(content: &str) -> String {
    let value = content.encode_utf16().fold(2_166_136_261_u32, |h, c| (h ^ u32::from(c)).wrapping_mul(16_777_619));
    format!("{value:08x}")
}

/// VTCode eec794a / Minion 26e373e + minionx 0.1.23 / Autohand d01ef07 /
/// Stakpak 760cd2b all retain identifiable OpenAI-style call/result history.
pub(super) fn openai(r: &Record, pm: &PathMap, g: &mut FileGroup, seen: &mut HashSet<String>) {
    let mut calls: HashMap<String, (String, Value, String, i64, String)> = HashMap::new();
    let mut snapshots: HashMap<String, String> = HashMap::new();
    for m in &r.rows {
        if role(m, "assistant") {
            // Tool IDs belong to an assistant batch, not the entire conversation.
            calls.clear();
            for c in m["tool_calls"]
                .as_array()
                .or_else(|| m["toolCalls"].as_array())
                .into_iter()
                .flatten()
            {
                let Some(id) = c["id"].as_str() else { continue };
                let name = c["function"]["name"]
                    .as_str()
                    .or_else(|| c["tool"].as_str())
                    .unwrap_or("");
                let a = if c["function"].is_object() {
                    parsed(&c["function"]["arguments"])
                } else {
                    parsed(&c["args"])
                };
                let model = if r.src == Src::Stakpak {
                    m["model"]["id"].as_str().unwrap_or("")
                } else {
                    ""
                };
                let at = crate::any_ts(&m["completed_at"])
                    .or_else(|| crate::any_ts(&m["created_at"]))
                    .unwrap_or(0);
                let identity = format!("{id}:{}:{name}:{a}", m["id"].as_str().unwrap_or(""));
                calls.insert(id.into(), (name.into(), a, model.into(), at, identity));
            }
        }
        if !role(m, "tool") || m["_meta"]["stream"].is_string() {
            continue;
        }
        let Some((name, a, model, call_at, identity)) =
            calls.remove(m["tool_call_id"].as_str().unwrap_or(""))
        else {
            continue;
        };
        if m["is_error"] == true || m["isError"] == true { snapshots.clear(); continue; }
        if seen.contains(&identity) {
            continue;
        }
        let receipt = text(&m["content"]);
        let at = crate::any_ts(&m["metadata"]["timestamp"])
            .or_else(|| crate::any_ts(&m["timestamp"]))
            .or_else(|| crate::any_ts(&m["created_at"]))
            .unwrap_or(call_at);
        let before = g.evs.len();
        match r.src {
            Src::MinionCode => {
                let p = a["file_path"].as_str().unwrap_or("");
                let key = crate::pi::abs(&r.cwd, p);
                if name == "file_read" {
                    snapshots.remove(&key);
                    if let Some(content) = minion_before(r, &a, &receipt) { snapshots.insert(key, content); }
                    continue;
                }
                if !matches!(name.as_str(), "file_write" | "file_edit" | "multi_edit") { snapshots.clear(); }
                // A failed write can still have truncated the file. Do not
                // retain an earlier snapshot across an attempted mutation.
                let old = snapshots.remove(&key);
                let ok = match name.as_str() {
                    "file_write" => receipt.starts_with("Successfully wrote to file: "),
                    "file_edit" => {
                        receipt.starts_with("Successfully created new file: ")
                            || (receipt.starts_with("The file ")
                                && receipt
                                    .contains(" has been updated. Here's the result of the edit:"))
                    }
                    "multi_edit" => {
                        receipt.starts_with("Successfully applied ")
                            && receipt.contains(" edits to ")
                    }
                    _ => false,
                };
                if !ok || p.is_empty() {
                    continue;
                }
                let added = if name == "file_write" {
                    let Some(content) = a["content"].as_str() else { continue; };
                    snapshots.insert(key, content.into());
                    let Some(old) = old else { continue; };
                    crate::added_lines(&old, content)
                } else if name == "multi_edit" {
                    a["edits"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|e| {
                            crate::added_lines(
                                e["old_string"].as_str().unwrap_or(""),
                                e["new_string"].as_str().unwrap_or(""),
                            )
                        })
                        .collect()
                } else {
                    crate::added_lines(
                        a["old_string"].as_str().unwrap_or(""),
                        a["new_string"].as_str().unwrap_or(""),
                    )
                };
                push(r, pm, g, p, &added, "", at);
            }
            // Ordinary file tools also emit success-looking preview diffs.
            Src::Autohand if name == "git_apply_patch" && receipt == "Applied git patch." => {
                let patch = a["patch"]
                    .as_str()
                    .or_else(|| a["diff"].as_str())
                    .unwrap_or("");
                for (p, added) in crate::trace_import::diff_files(patch) {
                    push(r, pm, g, &p, &added, "", at);
                }
            }
            Src::VtCode => {
                let out: Value = serde_json::from_str(&receipt).unwrap_or_default();
                if !out.is_object()
                    || explicit_error(&out)
                    || out["reused_recent_result"] == true
                    || out["reused_spooled_output"] == true
                {
                    continue;
                }
                // Current model-history compaction removes success:true/status:
                // success and the message. The write-specific actual byte count
                // and result diff survive; don't require a discarded flag.
                let is_write =
                    name == "write_file" || (name == "unified_file" && a["action"] == "write");
                if !is_write || !out["bytes_written"].is_u64() || out["skipped"] == true {
                    continue;
                }
                let p = out["path"]
                    .as_str()
                    .or_else(|| a["path"].as_str())
                    .unwrap_or("");
                if p.is_empty() {
                    continue;
                }
                if let Some(diffs) = out["diff"].as_array() {
                    for d in diffs {
                        if d["skipped"] != true && d["suppressed"] != true {
                            let added = crate::added_from_diff(d["content"].as_str().unwrap_or(""));
                            push(r, pm, g, d["path"].as_str().unwrap_or(p), &added, "", at);
                        }
                    }
                } else if out["diff_preview"].is_object()
                    && out["diff_preview"]["skipped"] != true
                    && out["diff_preview"]["suppressed"] != true
                {
                    let added = crate::added_from_diff(
                        out["diff_preview"]["content"].as_str().unwrap_or(""),
                    );
                    push(r, pm, g, p, &added, "", at);
                } else if let Some(content) = a["content"].as_str() {
                    if matches!(out["mode"].as_str(), Some("append" | "fail_if_exists" | "skip_if_exists"))
                        && out["bytes_written"].as_u64() == Some(content.len() as u64) {
                        push(r, pm, g, p, content, "", at)
                    }
                }
            }
            Src::Stakpak => {
                let p = a["path"].as_str().unwrap_or("");
                if p.is_empty() {
                    continue;
                }
                if name == "create" && receipt.starts_with("Successfully created file ") {
                    push(
                        r,
                        pm,
                        g,
                        p,
                        a["file_text"].as_str().unwrap_or(""),
                        &model,
                        at,
                    );
                } else if name == "str_replace"
                    && receipt.starts_with("Successfully replaced ")
                    && receipt.contains(" occurrences of text (local)\n\n```diff\n")
                {
                    if let Some(diff) = receipt
                        .split_once("```diff\n")
                        .and_then(|(_, s)| s.split_once("\n```"))
                    {
                        push(r, pm, g, p, &crate::added_from_diff(diff.0), &model, at);
                    }
                }
            }
            _ => {}
        }
        if g.evs.len() > before {
            seen.insert(identity);
        }
    }
}

/// IBM Bob 2.0.8 serializes the actual pushEdit(before,after) into _meta.changes
/// only after writeFile/applyApprovedEdit. Rejected/failed signature results must
/// never consume a proposed approval diff or the original request parameters.
pub(super) fn bob(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    for m in &r.rows {
        if !role(m, "tool") || m["toolUsage"]["signature"]["isError"] == true {
            continue;
        }
        let Some(signature) = m["toolUsage"]["signature"].as_object() else {
            continue;
        };
        if signature.get("id").and_then(Value::as_str).is_none() {
            continue;
        }
        let at = crate::any_ts(&m["_meta"]["timestamp"]).unwrap_or(0);
        for (uri, change) in m["_meta"]["changes"].as_object().into_iter().flatten() {
            if let (Some(before), Some(after)) =
                (change["before"].as_str(), change["after"].as_str())
            {
                push(r, pm, g, uri, &crate::added_lines(before, after), "", at);
            }
        }
    }
}

fn complete_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .split_inclusive('\n')
        .filter(|s| s.ends_with('\n'))
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect()
}
/// Dirac 3615882 API conversation replay, matching its framed baseline/offset
/// semantics. Reading only the asynchronously updated JSON mirror can miss the
/// last successful tools, so prefer the baseline and incremental journal.
pub(super) fn dirac_history(task: &Path) -> Vec<Value> {
    let baseline = task.join("api_conversation_history.baseline.jsonl");
    let (mut rows, mut offset) = if baseline.is_file() {
        let framed = complete_jsonl(&baseline);
        let Some(header) = framed.first().filter(|r| r["type"] == "baseline") else {
            return vec![];
        };
        let Some(offset) = header["offset"].as_i64() else {
            return vec![];
        };
        if framed.iter().skip(1).any(|r| r["type"] != "message") {
            return vec![];
        }
        (
            framed
                .into_iter()
                .skip(1)
                .map(|r| r["message"].clone())
                .collect::<Vec<_>>(),
            offset,
        )
    } else {
        (
            super::read(&task.join("api_conversation_history.json"))
                .as_array()
                .cloned()
                .unwrap_or_default(),
            -1,
        )
    };
    for op in complete_jsonl(&task.join("api_conversation_history.jsonl")) {
        let Some(next) = op["offset"].as_i64() else {
            return vec![];
        };
        if next <= offset {
            continue;
        }
        if next != offset + 1 && op["type"] != "reset" {
            return vec![];
        }
        offset = next;
        match op["type"].as_str() {
            Some("reset") => {
                let Some(a) = op["messages"].as_array() else {
                    return vec![];
                };
                rows = a.clone();
            }
            Some("append_message") => rows.push(op["message"].clone()),
            Some("append_user_content") => {
                let Some(last) = rows.last_mut().filter(|r| r["role"] == "user") else {
                    return vec![];
                };
                if let Some(s) = last["content"].as_str() {
                    last["content"] = serde_json::json!([{"type":"text","text":s},op["content"]]);
                } else if let Some(a) = last["content"].as_array_mut() {
                    a.push(op["content"].clone())
                } else {
                    return vec![];
                }
            }
            _ => return vec![],
        }
    }
    rows
}

fn dirac_write_content(raw: &str, model: &str, path: &str) -> String {
    let mut content = raw
        .split_inclusive('\n')
        .map(|line| {
            if let Some((anchor, body)) = line.split_once('§') {
                if anchor.starts_with(|c: char| c.is_ascii_uppercase())
                    && anchor.chars().all(|c| c.is_ascii_alphabetic())
                {
                    return body.to_string();
                }
            }
            line.to_string()
        })
        .collect::<String>();
    if content.starts_with("```") {
        content = content
            .split_once('\n')
            .map(|(_, s)| s.trim().to_string())
            .unwrap_or_default();
    }
    if content.ends_with("```") {
        content = content
            .rsplit_once('\n')
            .map(|(s, _)| s.trim().to_string())
            .unwrap_or_default();
    }
    if !model.contains("claude") {
        if !path.to_lowercase().ends_with(".xml") {
            content = content
                .replace("&gt;", ">")
                .replace("&lt;", "<")
                .replace("&quot;", "\"")
                .replace("&amp;", "&")
                .replace("&apos;", "'");
        }
        content = content.replace('\u{fffd}', "");
    }
    content
}
pub(super) fn dirac(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    let mut calls: HashMap<&str, (&str, &Value, &str, i64)> = HashMap::new();
    let mut snapshots: HashMap<String, String> = HashMap::new();
    for m in &r.rows {
        if role(m, "assistant") {
            for c in m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["type"] == "tool_use")
            {
                if let (Some(id), Some(name)) = (c["id"].as_str(), c["name"].as_str()) {
                    calls.insert(
                        id,
                        (
                            name,
                            &c["input"],
                            m["modelInfo"]["modelId"].as_str().unwrap_or(""),
                            crate::any_ts(&m["ts"]).unwrap_or(0),
                        ),
                    );
                }
            }
        }
        if !role(m, "user") {
            continue;
        }
        for result in m["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_result")
        {
            let Some((name, a, model, call_at)) =
                calls.remove(result["tool_use_id"].as_str().unwrap_or(""))
            else {
                continue;
            };
            if result["is_error"] == true {
                snapshots.clear();
                continue;
            }
            let receipt = text(&result["content"]);
            let at = crate::any_ts(&m["ts"]).unwrap_or(call_at);
            if name == "read_file" {
                // The raw, unanchored single-file branch emits the complete
                // source after this hash header. Rich/extracted files and
                // ranges have different contracts and cannot be snapshots.
                let Some(paths) = a["paths"].as_array().filter(|p| p.len() == 1) else { continue; };
                let Some(path) = paths[0].as_str() else { continue; };
                let key = crate::pi::abs(&r.cwd, path); snapshots.remove(&key);
                let ext = Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if a["include_anchors"] == true || !a["start_line"].is_null() || !a["end_line"].is_null()
                    || matches!(ext.as_str(), "pdf" | "docx" | "xlsx" | "png" | "jpg" | "jpeg" | "webp" | "ipynb") { continue; }
                if let Some((header, content)) = receipt.split_once('\n') {
                    if header.strip_prefix("[File Hash: ").and_then(|h| h.strip_suffix(']')) == Some(dirac_hash(content).as_str()) { snapshots.insert(key, content.into()); }
                }
                continue;
            }
            if !matches!(name, "write_to_file" | "edit_file") { snapshots.clear(); }
            if name == "write_to_file" {
                let Some(p) = a["path"].as_str() else {
                    continue;
                };
                let key = crate::pi::abs(&r.cwd, p);
                let before = snapshots.remove(&key);
                // A reviewed file may contain manual edits and editor formatting.
                // Those success receipts explicitly flag the changed content.
                if receipt.lines().next() != Some(&format!("Saved {}.", p.replace('\\', "/")))
                    || receipt.contains("User changes (preserve these):")
                    || receipt.contains("Auto-formatting:")
                {
                    continue;
                }
                if let Some(content) = a["content"].as_str() {
                    let after = dirac_write_content(content, model, p);
                    if let Some(before) = before { push(r, pm, g, p, &crate::added_lines(&before, &after), model, at); }
                    snapshots.insert(key, after);
                }
            } else if name == "edit_file" {
                for p in a["files"].as_array().into_iter().flatten().filter_map(|f| f["path"].as_str()) { snapshots.remove(&crate::pi::abs(&r.cwd, p)); }
                let Some(files) = a["files"].as_array().filter(|f| f.len() == 1) else {
                    continue;
                };
                let Some(p) = files[0]["path"].as_str() else {
                    continue;
                };
                if !receipt.starts_with("Applied ")
                    || !receipt
                        .lines()
                        .next()
                        .unwrap_or("")
                        .contains(" edit(s) successfully")
                    || receipt.contains("User modified this file")
                    || receipt.contains("Auto-formatting applied.")
                {
                    continue;
                }
                if let Some((_, body)) = receipt.split_once("Diff (unanchored):\n\n") {
                    let diff = body.split("\n\n---\n\n").next().unwrap_or("");
                    push(r, pm, g, p, &crate::added_from_diff(diff), model, at);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into(), "b.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    fn record(src: Src, rows: Vec<Value>) -> Record {
        Record {
            src,
            id: "native".into(),
            cwd: "/repo/".into(),
            model: "final-selection-must-not-label-history".into(),
            provider: None,
            start: 0,
            end: 0,
            rows,
        }
    }
    fn call(id: &str, name: &str, args: Value) -> Value {
        json!({"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}})
    }
    fn result(id: &str, content: impl Into<Value>) -> Value {
        json!({"role":"tool","tool_call_id":id,"content":content.into()})
    }
    fn run(r: &Record) -> FileGroup {
        let mut g = FileGroup::default();
        openai(r, &pm(), &mut g, &mut HashSet::new());
        g
    }
    #[test]
    fn vtcode_actual_compacted_write_diff_and_errors() {
        let args = json!({"path":"a.rs","content":"context_line\nnew_actual_line\n"});
        let rows=vec![json!({"role":"Assistant","tool_calls":[call("ok","write_file",args.clone()),call("bad","write_file",args.clone()),call("skip","write_file",args.clone()),call("reuse","write_file",args.clone())]}),
            json!({"role":"Tool","tool_call_id":"ok","metadata":{"timestamp":1780000000000_i64},"content":json!({"tool":"write_file","path":"a.rs","mode":"overwrite","bytes_written":29,"diff":[{"path":"a.rs","operation":"updated","content":"@@ -1 +1,2 @@\n context_line\n+new_actual_line","skipped":false}]}).to_string()}),
            result("bad",json!({"success":false,"error":"disk full","path":"a.rs","mode":"overwrite","bytes_written":29,"diff":[{"content":"+failed_line"}]}).to_string()),
            result("skip",json!({"success":true,"skipped":true,"message":"File already exists","diff":[]}).to_string()),
            result("reuse",json!({"path":"a.rs","mode":"overwrite","bytes_written":29,"reused_recent_result":true}).to_string())];
        let g = run(&record(Src::VtCode, rows));
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].ts, 1780000000);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key(
                "a.rs",
                &creditia_core::hash::normalize("new_actual_line", ".rs")
            )
        );
    }
    #[test]
    fn minion_persisted_history_matches_native_receipts_not_proposals() {
        let rows=vec![json!({"role":"assistant","tool_calls":[call("read","file_read",json!({"file_path":"a.rs"}))]}),result("read","File: /repo/a.rs\n(empty file)"),json!({"role":"assistant","tool_calls":[call("write","file_write",json!({"file_path":"a.rs","content":"accepted_write"})),call("bad","file_write",json!({"file_path":"a.rs","content":"rejected_write"})),call("edit","file_edit",json!({"file_path":"a.rs","old_string":"old","new_string":"actual_edit"})),call("multi","multi_edit",json!({"file_path":"a.rs","edits":[{"old_string":"old","new_string":"actual_multi"}]}))]}),
            result("write","Successfully wrote to file: /repo/a.rs (14 characters)"),result("bad","Error writing file: Permission denied"),result("edit","The file /repo/a.rs has been updated. Here's the result of the edit:\n1 | actual_edit"),result("multi","Successfully applied 1 edits to /repo/a.rs\nEdit 1: Replaced 1 occurrence(s)")];
        let g = run(&record(Src::MinionCode, rows));
        assert_eq!(g.evs.len(), 3);
        assert!(g.evs.iter().all(|e| e.model.is_empty() && e.ts == 0));
    }
    #[test]
    fn autohand_git_patch_receipt_excludes_preview_and_failed_apply() {
        let patch = "--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+actual_git_patch\n";
        let rows = vec![
            json!({"role":"assistant","toolCalls":[{"id":"preview","tool":"write_file","args":{"path":"a.rs","content":"preview_only"}},{"id":"git","tool":"git_apply_patch","args":{"patch":patch}},{"id":"bad","tool":"git_apply_patch","args":{"patch":patch}}]}),
            result("preview", "  Added 1 line, removed 0 lines\n+preview_only"),
            json!({"role":"tool","tool_call_id":"git","timestamp":"2026-05-28T00:00:00Z","content":"Applied git patch."}),
            result("bad", "git apply failed"),
        ];
        let g = run(&record(Src::Autohand, rows));
        assert_eq!(g.evs.len(), 1);
        assert!(g.evs[0].ts > 0);
        assert!(g.evs[0].model.is_empty());
    }
    #[test]
    fn stakpak_actual_local_diff_retains_call_model_and_dedups_checkpoints() {
        let rows=vec![json!({"id":"m1","role":"assistant","model":{"provider":"p","id":"first"},"tool_calls":[call("create","create",json!({"path":"a.rs","file_text":"actual_created"})),call("failed","create",json!({"path":"a.rs","file_text":"failed_created"}))]}),result("create","Successfully created file /repo/a.rs with 1 lines"),result("failed","FILE_EXISTS\nFile already exists: /repo/a.rs"),json!({"id":"m2","role":"assistant","model":{"id":"second"},"tool_calls":[call("replace","str_replace",json!({"path":"a.rs","old_str":"requested_old","new_str":"requested_wrong"}))]}),result("replace","Successfully replaced 1 occurrences of text (local)\n\n```diff\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old_actual\n+actual_result\n```" )];
        let r = record(Src::Stakpak, rows);
        let mut seen = HashSet::new();
        let mut g = FileGroup::default();
        openai(&r, &pm(), &mut g, &mut seen);
        openai(&r, &pm(), &mut g, &mut seen);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "first");
        assert_eq!(g.evs[1].model, "second");
        assert_eq!(
            g.evs[1].key,
            creditia_core::hash::line_key(
                "a.rs",
                &creditia_core::hash::normalize("actual_result", ".rs")
            )
        );
    }
    #[test]
    fn bob_actual_post_write_metadata_excludes_approval_previews_and_errors() {
        let rows = vec![
            json!({"role":"assistant","toolCalls":[{"id":"pending","name":"write_file","arguments":{"path":"a.rs","content":"pending_only"}}],"_meta":{"changes":{"file:///repo/a.rs":{"before":"","after":"approval_preview"}}}}),
            json!({"role":"tool","toolUsage":{"signature":{"id":"ok","name":"write_file","arguments":{"path":"a.rs","content":"requested_wrong"}}},"_meta":{"timestamp":1780000000000_i64,"changes":{"file:///repo/a.rs":{"before":"context","after":"context\nactual_after_review"}}}}),
            json!({"role":"tool","toolUsage":{"signature":{"id":"bad","name":"write_file","isError":true}},"_meta":{"changes":{"file:///repo/a.rs":{"before":"","after":"failed_only"}}}}),
        ];
        let r = record(Src::Bob, rows);
        let mut g = FileGroup::default();
        bob(&r, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].ts, 1780000000);
        assert!(g.evs[0].model.is_empty());
    }
    #[test]
    fn dirac_paired_native_write_edit_and_manual_failure_boundaries() {
        let rows = vec![
            json!({"role":"assistant","content":[{"type":"tool_use","id":"read","name":"read_file","input":{"paths":["a.rs"]}}]}),
            json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"read","content":format!("[File Hash: {}]\n",dirac_hash(""))}]}),
            json!({"role":"assistant","ts":1780000000000_i64,"modelInfo":{"modelId":"claude-first"},"content":[{"type":"tool_use","id":"ok","name":"write_to_file","input":{"path":"a.rs","content":"A§actual_written"}},{"type":"tool_use","id":"manual","name":"write_to_file","input":{"path":"b.rs","content":"manual_wrong"}},{"type":"tool_use","id":"bad","name":"write_to_file","input":{"path":"b.rs","content":"failed_wrong"}}]}),
            json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"ok","content":"Saved a.rs."},{"type":"tool_result","tool_use_id":"manual","content":"Saved b.rs.\n\nUser changes (preserve these):\nUser made manual changes in the editor."},{"type":"tool_result","tool_use_id":"bad","is_error":true,"content":"Saved b.rs."}]}),
            json!({"role":"assistant","modelInfo":{"modelId":"second"},"content":[{"type":"tool_use","id":"edit","name":"edit_file","input":{"files":[{"path":"a.rs","edits":[{"anchor":"A","edit_type":"replace","text":"requested_changed"}]}]}}]}),
            json!({"role":"user","ts":1780000010000_i64,"content":[{"type":"tool_result","tool_use_id":"edit","content":"Applied 1 edit(s) successfully (+1, -1 lines).\n\nDiff (unanchored):\n\n-old\n+actual_edit_result"}]}),
        ];
        let r = record(Src::Dirac, rows);
        let mut g = FileGroup::default();
        dirac(&r, &pm(), &mut g);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "claude-first");
        assert_eq!(g.evs[1].model, "second");
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key(
                "a.rs",
                &creditia_core::hash::normalize("actual_written", ".rs")
            )
        );
        assert_eq!(
            dirac_write_content("```rs\nB§value = &lt;x&gt;\n```", "other", "a.rs"),
            "value = <x>"
        );
    }
    #[test]
    fn successful_overwrites_need_complete_before_and_never_claim_preserved_human_lines() {
        let after = "human_original_line\nmodel_new_line\n";
        for (read_args, read_result, want) in [
            (json!({"file_path":"a.rs"}), "File: /repo/a.rs\nTotal lines: 1\n\n    1→human_original_line\n", 1),
            (json!({"file_path":"a.rs","limit":1}), "File: /repo/a.rs\nTotal lines: 2\n\n    1→human_original_line\n", 0),
            (json!({"file_path":"a.rs"}), "File: /repo/a.rs\nTotal lines: 2\n\n    1→human_original_line\n", 0),
        ] {
            let r = record(Src::MinionCode, vec![json!({"role":"assistant","tool_calls":[call("read","file_read",read_args)]}), result("read",read_result),json!({"role":"assistant","tool_calls":[call("write","file_write",json!({"file_path":"a.rs","content":after}))]}), result("write","Successfully wrote to file: /repo/a.rs (35 characters)")]);
            let g = run(&r); assert_eq!(g.evs.len(),want);
            assert!(g.evs.iter().all(|e| e.key != creditia_core::hash::line_key("a.rs", "human_original_line")));
        }
        for before in [None, Some("human_original_line\n")] {
            let mut rows = vec![];
            if let Some(before) = before {
                rows.push(json!({"role":"assistant","content":[{"type":"tool_use","id":"r","name":"read_file","input":{"paths":["a.rs"]}}]}));
                rows.push(json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"r","content":format!("[File Hash: {}]\n{before}",dirac_hash(before))}]}));
            }
            rows.extend([json!({"role":"assistant","content":[{"type":"tool_use","id":"w","name":"write_to_file","input":{"path":"a.rs","content":after}}]}), json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"w","content":"Saved a.rs."}]})]);
            let mut g=FileGroup::default();dirac(&record(Src::Dirac,rows),&pm(),&mut g);
            assert_eq!(g.evs.len(),usize::from(before.is_some()));
            assert!(g.evs.iter().all(|e| e.key != creditia_core::hash::line_key("a.rs", "human_original_line")));
        }
        let r = record(Src::VtCode, vec![json!({"role":"Assistant","tool_calls":[call("w","write_file",json!({"path":"a.rs","content":after}))]}),result("w", json!({"path":"a.rs","mode":"overwrite","bytes_written":after.len()}).to_string())]);
        assert!(run(&r).evs.is_empty(),"byte count alone cannot prove which overwritten lines were added");
    }
    #[test]
    fn dirac_replays_baseline_and_newer_operations_without_partial_tail() {
        let p = std::env::temp_dir().join(format!("creditia-dirac-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("api_conversation_history.baseline.jsonl"),"{\"type\":\"baseline\",\"offset\":3}\n{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":[]}}\n").unwrap();
        std::fs::write(p.join("api_conversation_history.jsonl"),"{\"type\":\"append_message\",\"offset\":3,\"message\":{\"role\":\"user\",\"content\":\"already in baseline\"}}\n{\"type\":\"append_message\",\"offset\":4,\"message\":{\"role\":\"user\",\"content\":[]}}\n{\"type\":\"append_user_content\",\"offset\":5,\"content\":{\"type\":\"tool_result\",\"tool_use_id\":\"ok\",\"content\":\"Saved a.rs.\"}}\n{\"type\":\"reset\",\"offset\":6,\"messages\":[]}").unwrap();
        let rows = dirac_history(&p);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["content"][0]["tool_use_id"], "ok");
        std::fs::remove_dir_all(p).unwrap();
    }
}
