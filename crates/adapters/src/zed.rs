//! Zed 的 agent 面板：`<数据目录>/threads/threads.db`（Linux `~/.local/share/zed`，macOS `~/Library/Application Support/Zed`，
//! Windows `%LOCALAPPDATA%\Zed`）。threads 表一行一个线程：folder_paths（项目根目录，换行分隔）、created_at / updated_at（RFC 3339）、
//! data（data_type 为 zstd 时是压缩过的 JSON）。只读 version 0.3.0 的新格式（老版 agent 的线程 Zed 打开时才会升级）。
//! JSON 里 messages[] 是 `{"User": …}` / `{"Agent": …}`；Agent 的 tool_results 按工具调用 id 存结果，
//! edit_file / write_file 成功时 output 带着改之前和改之后的完整内容（old_text、new_text），两者之差就是这次写下的行；
//! is_error 的不算。terminal 的命令原文作为命令里提到的文件的候选证据。
//! 模型取线程的 model（provider + model）；线程里没有逐条时间，证据时间取线程的创建时间。

use crate::{added_lines, open_ro, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, parse_iso};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(p) = env("CREDITIA_ZED_DIR") {
        return p;
    }
    #[cfg(target_os = "macos")]
    return crate::home().join("Library/Application Support/Zed");
    #[cfg(target_os = "windows")]
    return env("LOCALAPPDATA").unwrap_or_default().join("Zed");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    env("FLATPAK_XDG_DATA_HOME").unwrap_or_else(crate::xdg_data).join("zed")
}

fn db_path() -> PathBuf {
    data_dir().join("threads/threads.db")
}

struct Row {
    id: String,
    folders: Vec<String>,
    created: i64,
    updated: i64,
    kind: String,
    data: Vec<u8>,
}

/// 线程的元数据（不取 data，解压按需做）
fn rows(c: &rusqlite::Connection) -> Vec<Row> {
    let has = |col: &str| c.prepare(&format!("SELECT 1 FROM pragma_table_info('threads') WHERE name = '{col}'")).and_then(|mut q| q.exists([])).unwrap_or(false);
    let folders = if has("folder_paths") { "folder_paths" } else { "NULL" };
    let created = if has("created_at") { "created_at" } else { "updated_at" };
    let sql = format!("SELECT id, {folders}, {created}, updated_at FROM threads");
    let Ok(mut q) = c.prepare(&sql) else { return vec![] };
    let it = q.query_map([], |r| {
        Ok(Row {
            id: r.get(0)?,
            folders: r.get::<_, Option<String>>(1)?.unwrap_or_default().lines().filter(|l| !l.is_empty()).map(String::from).collect(),
            created: r.get::<_, Option<String>>(2)?.and_then(|s| parse_iso(&s)).unwrap_or(0),
            updated: r.get::<_, Option<String>>(3)?.and_then(|s| parse_iso(&s)).unwrap_or(0),
            kind: String::new(),
            data: vec![],
        })
    });
    it.into_iter().flatten().flatten().collect()
}

fn decode(r: &Row) -> Value {
    let bytes = if r.kind == "zstd" { zstd::stream::decode_all(&r.data[..]).unwrap_or_default() } else { r.data.clone() };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// 线程的项目根目录：表里的 folder_paths，老线程用 JSON 里的 worktree 快照
fn roots(r: &Row, v: &Value) -> Vec<String> {
    if !r.folders.is_empty() {
        return r.folders.clone();
    }
    v["initial_project_snapshot"]["worktree_snapshots"].as_array().into_iter().flatten().filter_map(|w| w["worktree_path"].as_str().map(String::from)).collect()
}

/// 工具里的路径以项目根目录的名字开头（`demo/src/a.rs`）：换成绝对路径
fn resolve(p: &str, roots: &[String]) -> String {
    let p = p.replace('\\', "/");
    if p.starts_with('/') || p.get(1..2) == Some(":") {
        return p;
    }
    let first = p.split('/').next().unwrap_or("");
    for r in roots {
        let r = r.replace('\\', "/");
        let r = r.trim_end_matches('/');
        if r.rsplit('/').next() == Some(first) {
            let parent = r.rsplit_once('/').map(|x| x.0).unwrap_or("");
            return format!("{parent}/{p}");
        }
    }
    // 单根项目也可能直接给相对路径
    roots.first().map(|r| format!("{}/{p}", r.trim_end_matches('/'))).unwrap_or(p)
}

/// 工具调用的输入：新格式是 {"type": "json", "value": …}，老的直接是对象
fn input_of(u: &Value) -> Value {
    let i = &u["input"];
    if i["type"] == "json" && i.get("value").is_some() {
        return i["value"].clone();
    }
    if i["type"] == "text" {
        return i["value"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    }
    if i.is_object() {
        return i.clone();
    }
    u["raw_input"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
}

/// 一个线程里成功写下的（绝对路径, 新增的行），shell 命令（路径为空, 命令, 工作目录）另算
struct Parsed {
    edits: Vec<(String, String)>,
    shells: Vec<(String, String)>,
    prompts: i64,
    agents: usize,
}

fn parse(v: &Value, roots: &[String]) -> Parsed {
    let mut out = Parsed { edits: vec![], shells: vec![], prompts: 0, agents: 0 };
    for m in v["messages"].as_array().into_iter().flatten() {
        if let Some(u) = m.get("User") {
            if u["content"].as_array().into_iter().flatten().any(|c| c["Text"].as_str().is_some_and(|t| !t.trim().is_empty())) {
                out.prompts += 1;
            }
            continue;
        }
        let Some(a) = m.get("Agent") else { continue };
        out.agents += 1;
        let results = &a["tool_results"];
        for c in a["content"].as_array().into_iter().flatten() {
            let Some(u) = c.get("ToolUse") else { continue };
            let id = u["id"].as_str().unwrap_or("");
            let res = &results[id];
            if res.is_null() || res["is_error"] == true {
                continue;
            }
            let name = u["name"].as_str().unwrap_or("");
            match name {
                "edit_file" | "write_file" | "streaming_edit_file" => {
                    let o = &res["output"];
                    let (Some(new), Some(path)) = (o["new_text"].as_str(), o["input_path"].as_str().or(o["original_path"].as_str())) else { continue };
                    let text = added_lines(o["old_text"].as_str().unwrap_or(""), new);
                    if !text.trim().is_empty() {
                        out.edits.push((resolve(path, roots), text));
                    }
                }
                "terminal" | "sandboxed_terminal" => {
                    let i = input_of(u);
                    if let Some(cmd) = i["command"].as_str() {
                        let cd = i["cd"].as_str().unwrap_or("");
                        out.shells.push((cmd.to_string(), if cd.is_empty() { roots.first().cloned().unwrap_or_default() } else { resolve(cd, roots) }));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = db_path();
    if !db.is_file() {
        return;
    }
    let c = match open_ro(&db) {
        Ok(c) => c,
        Err(e) => {
            st.warnings.push(creditia_core::tr!("zed: 打不开 {}：{e}", "zed: couldn't open {}: {e}", db.display()));
            return;
        }
    };
    let mut n = 0;
    // 先只读元数据挑出本项目的线程，再按需解压
    for meta in rows(&c) {
        let gk = format!("zed:{}", meta.id);
        // 指纹：线程的更新时间（库文件整体会被别的线程改动）
        let fp = (meta.updated.max(0) as u64, 0);
        if !meta.folders.is_empty() && !meta.folders.iter().any(|f| pm.in_project(f)) {
            continue;
        }
        if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
            n += store.groups[&gk].sessions.len();
            continue;
        }
        let Ok((kind, data)) = c.query_row("SELECT data_type, data FROM threads WHERE id = ?1", [&meta.id], |x| Ok((x.get::<_, Option<String>>(0)?.unwrap_or_default(), x.get::<_, Option<Vec<u8>>>(1)?.unwrap_or_default()))) else { continue };
        let r = Row { kind, data, ..meta };
        let v = decode(&r);
        // 只认新格式
        if v["version"].as_str() != Some("0.3.0") {
            continue;
        }
        let roots = roots(&r, &v);
        if !roots.iter().any(|f| pm.in_project(f)) {
            continue;
        }
        let p = parse(&v, &roots);
        if p.agents == 0 {
            continue;
        }
        let model = v["model"]["model"].as_str().unwrap_or("").to_string();
        let provider = v["model"]["provider"].as_str().filter(|s| !s.is_empty()).map(String::from);
        let start = if r.created > 0 { r.created } else { r.updated };
        let mut g = FileGroup { fp, ..Default::default() };
        for (abs, text) in &p.edits {
            crate::push_text(&mut g.evs, pm, abs, text, Src::Zed, &model, &r.id, start);
        }
        for (cmd, cwd) in &p.shells {
            for rel in crate::shell_targets(cmd, cwd, pm) {
                crate::push_rel(&mut g.evs, &rel, cmd, Src::Zed, &model, &r.id, start);
            }
        }
        let u = &v["cumulative_token_usage"];
        let tokens = ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"].iter().map(|k| u[*k].as_i64().unwrap_or(0)).sum();
        g.sessions.push(Session { src: Src::Zed, id: r.id.clone(), model, start, end: r.updated.max(start), tokens, branch: None, prompts: p.prompts, reviewer: false, has_content: true, provider });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("zed: 项目相关线程 {n} 个", "zed: {n} threads for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let Ok(c) = open_ro(&db_path()) else { return vec![] };
    let mut m: HashMap<String, Activity> = HashMap::new();
    for r in rows(&c) {
        for f in &r.folders {
            let e = m.entry(f.clone()).or_insert(Activity { src: Src::Zed, cwd: f.clone(), remote: None, sessions: 0, last: 0 });
            e.sessions += 1;
            e.last = e.last.max(r.updated);
        }
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let db = db_path();
    let rs = open_ro(&db).map(|c| rows(&c)).unwrap_or_default();
    Health {
        source: "zed".into(),
        path: db.display().to_string(),
        readable: db.is_file(),
        format: "threads.db (zstd JSON, version 0.3.0)".into(),
        records: rs.len() as u64,
        earliest: rs.iter().map(|r| r.created).filter(|&t| t > 0).min().map(date_cn),
        latest: rs.iter().map(|r| r.updated).max().filter(|&t| t > 0).map(date_cn),
        note: creditia_core::tr!(
            "编辑结果带改前改后的完整内容；线程级的模型。尚未用真实日志核对",
            "edit results carry full before / after contents; model per thread. Not yet checked against real logs"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_start_with_the_root_name() {
        let roots = vec!["/home/me/demo".to_string(), "/home/me/lib".to_string()];
        assert_eq!(resolve("demo/src/a.rs", &roots), "/home/me/demo/src/a.rs");
        assert_eq!(resolve("lib/x.rs", &roots), "/home/me/lib/x.rs");
        assert_eq!(resolve("/abs/y.rs", &roots), "/abs/y.rs");
        assert_eq!(resolve("src/a.rs", &roots[..1]), "/home/me/demo/src/a.rs");
    }

    #[test]
    fn thread_to_edits() {
        let v = json!({ "version": "0.3.0", "messages": [
            { "User": { "id": "u", "content": [{ "Text": "do it" }] } },
            { "Agent": { "content": [
                { "Text": "ok" },
                { "ToolUse": { "id": "t1", "name": "edit_file", "raw_input": "", "input": { "type": "json", "value": { "path": "demo/a.rs" } }, "is_input_complete": true } },
                { "ToolUse": { "id": "t2", "name": "write_file", "raw_input": "", "input": { "type": "json", "value": {} }, "is_input_complete": true } },
                { "ToolUse": { "id": "t3", "name": "terminal", "raw_input": "", "input": { "type": "json", "value": { "command": "ls", "cd": "demo" } }, "is_input_complete": true } }
              ],
              "tool_results": {
                "t1": { "tool_use_id": "t1", "tool_name": "edit_file", "is_error": false, "content": "ok", "output": { "input_path": "demo/a.rs", "old_text": "a\n", "new_text": "a\nb\n", "diff": "" } },
                "t2": { "tool_use_id": "t2", "tool_name": "write_file", "is_error": true, "content": "denied", "output": { "error": "denied" } },
                "t3": { "tool_use_id": "t3", "tool_name": "terminal", "is_error": false, "content": "", "output": "" }
              } } },
            "Resume"
        ] });
        let p = parse(&v, &["/r/demo".to_string()]);
        assert_eq!(p.prompts, 1);
        assert_eq!(p.agents, 1);
        assert_eq!(p.edits, vec![("/r/demo/a.rs".to_string(), "b\n".to_string())]);
        assert_eq!(p.shells, vec![("ls".to_string(), "/r/demo".to_string())]);
    }
}
