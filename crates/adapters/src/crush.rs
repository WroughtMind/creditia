//! Crush（charmbracelet/crush）：每个项目一个 SQLite 库，默认在 `<项目>/.crush/crush.db`。
//! 全局的 `~/.local/share/crush/projects.json`（Windows 是 `%LOCALAPPDATA%\crush`）登记了每个项目的 `path` 和 `data_dir`；
//! 当前仓库根目录下的 `.crush/crush.db` 没登记也会读。
//! 证据取 `files` 表：write / edit / multiedit 成功写盘后，按（会话, 绝对路径）存一版完整内容（version 从 0 起，0 是改之前的样子），
//! 相邻两版之间多出来的行就是这次写下的。用户在两次编辑之间手改过文件时，Crush 会先存一版「手改后的样子」、紧接着再存 AI 的新版本，
//! 两版时间相差不到一秒：这种中间版本和上一版的差别是人改的，不算。
//! 模型取写入之前最近一条 assistant 消息的 `model`，渠道取 `provider`；token 是会话的 prompt_tokens + completion_tokens。

use crate::{added_lines, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 全局数据目录（projects.json 所在处）
pub fn global_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(p) = env("CREDITIA_CRUSH_DIR").or_else(|| env("CRUSH_GLOBAL_DATA")) {
        return p;
    }
    #[cfg(target_os = "windows")]
    if let Some(p) = env("LOCALAPPDATA") {
        return p.join("crush");
    }
    crate::xdg_data().join("crush")
}

/// （项目目录, 库文件）：登记过的项目，加上当前仓库根目录下的 .crush
fn dbs(roots: &[String]) -> Vec<(String, PathBuf)> {
    let reg: Value = std::fs::read_to_string(global_dir().join("projects.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut out: Vec<(String, PathBuf)> = vec![];
    for p in reg["projects"].as_array().into_iter().flatten() {
        let (Some(path), Some(data)) = (p["path"].as_str(), p["data_dir"].as_str()) else { continue };
        out.push((path.to_string(), PathBuf::from(data).join("crush.db")));
    }
    for r in roots {
        out.push((r.clone(), Path::new(r).join(".crush/crush.db")));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|(_, db)| db.is_file() && seen.insert(std::fs::canonicalize(db).unwrap_or_else(|_| db.clone())));
    out
}

#[derive(Default)]
struct Sess {
    id: String,
    parent: Option<String>,
    created: i64,
    updated: i64,
    tokens: i64,
    prompts: i64,
    /// assistant 消息：(时间, 模型, 渠道)，按时间排
    turns: Vec<(i64, String, String)>,
}

/// files 表里的一版：(会话, 路径, 版本, 内容, 时间)
type Version = (String, String, i64, String, i64);

fn sessions(c: &rusqlite::Connection) -> Vec<Sess> {
    let mut by: HashMap<String, Sess> = HashMap::new();
    let mut order: Vec<String> = vec![];
    if let Ok(mut q) = c.prepare("SELECT id, parent_session_id, created_at, updated_at, prompt_tokens, completion_tokens FROM sessions") {
        let rows = q.query_map([], |r| Ok(Sess {
            id: r.get(0)?,
            parent: r.get::<_, Option<String>>(1)?.filter(|s| !s.is_empty()),
            created: secs(r.get::<_, Option<i64>>(2)?.unwrap_or(0)),
            updated: secs(r.get::<_, Option<i64>>(3)?.unwrap_or(0)),
            tokens: r.get::<_, Option<i64>>(4)?.unwrap_or(0) + r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            ..Default::default()
        }));
        for s in rows.into_iter().flatten().flatten() {
            order.push(s.id.clone());
            by.insert(s.id.clone(), s);
        }
    }
    // provider 列是后来加的：老库没有就不取
    let sql = if has_column(c, "messages", "provider") {
        "SELECT session_id, role, parts, model, provider, created_at FROM messages ORDER BY created_at"
    } else {
        "SELECT session_id, role, parts, model, '', created_at FROM messages ORDER BY created_at"
    };
    if let Ok(mut q) = c.prepare(sql) {
        let rows = q.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?.unwrap_or_default(), r.get::<_, Option<String>>(3)?.unwrap_or_default(), r.get::<_, Option<String>>(4)?.unwrap_or_default(), r.get::<_, Option<i64>>(5)?.unwrap_or(0)))
        });
        for (sid, role, parts, model, provider, ts) in rows.into_iter().flatten().flatten() {
            let Some(s) = by.get_mut(&sid) else { continue };
            if role == "user" {
                // 自动补的续写（hidden）不算用户提的
                let parts: Value = serde_json::from_str(&parts).unwrap_or_default();
                let typed = parts.as_array().into_iter().flatten().any(|p| p["type"] == "text" && p["data"]["hidden"] != true && p["data"]["text"].as_str().is_some_and(|t| !t.trim().is_empty()));
                if typed {
                    s.prompts += 1;
                }
            } else if role == "assistant" {
                s.turns.push((secs(ts), model, provider));
            }
        }
    }
    order.into_iter().filter_map(|id| by.remove(&id)).collect()
}

fn versions(c: &rusqlite::Connection) -> Vec<Version> {
    let Ok(mut q) = c.prepare("SELECT session_id, path, version, content, created_at FROM files ORDER BY session_id, path, version") else { return vec![] };
    let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default(), secs(r.get::<_, Option<i64>>(4)?.unwrap_or(0)))));
    rows.into_iter().flatten().flatten().collect()
}

fn has_column(c: &rusqlite::Connection, table: &str, col: &str) -> bool {
    c.prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1")).and_then(|mut q| q.exists([col])).unwrap_or(false)
}

/// 库里存的是 unix 秒；万一是毫秒也认
fn secs(t: i64) -> i64 {
    if t > 10_000_000_000 { t / 1000 } else { t }
}

/// 一个文件在一个会话里的各版之间 AI 写下的行：[(新增的行, 时间)]。紧跟着下一版（一秒内）的是用户手改后存的中间版本，它和上一版的差别不算
fn written(vs: &[&Version]) -> Vec<(String, i64)> {
    let mut out = vec![];
    for k in 1..vs.len() {
        let intermediate = vs.get(k + 1).is_some_and(|n| n.4 - vs[k].4 <= 1);
        if intermediate {
            continue;
        }
        let text = added_lines(&vs[k - 1].3, &vs[k].3);
        if !text.trim().is_empty() {
            out.push((text, vs[k].4));
        }
    }
    out
}

fn top<'a>(it: impl Iterator<Item = &'a str>) -> String {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for s in it.filter(|s| !s.is_empty()) {
        *m.entry(s).or_default() += 1;
    }
    m.into_iter().max_by_key(|(s, n)| (*n, std::cmp::Reverse(*s))).map(|(s, _)| s.to_string()).unwrap_or_default()
}

pub fn scan(store: &mut Store, pm: &PathMap, roots: &[String], st: &mut Stats) {
    let mut n = 0;
    for (proj, db) in dbs(roots) {
        if !pm.in_project(&proj) && !roots.contains(&proj) {
            continue;
        }
        let gk = format!("crush:{}", db.display());
        // 指纹：库文件 + WAL
        let fp = {
            let a = fingerprint(&db);
            let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
            (a.0.max(b.0), a.1 + b.1)
        };
        if let Some(g) = store.groups.get(&gk).filter(|g| g.fp == fp) {
            n += g.sessions.len();
            continue;
        }
        let c = match open_ro(&db) {
            Ok(c) => c,
            Err(e) => {
                st.warnings.push(creditia_core::tr!("crush: 打不开 {}：{e}", "crush: couldn't open {}: {e}", db.display()));
                continue;
            }
        };
        let mut g = FileGroup { fp, ..Default::default() };
        let ss = sessions(&c);
        let vs = versions(&c);
        let mut by_file: HashMap<(&str, &str), Vec<&Version>> = HashMap::new();
        for v in &vs {
            by_file.entry((v.0.as_str(), v.1.as_str())).or_default().push(v);
        }
        let mut with_ev: std::collections::HashSet<String> = std::collections::HashSet::new();
        let idx: HashMap<&str, &Sess> = ss.iter().map(|s| (s.id.as_str(), s)).collect();
        for ((sid, path), list) in &by_file {
            let Some(s) = idx.get(sid) else { continue };
            let fallback = top(s.turns.iter().map(|t| t.1.as_str()));
            for (text, ts) in written(list) {
                // 写入之前最近的一条 assistant 消息用的模型
                let model = s.turns.iter().rev().find(|t| t.0 <= ts && !t.1.is_empty()).map(|t| t.1.clone()).unwrap_or_else(|| fallback.clone());
                if crate::push_text(&mut g.evs, pm, path, &text, Src::Crush, &model, sid, ts) {
                    with_ev.insert(sid.to_string());
                }
            }
        }
        // 子会话（agent 工具派出去的）没写文件就并进父会话：只加 token
        let mut extra: HashMap<String, i64> = HashMap::new();
        for s in &ss {
            if let Some(p) = &s.parent {
                if !with_ev.contains(&s.id) && idx.contains_key(p.as_str()) {
                    *extra.entry(p.clone()).or_default() += s.tokens;
                }
            }
        }
        for s in &ss {
            if s.turns.is_empty() || (s.parent.as_ref().is_some_and(|p| idx.contains_key(p.as_str())) && !with_ev.contains(&s.id)) {
                continue;
            }
            let model = top(s.turns.iter().map(|t| t.1.as_str()));
            let provider = Some(top(s.turns.iter().map(|t| t.2.as_str()))).filter(|p| !p.is_empty());
            let end = s.updated.max(s.turns.last().map(|t| t.0).unwrap_or(0));
            let tokens = s.tokens + extra.get(&s.id).copied().unwrap_or(0);
            g.sessions.push(Session { src: Src::Crush, id: s.id.clone(), model, start: s.created, end, tokens, branch: None, prompts: s.prompts, reviewer: false, has_content: true, provider });
        }
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("crush: 项目相关会话 {n} 个", "crush: {n} sessions for this project"));
    }
}

/// 每个登记过的项目：会话数、最近一次活动
fn summary(db: &Path) -> Option<(u64, i64, i64)> {
    let c = open_ro(db).ok()?;
    c.query_row("SELECT COUNT(*), COALESCE(MIN(created_at), 0), COALESCE(MAX(updated_at), 0) FROM sessions WHERE parent_session_id IS NULL OR parent_session_id = ''", [], |r| Ok((r.get::<_, i64>(0)? as u64, secs(r.get(1)?), secs(r.get(2)?)))).ok()
}

pub fn discover() -> Vec<Activity> {
    dbs(&[])
        .into_iter()
        .filter_map(|(path, db)| summary(&db).filter(|s| s.0 > 0).map(|(n, _, last)| Activity { src: Src::Crush, cwd: path, remote: None, sessions: n, last }))
        .collect()
}

pub fn doctor() -> Health {
    let all: Vec<(u64, i64, i64)> = dbs(&[]).iter().filter_map(|(_, db)| summary(db)).collect();
    let reg = global_dir().join("projects.json");
    Health {
        source: "crush".into(),
        path: reg.display().to_string(),
        readable: reg.is_file(),
        format: "projects.json + <project>/.crush/crush.db (sessions, messages, files)".into(),
        records: all.iter().map(|s| s.0).sum(),
        earliest: all.iter().map(|s| s.1).filter(|&t| t > 0).min().map(date_cn),
        latest: all.iter().map(|s| s.2).max().filter(|&t| t > 0).map(date_cn),
        note: creditia_core::tr!(
            "每个项目一个库；逐版文件内容、每条消息的模型与渠道。尚未用真实日志核对",
            "one database per project; per-version file contents, model and provider per message. Not yet checked against real logs"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(ver: i64, content: &str, ts: i64) -> Version {
        ("s".into(), "/r/a.rs".into(), ver, content.into(), ts)
    }

    #[test]
    fn versions_to_written_lines() {
        // v0 改之前；v1 AI 写；用户手改（v2 是手改后的样子，紧跟着 v3 是 AI 的新版本）
        let vs = [v(0, "a\n", 100), v(1, "a\nai one\n", 110), v(2, "a\nai one\nhuman\n", 200), v(3, "a\nai one\nhuman\nai two\n", 200)];
        let refs: Vec<&Version> = vs.iter().collect();
        assert_eq!(written(&refs), vec![("ai one\n".to_string(), 110), ("ai two\n".to_string(), 200)]);
        // 只有初版：没写过
        assert!(written(&refs[..1]).is_empty());
    }

    #[test]
    fn reads_a_database() {
        let dir = std::env::temp_dir().join(format!("creditia-crush-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("proj/.crush")).unwrap();
        let db = dir.join("proj/.crush/crush.db");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, title TEXT, message_count INTEGER, prompt_tokens INTEGER, completion_tokens INTEGER, cost REAL, updated_at INTEGER, created_at INTEGER);
             CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT, role TEXT, parts TEXT, model TEXT, created_at INTEGER, updated_at INTEGER, finished_at INTEGER, provider TEXT);
             CREATE TABLE files (id TEXT PRIMARY KEY, session_id TEXT, path TEXT, content TEXT, version INTEGER, created_at INTEGER, updated_at INTEGER);
             INSERT INTO sessions VALUES ('s1', NULL, 't', 4, 1000, 200, 0, 300, 100), ('sub', 's1', 't', 2, 50, 5, 0, 150, 120);
             INSERT INTO messages VALUES
               ('m1', 's1', 'user', '[{\"type\":\"text\",\"data\":{\"text\":\"add it\"}}]', '', 100, 100, 100, ''),
               ('m2', 's1', 'assistant', '[]', 'claude-sonnet-4-5', 105, 105, 106, 'anthropic'),
               ('m3', 's1', 'user', '[{\"type\":\"text\",\"data\":{\"text\":\"continue\",\"hidden\":true}}]', '', 130, 130, 130, ''),
               ('m4', 's1', 'assistant', '[]', 'gpt-5', 140, 140, 141, 'openai'),
               ('m5', 'sub', 'assistant', '[]', 'gpt-5', 125, 125, 125, 'openai');
             INSERT INTO files VALUES
               ('f0', 's1', '/r/proj/src/a.rs', '', 0, 110, 110),
               ('f1', 's1', '/r/proj/src/a.rs', 'fn first_written() {}\n', 1, 110, 110),
               ('f2', 's1', '/r/proj/src/a.rs', 'fn first_written() {}\nfn later_written() {}\n', 2, 150, 150);",
        )
        .unwrap();
        drop(c);
        let c = open_ro(&db).unwrap();
        let ss = sessions(&c);
        let s1 = ss.iter().find(|s| s.id == "s1").unwrap();
        // hidden 的续写不算一次提问
        assert_eq!(s1.prompts, 1);
        assert_eq!(s1.turns.len(), 2);
        let vs = versions(&c);
        let refs: Vec<&Version> = vs.iter().collect();
        let w = written(&refs);
        assert_eq!(w, vec![("fn first_written() {}\n".to_string(), 110), ("fn later_written() {}\n".to_string(), 150)]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
