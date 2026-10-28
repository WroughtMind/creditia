//! Native Codex stores also used by inference.sh's agentprotocol and MCPJam.
//! Never synthesize a second contribution under the orchestration product name.
use super::*;
pub(super) use crate::harness_paths::{cwd, target};
use crate::harness_paths::{directories, mcpjam_sessions};
use std::collections::{BTreeMap, HashSet};

pub(super) fn roots() -> Vec<PathBuf> {
    let mut out = vec![source_dir(Src::Codex)];
    // Only the official session layout; do not traverse project symlinks or
    // inspect bridge configuration, credentials, or arbitrary RPC exports.
    for session in mcpjam_sessions() {
        for run in directories(&session.join("work/.agent-runs")) {
            let home = run.join("codex-home");
            if home.is_dir() && !home.is_symlink() && !out.contains(&home) {
                out.push(home);
            }
        }
    }
    out
}

/// Reconciliation needs a complete snapshot, including rows the edit parser
/// does not use. A read/decode error or interrupted JSON row is not an empty
/// history and must leave previously collected evidence available.
pub(super) fn read_rollout(path: &Path) -> Result<String, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let bytes = if path.extension().is_some_and(|ext| ext == "zst") {
        zstd::stream::decode_all(&raw[..]).map_err(|e| e.to_string())?
    } else {
        raw
    };
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    if text.trim().is_empty() {
        return Err("empty rollout snapshot".into());
    }
    for (index, line) in text.lines().enumerate() {
        if !line.trim().is_empty() {
            serde_json::from_str::<Value>(line)
                .map_err(|e| format!("invalid rollout row {}: {e}", index + 1))?;
        }
    }
    Ok(text)
}

pub(super) fn all(warnings: &mut Vec<String>) -> Vec<(Thread, Option<PathBuf>)> {
    let mut selected: BTreeMap<String, (Thread, Option<PathBuf>, bool, bool)> = BTreeMap::new();
    for base in roots() {
        let files = rollout_files(&base);
        let mut indexed = HashSet::new();
        let mut rows = vec![];
        if base.join("state_5.sqlite").exists() {
            match threads(&base) {
                Ok(threads) => {
                    for thread in threads {
                        let name = Path::new(&thread.rollout)
                            .file_name()
                            .map(|s| s.to_string_lossy().replace(".zst", ""))
                            .unwrap_or_default();
                        let file = files.get(&name).cloned();
                        if file.is_some() {
                            indexed.insert(name);
                        }
                        rows.push((thread, file, true));
                    }
                }
                Err(e) => warnings.push(e),
            }
        }
        // agentprotocol's same-agent export preserves native rollout rows but
        // does not create the optional SQLite index. Read their own header.
        let mut orphaned: Vec<_> = files
            .iter()
            .filter(|(name, _)| !indexed.contains(*name))
            .collect();
        orphaned.sort_by_key(|(name, _)| *name);
        for (_, path) in orphaned {
            if let Some(t) = header(path, warnings) {
                rows.push((t, Some(path.clone()), false));
            }
        }
        let referenced: HashSet<_> = rows
            .iter()
            .filter_map(|(t, _, _)| t.base.as_ref().map(|base| (t.id.clone(), base.clone())))
            .collect();
        for (thread, file, indexed) in rows {
            let name = Path::new(&thread.rollout)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let leaf = !referenced.iter().any(|(owner, id)| {
                owner == &thread.id
                    && (name
                        .trim_end_matches(".zst")
                        .ends_with(&format!("-{id}.jsonl"))
                        || name
                            .trim_end_matches(".zst")
                            .ends_with(&format!("_{id}.jsonl")))
            });
            let quality = |t: &Thread, p: &Option<PathBuf>, indexed: bool, leaf: bool| {
                (
                    p.is_some(),
                    indexed,
                    leaf,
                    t.end,
                    p.as_ref().map(|p| fingerprint(p).1).unwrap_or(0),
                )
            };
            if selected
                .get(&thread.id)
                .is_none_or(|(old, old_file, old_indexed, old_leaf)| {
                    quality(&thread, &file, indexed, leaf)
                        > quality(old, old_file, *old_indexed, *old_leaf)
                })
            {
                selected.insert(thread.id.clone(), (thread, file, indexed, leaf));
            }
        }
    }
    selected.into_values().map(|(t, f, _, _)| (t, f)).collect()
}
fn header(path: &Path, warnings: &mut Vec<String>) -> Option<Thread> {
    let data = match read_rollout(path) {
        Ok(data) => data,
        Err(e) => {
            warnings.push(format!("codex: {}: {e}", path.display()));
            return None;
        }
    };
    let mut lines = data.lines().filter(|l| !l.trim().is_empty());
    let first: Value = serde_json::from_str(lines.next()?).ok()?;
    let m = &first["payload"];
    let id = m["id"].as_str().filter(|s| !s.is_empty())?;
    let cwd = m["cwd"]
        .as_str()
        .filter(|s| Path::new(s).is_absolute() || s.as_bytes().get(1) == Some(&b':'))?;
    if first["type"] != "session_meta"
        || m["originator"] == "agentprotocol" && m["cli_version"].as_str().is_none_or(str::is_empty)
    {
        return None; // a cross-agent conversion is not proof that Codex ran it
    }
    let start = m["timestamp"]
        .as_str()
        .or(first["timestamp"].as_str())
        .and_then(parse_iso)?;
    let mut t = Thread {
        id: id.into(),
        rollout: path.display().to_string(),
        start,
        end: start,
        tokens: 0,
        model: String::new(),
        branch: m["git"]["branch"].as_str().map(String::from),
        cwd: cwd.into(),
        origin: m["git"]["repository_url"].as_str().unwrap_or("").into(),
        base: m["history_base"]["thread_id"].as_str().map(String::from),
    };
    for line in lines {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        t.end = t.end.max(
            row["timestamp"]
                .as_str()
                .and_then(parse_iso)
                .unwrap_or(start),
        );
        if row["type"] == "turn_context" {
            if let Some(model) = row["payload"]["model"].as_str() {
                t.model = model.into();
            }
        }
        if row["payload"]["type"] == "token_count" {
            t.tokens = row["payload"]["info"]["total_token_usage"]["total_tokens"]
                .as_i64()
                .unwrap_or(t.tokens);
        }
    }
    Some(t)
}

pub(super) type PatchCalls = HashMap<String, (String, String, i64, String)>;

/// A launch error (including macOS refusing a symlinked writable root) is not
/// a successful command merely because it contains no numeric exit code.
pub(super) fn process_exit_code(output: &Value) -> Option<i64> {
    let parsed = output
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let value = parsed.as_ref().unwrap_or(output);
    if let Some(code) = value["metadata"]["exit_code"].as_i64() {
        return Some(code);
    }
    let text = value.as_str()?;
    // Read only native receipt headers, not the command's output body.
    for line in text.lines() {
        if matches!(line, "Output:" | "Final output:") {
            break;
        }
        for prefix in ["Exit code: ", "Process exited with code "] {
            if let Some(code) = line.strip_prefix(prefix) {
                return code.trim().parse().ok();
            }
        }
    }
    None
}

pub(super) fn running_process(output: &Value) -> Option<u64> {
    output
        .as_str()?
        .lines()
        .take_while(|line| !matches!(*line, "Output:" | "Final output:"))
        .find_map(|line| {
            line.strip_prefix("Process running with session ID ")?
                .parse()
                .ok()
        })
}

/// Codex Desktop's native CommandExecution receipt is also retained by
/// agentprotocol exports. Accept only a literal, quoted apply_patch heredoc
/// with a successful native process receipt and matching per-file results.
pub(super) fn command_patch(
    item: &Value,
    g: &mut FileGroup,
    pm: &PathMap,
    t: &Thread,
    model: &str,
    ts: i64,
) {
    if item["type"] != "CommandExecution" || item["status"] != "completed" || item["exit_code"] != 0
    {
        return;
    }
    let workdir = crate::file_uri_path(item["cwd"].as_str().unwrap_or(&t.cwd));
    let call = serde_json::json!({"type":"local_shell_call","action":{"command":item["command"],"working_directory":workdir}});
    let Some((command, workdir)) = super::shell_call(&call, &t.cwd) else {
        return;
    };
    let Some((first, rest)) = command.split_once('\n') else {
        return;
    };
    let Some(marker) = first
        .trim()
        .strip_prefix("apply_patch <<'")
        .and_then(|s| s.strip_suffix('\''))
    else {
        return;
    };
    if marker.is_empty()
        || !marker
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return;
    }
    let Some((patch, after)) = rest.split_once(&format!("\n{marker}")) else {
        return;
    };
    if !after.is_empty() && !after.starts_with('\n') {
        return;
    }
    let text = item["aggregated_output"].as_str().unwrap_or("");
    if !text.starts_with("Success. Updated the following files:\n") {
        return;
    }
    let written: HashSet<_> = text
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("A ") || l.starts_with("M ") || l.starts_with("D "))
        .filter_map(|l| l.strip_prefix("A ").or_else(|| l.strip_prefix("M ")))
        .collect();
    for (path, body) in crate::apply_patch_files(patch) {
        if written.contains(path.as_str()) {
            crate::push_text(
                &mut g.evs,
                pm,
                &target(&path, &workdir),
                &body,
                Src::Codex,
                model,
                &t.id,
                ts,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn patch_receipt(
    p: &Value,
    calls: &mut PatchCalls,
    g: &mut FileGroup,
    pm: &PathMap,
    t: &Thread,
    model: &str,
    ts: i64,
) {
    let Some(id) = p["call_id"].as_str().filter(|s| !s.is_empty()) else {
        return;
    };
    match p["type"].as_str() {
        Some("custom_tool_call" | "function_call")
            if matches!(
                p["name"].as_str(),
                Some("apply_patch" | "functions.apply_patch")
            ) =>
        {
            let args = p["input"]
                .as_str()
                .or(p["arguments"].as_str())
                .unwrap_or("");
            let parsed = serde_json::from_str::<Value>(args).ok();
            let patch = parsed
                .as_ref()
                .and_then(|p| p["patch"].as_str())
                .unwrap_or(args);
            calls.insert(id.into(), (patch.into(), model.into(), ts, t.cwd.clone()));
        }
        Some("custom_tool_call_output" | "function_call_output") => {
            let Some((patch, model, at, workdir)) = calls.remove(id) else {
                return;
            };
            let raw = &p["output"];
            let parsed = raw
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok());
            let output = parsed.as_ref().unwrap_or(raw);
            let text = output["output"].as_str().or(output.as_str()).unwrap_or("");
            // Native Codex 0.149.1 (MCPJam) wraps custom tool output in a
            // process receipt. A zero exit is necessary; never search for a
            // success substring inside failed output or an arbitrary message.
            let text = if let Some(rest) = text.strip_prefix("Exit code: 0\nWall time: ") {
                let Some((elapsed, body)) = rest.split_once(" seconds\nOutput:\n") else {
                    return;
                };
                if !elapsed
                    .parse::<f64>()
                    .is_ok_and(|n| n.is_finite() && n >= 0.0)
                {
                    return;
                }
                body
            } else {
                text
            };
            if output["metadata"]["exit_code"]
                .as_i64()
                .is_some_and(|n| n != 0)
                || !text.starts_with("Success. Updated the following files:\n")
            {
                return;
            }
            let written: HashSet<_> = text
                .lines()
                .skip(1)
                .filter_map(|l| l.strip_prefix("A ").or_else(|| l.strip_prefix("M ")))
                .collect();
            for (path, body) in crate::apply_patch_files(&patch) {
                if written.contains(path.as_str()) {
                    crate::push_text(
                        &mut g.evs,
                        pm,
                        &target(&path, &workdir),
                        &body,
                        Src::Codex,
                        &model,
                        &t.id,
                        at,
                    );
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn real_inference_driver_command_receipt_requires_success_and_literal_patch() {
        let row: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/codex/inference-live-command.json"
        ))
        .unwrap();
        let item = row["payload"]["item"].clone();
        let workdir = crate::file_uri_path(item["cwd"].as_str().unwrap());
        let pm = PathMap {
            known: ["native-driver.txt".into()].into_iter().collect(),
            markers: vec![format!("{workdir}/")],
            excludes: vec![],
        };
        let thread = Thread {
            id: row["payload"]["thread_id"].as_str().unwrap().into(),
            rollout: String::new(),
            start: 1,
            end: 2,
            tokens: 0,
            model: "gpt-6-luna".into(),
            branch: None,
            cwd: workdir,
            origin: String::new(),
            base: None,
        };
        let count = |it: &Value| {
            let mut g = FileGroup::default();
            command_patch(it, &mut g, &pm, &thread, "gpt-6-luna", 2);
            g.evs.len()
        };
        assert_eq!(count(&item), 3);
        for (field, value) in [
            ("exit_code", json!(1)),
            ("status", json!("inProgress")),
            (
                "aggregated_output",
                json!("Success. Updated the following files:\nA another.txt\n"),
            ),
        ] {
            let mut invalid = item.clone();
            invalid[field] = value;
            assert_eq!(count(&invalid), 0, "{field}");
        }
        let mut skipped = item.clone();
        skipped["command"][2] = json!(format!("false && {}", item["command"][2].as_str().unwrap()));
        assert_eq!(
            count(&skipped),
            0,
            "an unexecuted patch in arbitrary shell text is not a write receipt"
        );
    }
}
