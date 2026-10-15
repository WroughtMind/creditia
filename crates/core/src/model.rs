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
    Gemini,
    Qwen,
    OpenCode,
    Cline,
    Roo,
    Kilo,
    Aider,
    Copilot,
    Amp,
    Trae,
    Crush,
    Goose,
    Continue,
    Zed,
    AmazonQ,
}

impl Src {
    pub fn harness(self) -> &'static str {
        match self {
            Src::Codex => "codex",
            Src::Claude => "claude-code",
            Src::Cursor => "cursor",
            Src::Grok => "grok",
            Src::Kimi => "kimi",
            Src::Gemini => "gemini",
            Src::Qwen => "qwen",
            Src::OpenCode => "opencode",
            Src::Cline => "cline",
            Src::Roo => "roo",
            Src::Kilo => "kilo",
            Src::Aider => "aider",
            Src::Copilot => "copilot",
            Src::Amp => "amp",
            Src::Trae => "trae",
            Src::Crush => "crush",
            Src::Goose => "goose",
            Src::Continue => "continue",
            Src::Zed => "zed",
            Src::AmazonQ => "amazon-q",
        }
    }
    pub fn all() -> [Src; 20] {
        [Src::Codex, Src::Claude, Src::Cursor, Src::Grok, Src::Kimi, Src::Gemini, Src::Qwen, Src::OpenCode, Src::Cline, Src::Roo, Src::Kilo, Src::Aider, Src::Copilot, Src::Amp, Src::Trae, Src::Crush, Src::Goose, Src::Continue, Src::Zed, Src::AmazonQ]
    }
    pub fn from_harness(h: &str) -> Option<Src> {
        Src::all().into_iter().find(|s| s.harness() == h)
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
    /// 同一次写入里紧跟在它后面的那一行（下一条长行）的键；0 是不知道或它是最后一行。
    /// 证据里本来相邻的两行，提交里中间多出来的行不是这次写入写的
    #[serde(default)]
    pub next: u32,
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
    /// 谁提供的模型：官方、云厂商（bedrock / vertex / azure）、聚合（openrouter）、中转站（relay:<主机>）
    #[serde(default)]
    pub provider: Option<String>,
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

/// 日志缓存的格式版本：adapter 的读法变了就加一，升级后整份重新解析（2：编辑只记新增的行）
pub const STORE_VERSION: u32 = 3;

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
    /// 每个改动块（diff 的 hunk）在 keys 里的起点；空表示整个文件算一块
    #[serde(default)]
    pub hunks: Vec<u32>,
    /// 每个新增行如果过短（keys 里是 0），这里是它本身的哈希（short_hash）；长行是 0
    #[serde(default)]
    pub short: Vec<u32>,
    /// 每个新增行在提交后的文件里是第几行（从 1 起）：存活代码按 blame 的原行号对回来
    #[serde(default)]
    pub at: Vec<u32>,
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
    /// `Assisted-by: 工具:模型` trailer（Crush 默认写这个，Linux 内核的 AI 署名也用这个格式）
    #[serde(default)]
    pub assisted: Vec<String>,
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
#[derive(Clone, Copy, Debug)]
pub struct BlameLine<'a> {
    pub commit: &'a str,
    pub rel: &'a str,
    pub key: u32,
    /// 这一行在引入它的提交里是第几行（blame 的原行号）
    pub orig: u32,
}

/// 一个文件的 blame。提交和路径各存一次，每行只存下标：大仓库有几百万行，逐行存字符串太占内存
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct BlameFile {
    pub commits: Vec<String>,
    pub rels: Vec<String>,
    /// 每一行：[提交下标, 路径下标, 行哈希, 原行号]
    pub lines: Vec<[u32; 4]>,
}

impl BlameFile {
    pub fn len(&self) -> usize {
        self.lines.len()
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<BlameLine<'_>> {
        let l = self.lines.get(i)?;
        Some(BlameLine { commit: &self.commits[l[0] as usize], rel: &self.rels[l[1] as usize], key: l[2], orig: l[3] })
    }
    pub fn iter(&self) -> impl Iterator<Item = BlameLine<'_>> {
        (0..self.lines.len()).filter_map(|i| self.get(i))
    }
    /// 追加一行（提交、路径去重）
    pub fn push(&mut self, commit: &str, rel: &str, key: u32, orig: u32) {
        let ci = match self.commits.iter().rposition(|c| c == commit) {
            Some(i) => i,
            None => {
                self.commits.push(commit.to_string());
                self.commits.len() - 1
            }
        };
        let ri = match self.rels.iter().rposition(|r| r == rel) {
            Some(i) => i,
            None => {
                self.rels.push(rel.to_string());
                self.rels.len() - 1
            }
        };
        self.lines.push([ci as u32, ri as u32, key, orig]);
    }
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

/// 不同工具、不同渠道记录的同一模型名不一样，统一成官方 id：
/// - 去掉渠道前缀：`anthropic/`、`openrouter/anthropic/`、`bedrock/`、`us.anthropic.`、`vertex_ai/`……
/// - 去掉渠道后缀：Bedrock 的 `-v1:0`、Vertex 的 `@20250514`、OpenRouter 的 `:free` / `:thinking`、中转站常见的 `-thinking`
/// - Claude 系列官方 id 用横线（`claude-fable-5.1` → `claude-fable-5-1`），去掉 8 位日期后缀；GPT 系列官方名本身带点（`gpt-5.6-sol`），保留
pub fn normalize_model(m: &str) -> String {
    let mut m = m.trim().to_lowercase();
    // 中转站常见的方括号标签：`[逆向]claude-…`、`【官方】gpt-…`
    while let Some(end) = m.find(']').or_else(|| m.find('】')) {
        if m.starts_with('[') || m.starts_with('【') { m = m[end + m[end..].chars().next().unwrap().len_utf8()..].trim().to_string(); } else { break; }
    }
    m = m.rsplit('/').next().unwrap_or(&m).to_string();
    for p in ["us.anthropic.", "eu.anthropic.", "apac.anthropic.", "global.anthropic.", "anthropic.", "openai.", "google.", "xai.", "moonshotai.", "deepseek."] {
        if let Some(x) = m.strip_prefix(p) { m = x.to_string(); }
    }
    if let Some((a, _)) = m.split_once('@') { m = a.to_string(); }
    if let Some((a, _)) = m.split_once(':') { m = a.to_string(); }
    for suf in ["-v1", "-v2", "-thinking", "-latest"] {
        if let Some(x) = m.strip_suffix(suf) { m = x.to_string(); }
    }
    if m.starts_with("claude") {
        m = m.replace('.', "-");
        // 去掉日期后缀 claude-sonnet-4-5-20250929 → claude-sonnet-4-5
        if let Some((a, d)) = m.rsplit_once('-') { if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) { m = a.to_string(); } }
    }
    m
}

/// 从模型串本身能看出的渠道（看不出就 None，不猜）
pub fn provider_hint(raw: &str) -> Option<String> {
    let l = raw.trim().to_lowercase();
    if l.starts_with("openrouter/") || l.contains(":free") { return Some("openrouter".into()); }
    if l.starts_with("bedrock/") || l.starts_with("us.anthropic.") || l.starts_with("eu.anthropic.") || l.starts_with("apac.anthropic.") || l.starts_with("global.anthropic.") || l.starts_with("anthropic.") || l.ends_with("-v1:0") { return Some("aws-bedrock".into()); }
    if l.starts_with("vertex_ai/") || l.starts_with("vertex/") || (l.starts_with("claude") && l.contains('@')) { return Some("google-vertex".into()); }
    if l.starts_with("azure/") { return Some("azure".into()); }
    None
}

/// 模型出自哪家（不是谁提供的）：用于分组和配色，按名字规则判断，不调用任何服务
pub fn vendor(model: &str) -> &'static str {
    let m = normalize_model(model);
    let has = |s: &str| m.contains(s);
    if m.starts_with("gpt") || has("codex") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") || has("chatgpt") { "openai" }
    else if has("claude") || has("opus") || has("sonnet") || has("fable") || has("haiku") { "anthropic" }
    else if has("gemini") || has("gemma") { "google" }
    else if has("grok") { "xai" }
    else if has("kimi") || m.starts_with("k2") || m.starts_with("k3") || has("moonshot") { "moonshot" }
    else if has("deepseek") { "deepseek" }
    else if has("qwen") || has("qwq") { "alibaba" }
    else if has("glm") || has("zhipu") { "zhipu" }
    else if has("minimax") || m.starts_with("abab") { "minimax" }
    else if has("doubao") || m.starts_with("seed-") { "bytedance" }
    else if has("mistral") || has("codestral") || has("devstral") { "mistral" }
    else if has("llama") { "meta" }
    else if has("composer") || m == "auto" || m == "default" { "cursor" }
    else { "unknown" }
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

/// 板块分类：按通行的目录与文件命名约定（不是关键词猜测），只用于片尾字幕里的分工
pub fn area(rel: &str, binary: bool) -> &'static str {
    let l = rel.to_lowercase();
    let segs: Vec<&str> = l.split('/').collect();
    let file = *segs.last().unwrap_or(&"");
    let dirs = &segs[..segs.len().saturating_sub(1)];
    let ext = file.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let in_dir = |names: &[&str]| dirs.iter().any(|d| names.contains(d));
    if binary || matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "icns" | "svg" | "ttf" | "otf" | "woff" | "woff2" | "mp3" | "mp4" | "wav") {
        return "assets";
    }
    // 测试：测试目录，或者各语言约定的测试文件名
    if in_dir(&["test", "tests", "__tests__", "spec", "specs", "e2e", "testdata", "fixtures"])
        || file.contains(".test.") || file.contains(".spec.") || file.contains("_test.") || file.starts_with("test_") || file.ends_with("tests.swift") || file.ends_with("test.java") || file.ends_with("test.kt")
    {
        return "tests";
    }
    if matches!(ext, "md" | "mdx" | "rst" | "adoc" | "txt") || in_dir(&["docs", "doc"]) {
        return "docs";
    }
    // 构建与发布：CI、容器、脚本、包管理清单
    if segs.first() == Some(&".github") || in_dir(&["scripts", ".circleci", "ci"]) || matches!(ext, "yml" | "yaml" | "toml" | "sh" | "ps1" | "nix" | "gradle" | "cmake" | "mk")
        || matches!(file, "dockerfile" | "makefile" | "justfile" | "package.json" | "cargo.toml" | "pyproject.toml" | "go.mod" | "package.swift" | "podfile" | "gemfile" | "build.rs" | "vite.config.ts" | "tsconfig.json")
    {
        return "build";
    }
    // 界面：前端文件类型，或者常见的界面目录
    if matches!(ext, "css" | "scss" | "sass" | "less" | "html" | "vue" | "svelte" | "astro" | "tsx" | "jsx" | "xib" | "storyboard")
        || in_dir(&["components", "views", "pages", "ui", "styles", "layouts", "screens", "widgets"])
    {
        return "frontend";
    }
    "core"
}

/// 默认排除：lockfile、构建产物、压缩产物
pub fn excluded(rel: &str) -> bool {
    let f = rel.rsplit('/').next().unwrap_or(rel);
    matches!(f, "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "Package.resolved" | "Cargo.lock" | "bun.lockb" | "bun.lock" | "go.sum" | "poetry.lock" | "uv.lock" | "Gemfile.lock" | "composer.lock" | "Podfile.lock" | "pubspec.lock" | "flake.lock")
        || f.ends_with(".pb.go")
        || f.ends_with("_pb2.py")
        || rel.starts_with("vendor/")
        || rel.contains("/vendor/")
        || rel.starts_with("third_party/")
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
    /// 关键词形如 `/demo/`：末尾补一个 `/` 再匹配，这样目录 `…/demo` 和文件 `…/demo/src/a.rs` 都能命中，
    /// 而 `…/demo-film/` 不会被 `/demo/` 误认
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
        assert_eq!(normalize_model("us.anthropic.claude-sonnet-4-5-20250929-v1:0"), "claude-sonnet-4-5");
        assert_eq!(normalize_model("openrouter/anthropic/claude-opus-5.5"), "claude-opus-5-5");
        assert_eq!(normalize_model("claude-opus-4@20250514"), "claude-opus-4");
        assert_eq!(normalize_model("[逆向]claude-fable-5-1-thinking"), "claude-fable-5-1");
        assert_eq!(normalize_model("deepseek/deepseek-chat:free"), "deepseek-chat");
        assert_eq!(provider_hint("us.anthropic.claude-sonnet-4-5-20250929-v1:0").as_deref(), Some("aws-bedrock"));
        assert_eq!(provider_hint("openrouter/x/y").as_deref(), Some("openrouter"));
        assert_eq!(provider_hint("claude-opus-5-5"), None);
        assert_eq!(vendor("kimi-k3"), "moonshot");
        assert_eq!(vendor("qwen3-coder-plus"), "alibaba");
        assert_eq!(vendor("glm-5"), "zhipu");
        let mut r = Registry::default();
        let a = r.agent("claude-code", Some("anthropic/claude-fable-5.1"));
        let b = r.agent("claude-code", Some("claude-fable-5-1"));
        assert_eq!(a, b);
        assert_eq!(r.list[a].id, "a:claude-code/claude-fable-5-1");
    }

    #[test]
    fn path_map_boundaries() {
        let pm = PathMap { known: ["src/a.rs".to_string()].into_iter().collect(), markers: vec!["/demo/".into(), "/demo-".into()], excludes: vec!["/demo-film/".into()] };
        assert!(pm.in_project("/Users/x/projects/demo"));
        assert!(pm.in_project("/private/tmp/demo-task1/src/a.rs"));
        assert!(pm.in_project("github.com/org/demo.git"));
        assert!(!pm.in_project("/Users/x/projects/demo-film/src/a.rs"));
        assert_eq!(pm.to_rel("C:\\Users\\x\\demo\\src\\a.rs").as_deref(), Some("src/a.rs"));
    }
}

#[cfg(test)]
mod area_tests {
    use super::area;
    #[test]
    fn conventions() {
        assert_eq!(area("src/theme.rs", false), "core"); // 文件名里有 theme 不再算界面
        assert_eq!(area("web/components/Button.tsx", false), "frontend");
        assert_eq!(area("pkg/foo_test.go", false), "tests");
        assert_eq!(area("src/a.test.ts", false), "tests");
        assert_eq!(area("tests/e2e.rs", false), "tests");
        assert_eq!(area("README.md", false), "docs");
        assert_eq!(area(".github/workflows/ci.yml", false), "build");
        assert_eq!(area("Dockerfile", false), "build");
        assert_eq!(area("crates/core/src/engine.rs", false), "core");
        assert_eq!(area("assets/logo.svg", false), "assets");
    }
}
