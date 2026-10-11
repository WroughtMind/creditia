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
    provider: Option<String>,
    branch: Option<String>,
    cwd: String,
    origin: String,
}

fn threads(base: &Path) -> Result<Vec<Thread>, String> {
    let db = std::fs::read_dir(base).into_iter().flatten().flatten().filter_map(|e| {
        let name=e.file_name().to_string_lossy().to_string();
        let version=name.strip_prefix("state_")?.strip_suffix(".sqlite")?.parse::<u32>().ok()?;
        Some((version,e.path()))
    }).max_by_key(|x|x.0).map(|x|x.1).unwrap_or_else(||base.join("state_5.sqlite"));
    // 没装 Codex 或从没用过：不是错误，安静跳过
    if !db.exists() {
        return rollout_threads(base);
    }
    let conn = open_ro(&db).map_err(|e| format!("codex: 线程索引不可读 {e}"))?;
    let mut cols=conn.prepare("pragma table_info(threads)").map_err(|e|e.to_string())?;
    let columns:std::collections::HashSet<String>=cols.query_map([],|r|r.get(1)).map_err(|e|e.to_string())?.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?;
    let optional=|name:&str,empty:&str|if columns.contains(name){name.to_string()}else{empty.into()};
    let query=format!("select id,rollout_path,created_at,updated_at,{},coalesce({},''),{},cwd,coalesce({},''),{} from threads",optional("tokens_used","0"),optional("model","NULL"),optional("git_branch","NULL"),optional("git_origin_url","NULL"),optional("model_provider","NULL"));
    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| format!("codex: threads 表结构不认识 {e}"))?;
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
                provider: r.get::<_,Option<String>>(9)?.filter(|p|!p.is_empty()),
            })
        })
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(rows)
}

fn rollout_threads(base:&Path)->Result<Vec<Thread>,String>{
    let mut out=vec![];
    for file in rollout_files(base).into_values(){
        let text=read_maybe_zst(&file);let mut thread=None;
        for line in text.lines(){
            let Ok(o)=serde_json::from_str::<Value>(line) else {continue};let p=&o["payload"];let ts=o["timestamp"].as_str().and_then(parse_iso).unwrap_or(0);
            if o["type"]=="session_meta" {
                thread=Some(Thread{id:p["id"].as_str().unwrap_or("").into(),rollout:file.display().to_string(),start:ts,end:ts,tokens:0,model:String::new(),provider:p["model_provider"].as_str().map(String::from),branch:p["git"]["branch"].as_str().map(String::from),cwd:p["cwd"].as_str().unwrap_or("").into(),origin:p["git"]["repository_url"].as_str().unwrap_or("").into()});
            }
            if let Some(t)=thread.as_mut(){t.end=t.end.max(ts);if o["type"]=="turn_context"{if let Some(m)=p["model"].as_str(){t.model=m.into();}}if p["type"]=="token_count"{if let Some(n)=p["info"]["total_token_usage"]["total_tokens"].as_i64(){t.tokens=n;}}}
        }
        if let Some(t)=thread{out.push(t);}
    }Ok(out)
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
        let mut g = FileGroup { fp, ..Default::default() };
        let mut prompts = 0;
        if let Some(f) = file {
            let data = read_maybe_zst(f);
            let mut cur_model = String::new();
            let mut provider = t.provider.clone();
            let mut pending:HashMap<String,(String,String,Option<String>)>=HashMap::new();
            for line in data.lines() {
                if line.contains("\"task_started\"") {
                    prompts += 1;
                }
                let Ok(o) = serde_json::from_str::<Value>(line) else { continue };
                let p = &o["payload"];
                if o["type"] == "turn_context" {
                    if let Some(p) = p["model_provider"].as_str() {provider=Some(p.into());}
                    if let Some(m) = p["model"].as_str() {
                        cur_model = m.to_string();
                    }
                    continue;
                }
                if matches!(p["type"].as_str(),Some("custom_tool_call")|Some("function_call")) && p["name"].as_str().map(|n|n.ends_with("apply_patch")).unwrap_or(false) {
                    if let Some(call)=p["call_id"].as_str(){
                        let patch=p["input"].as_str().or(p["arguments"].as_str()).unwrap_or("");
                        pending.insert(call.into(),(patch.into(),cur_model.clone(),provider.clone()));
                    }
                }
                if matches!(p["type"].as_str(),Some("custom_tool_call_output")|Some("function_call_output")) {
                    if let Some((patch,model,provider))=p["call_id"].as_str().and_then(|id|pending.remove(id)) {
                        let output=if let Some(s)=p["output"].as_str(){s.to_string()}else{p["output"].as_array().into_iter().flatten().filter_map(|v|v["text"].as_str()).collect::<Vec<_>>().join("\n")};
                        if output.contains("Success. Updated the following files:") {
                            let before=g.evs.len();crate::push_patch(&mut g.evs,pm,&t.cwd,&patch,Src::Codex,&model,&t.id,o["timestamp"].as_str().and_then(parse_iso).unwrap_or(0));
                            for ev in &mut g.evs[before..]{ev.provider=provider.clone();}
                        }
                    }
                }
                let it = &p["item"];
                if it["type"] != "FileChange" || it["status"]!="completed" {
                    continue;
                }
                let ts = o["timestamp"].as_str().and_then(parse_iso).unwrap_or(0);
                let ev_start=g.evs.len();
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
                for ev in &mut g.evs[ev_start..]{ev.provider=provider.clone();}
            }
        }
        if reviewer {
            g.evs.clear();
        }
        g.sessions.push(Session { provider: t.provider, src: Src::Codex, id: t.id.clone(), model: t.model, start: t.start, end: t.end, tokens: t.tokens, branch: t.branch, prompts, reviewer, has_content: file.is_some() });
        store.groups.insert(gk, g);
    }
    st.notes.push(format!("codex: 线程 {} 个，内容文件存在 {found}，缺失 {missing}", found + missing));
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
    let mut h = Health { source: "codex".into(), path: base.display().to_string(), format: "state_*.sqlite / rollout jsonl".into(), ..Default::default() };
    match threads(&base) {
        Ok(rows) => {
            h.readable = base.exists();
            h.records = rows.len() as u64;
            h.earliest = rows.iter().map(|t| t.start).min().map(date_cn);
            h.latest = rows.iter().map(|t| t.end).max().map(date_cn);
            let files = rollout_files(&base);
            let have = rows.iter().filter(|t| files.contains_key(&Path::new(&t.rollout).file_name().map(|s| s.to_string_lossy().replace(".zst", "")).unwrap_or_default())).count();
            h.note = format!("{} 个线程有内容文件", have);
        }
        Err(e) => h.note = e,
    }
    h
}
