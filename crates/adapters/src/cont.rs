//! Continue（continuedev/continue）：`~/.continue/sessions/<会话>.json`（`CONTINUE_GLOBAL_DIR` 设了就在它下面），VS Code / JetBrains 扩展和 `cn` 命令行共用。
//! 会话有 workspaceDirectory（常是 file:// 地址）、history[]、usage；`sessions.json` 是索引，带 dateCreated（毫秒）。
//! history 里 assistant 消息的 toolCallStates：status 为 done 才算写进去了（报错是 errored，用户拒绝 diff 是 canceled）。
//! 编辑工具：扩展的 create_new_file（filepath, contents）、single_find_and_replace、multi_edit、edit_existing_file（changes 是带
//! 「... existing code ...」占位的片段，去掉占位行当候选证据），命令行的 Write / Edit / MultiEdit（file_path）；
//! run_terminal_command / Bash 的命令原文作为命令里提到的文件的候选证据。
//! 模型取 promptLogs 的 modelTitle（用户给模型起的标题，规整成小写连字符），没有就取会话的 chatModelTitle；渠道取 modelProvider。

use crate::{file_uri_path, new_text, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn global_dir() -> PathBuf {
    let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    env("CREDITIA_CONTINUE_DIR").or_else(|| env("CONTINUE_GLOBAL_DIR")).unwrap_or_else(|| crate::home().join(".continue"))
}

fn sessions_dir() -> PathBuf {
    global_dir().join("sessions")
}

fn read(p: &Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// 会话文件，配上索引里的创建时间
fn files() -> Vec<(PathBuf, i64)> {
    let dir = sessions_dir();
    let index = read(&dir.join("sessions.json"));
    let created: HashMap<String, i64> = index
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| Some((m["sessionId"].as_str()?.to_string(), crate::any_ts(&m["dateCreated"])?)))
        .collect();
    let mut v: Vec<(PathBuf, i64)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json") && p.file_name().is_some_and(|n| n != "sessions.json"))
        .map(|p| {
            let id = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let t = created.get(&id).copied().unwrap_or(0);
            (p, t)
        })
        .collect();
    v.sort();
    v
}

fn workspace(v: &Value) -> String {
    file_uri_path(v["workspaceDirectory"].as_str().unwrap_or("")).trim_end_matches('/').to_string()
}

fn mtime(p: &Path) -> i64 {
    fingerprint(p).0 as i64
}
/// 文件的创建时间（取不到就用修改时间）
fn birth(p: &Path) -> i64 {
    std::fs::metadata(p).and_then(|m| m.created()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or_else(|| mtime(p))
}

/// 模型标题 → 小写连字符（「Claude Sonnet 4.5」→ claude-sonnet-4.5）
fn model_id(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join("-").to_lowercase()
}

/// 一次成功的工具调用写下的（路径, 新增的行）；shell 命令路径为空
fn edits(name: &str, a: &Value) -> Vec<(String, String)> {
    let path = ["filepath", "file_path", "path"].iter().find_map(|k| a[*k].as_str()).unwrap_or("").to_string();
    let out = match name {
        "run_terminal_command" | "Bash" => a["command"].as_str().map(|c| vec![(String::new(), c.to_string())]).unwrap_or_default(),
        // 片段里「... existing code ...」这类占位行不是新写的
        "edit_existing_file" => {
            let t: String = a["changes"].as_str().unwrap_or("").lines().filter(|l| !l.contains("... existing code") && !l.contains("... rest of")).map(|l| format!("{l}\n")).collect();
            vec![(path, t)]
        }
        "create_new_file" | "single_find_and_replace" | "multi_edit" | "Write" | "Edit" | "MultiEdit" => new_text(name, a).map(|t| vec![(path, t)]).unwrap_or_default(),
        _ => vec![],
    };
    out.into_iter().filter(|(_, t)| !t.trim().is_empty()).collect()
}

fn args_of(state: &Value) -> Value {
    if state["parsedArgs"].is_object() {
        return state["parsedArgs"].clone();
    }
    state["toolCall"]["function"]["arguments"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
}

fn top(m: HashMap<String, usize>) -> String {
    m.into_iter().max_by_key(|(k, n)| (*n, std::cmp::Reverse(k.clone()))).map(|(k, _)| k).unwrap_or_default()
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut n = 0;
    for (f, created) in files() {
        let v = read(&f);
        let ws = workspace(&v);
        if !pm.in_project(&ws) {
            continue;
        }
        let id = v["sessionId"].as_str().map(String::from).unwrap_or_else(|| f.file_stem().unwrap_or_default().to_string_lossy().to_string());
        let gk = format!("continue:{id}");
        let fp = fingerprint(&f);
        if store.groups.get(&gk).is_some_and(|g| g.fp == fp) {
            n += 1;
            continue;
        }
        let mut g = FileGroup { fp, ..Default::default() };
        // 记录里没有逐条时间：开始和证据时间用索引里的创建时间（没有就用文件的创建时间），结束用文件的修改时间。
        // 证据时间得在提交之前，所以不用修改时间（会话可能在提交之后还在继续）
        let end = mtime(&f);
        let start = if created > 0 { created } else { birth(&f).min(end) };
        let fallback = model_id(v["chatModelTitle"].as_str().unwrap_or(""));
        let (mut models, mut providers): (HashMap<String, usize>, HashMap<String, usize>) = Default::default();
        let mut prompts = 0;
        let mut model = fallback.clone();
        let mut any = false;
        for item in v["history"].as_array().into_iter().flatten() {
            let role = item["message"]["role"].as_str().unwrap_or("");
            if role == "user" {
                prompts += 1;
                continue;
            }
            if role != "assistant" {
                continue;
            }
            any = true;
            if let Some(log) = item["promptLogs"].as_array().and_then(|l| l.last()) {
                if let Some(t) = log["modelTitle"].as_str().filter(|t| !t.is_empty()) {
                    model = model_id(t);
                    *models.entry(model.clone()).or_default() += 1;
                }
                if let Some(p) = log["modelProvider"].as_str().filter(|p| !p.is_empty()) {
                    *providers.entry(p.to_string()).or_default() += 1;
                }
            }
            for s in item["toolCallStates"].as_array().into_iter().flatten().filter(|s| s["status"] == "done") {
                let name = s["toolCall"]["function"]["name"].as_str().unwrap_or("");
                for (path, text) in edits(name, &args_of(s)) {
                    if path.is_empty() {
                        for rel in crate::shell_targets(&text, &ws, pm) {
                            crate::push_rel(&mut g.evs, &rel, &text, Src::Continue, &model, &id, start);
                        }
                        continue;
                    }
                    let p = file_uri_path(&path);
                    let abs = if Path::new(&p).is_absolute() || p.get(1..2) == Some(":") { p } else { format!("{ws}/{p}") };
                    crate::push_text(&mut g.evs, pm, &abs, &text, Src::Continue, &model, &id, start);
                }
            }
        }
        if !any {
            continue;
        }
        let u = &v["usage"];
        let tokens = u["promptTokens"].as_i64().unwrap_or(0) + u["completionTokens"].as_i64().unwrap_or(0);
        let model = if models.is_empty() { fallback } else { top(models) };
        let provider = Some(top(providers)).filter(|p| !p.is_empty());
        g.sessions.push(Session { src: Src::Continue, id, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true, provider });
        n += 1;
        store.groups.insert(gk, g);
    }
    if n > 0 {
        st.notes.push(creditia_core::tr!("continue: 项目相关会话 {n} 个", "continue: {n} sessions for this project"));
    }
}

pub fn discover() -> Vec<Activity> {
    let mut m: HashMap<String, Activity> = HashMap::new();
    for (f, _) in files() {
        let ws = workspace(&read(&f));
        if ws.is_empty() {
            continue;
        }
        let e = m.entry(ws.clone()).or_insert(Activity { src: Src::Continue, cwd: ws, remote: None, sessions: 0, last: 0 });
        e.sessions += 1;
        e.last = e.last.max(mtime(&f));
    }
    m.into_values().collect()
}

pub fn doctor() -> Health {
    let fs = files();
    Health {
        source: "continue".into(),
        path: sessions_dir().display().to_string(),
        readable: sessions_dir().is_dir(),
        format: "sessions/<id>.json (history + toolCallStates)".into(),
        records: fs.len() as u64,
        earliest: fs.iter().map(|f| if f.1 > 0 { f.1 } else { mtime(&f.0) }).filter(|&t| t > 0).min().map(date_cn),
        latest: fs.iter().map(|f| mtime(&f.0)).max().filter(|&t| t > 0).map(date_cn),
        note: creditia_core::tr!(
            "扩展和命令行共用；工具调用带状态，模型是用户起的标题。尚未用真实日志核对",
            "shared by the editor extensions and the CLI; tool calls carry a status, models are user-given titles. Not yet checked against real logs"
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_calls_to_edits() {
        assert_eq!(edits("create_new_file", &json!({ "filepath": "src/a.ts", "contents": "export const a = 1;" })), vec![("src/a.ts".into(), "export const a = 1;".into())]);
        assert_eq!(edits("single_find_and_replace", &json!({ "filepath": "a.ts", "old_string": "x", "new_string": "x\ny" })), vec![("a.ts".into(), "y\n".into())]);
        assert_eq!(edits("edit_existing_file", &json!({ "filepath": "a.ts", "changes": "// ... existing code ...\nconst fresh = 2;\n// ... existing code ..." })), vec![("a.ts".into(), "const fresh = 2;\n".into())]);
        assert_eq!(edits("Write", &json!({ "file_path": "/r/b.py", "content": "print(1)" })), vec![("/r/b.py".into(), "print(1)".into())]);
        assert_eq!(edits("run_terminal_command", &json!({ "command": "ls" })), vec![(String::new(), "ls".into())]);
        assert!(edits("read_file", &json!({ "filepath": "a.ts" })).is_empty());
    }

    #[test]
    fn model_titles() {
        assert_eq!(model_id("Claude Sonnet 4.5"), "claude-sonnet-4.5");
        assert_eq!(model_id("gpt-5"), "gpt-5");
    }
}
