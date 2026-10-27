//! Amp：`~/.local/share/amp/threads/T-<id>.json`，一个线程一个文件。
//! `env.initial.trees[].uri` 是工作目录，`messages[]` 里 assistant 的 `tool_use`（edit_file、create_file、apply_patch、Bash）
//! 配上下一条消息里 `tool_result` 的 `run.status`：只有 done 的才算写进去了（error、rejected-by-user、cancelled 都不算）。
//! 模型取 assistant 消息的 `usage.model`，没有就取 `env.initial.tags` 里的 `model:…`。
//! 2026 年 3 月以后的 Amp 把线程存在服务器上、本地不再有这些文件：那之后的线程读不到，doctor 会说明。

use crate::{added_lines, any_ts, apply_patch_files, file_uri_path, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn threads() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(source_dir(Src::Amp).join("threads")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "json").unwrap_or(false)).collect();
    v.sort();
    v
}

fn cwd_of(v: &Value) -> String {
    v["env"]["initial"]["trees"].as_array().and_then(|t| t.first()).and_then(|t| t["uri"].as_str()).map(file_uri_path).unwrap_or_default()
}

/// 一次工具调用写下的（路径, 新增的行）
fn edits(name: &str, input: &Value, cwd: &str) -> Vec<(String, String)> {
    let path = input["path"].as_str().map(file_uri_path).unwrap_or_default();
    let s = |k: &str| input[k].as_str().unwrap_or("").to_string();
    let abs = |p: &str| if p.starts_with('/') || p.get(1..2) == Some(":") || cwd.is_empty() { p.to_string() } else { format!("{}/{}", cwd.trim_end_matches('/'), p) };
    match name {
        "create_file" => vec![(abs(&path), s("content"))],
        "edit_file" => vec![(abs(&path), added_lines(&s("old_str"), &s("new_str")))],
        "apply_patch" => apply_patch_files(&s("patchText")).into_iter().map(|(p, t)| (abs(&p), t)).collect(),
        _ => vec![],
    }
}

fn parse(v: &Value, pm: &PathMap, g: &mut FileGroup) {
    let id = v["id"].as_str().unwrap_or("").to_string();
    let cwd = cwd_of(v);
    let created = any_ts(&v["created"]).unwrap_or(0);
    let tag_model = v["env"]["initial"]["tags"].as_array().into_iter().flatten().filter_map(|t| t.as_str()?.strip_prefix("model:")).next().unwrap_or("").to_string();
    let msgs = v["messages"].as_array().cloned().unwrap_or_default();
    // 每个 tool_use 的结果
    let mut status: HashMap<String, (String, i64)> = HashMap::new();
    for m in &msgs {
        for b in m["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_result") {
            let code = b["run"]["result"]["exitCode"].as_i64().unwrap_or(0);
            status.insert(b["toolUseID"].as_str().unwrap_or("").to_string(), (b["run"]["status"].as_str().unwrap_or("").to_string(), code));
        }
    }
    let (mut start, mut end, mut tokens, mut prompts) = (created, created, 0i64, 0i64);
    let mut ts = created;
    let mut models: HashMap<String, i64> = HashMap::new();
    for m in &msgs {
        if let Some(t) = any_ts(&m["meta"]["sentAt"]).or(any_ts(&m["usage"]["timestamp"])) {
            ts = t;
            if start == 0 || t < start { start = t; }
            end = end.max(t);
        }
        let u = &m["usage"];
        if m["role"] == "user" && m["content"].as_array().map(|c| c.iter().any(|b| b["type"] == "text")).unwrap_or(false) {
            prompts += 1;
        }
        tokens += ["inputTokens", "outputTokens", "cacheCreationInputTokens", "cacheReadInputTokens"].iter().map(|k| u[*k].as_i64().unwrap_or(0)).sum::<i64>();
        let model = u["model"].as_str().map(String::from).unwrap_or_else(|| tag_model.clone());
        if m["role"] != "assistant" {
            continue;
        }
        if !model.is_empty() {
            *models.entry(model.clone()).or_default() += 1;
        }
        for b in m["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_use") {
            let name = b["name"].as_str().unwrap_or("");
            match status.get(b["id"].as_str().unwrap_or("")) {
                Some((s, code)) if s == "done" && *code == 0 => {}
                _ => continue,
            }
            // shell：命令原文作为它提到的文件的候选证据（同 Claude Code、Codex）
            if name == "Bash" || name == "shell_command" {
                let input = &b["input"];
                let cmd = input["cmd"].as_str().or(input["command"].as_str()).unwrap_or("");
                let wd = input["cwd"].as_str().or(input["workdir"].as_str()).unwrap_or(&cwd);
                for rel in crate::shell_targets(cmd, wd, pm) {
                    crate::push_rel(&mut g.evs, &rel, cmd, Src::Amp, &model, &id, ts);
                }
                continue;
            }
            for (p, text) in edits(name, &b["input"], &cwd) {
                crate::push_text(&mut g.evs, pm, &p, &text, Src::Amp, &model, &id, ts);
            }
        }
    }
    let model = models.into_iter().max_by_key(|(m, n)| (*n, m.clone())).map(|(m, _)| m).unwrap_or(tag_model);
    if start > 0 {
        g.sessions.push(Session { src: Src::Amp, id, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true, provider: None });
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for f in threads() {
        let gk = format!("amp:{}", f.display());
        let fp = fingerprint(&f);
        if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
            n += g.sessions.len();
            continue;
        }
        let v: Value = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut g = FileGroup { fp, ..Default::default() };
        if pm.in_project(&cwd_of(&v)) {
            parse(&v, pm, &mut g);
        }
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("amp: 项目相关线程 {n} 个", "amp: {n} threads for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for f in threads() {
        let Some(v) = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        let cwd = cwd_of(&v);
        if cwd.is_empty() {
            continue;
        }
        let remote = v["env"]["initial"]["trees"][0]["repository"]["url"].as_str().map(String::from);
        let last = v["messages"].as_array().into_iter().flatten().filter_map(|m| any_ts(&m["meta"]["sentAt"]).or(any_ts(&m["usage"]["timestamp"]))).max().or(any_ts(&v["created"])).unwrap_or(0);
        let e = m.entry(cwd.clone()).or_insert(Activity { src: Src::Amp, cwd, remote, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(last);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let base = source_dir(Src::Amp);
    let ts = threads();
    let created: Vec<i64> = ts.iter().filter_map(|f| std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| any_ts(&v["created"]))).collect();
    Health {
        source: "amp".into(),
        path: base.join("threads").display().to_string(),
        readable: base.is_dir(),
        format: "threads/T-*.json".into(),
        records: ts.len() as u64,
        earliest: created.iter().min().copied().map(date_local),
        latest: created.iter().max().copied().map(date_local),
        note: if base.is_dir() && ts.is_empty() {
            creditia_core::tr!("本地没有线程文件：新版 Amp 把线程存在 ampcode.com 上，读不到", "no local thread files: newer Amp keeps threads on ampcode.com, which creditia does not read")
        } else {
            String::new()
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_finished_tool_runs_count() {
        let pm = PathMap { known: ["src/a.ts".to_string(), "src/b.ts".to_string(), "src/c.ts".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] };
        let v = json!({
            "id": "T-1", "created": 1_787_000_000_000i64,
            "env": { "initial": { "trees": [{ "uri": "file:///home/me/demo" }], "tags": ["model:claude-opus-4-6"] } },
            "messages": [
                { "role": "user", "content": [{ "type": "text", "text": "go" }], "meta": { "sentAt": 1_787_000_010_000i64 } },
                { "role": "assistant", "usage": { "model": "claude-opus-4-6", "inputTokens": 10, "outputTokens": 5, "timestamp": "2026-08-17T20:54:00Z" }, "content": [
                    { "type": "tool_use", "id": "u1", "name": "create_file", "input": { "path": "/home/me/demo/src/a.ts", "content": "export const alpha = 1;" } },
                    { "type": "tool_use", "id": "u2", "name": "edit_file", "input": { "path": "src/b.ts", "old_str": "x", "new_str": "x\nconst rejectedLine = 2;" } },
                    { "type": "tool_use", "id": "u3", "name": "apply_patch", "input": { "patchText": "*** Begin Patch\n*** Update File: src/c.ts\n@@\n+const patched = 3;\n*** End Patch" } }
                ] },
                { "role": "user", "content": [
                    { "type": "tool_result", "toolUseID": "u1", "run": { "status": "done", "result": {} } },
                    { "type": "tool_result", "toolUseID": "u2", "run": { "status": "rejected-by-user" } },
                    { "type": "tool_result", "toolUseID": "u3", "run": { "status": "done" } }
                ] }
            ]
        });
        let mut g = FileGroup::default();
        parse(&v, &pm, &mut g);
        let files: Vec<&str> = g.evs.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(files, vec!["src/a.ts", "src/c.ts"]);
        assert_eq!(g.sessions[0].model, "claude-opus-4-6");
        assert_eq!(g.sessions[0].tokens, 15);
        assert_eq!(g.sessions[0].prompts, 1);
    }
}
