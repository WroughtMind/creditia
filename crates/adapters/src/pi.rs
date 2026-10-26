//! Pi / Oh My Pi / Command Code: append-only session JSONL. Sources checked at
//! earendil-works/pi 698a2d2, can1357/oh-my-pi 470b7ef and command-code npm 1.83.0.
//! Only a paired successful tool result is evidence. OMP hashline edits use the
//! actual old/new snapshots (or numbered result diff), never the requested patch.
use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn data_dir(src: Src) -> PathBuf {
    let official = match src {
        Src::Pi => "PI_CODING_AGENT_DIR",
        Src::Omp => "PI_CODING_AGENT_DIR",
        _ => "",
    };
    let custom = format!(
        "CREDITIA_{}_DIR",
        src.harness().to_uppercase().replace('-', "_")
    );
    std::env::var(&custom)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var(official)
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| match src {
            Src::Pi => crate::home().join(".pi/agent"),
            Src::Omp => crate::home().join(".omp/agent"),
            Src::CommandCode => crate::home().join(".commandcode"),
            _ => crate::source_dir(src),
        })
}
fn root(src: Src) -> PathBuf {
    data_dir(src).join(if src == Src::CommandCode {
        "projects"
    } else {
        "sessions"
    })
}
fn files(src: Src) -> Vec<PathBuf> {
    let mut out = jsonl_files(&root(src));
    let custom = format!(
        "CREDITIA_{}_DIR",
        src.harness().to_uppercase().replace('-', "_")
    );
    if src == Src::Omp
        && std::env::var(&custom).is_err()
        && std::env::var("PI_CODING_AGENT_DIR").is_err()
    {
        out.extend(jsonl_files(&crate::home().join(".omp/profiles")));
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            out.extend(jsonl_files(&PathBuf::from(xdg).join("omp")));
        }
    }
    out.sort();
    out.dedup();
    // A migration can leave identical session files under the previous path.
    // Stable session id selects one; mtime selects the freshest copy.
    use std::io::BufRead;
    let mut ids: HashMap<String, PathBuf> = HashMap::new();
    let mut unknown = vec![];
    for p in out {
        let id = std::fs::File::open(&p).ok().and_then(|f| {
            std::io::BufReader::new(f)
                .lines()
                .map_while(Result::ok)
                .find_map(|line| {
                    let v: Value = serde_json::from_str(&line).ok()?;
                    (v["type"] == "session")
                        .then(|| v["id"].as_str().map(str::to_string))
                        .flatten()
                })
        });
        if let Some(id) = id {
            if ids
                .get(&id)
                .is_none_or(|q| fingerprint(&p).0 > fingerprint(q).0)
            {
                ids.insert(id, p);
            }
        } else {
            unknown.push(p);
        }
    }
    unknown.extend(ids.into_values());
    unknown.sort();
    unknown
}
fn belongs(src: Src, rows: &[Value]) -> bool {
    if !matches!(src, Src::Pi | Src::Omp) {
        return true;
    }
    let custom = format!(
        "CREDITIA_{}_DIR",
        src.harness().to_uppercase().replace('-', "_")
    );
    if std::env::var(&custom).is_ok() || std::env::var("PI_CODING_AGENT_DIR").is_err() {
        return true;
    }
    // Both products use the same official env key. Inspect the actual current
    // format so setting that key cannot import one session as two executors.
    let omp = rows.iter().any(|r| {
        r.get("runtime").is_some() || r["type"] == "model_change" && r["model"].is_string()
    });
    let pi = rows
        .iter()
        .any(|r| r["type"] == "model_change" && r["modelId"].is_string());
    if omp {
        src == Src::Omp
    } else if pi {
        src == Src::Pi
    } else {
        false
    }
}
pub(crate) fn jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|s| s == "jsonl"))
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}
pub(crate) fn read_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
pub(crate) fn abs(cwd: &str, path: &str) -> String {
    crate::harness_paths::target(path, cwd)
}
fn text(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
/// OMP's result diff is `+12|actual line`, while Pi uses `+12 actual line`.
fn numbered_added(diff: &str) -> String {
    diff.lines()
        .filter_map(|l| {
            let l = l.strip_prefix('+')?;
            if l.starts_with('+') {
                return None;
            }
            let n = l
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ' ')
                .count();
            let rest = &l[n..];
            Some(format!("{}\n", rest.strip_prefix('|').unwrap_or(rest)))
        })
        .collect()
}
fn edits(src: Src, name: &str, args: &Value, result: &Value) -> Vec<(String, String)> {
    if src == Src::Omp && matches!(name, "edit" | "apply_patch" | "ast_edit") {
        let d = &result["details"];
        let files: Vec<&Value> = d["perFileResults"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_else(|| vec![d]);
        return files
            .into_iter()
            .filter_map(|f| {
                let p = f["path"].as_str().or_else(|| args["path"].as_str())?;
                if f["op"] == "delete" {
                    return None;
                }
                let added = match (f["oldText"].as_str(), f["newText"].as_str()) {
                    (Some(old), Some(new)) => added_lines(old, new),
                    (None, Some(new)) if f["op"] == "create" => new.to_string(),
                    _ => numbered_added(f["diff"].as_str().unwrap_or("")),
                };
                (!added.is_empty()).then(|| (p.into(), added))
            })
            .collect();
    }
    let Some(p) = crate::path_arg(args) else {
        return vec![];
    };
    let new = match name {
        // A successful full overwrite does not identify the newly added lines.
        // Pi/OMP/MiniMax write receipts retain neither before nor create proof.
        // Command Code does distinguish creation from overwrite explicitly.
        "write_file"
            if src == Src::CommandCode
                && text(&result["content"]).starts_with("File created successfully at: ") =>
        {
            args["content"].as_str().map(str::to_string)
        }
        "write" | "write_file" => None,
        "edit" => {
            let mut all = String::new();
            let e = &args["edits"];
            let parsed: Value = e
                .as_str()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| e.clone());
            let es: Vec<&Value> = parsed
                .as_array()
                .map(|v| v.iter().collect())
                .unwrap_or_else(|| {
                    if parsed.is_object() {
                        vec![&parsed]
                    } else {
                        vec![]
                    }
                });
            for e in es {
                if let (Some(old), Some(new)) = (e["oldText"].as_str(), e["newText"].as_str()) {
                    all.push_str(&added_lines(old, new));
                }
            }
            if let (Some(old), Some(new)) = (args["oldText"].as_str(), args["newText"].as_str()) {
                all.push_str(&added_lines(old, new));
            }
            (!all.is_empty()).then_some(all)
        }
        "edit_file" => args["old_string"]
            .as_str()
            .zip(args["new_string"].as_str())
            .map(|(old, new)| added_lines(old, new)),
        _ => None,
    };
    new.map(|s| vec![(p, s)]).unwrap_or_default()
}
fn command_ok(name: &str, result: &Value) -> bool {
    // The current harness drops isError when serializing tool_result. Match its
    // concrete success receipts instead of treating every result as successful.
    if result["is_error"] == true {
        return false;
    }
    let t = text(&result["content"]);
    match name {
        "write_file" => {
            t.starts_with("File created successfully at: ")
                || (t.starts_with("The file ") && t.contains(" has been updated successfully."))
        }
        "edit_file" => t.starts_with("Edited ") || t.starts_with("Created "),
        _ => false,
    }
}
struct Call {
    name: String,
    args: Value,
    model: String,
    ts: i64,
}
/// Parse one transcript; pending/failed/refused calls never produce evidence.
pub(crate) fn parse(src: Src, rows: &[Value], pm: &PathMap) -> FileGroup {
    let Some(h) = rows.iter().find(|r| r["type"] == "session") else {
        return FileGroup::default();
    };
    let cwd = h["cwd"].as_str().unwrap_or("");
    if !pm.in_project(cwd) {
        return FileGroup::default();
    }
    let id = h["id"].as_str().unwrap_or("");
    let start = any_ts(&h["timestamp"]).unwrap_or(0);
    let mut end = start;
    let mut g = FileGroup::default();
    let (mut model, mut provider) = (String::new(), None);
    let mut pending: HashMap<String, Call> = HashMap::new();
    let (mut tokens, mut prompts, mut has_content) = (0, 0, false);
    let mut models: HashMap<String, usize> = HashMap::new();
    for r in rows {
        let ts = any_ts(&r["timestamp"]).unwrap_or(end);
        end = end.max(ts);
        if r["type"] == "model_change" {
            if let Some(m) = r["modelId"].as_str() {
                model = m.into();
                provider = r["provider"].as_str().map(str::to_string);
            } else if let Some(m) = r["model"].as_str() {
                model = m.into();
            }
        }
        if r["type"] != "message" {
            continue;
        }
        // Forks/clones retain their parent's entries. They are already owned
        // by the original session, not work newly done in this child.
        if h["parentSession"].is_string() && ts > 0 && ts < start {
            continue;
        }
        let m = &r["message"];
        if m["role"] == "assistant" {
            has_content = true;
            if let Some(v) = m["model"].as_str().or(r["model"].as_str()) {
                model = v.into();
            }
            if let Some(v) = m["provider"].as_str() {
                provider = Some(v.into());
            }
            *models.entry(model.clone()).or_default() += 1;
            let u = if src == Src::CommandCode {
                &r["usage"]
            } else {
                &m["usage"]
            };
            tokens += u["totalTokens"].as_i64().unwrap_or_else(|| {
                [
                    "inputTokens",
                    "outputTokens",
                    "cacheReadTokens",
                    "cacheWriteTokens",
                ]
                .iter()
                .map(|k| u[*k].as_i64().unwrap_or(0))
                .sum()
            });
            for b in m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| b["type"] == "toolCall" || b["type"] == "tool_use")
            {
                let args = if b["type"] == "tool_use" {
                    &b["input"]
                } else {
                    &b["arguments"]
                };
                if let Some(call_id) = b["id"].as_str() {
                    pending.insert(
                        call_id.into(),
                        Call {
                            name: b["name"].as_str().unwrap_or("").into(),
                            args: args.clone(),
                            model: model.clone(),
                            ts,
                        },
                    );
                }
            }
        }
        if m["role"] == "user" && (!text(&m["content"]).trim().is_empty()) {
            prompts += 1;
        }
        let results: Vec<&Value> = if m["role"] == "toolResult" {
            vec![m]
        } else {
            m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| b["type"] == "tool_result")
                .collect()
        };
        for result in results {
            let call_id = result["toolCallId"]
                .as_str()
                .or(result["tool_use_id"].as_str())
                .unwrap_or("");
            let Some(call) = pending.remove(call_id) else {
                continue;
            };
            let ok = if src == Src::CommandCode {
                command_ok(&call.name, result)
            } else {
                result["isError"] == false
            };
            if !ok {
                continue;
            }
            for (p, s) in edits(src, &call.name, &call.args, result) {
                crate::push_text(
                    &mut g.evs,
                    pm,
                    &abs(cwd, &p),
                    &s,
                    src,
                    &call.model,
                    id,
                    call.ts,
                );
            }
            if matches!(call.name.as_str(), "bash")
                && result["details"]["exitCode"]
                    .as_i64()
                    .is_none_or(|n| n == 0)
            {
                if let Some(cmd) = call.args["command"].as_str() {
                    for rel in crate::shell_targets(cmd, cwd, pm) {
                        crate::push_rel(&mut g.evs, &rel, cmd, src, &call.model, id, call.ts);
                    }
                }
            }
        }
    }
    if has_content {
        let model = models
            .into_iter()
            .max_by_key(|(m, n)| (*n, std::cmp::Reverse(m.clone())))
            .map(|(m, _)| m)
            .unwrap_or(model);
        g.sessions.push(Session {
            src,
            id: id.into(),
            model,
            start,
            end,
            tokens,
            branch: None,
            prompts,
            reviewer: false,
            has_content,
            provider,
        });
    }
    g
}
pub fn scan(src: Src, store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    let prefix = format!("{}:jsonl:", src.harness());
    let files = files(src);
    let keys: std::collections::HashSet<String> = files
        .iter()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(&prefix) || keys.contains(k));
    for p in files {
        let key = format!("{prefix}{}", p.display());
        let fp = fingerprint(&p);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let rows = read_jsonl(&p);
        let mut g = if belongs(src, &rows) {
            parse(src, &rows, pm)
        } else {
            FileGroup::default()
        };
        g.fp = fp;
        store.groups.insert(key, g);
    }
}
pub fn discover(src: Src) -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    for p in files(src) {
        let rows = read_jsonl(&p);
        if !belongs(src, &rows) {
            continue;
        }
        let Some(h) = rows.iter().find(|v| v["type"] == "session") else {
            continue;
        };
        let Some(cwd) = h["cwd"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let last = rows
            .iter()
            .filter_map(|v| any_ts(&v["timestamp"]))
            .max()
            .unwrap_or(0);
        let e = out.entry(cwd.into()).or_insert(Activity {
            src,
            cwd: cwd.into(),
            remote: None,
            sessions: 0,
            last: 0,
        });
        e.sessions += 1;
        e.last = e.last.max(last);
    }
    out.into_values().collect()
}
pub fn doctor(src: Src) -> Health {
    let activities = discover(src);
    Health { source: src.harness().into(), path: root(src).display().to_string(), readable: root(src).is_dir(), format: "session JSONL v3".into(), records: activities.iter().map(|a| a.sessions).sum(), note: creditia_core::tr!("工具调用必须配对成功结果；OMP hashline 使用实际结果快照。整份覆盖缺少before或新建证明时排除；Command Code write只保留明确新建回执。尚未完成各产品真实运行验收。", "Tool calls require matching successful results; OMP hashline uses actual result snapshots. Full overwrites without before/create proof are excluded; Command Code full writes require an explicit creation receipt. Product runtime validation remains pending."), ..Default::default() }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    #[test]
    fn successful_edits_outside_cwd_do_not_match_project_suffixes() {
        for src in [Src::Pi, Src::Omp] {
            let mut rows = vec![
                json!({"type":"session","id":"outside","cwd":"/repo"}),
                json!({"type":"message","message":{"role":"assistant","model":"writer","content":[{"type":"toolCall","id":"edit","name":"edit","arguments":{"path":"../other/a.rs","oldText":"old line","newText":"outside generated line"}}]}}),
                json!({"type":"message","message":{"role":"toolResult","toolCallId":"edit","isError":false,"details":{"path":"../other/a.rs","oldText":"old line","newText":"outside generated line"}}}),
            ];
            assert!(parse(src, &rows, &pm()).evs.is_empty(), "{}", src.harness());
            rows[1]["message"]["content"][0]["arguments"]["path"] = json!("a.rs");
            rows[2]["message"]["details"]["path"] = json!("a.rs");
            assert_eq!(parse(src, &rows, &pm()).evs.len(), 1, "{}", src.harness());
        }
    }
    #[test]
    fn full_overwrite_without_before_never_claims_human_context() {
        let human = "human_original_line = compute_human(73);";
        let model_line = "model_new_line = compute_model(91);";
        let after = format!("{human}\n{model_line}");
        for src in [Src::Pi, Src::Omp] {
            let rows = vec![
                json!({"type":"session","id":"delta","cwd":"/repo"}),
                json!({"type":"message","message":{"role":"assistant","model":"writer","content":[{"type":"toolCall","id":"write","name":"write","arguments":{"path":"a.rs","content":after}},{"type":"toolCall","id":"edit","name":"edit","arguments":{"path":"a.rs","oldText":human,"newText":after}},{"type":"toolCall","id":"failed","name":"edit","arguments":{"path":"a.rs","oldText":human,"newText":"failed_only_line"}},{"type":"toolCall","id":"pending","name":"write","arguments":{"path":"a.rs","content":"pending_only_line"}}]}}),
                json!({"type":"message","message":{"role":"toolResult","toolCallId":"write","isError":false,"content":[{"type":"text","text":"Successfully wrote to a.rs"}]}}),
                json!({"type":"message","message":{"role":"toolResult","toolCallId":"failed","isError":true}}),
                json!({"type":"message","message":{"role":"toolResult","toolCallId":"edit","isError":false,"details":{"path":"a.rs","oldText":human,"newText":after}}}),
            ];
            let g = parse(src, &rows, &pm());
            assert_eq!(g.evs.len(), 1, "{}", src.harness());
            assert_eq!(
                g.evs[0].key,
                creditia_core::hash::line_key(
                    "a.rs",
                    &creditia_core::hash::normalize(model_line, ".rs")
                )
            );
            assert_ne!(
                g.evs[0].key,
                creditia_core::hash::line_key(
                    "a.rs",
                    &creditia_core::hash::normalize(human, ".rs")
                )
            );
        }
        // Missing oldText is not an empty file. Only OMP's actual create op is.
        assert!(edits(
            Src::Omp,
            "edit",
            &json!({"path":"a.rs"}),
            &json!({"details":{"newText":after}})
        )
        .is_empty());
        assert!(edits(
            Src::Pi,
            "edit",
            &json!({"path":"a.rs","newText":after}),
            &json!({})
        )
        .is_empty());
        assert_eq!(
            edits(
                Src::Omp,
                "edit",
                &json!({"path":"a.rs"}),
                &json!({"details":{"op":"create","newText":model_line}})
            ),
            vec![("a.rs".into(), model_line.into())]
        );
    }
    #[test]
    fn command_overwrite_receipt_is_not_create_proof() {
        let human = "human_original_line = original_command_work();";
        let model_line = "model_new_line = generated_command_work();";
        let after = format!("{human}\n{model_line}");
        let rows = vec![
            json!({"type":"session","id":"c","cwd":"/repo"}),
            json!({"type":"message","model":"command-model","message":{"role":"assistant","content":[{"type":"tool_use","id":"write","name":"write_file","input":{"file_path":"/repo/a.rs","content":after}},{"type":"tool_use","id":"edit","name":"edit_file","input":{"file_path":"/repo/a.rs","old_string":human,"new_string":after}}]}}),
            json!({"type":"message","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"write","content":"The file /repo/a.rs has been updated successfully."},{"type":"tool_result","tool_use_id":"edit","content":"Edited /repo/a.rs (1 replacement)"}]}}),
        ];
        let g = parse(Src::CommandCode, &rows, &pm());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key(
                "a.rs",
                &creditia_core::hash::normalize(model_line, ".rs")
            )
        );
        assert!(edits(
            Src::CommandCode,
            "write_file",
            &json!({"path":"a.rs","content":after}),
            &json!({"content":"The file /repo/a.rs has been updated successfully."})
        )
        .is_empty());
        assert_eq!(
            edits(
                Src::CommandCode,
                "write_file",
                &json!({"path":"a.rs","content":model_line}),
                &json!({"content":"File created successfully at: /repo/a.rs"})
            ),
            vec![("a.rs".into(), model_line.into())]
        );
    }
    #[test]
    fn pi_success_failure_pending_and_models() {
        let rows = vec![
            json!({"type":"session","id":"s","cwd":"/repo","timestamp":1}),
            json!({"type":"message","message":{"role":"assistant","model":"m1","provider":"p","content":[{"type":"toolCall","id":"ok","name":"edit","arguments":{"path":"a.rs","oldText":"human_original_line","newText":"human_original_line\nlet first = true;"}},{"type":"toolCall","id":"bad","name":"write","arguments":{"path":"a.rs","content":"let bad = true;"}}]}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"ok","isError":false}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"bad","isError":true}}),
            json!({"type":"message","message":{"role":"assistant","model":"m2","content":[{"type":"toolCall","id":"edit","name":"edit","arguments":{"path":"a.rs","oldText":"unchanged\nold","newText":"unchanged\nlet second = true;"}},{"type":"toolCall","id":"pending","name":"write","arguments":{"path":"a.rs","content":"let pending = true;"}}]}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"edit","isError":false}}),
        ];
        let g = parse(Src::Pi, &rows, &pm());
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "m1");
        assert_eq!(g.evs[1].model, "m2");
    }
    #[test]
    fn omp_hashline_records_actual_applied_lines() {
        let result = json!({"details":{"perFileResults":[{"path":"a.rs","oldText":"same\nold","newText":"same\nlet actual = 1;"},{"path":"a.rs","diff":"-2|old\n+2|let pruned = 1;"}]}});
        assert_eq!(
            edits(
                Src::Omp,
                "edit",
                &json!({"input":"untrusted request"}),
                &result
            ),
            vec![
                ("a.rs".into(), "let actual = 1;\n".into()),
                ("a.rs".into(), "let pruned = 1;\n".into())
            ]
        );
        assert_eq!(
            numbered_added("-1 old\n+1 let valid = 1;"),
            "let valid = 1;\n"
        );
    }
    #[test]
    fn command_success_receipt_required() {
        assert!(command_ok(
            "write_file",
            &json!({"content":[{"type":"text","text":"File created successfully at: /repo/a.rs"}]})
        ));
        for t in [
            "Permission denied",
            "File has not been read yet.",
            "Interrupted",
            "",
        ] {
            assert!(!command_ok(
                "write_file",
                &json!({"content":[{"type":"text","text":t}]})
            ));
        }
        assert!(!command_ok(
            "edit_file",
            &json!({"is_error":true,"content":"Edited /repo/a.rs"})
        ));
    }
    #[test]
    fn omp_results_keep_call_model_and_reject_failed_write() {
        let rows = vec![
            json!({"type":"title","title":"name"}),
            json!({"type":"session","id":"o","cwd":"/repo"}),
            json!({"type":"message","message":{"role":"assistant","model":"before","content":[{"type":"toolCall","id":"a","name":"edit","arguments":{"input":"[a.rs#tag]\nPUT 1-1:\n+requested"}}]}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"a","isError":false,"details":{"path":"/repo/a.rs","oldText":"old","newText":"let actual = 1;"}}}),
            json!({"type":"message","message":{"role":"assistant","model":"after","content":[{"type":"toolCall","id":"b","name":"edit","arguments":{"path":"a.rs","oldText":"human_original_line","newText":"human_original_line\nlet second = 2;"}},{"type":"toolCall","id":"bad","name":"write","arguments":{"path":"a.rs","content":"let denied = 3;"}}]}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"b","isError":false,"details":{"path":"a.rs","oldText":"human_original_line","newText":"human_original_line\nlet second = 2;"}}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"bad","isError":true}}),
        ];
        let g = parse(Src::Omp, &rows, &pm());
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "before");
        assert_eq!(g.evs[1].model, "after");
    }
    #[test]
    fn command_v3_models_and_real_success_receipts() {
        let rows = vec![
            json!({"type":"session","id":"c","cwd":"/repo"}),
            json!({"type":"model_change","model":"first"}),
            json!({"type":"message","model":"first","message":{"role":"assistant","content":[{"type":"tool_use","id":"a","name":"write_file","input":{"file_path":"/repo/a.rs","content":"let first = 1;"}},{"type":"tool_use","id":"bad","name":"write_file","input":{"file_path":"/repo/a.rs","content":"let bad = 3;"}}]}}),
            json!({"type":"message","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"a","content":[{"type":"text","text":"File created successfully at: /repo/a.rs"}]},{"type":"tool_result","tool_use_id":"bad","content":[{"type":"text","text":"Permission denied for write_file"}]}]}}),
            json!({"type":"message","model":"second","message":{"role":"assistant","content":[{"type":"tool_use","id":"b","name":"edit_file","input":{"file_path":"/repo/a.rs","old_string":"same\nold","new_string":"same\nlet second = 2;"}}]}}),
            json!({"type":"message","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"b","content":[{"type":"text","text":"Edited /repo/a.rs (1 replacement)"}]}]}}),
        ];
        let g = parse(Src::CommandCode, &rows, &pm());
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "first");
        assert_eq!(g.evs[1].model, "second");
    }
}
