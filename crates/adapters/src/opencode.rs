//! OpenCode：`~/.local/share/opencode/storage/`。
//! session/<项目>/<会话>.json 有 directory；message/<会话>/<消息>.json 有 modelID、providerID、tokens；
//! part/<消息>/<片段>.json 是工具调用（edit / write / patch），只认 state.status 为 completed 的。
//! providerID 原样记为渠道（anthropic、openai、openrouter，或者用户自定义的中转站名）。

use crate::{any_ts, new_text, path_arg, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn storage() -> PathBuf {
    source_dir(Src::OpenCode).join("storage")
}
fn read(p: &Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "json").unwrap_or(false)).collect();
    v.sort();
    v
}

struct Sess {
    file: PathBuf,
    id: String,
    dir: String,
    created: i64,
    updated: i64,
}

fn sessions() -> Vec<Sess> {
    let mut out = vec![];
    for proj in std::fs::read_dir(storage().join("session")).into_iter().flatten().flatten() {
        for f in json_files(&proj.path()) {
            let v = read(&f);
            let id = v["id"].as_str().map(String::from).unwrap_or_else(|| f.file_stem().unwrap_or_default().to_string_lossy().to_string());
            out.push(Sess { file: f, id, dir: v["directory"].as_str().unwrap_or("").to_string(), created: any_ts(&v["time"]["created"]).unwrap_or(0), updated: any_ts(&v["time"]["updated"]).unwrap_or(0) });
        }
    }
    out
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for s in sessions().into_iter().filter(|s| pm.in_project(&s.dir)) {
        let msg_dir = storage().join("message").join(&s.id);
        let gk = format!("opencode:{}", s.id);
        // 指纹：会话文件 + 消息目录的修改时间
        let fp = { let a = fingerprint(&s.file); let b = fingerprint(&msg_dir); (a.0.max(b.0), a.1 + b.1) };
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            n += 1;
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let (mut tokens, mut prompts) = (0i64, 0i64);
        let mut end = s.updated;
        let mut models: HashMap<String, usize> = HashMap::new();
        let mut providers: HashMap<String, usize> = HashMap::new();
        for mf in json_files(&msg_dir) {
            let m = read(&mf);
            let ts = any_ts(&m["time"]["created"]).unwrap_or(s.created);
            end = end.max(any_ts(&m["time"]["completed"]).unwrap_or(ts));
            if m["role"] == "user" {
                prompts += 1;
                continue;
            }
            let model = m["modelID"].as_str().unwrap_or("").to_string();
            if let Some(p) = m["providerID"].as_str() {
                *providers.entry(p.to_string()).or_default() += 1;
            }
            if !model.is_empty() {
                *models.entry(model.clone()).or_default() += 1;
            }
            let t = &m["tokens"];
            tokens += ["input", "output", "reasoning"].iter().map(|k| t[*k].as_i64().unwrap_or(0)).sum::<i64>() + t["cache"]["read"].as_i64().unwrap_or(0) + t["cache"]["write"].as_i64().unwrap_or(0);
            let cwd = m["path"]["cwd"].as_str().unwrap_or(&s.dir).to_string();
            let mid = m["id"].as_str().map(String::from).unwrap_or_else(|| mf.file_stem().unwrap_or_default().to_string_lossy().to_string());
            for pf in json_files(&storage().join("part").join(&mid)) {
                let p = read(&pf);
                if p["type"] != "tool" || p["state"]["status"] != "completed" {
                    continue;
                }
                let name = p["tool"].as_str().unwrap_or("");
                let input = &p["state"]["input"];
                let (Some(path), Some(text)) = (path_arg(input), new_text(name, input)) else { continue };
                let abs = if Path::new(&path).is_absolute() { path } else { format!("{}/{}", cwd.trim_end_matches('/'), path) };
                crate::push_text(&mut g.evs, pm, &abs, &text, Src::OpenCode, &model, &s.id, any_ts(&p["state"]["time"]["end"]).unwrap_or(ts));
            }
        }
        let model = models.into_iter().max_by_key(|(_, k)| *k).map(|(m, _)| m).unwrap_or_default();
        let provider = providers.into_iter().max_by_key(|(_, k)| *k).map(|(p, _)| p);
        g.sessions.push(Session { src: Src::OpenCode, id: s.id.clone(), model, start: s.created, end, tokens, branch: None, prompts, reviewer: false, has_content: msg_dir.is_dir(), provider });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(format!("opencode: 项目相关会话 {n} 个"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for s in sessions().into_iter().filter(|s| !s.dir.is_empty()) {
        let e = m.entry(s.dir.clone()).or_insert(Activity { src: Src::OpenCode, cwd: s.dir.clone(), remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(s.updated);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let ss = sessions();
    Health {
        source: "opencode".into(),
        path: storage().display().to_string(),
        readable: storage().is_dir(),
        format: "storage/session + message + part".into(),
        records: ss.len() as u64,
        earliest: ss.iter().map(|s| s.created).filter(|&t| t > 0).min().map(date_cn),
        latest: ss.iter().map(|s| s.updated).max().filter(|&t| t > 0).map(date_cn),
        note: "逐条消息含 modelID、providerID 与 token".into(),
    }
}
