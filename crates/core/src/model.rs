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
    Antigravity,
    Zcode,
    Pi,
    Omp,
    CommandCode,
    Muse,
    T3,
    Vibe,
    MiniMax,
    Droid,
    Auggie,
    Qoder,
    CodeBuddy,
    Junie,
    Kiro,
    Iflow,
    Neovate,
    Costrict,
    CodeFlicker,
    Tabnine,
    Comate,
    Devin,
    Jules,
    OpenHands,
    SweAgent,
    MiniSweAgent,
    Replit,
    Bolt,
    Lovable,
    V0,
    Warp,
    Windsurf,
    TraeIde,
    AntigravityIde,
    Dsh,
    Cortex,
    Bob,
    Poolside,
    Autohand,
    VtCode,
    Kimchi,
    Nova,
    GlmAgent,
    Dirac,
    Corust,
    Crow,
    DimCode,
    MinionCode,
    SiGit,
    Stakpak,
    AiderDesk,
    Astrbot,
    Openclaw,
    CodeartsAgent,
    Codemaker,
    Codestudio,
    Deepagents,
    Dexto,
    Eve,
    Firebender,
    Forgecode,
    Fx,
    HermesAgent,
    InferenceSh,
    Jazz,
    Kode,
    Lingma,
    Loaf,
    Mcpjam,
    Moxby,
    Mux,
    Ona,
    PositAssistant,
    Reasonix,
    Rovodev,
    SarvamCode,
    Terramind,
    Tinycloud,
    Zencoder,
    Zenflow,
    Pochi,
    Promptscript,
    Adal,
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
            Src::Antigravity => "antigravity",
            Src::Zcode => "zcode",
            Src::Pi => "pi",
            Src::Omp => "omp",
            Src::CommandCode => "command-code",
            Src::Muse => "muse",
            Src::T3 => "t3",
            Src::Vibe => "mistral-vibe",
            Src::MiniMax => "minimax-code",
            Src::Droid => "droid",
            Src::Auggie => "auggie",
            Src::Qoder => "qoder",
            Src::CodeBuddy => "codebuddy",
            Src::Junie => "junie",
            Src::Kiro => "kiro",
            Src::Iflow => "iflow",
            Src::Neovate => "neovate",
            Src::Costrict => "costrict",
            Src::CodeFlicker => "codeflicker",
            Src::Tabnine => "tabnine",
            Src::Comate => "comate",
            Src::Devin => "devin",
            Src::Jules => "jules",
            Src::OpenHands => "openhands",
            Src::SweAgent => "swe-agent",
            Src::MiniSweAgent => "mini-swe-agent",
            Src::Replit => "replit",
            Src::Bolt => "bolt",
            Src::Lovable => "lovable",
            Src::V0 => "v0",
            Src::Warp => "warp",
            Src::Windsurf => "windsurf",
            Src::TraeIde => "trae-ide",
            Src::AntigravityIde => "antigravity-ide",
            Src::Dsh => "dsh",
            Src::Cortex => "cortex-code",
            Src::Bob => "bob",
            Src::Poolside => "poolside",
            Src::Autohand => "autohand",
            Src::VtCode => "vtcode",
            Src::Kimchi => "kimchi",
            Src::Nova => "nova",
            Src::GlmAgent => "glm-agent",
            Src::Dirac => "dirac",
            Src::Corust => "corust",
            Src::Crow => "crow-cli",
            Src::DimCode => "dimcode",
            Src::MinionCode => "minion-code",
            Src::SiGit => "sigit",
            Src::Stakpak => "stakpak",
            Src::AiderDesk => "aider-desk",
            Src::Astrbot => "astrbot",
            Src::Openclaw => "openclaw",
            Src::CodeartsAgent => "codearts-agent",
            Src::Codemaker => "codemaker",
            Src::Codestudio => "codestudio",
            Src::Deepagents => "deepagents",
            Src::Dexto => "dexto",
            Src::Eve => "eve",
            Src::Firebender => "firebender",
            Src::Forgecode => "forgecode",
            Src::Fx => "fx",
            Src::HermesAgent => "hermes-agent",
            Src::InferenceSh => "inference-sh",
            Src::Jazz => "jazz",
            Src::Kode => "kode",
            Src::Lingma => "lingma",
            Src::Loaf => "loaf",
            Src::Mcpjam => "mcpjam",
            Src::Moxby => "moxby",
            Src::Mux => "mux",
            Src::Ona => "ona",
            Src::PositAssistant => "posit-assistant",
            Src::Reasonix => "reasonix",
            Src::Rovodev => "rovodev",
            Src::SarvamCode => "sarvam-code",
            Src::Terramind => "terramind",
            Src::Tinycloud => "tinycloud",
            Src::Zencoder => "zencoder",
            Src::Zenflow => "zenflow",
            Src::Pochi => "pochi",
            Src::Promptscript => "promptscript",
            Src::Adal => "adal",
        }
    }
    pub fn all() -> &'static [Src] {
        &[Src::Codex, Src::Claude, Src::Cursor, Src::Grok, Src::Kimi, Src::Gemini, Src::Qwen, Src::OpenCode, Src::Cline, Src::Roo, Src::Kilo, Src::Aider, Src::Copilot, Src::Amp, Src::Trae, Src::Crush, Src::Goose, Src::Continue, Src::Zed, Src::AmazonQ, Src::Antigravity,
          Src::Zcode, Src::Pi, Src::Omp, Src::CommandCode, Src::Muse, Src::T3, Src::Vibe, Src::MiniMax,
          Src::Droid, Src::Auggie, Src::Qoder, Src::CodeBuddy, Src::Junie, Src::Kiro, Src::Iflow, Src::Neovate, Src::Costrict, Src::CodeFlicker, Src::Tabnine, Src::Comate,
          Src::Devin, Src::Jules, Src::OpenHands, Src::SweAgent, Src::MiniSweAgent, Src::Replit, Src::Bolt, Src::Lovable, Src::V0, Src::Warp, Src::Windsurf, Src::TraeIde, Src::AntigravityIde,
          Src::Dsh, Src::Cortex, Src::Bob, Src::Poolside, Src::Autohand, Src::VtCode, Src::Kimchi, Src::Nova, Src::GlmAgent, Src::Dirac, Src::Corust, Src::Crow, Src::DimCode, Src::MinionCode, Src::SiGit, Src::Stakpak, Src::AiderDesk, Src::Astrbot, Src::Openclaw, Src::CodeartsAgent, Src::Codemaker, Src::Codestudio, Src::Deepagents, Src::Dexto, Src::Eve, Src::Firebender, Src::Forgecode, Src::Fx, Src::HermesAgent, Src::InferenceSh, Src::Jazz, Src::Kode, Src::Lingma, Src::Loaf, Src::Mcpjam, Src::Moxby, Src::Mux, Src::Ona, Src::PositAssistant, Src::Reasonix, Src::Rovodev, Src::SarvamCode, Src::Terramind, Src::Tinycloud, Src::Zencoder, Src::Zenflow, Src::Pochi, Src::Promptscript, Src::Adal]
    }
    pub fn from_harness(h: &str) -> Option<Src> {
        Src::all().iter().copied().find(|s| s.harness() == h)
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
    /// A successful tool call records one occurrence. Cursor's editor tracking
    /// table is a persistent line set instead, so those rows remain reusable.
    #[serde(default)]
    pub single_use: bool,
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
    /// The available native records authoritatively rebuild these edit sessions,
    /// including an empty result after a failed, conflicting or reverted edit.
    /// Missing/unreadable history does not assert reconciliation. The library
    /// synchronizes only these sessions and preserves other historical evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reconciled_sessions: Vec<(Src, String)>,
    /// Observed native event identities, ignoring their old model value. Replace
    /// only these rows with matching `evs`; an observation with no matching edit
    /// retracts it. Unlike whole-session reconciliation, absent rows survive.
    /// Only use with a native unique identity (Cursor's line hash), not merely
    /// equal content and second-level timestamps shared by different calls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reconciled_events: Vec<Ev>,
    /// Native snapshot progress also survives an empty/retracted session, which
    /// need not appear in this project's user-facing session list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_revisions: Vec<(Src, String, i64)>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    pub version: u32,
    pub groups: HashMap<String, FileGroup>,
    /// 上次扫描时认项目用的关键词（含各个根目录）。变了（新登记了 worktree、克隆、手动关联的路径），
    /// 之前因为认不出而跳过的日志要重新读，不能因为文件没变就一直用空结果
    #[serde(default)]
    pub markers: Vec<String>,
    /// Confirmed host ownership outlives a host's mutable session index.
    /// Keys retain the engine namespace, e.g. `opencode:ses_…`.
    #[serde(default)]
    pub session_owners: HashMap<String, Src>,
}

impl Store {
    /// 认项目的关键词变了：没读出任何证据的日志分组作废重读（读出了证据的照旧用缓存）
    pub fn rescan_if_markers_changed(&mut self, markers: &[String]) {
        let mut m = markers.to_vec();
        m.sort();
        m.dedup();
        if self.markers != m {
            for g in self.groups.values_mut() {
                if g.evs.is_empty() {
                    g.fp = (0, 0);
                }
            }
            self.markers = m;
        }
    }
}

/// 日志缓存的格式版本：adapter 的读法变了就加一，升级后整份重新解析（2：编辑只记新增的行）
pub const STORE_VERSION: u32 = 6;

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
    /// 格式化拆行用：第 i 行起连续 2、3、4 行（同一改动块内）规范化后拼起来的键，依次放在 `joins[i * 3 ..]`；0 是没有。
    /// agent 写的一长行被格式化工具拆成几行，拼回去就和证据对得上
    #[serde(default)]
    pub joins: Vec<u32>,
}

/// 拆行识别看的窗口：连续 2 到 4 行
pub const JOIN_WINDOWS: usize = 3;

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
    /// 提交盖章 hook 写的 `Creditia-Lines`：提交时按本机日志算出的（工具/模型, 行数）和新增总行数。
    /// 有它的提交，章只认这些行数，其余的行提交时已经核对过日志、没有 AI 证据
    #[serde(default)]
    pub lines_stamp: Option<(Vec<(String, usize)>, usize)>,
    /// 在 GitHub 网页上合并（提交者是 GitHub）：作者只是点了合并、或者开 PR 的人，不说明代码是谁写的
    #[serde(default)]
    pub web: bool,
    /// 作者写提交时的时区偏移（秒，git 的 `%ai`）：按天、按周、作息按提交者当地的时间算
    #[serde(default)]
    pub tz: i64,
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

/// 不同工具、不同渠道记录的同一模型名不一样，统一成官方 id。规则按「它是不是同一个模型」来定，而不是按字符串长得像不像：
/// - 渠道：`anthropic/`、`openrouter/anthropic/`、`bedrock/`、`us.anthropic.`、`vertex_ai/`、`models/`……只留最后一段；
///   Bedrock 的 `-v1:0`、Vertex 的 `@20250514`、OpenRouter 的 `:free` / `:thinking` / `:nitro` 去掉
/// - 快照与预览：日期（`-20250929`、`-2024-08-06`、`-0709`、`-2507`）、`-latest`、`-preview` / `-exp` 去掉
/// - 界面写法：`Claude Sonnet 4.5`、`Gemini 3 Pro (High)`、`CLAUDE_SONNET_4_20250514_V1_0`（空格、下划线、括号标签）
/// - 同一模型的调用档位不是另一个模型：OpenAI 系的推理强度（`-high`、`-xhigh`、Cursor 的 `-fast`）、
///   Claude 的 `-thinking`、Cursor MAX 模式的 `-max`、`[1m]` 上下文、`-instruct`
/// - 真不同的模型保留：`kimi-k2-thinking`、`gpt-5.1-codex-max`、本地模型的尺寸（`gpt-oss:20b` → `gpt-oss-20b`）
/// - Claude 用官方词序：4 代起 `claude-<系列>-<版本>`（Cursor 的 `claude-4.5-sonnet` → `claude-sonnet-4-5`），
///   3 代及以前 `claude-<版本>-<系列>`；版本用横线。GPT 系列官方名本身带点（`gpt-5.6-sol`），保留
pub fn normalize_model(m: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;
    struct Re { tags: Regex, sep: Regex, size: Regex, dated: Regex, effort: Regex, gpt_join: Regex, gpt_dash: Regex }
    static RE: OnceLock<Re> = OnceLock::new();
    let re = RE.get_or_init(|| Re {
        // 中转站、界面的括号标签：`[逆向]`、`【官方】`、`[1m]`、`(High)`、`(1M context)`
        tags: Regex::new(r"\[[^\]]*\]|【[^】]*】|\([^)]*\)|（[^）]*）").unwrap(),
        sep: Regex::new(r"[\s_]+").unwrap(),
        size: Regex::new(r"^\d+(\.\d+)?[bmk](-a\d+(\.\d+)?b)?$").unwrap(),
        // 日期快照：8 位、YYYY-MM-DD、MM-DD（预览版）、4 位 MMDD / YYMM
        dated: Regex::new(r"-(\d{8}|\d{4}-\d{2}-\d{2}|\d{2}-\d{2}|\d{4})$").unwrap(),
        effort: Regex::new(r"-(minimal|low|medium|high|xhigh|fast)$").unwrap(),
        gpt_join: Regex::new(r"^gpt(\d)").unwrap(),
        // `gpt-5-1-codex` → `gpt-5.1-codex`（官方写法是点）
        gpt_dash: Regex::new(r"^gpt-(\d+)-(\d)(-|$)").unwrap(),
    });
    let mut m = re.tags.replace_all(m.trim(), " ").to_lowercase();
    m = m.trim().rsplit('/').next().unwrap_or("").trim().to_string();
    for p in ["us.", "eu.", "apac.", "global.", "jp.", "au."] {
        if let Some(x) = m.strip_prefix(p).filter(|x| x.contains('.')) {
            m = x.to_string();
        }
    }
    // Bedrock 的 `厂商.模型-版本:修订`（`anthropic.claude-…-v1:0`、`openai.gpt-oss-120b-1:0`）
    let mut bedrock = false;
    for p in ["anthropic.", "openai.", "google.", "xai.", "moonshotai.", "moonshot.", "deepseek.", "meta.", "mistral.", "qwen.", "amazon."] {
        if let Some(x) = m.strip_prefix(p) {
            m = x.to_string();
            bedrock = true;
        }
    }
    if let Some((a, _)) = m.split_once('@') {
        m = a.to_string();
    }
    // `:20b` 这类尺寸是不同的模型，接回名字里；其余（`:free`、`:latest`、Bedrock 的 `:0`）是渠道标签
    if let Some((a, tag)) = m.clone().split_once(':') {
        m = if re.size.is_match(tag.trim()) && !a.ends_with(tag.trim()) { format!("{}-{}", a.trim(), tag.trim()) } else { a.trim().to_string() };
        if bedrock && tag.chars().all(|c| c.is_ascii_digit()) {
            if let Some((b, v)) = m.rsplit_once('-') {
                let n = v.trim_start_matches('v');
                if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                    m = b.to_string();
                }
            }
        }
    }
    m = re.sep.replace_all(&m, "-").to_string();
    while m.contains("--") {
        m = m.replace("--", "-");
    }
    m = m.trim_matches('-').to_string();
    // These unversioned Gemini experiments have no stable family to collapse to.
    // Dropping their experiment ID would merge distinct models into just "gemini".
    if m.starts_with("gemini-exp-") {
        return m;
    }
    let openai = |m: &str| m.starts_with("gpt") || m.starts_with("chatgpt") || (m.starts_with('o') && m[1..].starts_with(|c: char| c.is_ascii_digit())) || m.starts_with("codex-");
    let claude = |m: &str| m.starts_with("claude");
    // 后缀反复剥，直到不再变化：`claude-sonnet-4-5-20250929-v1-0`、`gemini-2.5-pro-preview-05-06`
    loop {
        let before = m.clone();
        for suf in ["-v1-0", "-v2-0", "-v1", "-v2", "-latest", "-preview", "-exp", "-experimental", "-instruct", "-free"] {
            if let Some(x) = m.strip_suffix(suf).filter(|x| !x.is_empty()) {
                m = x.to_string();
            }
        }
        if let Some(c) = re.dated.captures(&m) {
            // 只剥像日期的四位数：月份 01-12 开头的 MMDD，或 23-29 开头的 YYMM；`-1106` 这类老快照也算
            let d = &c[1];
            let looks_dated = d.len() != 4 || matches!(&d[..2], "01" | "02" | "03" | "04" | "05" | "06" | "07" | "08" | "09" | "10" | "11" | "12" | "23" | "24" | "25" | "26" | "27" | "28" | "29");
            if looks_dated && c.get(0).unwrap().start() > 0 {
                m.truncate(c.get(0).unwrap().start());
            }
        }
        if openai(&m) {
            if let Some(c) = re.effort.find(&m) {
                m.truncate(c.start());
            }
        }
        if openai(&m) || claude(&m) {
            for suf in ["-thinking", "-reasoning"] {
                if let Some(x) = m.strip_suffix(suf) {
                    m = x.to_string();
                }
            }
        }
        if claude(&m) {
            for suf in ["-max", "-1m"] {
                if let Some(x) = m.strip_suffix(suf) {
                    m = x.to_string();
                }
            }
        }
        if m == before {
            break;
        }
    }
    if openai(&m) {
        m = re.gpt_join.replace(&m, "gpt-$1").to_string();
        m = re.gpt_dash.replace(&m, "gpt-$1.$2$3").to_string();
    }
    if claude(&m) {
        m = claude_id(&m);
    }
    m
}

/// Claude 的官方词序：4 代起 `claude-<系列>-<版本>`，3 代及以前 `claude-<版本>-<系列>`；版本号一律横线
fn claude_id(m: &str) -> String {
    let rest = m.strip_prefix("claude").unwrap_or(m).trim_start_matches('-');
    let mut ver: Vec<String> = vec![];
    let mut words: Vec<String> = vec![];
    for t in rest.split(['-', '.']).filter(|t| !t.is_empty()) {
        if t.chars().all(|c| c.is_ascii_digit()) {
            ver.push(t.to_string());
        } else {
            words.push(t.to_string());
        }
    }
    let major: u32 = ver.first().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut out = vec!["claude".to_string()];
    // 第一个词是系列（opus、sonnet、haiku、fable……），其余的词跟在版本后面
    let (family, extra) = if words.is_empty() { (vec![], vec![]) } else { (words[..1].to_vec(), words[1..].to_vec()) };
    if major == 3 {
        out.extend(ver);
        out.extend(family);
    } else {
        out.extend(family);
        out.extend(ver);
    }
    out.extend(extra);
    out.join("-")
}

/// 工具名的别名：盖章、`Assisted-by`、命令行参数里写法五花八门，统一成 creditia 的工具 id
pub fn canonical_harness(h: &str) -> String {
    let k = h.trim().to_lowercase().split(|c: char| c.is_whitespace() || c == '_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    if let Some(harness) = crate::agents::canonical_harness(&k) {
        return harness.to_string();
    }
    let k = k.as_str();
    match k {
        "claude" | "claude-code" | "claudecode" | "anthropic-claude" | "anthropic-claude-code" | "claude-code-cli" => "claude-code",
        "codex" | "openai-codex" | "codex-cli" | "chatgpt-codex" => "codex",
        "cursor" | "cursor-agent" | "cursoragent" | "cursor-composer" | "composer" => "cursor",
        "copilot" | "copilot-cli" | "github-copilot" | "gh-copilot" | "copilot-chat" | "github-copilot-chat" | "copilot-coding" => "copilot",
        "gemini" | "gemini-cli" | "google-gemini" | "google-gemini-cli" => "gemini",
        "qwen" | "qwen-code" | "qwencode" => "qwen",
        "roo" | "roo-code" | "roocode" | "roo-cline" => "roo",
        "kilo" | "kilo-code" | "kilocode" => "kilo",
        "opencode" | "open-code" | "sst-opencode" => "opencode",
        "amp" | "sourcegraph-amp" | "ampcode" => "amp",
        "trae" | "trae-ai" | "traecli" => "trae",
        "crush" | "charm-crush" => "crush",
        "goose" | "block-goose" => "goose",
        "continue" | "continue-dev" => "continue",
        "zed" | "zed-ai" => "zed",
        "amazon-q" | "amazonq" | "q" | "q-developer" | "amazon-q-developer" | "aws-q" => "amazon-q",
        "agy" | "antigravity" | "google-antigravity" => "antigravity",
        "grok" | "xai-grok" | "grok-code" => "grok",
        "kimi" | "kimi-code" | "moonshot-kimi" => "kimi",
        "windsurf" | "cascade" | "codeium" => "windsurf",
        "devin" | "devin-ai" => "devin",
        "jules" | "google-jules" => "jules",
        other => other,
    }
    .to_string()
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
        let harness = canonical_harness(harness);
        let harness = harness.as_str();
        let model = model.map(normalize_model).filter(|m| !m.is_empty());
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
    /// 带模型的分组：agent 用贡献者 id（工具 + 模型），人按人；按天、按文件的统计用它，渲染时再按工具或模型归并
    pub fn fine(&self, i: usize) -> String {
        let w = &self.list[i];
        match w.kind {
            "agent" | "human" => w.id.clone(),
            _ => UNATTR.into(),
        }
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
    /// 取能在仓库路径集合里找到的最长后缀；找不到时，路径在某个已知根目录（主仓库、worktree、登记过的克隆）下面，
    /// 就按根目录切出相对路径：agent 新建、还没提交的文件也认得出，提交以后才不会丢
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
        let lower = abs.to_lowercase();
        let root = self.markers.iter().filter(|m| m.len() > 4 && lower.starts_with(m.as_str())).max_by_key(|m| m.len())?;
        // 小写不改变 ASCII 的字节长度；含非 ASCII 大小写变化时长度可能不同，退回按字符数切
        let rest = if lower.len() == abs.len() { abs.get(root.len()..)? } else { &abs[abs.char_indices().nth(root.chars().count())?.0..] };
        let rel = rest.trim_matches('/');
        (!rel.is_empty() && !rel.split('/').any(|s| s == "..")).then(|| rel.to_string())
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
        // 工具名别名
        let c = r.agent("Claude Code", Some("claude-fable-5-1"));
        assert_eq!(c, a);
    }

    /// 同一个模型在各工具、渠道、界面里的写法：都要归到同一个 id
    #[test]
    fn same_model_everywhere() {
        let groups: &[(&str, &[&str])] = &[
            ("claude-sonnet-4-5", &[
                "claude-sonnet-4-5", "claude-sonnet-4-5-20250929", "claude-sonnet-4.5", "copilot/claude-sonnet-4.5",
                "anthropic/claude-sonnet-4.5", "openrouter/anthropic/claude-sonnet-4.5", "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
                "global.anthropic.claude-sonnet-4-5-20250929-v1:0", "claude-sonnet-4-5@20250929", "vertex_ai/claude-sonnet-4-5@20250929",
                "claude-4.5-sonnet", "claude-4.5-sonnet-thinking", "Claude Sonnet 4.5", "Claude Sonnet 4.5 (Thinking)",
                "claude-sonnet-4-5-thinking", "[逆向]claude-sonnet-4-5-thinking", "【官方】claude-sonnet-4.5", "claude-sonnet-4.5:thinking",
                "CLAUDE_SONNET_4_5_20250929_V1_0", "claude-sonnet-4-5-latest",
            ]),
            ("claude-opus-4-6", &["claude-opus-4-6", "claude-opus-4-6[1m]", "Claude Opus 4.6 (1M context)", "claude-4.6-opus-max", "claude-opus-4-6-1m"]),
            ("claude-sonnet-4", &["claude-sonnet-4", "claude-4-sonnet", "claude-sonnet-4-20250514", "CLAUDE_SONNET_4_20250514_V1_0", "claude-4-sonnet-thinking"]),
            ("claude-3-7-sonnet", &["claude-3-7-sonnet", "claude-3.7-sonnet", "claude-3-7-sonnet-20250219", "claude-3.7-sonnet-max", "claude-sonnet-3.7", "anthropic.claude-3-7-sonnet-20250219-v1:0"]),
            ("claude-3-5-sonnet", &["claude-3-5-sonnet-20241022", "claude-3.5-sonnet", "anthropic.claude-3-5-sonnet-20241022-v2:0", "claude-3-5-sonnet-latest"]),
            // Copilot session-state: microsoft/vscode#338949, comment 6099460320.
            ("claude-haiku-4-5", &["claude-haiku-4-5", "claude-haiku-4.5", "claude-haiku-4-5-20251001", "claude-4.5-haiku", "Claude Haiku 4.5"]),
            ("gpt-6-luna", &["gpt-6-luna"]),
            // Kimi's real startup log: MoonshotAI/kimi-cli#290 uses openai/gpt-5.1-codex.
            ("gpt-5.1-codex", &["gpt-5.1-codex", "openai/gpt-5.1-codex", "gpt-5.1-codex-high", "gpt-5.1-codex-medium", "GPT-5.1 Codex", "gpt-5-1-codex", "gpt5.1-codex"]),
            ("gpt-5.1-codex-max", &["gpt-5.1-codex-max", "gpt-5.1-codex-max-xhigh", "GPT-5.1-Codex-Max"]),
            ("gpt-5", &["gpt-5", "gpt-5-high", "gpt-5-high-fast", "gpt-5-2025-08-07", "openai/gpt-5", "azure/gpt-5", "gpt-5-thinking", "GPT-5"]),
            ("gpt-4.1", &["gpt-4.1", "gpt-4-1", "gpt-4.1-2025-04-14"]),
            ("o4-mini", &["o4-mini", "o4-mini-high", "o4-mini-2025-04-16"]),
            ("gemini-2.5-pro", &["gemini-2.5-pro", "models/gemini-2.5-pro", "google/gemini-2.5-pro", "gemini-2.5-pro-preview-05-06", "gemini-2.5-pro-exp-03-25", "vertex_ai/gemini-2.5-pro", "Gemini 2.5 Pro"]),
            ("gemini-3-pro", &["gemini-3-pro", "gemini-3-pro-preview", "Gemini 3 Pro (High)", "Gemini 3 Pro (Low)"]),
            // Real Continue promptLogs: continuedev/continue#2085, comment 2687146546.
            ("gemini-exp-1206", &["gemini-exp-1206", "Gemini Exp 1206", "models/gemini-exp-1206"]),
            ("grok-4", &["grok-4", "grok-4-0709", "x-ai/grok-4", "xai/grok-4"]),
            ("grok-code-fast-1", &["grok-code-fast-1", "x-ai/grok-code-fast-1"]),
            ("kimi-k2", &["kimi-k2", "kimi-k2-0905", "kimi-k2-0905-preview", "moonshotai/kimi-k2", "moonshotai/kimi-k2-instruct", "kimi-k2-instruct-0905"]),
            ("kimi-k2-thinking", &["kimi-k2-thinking", "moonshotai/kimi-k2-thinking"]),
            ("deepseek-v3", &["deepseek-v3", "deepseek-v3-0324", "deepseek/deepseek-v3", "deepseek-ai/deepseek-v3"]),
            ("qwen3-coder-plus", &["qwen3-coder-plus", "qwen/qwen3-coder-plus", "qwen3-coder-plus-2025-07-22"]),
            ("glm-4.6", &["glm-4.6", "z-ai/glm-4.6", "zhipuai/glm-4.6"]),
            ("gpt-oss-120b", &["gpt-oss-120b", "gpt-oss:120b", "openai/gpt-oss-120b", "openai.gpt-oss-120b-1:0"]),
            ("mistral-large", &["mistral-large-2411", "mistral-large-latest", "mistral/mistral-large-latest"]),
        ];
        for (want, names) in groups {
            for n in *names {
                assert_eq!(normalize_model(n), *want, "{n}");
            }
        }
        // 真不同的模型：不能并在一起
        let distinct = [
            ("kimi-k2-thinking", "kimi-k2"), ("gpt-oss:20b", "gpt-oss:120b"), ("qwen3-coder:30b", "qwen3-coder:480b"),
            ("gpt-5.1-codex-max", "gpt-5.1-codex"), ("gpt-5-mini", "gpt-5"), ("o3-mini", "o3"), ("claude-opus-4-1", "claude-opus-4"),
            ("claude-sonnet-4-5", "claude-sonnet-4"), ("gemini-2.5-flash", "gemini-2.5-pro"), ("gemini-2.0-flash-thinking-exp", "gemini-2.0-flash"),
            ("grok-4-fast", "grok-4"), ("deepseek-reasoner", "deepseek-chat"), ("composer-1", "composer-2"),
            ("gemini-exp-1206", "gemini-exp-1121"),
        ];
        for (a, b) in distinct {
            assert_ne!(normalize_model(a), normalize_model(b), "{a} vs {b}");
        }
        assert_eq!(normalize_model("qwen3-coder:30b"), "qwen3-coder-30b");
        assert_eq!(normalize_model("llama3.1:latest"), "llama3.1");
        assert_eq!(normalize_model("claude-instant-1.2"), "claude-instant-1-2");
        assert_eq!(normalize_model("gpt-5.6-sol"), "gpt-5.6-sol");
        assert_eq!(normalize_model("auto"), "auto");
        assert_eq!(normalize_model(""), "");
    }

    #[test]
    fn canonical_agent_ids_keep_session_and_registry_identity() {
        for src in Src::all() {
            let harness = src.harness();
            assert_eq!(canonical_harness(harness), harness);
            let mut reg = Registry::default();
            let who = reg.agent(harness, Some("m"));
            assert_eq!(reg.list[who].id, format!("a:{harness}/m"));
        }
        assert_eq!(canonical_harness("my-own-agent"), "my-own-agent");
        assert_eq!(canonical_harness("my-own-cli"), "my-own-cli");
    }

    #[test]
    fn harness_aliases() {
        for (raw, want) in [
            ("Claude Code", "claude-code"), ("claude", "claude-code"), ("claude_code", "claude-code"), ("openai-codex", "codex"),
            ("Codex CLI", "codex"), ("cursor-agent", "cursor"), ("GitHub Copilot", "copilot"), ("copilot-cli", "copilot"),
            ("Gemini CLI", "gemini"), ("Qwen Code", "qwen"), ("Roo Code", "roo"), ("Kilo Code", "kilo"), ("Amazon Q", "amazon-q"),
            ("q", "amazon-q"), ("agy", "antigravity"), ("Antigravity CLI", "antigravity"), ("cascade", "windsurf"), ("my-own-bot", "my-own-bot"),
        ] {
            assert_eq!(canonical_harness(raw), want, "{raw}");
        }
    }

    #[test]
    fn path_map_boundaries() {
        let pm = PathMap { known: ["src/a.rs".to_string()].into_iter().collect(), markers: vec!["/demo/".into(), "/demo-".into()], excludes: vec!["/demo-film/".into()] };
        assert!(pm.in_project("/Users/x/projects/demo"));
        assert!(pm.in_project("/private/tmp/demo-task1/src/a.rs"));
        assert!(pm.in_project("github.com/org/demo.git"));
        assert!(!pm.in_project("/Users/x/projects/demo-film/src/a.rs"));
        assert_eq!(pm.to_rel("C:\\Users\\x\\demo\\src\\a.rs").as_deref(), Some("src/a.rs"));
        // 还没进历史的新文件：按已知根目录切；worktree 在主仓库里面时取最长的根
        let pm = PathMap { known: HashSet::new(), markers: vec!["/home/me/demo/".into(), "/home/me/demo/.claude/worktrees/x/".into()], excludes: vec![] };
        assert_eq!(pm.to_rel("/home/me/demo/src/New.rs").as_deref(), Some("src/New.rs"));
        assert_eq!(pm.to_rel("/home/me/demo/.claude/worktrees/x/src/b.rs").as_deref(), Some("src/b.rs"));
        assert_eq!(pm.to_rel("/home/me/demo"), None);
        assert_eq!(pm.to_rel("/home/me/demo/../etc/passwd"), None);
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
