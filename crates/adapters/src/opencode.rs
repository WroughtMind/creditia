//! OpenCode：`~/.local/share/opencode/`。存储换过三代，都读，同一个会话只算一次（库里的优先）：
//! - 最早是 `storage/` 下的 JSON 文件：session/<项目>/<会话>.json 有 directory；message/<会话>/<消息>.json 有 modelID、providerID、tokens；
//!   part/<消息>/<片段>.json 是工具调用。
//! - 2026 年 1 月底起是 SQLite（`opencode.db`，别的发布渠道是 `opencode-<渠道>.db`，`OPENCODE_DB` 可改）：
//!   v1 表 session / message / part，message.data 和 part.data 就是原来 JSON 文件的内容（去掉 id）；
//! - v2 表 session_v2 / session_message：type 列是 user / assistant…，assistant 的 data.content[] 里 type 为 tool 的是工具调用，
//!   模型在 data.model（id + providerID）。
//!
//! 工具调用只认 state.status 为 completed 的：edit / write（filePath 或 path）、patch（apply_patch 格式的 patchText），
//! bash / shell 的命令原文作为候选证据。providerID 原样记为渠道（anthropic、openai、openrouter，或者用户自定义的中转站名）。
//! Kilo Code 的命令行是 OpenCode 的分支：`~/.local/share/kilo/kilo.db`，同样的表，记为 Kilo Code。

use crate::{any_ts, apply_patch_files, new_text, open_ro, path_arg, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, Ev, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
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

/// Kilo 命令行的数据目录
pub fn kilo_dir() -> PathBuf {
    std::env::var("CREDITIA_KILO_CLI_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| crate::xdg_data().join("kilo"))
}

/// 各个库：(文件, 记成哪个工具)。OpenCode 的 opencode*.db，Kilo 命令行的 kilo*.db（它也可能沿用 opencode-<渠道>.db）
pub fn costrict_dir() -> PathBuf {
    std::env::var("CREDITIA_COSTRICT_CLI_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| crate::xdg_data().join("costrict"))
}

fn dbs() -> Vec<(PathBuf, Src)> {
    let mut out: Vec<(PathBuf, Src)> = vec![];
    let mut add = |dir: PathBuf, prefixes: &[&str], src: Src| {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let n = p.file_name().unwrap_or_default().to_string_lossy();
                n.ends_with(".db") && prefixes.iter().any(|x| n.starts_with(x))
            })
            .collect();
        v.sort();
        out.extend(v.into_iter().map(|p| (p, src)));
    };
    add(source_dir(Src::OpenCode), &["opencode"], Src::OpenCode);
    add(kilo_dir(), &["kilo", "opencode"], Src::Kilo);
    add(costrict_dir(), &["costrict", "opencode"], Src::Costrict);
    if let Some(p) = std::env::var("OPENCODE_DB").ok().filter(|s| s.ends_with(".db")).map(PathBuf::from).filter(|p| p.is_absolute() && p.is_file()) {
        if !out.iter().any(|(q, _)| q == &p) {
            out.push((p, Src::OpenCode));
        }
    }
    out
}

/// 一个会话里收集到的东西，三代存储共用
#[derive(Default)]
struct Acc {
    evs: Vec<Ev>,
    tokens: i64,
    prompts: i64,
    end: i64,
    models: HashMap<String, usize>,
    providers: HashMap<String, usize>,
}

impl Acc {
    fn assistant(&mut self, model: &str, provider: &str) {
        if !model.is_empty() {
            *self.models.entry(model.to_string()).or_default() += 1;
        }
        if !provider.is_empty() {
            *self.providers.entry(provider.to_string()).or_default() += 1;
        }
    }
    /// Tabnine's engine records preserve the historical model/time and actual
    /// completed diff; its UI selection is mutable and cannot replace them.
    #[allow(clippy::too_many_arguments)]
    fn completed_tool(&mut self, pm: &PathMap, src: Src, name: &str, state: &Value, cwd: &str, model: &str, sid: &str, ts: i64) {
        if src != Src::Tabnine {
            self.tool(pm, src, name, &state["input"], cwd, model, sid, ts);
            return;
        }
        if !matches!(name, "edit" | "write" | "patch" | "apply_patch") { return; }
        if let Some(diff) = state["metadata"]["diff"].as_str().filter(|_| matches!(name, "edit" | "write")).and_then(crate::tabnine_ide::diff_added) {
            if let Some(p) = path_arg(&state["input"]) {
                crate::trace_import::push_scoped_events(&mut self.evs, pm, cwd, &p, &diff, src, model, sid, ts);
            }
            return;
        }
        self.tool(pm, src, name, &state["input"], cwd, model, sid, ts);
    }
    /// 一次完成的工具调用
    #[allow(clippy::too_many_arguments)]
    fn tool(&mut self, pm: &PathMap, src: Src, name: &str, input: &Value, cwd: &str, model: &str, sid: &str, ts: i64) {
        let abs = |p: &str| if Path::new(p).is_absolute() || p.get(1..2) == Some(":") { p.to_string() } else { format!("{}/{}", cwd.trim_end_matches('/'), p) };
        match name {
            "bash" | "shell" => {
                if src == Src::Tabnine { return; }
                let Some(cmd) = input["command"].as_str() else { return };
                let wd = input["workdir"].as_str().map(abs).unwrap_or_else(|| cwd.to_string());
                for rel in crate::shell_targets(cmd, &wd, pm) {
                    crate::push_rel(&mut self.evs, &rel, cmd, src, model, sid, ts);
                }
            }
            "patch" | "apply_patch" => {
                for (p, text) in apply_patch_files(input["patchText"].as_str().unwrap_or("")) {
                    crate::push_text(&mut self.evs, pm, &abs(&p), &text, src, model, sid, ts);
                }
            }
            _ => {
                if matches!(src, Src::Costrict | Src::Tabnine) {
                    let n = name.to_lowercase();
                    let old = ["old_string", "oldString", "old_str"].iter().any(|k| input[*k].is_string());
                    let edits = input["edits"].as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|e| ["old_string", "oldString", "old_str"].iter().any(|k| e[*k].is_string())));
                    if n.contains("write") || n.contains("create") || (!old && !edits) { return; }
                }
                if let (Some(p), Some(text)) = (path_arg(input), new_text(name, input)) {
                    crate::push_text(&mut self.evs, pm, &abs(&p), &text, src, model, sid, ts);
                }
            }
        }
    }
    fn session(self, src: Src, id: &str, start: i64, end: i64, tokens: Option<i64>) -> FileGroup {
        let top = |m: HashMap<String, usize>| m.into_iter().max_by_key(|(k, n)| (*n, std::cmp::Reverse(k.clone()))).map(|(k, _)| k);
        let model = top(self.models).unwrap_or_default();
        let provider = top(self.providers);
        let tokens = tokens.filter(|&t| t > 0).unwrap_or(self.tokens);
        let s = Session { src, id: id.to_string(), model, start, end: end.max(self.end), tokens, branch: None, prompts: self.prompts, reviewer: false, has_content: true, provider };
        FileGroup { evs: self.evs, sessions: vec![s], ..Default::default() }
    }
}

fn v1_tokens(t: &Value) -> i64 {
    ["input", "output", "reasoning"].iter().map(|k| t[*k].as_i64().unwrap_or(0)).sum::<i64>() + t["cache"]["read"].as_i64().unwrap_or(0) + t["cache"]["write"].as_i64().unwrap_or(0)
}

/// v1 的一条消息（JSON 文件或 message.data）：返回模型，顺便记 token、提问
fn v1_message(acc: &mut Acc, m: &Value, fallback_ts: i64) -> (String, i64) {
    let ts = any_ts(&m["time"]["created"]).unwrap_or(fallback_ts);
    acc.end = acc.end.max(any_ts(&m["time"]["completed"]).unwrap_or(ts));
    if m["role"] == "user" {
        acc.prompts += 1;
        return (String::new(), ts);
    }
    let model = m["modelID"].as_str().unwrap_or("").to_string();
    acc.assistant(&model, m["providerID"].as_str().unwrap_or(""));
    acc.tokens += v1_tokens(&m["tokens"]);
    (model, ts)
}

// ───────────────────────── JSON 文件 ─────────────────────────

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

fn scan_files(store: &mut Store, pm: &PathMap, seen: &mut HashSet<(Src, String)>, tabnine: &HashSet<String>) -> usize {
    let mut n = 0;
    for s in sessions().into_iter().filter(|s| pm.in_project(&s.dir)) {
        let src = engine_source(Src::OpenCode, &s.id, tabnine);
        if !seen.insert((src, s.id.clone())) {
            continue;
        }
        let msg_dir = storage().join("message").join(&s.id);
        let gk = format!("{}:{}", src.harness(), s.id);
        // 指纹：会话文件 + 消息目录的修改时间
        let fp = { let a = fingerprint(&s.file); let b = fingerprint(&msg_dir); (a.0.max(b.0), a.1 + b.1) };
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            n += 1;
            continue;
        }
        let mut acc = Acc::default();
        for mf in json_files(&msg_dir) {
            let m = read(&mf);
            let (model, ts) = v1_message(&mut acc, &m, s.created);
            if m["role"] == "user" {
                continue;
            }
            let cwd = m["path"]["cwd"].as_str().unwrap_or(&s.dir).to_string();
            let mid = m["id"].as_str().map(String::from).unwrap_or_else(|| mf.file_stem().unwrap_or_default().to_string_lossy().to_string());
            for pf in json_files(&storage().join("part").join(&mid)) {
                let p = read(&pf);
                if p["type"] != "tool" || p["state"]["status"] != "completed" {
                    continue;
                }
                acc.completed_tool(pm, src, p["tool"].as_str().unwrap_or(""), &p["state"], &cwd, &model, &s.id, any_ts(&p["state"]["time"]["end"]).unwrap_or(ts));
            }
        }
        let mut g = acc.session(src, &s.id, s.created, s.updated, None);
        g.fp = fp;
        store.groups.insert(gk, g);
        n += 1;
    }
    n
}

// ───────────────────────── SQLite ─────────────────────────

fn has_table(c: &rusqlite::Connection, t: &str) -> bool {
    c.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |_| Ok(())).is_ok()
}

/// 库里的会话：(id, 目录, 创建, 更新, token 合计, 是否 v2)。v2 表里有的就不再算 v1 的
fn db_sessions(c: &rusqlite::Connection) -> Vec<(String, String, i64, i64, i64, bool)> {
    let mut out = vec![];
    let mut ids = HashSet::new();
    if has_table(c, "session_v2") && has_table(c, "session_message") {
        let sql = "SELECT s.id, s.directory, s.time_created, s.time_updated, s.tokens_input + s.tokens_output + s.tokens_reasoning + s.tokens_cache_read + s.tokens_cache_write FROM session_v2 s WHERE EXISTS (SELECT 1 FROM session_message m WHERE m.session_id = s.id)";
        if let Ok(mut q) = c.prepare(sql) {
            for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<i64>>(4)?.unwrap_or(0), true))).into_iter().flatten().flatten() {
                ids.insert(r.0.clone());
                out.push(r);
            }
        }
    }
    if has_table(c, "session") && has_table(c, "message") {
        if let Ok(mut q) = c.prepare("SELECT id, directory, time_created, time_updated FROM session") {
            for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, 0, false))).into_iter().flatten().flatten() {
                if !ids.contains(&r.0) {
                    out.push(r);
                }
            }
        }
    }
    for r in out.iter_mut() {
        r.2 = any_ts(&Value::from(r.2)).unwrap_or(0);
        r.3 = any_ts(&Value::from(r.3)).unwrap_or(0);
    }
    out
}

fn rows(c: &rusqlite::Connection, sql: &str, id: &str) -> Vec<(String, String, String, i64)> {
    let Ok(mut q) = c.prepare(sql) else { return vec![] };
    let it = q.query_map([id], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, Option<String>>(2)?.unwrap_or_default(), r.get::<_, Option<i64>>(3)?.unwrap_or(0))));
    it.into_iter().flatten().flatten().collect()
}

fn v1_db(acc: &mut Acc, c: &rusqlite::Connection, pm: &PathMap, src: Src, sid: &str, dir: &str, start: i64) {
    // (消息 id, data)
    let msgs = rows(c, "SELECT id, data, '', time_created FROM message WHERE session_id = ?1 ORDER BY time_created, id", sid);
    let mut parts: HashMap<String, Vec<Value>> = HashMap::new();
    for (mid, data, _, _) in rows(c, "SELECT message_id, data, '', time_created FROM part WHERE session_id = ?1 ORDER BY id", sid) {
        parts.entry(mid).or_default().push(serde_json::from_str(&data).unwrap_or_default());
    }
    for (mid, data, _, created) in msgs {
        let m: Value = serde_json::from_str(&data).unwrap_or_default();
        let (model, ts) = v1_message(acc, &m, any_ts(&Value::from(created)).unwrap_or(start));
        if m["role"] == "user" {
            continue;
        }
        let cwd = m["path"]["cwd"].as_str().unwrap_or(dir).to_string();
        for p in parts.get(&mid).into_iter().flatten() {
            if p["type"] != "tool" || p["state"]["status"] != "completed" {
                continue;
            }
            acc.completed_tool(pm, src, p["tool"].as_str().unwrap_or(""), &p["state"], &cwd, &model, sid, any_ts(&p["state"]["time"]["end"]).unwrap_or(ts));
        }
    }
}

fn v2_db(acc: &mut Acc, c: &rusqlite::Connection, pm: &PathMap, src: Src, sid: &str, dir: &str, start: i64) {
    for (kind, data, _, created) in rows(c, "SELECT type, data, '', time_created FROM session_message WHERE session_id = ?1 ORDER BY seq", sid) {
        let ts = any_ts(&Value::from(created)).unwrap_or(start);
        acc.end = acc.end.max(ts);
        match kind.as_str() {
            "user" => acc.prompts += 1,
            "assistant" => {
                let m: Value = serde_json::from_str(&data).unwrap_or_default();
                let model = m["model"]["id"].as_str().unwrap_or("").to_string();
                acc.assistant(&model, m["model"]["providerID"].as_str().unwrap_or(""));
                acc.end = acc.end.max(any_ts(&m["time"]["completed"]).unwrap_or(ts));
                for t in m["content"].as_array().into_iter().flatten().filter(|t| t["type"] == "tool" && t["state"]["status"] == "completed") {
                    let at = any_ts(&t["time"]["completed"]).unwrap_or(ts);
                    acc.completed_tool(pm, src, t["name"].as_str().unwrap_or(""), &t["state"], dir, &model, sid, at);
                }
            }
            _ => {}
        }
    }
}

fn engine_source(src: Src, id: &str, tabnine: &HashSet<String>) -> Src {
    if src == Src::OpenCode && tabnine.contains(id) { Src::Tabnine } else { src }
}

fn scan_dbs(store: &mut Store, pm: &PathMap, st: &mut Stats, seen: &mut HashSet<(Src, String)>, tabnine: &HashSet<String>) -> HashMap<Src, usize> {
    let mut n: HashMap<Src, usize> = HashMap::new();
    for (db, src) in dbs() {
        let c = match open_ro(&db) {
            Ok(c) => c,
            Err(e) => {
                st.warnings.push(creditia_core::tr!("{}: 打不开 {}：{e}", "{}: couldn't open {}: {e}", src.harness(), db.display()));
                continue;
            }
        };
        let fp = {
            let a = fingerprint(&db);
            let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
            (a.0.max(b.0), a.1 + b.1)
        };
        for (sid, dir, created, updated, tokens, v2) in db_sessions(&c) {
            let src = engine_source(src, &sid, tabnine);
            if !pm.in_project(&dir) || seen.contains(&(src, sid.clone())) {
                continue;
            }
            seen.insert((src, sid.clone()));
            *n.entry(src).or_default() += 1;
            let gk = format!("{}:{sid}", src.harness());
            if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
                continue;
            }
            let mut acc = Acc::default();
            if v2 {
                v2_db(&mut acc, &c, pm, src, &sid, &dir, created);
            } else {
                v1_db(&mut acc, &c, pm, src, &sid, &dir, created);
            }
            let mut g = acc.session(src, &sid, created, updated, Some(tokens));
            g.fp = fp;
            store.groups.insert(gk, g);
        }
    }
    n
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut tabnine = crate::tabnine_ide::session_ids();
    tabnine.extend(store.session_owners.iter().filter(|(_, owner)| **owner == Src::Tabnine).filter_map(|(key, _)| key.strip_prefix("opencode:").map(String::from)));
    tabnine.extend(store.groups.values().flat_map(|g| &g.sessions).filter(|s| s.src == Src::Tabnine).map(|s| s.id.clone()));
    for sid in &tabnine { store.session_owners.insert(format!("opencode:{sid}"), Src::Tabnine); }
    // Ownership may become known after a previous scan cached generic OpenCode.
    for sid in &tabnine { store.groups.remove(&format!("opencode:{sid}")); }
    let mut seen = HashSet::new();
    let by = scan_dbs(store, pm, st, &mut seen, &tabnine);
    let n = by.get(&Src::OpenCode).copied().unwrap_or(0) + scan_files(store, pm, &mut seen, &tabnine);
    if n > 0 {
        st.notes.push(creditia_core::tr!("opencode: 项目相关会话 {n} 个", "opencode: {n} sessions for this project"));
    }
    if let Some(n) = by.get(&Src::Costrict).filter(|&&n| n > 0) {
        st.notes.push(format!("costrict-cli: {n} sessions for this project"));
    }
    if let Some(k) = by.get(&Src::Kilo).filter(|&&k| k > 0) {
        st.notes.push(creditia_core::tr!("kilo 命令行: 项目相关会话 {k} 个", "kilo cli: {k} sessions for this project"));
    }
    if let Some(n) = by.get(&Src::Tabnine).filter(|&&n| n > 0) {
        st.notes.push(format!("tabnine-ide: {n} native engine sessions for this project"));
    }
}

/// 所有会话的（工具, 目录, 创建, 更新），discover / doctor 用
fn all_sessions() -> Vec<(Src, String, i64, i64)> {
    let tabnine = crate::tabnine_ide::session_ids();
    let mut seen = HashSet::new();
    let mut out = vec![];
    for (db, src) in dbs() {
        let Ok(c) = open_ro(&db) else { continue };
        for (id, dir, created, updated, _, _) in db_sessions(&c) {
            let src = engine_source(src, &id, &tabnine);
            if seen.insert((src, id)) {
                out.push((src, dir, created, updated));
            }
        }
    }
    for s in sessions() {
        let src = engine_source(Src::OpenCode, &s.id, &tabnine);
        if seen.insert((src, s.id)) {
            out.push((src, s.dir, s.created, s.updated));
        }
    }
    out
}

pub(crate) fn tabnine_native_session_ids() -> HashSet<String> {
    let mapped = crate::tabnine_ide::session_ids();
    let mut found = HashSet::new();
    for (db, src) in dbs().into_iter().filter(|(_, src)| *src == Src::OpenCode) {
        let Ok(c) = open_ro(&db) else { continue };
        for (id, _, _, _, _, _) in db_sessions(&c) {
            if engine_source(src, &id, &mapped) == Src::Tabnine { found.insert(id); }
        }
    }
    for s in sessions() {
        if mapped.contains(&s.id) { found.insert(s.id); }
    }
    found
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<(Src, String), Activity> = HashMap::new();
    for (src, dir, _, updated) in all_sessions().into_iter().filter(|s| !s.1.is_empty()) {
        let e = m.entry((src, dir.clone())).or_insert(Activity { src, cwd: dir, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(updated);
    }
    m.into_values().collect()
}

pub fn doctor() -> Vec<Health> {
    let all = all_sessions();
    let health = |src: Src, name: &str, path: PathBuf, readable: bool, note: String| {
        let ss: Vec<&(Src, String, i64, i64)> = all.iter().filter(|s| s.0 == src).collect();
        Health {
            source: name.into(),
            path: path.display().to_string(),
            readable,
            format: "opencode.db (session / message / part, session_v2 / session_message) + storage/*.json".into(),
            records: ss.len() as u64,
            earliest: ss.iter().map(|s| s.2).filter(|&t| t > 0).min().map(date_cn),
            latest: ss.iter().map(|s| s.3).max().filter(|&t| t > 0).map(date_cn),
            note,
            ..Default::default()
        }
    };
    let oc = source_dir(Src::OpenCode);
    let mut out = vec![health(Src::OpenCode, "opencode", oc.clone(), oc.is_dir(), creditia_core::tr!("逐条消息含模型、渠道与 token；新版存在 SQLite 里", "each message has model, provider and tokens; newer versions keep them in SQLite"))];
    // Kilo 命令行：装了才列
    if dbs().iter().any(|d| d.1 == Src::Kilo) {
        out.push(health(Src::Kilo, "kilo-cli", kilo_dir(), true, creditia_core::tr!("OpenCode 的分支，同样的表。尚未用真实日志核对", "a fork of OpenCode with the same tables. Not yet checked against real logs")));
    }
    if dbs().iter().any(|d| d.1 == Src::Costrict) {
        let mut h = health(Src::Costrict, "costrict-cli", costrict_dir(), true, creditia_core::tr!("官方 CoStrict SQLite session/message/part 格式；只读取已完成工具；已通过构造记录测试，尚无本机真实会话核验", "Official CoStrict SQLite session/message/part; completed tools only; fixture validated, no local real-session validation"));
        h.format = "costrict/opencode*.db (session / message / part)".into();
        out.push(h);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tabnine_engine_ownership_preserves_model_time_and_actual_diff() {
        let pm = PathMap { known: ["src/a.ts".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] };
        let ids = ["ses-native".to_string()].into_iter().collect();
        assert_eq!(engine_source(Src::OpenCode, "ses-native", &ids), Src::Tabnine);
        assert_eq!(engine_source(Src::Kilo, "ses-native", &ids), Src::Kilo);
        let mut acc = Acc::default();
        let state = json!({"input":{"filePath":"src/a.ts","content":"unproven whole text"}});
        acc.completed_tool(&pm, Src::Tabnine, "write", &state, "/w/demo", "native-model", "ses-native", 10);
        assert!(acc.evs.is_empty());
        let state = json!({"input":{"filePath":"src/a.ts"},"metadata":{"diff":"--- a\n+++ a\n@@ -1 +1,2 @@\n-keep\n+keep\n+const actualAdded = 1;\n"}});
        acc.completed_tool(&pm, Src::Tabnine, "read", &state, "/w/demo", "native-model", "ses-native", 10);
        assert!(acc.evs.is_empty());
        acc.completed_tool(&pm, Src::Tabnine, "write", &state, "/w/demo", "native-model", "ses-native", 10);
        assert_eq!(acc.evs.len(), 1);
        assert_eq!(acc.evs[0].model, "native-model");
        assert_eq!(acc.evs[0].ts, 10);
        assert_eq!(acc.evs[0].src, Src::Tabnine);
    }

    #[test]
    fn tool_kinds() {
        let pm = PathMap { known: ["src/a.ts".to_string(), "src/b.ts".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] };
        let mut acc = Acc::default();
        // v2 的 edit 用 path，v1 用 filePath；patch 是 apply_patch 格式
        acc.tool(&pm, Src::OpenCode, "edit", &json!({ "path": "src/a.ts", "oldString": "a", "newString": "a\nexport const fresh = 1;" }), "/w/demo", "m", "s", 1);
        acc.tool(&pm, Src::OpenCode, "write", &json!({ "filePath": "/w/demo/src/b.ts", "content": "export const written = 2;" }), "/w/demo", "m", "s", 1);
        acc.tool(&pm, Src::OpenCode, "patch", &json!({ "patchText": "*** Begin Patch\n*** Update File: src/a.ts\n@@\n+export const patched = 3;\n*** End Patch" }), "/w/demo", "m", "s", 1);
        acc.tool(&pm, Src::OpenCode, "read", &json!({ "filePath": "src/a.ts" }), "/w/demo", "m", "s", 1);
        let files: HashSet<&str> = acc.evs.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(files, ["src/a.ts", "src/b.ts"].into_iter().collect());
        assert_eq!(acc.evs.len(), 3);
        let mut strict = Acc::default();
        strict.tool(&pm, Src::Costrict, "write", &json!({"filePath":"/w/demo/src/a.ts","content":"human retained context"}), "/w/demo", "m", "s", 1);
        strict.tool(&pm, Src::Costrict, "edit", &json!({"filePath":"/w/demo/src/a.ts","oldString":"human context","newString":"human context\nactual added line"}), "/w/demo", "m", "s", 1);
        assert_eq!(strict.evs.len(), 1);
    }
}
