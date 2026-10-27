//! Muse native session JSONL and official export_schema_version=1 trajectories.
//! Contract: dev.meta.ai/docs/cookbook/audit-agent-sessions, official approval_slice
//! sample, SDK bb863c9; older tool-result envelope corroborated by muse-code-acp.
//! Approval/intent alone is NOT a write. Require the matching completed task or
//! tool result outcome=success. Older encrypted model arguments cannot supply
//! lines unless raw_args was retained in approval; never infer them by readback.
use crate::{Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
pub fn data_dir() -> PathBuf {
    std::env::var("CREDITIA_MUSE_DIR")
        .or_else(|_| std::env::var("CREDITIA_MUSE_CODE_DIR"))
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::xdg_data().join("muse"))
}
fn root() -> PathBuf {
    data_dir().join("sessions")
}
fn files() -> Vec<PathBuf> {
    let mut out: Vec<_> = walkdir::WalkDir::new(data_dir())
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && (e.file_name() == "session.jsonl"
                    || e.path().extension().is_some_and(|s| s == "json")
                        && e.file_name().to_string_lossy().starts_with("trajectory"))
        })
        .map(|e| e.into_path())
        .collect();
    out.sort_by_key(|p| (p.extension().is_some_and(|s| s == "json"), p.clone()));
    out
}
fn rows(p: &Path) -> Vec<Value> {
    if p.extension().is_some_and(|s| s == "jsonl") {
        return crate::pi::read_jsonl(p);
    }
    let v: Value = std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if v["export_schema_version"] != 1 {
        return vec![];
    }
    v["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["kind"] == "record")
        .filter_map(|e| e.get("envelope").cloned())
        .collect()
}
// Muse journal clocks are microseconds, unlike the other adapters' milliseconds.
fn ts(v: &Value) -> i64 {
    v.as_i64()
        .map(|t| {
            if t > 100_000_000_000_000 {
                t / 1_000_000
            } else if t > 10_000_000_000 {
                t / 1000
            } else {
                t
            }
        })
        .unwrap_or(0)
}
fn metadata(rows: &[Value]) -> Option<(&str, &str)> {
    rows.iter().find_map(|r| {
        if r["payload_type"] != "runtime.session.metadata" && r["payload"]["kind"] != "metadata" {
            return None;
        }
        Some((
            r["stream"]["id"].as_str()?,
            r["payload"]["record"]["workspace_root"].as_str()?,
        ))
    })
}
struct Call {
    tool: String,
    args: Value,
    ts: i64,
}
fn write(out: &mut FileGroup, pm: &PathMap, cwd: &str, sid: &str, c: &Call) {
    let Some(path) = crate::path_arg(&c.args) else {
        return;
    };
    let text = match c.tool.as_str() {
        // Approval + completion proves execution, not creation or the previous
        // contents of a full-file write. Native receipts don't retain before.
        "write_file" => None,
        "edit_file" => c.args["old_string"]
            .as_str()
            .zip(c.args["new_string"].as_str())
            .map(|(old, new)| crate::added_lines(old, new)),
        _ => None,
    };
    if let Some(text) = text {
        crate::push_text(
            &mut out.evs,
            pm,
            &crate::pi::abs(cwd, &path),
            &text,
            Src::Muse,
            "",
            sid,
            c.ts,
        );
    }
}
fn parse(rows: &[Value], pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let Some((id, cwd)) = metadata(rows) else {
        return g;
    };
    if !pm.in_project(cwd) {
        return g;
    }
    let mut approvals: HashMap<String, Call> = HashMap::new();
    let mut tasks: HashMap<String, String> = HashMap::new();
    let mut done = HashSet::new();
    let (mut prompts, mut start, mut end, mut content) = (0, 0, 0, false);
    for r in rows {
        let time = ts(&r["recorded_at"]);
        if start == 0 {
            start = time;
        }
        end = end.max(time);
        let p = &r["payload"];
        let e = &p["event"];
        if r["payload_type"] == "runtime.user_intent.accepted"
            || (p["kind"] == "run" && e["kind"] == "started")
        {
            prompts += 1;
            content = true;
        }
        // Real official approval records carry verbatim raw_args. These are
        // proposals until the same task has a committed successful terminal.
        if p["kind"] == "approval" && e["kind"] == "requested" {
            if let (Some(call), Some(tool), Some(args)) = (
                e["tool_call_id"].as_str(),
                e["tool_name"].as_str(),
                e["raw_args"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok()),
            ) {
                approvals.insert(
                    call.into(),
                    Call {
                        tool: tool.into(),
                        args,
                        ts: time,
                    },
                );
            }
        }
        if e["kind"] == "side_effect_intent" {
            if let (Some(task), Some(key)) = (
                p["task_id"].as_str().or(e["task_id"].as_str()),
                e["idempotency_key"].as_str(),
            ) {
                if e["operation"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("tool:"))
                {
                    tasks.insert(task.into(), key.strip_prefix("tool:").unwrap_or(key).into());
                }
            }
        }
        if p["kind"] == "task" && e["kind"] == "completed" {
            if let Some(call) = tasks.get(p["task_id"].as_str().unwrap_or("")) {
                if done.insert(call.clone()) {
                    if let Some(c) = approvals.get(call) {
                        write(&mut g, pm, cwd, id, c);
                    }
                }
            }
        }
        if p["kind"] == "tool_result" && p["correlation_facts"]["outcome"] == "success" {
            let call = p["call_id"].as_str().unwrap_or("");
            if !done.insert(call.to_string()) {
                continue;
            }
            if let Some(c) = approvals.get(call) {
                write(&mut g, pm, cwd, id, c);
            }
            if p["correlation_facts"]["tool_name"] == "bash" {
                let result: Value = p["text"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                if result["exit_code"] == 0 {
                    if let Some(cmd) = result["command"].as_str() {
                        for rel in crate::shell_targets(cmd, cwd, pm) {
                            crate::push_rel(&mut g.evs, &rel, cmd, Src::Muse, "", id, time);
                        }
                    }
                }
            }
        }
    }
    g.sessions.push(Session {
        src: Src::Muse,
        id: id.into(),
        model: String::new(),
        start,
        end,
        tokens: 0,
        branch: None,
        prompts,
        reviewer: false,
        has_content: content,
        provider: None,
    });
    g
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    // Parse every time: export and native files can contain the same stream.
    // Native comes first and each stream is claimed once.
    let prefix = "muse-code:session:";
    store.groups.retain(|k, _| !k.starts_with(prefix));
    let mut seen = HashSet::new();
    let mut missing = 0;
    for p in files() {
        let rs = rows(&p);
        let Some((id, cwd)) = metadata(&rs) else {
            continue;
        };
        if !pm.in_project(cwd) || !seen.insert(id.to_string()) {
            continue;
        }
        let mut g = parse(&rs, pm);
        if g.evs.is_empty() {
            missing += 1;
        }
        g.fp = fingerprint(&p);
        store.groups.insert(format!("{prefix}{}", p.display()), g);
    }
    if missing > 0 {
        st.notes.push(creditia_core::tr!("Muse Code: {missing} 个会话缺少可确认的逐行编辑；旧格式工具参数可能仅在加密响应中，不能从成功路径回执还原内容。", "Muse Code: {missing} sessions lack confirmed line evidence; older tool arguments may exist only inside encrypted responses, so path receipts cannot reconstruct contents."));
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    let mut seen = HashSet::new();
    for p in files() {
        let rs = rows(&p);
        let Some((id, cwd)) = metadata(&rs) else {
            continue;
        };
        if !seen.insert(id.to_string()) {
            continue;
        }
        let e = out.entry(cwd.into()).or_insert(Activity {
            src: Src::Muse,
            cwd: cwd.into(),
            remote: None,
            sessions: 0,
            last: 0,
        });
        e.sessions += 1;
        e.last = e
            .last
            .max(rs.iter().map(|r| ts(&r["recorded_at"])).max().unwrap_or(0));
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    Health{source:Src::Muse.harness().into(),path:root().display().to_string(),readable:root().is_dir(),format:"native session.jsonl + official trajectory JSON v1".into(),records:discover().iter().map(|a|a.sessions).sum(),note:creditia_core::tr!("可读会话与已保留old/new参数且确认完成的edit差分；write_file无before或新建证明，故排除整份覆盖。成功 shell 取零退出结果。旧日志加密响应中的参数、未核实模型字段不猜测；模型留空。MSP/T3 中的可读编辑由对应读取器处理。未做 Muse 本机真实运行验收。", "Reads sessions and completed edit deltas with retained old/new arguments. Full write_file overwrites lack before/create proof and are excluded; shell evidence requires zero exit. Encrypted historical arguments and unverified model fields remain unknown. MSP/T3 readable edits use their respective readers. Muse runtime validation remains pending."),..Default::default()}
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn completed_full_write_does_not_turn_human_context_into_model_lines() {
        let human = "human_original_line = muse_existing_work();";
        let new = "model_new_line = muse_added_work();";
        let after = format!("{human}\n{new}");
        let mut rows = vec![
            json!({"payload_type":"runtime.session.metadata","stream":{"id":"s"},"payload":{"record":{"workspace_root":"/repo"}}}),
        ];
        for (call, tool, args, status) in [
            (
                "write",
                "write_file",
                json!({"file_path":"/repo/a.rs","content":after}),
                "completed",
            ),
            (
                "edit",
                "edit_file",
                json!({"file_path":"/repo/a.rs","old_string":human,"new_string":after}),
                "completed",
            ),
            (
                "failed",
                "edit_file",
                json!({"file_path":"/repo/a.rs","old_string":human,"new_string":"failed_only_line"}),
                "failed",
            ),
            (
                "pending",
                "write_file",
                json!({"file_path":"/repo/a.rs","content":"pending_only_line"}),
                "started",
            ),
        ] {
            rows.push(json!({"payload":{"kind":"approval","event":{"kind":"requested","tool_call_id":call,"tool_name":tool,"raw_args":args.to_string()}}}));
            rows.push(json!({"payload":{"kind":"task","task_id":call,"event":{"kind":"side_effect_intent","operation":format!("tool:{tool}"),"idempotency_key":format!("tool:{call}")}}}));
            rows.push(json!({"payload":{"kind":"task","task_id":call,"event":{"kind":status}}}));
        }
        rows.push(json!({"payload":{"kind":"tool_result","call_id":"write","correlation_facts":{"outcome":"success","tool_name":"write_file"},"text":"wrote 100 bytes to /repo/a.rs"}}));
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse(&rows, &pm);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key("a.rs", &creditia_core::hash::normalize(new, ".rs"))
        );
        assert_ne!(
            g.evs[0].key,
            creditia_core::hash::line_key("a.rs", &creditia_core::hash::normalize(human, ".rs"))
        );
    }
    #[test]
    fn approval_is_not_success_and_unknown_model_stays_unknown() {
        let mut rows = vec![
            json!({"payload_type":"runtime.session.metadata","stream":{"id":"s"},"recorded_at":1785704000000000i64,"payload":{"kind":"metadata","record":{"workspace_root":"/repo"}}}),
        ];
        for (call, task, status) in [
            ("a", "ta", "completed"),
            ("b", "tb", "failed"),
            ("c", "tc", "cancelled"),
        ] {
            rows.push(json!({"payload":{"kind":"approval","event":{"kind":"requested","tool_call_id":call,"tool_name":"edit_file","raw_args":json!({"file_path":"/repo/a.rs","old_string":"keep\nold","new_string":"keep\nlet confirmed = true;"}).to_string()}}}));
            rows.push(json!({"payload":{"kind":"task","task_id":task,"event":{"kind":"side_effect_intent","operation":"tool:edit_file","idempotency_key":format!("tool:{call}")}}}));
            rows.push(json!({"payload":{"kind":"task","task_id":task,"event":{"kind":status}}}));
        }
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse(&rows, &pm);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.sessions[0].model, "");
        assert_eq!(g.sessions[0].start, 1785704000);
    }
    #[test]
    fn successful_path_receipt_cannot_reconstruct_file() {
        let rows = vec![
            json!({"payload_type":"runtime.session.metadata","stream":{"id":"s"},"payload":{"record":{"workspace_root":"/repo"}}}),
            json!({"payload":{"kind":"tool_result","call_id":"x","text":"wrote 100 bytes to /repo/a.rs","correlation_facts":{"tool_name":"write_file","outcome":"success"}}}),
        ];
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        assert!(parse(&rows, &pm).evs.is_empty());
    }
}
