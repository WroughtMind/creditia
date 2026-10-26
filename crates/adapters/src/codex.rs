//! Codex：`state_5.sqlite` 的 threads 表（模型、token、分支、cwd）+ rollout jsonl 里的 FileChange 事件

use crate::{open_ro, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, parse_iso};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
mod histories;

struct Thread {
    id: String,
    rollout: String,
    start: i64,
    end: i64,
    tokens: i64,
    model: String,
    branch: Option<String>,
    cwd: String,
    origin: String,
}

fn threads(base: &Path) -> Result<Vec<Thread>, String> {
    let db = base.join("state_5.sqlite");
    let conn = open_ro(&db).map_err(|e| {
        creditia_core::tr!(
            "codex: 线程索引不可读 {e}",
            "codex: thread index unreadable {e}"
        )
    })?;
    let mut stmt = conn
        .prepare("select id, rollout_path, created_at, updated_at, tokens_used, coalesce(model,''), git_branch, cwd, coalesce(git_origin_url,'') from threads")
        .map_err(|e| creditia_core::tr!("codex: threads 表结构不认识 {e}", "codex: unknown threads table layout {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Thread {
                id: r.get(0)?,
                rollout: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
                tokens: r.get(4)?,
                model: r.get(5)?,
                branch: r.get(6)?,
                cwd: r.get(7)?,
                origin: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(rows)
}

/// 会话文件索引：文件名 → 实际路径（可能已压缩、已归档）
fn rollout_files(base: &Path) -> HashMap<String, PathBuf> {
    let mut files = HashMap::new();
    for dir in ["sessions", "archived_sessions"] {
        for e in walkdir::WalkDir::new(base.join(dir)).into_iter().flatten() {
            let name = e.file_name().to_string_lossy().replace(".zst", "");
            if name.ends_with(".jsonl") {
                files.insert(name, e.path().to_path_buf());
            }
        }
    }
    files
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let rows = histories::all(&mut st.warnings);
    // Reparse once to join background commands with their final poll receipts.
    store
        .groups
        .retain(|key, _| !key.starts_with("codex:") || key.starts_with("codex:rollout-v4:"));
    let (mut missing, mut found) = (0, 0);
    for (t, file) in rows
        .into_iter()
        .filter(|(t, _)| pm.in_project(&histories::cwd(&t.cwd)) || pm.in_project(&t.origin))
    {
        let file = file.as_ref();
        let reviewer = t.model == "codex-auto-review";
        let gk = format!("codex:rollout-v4:{}", t.id);
        let fp = file.map(|f| fingerprint(f)).unwrap_or((0, 0));
        if file.is_some() {
            found += 1
        } else {
            missing += 1
        }
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        let mut prompts = 0;
        let mut provider: Option<String> = None;
        if let Some(f) = file {
            let data = match histories::read_rollout(f) {
                Ok(data) => data,
                Err(e) => {
                    st.warnings.push(format!("codex: {}: {e}", f.display()));
                    continue;
                }
            };
            if fingerprint(f) != fp {
                st.warnings.push(format!(
                    "codex: {}: rollout changed while reading",
                    f.display()
                ));
                continue;
            }
            if data
                .lines()
                .find(|line| !line.trim().is_empty())
                .and_then(|line| serde_json::from_str::<Value>(line).ok())
                .is_some_and(|row| row["type"] == "session_meta" && row["payload"]["id"] == t.id)
            {
                g.reconciled_sessions.push((Src::Codex, t.id.clone()));
            }
            let mut cur_model = t.model.clone();
            let mut shells: Vec<(String, String, String, String, i64)> = vec![];
            let mut patches = HashMap::new();
            let mut failed: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut completed = std::collections::HashSet::new();
            let mut command_ids = std::collections::HashSet::new();
            let mut running: HashMap<u64, String> = HashMap::new();
            let mut polls: HashMap<String, (u64, String)> = HashMap::new();
            for line in data.lines() {
                if line.contains("\"task_started\"") {
                    prompts += 1;
                }
                if provider.is_none() && line.contains("\"session_meta\"") {
                    if let Ok(o) = serde_json::from_str::<Value>(line) {
                        provider = o["payload"]["model_provider"]
                            .as_str()
                            .filter(|p| !p.is_empty())
                            .map(String::from);
                    }
                }
                let is_ctx = line.contains("\"turn_context\"");
                let is_call = line.contains("\"function_call")
                    || line.contains("\"local_shell_call")
                    || line.contains("\"custom_tool_call");
                if !line.contains("\"FileChange\"")
                    && !line.contains("\"CommandExecution\"")
                    && !line.contains("\"patch_apply_end\"")
                    && !is_ctx
                    && !is_call
                {
                    continue;
                }
                let Ok(o) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                let p = &o["payload"];
                if o["type"] == "turn_context" {
                    if let Some(m) = p["model"].as_str() {
                        cur_model = m.to_string();
                    }
                    continue;
                }
                let ts = o["timestamp"]
                    .as_str()
                    .and_then(parse_iso)
                    .unwrap_or(t.start);
                // Native event and response_item can describe the same call
                // on opposite sides of a second boundary. Prefer the native
                // completion once; do not persist two sets of line evidence.
                if p["type"] == "patch_apply_end" {
                    patches.remove(p["call_id"].as_str().unwrap_or(""));
                }
                histories::patch_receipt(p, &mut patches, &mut g, pm, &t, &cur_model, ts);
                if p["type"] == "item_completed" {
                    histories::command_patch(&p["item"], &mut g, pm, &t, &cur_model, ts);
                }
                // shell 调用：命令原文（heredoc、脚本里的新文本）作为它提到的文件的候选证据；退出码非 0 的不算
                if is_call {
                    let ts = o["timestamp"]
                        .as_str()
                        .and_then(parse_iso)
                        .unwrap_or(t.start);
                    match p["type"].as_str() {
                        Some("function_call") | Some("local_shell_call") => {
                            if let Some((cmd, wd)) = shell_call(p, &t.cwd) {
                                let id = p["call_id"].as_str().unwrap_or("").to_string();
                                command_ids.insert(id.clone());
                                shells.push((id, cmd, wd, cur_model.clone(), ts));
                            } else if p["name"] == "write_stdin" {
                                let args = p["arguments"]
                                    .as_str()
                                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                                if let Some(args) = args {
                                    if let Some(sid) = args["session_id"].as_u64() {
                                        if let Some(origin) = running.get(&sid) {
                                            if args["chars"].as_str().is_none_or(str::is_empty) {
                                                if let Some(id) = p["call_id"]
                                                    .as_str()
                                                    .filter(|s| !s.is_empty())
                                                {
                                                    polls.insert(id.into(), (sid, origin.clone()));
                                                }
                                            } else {
                                                // Interactive input can change the command's meaning;
                                                // its initial text alone is no longer write evidence.
                                                failed.insert(origin.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some("function_call_output") => {
                            let id = p["call_id"].as_str().unwrap_or("");
                            let poll = polls
                                .remove(id)
                                .filter(|(sid, origin)| running.get(sid) == Some(origin));
                            let origin = poll
                                .as_ref()
                                .map(|(_, origin)| origin.as_str())
                                .unwrap_or(id);
                            if !command_ids.contains(origin) {
                                continue;
                            }
                            if let Some(code) = histories::process_exit_code(&p["output"]) {
                                if code == 0 {
                                    completed.insert(origin.to_string());
                                } else {
                                    failed.insert(origin.to_string());
                                }
                                if let Some((sid, _)) = poll {
                                    running.remove(&sid);
                                }
                            } else if poll.is_none() {
                                if let Some(sid) = histories::running_process(&p["output"]) {
                                    running.insert(sid, origin.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                let it = &p["item"];
                let changes = if p["type"] == "patch_apply_end" && p["success"] == true {
                    &p["changes"]
                } else if p["type"] == "item_completed"
                    && it["type"] == "FileChange"
                    && !matches!(
                        it["status"].as_str(),
                        Some("failed" | "declined" | "inProgress")
                    )
                {
                    &it["changes"]
                } else {
                    continue;
                };
                if let Some(ch) = changes.as_object() {
                    for (path, v) in ch {
                        let target =
                            histories::target(v["move_path"].as_str().unwrap_or(path), &t.cwd);
                        match v["type"].as_str() {
                            Some("add") => {
                                crate::push_text(
                                    &mut g.evs,
                                    pm,
                                    &target,
                                    v["content"].as_str().unwrap_or(""),
                                    Src::Codex,
                                    &cur_model,
                                    &t.id,
                                    ts,
                                );
                            }
                            Some("update") => {
                                let added: String = v["unified_diff"]
                                    .as_str()
                                    .unwrap_or("")
                                    .lines()
                                    .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                                    .map(|l| format!("{}\n", &l[1..]))
                                    .collect();
                                crate::push_text(
                                    &mut g.evs,
                                    pm,
                                    &target,
                                    &added,
                                    Src::Codex,
                                    &cur_model,
                                    &t.id,
                                    ts,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            for (id, cmd, wd, model, ts) in shells {
                if !id.is_empty() && !failed.contains(&id) && completed.contains(&id) {
                    for rel in crate::shell_targets(&cmd, &histories::cwd(&wd), pm) {
                        crate::push_rel(&mut g.evs, &rel, &cmd, Src::Codex, &model, &t.id, ts);
                    }
                }
            }
        }
        if reviewer {
            g.evs.clear();
        }
        g.sessions.push(Session {
            src: Src::Codex,
            id: t.id.clone(),
            model: t.model,
            start: t.start,
            end: t.end,
            tokens: t.tokens,
            branch: t.branch,
            prompts,
            reviewer,
            has_content: file.is_some(),
            provider,
        });
        store.groups.insert(gk, g);
    }
    st.notes.push(creditia_core::tr!(
        "codex: 线程 {} 个，内容文件存在 {found}，缺失 {missing}",
        "codex: {} threads, {found} with content files, {missing} missing",
        found + missing
    ));
}

pub fn discover() -> Vec<Activity> {
    let rows = histories::all(&mut vec![]);
    let mut m: HashMap<String, Activity> = HashMap::new();
    for (t, _) in rows {
        let cwd = histories::cwd(&t.cwd);
        let e = m.entry(cwd.clone()).or_insert(Activity {
            src: Src::Codex,
            cwd,
            remote: None,
            sessions: 0,
            last: 0,
        });
        e.sessions += 1;
        e.last = e.last.max(t.end);
        if !t.origin.is_empty() {
            e.remote = Some(t.origin);
        }
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let mut warnings = vec![];
    let rows = histories::all(&mut warnings);
    let have = rows.iter().filter(|(_, f)| f.is_some()).count();
    Health {
        source: "codex".into(),
        path: histories::roots()
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        format: "native rollout JSONL (optional state_5.sqlite); MCPJam local Codex homes".into(),
        readable: !rows.is_empty(),
        records: rows.len() as u64,
        earliest: rows.iter().map(|(t, _)| t.start).min().map(date_cn),
        latest: rows.iter().map(|(t, _)| t.end).max().map(date_cn),
        note: format!(
            "{have} native transcripts; duplicate thread IDs counted once. {}",
            warnings.join("; ")
        ),
        ..Default::default()
    }
}

/// Codex 的 shell 调用：`shell` / `exec_command` 的参数（JSON 字符串），或 `local_shell_call` 的 action。返回（命令原文，工作目录）
pub(crate) fn shell_call(p: &Value, cwd: &str) -> Option<(String, String)> {
    let args: Value = match p["type"].as_str() {
        Some("local_shell_call") => p["action"].clone(),
        _ => {
            let name = p["name"].as_str().unwrap_or("");
            if !(name == "shell"
                || name == "exec_command"
                || name == "container.exec"
                || name == "local_shell")
            {
                return None;
            }
            serde_json::from_str(p["arguments"].as_str()?).ok()?
        }
    };
    let cmd = match (&args["command"], &args["cmd"]) {
        (Value::Array(a), _) => {
            let parts: Vec<&str> = a.iter().filter_map(|x| x.as_str()).collect();
            // ["bash", "-lc", "<脚本>"]：取脚本本身
            if parts.len() >= 3 && (parts[1] == "-lc" || parts[1] == "-c") {
                parts[2..].join(" ")
            } else {
                parts.join(" ")
            }
        }
        (Value::String(c), _) | (_, Value::String(c)) => c.clone(),
        _ => return None,
    };
    let wd = args["workdir"]
        .as_str()
        .or(args["working_directory"].as_str())
        .unwrap_or(cwd)
        .to_string();
    Some((cmd, wd))
}

/// shell 调用的结果是否失败：JSON 里的 exit_code，或文本里的「exit code N」「exited with code N」
pub(crate) fn call_failed(out: &Value) -> bool {
    let text = match out {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    if let Ok(v) = serde_json::from_str::<Value>(&text) {
        if let Some(c) = v["metadata"]["exit_code"].as_i64() {
            return c != 0;
        }
    }
    let lower = text.to_lowercase();
    for key in ["exit code: ", "exit code ", "exited with code "] {
        if let Some(i) = lower.find(key) {
            let n: String = lower[i + key.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(c) = n.parse::<i64>() {
                return c != 0;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shell_calls_and_exit_codes() {
        let p = json!({ "type": "function_call", "name": "shell", "arguments": json!({ "command": ["bash", "-lc", "cat > a.rs <<'EOF'\nx\nEOF"], "workdir": "/r" }).to_string(), "call_id": "c1" });
        assert_eq!(
            shell_call(&p, "/cwd"),
            Some(("cat > a.rs <<'EOF'\nx\nEOF".to_string(), "/r".to_string()))
        );
        let p = json!({ "type": "function_call", "name": "exec_command", "arguments": json!({ "cmd": "echo hi > b.ts" }).to_string() });
        assert_eq!(
            shell_call(&p, "/cwd"),
            Some(("echo hi > b.ts".to_string(), "/cwd".to_string()))
        );
        assert_eq!(
            shell_call(
                &json!({ "type": "function_call", "name": "apply_patch", "arguments": "{}" }),
                "/cwd"
            ),
            None
        );
        assert!(!call_failed(&json!(
            json!({ "output": "", "metadata": { "exit_code": 0 } }).to_string()
        )));
        assert!(call_failed(&json!(
            json!({ "output": "", "metadata": { "exit_code": 2 } }).to_string()
        )));
        assert!(call_failed(&json!(
            "Process exited with code 1\nOutput: nope"
        )));
        assert!(!call_failed(&json!("Exit code: 0\nWall time: 1s")));
    }
}
