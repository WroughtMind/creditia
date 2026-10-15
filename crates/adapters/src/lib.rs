//! 各数据源 adapter：git、Codex、Claude Code、Cursor、Grok、Kimi、Gemini CLI、Qwen Code、OpenCode、Cline / Roo / Kilo、
//! Aider、GitHub Copilot（VS Code 和命令行）、Amp、TRAE CLI。
//! 只读打开，输出统一的行级证据和会话元数据。只认「成功写进文件」的编辑；路径映射回仓库内相对路径。

pub mod aider;
pub mod amp;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod git;
pub mod grok;
pub mod kimi;
pub mod opencode;
pub mod others;
pub mod trae;

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
        Src::Gemini => env("CREDITIA_GEMINI_DIR").or_else(|| env("GEMINI_CLI_HOME").map(|p| p.join(".gemini"))).unwrap_or_else(|| home().join(".gemini")),
        Src::Qwen => env("CREDITIA_QWEN_DIR").unwrap_or_else(|| home().join(".qwen")),
        Src::OpenCode => env("CREDITIA_OPENCODE_DIR").unwrap_or_else(|| xdg_data().join("opencode")),
        // VS Code 扩展：真实位置有多个（Code、Cursor、Windsurf……），见 cline::roots
        Src::Cline | Src::Roo | Src::Kilo => env("CREDITIA_VSCODE_STORAGE").unwrap_or_else(vscode_storage),
        // Copilot 命令行；VS Code 里的 Copilot Chat 见 copilot::user_dirs
        Src::Copilot => env("CREDITIA_COPILOT_DIR").or_else(|| env("COPILOT_HOME")).unwrap_or_else(|| home().join(".copilot")),
        Src::Amp => env("CREDITIA_AMP_DIR").unwrap_or_else(|| xdg_data().join("amp")),
        Src::Trae => env("CREDITIA_TRAE_DIR").or_else(|| env("TRAECLI_HOME")).or_else(|| env("TRAE_HOME").map(|p| p.join("cli"))).unwrap_or_else(|| home().join(".trae/cli")),
        // Aider 的记录在每个仓库根目录里（.aider.chat.history.md），没有统一位置
        Src::Aider => home(),
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

/// XDG 数据目录（OpenCode 等用）；Windows / macOS 上 OpenCode 也用 ~/.local/share
pub fn xdg_data() -> PathBuf {
    std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

/// VS Code 系编辑器的 globalStorage（默认 Code；其他编辑器在 cline::roots 里一起找）
pub fn vscode_storage() -> PathBuf {
    vscode_user_dirs().into_iter().next().unwrap_or_default().join("globalStorage")
}
/// 各个 VS Code 系编辑器的 User 目录：Code、Code - Insiders、Cursor、Windsurf、VSCodium、Trae（国际版、国内版）
pub fn vscode_user_dirs() -> Vec<PathBuf> {
    let names = ["Code", "Code - Insiders", "Cursor", "Windsurf", "VSCodium", "Trae", "Trae CN"];
    #[cfg(target_os = "macos")]
    let base = home().join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var("APPDATA").unwrap_or_default());
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    names.iter().map(|n| base.join(n).join("User")).collect()
}

/// 编辑工具的参数里新写下的文本：write 类取 content，edit / replace 类取 new_string（各家字段名不同）
pub fn new_text(name: &str, args: &serde_json::Value) -> Option<String> {
    let n = name.to_lowercase();
    let get = |ks: &[&str]| ks.iter().find_map(|k| args[*k].as_str().map(String::from));
    if n.contains("write") || n.contains("create") {
        return get(&["content", "file_text", "contents", "text"]);
    }
    if n.contains("edit") || n.contains("replace") || n.contains("patch") {
        // 编辑的新文本里通常带着几行没改的上下文：减掉旧文本里已有的行
        let old_of = |e: &serde_json::Value| ["old_string", "oldString", "old_str"].iter().find_map(|k| e[*k].as_str()).unwrap_or("").to_string();
        if let Some(edits) = args["edits"].as_array() {
            let all: Vec<String> = edits.iter().filter_map(|e| e["new_string"].as_str().or(e["newString"].as_str()).map(|t| added_lines(&old_of(e), t))).collect();
            if !all.is_empty() { return Some(all.concat()); }
        }
        return get(&["new_string", "newString", "new_str", "replacement", "content"]).map(|t| added_lines(&old_of(args), &t));
    }
    None
}

/// shell 命令里提到的仓库内文件（相对路径按命令的工作目录和命令里 `cd` 过的目录解析）。
/// 命令原文（heredoc、printf、python 脚本里的新文本）作为这些文件的候选证据：只有真出现在提交新增行里的才算数
pub fn shell_targets(cmd: &str, cwd: &str, pm: &PathMap) -> Vec<String> {
    use std::sync::OnceLock;
    static CD: OnceLock<regex::Regex> = OnceLock::new();
    let cd = CD.get_or_init(|| regex::Regex::new(r#"(?:^|[;&|(]\s*)cd\s+["']?([^\s;&|)"']+)"#).unwrap());
    let join = |base: &str, p: &str| if p.starts_with('/') { p.to_string() } else { format!("{}/{}", base.trim_end_matches('/'), p) };
    let mut dirs = vec![cwd.to_string()];
    for c in cd.captures_iter(cmd) {
        dirs.push(join(cwd, &c[1]));
    }
    let mut out: Vec<String> = vec![];
    for tok in cmd.split(|c: char| c.is_whitespace() || ";&|<>()=,`'\"{}[]".contains(c)) {
        let tok = tok.trim_end_matches(':');
        if tok.len() < 3 || tok.starts_with('-') || tok.starts_with('$') || !(tok.contains('/') || tok.contains('.')) {
            continue;
        }
        let cands: Vec<String> = if tok.starts_with('/') { vec![tok.to_string()] } else { dirs.iter().map(|d| join(d, tok)).collect() };
        for abs in cands {
            if let Some(rel) = pm.to_rel(&abs) {
                if !out.contains(&rel) {
                    out.push(rel);
                }
                break;
            }
        }
    }
    out
}

/// 把一段文本记成某个仓库内相对路径的证据（push_text 的相对路径版）
pub fn push_rel(out: &mut Vec<Ev>, rel: &str, text: &str, src: Src, model: &str, session: &str, ts: i64) {
    let ext = ext_of(rel);
    for line in text.lines() {
        let n = normalize(line, &ext);
        if n.chars().count() >= 4 {
            out.push(Ev { rel: rel.to_string(), key: line_key(rel, &n), src, model: model.to_string(), session: session.to_string(), ts, tab: false });
        }
    }
}

/// new 里比 old 多出来的行（按行计数相减，顺序不变）：编辑的上下文行、原样保留的行都不算新写的
pub fn added_lines(old: &str, new: &str) -> String {
    let mut have: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for l in old.lines() {
        *have.entry(l.trim_end()).or_default() += 1;
    }
    let mut out = String::new();
    for l in new.lines() {
        match have.get_mut(l.trim_end()) {
            Some(n) if *n > 0 => *n -= 1,
            _ => {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}

/// Claude Code 等工具结果里的 structuredPatch（统一 diff 的 hunk 列表）中新增的行
pub fn added_from_patch(patch: &serde_json::Value) -> String {
    let mut out = String::new();
    for h in patch.as_array().into_iter().flatten() {
        for l in h["lines"].as_array().into_iter().flatten().filter_map(|l| l.as_str()) {
            if let Some(a) = l.strip_prefix('+') {
                out.push_str(a);
                out.push('\n');
            }
        }
    }
    out
}

/// apply_patch 格式（Codex 发明，Copilot、Amp、TRAE 也用）里每个文件新增的行：
/// `*** Add File: p` 下面全是新内容，`*** Update File: p` 下面 `+` 开头的行是新增的；`*** Move to:` 换成新路径
pub fn apply_patch_files(patch: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    let mut cur: Option<usize> = None;
    for l in patch.lines() {
        let t = l.trim_end();
        if let Some(p) = t.strip_prefix("*** Add File: ").or(t.strip_prefix("*** Update File: ")) {
            out.push((p.trim().to_string(), String::new()));
            cur = Some(out.len() - 1);
        } else if let Some(p) = t.strip_prefix("*** Move to: ") {
            if let Some(i) = cur { out[i].0 = p.trim().to_string(); }
        } else if t.starts_with("*** ") {
            // Delete File、End Patch、End of File
            if !t.starts_with("*** End of File") { cur = None; }
        } else if let (Some(i), Some(a)) = (cur, l.strip_prefix('+')) {
            out[i].1.push_str(a);
            out[i].1.push('\n');
        }
    }
    out.retain(|(_, t)| !t.is_empty());
    out
}

/// `file:///Users/me/x`、`file:///c%3A/work` 这类地址 → 本地路径；不是 file 地址原样返回
pub fn file_uri_path(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else { return uri.to_string() };
    let p = percent_decode(rest.strip_prefix("localhost").unwrap_or(rest));
    // /C:/work → C:/work
    let b = p.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() { p[1..].to_string() } else { p }
}

/// 文件路径参数（各家字段名不同）
pub fn path_arg(args: &serde_json::Value) -> Option<String> {
    ["file_path", "filePath", "path", "absolute_path", "target_file"].iter().find_map(|k| args[*k].as_str().map(String::from))
}

/// 统一 diff / SEARCH-REPLACE 块里新增的行
pub fn added_from_diff(diff: &str) -> String {
    if diff.contains(">>>>>>> REPLACE") || diff.contains("+++++++ REPLACE") {
        let mut out = String::new();
        let mut on = false;
        for l in diff.lines() {
            let t = l.trim();
            if t == "=======" { on = true; continue; }
            if t.starts_with(">>>>>>>") || t.starts_with("+++++++") || t.starts_with("<<<<<<<") || t.starts_with("-------") { on = false; continue; }
            if on { out.push_str(l); out.push('\n'); }
        }
        return out;
    }
    diff.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).map(|l| format!("{}\n", &l[1..])).collect()
}

pub fn open_ro(p: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    Ok(c)
}

/// 把一段新文本拆成行，生成证据。返回是否映射到了本项目
#[allow(clippy::too_many_arguments)]
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
    /// 装了、但 creditia 读不了它的编辑记录（Windsurf、TRAE IDE）：只说明，不算警告
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unsupported: bool,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_text_by_tool_kind() {
        assert_eq!(new_text("Write", &json!({ "file_path": "a", "content": "x = 1" })).as_deref(), Some("x = 1"));
        assert_eq!(new_text("create_file", &json!({ "file_text": "y" })).as_deref(), Some("y"));
        assert_eq!(new_text("Edit", &json!({ "old_string": "a", "new_string": "b" })).as_deref(), Some("b\n"));
        assert_eq!(new_text("edit", &json!({ "oldString": "a", "newString": "c" })).as_deref(), Some("c\n"));
        // 新文本里带着的上下文行（旧文本里就有）不算新写的
        assert_eq!(new_text("Edit", &json!({ "old_string": "ctx\nold\nctx2", "new_string": "ctx\nnew\nctx2" })).as_deref(), Some("new\n"));
        // MultiEdit：每段新增的行连起来
        assert_eq!(new_text("MultiEdit", &json!({ "edits": [{ "new_string": "one" }, { "old_string": "k", "newString": "k\ntwo" }] })).as_deref(), Some("one\ntwo\n"));
        // 读文件、跑命令这类不算写入
        assert_eq!(new_text("Read", &json!({ "file_path": "a", "content": "x" })), None);
        assert_eq!(new_text("Bash", &json!({ "command": "ls" })), None);
    }

    #[test]
    fn shell_targets_resolve_repo_files() {
        let pm = PathMap { known: ["src/a.rs".to_string(), "web/b.ts".to_string(), "README.md".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] };
        // 相对路径按工作目录解析；绝对路径直接认；`cd` 过的目录也算
        assert_eq!(shell_targets("cat > src/a.rs <<'EOF'\nfn x() {}\nEOF", "/home/me/demo", &pm), vec!["src/a.rs"]);
        assert_eq!(shell_targets("tee /home/me/demo/web/b.ts < x", "/tmp", &pm), vec!["web/b.ts"]);
        assert_eq!(shell_targets("cd web && printf 'a' > b.ts", "/home/me/demo", &pm), vec!["web/b.ts"]);
        // 仓库外的文件、选项、变量不算
        assert!(shell_targets("ls -la /etc/hosts $HOME/x.rs --out=foo.rs", "/home/me/demo", &pm).is_empty());
    }

    #[test]
    fn added_lines_and_patch() {
        // 按次数相减：旧文本里有一行 x，新文本里两行 x，多出来的那一行算新写的
        assert_eq!(added_lines("x\ny", "x\nx\ny\nz"), "x\nz\n");
        let patch = json!([{ "oldStart": 1, "oldLines": 3, "newStart": 1, "newLines": 3, "lines": [" keep", "-gone", "+fresh", " keep2"] }, { "lines": ["+tail"] }]);
        assert_eq!(added_from_patch(&patch), "fresh\ntail\n");
        assert_eq!(added_from_patch(&json!([])), "");
    }

    #[test]
    fn apply_patch_and_file_uris() {
        let patch = "*** Begin Patch\n*** Add File: src/new.rs\n+fn a() {}\n+fn b() {}\n*** Update File: src/old.rs\n@@ fn main\n-    old();\n+    new();\n     keep();\n*** Update File: src/moved.rs\n*** Move to: src/there.rs\n@@\n+moved line\n*** Delete File: gone.rs\n*** End Patch";
        assert_eq!(apply_patch_files(patch), vec![
            ("src/new.rs".to_string(), "fn a() {}\nfn b() {}\n".to_string()),
            ("src/old.rs".to_string(), "    new();\n".to_string()),
            ("src/there.rs".to_string(), "moved line\n".to_string()),
        ]);
        assert!(apply_patch_files("no patch here").is_empty());
        assert_eq!(file_uri_path("file:///Users/me/my%20app"), "/Users/me/my app");
        assert_eq!(file_uri_path("file:///c%3A/work/x"), "c:/work/x");
        assert_eq!(file_uri_path("/plain/path"), "/plain/path");
    }

    #[test]
    fn path_arg_field_names() {
        for k in ["file_path", "filePath", "path", "absolute_path", "target_file"] {
            assert_eq!(path_arg(&json!({ k: "/r/a.rs" })).as_deref(), Some("/r/a.rs"), "{k}");
        }
        assert_eq!(path_arg(&json!({ "command": "ls" })), None);
    }

    #[test]
    fn added_lines_from_diffs() {
        let unified = "--- a/x.rs\n+++ b/x.rs\n@@ -1,2 +1,3 @@\n keep\n-old\n+new one\n+new two\n";
        assert_eq!(added_from_diff(unified), "new one\nnew two\n");
        let search_replace = "<<<<<<< SEARCH\nold line\n=======\nreplacement a\nreplacement b\n>>>>>>> REPLACE\n";
        assert_eq!(added_from_diff(search_replace), "replacement a\nreplacement b\n");
        // Roo 的写法
        let roo = "<<<<<<< SEARCH\n:start_line:3\n-------\nold\n=======\nfresh\n+++++++ REPLACE\n";
        assert_eq!(added_from_diff(roo), "fresh\n");
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("%2FUsers%2Fme%2F%E7%A4%BA%E4%BE%8B"), "/Users/me/示例");
        assert_eq!(percent_decode("plain-name"), "plain-name");
        // 不完整的转义原样保留
        assert_eq!(percent_decode("50%"), "50%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn timestamps_in_any_unit() {
        assert_eq!(any_ts(&json!(1_787_000_000)), Some(1_787_000_000));
        assert_eq!(any_ts(&json!(1_787_000_000_123i64)), Some(1_787_000_000));
        assert_eq!(any_ts(&json!(1_787_000_000.5)), Some(1_787_000_000));
        assert_eq!(any_ts(&json!("1787000000123")), Some(1_787_000_000));
        assert_eq!(any_ts(&json!("2026-08-17T20:53:20.000Z")), Some(1_787_000_000));
        assert_eq!(any_ts(&json!(null)), None);
        assert_eq!(any_ts(&json!("yesterday")), None);
    }

    #[test]
    fn push_text_skips_short_lines_and_foreign_paths() {
        let pm = PathMap { known: ["src/a.rs".to_string()].into_iter().collect(), markers: vec!["/demo/".into()], excludes: vec![] };
        let mut out = vec![];
        let text = "fn main() { run(); }\n}\n\n  let value = compute();\n";
        assert!(push_text(&mut out, &pm, "/home/me/demo/src/a.rs", text, Src::Claude, "m", "s", 1));
        // 「}」和空行太短，不算
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| e.rel == "src/a.rs" && e.src == Src::Claude));
        // 别的项目的路径：不映射
        assert!(!push_text(&mut out, &pm, "/home/me/other/src/a.rs", text, Src::Claude, "m", "s", 1));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn zstd_or_plain() {
        let dir = std::env::temp_dir().join(format!("creditia-zst-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jsonl"), "plain\n").unwrap();
        std::fs::write(dir.join("b.jsonl.zst"), zstd::stream::encode_all(&b"packed\n"[..], 3).unwrap()).unwrap();
        assert_eq!(read_maybe_zst(&dir.join("a.jsonl")), "plain\n");
        assert_eq!(read_maybe_zst(&dir.join("b.jsonl.zst")), "packed\n");
        assert_eq!(read_maybe_zst(&dir.join("missing")), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
