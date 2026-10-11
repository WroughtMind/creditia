//! Cursor：行级记录 `ai-tracking/ai-code-tracking.db`（逐行哈希 → 模型、对话、来源），模型为 `default` 时按 requestId 关联消息中的模型；无法对应时保留 Auto。

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
    request: String,
    ts_ms: i64,
    model: String,
}

fn rows() -> Result<Vec<Row>, String> {
    // 没装 Cursor 或从没用过：不是错误，安静跳过
    if !db_path().exists() {
        return Ok(vec![]);
    }
    let conn = open_ro(&db_path()).map_err(|e| format!("cursor: 行级记录不可读 {e}"))?;
    let mut stmt = conn
        .prepare("select hash, source, coalesce(fileName,''), coalesce(conversationId,''), coalesce(timestamp,createdAt), coalesce(model,''), coalesce(requestId,'') from ai_code_hashes")
        .map_err(|e| format!("cursor: ai_code_hashes 表结构不认识 {e}"))?;
    let r = stmt
        .query_map([], |r| Ok(Row { hash: r.get(0)?, source: r.get(1)?, file: r.get(2)?, conv: r.get(3)?, ts_ms: r.get(4)?, model: r.get(5)?, request: r.get(6)? }))
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(r)
}

/// 对话中可能切换模型，只有同一个请求的消息才用于补齐行级模型。
struct ModelResolver {
    conn: Option<rusqlite::Connection>,
    cache: HashMap<String, HashMap<String, std::collections::HashSet<String>>>,
}
impl ModelResolver {
    fn new() -> Self { Self { conn: open_ro(&cursor_global_storage().join("state.vscdb")).ok(), cache: HashMap::new() } }
    fn resolve(&mut self, conv: &str, request: &str, m: &str) -> String {
        if !matches!(m,"default"|"auto"|""){return m.into()}
        let requests=self.cache.entry(conv.into()).or_insert_with(|| {
            let mut result:HashMap<String,std::collections::HashSet<String>>=HashMap::new();
            if let Some(c)=&self.conn { if let Ok(mut q)=c.prepare("select json_extract(value,'$.requestId'),json_extract(value,'$.modelInfo.modelName') from cursorDiskKV where key like ?1") {
                if let Ok(rows)=q.query_map([format!("bubbleId:{conv}:%")], |r|Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?))) {
                    for (request,model) in rows.flatten(){if let (Some(request),Some(model))=(request,model){if !matches!(model.as_str(),"default"|"auto"|""){result.entry(request).or_default().insert(model);}}}
                }
            }}result
        });
        requests.get(request).filter(|models|models.len()==1).and_then(|models|models.iter().next()).cloned().unwrap_or_else(||"auto".into())
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = db_path();
    let fp = fingerprint(&db);
    let gk = "cursor:ai-code-tracking".to_string();
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
        let model = res.resolve(&r.conv, &r.request, &r.model);
        let e = conv.entry(r.conv.clone()).or_insert((ts, ts, model.clone()));
        e.0 = e.0.min(ts);
        e.1 = e.1.max(ts);
        if e.2 != model { e.2.clear(); }
        g.evs.push(Ev { provider: None, rel, key, src: Src::Cursor, model, session: r.conv, ts, tab: r.source == "tab" });
    }
    for (id, (start, end, model)) in conv {
        g.sessions.push(Session { provider: None, src: Src::Cursor, id, model, start, end, tokens: 0, branch: None, prompts: 0, reviewer: false, has_content: true });
    }
    st.notes.push(format!("cursor: 行级记录 {total} 条，无法映射到仓库路径 {unmapped} 条"));
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
            h.readable = db.exists();
            h.records = r.len() as u64;
            h.earliest = r.iter().map(|x| x.ts_ms / 1000).min().map(date_cn);
            h.latest = r.iter().map(|x| x.ts_ms / 1000).max().map(date_cn);
            let auto = r.iter().filter(|x| x.model == "default").count();
            let vscdb = cursor_global_storage().join("state.vscdb");
            h.note = format!("{auto} 条模型为 Auto；对话库 {}", if vscdb.exists() { "可用" } else { "未找到" });
        }
        Err(e) => h.note = e,
    }
    h
}
