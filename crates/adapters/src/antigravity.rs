//! Antigravity CLI（`agy`，Google 用来接替 Gemini CLI 的命令行，闭源）：`~/.gemini/antigravity-cli/`。
//! 没有公开的格式说明，下面按第三方工具对真实日志的观察来读，尚未用真实日志核对：
//! brain/<对话>/.system_generated/logs/transcript_full.jsonl（没有就读截短过的 transcript.jsonl）每行一步：
//! step_index、source（USER_EXPLICIT / MODEL / SYSTEM）、type、status、created_at、content、tool_calls[{name, args}]。
//! PLANNER_RESPONSE 里的工具调用没有 id，结果是之后第一个不是 PLANNER_RESPONSE 的 MODEL 步骤，按先后一一对上；
//! 结果的 status 不是 DONE 的不算。write_to_file（CodeContent）、replace_file_content / multi_replace_file_content
//! （TargetContent → ReplacementContent）写下的文本是证据，run_command 的 CommandLine 是候选证据。
//! 日志里不记模型 id：USER_INPUT 里的「`Model Selection` from … to Gemini 3.1 Pro (Low)」是界面上的名字，转成小写连字符。
//! 工作目录取 conversation_summaries.db、cache/conversation_metadata.json、cache/last_conversations.json、history.jsonl。

use crate::{added_lines, any_ts, file_uri_path, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    crate::source_dir(Src::Antigravity)
}

/// 对话 id → 工作目录；标成内部对话（is_internal）的 id 对应空串
fn workspaces(root: &Path) -> HashMap<String, String> {
    let mut m: HashMap<String, String> = HashMap::new();
    let mut put = |id: &str, uri: &str| {
        if !id.is_empty() && !uri.is_empty() {
            m.entry(id.to_string()).or_insert_with(|| file_uri_path(uri));
        }
    };
    if let Ok(c) = open_ro(&root.join("conversation_summaries.db")) {
        if let Ok(mut q) = c.prepare("SELECT conversation_id, workspace_uris FROM conversation_summaries") {
            let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())));
            for (id, ws) in rows.into_iter().flatten().flatten() {
                let ws: Value = serde_json::from_str(&ws).unwrap_or_default();
                put(&id, ws[0].as_str().unwrap_or(""));
            }
        }
    }
    let read = |p: PathBuf| -> Value { std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default() };
    let meta = read(root.join("cache/conversation_metadata.json"));
    let mut internal = vec![];
    for (id, e) in meta["conversations"].as_object().into_iter().flatten() {
        if e["is_internal"] == true || e["summary"]["Internal"] == true {
            internal.push(id.clone());
            continue;
        }
        put(id, e["summary"]["WorkspaceURIs"][0].as_str().unwrap_or(""));
    }
    for (ws, id) in read(root.join("cache/last_conversations.json")).as_object().into_iter().flatten() {
        put(id.as_str().unwrap_or(""), ws);
    }
    for l in std::fs::read_to_string(root.join("history.jsonl")).unwrap_or_default().lines() {
        let v: Value = serde_json::from_str(l).unwrap_or_default();
        put(v["conversationId"].as_str().unwrap_or(""), v["workspace"].as_str().unwrap_or(""));
    }
    for id in internal {
        m.insert(id, String::new());
    }
    m
}

/// (对话 id, 记录文件)
fn transcripts(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    for e in std::fs::read_dir(root.join("brain")).into_iter().flatten().flatten() {
        let logs = e.path().join(".system_generated/logs");
        if let Some(f) = ["transcript_full.jsonl", "transcript.jsonl"].iter().map(|n| logs.join(n)).find(|p| p.is_file()) {
            out.push((e.file_name().to_string_lossy().to_string(), f));
        }
    }
    out.sort();
    out
}

/// 界面上的模型名 → 模型 id：`Gemini 3.1 Pro (Low)` → `gemini-3.1-pro`
fn model_id(display: &str) -> String {
    let base = display.split('(').next().unwrap_or("").trim();
    base.split_whitespace().collect::<Vec<_>>().join("-").to_lowercase()
}

/// USER_INPUT 里最后一次「Model Selection」改成了什么
fn model_change(content: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"`Model Selection` from (?:.+?) to (.+?)\.(?:\s|$)").unwrap());
    re.captures_iter(content).last().map(|c| model_id(&c[1])).filter(|m| !m.is_empty() && m != "none")
}

/// 一次工具调用写下的（路径, 新增的行）；命令的路径为空、第三项是命令的工作目录
fn edits(name: &str, a: &Value) -> Vec<(String, String, String)> {
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let path = file_uri_path(&s(a, "TargetFile"));
    let out = match name {
        "write_to_file" if a["EmptyFile"] != true => vec![(path, s(a, "CodeContent"), String::new())],
        "replace_file_content" => vec![(path, added_lines(&s(a, "TargetContent"), &s(a, "ReplacementContent")), String::new())],
        "multi_replace_file_content" => {
            // 参数里的数组有时是 JSON 字符串
            let chunks = match &a["ReplacementChunks"] {
                Value::String(t) => serde_json::from_str(t).unwrap_or_default(),
                v => v.clone(),
            };
            let text: String = chunks.as_array().into_iter().flatten().map(|c| added_lines(&s(c, "TargetContent"), &s(c, "ReplacementContent"))).collect();
            vec![(path, text, String::new())]
        }
        "run_command" => vec![(String::new(), s(a, "CommandLine"), s(a, "Cwd"))],
        _ => vec![],
    };
    out.into_iter().filter(|(_, t, _)| !t.trim().is_empty()).collect()
}

#[derive(Default)]
struct Parsed {
    /// (路径, 文本, 命令工作目录, 模型, 时间)
    edits: Vec<(String, String, String, String, i64)>,
    start: i64,
    end: i64,
    prompts: i64,
    models: HashMap<String, usize>,
}

fn parse(text: &str) -> Parsed {
    let mut p = Parsed::default();
    let mut model = String::new();
    // 还在等结果的调用（先进先出），结果出来才知道成没成
    let mut open: std::collections::VecDeque<(String, Value, String, i64, bool)> = Default::default();
    let done = |p: &mut Parsed, (name, args, model, ts, cut): (String, Value, String, i64, bool), ok: bool| {
        // 截短过的参数不当证据
        if ok && !cut {
            for (path, t, cwd) in edits(&name, &args) {
                p.edits.push((path, t, cwd, model.clone(), ts));
            }
        }
    };
    for l in text.lines() {
        let v: Value = match serde_json::from_str(l) { Ok(v) => v, Err(_) => continue };
        let ts = any_ts(&v["created_at"]).unwrap_or(0);
        if ts > 0 {
            p.start = if p.start == 0 { ts } else { p.start.min(ts) };
            p.end = p.end.max(ts);
        }
        let kind = v["type"].as_str().unwrap_or("");
        let source = v["source"].as_str().unwrap_or("");
        if kind == "USER_INPUT" {
            let c = v["content"].as_str().unwrap_or("");
            if let Some(m) = model_change(c) {
                model = m;
            }
            if source != "SYSTEM" {
                p.prompts += 1;
            }
            continue;
        }
        if source == "SYSTEM" {
            continue;
        }
        if kind == "PLANNER_RESPONSE" {
            if !model.is_empty() {
                *p.models.entry(model.clone()).or_default() += 1;
            }
            let cut = v["truncated_fields"].as_array().into_iter().flatten().any(|f| f.as_str().unwrap_or("").starts_with("tool_calls"));
            for c in v["tool_calls"].as_array().into_iter().flatten() {
                open.push_back((c["name"].as_str().unwrap_or("").to_string(), c["args"].clone(), model.clone(), ts, cut));
            }
            continue;
        }
        if source == "MODEL" {
            if let Some(call) = open.pop_front() {
                let ok = matches!(v["status"].as_str(), None | Some("DONE"));
                done(&mut p, call, ok);
            }
        }
    }
    // 最后没等到结果的（会话中断）：文件里有没有这些行由提交去对，这里照常记
    while let Some(call) = open.pop_front() {
        done(&mut p, call, true);
    }
    p
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let root = root();
    let ws = workspaces(&root);
    let mut n = 0;
    for (id, f) in transcripts(&root) {
        let cwd = ws.get(&id).cloned();
        if cwd.as_deref() == Some("") {
            continue;
        }
        let gk = format!("antigravity:{id}");
        let fp = fingerprint(&f);
        if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
            n += 1;
            continue;
        }
        let p = parse(&std::fs::read_to_string(&f).unwrap_or_default());
        let base = cwd.clone().unwrap_or_default();
        let mut g = FileGroup { fp, ..Default::default() };
        let mut hit = cwd.as_deref().is_some_and(|c| pm.in_project(c));
        for (path, text, ccwd, model, ts) in &p.edits {
            if path.is_empty() {
                let dir = if ccwd.is_empty() { &base } else { ccwd };
                if dir.is_empty() {
                    continue;
                }
                for rel in crate::shell_targets(text, dir, pm) {
                    crate::push_rel(&mut g.evs, &rel, text, Src::Antigravity, model, &id, *ts);
                    hit = true;
                }
                continue;
            }
            let abs = if Path::new(path).is_absolute() || base.is_empty() { path.clone() } else { format!("{}/{path}", base.trim_end_matches('/')) };
            hit |= crate::push_text(&mut g.evs, pm, &abs, text, Src::Antigravity, model, &id, *ts);
        }
        if !hit {
            continue;
        }
        let model = p.models.iter().max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone()))).map(|(k, _)| k.clone()).unwrap_or_default();
        g.sessions.push(Session { src: Src::Antigravity, id: id.clone(), model, start: p.start, end: p.end, tokens: 0, branch: None, prompts: p.prompts, reviewer: false, has_content: true, provider: None });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("antigravity: 项目相关对话 {n} 个", "antigravity: {n} conversations for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let root = root();
    let ws = workspaces(&root);
    let mut m: HashMap<String, Activity> = HashMap::new();
    for (id, f) in transcripts(&root) {
        let Some(cwd) = ws.get(&id).filter(|c| !c.is_empty()) else { continue };
        let e = m.entry(cwd.clone()).or_insert(Activity { src: Src::Antigravity, cwd: cwd.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(fingerprint(&f).0 as i64);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let root = root();
    let ts: Vec<i64> = transcripts(&root).iter().map(|(_, f)| fingerprint(f).0 as i64).filter(|&t| t > 0).collect();
    Health {
        source: "antigravity".into(),
        path: root.display().to_string(),
        readable: root.join("brain").is_dir(),
        format: "brain/<id>/.system_generated/logs/transcript_full.jsonl".into(),
        records: ts.len() as u64,
        earliest: ts.iter().min().copied().map(date_cn),
        latest: ts.iter().max().copied().map(date_cn),
        note: creditia_core::tr!(
            "闭源，格式按第三方观察，尚未用真实日志核对；模型取自对话里的模型设置变更，没换过模型的对话不知道模型",
            "closed source; format based on third-party observation, not yet checked against real logs. The model comes from model-setting changes in the conversation, so conversations that never switched models have none"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transcript_to_edits() {
        let lines = [
            json!({ "step_index": 0, "source": "USER_EXPLICIT", "type": "USER_INPUT", "status": "DONE", "created_at": "2026-09-30T10:00:00Z",
                "content": "<USER_REQUEST>\nadd y\n</USER_REQUEST>\n<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Gemini 3.1 Pro (Low). No need to comment on this change.\n</USER_SETTINGS_CHANGE>" }),
            json!({ "step_index": 1, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": "2026-09-30T10:00:05Z", "tool_calls": [
                { "name": "write_to_file", "args": { "TargetFile": "/p/x.py", "CodeContent": "x = 1", "Overwrite": false } },
                { "name": "replace_file_content", "args": { "TargetFile": "/p/x.py", "TargetContent": "x = 1", "ReplacementContent": "x = 2" } } ] }),
            json!({ "step_index": 2, "source": "MODEL", "type": "CODE_ACTION", "status": "DONE", "created_at": "2026-09-30T10:00:06Z", "content": "Created file" }),
            json!({ "step_index": 3, "source": "MODEL", "type": "CODE_ACTION", "status": "ERROR", "created_at": "2026-09-30T10:00:07Z", "content": "target not found" }),
            json!({ "step_index": 4, "source": "SYSTEM", "type": "SYSTEM_MESSAGE", "status": "DONE", "content": "reminder" }),
            json!({ "step_index": 5, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": "2026-09-30T10:00:20+08:00", "tool_calls": [
                { "name": "multi_replace_file_content", "args": { "TargetFile": "src/y.py", "ReplacementChunks": [{ "TargetContent": "a", "ReplacementContent": "a\nb" }] } },
                { "name": "run_command", "args": { "CommandLine": "go test ./...", "Cwd": "/p" } } ] }),
            json!({ "step_index": 6, "source": "MODEL", "type": "GENERIC", "status": "DONE", "created_at": "2026-09-30T10:00:21Z", "content": "ok" }),
        ];
        let text = lines.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n");
        let p = parse(&text);
        assert_eq!(p.prompts, 1);
        let got: Vec<(&str, &str, &str, &str)> = p.edits.iter().map(|(a, b, c, d, _)| (a.as_str(), b.as_str(), c.as_str(), d.as_str())).collect();
        // 报错的 replace 不算；最后没等到结果的命令照常记
        assert_eq!(got, vec![("/p/x.py", "x = 1", "", "gemini-3.1-pro"), ("src/y.py", "b\n", "", "gemini-3.1-pro"), ("", "go test ./...", "/p", "gemini-3.1-pro")]);
        assert_eq!((p.start, p.end), (creditia_core::util::parse_iso("2026-09-30T02:00:20Z").unwrap(), creditia_core::util::parse_iso("2026-09-30T10:00:21Z").unwrap()));
        assert_eq!(model_id("Claude Sonnet 4.6 (Thinking)"), "claude-sonnet-4.6");
        assert_eq!(model_change("`Model Selection` from Gemini 3.1 Pro (High) to GPT-OSS 120B (Medium)."), Some("gpt-oss-120b".into()));
    }
}
