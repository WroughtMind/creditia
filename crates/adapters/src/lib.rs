//! 各数据源 adapter：git、Codex、Claude Code、Cursor、Grok、Kimi。只读打开，输出统一的行级证据和会话元数据。

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod git;
pub mod grok;
pub mod kimi;

use creditia_core::hash::{ext_of, line_key, normalize};
use creditia_core::model::{Ev, PathMap, Src};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// adapter 运行时收集的提示：warnings 进退出码 2，notes 进报告
#[derive(Default, Debug)]
pub struct Stats {
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default())
}

/// 各数据源根目录：环境变量优先，便于测试重定向和自定义路径
pub fn source_dir(src: Src) -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    match src {
        Src::Codex => env("CODEX_HOME").unwrap_or_else(|| home().join(".codex")),
        Src::Claude => env("CLAUDE_CONFIG_DIR").unwrap_or_else(|| home().join(".claude")),
        Src::Cursor => env("CREDITIA_CURSOR_DIR").unwrap_or_else(|| home().join(".cursor")),
        Src::Grok => env("CREDITIA_GROK_DIR").unwrap_or_else(|| home().join(".grok")),
        Src::Kimi => env("CREDITIA_KIMI_DIR").unwrap_or_else(|| home().join(".kimi-code")),
    }
}

/// Cursor 对话库（state.vscdb）所在目录，各平台不同
pub fn cursor_global_storage() -> PathBuf {
    if let Ok(p) = std::env::var("CREDITIA_CURSOR_STORAGE") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "macos")]
    { home().join("Library/Application Support/Cursor/User/globalStorage") }
    #[cfg(target_os = "windows")]
    { PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Cursor/User/globalStorage") }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    { home().join(".config/Cursor/User/globalStorage") }
}

pub fn open_ro(p: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    Ok(c)
}

/// 把一段新文本拆成行，生成证据。返回是否映射到了本项目
pub fn push_text(out: &mut Vec<Ev>, pm: &PathMap, abs: &str, text: &str, src: Src, model: &str, session: &str, ts: i64) -> bool {
    let Some(rel) = pm.to_rel(abs) else { return false };
    let ext = ext_of(&rel);
    for line in text.lines() {
        let n = normalize(line, &ext);
        if n.chars().count() < 4 {
            continue;
        }
        out.push(Ev { rel: rel.clone(), key: line_key(&rel, &n), src, model: model.to_string(), session: session.to_string(), ts, tab: false });
    }
    true
}

/// 读取可能是 zstd 压缩的文本文件
pub fn read_maybe_zst(p: &Path) -> String {
    let raw = std::fs::read(p).unwrap_or_default();
    let bytes = if p.extension().map(|e| e == "zst").unwrap_or(false) {
        zstd::stream::decode_all(&raw[..]).unwrap_or_default()
    } else {
        raw
    };
    String::from_utf8_lossy(&bytes).into_owned()
}

/// doctor 用：一个数据源的体检结果
#[derive(serde::Serialize, Debug, Default)]
pub struct Health {
    pub source: String,
    pub path: String,
    pub readable: bool,
    pub format: String,
    pub records: u64,
    pub earliest: Option<String>,
    pub latest: Option<String>,
    pub note: String,
}

/// discover 用：某个工具在某个目录下的活动
#[derive(serde::Serialize, Debug, Clone)]
pub struct Activity {
    pub src: Src,
    pub cwd: String,
    pub remote: Option<String>,
    pub sessions: u64,
    pub last: i64,
}

/// 百分号解码（Grok 的会话目录名是 URL 编码的 cwd）
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 把 JSON 里的 epoch 毫秒 / 秒 / ISO 字符串统一成 epoch 秒
pub fn any_ts(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).map(|x| if x > 10_000_000_000 { x / 1000 } else { x }),
        serde_json::Value::String(s) => creditia_core::util::parse_iso(s).or_else(|| s.parse::<i64>().ok().map(|x| if x > 10_000_000_000 { x / 1000 } else { x })),
        _ => None,
    }
}
