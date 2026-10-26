//! Cursor：行级记录 `ai-tracking/ai-code-tracking.db`（逐行哈希 → 模型、对话、来源），模型为 `default` 时回退到对话库的对话级模型。

use crate::{cursor_global_storage, open_ro, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, Ev, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use std::collections::HashMap;
use std::path::PathBuf;

fn db_path() -> PathBuf {
    source_dir(Src::Cursor).join("ai-tracking/ai-code-tracking.db")
}

struct Row {
    hash: String,
    source: String,
    file: String,
    conv: String,
    ts_ms: i64,
    model: String,
}

fn rows() -> Result<Vec<Row>, String> {
    let conn = open_ro(&db_path()).map_err(|e| creditia_core::tr!("cursor: 行级记录不可读 {e}", "cursor: line records unreadable {e}"))?;
    let mut stmt = conn
        .prepare("select hash, source, coalesce(fileName,''), coalesce(conversationId,''), coalesce(timestamp,createdAt), coalesce(model,'') from ai_code_hashes")
        .map_err(|e| creditia_core::tr!("cursor: ai_code_hashes 表结构不认识 {e}", "cursor: unknown ai_code_hashes table layout {e}"))?;
    let r = stmt
        .query_map([], |r| Ok(Row { hash: r.get(0)?, source: r.get(1)?, file: r.get(2)?, conv: r.get(3)?, ts_ms: r.get(4)?, model: r.get(5)? }))
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(r)
}

/// Auto 模式记为 default：回退到对话库里的对话级模型，仍然没有就记 `auto`
struct ModelResolver {
    conn: Option<rusqlite::Connection>,
    cache: HashMap<String, String>,
}

impl ModelResolver {
    fn new() -> Self {
        ModelResolver { conn: open_ro(&cursor_global_storage().join("state.vscdb")).ok(), cache: HashMap::new() }
    }
    fn resolve(&mut self, conv: &str, m: &str) -> String {
        if m != "default" && !m.is_empty() {
            return m.to_string();
        }
        if let Some(x) = self.cache.get(conv) {
            return x.clone();
        }
        let got: Option<String> = self.conn.as_ref().and_then(|c| {
            c.query_row("select json_extract(value,'$.modelConfig.modelName') from cursorDiskKV where key = ?1", [format!("composerData:{conv}")], |r| r.get::<_, Option<String>>(0)).ok().flatten()
        });
        let x = match got.as_deref() {
            Some(v) if v != "default" && !v.is_empty() => v.to_string(),
            _ => "auto".to_string(),
        };
        self.cache.insert(conv.to_string(), x.clone());
        x
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = db_path();
    if !db.exists() {
        return; // 没装 Cursor 不是问题
    }
    let fp = fingerprint(&db);
    let gk = "cursor:ai-code-tracking".to_string();
    if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
        return;
    }
    let all = match rows() {
        Ok(r) => r,
        Err(e) => return st.warnings.push(e),
    };
    let mut res = ModelResolver::new();
    let mut g = FileGroup { fp, ..Default::default() };
    let mut conv: HashMap<String, (i64, i64, String)> = HashMap::new();
    let (mut unmapped, mut total) = (0, 0);
    for r in all {
        if !pm.in_project(&r.file) {
            continue;
        }
        total += 1;
        let Some(rel) = pm.to_rel(&r.file) else {
            unmapped += 1;
            continue;
        };
        let Ok(key) = u32::from_str_radix(&r.hash, 16) else { continue };
        let ts = r.ts_ms / 1000;
        let model = res.resolve(&r.conv, &r.model);
        let e = conv.entry(r.conv.clone()).or_insert((ts, ts, model.clone()));
        e.0 = e.0.min(ts);
        e.1 = e.1.max(ts);
        g.evs.push(Ev { rel, key, src: Src::Cursor, model, session: r.conv, ts, tab: r.source == "tab", next: 0, single_use: false });
    }
    for (id, (start, end, model)) in conv {
        g.sessions.push(Session { src: Src::Cursor, id, model, start, end, tokens: 0, branch: None, prompts: 0, reviewer: false, has_content: true, provider: None });
    }
    st.notes.push(creditia_core::tr!("cursor: 行级记录 {total} 条，无法映射到仓库路径 {unmapped} 条", "cursor: {total} line records, {unmapped} not mapped to a repository path"));
    store.groups.insert(gk, g);
}

/// Cursor 没有 cwd：从文件路径往上找到含 `.git` 的目录（主仓库或 worktree）；找不到的（已删除的临时目录）跳过
pub fn discover() -> Vec<Activity> {
    let Ok(all) = rows() else { return vec![] };
    let mut repo_of: HashMap<String, Option<String>> = HashMap::new();
    let mut convs: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    let mut m: HashMap<String, Activity> = HashMap::new();
    for r in all {
        let Some(dir) = std::path::Path::new(&r.file).parent() else { continue };
        let key = dir.to_string_lossy().to_string();
        let root = repo_of
            .entry(key)
            .or_insert_with(|| {
                let mut d = Some(dir.to_path_buf());
                while let Some(p) = d {
                    if p.join(".git").exists() {
                        return Some(p.to_string_lossy().to_string());
                    }
                    d = p.parent().map(|x| x.to_path_buf());
                }
                None
            })
            .clone();
        let Some(root) = root else { continue };
        convs.entry(root.clone()).or_default().insert(r.conv.clone());
        let e = m.entry(root.clone()).or_insert(Activity { src: Src::Cursor, cwd: root, remote: None, sessions: 0, last: 0 });
        e.last = e.last.max(r.ts_ms / 1000);
    }
    for (k, a) in m.iter_mut() {
        a.sessions = convs.get(k).map(|s| s.len() as u64).unwrap_or(1);
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let db = db_path();
    let mut h = Health { source: "cursor".into(), path: db.display().to_string(), format: "ai-code-tracking.db / ai_code_hashes".into(), ..Default::default() };
    match rows() {
        Ok(r) => {
            h.readable = true;
            h.records = r.len() as u64;
            h.earliest = r.iter().map(|x| x.ts_ms / 1000).min().map(date_cn);
            h.latest = r.iter().map(|x| x.ts_ms / 1000).max().map(date_cn);
            let auto = r.iter().filter(|x| x.model == "default").count();
            let vscdb = cursor_global_storage().join("state.vscdb");
            h.note = if vscdb.exists() { creditia_core::tr!("只保留 30 天；{auto} 条模型为 Auto；对话库可用", "kept for 30 days; {auto} records with model Auto; chat database found") } else { creditia_core::tr!("只保留 30 天；{auto} 条模型为 Auto；对话库未找到", "kept for 30 days; {auto} records with model Auto; chat database not found") };
        }
        Err(e) => h.note = e,
    }
    h
}
