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
        parse(&text, pm, &mut g);
        sessions += g.sessions.len();
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("claude-code: 项目相关会话 {sessions} 个（按 cwd 识别）"));
}

fn parse(text: &str, pm: &PathMap, g: &mut FileGroup) {
    let mut in_project = false;
    let mut current_cwd = String::new();
    let mut completed: HashMap<String,i64> = HashMap::new();
    for line in text.lines() {
        let Ok(o)=serde_json::from_str::<Value>(line) else {continue};
        for block in o["message"]["content"].as_array().into_iter().flatten() {
            if block["type"]=="tool_result" && block["is_error"]!=true {
                if let Some(id)=block["tool_use_id"].as_str(){ completed.insert(id.into(),o["timestamp"].as_str().and_then(parse_iso).unwrap_or(0)); }
            }
        }
    }
    let (mut sid, mut start, mut end, mut prompts, mut tokens) = (String::new(), i64::MAX, 0i64, 0i64, 0i64);
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut seen_msg: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(cwd) = o["cwd"].as_str() {
            current_cwd = cwd.into();
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
            let Some(&completed_at) = b["id"].as_str().and_then(|id| completed.get(id)) else {continue};
            let ts = completed_at;
            let inp = &b["input"];
            let input_path = inp["file_path"].as_str().unwrap_or("");
            let path = if Path::new(input_path).is_absolute() || input_path.as_bytes().get(1)==Some(&b':') { input_path.into() } else { format!("{current_cwd}/{input_path}") };
            match b["name"].as_str() {
                Some("Write") => {
                    crate::push_text(&mut g.evs, pm, &path, inp["content"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("Edit") => {
                    crate::push_text(&mut g.evs, pm, &path, inp["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                }
                Some("MultiEdit") => {
                    for ed in inp["edits"].as_array().into_iter().flatten() {
                        crate::push_text(&mut g.evs, pm, &path, ed["new_string"].as_str().unwrap_or(""), Src::Claude, model, &sid, ts);
                    }
                }
                _ => {}
            }
        }
    }
    if !in_project {
        g.evs.clear();
        return;
    }
    let model = if models.len()==1 { models.into_keys().next().unwrap() } else {String::new()};
    if start <= end {
        g.sessions.push(Session { provider: None, src: Src::Claude, id: sid, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true });
    }
}

/// 读文件开头几条记录拿 cwd 和时间
fn head_info(p: &Path) -> Option<(String, i64)> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(p).ok()?;
    let mut cwd = None;
    let mut last = 0;
    for l in BufReader::new(f).lines().map_while(Result::ok) {
        if cwd.is_some() { break; }
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
    for p in files.iter() {
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
