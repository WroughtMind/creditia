//! 证据库：只存「路径 + 行哈希 + 工具 + 模型 + 会话 + 时间」，不存代码和 prompt。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Src {
    Codex,
    Claude,
    Cursor,
}

impl Src {
    pub fn harness(self) -> &'static str {
        match self {
            Src::Codex => "codex",
            Src::Claude => "claude-code",
            Src::Cursor => "cursor",
        }
    }
}

/// 一条行级证据
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ev {
    pub rel: String,
    pub key: u32,
    pub src: Src,
    pub model: String,
    pub session: String,
    pub ts: i64,
    /// Cursor 的 tab 补全（算人的 AI 补全，不算 agent）
    #[serde(default)]
    pub tab: bool,
}

/// 会话元数据（泳道图、作息图、精力小票用）
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub src: Src,
    pub id: String,
    pub model: String,
    pub start: i64,
    pub end: i64,
    pub tokens: i64,
    pub branch: Option<String>,
    pub prompts: i64,
    /// codex-auto-review 这类只审阅不写代码的会话
    pub reviewer: bool,
    pub has_content: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct FileGroup {
    pub fp: (u64, u64),
    pub evs: Vec<Ev>,
    pub sessions: Vec<Session>,
}

/// 按来源文件分组缓存，文件没变就不重新解析
#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    pub version: u32,
    pub groups: HashMap<String, FileGroup>,
}

pub const STORE_VERSION: u32 = 1;

pub fn fingerprint(path: &std::path::Path) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(m) => {
            let mt = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (mt, m.len())
        }
        Err(_) => (0, 0),
    }
}

/// 匹配用的内存索引
pub struct Index {
    pub map: HashMap<(String, u32), Vec<Ev>>,
}

impl Index {
    pub fn build(store: &Store) -> Index {
        let mut map: HashMap<(String, u32), Vec<Ev>> = HashMap::new();
        for g in store.groups.values() {
            for e in &g.evs {
                map.entry((e.rel.clone(), e.key)).or_default().push(e.clone());
            }
        }
        Index { map }
    }
}
