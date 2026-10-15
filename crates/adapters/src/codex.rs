//! Codex：`state_5.sqlite` 的 threads 表（模型、token、分支、cwd）+ rollout jsonl 里的 FileChange 事件

use crate::{open_ro, read_maybe_zst, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, parse_iso};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    let conn = open_ro(&db).map_err(|e| creditia_core::tr!("codex: 线程索引不可读 {e}", "codex: thread index unreadable {e}"))?;
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
    let base = source_dir(Src::Codex);
    if !base.join("state_5.sqlite").exists() {
        return; // 没装 Codex 不是问题
    }
    let rows = match threads(&base) {
        Ok(r) => r,
        Err(e) => return st.warnings.push(e),
    };
    let files = rollout_files(&base);
    let (mut missing, mut found) = (0, 0);
    for t in rows.into_iter().filter(|t| pm.in_project(&t.cwd) || pm.in_project(&t.origin)) {
        let name = Path::new(&t.rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default();
        let file = files.get(&name);
        let reviewer = t.model == "codex-auto-review";
        let gk = format!("codex:{}", t.id);
        let fp = file.map(|f| fingerprint(f)).unwrap_or((0, 0));
        if file.is_some() { found += 1 } else { missing += 1 }
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let mut prompts = 0;
        let mut provider: Option<String> = None;
        if let Some(f) = file {
            let data = read_maybe_zst(f);
            let mut cur_model = t.model.clone();
            let mut shells: Vec<(String, String, String, String, i64)> = vec![];
            let mut failed: std::collections::HashSet<String> = std::collections::HashSet::new();
            for line in data.lines() {
                if line.contains("\"task_started\"") {
                    prompts += 1;
                }
                if provider.is_none() && line.contains("\"session_meta\"") {
                    if let Ok(o) = serde_json::from_str::<Value>(line) {
                        provider = o["payload"]["model_provider"].as_str().filter(|p| !p.is_empty()).map(String::from);
                    }
                }
                let is_ctx = line.contains("\"turn_context\"");
                let is_call = line.contains("\"function_call") || line.contains("\"local_shell_call");
                if !line.contains("\"FileChange\"") && !is_ctx && !is_call {
                    continue;
                }
                let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
                let p = &o["payload"];
                if o["type"] == "turn_context" {
                    if let Some(m) = p["model"].as_str() {
                        cur_model = m.to_string();
                    }
                    continue;
                }
                // shell 调用：命令原文（heredoc、脚本里的新文本）作为它提到的文件的候选证据；退出码非 0 的不算
                if is_call {
                    let ts = o["timestamp"].as_str().and_then(parse_iso).unwrap_or(t.start);
                    match p["type"].as_str() {
                        Some("function_call") | Some("local_shell_call") => {
                            if let Some((cmd, wd)) = shell_call(p, &t.cwd) {
                                let id = p["call_id"].as_str().unwrap_or("").to_string();
                                shells.push((id, cmd, wd, cur_model.clone(), ts));
                            }
                        }
                        Some("function_call_output") if call_failed(&p["output"]) => {
                            failed.insert(p["call_id"].as_str().unwrap_or("").to_string());
                        }
                        _ => {}
                    }
                    continue;
                }
                let it = &p["item"];
                if it["type"] != "FileChange" {
                    continue;
                }
                let ts = o["timestamp"].as_str().and_then(parse_iso).unwrap_or(t.start);
                if let Some(ch) = it["changes"].as_object() {
                    for (path, v) in ch {
                        let target = v["move_path"].as_str().unwrap_or(path);
                        match v["type"].as_str() {
                            Some("add") => {
                                crate::push_text(&mut g.evs, pm, target, v["content"].as_str().unwrap_or(""), Src::Codex, &cur_model, &t.id, ts);
                            }
                            Some("update") => {
                                let added: String = v["unified_diff"]
                                    .as_str()
                                    .unwrap_or("")
                                    .lines()
                                    .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                                    .map(|l| format!("{}\n", &l[1..]))
                                    .collect();
                                crate::push_text(&mut g.evs, pm, target, &added, Src::Codex, &cur_model, &t.id, ts);
                            }
                            _ => {}
                        }
                    }
                }
            }
            for (id, cmd, wd, model, ts) in shells {
                if !failed.contains(&id) {
                    for rel in crate::shell_targets(&cmd, &wd, pm) {
                        crate::push_rel(&mut g.evs, &rel, &cmd, Src::Codex, &model, &t.id, ts);
                    }
                }
            }
        }
        if reviewer {
            g.evs.clear();
        }
        g.sessions.push(Session { src: Src::Codex, id: t.id.clone(), model: t.model, start: t.start, end: t.end, tokens: t.tokens, branch: t.branch, prompts, reviewer, has_content: file.is_some(), provider });
        store.groups.insert(gk, g);
    }
    st.notes.push(creditia_core::tr!("codex: 线程 {} 个，内容文件存在 {found}，缺失 {missing}", "codex: {} threads, {found} with content files, {missing} missing", found + missing));
}

pub fn discover() -> Vec<Activity> {
    let base = source_dir(Src::Codex);
    let Ok(rows) = threads(&base) else { return vec![] };
    let mut m: HashMap<String, Activity> = HashMap::new();
    for t in rows {
        let e = m.entry(t.cwd.clone()).or_insert(Activity { src: Src::Codex, cwd: t.cwd.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(t.end);
        if !t.origin.is_empty() {
            e.remote = Some(t.origin);
        }
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Codex);
    let mut h = Health { source: "codex".into(), path: base.display().to_string(), format: "state_5.sqlite + rollout jsonl".into(), ..Default::default() };
    match threads(&base) {
        Ok(rows) => {
            h.readable = true;
            h.records = rows.len() as u64;
            h.earliest = rows.iter().map(|t| t.start).min().map(date_cn);
            h.latest = rows.iter().map(|t| t.end).max().map(date_cn);
            let files = rollout_files(&base);
            let have = rows.iter().filter(|t| files.contains_key(&Path::new(&t.rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default())).count();
            h.note = creditia_core::tr!("{} 个线程有内容文件", "{} threads have content files", have);
        }
        Err(e) => h.note = e,
    }
    h
}

/// Codex 的 shell 调用：`shell` / `exec_command` 的参数（JSON 字符串），或 `local_shell_call` 的 action。返回（命令原文，工作目录）
pub(crate) fn shell_call(p: &Value, cwd: &str) -> Option<(String, String)> {
    let args: Value = match p["type"].as_str() {
        Some("local_shell_call") => p["action"].clone(),
        _ => {
            let name = p["name"].as_str().unwrap_or("");
            if !(name == "shell" || name == "exec_command" || name == "container.exec" || name == "local_shell") {
                return None;
            }
            serde_json::from_str(p["arguments"].as_str()?).ok()?
        }
    };
    let cmd = match (&args["command"], &args["cmd"]) {
        (Value::Array(a), _) => {
            let parts: Vec<&str> = a.iter().filter_map(|x| x.as_str()).collect();
            // ["bash", "-lc", "<脚本>"]：取脚本本身
            if parts.len() >= 3 && (parts[1] == "-lc" || parts[1] == "-c") { parts[2..].join(" ") } else { parts.join(" ") }
        }
        (Value::String(c), _) | (_, Value::String(c)) => c.clone(),
        _ => return None,
    };
    let wd = args["workdir"].as_str().or(args["working_directory"].as_str()).unwrap_or(cwd).to_string();
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
            let n: String = lower[i + key.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
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
        assert_eq!(shell_call(&p, "/cwd"), Some(("cat > a.rs <<'EOF'\nx\nEOF".to_string(), "/r".to_string())));
        let p = json!({ "type": "function_call", "name": "exec_command", "arguments": json!({ "cmd": "echo hi > b.ts" }).to_string() });
        assert_eq!(shell_call(&p, "/cwd"), Some(("echo hi > b.ts".to_string(), "/cwd".to_string())));
        assert_eq!(shell_call(&json!({ "type": "function_call", "name": "apply_patch", "arguments": "{}" }), "/cwd"), None);
        assert!(!call_failed(&json!(json!({ "output": "", "metadata": { "exit_code": 0 } }).to_string())));
        assert!(call_failed(&json!(json!({ "output": "", "metadata": { "exit_code": 2 } }).to_string())));
        assert!(call_failed(&json!("Process exited with code 1\nOutput: nope")));
        assert!(!call_failed(&json!("Exit code: 0\nWall time: 1s")));
    }
}
