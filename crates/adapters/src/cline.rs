//! Cline、Roo Code、Kilo Code（VS Code 扩展，格式同源）：`<编辑器>/User/globalStorage/<扩展>/tasks/<任务>/`。
//! 编辑器可能是 Code、Cursor、Windsurf、VSCodium……全都找。
//! - ui_messages.json 里 tool 消息（JSON 文本）有 newFileCreated / editedExistingFile / appliedDiff，路径相对于工作目录；
//! - 工作目录取自对话首条消息里的环境说明「Current Working Directory (…)」；
//! - 模型和渠道取自 task_metadata.json 的 model_usage（新版本才有）。
//!
//! 被拒绝、失败的编辑也会出现在 ui_messages 里：每次编辑都要在对话记录里找到对应的工具结果（`[write_to_file for 'a.ts'] Result:`），
//! 结果是成功写入才算证据；找不到结果、被拒绝、报错的一律不算（人后来自己写了同样的行，不会被记成 agent 的逐行证据）。

use crate::{added_from_diff, any_ts, vscode_user_dirs, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const EXT: [(Src, &str); 3] = [(Src::Cline, "saoudrizwan.claude-dev"), (Src::Roo, "rooveterinaryinc.roo-cline"), (Src::Kilo, "kilocode.kilo-code")];

/// 某个扩展在所有编辑器里的 tasks 目录
fn roots(src: Src) -> Vec<PathBuf> {
    let ext = EXT.iter().find(|(s, _)| *s == src).map(|(_, e)| *e).unwrap_or("");
    let mut out = vec![];
    if let Ok(p) = std::env::var("CREDITIA_VSCODE_STORAGE") {
        out.push(PathBuf::from(p).join(ext).join("tasks"));
        return out;
    }
    for u in vscode_user_dirs() {
        let d = u.join("globalStorage").join(ext).join("tasks");
        if d.is_dir() {
            out.push(d);
        }
    }
    out
}

fn tasks(src: Src) -> Vec<PathBuf> {
    roots(src).into_iter().flat_map(|r| std::fs::read_dir(r).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect::<Vec<_>>()).collect()
}

/// 对话首条消息里的工作目录
fn cwd_of(task: &Path) -> Option<String> {
    let t = std::fs::read_to_string(task.join("api_conversation_history.json")).ok()?;
    let i = t.find("Current Working Directory (")? + "Current Working Directory (".len();
    let rest = &t[i..];
    let j = rest.find(')')?;
    Some(rest[..j].replace("\\\\", "/").to_string())
}

/// Cline / Roo / Kilo 写入成功时工具结果的原话（不同版本措辞略有不同）
const SUCCESS: [&str; 5] = ["successfully saved to", "has been successfully saved", "Changes successfully applied", "The user made the following updates", "successfully applied"];
const DENIED: [&str; 2] = ["The user denied this operation", "user denied"];

/// 对话记录里每个文件的编辑结果，按出现顺序：true = 写入成功
fn outcomes(task: &Path) -> HashMap<String, std::collections::VecDeque<bool>> {
    let mut out: HashMap<String, std::collections::VecDeque<bool>> = HashMap::new();
    let Ok(t) = std::fs::read_to_string(task.join("api_conversation_history.json")) else { return out };
    // JSON 字符串里的换行、引号是转义过的，直接在原文上找标记
    let re = regex::Regex::new(r"\[(write_to_file|replace_in_file|apply_diff|insert_content|search_and_replace|edit_file|new_rule) for '([^']+)'\] Result:").unwrap();
    let marks: Vec<(usize, usize, String)> = re.captures_iter(&t).map(|c| { let m = c.get(0).unwrap(); (m.start(), m.end(), c[2].trim_start_matches("./").to_string()) }).collect();
    for (i, (_, end, path)) in marks.iter().enumerate() {
        // 只看结果开头那一句：成功的结果后面会附上文件全文，全文里出现 error 之类的词不能当成失败
        let stop = marks.get(i + 1).map(|m| m.0).unwrap_or(t.len()).min(end + 240);
        let head = &t[*end..stop];
        let ok = SUCCESS.iter().any(|p| head.contains(p)) && !DENIED.iter().any(|p| head.contains(p));
        out.entry(path.clone()).or_default().push_back(ok);
    }
    out
}

fn model_of(task: &Path) -> (String, Option<String>) {
    let v: Value = std::fs::read_to_string(task.join("task_metadata.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let last = v["model_usage"].as_array().and_then(|a| a.last()).cloned().unwrap_or_default();
    (last["model_id"].as_str().unwrap_or("").to_string(), last["model_provider_id"].as_str().map(String::from))
}

pub fn scan_src(src: Src, store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    let mut unconfirmed = 0;
    for task in tasks(src) {
        let Some(cwd) = cwd_of(&task) else { continue };
        if !pm.in_project(&cwd) {
            continue;
        }
        let ui = task.join("ui_messages.json");
        let gk = format!("{}:{}", src.harness(), task.display());
        let fp = fingerprint(&ui);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            n += 1;
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let id = task.file_name().unwrap_or_default().to_string_lossy().to_string();
        let (model, provider) = model_of(&task);
        let msgs: Vec<Value> = std::fs::read_to_string(&ui).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let (mut start, mut end, mut prompts, mut tokens) = (i64::MAX, 0i64, 0i64, 0i64);
        let mut results = outcomes(&task);
        for m in &msgs {
            let ts = any_ts(&m["ts"]).unwrap_or(0);
            if ts > 0 {
                start = start.min(ts);
                end = end.max(ts);
            }
            let kind = m["say"].as_str().or(m["ask"].as_str()).unwrap_or("");
            if kind == "task" || kind == "user_feedback" {
                prompts += 1;
            }
            if kind == "api_req_started" {
                if let Some(t) = m["text"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()) {
                    tokens += ["tokensIn", "tokensOut", "cacheReads", "cacheWrites"].iter().map(|k| t[*k].as_i64().unwrap_or(0)).sum::<i64>();
                }
            }
            if kind != "tool" {
                continue;
            }
            let Some(t) = m["text"].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()) else { continue };
            let rel = t["path"].as_str().unwrap_or("");
            let text = match t["tool"].as_str() {
                Some("newFileCreated") => t["content"].as_str().map(String::from),
                Some("editedExistingFile") | Some("appliedDiff") => t["diff"].as_str().map(added_from_diff).or(t["content"].as_str().map(String::from)),
                _ => None,
            };
            let Some(text) = text else { continue };
            // 这次编辑在对话记录里的结果：没有结果或没成功的不算
            let ok = results.get_mut(rel.trim_start_matches("./")).and_then(|q| q.pop_front()).unwrap_or(false);
            if !ok {
                unconfirmed += 1;
                continue;
            }
            let abs = if Path::new(rel).is_absolute() { rel.to_string() } else { format!("{}/{}", cwd.trim_end_matches('/'), rel) };
            crate::push_text(&mut g.evs, pm, &abs, &text, src, &model, &id, ts);
        }
        if start != i64::MAX {
            g.sessions.push(Session { src, id, model, start, end, tokens, branch: None, prompts: prompts.max(1), reviewer: false, has_content: true, provider });
        }
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("{}: 项目相关任务 {n} 个；没有确认写入成功（被拒绝、报错或找不到结果）的编辑 {unconfirmed} 次，不计入", "{}: {n} tasks for this project; {unconfirmed} edits without a confirmed write (rejected, failed or no result) are not counted", src.harness()));
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for (src, _) in EXT {
        scan_src(src, store, pm, st);
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<(String, Src), Activity> = HashMap::new();
    for (src, _) in EXT {
        for task in tasks(src) {
            let Some(cwd) = cwd_of(&task) else { continue };
            let last = std::fs::metadata(task.join("ui_messages.json")).ok().and_then(|x| x.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let e = m.entry((cwd.clone(), src)).or_insert(Activity { src, cwd, remote: None, sessions: 0, last: 0 });
            e.sessions += 1;
            e.last = e.last.max(last);
        }
    }
    m.into_values().collect()
}

pub fn doctor(src: Src) -> Health {
    let ts = tasks(src);
    let mt: Vec<i64> = ts.iter().filter_map(|t| std::fs::metadata(t.join("ui_messages.json")).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).collect();
    let rs = roots(src);
    Health {
        source: src.harness().into(),
        path: rs.first().map(|p| p.display().to_string()).unwrap_or_else(|| creditia_core::tr!("（未安装）", "(not installed)")),
        readable: !rs.is_empty(),
        format: creditia_core::tr!("globalStorage/<扩展>/tasks/<任务>/ui_messages.json", "globalStorage/<extension>/tasks/<task>/ui_messages.json"),
        records: ts.len() as u64,
        earliest: mt.iter().min().copied().map(date_cn),
        latest: mt.iter().max().copied().map(date_cn),
        note: creditia_core::tr!("{} 个编辑器里装了这个扩展", "installed in {} editors", rs.len()),
        ..Default::default()
    }
}
