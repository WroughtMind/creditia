//! Claude Code：`projects/*/*.jsonl`。目录名是编码过的 cwd（中文会变成 ---），所以不靠目录名，读每条记录里的 cwd。

use crate::{source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_local, parse_iso};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
mod histories;

fn session_files() -> Vec<PathBuf> {
    histories::files().into_iter().map(|(_, p)| p).collect()
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut sessions = 0;
    let files = histories::files();
    let keys: HashSet<_> = files.iter().map(|(key, _)| key.as_str()).collect();
    store.groups.retain(|key, _| !key.starts_with("claude:") || keys.contains(key.as_str()));
    for (gk, p) in files {
        let fp = fingerprint(&p);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            sessions += store.groups[&gk].sessions.len();
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        let text = match std::fs::read_to_string(&p) {
            Ok(text) => text,
            Err(e) => {
                store.groups.remove(&gk);
                st.warnings.push(format!("claude-code {}: {e}", p.display()));
                continue;
            }
        };
        // Host work/project may be a symlink whose spelling contains no repo
        // marker. Resolve each recorded cwd before testing project ownership.
        let session = gk.strip_prefix("claude:native-v3:").expect("native Claude identity");
        parse(&text, pm, session, &mut g);
        sessions += g.sessions.len();
        store.groups.insert(gk, g);
    }
    st.notes.push(creditia_core::tr!("claude-code: 项目相关会话 {sessions} 个（按 cwd 识别）", "claude-code: {sessions} sessions for this project (by cwd)"));
}

/// Older libraries used the parent ID for both main and subagent transcripts.
/// Only completely read transcripts may claim matching old evidence.
pub fn legacy_sessions(store: &Store) -> Vec<(String, String)> {
    let mut aliases: Vec<_> = store.groups.iter()
        .filter(|(key, _)| key.starts_with("claude:native-v3:"))
        .flat_map(|(_, g)| &g.reconciled_sessions)
        .filter(|(src, _)| *src == Src::Claude)
        .map(|(_, id)| (id.rsplit_once(":agent-").map_or(id.as_str(), |(parent, _)| parent).to_string(), id.clone()))
        .collect();
    aliases.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| (a.0 != a.1).cmp(&(b.0 != b.1))).then_with(|| a.1.cmp(&b.1)));
    aliases.dedup();
    aliases
}

fn parse(text: &str, pm: &PathMap, session: &str, g: &mut FileGroup) {
    let mut in_project = false;
    let (sid, mut start, mut end, mut prompts, mut tokens) = (session.to_string(), i64::MAX, 0i64, 0i64, 0i64);
    let mut complete = true;
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut seen_msg: HashSet<String> = HashSet::new();
    let mut providers: HashMap<String, usize> = HashMap::new();
    // 每次编辑调用先挂起，等到工具结果：报错的（比如 old_string 没找到）丢掉，其余记为证据。
    // 工具结果里有 structuredPatch 时只取其中新增的行（覆盖写入整个文件时，没变的行不算这次写的）
    // 最后一项：shell 命令的工作目录（Bash 调用；命令原文作为它提到的文件的候选证据）
    let mut pending: Vec<(String, String, String, String, i64, Option<String>)> = vec![];
    let mut failed: HashSet<String> = HashSet::new();
    let mut completed: HashSet<String> = HashSet::new();
    let mut patched: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() { continue; }
        let Ok(o) = serde_json::from_str::<Value>(line) else { complete = false; continue };
        if let Some(cwd) = o["cwd"].as_str() {
            if pm.in_project(&crate::harness_paths::cwd(cwd)) {
                in_project = true;
            }
        }
        let Some(ts) = o["timestamp"].as_str().and_then(parse_iso) else { continue };
        start = start.min(ts);
        end = end.max(ts);
        let m = &o["message"];
        if o["type"] == "user" {
            for b in m["content"].as_array().into_iter().flatten() {
                if b["type"] != "tool_result" {
                    continue;
                }
                let Some(id) = b["tool_use_id"].as_str() else { continue };
                if b["is_error"] == true {
                    failed.insert(id.to_string());
                    continue;
                }
                completed.insert(id.to_string());
                let r = &o["toolUseResult"];
                if r["type"] == "create" {
                    if let Some(c) = r["content"].as_str() {
                        patched.insert(id.to_string(), c.to_string());
                    }
                } else if r["structuredPatch"].is_array() {
                    patched.insert(id.to_string(), crate::added_from_patch(&r["structuredPatch"]));
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
            if b["name"] == "Bash" {
                if let Some(cmd) = inp["command"].as_str() {
                    let cwd = crate::harness_paths::cwd(o["cwd"].as_str().unwrap_or(""));
                    pending.push((b["id"].as_str().unwrap_or("").to_string(), String::new(), cmd.to_string(), model.to_string(), ts, Some(cwd)));
                }
                continue;
            }
            let path = inp["file_path"].as_str().unwrap_or("");
            // Tool input is used only after a matching successful result.
            let text = match b["name"].as_str() {
                Some("Write") => inp["content"].as_str().unwrap_or("").to_string(),
                Some("Edit") => crate::added_lines(inp["old_string"].as_str().unwrap_or(""), inp["new_string"].as_str().unwrap_or("")),
                Some("MultiEdit") => inp["edits"].as_array().into_iter().flatten().map(|ed| crate::added_lines(ed["old_string"].as_str().unwrap_or(""), ed["new_string"].as_str().unwrap_or(""))).collect(),
                _ => continue,
            };
            let path = crate::harness_paths::target(path, o["cwd"].as_str().unwrap_or(""));
            pending.push((b["id"].as_str().unwrap_or("").to_string(), path, text, model.to_string(), ts, None));
        }
    }
    if complete && !sid.is_empty() {
        g.reconciled_sessions.push((Src::Claude, sid.clone()));
    }
    for (id, path, text, model, ts, shell) in pending {
        if id.is_empty() || failed.contains(&id) || !completed.contains(&id) {
            continue;
        }
        if let Some(cwd) = shell {
            for rel in crate::shell_targets(&text, &cwd, pm) {
                crate::push_rel(&mut g.evs, &rel, &text, Src::Claude, &model, &sid, ts);
            }
            continue;
        }
        let text = patched.get(&id).unwrap_or(&text);
        crate::push_text(&mut g.evs, pm, &path, text, Src::Claude, &model, &sid, ts);
    }
    if !in_project {
        g.evs.clear();
        return;
    }
    // An interrupted/malformed transcript cannot replace its persisted snapshot.
    if !complete { g.evs.clear(); }
    let model = models.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or_default();
    if start <= end {
        let provider = providers.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p);
        g.sessions.push(Session { src: Src::Claude, id: sid, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: complete, provider });
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
            cwd = o["cwd"].as_str().map(crate::harness_paths::cwd);
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
    let mut h = Health { source: "claude-code".into(), path: base.display().to_string(), format: "projects/*/*.jsonl; MCPJam local home/.claude/projects".into(), readable: histories::roots().iter().any(|p| p.is_dir()), records: files.len() as u64, ..Default::default() };
    let (mut lo, mut hi) = (i64::MAX, 0);
    for p in files.iter().take(400) {
        if let Some((_, t)) = head_info(p) {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if hi > 0 {
        h.earliest = Some(date_local(lo));
        h.latest = Some(date_local(hi));
    }
    h.note = creditia_core::tr!("逐条记录含 cwd、model、Edit/Write 内容", "each record has cwd, model and Edit/Write content");
    h
}
