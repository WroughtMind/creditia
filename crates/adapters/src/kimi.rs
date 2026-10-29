//! Kimi（kimi-code）：`~/.kimi-code/sessions/<wd>/session_<uuid>/`，state.json 有 cwd，agents/*/wire.jsonl 有 Edit/Write 和逐请求 token。
//! 旧版 `~/.kimi/sessions/<md5>/<uuid>/wire.jsonl` 同样扫一遍。

use crate::{any_ts, home, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

struct Sess {
    dir: PathBuf,
    cwd: String,
    wires: Vec<PathBuf>,
}

fn sessions() -> Vec<Sess> {
    let base = source_dir(Src::Kimi);
    let mut out = vec![];
    // 全局索引：sessionDir → workDir（有 15/28 个 state.json 缺 cwd 时用）
    let mut index: HashMap<String, String> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(base.join("session_index.jsonl")) {
        for l in t.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                if let (Some(d), Some(w)) = (v["sessionDir"].as_str(), v["workDir"].as_str()) {
                    index.insert(d.trim_end_matches('/').to_string(), w.to_string());
                }
            }
        }
    }
    for e in walkdir::WalkDir::new(base.join("sessions")).max_depth(2).into_iter().flatten() {
        let dir = e.path().to_path_buf();
        let state = dir.join("state.json");
        if !state.exists() {
            continue;
        }
        let v: Value = std::fs::read_to_string(&state).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let cwd = v["cwd"].as_str().map(String::from).or_else(|| index.get(dir.to_string_lossy().trim_end_matches('/')).cloned()).unwrap_or_default();
        let wires = walkdir::WalkDir::new(dir.join("agents")).into_iter().flatten().filter(|x| x.file_name() == "wire.jsonl").map(|x| x.path().to_path_buf()).collect();
        out.push(Sess { dir, cwd, wires });
    }
    // 旧版目录
    let old = home().join(".kimi/sessions");
    for e in walkdir::WalkDir::new(&old).max_depth(2).into_iter().flatten() {
        let dir = e.path().to_path_buf();
        let wire = dir.join("wire.jsonl");
        if !wire.exists() {
            continue;
        }
        let meta: Value = std::fs::read_to_string(dir.join("metadata.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let cwd = meta["cwd"].as_str().or(meta["workDir"].as_str()).unwrap_or("").to_string();
        out.push(Sess { dir, cwd, wires: vec![wire] });
    }
    out
}

/// 解析一组 wire.jsonl，返回（起、止、模型、token、prompt 数）
fn parse_wires(s: &Sess, pm: &PathMap, g: &mut FileGroup, id: &str) -> (i64, i64, String, i64, i64) {
    let (mut start, mut end, mut tokens, mut prompts) = (i64::MAX, 0i64, 0i64, 0i64);
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut model = String::new();
    for w in &s.wires {
        // Each agent wire has its own request stream and model state.
        model.clear();
        let text = std::fs::read_to_string(w).unwrap_or_default();
        for line in text.lines() {
            let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
            let ts = any_ts(&o["time"]).or(any_ts(&o["timestamp"])).unwrap_or(0);
            if ts > 0 {
                start = start.min(ts);
                end = end.max(ts);
            }
            let ty = o["type"].as_str().unwrap_or("");
            let p = &o["payload"];
            match ty {
                "turn.prompt" => { prompts += 1; model.clear(); }
                "llm.request" => {
                    // Missing metadata belongs to this request, not the previous one.
                    model = p["model"].as_str().filter(|m| !m.trim().is_empty())
                        .or_else(|| p["modelAlias"].as_str().filter(|m| !m.trim().is_empty())).unwrap_or("").trim().to_string();
                    if !model.is_empty() {
                        *models.entry(model.clone()).or_default() += 1;
                    }
                }
                "usage.record" => {
                    let u = &p["usage"];
                    tokens += ["inputOther", "output", "inputCacheRead", "inputCacheCreation", "input_tokens", "output_tokens"].iter().map(|k| u[*k].as_i64().unwrap_or(0)).sum::<i64>();
                    if let Some(m) = p["model"].as_str() {
                        if model.is_empty() {
                            model = m.to_string();
                        }
                    }
                }
                _ => {}
            }
            // 工具调用：context.append_loop_event 里的 tool.call
            let ev = if ty == "context.append_loop_event" { &p["event"] } else { p };
            let ev_ty = ev["type"].as_str().unwrap_or("");
            if ev_ty == "tool.call" || ty == "tool.call" {
                let call = if ev["call"].is_object() { &ev["call"] } else { ev };
                let name = call["name"].as_str().unwrap_or("");
                let a = &call["args"];
                let a = if a.is_object() { a.clone() } else { call["arguments"].clone() };
                let path = a["path"].as_str().or(a["file_path"].as_str()).unwrap_or("");
                let content = match name {
                    "Edit" => a["new_string"].as_str(),
                    "Write" => a["content"].as_str(),
                    _ => None,
                };
                if let Some(c) = content {
                    crate::push_text(&mut g.evs, pm, path, c, Src::Kimi, &model, id, if ts > 0 { ts } else { end });
                }
            }
        }
    }
    let main_model = models.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or(model);
    (if start == i64::MAX { 0 } else { start }, end, main_model, tokens, prompts)
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for s in sessions() {
        if !pm.in_project(&s.cwd) {
            continue;
        }
        n += 1;
        let gk = format!("kimi:{}", s.dir.display());
        let fp = s.wires.iter().map(|w| fingerprint(w)).fold((0u64, 0u64), |a, b| (a.0.max(b.0), a.1 + b.1));
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let id = s.dir.file_name().map(|x| x.to_string_lossy().trim_start_matches("session_").to_string()).unwrap_or_default();
        let (start, end, model, tokens, prompts) = parse_wires(&s, pm, &mut g, &id);
        if end > 0 {
            g.sessions.push(Session { src: Src::Kimi, id, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: !s.wires.is_empty(), provider: None });
        }
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("kimi: 项目相关会话 {n} 个", "kimi: {n} sessions for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for s in sessions() {
        if s.cwd.is_empty() {
            continue;
        }
        let last = s.wires.iter().filter_map(|w| std::fs::metadata(w).ok()).filter_map(|x| x.modified().ok()).filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).max().unwrap_or(0);
        let e = m.entry(s.cwd.clone()).or_insert(Activity { src: Src::Kimi, cwd: s.cwd, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(last);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Kimi);
    let ss = sessions();
    let no_cwd = ss.iter().filter(|s| s.cwd.is_empty()).count();
    let mt: Vec<i64> = ss.iter().flat_map(|s| s.wires.iter()).filter_map(|w| std::fs::metadata(w).ok()).filter_map(|x| x.modified().ok()).filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).collect();
    Health {
        source: "kimi".into(),
        path: base.display().to_string(),
        readable: base.join("sessions").is_dir(),
        format: "sessions/*/session_*/state.json + agents/*/wire.jsonl".into(),
        records: ss.len() as u64,
        earliest: mt.iter().min().copied().map(date_local),
        latest: mt.iter().max().copied().map(date_local),
        note: creditia_core::tr!("{no_cwd} 个会话无 cwd（已用 session_index 回退）；仅识别顶层 llm.request / usage.record / tool.call 事件。官方 Python Wire 未记录逐请求模型，缺失保持未知，不以启动配置回填", "{no_cwd} sessions without cwd (recovered from session_index); only top-level llm.request / usage.record / tool.call events are read. Official Python Wire has no per-request model field; missing models stay unknown, without startup-config fallback"),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_model_boundaries_clear_missing_requests_and_other_agents() {
        let dir = std::env::temp_dir().join(format!("creditia-kimi-models-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let event = |ty: &str, payload: Value| json!({"type": ty, "time": 1_790_000_000, "payload": payload});
        let edit = |file: &str| event("tool.call", json!({"name": "Edit", "args": {"path": format!("/demo/src/{file}.rs"), "new_string": format!("pub fn {file}_model_boundary() -> usize {{ 734 }}")}}));
        let a = dir.join("a.jsonl");
        let b = dir.join("b.jsonl");
        let first = [
            event("llm.request", json!({"model": "known-model"})), edit("known"),
            event("llm.request", json!({})), edit("missing"),
            event("llm.request", json!({"model": "known-model"})),
        ];
        std::fs::write(&a, first.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
        std::fs::write(&b, edit("other_wire").to_string()).unwrap();
        let s = Sess { dir: dir.clone(), cwd: "/demo".into(), wires: vec![a, b] };
        let pm = PathMap { known: ["src/known.rs".into(), "src/missing.rs".into(), "src/other_wire.rs".into()].into(), markers: vec!["/demo/".into()], excludes: vec![] };
        let mut g = FileGroup::default();
        parse_wires(&s, &pm, &mut g, "models");
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(g.evs.iter().map(|e| e.model.as_str()).collect::<Vec<_>>(), vec!["known-model", "", ""]);
        assert!(g.reconciled_events.is_empty());
    }
}
