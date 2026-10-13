//! Claude Code：`projects/*/*.jsonl`。目录名是编码过的 cwd（中文会变成 ---），所以不靠目录名，读每条记录里的 cwd。

use crate::{source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, parse_iso};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn session_files() -> Vec<PathBuf> {
    let base = source_dir(Src::Claude).join("projects");
    walkdir::WalkDir::new(&base)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut sessions = 0;
    for p in session_files() {
        let gk = format!("claude:{}", p.display());
        let fp = fingerprint(&p);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            sessions += store.groups[&gk].sessions.len();
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        // 粗筛：关键词去掉末尾的 `/` 再找（cwd 字段后面跟的是引号不是斜杠）；精确判断在逐条记录的 cwd 上做
        let lower = text.to_lowercase();
        if pm.markers.iter().any(|m| lower.contains(m.trim_end_matches('/'))) {
            parse(&text, pm, &mut g);
        }
        sessions += g.sessions.len();
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("claude-code: 项目相关会话 {sessions} 个（按 cwd 识别）"));
}

fn parse(text: &str, pm: &PathMap, g: &mut FileGroup) {
    let mut in_project = false;
    let (mut sid, mut start, mut end, mut prompts, mut tokens) = (String::new(), i64::MAX, 0i64, 0i64, 0i64);
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut seen_msg: HashSet<String> = HashSet::new();
    let mut providers: HashMap<String, usize> = HashMap::new();
    // 每次编辑调用先挂起，等到工具结果：报错的（比如 old_string 没找到）丢掉，其余记为证据
    let mut pending: Vec<(String, Vec<creditia_core::model::Ev>)> = vec![];
    let mut failed: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(cwd) = o["cwd"].as_str() {
            if pm.in_project(cwd) {
                in_project = true;
            }
        }
        let Some(ts) = o["timestamp"].as_str().and_then(parse_iso) else { continue };
        if sid.is_empty() {
            sid = o["sessionId"].as_str().unwrap_or("").to_string();
        }
        start = start.min(ts);
        end = end.max(ts);
        let m = &o["message"];
        if o["type"] == "user" {
            for b in m["content"].as_array().into_iter().flatten() {
                if b["type"] == "tool_result" && b["is_error"] == true {
                    if let Some(id) = b["tool_use_id"].as_str() {
                        failed.insert(id.to_string());
                    }
                }
            }
        }
        if o["type"] == "user" && o["isMeta"] != true {
            let human = match &m["content"] {
                Value::String(_) => true,
                Value::Array(a) => a.iter().any(|b| b["type"] == "text") && !a.iter().any(|b| b["type"] == "tool_result"),
                _ => false,
            };
            if human {
                prompts += 1;
            }
        }
        if o["type"] != "assistant" {
            continue;
        }
        let model = m["model"].as_str().unwrap_or("");
        if model.is_empty() || model == "<synthetic>" {
            continue;
        }
        if let Some(id) = m["id"].as_str() {
            if let Some(p) = claude_provider(id, model) {
                *providers.entry(p).or_default() += 1;
            }
            if seen_msg.insert(id.to_string()) {
                let u = &m["usage"];
                tokens += ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"].iter().map(|k| u[*k].as_i64().unwrap_or(0)).sum::<i64>();
            }
        }
        *models.entry(model.to_string()).or_default() += 1;
        for b in m["content"].as_array().into_iter().flatten() {
            if b["type"] != "tool_use" {
                continue;
            }
            let inp = &b["input"];
            let path = inp["file_path"].as_str().unwrap_or("");
            let mut evs = vec![];
            match b["name"].as_str() {
                Some("Write") => {
                    crate::push_text(&mut evs, pm, path, inp["content"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("Edit") => {
                    crate::push_text(&mut evs, pm, path, inp["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("MultiEdit") => {
                    for ed in inp["edits"].as_array().into_iter().flatten() {
                        crate::push_text(&mut evs, pm, path, ed["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                    }
                }
                _ => {}
            }
            if !evs.is_empty() {
                pending.push((b["id"].as_str().unwrap_or("").to_string(), evs));
            }
        }
    }
    for (id, evs) in pending {
        if !failed.contains(&id) {
            g.evs.extend(evs);
        }
    }
    if !in_project {
        g.evs.clear();
        return;
    }
    let model = models.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or_default();
    if start <= end {
        let provider = providers.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p);
        g.sessions.push(Session { src: Src::Claude, id: sid, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true, provider });
    }
}

/// Claude Code 的渠道：Bedrock、Vertex 返回的消息 id 有固定前缀；不是 `msg_` 开头说明经过了改写响应的中转站。
/// 其余（含透传官方响应的中转站）看不出来，就不写，不猜
fn claude_provider(id: &str, model: &str) -> Option<String> {
    if let Some(p) = creditia_core::model::provider_hint(model) {
        return Some(p);
    }
    if id.starts_with("msg_bdrk_") {
        Some("aws-bedrock".into())
    } else if id.starts_with("msg_vrtx_") {
        Some("google-vertex".into())
    } else if !id.starts_with("msg_") {
        Some("relay".into())
    } else {
        None
    }
}

/// 读文件开头几条记录拿 cwd 和时间
fn head_info(p: &Path) -> Option<(String, i64)> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(p).ok()?;
    let mut cwd = None;
    let mut last = 0;
    // 某一行不是合法 UTF-8 就跳过它接着读，不因为一行坏掉丢掉整个会话
    #[allow(clippy::lines_filter_map_ok)]
    for (i, l) in BufReader::new(f).lines().flatten().enumerate() {
        if i > 40 && cwd.is_some() {
            break;
        }
        let Ok(o) = serde_json::from_str::<Value>(&l) else { continue };
        if cwd.is_none() {
            cwd = o["cwd"].as_str().map(String::from);
        }
        if let Some(t) = o["timestamp"].as_str().and_then(parse_iso) {
            last = last.max(t);
        }
    }
    let mt = std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(last);
    cwd.map(|c| (c, mt.max(last)))
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for p in session_files() {
        if p.to_string_lossy().contains("claude-mem-observer") {
            continue;
        }
        let Some((cwd, last)) = head_info(&p) else { continue };
        let e = m.entry(cwd.clone()).or_insert(Activity { src: Src::Claude, cwd, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(last);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Claude).join("projects");
    let files = session_files();
    let mut h = Health { source: "claude-code".into(), path: base.display().to_string(), format: "projects/*/*.jsonl".into(), readable: base.is_dir(), records: files.len() as u64, ..Default::default() };
    let (mut lo, mut hi) = (i64::MAX, 0);
    for p in files.iter().take(400) {
        if let Some((_, t)) = head_info(p) {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if hi > 0 {
        h.earliest = Some(date_cn(lo));
        h.latest = Some(date_cn(hi));
    }
    h.note = "逐条记录含 cwd、model、Edit/Write 内容".into();
    h
}
