//! GitHub Copilot：VS Code 里的 Copilot Chat（agent 模式）和 Copilot 命令行。
//!
//! VS Code 每个工作区一个目录 `<User>/workspaceStorage/<哈希>/`，`workspace.json` 写着打开的文件夹：
//! - `chatSessions/<会话>.json`（1.109 以前）或 `.jsonl`（第一行是整份快照，后面每行是一次增量：1 设值、2 往数组追加、3 删除）。
//!   每个请求的 `result.metadata.toolCallRounds` 里有编辑工具的原始参数，`toolCallResults` 里有结果，报错的不算；
//! - `GitHub.copilot-chat/transcripts/<会话>.jsonl`（较新的 Copilot Chat）：`tool.execution_start` 带参数，
//!   `tool.execution_complete` 带 success；同目录 `debug-logs/<会话>/main.jsonl` 的 `llm_request` 记着模型和 token。
//!   两份记录是同一个会话时只用证据多的那份。
//!
//! 编辑工具：create_file、replace_string_in_file、multi_replace_string_in_file、insert_edit_into_file、apply_patch。
//!
//! 命令行：`~/.copilot/session-state/<会话>/events.jsonl`，事件格式见 github/copilot-sdk 的 session-events schema：
//! `session.start`（cwd、selectedModel）、`session.model_change`、`tool.execution_start` / `tool.execution_complete`
//! （success、error、model）、`assistant.usage`（token）。编辑工具：create、edit、write、str_replace_editor、apply_patch。

use crate::{added_lines, any_ts, apply_patch_files, file_uri_path, home, source_dir, vscode_user_dirs, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ───────────────────────── 编辑工具的参数 ─────────────────────────

/// 一次工具调用写下的（路径, 新增的行）。不是编辑工具就是空的
pub fn call_edits(name: &str, args: &Value) -> Vec<(String, String)> {
    let s = |k: &str| args[k].as_str().unwrap_or("").to_string();
    let path = || args["filePath"].as_str().or(args["path"].as_str()).map(file_uri_path).unwrap_or_default();
    let one = |text: String| if path().is_empty() || text.is_empty() { vec![] } else { vec![(path(), text)] };
    match name {
        // VS Code Copilot Chat
        "create_file" | "copilot_createFile" => one(s("content")),
        "replace_string_in_file" | "copilot_replaceString" => one(added_lines(&s("oldString"), &s("newString"))),
        "insert_edit_into_file" | "copilot_insertEdit" => one(s("code")),
        "multi_replace_string_in_file" | "copilot_multiReplaceString" => args["replacements"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|r| call_edits("replace_string_in_file", r))
            .collect(),
        "apply_patch" | "copilot_applyPatch" => {
            // Copilot Chat 是 {input}；命令行的 apply_patch 参数就是补丁本身
            let patch = args.as_str().map(String::from).unwrap_or_else(|| s("input"));
            apply_patch_files(&patch)
        }
        // Copilot 命令行
        "create" | "write" => one(args["file_text"].as_str().or(args["content"].as_str()).unwrap_or("").to_string()),
        "edit" => one(added_lines(&s("old_str"), &s("new_str"))),
        "str_replace_editor" => match args["command"].as_str() {
            Some("create") => one(s("file_text")),
            Some("str_replace") => one(added_lines(&s("old_str"), &s("new_str"))),
            Some("insert") => one(s("new_str")),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// 相对路径按工作目录补全
fn abs_of(p: &str, cwd: &str) -> String {
    let p = p.replace('\\', "/");
    if p.starts_with('/') || p.get(1..2) == Some(":") || cwd.is_empty() { p } else { format!("{}/{}", cwd.trim_end_matches('/'), p.trim_start_matches("./")) }
}

/// 参数可能是对象，也可能是 JSON 字符串（apply_patch 在命令行里是补丁原文）
fn args_of(v: &Value) -> Value {
    match v {
        Value::String(t) => serde_json::from_str(t).unwrap_or_else(|_| v.clone()),
        _ => v.clone(),
    }
}

/// 工具结果里的报错：「ERROR: Your input to the tool was invalid」「Applying patch failed」
fn result_failed(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("ERROR") || t.starts_with("Applying patch failed") || t.starts_with("Error:")
}

/// 一个会话读出来的东西
#[derive(Default)]
struct Read {
    id: String,
    evs: Vec<(String, String, String, i64)>, // 绝对路径、文本、模型、时间
    model: String,
    start: i64,
    end: i64,
    tokens: i64,
    prompts: i64,
    cwd: String,
    branch: Option<String>,
}

impl Read {
    fn touch(&mut self, ts: i64) {
        if ts > 0 {
            if self.start == 0 || ts < self.start { self.start = ts; }
            self.end = self.end.max(ts);
        }
    }
    fn push(&mut self, edits: Vec<(String, String)>, model: &str, ts: i64) {
        let cwd = self.cwd.clone();
        for (p, t) in edits {
            self.evs.push((abs_of(&p, &cwd), t, model.to_string(), ts));
        }
    }
    fn into_group(self, src: Src, pm: &PathMap, g: &mut FileGroup, keep: bool) {
        let before = g.evs.len();
        for (abs, text, model, ts) in &self.evs {
            crate::push_text(&mut g.evs, pm, abs, text, src, if model.is_empty() { &self.model } else { model }, &self.id, *ts);
        }
        if self.start > 0 && (keep || g.evs.len() > before) {
            g.sessions.push(Session { src, id: self.id, model: self.model, start: self.start, end: self.end, tokens: self.tokens, branch: self.branch, prompts: self.prompts, reviewer: false, has_content: true, provider: None });
        }
    }
}

/// 出现最多的模型
fn top(counts: &HashMap<String, i64>) -> String {
    counts.iter().max_by_key(|(m, n)| (**n, (*m).clone())).map(|(m, _)| m.clone()).unwrap_or_default()
}

// ───────────────────────── VS Code ─────────────────────────

/// VS Code 系编辑器的 User 目录，加上远程开发的 ~/.vscode-server
pub fn user_dirs() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("CREDITIA_VSCODE_USER") {
        return vec![PathBuf::from(p)];
    }
    let mut v = vscode_user_dirs();
    for d in [".vscode-server/data/User", ".vscode-server-insiders/data/User", ".vscode-remote/data/User"] {
        v.push(home().join(d));
    }
    v.into_iter().filter(|d| d.is_dir()).collect()
}

/// 一个工作区目录（或者没有打开文件夹时的 emptyWindowChatSessions）
struct Ws {
    dir: PathBuf,
    folder: Option<String>,
    chats: Vec<PathBuf>,
    transcripts: Vec<PathBuf>,
}

fn list(dir: &Path, ext: &[&str]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| ext.iter().any(|e| x == *e)).unwrap_or(false)).collect();
    v.sort();
    v
}

/// chatSessions 里同一个会话有 .json 也有 .jsonl 时只读 .jsonl（VS Code 自己也是先读它）
fn chat_files(dir: &Path) -> Vec<PathBuf> {
    let all = list(dir, &["json", "jsonl"]);
    all.iter().filter(|p| p.extension().map(|x| x == "jsonl").unwrap_or(false) || !p.with_extension("jsonl").exists()).cloned().collect()
}

fn workspaces() -> Vec<Ws> {
    let mut out = vec![];
    for u in user_dirs() {
        for e in std::fs::read_dir(u.join("workspaceStorage")).into_iter().flatten().flatten() {
            let dir = e.path();
            let chats = chat_files(&dir.join("chatSessions"));
            let transcripts = list(&dir.join("GitHub.copilot-chat/transcripts"), &["jsonl"]);
            if chats.is_empty() && transcripts.is_empty() {
                continue;
            }
            let wj: Value = std::fs::read_to_string(dir.join("workspace.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let folder = wj["folder"].as_str().or(wj["workspace"].as_str()).map(file_uri_path);
            out.push(Ws { dir, folder, chats, transcripts });
        }
        let empty = u.join("globalStorage/emptyWindowChatSessions");
        let chats = chat_files(&empty);
        if !chats.is_empty() {
            out.push(Ws { dir: empty, folder: None, chats, transcripts: vec![] });
        }
    }
    out
}

/// 把 chatSessions 的 .jsonl 增量记录重放成一份完整状态
pub(crate) fn replay(text: &str) -> Option<Value> {
    let mut state: Option<Value> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // 正在写的最后一行可能不完整：前面的就是这份会话
        let Ok(e) = serde_json::from_str::<Value>(line) else { break };
        match e["kind"].as_i64() {
            Some(0) => state = Some(e["v"].clone()),
            Some(k @ 1..=3) => {
                let st = state.as_mut()?;
                let path: Vec<Value> = e["k"].as_array().cloned().unwrap_or_default();
                let (last, parent) = path.split_last()?;
                let mut cur = st;
                for key in parent {
                    cur = match key {
                        Value::String(s) => cur.get_mut(s.as_str())?,
                        Value::Number(n) => cur.get_mut(n.as_u64()? as usize)?,
                        _ => return None,
                    };
                }
                match (k, last) {
                    (1, Value::String(s)) => { cur.as_object_mut()?.insert(s.clone(), e["v"].clone()); }
                    (1, Value::Number(n)) => { *cur.get_mut(n.as_u64()? as usize)? = e["v"].clone(); }
                    (3, Value::String(s)) => { cur.as_object_mut()?.remove(s); }
                    (2, key) => {
                        let arr = match key {
                            Value::String(s) => cur.as_object_mut()?.entry(s.clone()).or_insert_with(|| Value::Array(vec![])),
                            Value::Number(n) => cur.get_mut(n.as_u64()? as usize)?,
                            _ => return None,
                        };
                        if !arr.is_array() { *arr = Value::Array(vec![]); }
                        let a = arr.as_array_mut()?;
                        if let Some(i) = e["i"].as_u64() { a.truncate(i as usize); }
                        a.extend(e["v"].as_array().cloned().unwrap_or_default());
                    }
                    _ => {}
                }
            }
            _ => return None,
        }
    }
    state
}

fn read_chat(path: &Path, cwd: &str) -> Option<Read> {
    let text = std::fs::read_to_string(path).ok()?;
    let v = if path.extension().map(|x| x == "jsonl").unwrap_or(false) { replay(&text)? } else { serde_json::from_str(&text).ok()? };
    let mut r = Read { id: v["sessionId"].as_str().map(String::from).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into()), cwd: cwd.to_string(), ..Default::default() };
    r.touch(any_ts(&v["creationDate"]).unwrap_or(0));
    r.touch(any_ts(&v["lastMessageDate"]).unwrap_or(0));
    let mut models: HashMap<String, i64> = HashMap::new();
    for req in v["requests"].as_array().into_iter().flatten() {
        let ts = any_ts(&req["timestamp"]).unwrap_or(r.start);
        r.touch(ts);
        r.prompts += 1;
        let model = req["modelId"].as_str().unwrap_or("").to_string();
        if !model.is_empty() {
            *models.entry(model.clone()).or_default() += 1;
        }
        let meta = &req["result"]["metadata"];
        for k in ["promptTokens", "outputTokens"] {
            r.tokens += meta[k].as_i64().unwrap_or(0);
        }
        let mut had = false;
        for round in meta["toolCallRounds"].as_array().into_iter().flatten() {
            for call in round["toolCalls"].as_array().into_iter().flatten() {
                let name = call["name"].as_str().unwrap_or("");
                let res = &meta["toolCallResults"][call["id"].as_str().unwrap_or("")];
                let failed = res["content"].as_array().into_iter().flatten().any(|p| p["value"].as_str().map(result_failed).unwrap_or(false));
                let edits = call_edits(name, &args_of(&call["arguments"]));
                if edits.is_empty() || failed {
                    continue;
                }
                had = true;
                r.push(edits, &model, ts);
            }
        }
        // 没有工具调用记录的（老版本的编辑模式）：回复里的 textEditGroup 就是写进文件的文本
        if !had {
            for part in req["response"].as_array().into_iter().flatten().filter(|p| p["kind"] == "textEditGroup") {
                let uri = &part["uri"];
                let p = uri["fsPath"].as_str().or(uri["path"].as_str()).map(String::from).or(uri.as_str().map(file_uri_path)).unwrap_or_default();
                let text: Vec<String> = part["edits"].as_array().into_iter().flatten().flat_map(|g| g.as_array().cloned().unwrap_or_default()).filter_map(|e| e["text"].as_str().map(String::from)).collect();
                if !p.is_empty() && !text.is_empty() {
                    r.push(vec![(p, text.join("\n"))], &model, ts);
                }
            }
        }
    }
    r.model = top(&models);
    Some(r)
}

/// 调试日志里一次会话用的模型（输出 token 最多的那个，标题、摘要这类小请求不算主模型）和 token 总数
fn debug_log(ws: &Path, sid: &str) -> (String, i64) {
    let f = ws.join("GitHub.copilot-chat/debug-logs").join(sid).join("main.jsonl");
    let mut out: HashMap<String, i64> = HashMap::new();
    let mut tokens = 0;
    for line in std::fs::read_to_string(f).unwrap_or_default().lines().filter(|l| l.contains("\"llm_request\"")) {
        let Ok(e) = serde_json::from_str::<Value>(line) else { continue };
        let a = &e["attrs"];
        let m = a["model"].as_str().unwrap_or("").to_string();
        let o = a["outputTokens"].as_i64().unwrap_or(0);
        tokens += a["inputTokens"].as_i64().unwrap_or(0) + o;
        if !m.is_empty() && m != "unknown" {
            *out.entry(m).or_default() += o.max(1);
        }
    }
    (top(&out), tokens)
}

/// transcripts 和命令行的 events.jsonl 是同一套事件
fn read_events(text: &str, fallback_id: &str, cwd: &str) -> Read {
    let mut r = Read { id: fallback_id.to_string(), cwd: cwd.to_string(), ..Default::default() };
    let mut cur_model = String::new();
    let mut models: HashMap<String, i64> = HashMap::new();
    let mut calls: HashMap<String, (String, Value, String, i64)> = HashMap::new();
    let mut shutdown_tokens = 0;
    for line in text.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else { continue };
        let d = &e["data"];
        let ts = any_ts(&e["timestamp"]).unwrap_or(0);
        match e["type"].as_str().unwrap_or("") {
            "session.start" => {
                if let Some(id) = d["sessionId"].as_str() { r.id = id.to_string(); }
                r.touch(any_ts(&d["startTime"]).unwrap_or(ts));
                if let Some(c) = d["context"]["cwd"].as_str() { r.cwd = c.to_string(); }
                r.branch = d["context"]["branch"].as_str().map(String::from);
                if let Some(m) = d["selectedModel"].as_str() { cur_model = m.to_string(); }
            }
            "session.model_change" => {
                if let Some(m) = d["newModel"].as_str() { cur_model = m.to_string(); }
            }
            "user.message" => {
                r.touch(ts);
                r.prompts += 1;
            }
            "assistant.usage" => {
                r.tokens += ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"].iter().map(|k| d[*k].as_i64().unwrap_or(0)).sum::<i64>();
                if let Some(m) = d["model"].as_str() { *models.entry(m.to_string()).or_default() += 1; }
            }
            "session.shutdown" => {
                r.touch(ts);
                for (_, mm) in d["modelMetrics"].as_object().into_iter().flatten() {
                    let u = &mm["usage"];
                    shutdown_tokens += ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"].iter().map(|k| u[*k].as_i64().unwrap_or(0)).sum::<i64>();
                }
            }
            "tool.execution_start" => {
                r.touch(ts);
                let m = d["model"].as_str().map(String::from).unwrap_or_else(|| cur_model.clone());
                calls.insert(d["toolCallId"].as_str().unwrap_or("").to_string(), (d["toolName"].as_str().unwrap_or("").to_string(), args_of(&d["arguments"]), m, ts));
            }
            "tool.execution_complete" => {
                r.touch(ts);
                let Some((name, args, m, t0)) = calls.remove(d["toolCallId"].as_str().unwrap_or("")) else { continue };
                let failed = d["success"] == false || !d["error"].is_null() || d["result"]["content"].as_str().map(result_failed).unwrap_or(false);
                let edits = call_edits(&name, &args);
                if failed || edits.is_empty() {
                    continue;
                }
                let m = d["model"].as_str().map(String::from).unwrap_or(m);
                if !m.is_empty() { *models.entry(m.clone()).or_default() += 1; }
                r.push(edits, &m, t0.max(ts));
            }
            _ => r.touch(ts),
        }
    }
    if r.tokens == 0 { r.tokens = shutdown_tokens; }
    r.model = if models.is_empty() { cur_model } else { top(&models) };
    r
}

/// 一个工作区的所有会话文件合起来的指纹：任何一个变了就整个工作区重读
fn fp_of(files: &[PathBuf]) -> (u64, u64) {
    files.iter().map(|f| creditia_core::model::fingerprint(f)).fold((0u64, 0u64), |a, b| (a.0.wrapping_add(b.0), a.1.wrapping_add(b.1).wrapping_add(1)))
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for ws in workspaces() {
        // 打开的是别的项目：跳过（Copilot 的编辑路径是绝对路径，没有 workspace.json 时靠路径认）
        if let Some(f) = &ws.folder {
            if !pm.in_project(f) {
                continue;
            }
        }
        let in_proj = ws.folder.is_some();
        let cwd = ws.folder.clone().unwrap_or_default();
        let mut files: Vec<PathBuf> = ws.chats.iter().chain(ws.transcripts.iter()).cloned().collect();
        files.extend(ws.transcripts.iter().map(|t| ws.dir.join("GitHub.copilot-chat/debug-logs").join(t.file_stem().unwrap_or_default()).join("main.jsonl")));
        let gk = format!("copilot:{}", ws.dir.display());
        let fp = fp_of(&files);
        if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
            n += g.sessions.len();
            continue;
        }
        let mut reads: HashMap<String, Read> = HashMap::new();
        for c in &ws.chats {
            if let Some(r) = read_chat(c, &cwd) {
                reads.insert(r.id.clone(), r);
            }
        }
        for t in &ws.transcripts {
            let sid = t.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let mut r = read_events(&std::fs::read_to_string(t).unwrap_or_default(), &sid, &cwd);
            let (m, tokens) = debug_log(&ws.dir, &r.id);
            if r.model.is_empty() { r.model = m; }
            if r.tokens == 0 { r.tokens = tokens; }
            match reads.get(&r.id) {
                // 同一个会话两份记录：用证据多的；模型以 chatSessions 里的为准
                Some(old) if old.evs.len() >= r.evs.len() => {}
                Some(old) => {
                    if r.model.is_empty() { r.model = old.model.clone(); }
                    reads.insert(r.id.clone(), r);
                }
                None => { reads.insert(r.id.clone(), r); }
            }
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let mut ids: Vec<String> = reads.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let r = reads.remove(&id).unwrap();
            let keep = in_proj || pm.in_project(&r.cwd);
            r.into_group(Src::Copilot, pm, &mut g, keep);
        }
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    // 命令行
    for (dir, events) in cli_sessions() {
        let gk = format!("copilot-cli:{}", dir.display());
        let fp = creditia_core::model::fingerprint(&events);
        if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
            n += g.sessions.len();
            continue;
        }
        let sid = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        let r = read_events(&std::fs::read_to_string(&events).unwrap_or_default(), &sid, "");
        let mut g = FileGroup { fp, ..Default::default() };
        let keep = pm.in_project(&r.cwd);
        r.into_group(Src::Copilot, pm, &mut g, keep);
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("copilot: 项目相关会话 {n} 个", "copilot: {n} sessions for this project"));
    }
}

fn cli_sessions() -> Vec<(PathBuf, PathBuf)> {
    let base = source_dir(Src::Copilot).join("session-state");
    let mut v: Vec<(PathBuf, PathBuf)> = std::fs::read_dir(base).into_iter().flatten().flatten().map(|e| e.path()).map(|d| (d.clone(), d.join("events.jsonl"))).filter(|(_, f)| f.is_file()).collect();
    v.sort();
    v
}

/// 命令行会话的工作目录：只读 session.start 那一行
fn cli_cwd(events: &Path) -> Option<String> {
    use std::io::BufRead;
    let f = std::fs::File::open(events).ok()?;
    for line in std::io::BufReader::new(f).lines().take(20).flatten() {
        if line.contains("\"session.start\"") {
            let v: Value = serde_json::from_str(&line).ok()?;
            return v["data"]["context"]["cwd"].as_str().map(String::from);
        }
    }
    None
}

fn mtime(p: &Path) -> i64 {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    let mut add = |cwd: String, n: u64, last: i64| {
        let e = m.entry(cwd.clone()).or_insert(Activity { src: Src::Copilot, cwd, remote: None, sessions: 0, last: 0 });
        e.sessions += n;
        e.last = e.last.max(last);
    };
    for ws in workspaces() {
        let Some(f) = ws.folder else { continue };
        let files: Vec<&PathBuf> = ws.chats.iter().chain(ws.transcripts.iter()).collect();
        add(f, files.len() as u64, files.iter().map(|p| mtime(p)).max().unwrap_or(0));
    }
    for (_, events) in cli_sessions() {
        if let Some(c) = cli_cwd(&events) {
            add(c, 1, mtime(&events));
        }
    }
    m.into_values().collect()
}

pub fn doctor() -> Vec<Health> {
    let ws = workspaces();
    let files: Vec<&PathBuf> = ws.iter().flat_map(|w| w.chats.iter().chain(w.transcripts.iter())).collect();
    let mt: Vec<i64> = files.iter().map(|p| mtime(p)).filter(|&t| t > 0).collect();
    let dirs = user_dirs();
    let vs = Health {
        source: "copilot-vscode".into(),
        path: dirs.first().map(|p| p.display().to_string()).unwrap_or_default(),
        readable: !dirs.is_empty(),
        format: "workspaceStorage/<ws>/chatSessions/*.jsonl · GitHub.copilot-chat/transcripts/*.jsonl".into(),
        records: files.len() as u64,
        earliest: mt.iter().min().copied().map(date_cn),
        latest: mt.iter().max().copied().map(date_cn),
        note: creditia_core::tr!("{} 个工作区有 Copilot Chat 记录", "{} workspaces with Copilot Chat sessions", ws.len()),
        ..Default::default()
    };
    let base = source_dir(Src::Copilot).join("session-state");
    let cs = cli_sessions();
    let mt: Vec<i64> = cs.iter().map(|(_, f)| mtime(f)).filter(|&t| t > 0).collect();
    let cli = Health {
        source: "copilot-cli".into(),
        path: base.display().to_string(),
        readable: base.is_dir(),
        format: "session-state/<id>/events.jsonl".into(),
        records: cs.len() as u64,
        earliest: mt.iter().min().copied().map(date_cn),
        latest: mt.iter().max().copied().map(date_cn),
        note: String::new(),
        ..Default::default()
    };
    vec![vs, cli]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn edit_tools_of_both_hosts() {
        assert_eq!(call_edits("create_file", &json!({ "filePath": "/r/a.ts", "content": "x\ny" })), vec![("/r/a.ts".into(), "x\ny".into())]);
        assert_eq!(call_edits("replace_string_in_file", &json!({ "filePath": "/r/a.ts", "oldString": "ctx\nold", "newString": "ctx\nnew" })), vec![("/r/a.ts".into(), "new\n".into())]);
        let multi = json!({ "replacements": [{ "filePath": "/r/a.ts", "oldString": "", "newString": "one" }, { "filePath": "/r/b.ts", "oldString": "", "newString": "two" }] });
        assert_eq!(call_edits("multi_replace_string_in_file", &multi).len(), 2);
        assert_eq!(call_edits("apply_patch", &json!({ "input": "*** Begin Patch\n*** Add File: /r/c.ts\n+made\n*** End Patch" })), vec![("/r/c.ts".into(), "made\n".into())]);
        assert_eq!(call_edits("apply_patch", &json!("*** Begin Patch\n*** Update File: src/d.rs\n@@\n+cli patch\n*** End Patch")), vec![("src/d.rs".into(), "cli patch\n".into())]);
        assert_eq!(call_edits("create", &json!({ "path": "/r/e.rs", "file_text": "fn e() {}" })), vec![("/r/e.rs".into(), "fn e() {}".into())]);
        assert_eq!(call_edits("edit", &json!({ "path": "/r/e.rs", "old_str": "a", "new_str": "a\nb" })), vec![("/r/e.rs".into(), "b\n".into())]);
        assert_eq!(call_edits("str_replace_editor", &json!({ "command": "create", "path": "/r/f.rs", "file_text": "f" })), vec![("/r/f.rs".into(), "f".into())]);
        assert!(call_edits("read_file", &json!({ "filePath": "/r/a.ts" })).is_empty());
        assert_eq!(call_edits("create_file", &json!({ "filePath": "file:///r/my%20dir/g.ts", "content": "g" }))[0].0, "/r/my dir/g.ts");
    }

    #[test]
    fn replays_incremental_chat_log() {
        let log = [
            json!({ "kind": 0, "v": { "sessionId": "s1", "creationDate": 1_787_000_000_000i64, "requests": [] } }),
            json!({ "kind": 2, "k": ["requests"], "v": [{ "requestId": "r1", "timestamp": 1_787_000_100_000i64, "modelId": "copilot/claude-sonnet-4.5", "result": {} }] }),
            json!({ "kind": 1, "k": ["requests", 0, "result"], "v": { "metadata": { "toolCallRounds": [{ "toolCalls": [
                { "name": "create_file", "id": "t1", "arguments": json!({ "filePath": "/home/me/demo/src/a.rs", "content": "fn alpha() {}" }).to_string() },
                { "name": "create_file", "id": "t2", "arguments": json!({ "filePath": "/home/me/demo/src/b.rs", "content": "fn beta() {}" }).to_string() }
            ] }], "toolCallResults": { "t2": { "content": [{ "value": "ERROR while calling tool: File already exists" }] } } } } }),
            json!({ "kind": 3, "k": ["requests", 0, "requestId"] }),
        ].iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n") + "\n{\"kind\":1,\"k\":[\"tru";
        let st = replay(&log).unwrap();
        assert_eq!(st["requests"][0]["modelId"], "copilot/claude-sonnet-4.5");
        assert!(st["requests"][0].get("requestId").is_none());
        let dir = std::env::temp_dir().join(format!("creditia-copilot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s1.jsonl");
        std::fs::write(&p, &log).unwrap();
        let r = read_chat(&p, "/home/me/demo").unwrap();
        assert_eq!(r.id, "s1");
        assert_eq!(r.model, "copilot/claude-sonnet-4.5");
        assert_eq!(r.prompts, 1);
        // 报错的那次 create_file 不算
        assert_eq!(r.evs.len(), 1);
        assert_eq!(r.evs[0].0, "/home/me/demo/src/a.rs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn event_logs_need_a_successful_completion() {
        let ev = |t: &str, d: Value| json!({ "type": t, "data": d, "timestamp": "2026-10-01T09:00:00.000Z", "id": "x", "parentId": null }).to_string();
        let log = [
            ev("session.start", json!({ "sessionId": "c1", "startTime": "2026-10-01T08:59:00.000Z", "selectedModel": "gpt-5.2", "context": { "cwd": "/home/me/demo", "branch": "main" } })),
            ev("user.message", json!({ "content": "add things" })),
            ev("session.model_change", json!({ "newModel": "claude-sonnet-4.5" })),
            ev("tool.execution_start", json!({ "toolCallId": "a", "toolName": "create", "arguments": { "path": "src/ok.rs", "file_text": "fn ok() {}" } })),
            ev("tool.execution_complete", json!({ "toolCallId": "a", "success": true, "model": "claude-sonnet-4.5" })),
            ev("tool.execution_start", json!({ "toolCallId": "b", "toolName": "edit", "arguments": { "path": "/home/me/demo/src/no.rs", "old_str": "", "new_str": "fn rejected() {}" } })),
            ev("tool.execution_complete", json!({ "toolCallId": "b", "success": false, "error": { "message": "denied" } })),
            ev("tool.execution_start", json!({ "toolCallId": "c", "toolName": "create", "arguments": { "path": "/home/me/demo/src/never.rs", "file_text": "fn never() {}" } })),
            ev("assistant.usage", json!({ "model": "claude-sonnet-4.5", "inputTokens": 100, "outputTokens": 20, "cacheReadTokens": 5 })),
        ].join("\n");
        let r = read_events(&log, "fallback", "");
        assert_eq!(r.id, "c1");
        assert_eq!(r.cwd, "/home/me/demo");
        assert_eq!(r.model, "claude-sonnet-4.5");
        assert_eq!(r.tokens, 125);
        assert_eq!(r.prompts, 1);
        assert_eq!(r.evs.len(), 1);
        assert_eq!(r.evs[0].0, "/home/me/demo/src/ok.rs");
        assert_eq!(r.branch.as_deref(), Some("main"));
    }
}
