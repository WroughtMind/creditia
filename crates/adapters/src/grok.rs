//! Grok（grok build）：`~/.grok/sessions/<URL 编码的 cwd>/<uuid>/`，summary.json 有 cwd 和模型，chat_history.jsonl 有 search_replace / write 的内容。

use crate::{any_ts, percent_decode, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

struct Sess {
    dir: PathBuf,
    cwd: String,
    start: i64,
    end: i64,
    branch: Option<String>,
}

fn sessions() -> Vec<Sess> {
    let base = source_dir(Src::Grok).join("sessions");
    let mut out = vec![];
    let Ok(dirs) = std::fs::read_dir(&base) else { return out };
    for d in dirs.flatten() {
        let cwd_dir = d.path();
        if !cwd_dir.is_dir() {
            continue;
        }
        let decoded = percent_decode(&d.file_name().to_string_lossy());
        for s in std::fs::read_dir(&cwd_dir).into_iter().flatten().flatten() {
            let dir = s.path();
            let summary = dir.join("summary.json");
            if !summary.exists() {
                continue;
            }
            let v: Value = std::fs::read_to_string(&summary).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let cwd = v["info"]["cwd"].as_str().or(v["cwd"].as_str()).map(String::from).unwrap_or_else(|| decoded.clone());
            let start = any_ts(&v["created_at"]).unwrap_or(0);
            let end = any_ts(&v["updated_at"]).or(any_ts(&v["last_active_at"])).unwrap_or(start);
            let branch = v["head_branch"].as_str().map(String::from);
            out.push(Sess { dir, cwd, start, end, branch });
        }
    }
    out
}

/// tool_calls 的参数可能是对象，也可能是 JSON 字符串
fn args_of(tc: &Value) -> Value {
    for k in ["args", "arguments", "input", "parameters"] {
        match &tc[k] {
            Value::Object(_) => return tc[k].clone(),
            Value::String(s) => {
                if let Ok(v) = serde_json::from_str::<Value>(s) {
                    return v;
                }
            }
            _ => {}
        }
    }
    if let Value::Object(_) = &tc["function"]["arguments"] {
        return tc["function"]["arguments"].clone();
    }
    if let Some(s) = tc["function"]["arguments"].as_str() {
        return serde_json::from_str(s).unwrap_or_default();
    }
    Value::Null
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for s in sessions() {
        if !pm.in_project(&s.cwd) {
            continue;
        }
        n += 1;
        let hist = s.dir.join("chat_history.jsonl");
        let gk = format!("grok:{}", s.dir.display());
        let fp = fingerprint(&hist);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let id = s.dir.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let mut prompts = 0;
        let mut models = HashSet::new();
        let mut pending: HashMap<String, (String, String, String)> = HashMap::new();
        let text = std::fs::read_to_string(&hist).unwrap_or_default();
        for line in text.lines() {
            let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
            let role = o["type"].as_str().unwrap_or("");
            if role == "user" { prompts += 1; }
            if role == "assistant" {
                let model = o["model_id"].as_str().unwrap_or("").to_string();
                if !model.is_empty() { models.insert(model.clone()); }
                for tc in o["tool_calls"].as_array().into_iter().flatten() {
                    let name = tc["name"].as_str().unwrap_or("");
                    let a = args_of(tc);
                    let path = a["file_path"].as_str().or(a["path"].as_str()).unwrap_or("");
                    let content = match name { "search_replace" => a["new_string"].as_str(), "write" => a["content"].as_str(), _ => None };
                    if let (Some(content), Some(call_id)) = (content, tc["id"].as_str()) {
                        let path = if Path::new(path).is_absolute() { PathBuf::from(path) } else { PathBuf::from(&s.cwd).join(path) };
                        pending.insert(call_id.into(), (path.to_string_lossy().into(), content.into(), model.clone()));
                    }
                }
            } else if role == "tool_result" {
                let Some((path, content, model)) = o["tool_call_id"].as_str().and_then(|key| pending.remove(key)) else { continue };
                let result = o["content"].as_str().unwrap_or("");
                let success = result.starts_with("Wrote file successfully to ") || (result.starts_with("The file ") && result.contains(" has been updated successfully."));
                if !success { continue; }
                // 该格式通常不记录逐条时间：会话结束时间仅作最晚发生时间，不猜测事件时间。
                let ts = any_ts(&o["ts"]).or(any_ts(&o["timestamp"])).unwrap_or(s.end);
                crate::push_text(&mut g.evs, pm, &path, &content, Src::Grok, &model, &id, ts);
            }
        }
        let model = if models.len() == 1 { models.into_iter().next().unwrap() } else { String::new() };
        g.sessions.push(Session { provider: None, src: Src::Grok, id, model, start: s.start, end: s.end, tokens: 0, branch: s.branch, prompts, reviewer: false, has_content: hist.exists() });
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(format!("grok: 项目相关会话 {n} 个"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for s in sessions() {
        let e = m.entry(s.cwd.clone()).or_insert(Activity { src: Src::Grok, cwd: s.cwd, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(s.end);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Grok).join("sessions");
    let ss = sessions();
    Health {
        source: "grok".into(),
        path: base.display().to_string(),
        readable: base.is_dir(),
        format: "sessions/<cwd>/<uuid>/summary.json + chat_history.jsonl".into(),
        records: ss.len() as u64,
        earliest: ss.iter().map(|s| s.start).filter(|&t| t > 0).min().map(date_cn),
        latest: ss.iter().map(|s| s.end).max().filter(|&t| t > 0).map(date_cn),
        note: "无逐请求 token；编辑按调用与成功结果配对，缺事件时间时使用会话结束上界".into(),
    }
}

#[allow(dead_code)]
fn _p(_: &Path) {}
