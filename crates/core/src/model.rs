//! 跨 crate 的数据模型：证据、会话、提交、项、归因结果。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ───────────────────────── 证据 ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Src {
    Codex,
    Claude,
    Cursor,
    Grok,
    Kimi,
}

impl Src {
    pub fn harness(self) -> &'static str {
        match self {
            Src::Codex => "codex",
            Src::Claude => "claude-code",
            Src::Cursor => "cursor",
            Src::Grok => "grok",
            Src::Kimi => "kimi",
        }
    }
    pub fn all() -> [Src; 5] {
        [Src::Codex, Src::Claude, Src::Cursor, Src::Grok, Src::Kimi]
    }
}

/// 一条行级证据。只存哈希与元数据，不存代码和 prompt。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ev {
    pub rel: String,
    pub key: u32,
    pub src: Src,
    pub model: String,
    pub session: String,
    pub ts: i64,
    /// Cursor 的 Tab 补全算人的 AI 补全，不算 agent
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

/// 按来源文件分组缓存，文件没变就不重新解析
#[derive(Serialize, Deserialize, Default)]
pub struct FileGroup {
    pub fp: (u64, u64),
    pub evs: Vec<Ev>,
    pub sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    pub version: u32,
    pub groups: HashMap<String, FileGroup>,
}

pub const STORE_VERSION: u32 = 1;

pub fn fingerprint(path: &std::path::Path) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(m) => {
            let mt = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
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

// ───────────────────────── git ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct FileAdd {
    pub rel: String,
    /// 每个新增行的哈希键；0 表示过短或空行（不作证据）
    pub keys: Vec<u32>,
    #[serde(default)]
    pub binary: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Commit {
    pub hash: String,
    pub email: String,
    #[serde(default)]
    pub name: String,
    pub author_ts: i64,
    pub commit_ts: i64,
    pub subject: String,
    #[serde(default)]
    pub coauthors: Vec<String>,
    #[serde(default)]
    pub files: Vec<FileAdd>,
}

/// 一个「项」：PR 或直接提交
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Unit {
    pub id: String,
    pub title: String,
    pub branch: Option<String>,
    pub ts: i64,
    pub commits: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PrMeta {
    pub source: String,
    pub number: u64,
    pub title: String,
    pub branch: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub reviews: Vec<String>,
}

/// HEAD 上存活的一行：来自哪个提交、该提交里的哪一行
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlameLine {
    pub commit: String,
    pub rel: String,
    pub key: u32,
}

// ───────────────────────── 归因 ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Verified,
    Manual,
    Declared,
    Inferred,
    Unattributed,
}

#[derive(Clone, Copy, Debug)]
pub struct Attr {
    pub who: usize,
    pub level: Level,
}

#[derive(Serialize, Clone, Debug)]
pub struct Who {
    pub id: String,
    pub kind: &'static str,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
}

pub const UNATTR: &str = "unattributed";

#[derive(Default)]
pub struct Registry {
    pub list: Vec<Who>,
    by_id: HashMap<String, usize>,
}

/// 不同工具记录的同一模型名不一样：去掉 `provider/` 前缀、统一小写；Claude 系列官方 id 用横线
/// （`anthropic/claude-fable-5.1` → `claude-fable-5-1`），GPT 系列官方名本身带点（`gpt-5.6-sol`），保留
pub fn normalize_model(m: &str) -> String {
    let m = m.trim().to_lowercase();
    let m = m.rsplit('/').next().unwrap_or(&m).to_string();
    if m.starts_with("claude") { m.replace('.', "-") } else { m }
}

impl Registry {
    pub fn agent(&mut self, harness: &str, model: Option<&str>) -> usize {
        let model = model.filter(|m| !m.is_empty()).map(normalize_model);
        let id = match &model {
            Some(m) => format!("a:{harness}/{m}"),
            None => format!("a:{harness}"),
        };
        self.get(id, |id| Who { id, kind: "agent", harness: Some(harness.into()), model: model.clone(), name: None })
    }
    pub fn human(&mut self, id: &str, name: &str) -> usize {
        self.get(id.into(), |id| Who { id, kind: "human", harness: None, model: None, name: Some(name.into()) })
    }
    pub fn unattributed(&mut self) -> usize {
        self.get(UNATTR.into(), |id| Who { id, kind: "unattributed", harness: None, model: None, name: None })
    }
    pub fn find(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }
    fn get(&mut self, id: String, mk: impl FnOnce(String) -> Who) -> usize {
        if let Some(&i) = self.by_id.get(&id) {
            return i;
        }
        self.list.push(mk(id.clone()));
        self.by_id.insert(id, self.list.len() - 1);
        self.list.len() - 1
    }
    /// 展示用的分组：agent 按工具，人按人
    pub fn group(&self, i: usize) -> String {
        let w = &self.list[i];
        match w.kind {
            "agent" => w.harness.clone().unwrap_or_else(|| UNATTR.into()),
            "human" => w.id.clone(),
            _ => UNATTR.into(),
        }
    }
}

/// 单个提交的归因结果
pub struct CommitAttr {
    pub lines: Vec<Vec<Attr>>,
    /// 每个工具的 verified 命中行数（报告与抽样用）
    pub hits: HashMap<&'static str, usize>,
    pub conflicts: usize,
    pub main: Option<usize>,
    pub tab_lines: usize,
    pub reason: String,
}

/// 板块分类：按路径规则（可在 config 里覆盖，二期）
pub fn area(rel: &str, binary: bool) -> &'static str {
    let l = rel.to_lowercase();
    if binary {
        return "assets";
    }
    if l.starts_with("tests/") || l.contains("selfcheck") || l.contains("/tests/") || l.contains("check/") || l.ends_with("tests.swift") {
        "tests"
    } else if l.ends_with(".md") || l.starts_with("docs/") {
        "docs"
    } else if l.starts_with("scripts/") || l.ends_with(".sh") || l.ends_with("package.swift") || l.ends_with(".yml") || l.ends_with(".yaml") || l.starts_with(".github/") {
        "build"
    } else if l.contains("/views/") || l.starts_with("website") || l.contains("/resources/editor/") || l.ends_with(".css") || l.ends_with(".html") || l.contains("theme") || l.starts_with("app/") {
        "frontend"
    } else if l.ends_with(".svg") || l.contains("/assets/") {
        "assets"
    } else {
        "core"
    }
}

/// 默认排除：lockfile、构建产物、压缩产物
pub fn excluded(rel: &str) -> bool {
    let f = rel.rsplit('/').next().unwrap_or(rel);
    matches!(f, "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "Package.resolved" | "Cargo.lock" | "bun.lockb")
        || rel.contains("/dist/")
        || rel.starts_with("dist/")
        || rel.contains("/build/")
        || rel.contains("node_modules/")
        || f.contains(".min.")
}

/// 路径映射：把日志里的绝对路径（主仓库、worktree、/tmp 副本）映射回仓库内相对路径
pub struct PathMap {
    pub known: HashSet<String>,
    pub markers: Vec<String>,
    pub excludes: Vec<String>,
}

impl PathMap {
    /// 关键词形如 `/魏碑/`：末尾补一个 `/` 再匹配，这样目录 `…/魏碑` 和文件 `…/魏碑/src/a.rs` 都能命中，
    /// 而 `…/weibei-launch-film/` 不会被 `/weibei/` 误认
    pub fn in_project(&self, s: &str) -> bool {
        // Windows 日志里的反斜杠先统一成 `/`；远程地址去掉 .git 后缀
        let s = s.replace('\\', "/");
        let s = s.trim_end_matches('/').trim_end_matches(".git");
        let lower = format!("{}/", s.to_lowercase());
        self.markers.iter().any(|m| lower.contains(m.as_str())) && !self.excludes.iter().any(|m| lower.contains(m.as_str()))
    }
    /// 取能在仓库历史路径集合里找到的最长后缀
    pub fn to_rel(&self, abs: &str) -> Option<String> {
        if !self.in_project(abs) {
            return None;
        }
        let abs = abs.replace('\\', "/");
        let parts: Vec<&str> = abs.split('/').filter(|s| !s.is_empty()).collect();
        for i in 0..parts.len() {
            let cand = parts[i..].join("/");
            if self.known.contains(&cand) {
                return Some(cand);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_unify() {
        assert_eq!(normalize_model("anthropic/claude-fable-5.1"), "claude-fable-5-1");
        assert_eq!(normalize_model("claude-fable-5-1"), "claude-fable-5-1");
        assert_eq!(normalize_model("gpt-5.6-sol"), "gpt-5.6-sol");
        assert_eq!(normalize_model("GPT-6"), "gpt-6");
        let mut r = Registry::default();
        let a = r.agent("claude-code", Some("anthropic/claude-fable-5.1"));
        let b = r.agent("claude-code", Some("claude-fable-5-1"));
        assert_eq!(a, b);
        assert_eq!(r.list[a].id, "a:claude-code/claude-fable-5-1");
    }

    #[test]
    fn path_map_boundaries() {
        let pm = PathMap { known: ["src/a.rs".to_string()].into_iter().collect(), markers: vec!["/weibei/".into(), "/weibei-".into()], excludes: vec!["/weibei-launch-film/".into()] };
        assert!(pm.in_project("/Users/x/projects/weibei"));
        assert!(pm.in_project("/private/tmp/weibei-task1/src/a.rs"));
        assert!(pm.in_project("github.com/org/weibei.git"));
        assert!(!pm.in_project("/Users/x/projects/weibei-launch-film/src/a.rs"));
        assert_eq!(pm.to_rel("C:\\Users\\x\\weibei\\src\\a.rs").as_deref(), Some("src/a.rs"));
    }
}
