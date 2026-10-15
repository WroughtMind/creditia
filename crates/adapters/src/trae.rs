//! TRAE CLI（traex）：codex-rs 的分支，会话写成和 Codex 一样的 rollout jsonl，放在自己的目录
//! `~/.trae/cli/sessions/YYYY/MM/DD/rollout-*.jsonl`（`TRAECLI_HOME`、`TRAE_HOME` 可以改位置），没有 Codex 的 sqlite 线程索引。
//! - `session_meta` 给会话 id、cwd、渠道；`turn_context` 给当前模型；`event_msg` 的 `user_message` 是一次提问；
//! - 编辑：`item_completed` 里的 FileChange（同 Codex）；没有 FileChange 的会话退回读 apply_patch 调用，结果报错的不算；
//! - 工具调用可能在 `response_item` 里，也可能在 `history_mutation` 的 items 里。
//!
//! TRAE IDE（VS Code 分支）的对话存在 state.vscdb 里，格式没有公开，不读。

use crate::codex::{call_failed, shell_call};
use crate::{any_ts, apply_patch_files, read_maybe_zst, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn rollouts() -> Vec<PathBuf> {
    let base = source_dir(Src::Trae);
    let mut v = vec![];
    for dir in ["sessions", "archived_sessions"] {
        for e in walkdir::WalkDir::new(base.join(dir)).into_iter().flatten() {
            let n = e.file_name().to_string_lossy();
            if e.file_type().is_file() && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst")) {
                v.push(e.path().to_path_buf());
            }
        }
    }
    v.sort();
    v
}

/// 只读开头几行拿 cwd
fn head_cwd(p: &Path) -> Option<String> {
    let text = read_maybe_zst(p);
    text.lines().take(10).filter(|l| l.contains("\"session_meta\"") || l.contains("\"turn_context\"")).find_map(|l| serde_json::from_str::<Value>(l).ok()?["payload"]["cwd"].as_str().map(String::from))
}

/// 一次调用里的 apply_patch 补丁原文：custom_tool_call 的 input，或 function_call 参数里的 input / patch
fn patch_of(it: &Value) -> Option<String> {
    if it["name"] != "apply_patch" {
        return None;
    }
    if let Some(s) = it["input"].as_str() {
        return Some(s.to_string());
    }
    let args: Value = serde_json::from_str(it["arguments"].as_str()?).ok()?;
    args["input"].as_str().or(args["patch"].as_str()).map(String::from)
}

fn parse(text: &str, fallback_id: &str, pm: &PathMap, g: &mut FileGroup) {
    let (mut id, mut cwd, mut provider) = (fallback_id.to_string(), String::new(), None);
    let (mut start, mut end, mut prompts, mut tokens) = (0i64, 0i64, 0i64, 0i64);
    let mut model = String::new();
    let mut models: HashMap<String, i64> = HashMap::new();
    let mut changes: Vec<(String, String, String, i64)> = vec![];
    // 调用 id → (补丁或命令, 工作目录, 模型, 时间)；失败的调用 id
    let mut patches: Vec<(String, String, String, String, i64)> = vec![];
    let mut shells: Vec<(String, String, String, String, i64)> = vec![];
    let mut failed: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
        let p = &o["payload"];
        let ts = any_ts(&o["timestamp"]).unwrap_or(0);
        if ts > 0 {
            if start == 0 { start = ts; }
            end = end.max(ts);
        }
        // 工具调用和结果：response_item 一条一个，history_mutation 一条多个
        let items: Vec<&Value> = match o["type"].as_str() {
            Some("response_item") => vec![p],
            Some("history_mutation") => p["items"].as_array().map(|a| a.iter().collect()).unwrap_or_default(),
            _ => vec![],
        };
        for it in items {
            let call = it["call_id"].as_str().unwrap_or("").to_string();
            match it["type"].as_str() {
                Some("function_call") | Some("custom_tool_call") | Some("local_shell_call") => {
                    if let Some(patch) = patch_of(it) {
                        patches.push((call, patch, cwd.clone(), model.clone(), ts));
                    } else if let Some((cmd, wd)) = shell_call(it, &cwd) {
                        shells.push((call, cmd, wd, model.clone(), ts));
                    }
                }
                Some("function_call_output") | Some("custom_tool_call_output") if call_failed(&it["output"]) => {
                    failed.insert(call);
                }
                _ => {}
            }
        }
        match o["type"].as_str() {
            Some("session_meta") => {
                if let Some(s) = p["id"].as_str() { id = s.to_string(); }
                if let Some(c) = p["cwd"].as_str() { cwd = c.to_string(); }
                provider = p["model_provider"].as_str().filter(|s| !s.is_empty()).map(String::from);
                if let Some(t) = any_ts(&p["timestamp"]) { start = if start == 0 { t } else { start.min(t) }; }
            }
            Some("turn_context") => {
                if let Some(m) = p["model"].as_str() { model = m.to_string(); }
                if let Some(c) = p["cwd"].as_str() { cwd = c.to_string(); }
            }
            Some("event_msg") => match p["type"].as_str() {
                Some("user_message") => prompts += 1,
                Some("token_count") => tokens = tokens.max(p["info"]["total_token_usage"]["total_tokens"].as_i64().unwrap_or(0)),
                Some("item_completed") if p["item"]["type"] == "FileChange" => {
                    for (path, v) in p["item"]["changes"].as_object().into_iter().flatten() {
                        let target = v["move_path"].as_str().unwrap_or(path).to_string();
                        let text = match v["type"].as_str() {
                            Some("add") => v["content"].as_str().unwrap_or("").to_string(),
                            Some("update") => crate::added_from_diff(v["unified_diff"].as_str().unwrap_or("")),
                            _ => continue,
                        };
                        changes.push((target, text, model.clone(), ts));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    let abs = |p: &str, wd: &str| if p.starts_with('/') || p.get(1..2) == Some(":") || wd.is_empty() { p.to_string() } else { format!("{}/{}", wd.trim_end_matches('/'), p) };
    // FileChange 是 Codex 系确认写进去的改动；没有它才退回读补丁调用
    if changes.is_empty() {
        for (call, patch, wd, m, ts) in patches {
            if !failed.contains(&call) {
                for (p, t) in apply_patch_files(&patch) {
                    changes.push((abs(&p, &wd), t, m.clone(), ts));
                }
            }
        }
    }
    for (p, text, m, ts) in changes {
        if !m.is_empty() { *models.entry(m.clone()).or_default() += 1; }
        crate::push_text(&mut g.evs, pm, &abs(&p, &cwd), &text, Src::Trae, &m, &id, ts);
    }
    for (call, cmd, wd, m, ts) in shells {
        if !failed.contains(&call) {
            for rel in crate::shell_targets(&cmd, &wd, pm) {
                crate::push_rel(&mut g.evs, &rel, &cmd, Src::Trae, &m, &id, ts);
            }
        }
    }
    let model = models.into_iter().max_by_key(|(m, n)| (*n, m.clone())).map(|(m, _)| m).unwrap_or(model);
    if start > 0 {
        g.sessions.push(Session { src: Src::Trae, id, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true, provider });
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for f in rollouts() {
        let gk = format!("trae:{}", f.display());
        let fp = fingerprint(&f);
        if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
            n += g.sessions.len();
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        if head_cwd(&f).map(|c| pm.in_project(&c)).unwrap_or(false) {
            let name = f.file_name().unwrap_or_default().to_string_lossy().replace(".jsonl", "").replace(".zst", "");
            parse(&read_maybe_zst(&f), &name, pm, &mut g);
        }
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("trae: 项目相关会话 {n} 个（TRAE CLI）", "trae: {n} sessions for this project (TRAE CLI)"));
    }
}

fn mtime(p: &Path) -> i64 {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for f in rollouts() {
        let Some(cwd) = head_cwd(&f) else { continue };
        let e = m.entry(cwd.clone()).or_insert(Activity { src: Src::Trae, cwd, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(mtime(&f));
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Trae);
    let fs = rollouts();
    let mt: Vec<i64> = fs.iter().map(|f| mtime(f)).filter(|&t| t > 0).collect();
    Health {
        source: "trae-cli".into(),
        path: base.display().to_string(),
        readable: base.is_dir(),
        format: "sessions/YYYY/MM/DD/rollout-*.jsonl".into(),
        records: fs.len() as u64,
        earliest: mt.iter().min().copied().map(date_cn),
        latest: mt.iter().max().copied().map(date_cn),
        note: String::new(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pm() -> PathMap {
        PathMap { known: ["src/a.go".to_string(), "src/b.go".to_string(), "src/c.go".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] }
    }

    #[test]
    fn file_changes_from_item_completed() {
        let lines = [
            json!({ "timestamp": "2026-10-01T09:00:00.000Z", "type": "session_meta", "payload": { "id": "t1", "cwd": "/w/demo", "model_provider": "trae" } }),
            json!({ "timestamp": "2026-10-01T09:00:00.500Z", "type": "turn_context", "payload": { "cwd": "/w/demo", "model": "doubao-seed-code" } }),
            json!({ "timestamp": "2026-10-01T09:00:01.000Z", "type": "event_msg", "payload": { "type": "user_message", "message": "add a" } }),
            json!({ "timestamp": "2026-10-01T09:00:02.000Z", "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "changes": {
                "/w/demo/src/a.go": { "type": "add", "content": "package demo\nfunc Alpha() {}\n" },
                "/w/demo/src/b.go": { "type": "update", "unified_diff": "@@ -1 +1,2 @@\n package demo\n+func Beta() {}\n" }
            } } } }),
        ].iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n");
        let mut g = FileGroup::default();
        parse(&lines, "x", &pm(), &mut g);
        let files: Vec<&str> = g.evs.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(files, vec!["src/a.go", "src/a.go", "src/b.go"]);
        assert_eq!(g.sessions[0].model, "doubao-seed-code");
        assert_eq!(g.sessions[0].provider.as_deref(), Some("trae"));
        assert_eq!(g.sessions[0].prompts, 1);
    }

    #[test]
    fn patch_calls_in_history_mutation_unless_they_failed() {
        let ok = "*** Begin Patch\n*** Add File: src/c.go\n+func Gamma() {}\n*** End Patch";
        let bad = "*** Begin Patch\n*** Add File: src/b.go\n+func Broken() {}\n*** End Patch";
        let lines = [
            json!({ "timestamp": "2026-10-01T09:00:00.000Z", "type": "session_meta", "payload": { "id": "t2", "cwd": "/w/demo" } }),
            json!({ "timestamp": "2026-10-01T09:00:02.000Z", "type": "history_mutation", "payload": { "operation": "append", "items": [
                { "type": "custom_tool_call", "call_id": "c1", "name": "apply_patch", "input": ok },
                { "type": "custom_tool_call_output", "call_id": "c1", "output": "Success. Updated the following files:\nA src/c.go" },
                { "type": "function_call", "call_id": "c2", "name": "apply_patch", "arguments": json!({ "input": bad }).to_string() },
                { "type": "function_call_output", "call_id": "c2", "output": "Exit code: 1\nverification failed" }
            ] } }),
        ].iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n");
        let mut g = FileGroup::default();
        parse(&lines, "x", &pm(), &mut g);
        let files: Vec<&str> = g.evs.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(files, vec!["src/c.go"]);
    }
}
