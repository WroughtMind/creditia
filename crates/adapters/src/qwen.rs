//! Qwen Code 新版的会话记录：`~/.qwen/projects/<目录名>/chats/<会话>.jsonl`（`QWEN_RUNTIME_DIR` 可改根目录）。
//! 旧版和 Gemini CLI 同格式（`tmp/<项目哈希>/chats/session-*.json`），见 gemini.rs。
//! 每行一条记录：uuid、sessionId、timestamp、type（user / assistant / tool_result / system）、cwd、gitBranch、
//! message（Gemini API 的 Content：parts 里的 functionCall / functionResponse）；assistant 带 model 与 usageMetadata，
//! tool_result 带 toolCallResult（callId、status）。只认 status 为 success 的调用：
//! write_file / edit 写下的文本是证据，run_shell_command 的命令原文是候选证据。

use crate::{any_ts, new_text, path_arg, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn projects_dir() -> PathBuf {
    source_dir(Src::Qwen).join("projects")
}

/// 全部会话文件：chats/ 下的 .jsonl，以及下一层子目录里的
fn chat_files() -> Vec<PathBuf> {
    let mut out = vec![];
    let jsonl = |d: &Path| -> Vec<PathBuf> { std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")).collect() };
    for p in std::fs::read_dir(projects_dir()).into_iter().flatten().flatten() {
        let chats = p.path().join("chats");
        out.extend(jsonl(&chats));
        for sub in std::fs::read_dir(&chats).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
            out.extend(jsonl(&sub.path()));
        }
    }
    out.sort();
    out
}

#[derive(Default)]
struct Parsed {
    id: String,
    cwd: String,
    branch: Option<String>,
    /// (路径, 文本, 命令的工作目录, 模型, 时间)；命令的路径为空
    edits: Vec<(String, String, String, String, i64)>,
    start: i64,
    end: i64,
    prompts: i64,
    tokens: i64,
    models: HashMap<String, usize>,
}

fn parse(text: &str) -> Parsed {
    let mut p = Parsed::default();
    let recs: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    // 成功的调用 id：toolCallResult.status 为 success；没有 toolCallResult 的看回复里有没有 error
    let mut ok: HashSet<String> = HashSet::new();
    for r in recs.iter().filter(|r| r["type"] == "tool_result") {
        let parts = r["message"]["parts"].as_array().cloned().unwrap_or_default();
        match r["toolCallResult"]["status"].as_str() {
            Some(s) => {
                if s == "success" {
                    ok.extend(r["toolCallResult"]["callId"].as_str().map(String::from));
                }
            }
            None => ok.extend(parts.iter().filter(|x| x["functionResponse"]["response"].get("error").is_none()).filter_map(|x| x["functionResponse"]["id"].as_str().map(String::from))),
        }
    }
    for r in &recs {
        let ts = any_ts(&r["timestamp"]).unwrap_or(0);
        if ts > 0 {
            p.start = if p.start == 0 { ts } else { p.start.min(ts) };
            p.end = p.end.max(ts);
        }
        if p.id.is_empty() {
            p.id = r["sessionId"].as_str().unwrap_or("").to_string();
        }
        let cwd = r["cwd"].as_str().unwrap_or("");
        if p.cwd.is_empty() && !cwd.is_empty() {
            p.cwd = cwd.to_string();
        }
        if let Some(b) = r["gitBranch"].as_str().filter(|b| !b.is_empty()) {
            p.branch = Some(b.to_string());
        }
        match r["type"].as_str() {
            Some("user") if r["subtype"].is_null() && matches!(r["provenance"].as_str(), None | Some("real_user")) => p.prompts += 1,
            Some("assistant") => {
                let model = r["model"].as_str().unwrap_or("").to_string();
                if !model.is_empty() {
                    *p.models.entry(model.clone()).or_default() += 1;
                }
                p.tokens += r["usageMetadata"]["totalTokenCount"].as_i64().unwrap_or(0);
                for c in r["message"]["parts"].as_array().into_iter().flatten().map(|x| &x["functionCall"]).filter(|c| c.is_object()) {
                    if !ok.contains(c["id"].as_str().unwrap_or("")) {
                        continue;
                    }
                    let (name, args) = (c["name"].as_str().unwrap_or(""), &c["args"]);
                    if name == "run_shell_command" {
                        let dir = args["directory"].as_str().map(|d| if Path::new(d).is_absolute() { d.to_string() } else { format!("{}/{d}", cwd.trim_end_matches('/')) }).unwrap_or_else(|| cwd.to_string());
                        if let Some(cmd) = args["command"].as_str().filter(|s| !s.trim().is_empty()) {
                            p.edits.push((String::new(), cmd.to_string(), dir, model.clone(), ts));
                        }
                        continue;
                    }
                    if let (Some(path), Some(t)) = (path_arg(args), new_text(name, args)) {
                        let abs = if Path::new(&path).is_absolute() { path } else { format!("{}/{path}", cwd.trim_end_matches('/')) };
                        p.edits.push((abs, t, String::new(), model.clone(), ts));
                    }
                }
            }
            _ => {}
        }
    }
    p
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for f in chat_files() {
        let gk = format!("qwen:{}", f.display());
        let fp = fingerprint(&f);
        if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
            n += 1;
            continue;
        }
        let p = parse(&std::fs::read_to_string(&f).unwrap_or_default());
        if p.id.is_empty() || !pm.in_project(&p.cwd) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        for (path, text, dir, model, ts) in &p.edits {
            if path.is_empty() {
                for rel in crate::shell_targets(text, dir, pm) {
                    crate::push_rel(&mut g.evs, &rel, text, Src::Qwen, model, &p.id, *ts);
                }
            } else {
                crate::push_text(&mut g.evs, pm, path, text, Src::Qwen, model, &p.id, *ts);
            }
        }
        let model = p.models.iter().max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone()))).map(|(k, _)| k.clone()).unwrap_or_default();
        g.sessions.push(Session { src: Src::Qwen, id: p.id.clone(), model, start: p.start, end: p.end, tokens: p.tokens, branch: p.branch.clone(), prompts: p.prompts, reviewer: false, has_content: true, provider: None });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("qwen: 项目相关会话 {n} 个（新版 projects/ 目录）", "qwen: {n} sessions for this project (newer projects/ folder)"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for f in chat_files() {
        let p = parse(&std::fs::read_to_string(&f).unwrap_or_default());
        if p.cwd.is_empty() {
            continue;
        }
        let e = m.entry(p.cwd.clone()).or_insert(Activity { src: Src::Qwen, cwd: p.cwd.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(p.end);
    }
    m.into_values().collect()
}

/// 旧版（tmp/ 下的哈希目录）的体检结果加上新版 projects/ 下的会话
pub fn doctor() -> Health {
    let mut h = crate::gemini::doctor(Src::Qwen);
    let files = chat_files();
    if files.is_empty() {
        return h;
    }
    let mt: Vec<i64> = files.iter().map(|f| fingerprint(f).0 as i64).filter(|&t| t > 0).collect();
    h.path = projects_dir().display().to_string();
    h.readable = true;
    h.format = format!("projects/<dir>/chats/*.jsonl + {}", h.format);
    h.records += files.len() as u64;
    h.earliest = mt.iter().min().copied().map(date_cn).into_iter().chain(h.earliest.take()).min();
    h.latest = mt.iter().max().copied().map(date_cn).into_iter().chain(h.latest.take()).max();
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn records_to_edits() {
        let base = |t: &str, ts: &str| json!({ "uuid": ts, "sessionId": "s1", "timestamp": ts, "type": t, "cwd": "/p", "gitBranch": "main", "version": "0.9.0" });
        let mut user = base("user", "2026-10-01T10:00:00Z");
        user["message"] = json!({ "role": "user", "parts": [{ "text": "go" }] });
        let mut asst = base("assistant", "2026-10-01T10:00:05Z");
        asst["model"] = json!("qwen3-coder-plus");
        asst["usageMetadata"] = json!({ "totalTokenCount": 120 });
        asst["message"] = json!({ "role": "model", "parts": [
            { "functionCall": { "id": "a", "name": "write_file", "args": { "file_path": "/p/x.py", "content": "x = 1" } } },
            { "functionCall": { "id": "b", "name": "edit", "args": { "file_path": "x.py", "old_string": "x = 1", "new_string": "x = 1\ny = 2" } } },
            { "functionCall": { "id": "c", "name": "edit", "args": { "file_path": "x.py", "old_string": "q", "new_string": "z = 3" } } },
            { "functionCall": { "id": "d", "name": "run_shell_command", "args": { "command": "ls", "directory": "src" } } } ] });
        let res = |id: &str, status: &str| {
            let mut r = base("tool_result", "2026-10-01T10:00:06Z");
            r["message"] = json!({ "role": "user", "parts": [{ "functionResponse": { "id": id, "name": "x", "response": { "output": "" } } }] });
            r["toolCallResult"] = json!({ "callId": id, "status": status });
            r
        };
        let mut sys = base("system", "2026-10-01T10:00:07Z");
        sys["subtype"] = json!("ui_telemetry");
        let text = [user, asst, res("a", "success"), res("b", "success"), res("c", "error"), res("d", "success"), sys].iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n");
        let p = parse(&text);
        assert_eq!((p.id.as_str(), p.cwd.as_str(), p.prompts, p.tokens), ("s1", "/p", 1, 120));
        assert_eq!(p.branch.as_deref(), Some("main"));
        let got: Vec<(&str, &str, &str)> = p.edits.iter().map(|(a, b, c, _, _)| (a.as_str(), b.as_str(), c.as_str())).collect();
        // 报错的 c 不算
        assert_eq!(got, vec![("/p/x.py", "x = 1", ""), ("/p/x.py", "y = 2\n", ""), ("", "ls", "/p/src")]);
    }
}
