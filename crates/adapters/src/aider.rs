//! Aider：仓库根目录下的 `.aider.chat.history.md`（`AIDER_CHAT_HISTORY_FILE` 指定的位置也读）。
//! 这是一份 Markdown 记录：`# aider chat started at …` 开一段会话，`#### ` 是用户的话，`> ` 是 Aider 自己的输出，
//! 其余是模型的回复，编辑以 SEARCH/REPLACE 块、整文件、统一 diff 或 apply_patch 的形式写在回复里。
//! 只认 Aider 随后报告了「Applied edit to <文件>」的那些文件的编辑；被拒绝、格式错误、没应用的不算。
//! 时间：会话开始时间是本地时间、不带时区；回复后面的「Commit <哈希>」能在 git 里查到准确时间和时区。

use crate::{added_lines, apply_patch_files, Health, Stats};
use creditia_core::model::{fingerprint, provider_hint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::{date_cn, days_from_civil};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const FILE: &str = ".aider.chat.history.md";

/// 一次确认过的编辑
#[derive(Debug, PartialEq)]
struct Edit {
    file: String,
    text: String,
    model: String,
    commit: Option<String>,
}

#[derive(Debug, Default)]
struct Sess {
    /// 本地时间（epoch 秒，未减时区）
    local: i64,
    model: String,
    format: String,
    editor_model: String,
    editor_format: String,
    prompts: i64,
    tokens: i64,
    edits: Vec<Edit>,
    commits: Vec<String>,
}

impl Sess {
    /// 真正写代码的模型和格式：architect 模式下是 editor 模型
    fn writer(&self) -> (&str, &str) {
        if self.format == "architect" && !self.editor_model.is_empty() {
            (&self.editor_model, &self.editor_format)
        } else {
            (&self.model, &self.format)
        }
    }
}

/// `2026-10-01 12:00:00` → epoch 秒（当作 UTC，调用方再减时区）
fn local_ts(s: &str) -> Option<i64> {
    let s = s.trim();
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    Some(days_from_civil(n(0..4)?, n(5..7)?, n(8..10)?) * 86400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?)
}

/// `2.3k`、`1.1M`、`145` → 数字
fn tok(s: &str) -> i64 {
    let s = s.trim().trim_end_matches(['.', ',']);
    let (num, mul) = match s.chars().last() {
        Some('k') | Some('K') => (&s[..s.len() - 1], 1e3),
        Some('m') | Some('M') => (&s[..s.len() - 1], 1e6),
        _ => (s, 1.0),
    };
    num.parse::<f64>().map(|v| (v * mul) as i64).unwrap_or(0)
}

/// `Tokens: 2.3k sent, 1.2k cache write, 800 cache hit, 145 received. Cost: …`：发送 + 缓存命中 + 接收（缓存写入已经算在发送里）
fn tokens_of(line: &str) -> i64 {
    let body = line.trim_start_matches("Tokens:").split(". Cost").next().unwrap_or("");
    body.split(',').map(|part| {
        let part = part.trim();
        let (n, what) = part.split_once(' ').unwrap_or((part, ""));
        if what.starts_with("sent") || what.starts_with("cache hit") || what.starts_with("received") { tok(n) } else { 0 }
    }).sum()
}

/// `Model: X with diff edit format, prompt cache` → (X, diff)
fn model_line(rest: &str) -> Option<(String, String)> {
    let (m, f) = rest.split_once(" with ")?;
    let f = f.split(" edit format").next()?.trim();
    Some((m.trim().to_string(), f.to_string()))
}

/// 一行像不像文件名：Aider 在编辑块前单独一行写文件名，可能带反引号、星号、井号、冒号
fn as_filename(l: &str) -> Option<String> {
    let t = l.trim().trim_matches(|c: char| c == '`' || c == '*' || c == '#' || c == ':' || c.is_whitespace());
    if t.is_empty() || t.contains(' ') && !t.contains('/') || t.starts_with("```") || t.len() > 300 {
        return None;
    }
    if t.contains('.') || t.contains('/') { Some(t.to_string()) } else { None }
}

fn is_fence(l: &str) -> bool {
    l.trim_start().starts_with("```")
}

fn marker(l: &str, c: char, word: &str) -> bool {
    let t = l.trim_end();
    let n = t.chars().take_while(|&x| x == c).count();
    (5..=9).contains(&n) && t[n..].trim() == word
}

/// 一段回复里的编辑块：(文件, 新写下的行)
fn blocks(chunk: &str, format: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = chunk.lines().collect();
    let mut out = vec![];
    // apply_patch 格式（patch）
    if chunk.contains("*** Begin Patch") {
        out.extend(apply_patch_files(chunk));
    }
    let mut last_file: Option<String> = None;
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        // SEARCH/REPLACE（diff、diff-fenced、editor-diff）：往回最多看三行找文件名，跳过围栏
        if marker(l, '<', "SEARCH") {
            let mut file = None;
            for j in (i.saturating_sub(3)..i).rev() {
                if lines[j].trim().is_empty() || is_fence(lines[j]) {
                    continue;
                }
                file = as_filename(lines[j]);
                break;
            }
            let file = file.or(last_file.clone());
            let (mut search, mut replace, mut in_rep) = (String::new(), String::new(), false);
            i += 1;
            while i < lines.len() && !marker(lines[i], '>', "REPLACE") {
                if !in_rep && marker(lines[i], '=', "") {
                    in_rep = true;
                } else if in_rep {
                    replace.push_str(lines[i]);
                    replace.push('\n');
                } else {
                    search.push_str(lines[i]);
                    search.push('\n');
                }
                i += 1;
            }
            if let Some(f) = file {
                out.push((f.clone(), added_lines(&search, &replace)));
                last_file = Some(f);
            }
            i += 1;
            continue;
        }
        // 统一 diff（udiff）：```diff 围栏里的 --- / +++ 和 + 行
        if let Some(f) = l.strip_prefix("+++ ") {
            let f = f.trim();
            let f = f.strip_prefix("b/").unwrap_or(f).to_string();
            let mut text = String::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("--- ") && !is_fence(lines[i]) {
                if let Some(a) = lines[i].strip_prefix('+') {
                    text.push_str(a);
                    text.push('\n');
                }
                i += 1;
            }
            out.push((f, text));
            continue;
        }
        // 整文件（whole、editor-whole）：文件名一行，紧跟着围栏里的全文
        if format.ends_with("whole") && is_fence(l) && i > 0 {
            if let Some(f) = as_filename(lines[i - 1]) {
                let mut text = String::new();
                i += 1;
                while i < lines.len() && !(lines[i].trim() == "```" || lines[i].trim() == "````") {
                    text.push_str(lines[i]);
                    text.push('\n');
                    i += 1;
                }
                out.push((f, text));
                i += 1;
                continue;
            }
        }
        i += 1;
    }
    out.retain(|(_, t)| !t.trim().is_empty());
    out
}

fn same_file(a: &str, b: &str) -> bool {
    let n = |s: &str| s.trim().trim_start_matches("./").replace('\\', "/");
    n(a) == n(b)
}

/// 读一份记录
fn parse(text: &str) -> Vec<Sess> {
    let mut out: Vec<Sess> = vec![];
    let mut chunk = String::new();
    // 还没确认的编辑块（文件、文本、模型）；确认了但还没等到提交的编辑的下标
    let mut pending: Vec<(String, String, String)> = vec![];
    let mut uncommitted: Vec<usize> = vec![];
    let mut prev_user = false;

    fn flush(out: &mut [Sess], chunk: &mut String, pending: &mut Vec<(String, String, String)>) {
        if let Some(s) = out.last() {
            if !chunk.trim().is_empty() {
                let (m, f) = s.writer();
                let m = m.to_string();
                for (file, t) in blocks(chunk, f) {
                    pending.push((file, t, m.clone()));
                }
            }
        }
        chunk.clear();
    }

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# aider chat started at ") {
            flush(&mut out, &mut chunk, &mut pending);
            pending.clear();
            uncommitted.clear();
            out.push(Sess { local: local_ts(rest).unwrap_or(0), ..Default::default() });
            prev_user = false;
            continue;
        }
        if out.is_empty() {
            continue;
        }
        if line.starts_with("#### ") || line == "####" {
            flush(&mut out, &mut chunk, &mut pending);
            // 新的一轮：上一轮没被确认的编辑作废
            pending.clear();
            uncommitted.clear();
            if !prev_user {
                out.last_mut().unwrap().prompts += 1;
            }
            prev_user = true;
            continue;
        }
        prev_user = false;
        if let Some(rest) = line.strip_prefix("> ") {
            flush(&mut out, &mut chunk, &mut pending);
            let rest = rest.trim_end();
            let s = out.last_mut().unwrap();
            if let Some(m) = rest.strip_prefix("Model: ").or(rest.strip_prefix("Main model: ")).and_then(model_line) {
                (s.model, s.format) = m;
            } else if let Some(m) = rest.strip_prefix("Editor model: ").and_then(model_line) {
                (s.editor_model, s.editor_format) = m;
            } else if let Some(f) = rest.strip_prefix("Applied edit to ") {
                let mut k = 0;
                while k < pending.len() {
                    if same_file(&pending[k].0, f) {
                        let (_, text, model) = pending.remove(k);
                        s.edits.push(Edit { file: f.trim().to_string(), text, model, commit: None });
                        uncommitted.push(s.edits.len() - 1);
                    } else {
                        k += 1;
                    }
                }
            } else if let Some(c) = rest.strip_prefix("Commit ") {
                let h = c.split_whitespace().next().unwrap_or("");
                if h.len() >= 7 && h.chars().all(|c| c.is_ascii_hexdigit()) {
                    for &i in &uncommitted {
                        s.edits[i].commit = Some(h.to_string());
                    }
                    uncommitted.clear();
                    s.commits.push(h.to_string());
                }
            } else if rest.starts_with("Tokens: ") {
                s.tokens += tokens_of(rest);
            }
            continue;
        }
        chunk.push_str(line);
        chunk.push('\n');
    }
    flush(&mut out, &mut chunk, &mut pending);
    out
}

/// 提交的准确时间和作者时区（秒）：`git show -s --format='%h %at %ai'`
fn commit_times(repo: &Path, hashes: &[String]) -> HashMap<String, (i64, i64)> {
    let mut m = HashMap::new();
    for chunk in hashes.chunks(200) {
        let mut args = vec!["show", "-s", "--format=%h %H %at %ai", "--no-patch"];
        args.extend(chunk.iter().map(|s| s.as_str()));
        for l in crate::git::git(repo, &args).lines() {
            let p: Vec<&str> = l.split_whitespace().collect();
            if p.len() < 6 {
                continue;
            }
            let (Ok(at), Some(z)) = (p[2].parse::<i64>(), tz_secs(p[5])) else { continue };
            for h in chunk.iter().filter(|h| p[1].starts_with(h.as_str())) {
                m.insert(h.clone(), (at, z));
            }
        }
    }
    m
}

/// `+0800` → 28800
fn tz_secs(z: &str) -> Option<i64> {
    let sign = match z.chars().next()? { '+' => 1, '-' => -1, _ => return None };
    let h: i64 = z.get(1..3)?.parse().ok()?;
    let m: i64 = z.get(3..5)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// 一个项目里能找到的记录文件：每个已知根目录下的 .aider.chat.history.md，加上 AIDER_CHAT_HISTORY_FILE
fn files(roots: &[String]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = roots.iter().map(|r| Path::new(r).join(FILE)).filter(|p| p.is_file()).collect();
    if let Ok(p) = std::env::var("AIDER_CHAT_HISTORY_FILE") {
        let p = PathBuf::from(p);
        if p.is_absolute() && p.is_file() && !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

pub fn scan(store: &mut Store, pm: &PathMap, roots: &[String], st: &mut Stats) {
    let Some(repo) = roots.first() else { return };
    let repo = Path::new(repo);
    let mut n = 0;
    for f in files(roots) {
        // 记录里的路径相对于 Aider 运行时的仓库根目录：就是记录文件所在的目录
        let root = f.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let gk = format!("aider:{}", f.display());
        let fp = fingerprint(&f);
        if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
            n += store.groups[&gk].sessions.len();
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        let sessions = parse(&text);
        let hashes: Vec<String> = sessions.iter().flat_map(|s| s.commits.iter().cloned()).collect();
        let times = commit_times(repo, &hashes);
        // 没有提交可查时区的会话：用这份记录里任意一个提交的时区，再不行用仓库最新提交的
        let fallback_tz = times.values().next().map(|t| t.1).or_else(|| {
            let ai = crate::git::git(repo, &["log", "-1", "--format=%ai"]);
            ai.split_whitespace().nth(2).and_then(tz_secs)
        });
        let mut g = FileGroup { fp, ..Default::default() };
        for (k, s) in sessions.iter().enumerate() {
            let tz = s.commits.iter().find_map(|h| times.get(h)).map(|t| t.1).or(fallback_tz);
            // 不知道时区就取最早可能的时刻（UTC+14），证据只会比提交早，不会落到提交之后
            let start = s.local - tz.unwrap_or(14 * 3600);
            let id = format!("aider-{}-{k}", s.local);
            let mut end = start;
            for e in &s.edits {
                let ts = e.commit.as_ref().and_then(|h| times.get(h)).map(|t| t.0).unwrap_or(start);
                end = end.max(ts);
                let abs = format!("{}/{}", root.trim_end_matches('/'), e.file.trim_start_matches("./"));
                crate::push_text(&mut g.evs, pm, &abs, &e.text, Src::Aider, &e.model, &id, ts);
            }
            for h in &s.commits {
                if let Some(t) = times.get(h) {
                    end = end.max(t.0);
                }
            }
            if s.local > 0 {
                let (m, _) = s.writer();
                g.sessions.push(Session { src: Src::Aider, id, model: m.to_string(), start, end, tokens: s.tokens, branch: None, prompts: s.prompts, reviewer: false, has_content: true, provider: provider_hint(m) });
            }
        }
        n += g.sessions.len();
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("aider: 项目相关会话 {n} 个（仓库里的 {FILE}）", "aider: {n} sessions for this project ({FILE} in the repository)"));
    }
}

/// doctor：Aider 的记录跟着仓库走，这里只能看当前目录
pub fn doctor() -> Health {
    let here = std::env::current_dir().unwrap_or_default();
    let top = crate::git::git(&here, &["rev-parse", "--show-toplevel"]).trim().to_string();
    let f = Path::new(if top.is_empty() { "." } else { &top }).join(FILE);
    let text = std::fs::read_to_string(&f).unwrap_or_default();
    let ss = parse(&text);
    let days: Vec<i64> = ss.iter().map(|s| s.local).filter(|&t| t > 0).collect();
    Health {
        source: "aider".into(),
        path: f.display().to_string(),
        readable: f.is_file(),
        format: creditia_core::tr!("<仓库>/{FILE}", "<repo>/{FILE}"),
        records: ss.len() as u64,
        earliest: days.iter().min().copied().map(date_cn),
        latest: days.iter().max().copied().map(date_cn),
        note: creditia_core::tr!("记录在每个仓库的根目录里；这里只看当前仓库", "kept in each repository's root; this only checks the current one"),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"
# aider chat started at 2026-10-01 09:00:00

> /usr/local/bin/aider --model sonnet
> Aider v0.86.1
> Main model: anthropic/claude-sonnet-4-5-20250929 with diff edit format, infinite output
> Weak model: anthropic/claude-haiku-4-5
> Git repo: .git with 12 files

#### add a greeting
#### and a farewell

I'll add both functions.

src/app.py
```python
<<<<<<< SEARCH
def main():
    pass
=======
def main():
    greet()
    farewell()
>>>>>>> REPLACE
```

```python
<<<<<<< SEARCH
=======
def greet():
    print("hello there")
>>>>>>> REPLACE
```

notes/plan.md
```
<<<<<<< SEARCH
=======
this file was never applied
>>>>>>> REPLACE
```

> Tokens: 2.3k sent, 1.0k cache write, 145 received. Cost: $0.01 message, $0.01 session.
> Applied edit to src/app.py
> Commit 1a2b3c4 feat: Add greeting

#### /ask what next

Maybe tests.

# aider chat started at 2026-10-02 10:00:00

> Model: gpt-5 with whole edit format

#### write the readme

README.md
```
# Demo project
It greets people.
```

> Applied edit to README.md
"#;

    #[test]
    fn parses_sessions_edits_and_commits() {
        let ss = parse(LOG);
        assert_eq!(ss.len(), 2);
        let a = &ss[0];
        assert_eq!(a.model, "anthropic/claude-sonnet-4-5-20250929");
        assert_eq!(a.format, "diff");
        assert_eq!(a.prompts, 2);
        assert_eq!(a.tokens, 2300 + 145);
        // 两个块都在 src/app.py（第二个块没写文件名，跟着上一个）；plan.md 没被应用，不算
        assert_eq!(a.edits.len(), 2);
        assert!(a.edits.iter().all(|e| e.file == "src/app.py" && e.commit.as_deref() == Some("1a2b3c4")));
        assert_eq!(a.edits[0].text, "    greet()\n    farewell()\n");
        assert_eq!(a.edits[1].text, "def greet():\n    print(\"hello there\")\n");
        let b = &ss[1];
        assert_eq!((b.model.as_str(), b.format.as_str()), ("gpt-5", "whole"));
        assert_eq!(b.edits, vec![Edit { file: "README.md".into(), text: "# Demo project\nIt greets people.\n".into(), model: "gpt-5".into(), commit: None }]);
    }

    #[test]
    fn architect_credits_the_editor_model() {
        let log = "# aider chat started at 2026-10-03 08:00:00\n> Main model: o3 with architect edit format  \n> Editor model: gpt-4.1 with editor-diff edit format  \n#### refactor  \nPlan: rename.\n\nlib.rs\n```rust\n<<<<<<< SEARCH\nfn a() {}\n=======\nfn renamed() {}\n>>>>>>> REPLACE\n```\n> Applied edit to lib.rs  \n";
        let ss = parse(log);
        assert_eq!(ss[0].edits[0].model, "gpt-4.1");
        assert_eq!(ss[0].writer().0, "gpt-4.1");
    }

    #[test]
    fn udiff_and_unconfirmed_edits() {
        let log = "# aider chat started at 2026-10-03 08:00:00\n> Model: deepseek/deepseek-chat with udiff edit format  \n#### fix  \n```diff\n--- src/a.rs\n+++ src/a.rs\n@@ ... @@\n-old\n+new line here\n```\n> Applied edit to src/a.rs  \n#### again  \n```diff\n--- src/b.rs\n+++ src/b.rs\n@@ ... @@\n+never applied\n```\n> The LLM did not conform to the edit format.  \n";
        let ss = parse(log);
        assert_eq!(ss[0].edits.len(), 1);
        assert_eq!(ss[0].edits[0].file, "src/a.rs");
        assert_eq!(ss[0].edits[0].text, "new line here\n");
    }

    #[test]
    fn small_helpers() {
        assert_eq!(tok("2.3k"), 2300);
        assert_eq!(tok("1.5M"), 1_500_000);
        assert_eq!(tok("145."), 145);
        assert_eq!(tz_secs("+0800"), Some(28800));
        assert_eq!(tz_secs("-0530"), Some(-19800));
        assert_eq!(local_ts("2026-08-17 20:53:20"), Some(1_787_000_000));
        assert_eq!(as_filename("`src/app.py`"), Some("src/app.py".into()));
        assert_eq!(as_filename("I'll add both functions."), None);
    }
}
