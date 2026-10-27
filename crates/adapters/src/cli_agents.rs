//! Native CLI transcripts verified against official sources on 2026-10-17.
//! Qoder: docs.qoder.com/cli/sessions and /extensions/hooks (Anthropic blocks).
//! iFlow: @iflow-ai/iflow-cli 0.5.19 bundle/iflow.js, MessageManager + SessionStorage.
//! CodeBuddy: @tencent-ai/codebuddy-code 2.168.2, PathUtils/SessionHistoryStore/AcpUtils.
//! Auggie: @augmentcode/auggie 0.36.0, SessionManager/SessionStore/AgentState.
//! Neovate: neovateai/neovate-code 0a24b363, src/{paths,message,session,project}.ts.
//! Droid: @factory/droid-sdk 0.9.1, session-discovery.ts and FactoryDroidMessageSchema.
//! Only matching successful results produce evidence; unknown models stay unknown.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Anthropic,
    Iflow,
    Neovate,
    Responses,
    Auggie,
    Cline,
    Droid,
}

#[derive(Clone)]
struct Source {
    src: Src,
    root: PathBuf,
    format: Format,
}

fn root(env: &str, official_env: Option<&str>, default: &str) -> PathBuf {
    std::env::var(env)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| official_env.and_then(|k| std::env::var(k).ok().filter(|s| !s.is_empty())))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(default))
}

fn sources() -> Vec<Source> {
    vec![
        Source {
            src: Src::Qoder,
            root: root("CREDITIA_QODER_DIR", Some("QODER_CONFIG_DIR"), ".qoder"),
            format: Format::Anthropic,
        },
        // CN keeps its own history, including a separate official config override.
        // https://docs.qoder.cn/en/cli/sessions
        Source {
            src: Src::Qoder,
            root: root("CREDITIA_QODER_CN_DIR", Some("QODERCN_CONFIG_DIR"), ".qoder-cn"),
            format: Format::Anthropic,
        },
        Source {
            src: Src::Iflow,
            root: root("CREDITIA_IFLOW_DIR", Some("IFLOW_HOME"), ".iflow"),
            format: Format::Iflow,
        },
        Source {
            src: Src::Neovate,
            root: root("CREDITIA_NEOVATE_DIR", None, ".neovate"),
            format: Format::Neovate,
        },
        Source {
            src: Src::CodeBuddy,
            root: root(
                "CREDITIA_CODEBUDDY_DIR",
                Some("CODEBUDDY_CONFIG_DIR"),
                ".codebuddy",
            ),
            format: Format::Responses,
        },
        Source {
            src: Src::Auggie,
            root: root("CREDITIA_AUGGIE_DIR", Some("AUGMENT_CACHE_DIR"), ".augment"),
            format: Format::Auggie,
        },
        Source {
            src: Src::Droid,
            root: root("CREDITIA_DROID_DIR", None, ".factory"),
            format: Format::Droid,
        },
    ]
}

fn files(s: &Source) -> Vec<PathBuf> {
    let dir = s
        .root
        .join(if matches!(s.format, Format::Auggie | Format::Droid) {
            "sessions"
        } else {
            "projects"
        });
    let ext = if s.format == Format::Auggie {
        "json"
    } else {
        "jsonl"
    };
    let mut out: Vec<_> = walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == ext))
        // HTTP request logs can contain credentials and duplicate generation inputs.
        .filter(|e| !e.path().components().any(|c| c.as_os_str() == "requests"))
        .filter(|e| !e.file_name().to_string_lossy().contains("-backup"))
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}

#[derive(Default)]
struct Parsed {
    id: String,
    cwd: String,
    branch: Option<String>,
    start: i64,
    end: i64,
    prompts: i64,
    tokens: i64,
    models: HashMap<String, usize>,
    edits: Vec<(String, String, String, i64)>,
}

fn string(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v[*k].as_str())
        .unwrap_or("")
        .to_string()
}
fn json_arg(v: &Value) -> Value {
    v.as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| v.clone())
}
fn abs(cwd: &str, path: &str) -> String {
    crate::harness_paths::target(path, cwd)
}

impl Parsed {
    fn time(&mut self, ts: i64) {
        if ts > 0 {
            self.start = if self.start == 0 {
                ts
            } else {
                self.start.min(ts)
            };
            self.end = self.end.max(ts);
        }
    }
    fn model(&mut self, model: &str) {
        if !model.is_empty() && model != "unknown" {
            *self.models.entry(model.into()).or_default() += 1;
        }
    }
    fn top_model(&self) -> String {
        self.models
            .iter()
            .max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone())))
            .map(|(k, _)| k.clone())
            .unwrap_or_default()
    }
    fn tool(&mut self, name: &str, args: &Value, cwd: &str, model: &str, ts: i64) {
        if matches!(name, "apply_patch" | "ApplyPatch" | "patch") {
            let patch = args
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| string(args, &["patch", "patchText", "input"]));
            for (path, text) in crate::apply_patch_files(&patch) {
                self.edits.push((abs(cwd, &path), text, model.into(), ts));
            }
            return;
        }
        let path = crate::path_arg(args).unwrap_or_default();
        if path.is_empty() {
            return;
        }
        let text = if name == "str-replace-editor" {
            match args["command"].as_str() {
                Some("insert") => Some(string(args, &["new_str"])),
                Some("str_replace") => {
                    let mut text = String::new();
                    if let Some(new) = args["new_str"].as_str() {
                        if let Some(old) = args["old_str"].as_str() { text.push_str(&added_lines(old, new)); }
                    }
                    let mut indices: Vec<_> = args
                        .as_object()
                        .into_iter()
                        .flat_map(|o| o.keys())
                        .filter_map(|k| k.strip_prefix("new_str_")?.parse::<usize>().ok())
                        .collect();
                    indices.sort();
                    for i in indices {
                        if let (Some(new),Some(old)) = (args[format!("new_str_{i}")].as_str(),args[format!("old_str_{i}")].as_str()) {
                            text.push_str(&added_lines(
                                old,
                                new,
                            ));
                        }
                    }
                    Some(text)
                }
                _ => None,
            }
        } else if name == "save-file" || name.to_lowercase().contains("write") || name.to_lowercase().contains("create") {
            // A successful full overwrite can retain arbitrary human context.
            // Only parse_auggie's verified new-file receipt permits full text.
            None
        } else if name == "edit_file" && args["new_text"].is_string() {
            args["new_text"].as_str().and_then(|s|args["old_text"].as_str().map(|old|added_lines(old,s)).or_else(||args["insert_line"].as_i64().map(|_|s.to_string())))
        } else if name == "replace" {
            args["new_string"]
                .as_str()
                .and_then(|s| args["old_string"].as_str().map(|old| added_lines(old,s)))
        } else {
            let old = ["old_string","oldString","old_str"].iter().any(|k|args[*k].is_string());
            let edits_have_old = args["edits"].as_array().is_some_and(|a|!a.is_empty()&&a.iter().all(|e|["old_string","oldString","old_str"].iter().any(|k|e[*k].is_string())));
            if old || edits_have_old { crate::new_text(name,args) } else { None }
        };
        if let Some(text) = text.filter(|s| !s.is_empty()) {
            self.edits.push((abs(cwd, &path), text, model.into(), ts));
        }
    }
}

fn parse_lines(text: &str, format: Format) -> Parsed {
    let mut p = Parsed::default();
    let records: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let mut ok = HashSet::new();
    for r in &records {
        let msg = if r["message"].is_object() {
            &r["message"]
        } else {
            r
        };
        for b in msg["content"].as_array().into_iter().flatten() {
            let id = match format {
                Format::Iflow
                    if b["type"] == "tool_result" && r["toolUseResult"]["status"] == "success" =>
                {
                    b["tool_use_id"].as_str()
                }
                Format::Neovate
                    if b["type"] == "tool-result"
                        && b["result"].is_object()
                        && b["result"]["isError"] != true =>
                {
                    b["toolCallId"].as_str()
                }
                Format::Neovate
                    if b["type"] == "tool_result"
                        && b["result"].is_object()
                        && b["result"]["isError"] != true =>
                {
                    b["id"].as_str()
                }
                Format::Anthropic if b["type"] == "tool_result" && b["is_error"] == false => {
                    b["tool_use_id"].as_str()
                }
                Format::Cline if b["type"] == "tool_result" && b["is_error"] != true => {
                    b["tool_use_id"].as_str()
                }
                Format::Droid if b["type"] == "tool_result" && b["isError"] != true => {
                    b["toolUseId"].as_str()
                }
                _ => None,
            };
            if let Some(id) = id.filter(|s| !s.is_empty()) {
                ok.insert(id.to_string());
            }
        }
        if format == Format::Responses
            && matches!(
                r["type"].as_str(),
                Some("function_call_result" | "function_call_output")
            )
        {
            let errored = [
                &r["providerData"]["error"],
                &r["providerData"]["toolResult"]["error"],
            ]
            .iter()
            .any(|e| !e.is_null() && **e != false && **e != "");
            if !errored
                && r["providerData"]["skipRun"] != true
                && matches!(r["status"].as_str(), None | Some("completed"))
            {
                if let Some(id) = r["callId"].as_str().filter(|s| !s.is_empty()) {
                    ok.insert(id.into());
                }
            }
        }
    }
    let mut current_model = String::new();
    for r in &records {
        let msg = if r["message"].is_object() {
            &r["message"]
        } else {
            r
        };
        let ts = any_ts(&r["timestamp"])
            .or_else(|| any_ts(&r["ts"]))
            .or_else(|| any_ts(&msg["createdAt"]))
            .unwrap_or(0);
        p.time(ts);
        if p.id.is_empty() {
            p.id = string(r, &["sessionId", "session_id"]);
        }
        if p.cwd.is_empty() {
            p.cwd = string(r, &["cwd"]);
        }
        let cwd = r["cwd"].as_str().unwrap_or(&p.cwd).to_string();
        if let Some(branch) = r["gitBranch"].as_str() {
            p.branch = Some(branch.into());
        }
        let role = msg["role"].as_str().or(r["type"].as_str()).unwrap_or("");
        if role == "user"
            && (msg["content"].is_string()
                || msg["content"].as_array().is_some_and(|a| {
                    a.iter()
                        .any(|b| matches!(b["type"].as_str(), Some("text" | "input_text")))
                }))
            && r["isCompactSummary"] != true
        {
            p.prompts += 1;
        }
        let model = msg["model"]
            .as_str()
            .or(msg["modelId"].as_str())
            .or(r["providerData"]["model"].as_str())
            .or(r["modelInfo"]["id"].as_str())
            .unwrap_or("");
        if role == "assistant" || !model.is_empty() {
            current_model = model.into();
            p.model(model);
        }
        if role == "assistant" {
            p.tokens += [
                "input_tokens",
                "output_tokens",
                "cache_read_input_tokens",
                "cache_creation_input_tokens",
            ]
            .iter()
            .map(|k| msg["usage"][*k].as_i64().unwrap_or(0))
            .sum::<i64>();
            if format == Format::Cline {
                p.tokens += [
                    "inputTokens",
                    "outputTokens",
                    "cacheReadTokens",
                    "cacheWriteTokens",
                ]
                .iter()
                .map(|k| r["metrics"][*k].as_i64().unwrap_or(0))
                .sum::<i64>();
            }
            for b in msg["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| b["type"] == "tool_use")
            {
                if ok.contains(b["id"].as_str().unwrap_or("")) {
                    p.tool(
                        b["name"].as_str().unwrap_or(""),
                        &b["input"],
                        &cwd,
                        &current_model,
                        ts,
                    );
                }
            }
        }
        if format == Format::Responses
            && r["type"] == "function_call"
            && ok.contains(r["callId"].as_str().unwrap_or(""))
        {
            p.tool(
                r["name"].as_str().unwrap_or(""),
                &json_arg(&r["arguments"]),
                &cwd,
                &current_model,
                ts,
            );
        }
    }
    p
}

fn parse_auggie(v: &Value) -> Parsed {
    let mut p = Parsed {
        id: string(v, &["sessionId"]),
        ..Default::default()
    };
    p.time(any_ts(&v["created"]).unwrap_or(0));
    p.time(any_ts(&v["modified"]).unwrap_or(0));
    let history: Vec<&Value> = v["chatHistory"].as_array().into_iter().flatten().collect();
    let mut ok = HashSet::new();
    let mut created = HashSet::new();
    for h in &history {
        for n in h["exchange"]["request_nodes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let result = &n["tool_result_node"];
            if result["is_error"] == false {
                if let Some(id) = result["tool_use_id"].as_str().filter(|s| !s.is_empty()) {
                    ok.insert(id.to_string());
                    // Official SaveFileTool rejects existing files, except an
                    // identical-content no-op success with a different receipt.
                    if result["content"].as_str().is_some_and(|s|s.starts_with("File saved.  Saved file {")) { created.insert(id.to_string()); }
                }
            }
            for folder in n["ide_state_node"]["workspace_folders"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if p.cwd.is_empty() {
                    p.cwd = string(folder, &["repository_root", "folder_root"]);
                }
            }
        }
    }
    for h in &history {
        let e = &h["exchange"];
        let model = e["model_id"].as_str().unwrap_or("");
        p.model(model);
        let ts = any_ts(&h["finishedAt"]).unwrap_or(0);
        p.time(ts);
        if e["request_message"].as_str().is_some_and(|s| !s.is_empty()) {
            p.prompts += 1;
        }
        for n in e["response_nodes"].as_array().into_iter().flatten() {
            let call = &n["tool_use"];
            if ok.contains(call["tool_use_id"].as_str().unwrap_or("")) {
                if call["tool_name"] == "save-file" {
                    let id=call["tool_use_id"].as_str().unwrap_or("");
                    let args=json_arg(&call["input_json"]);
                    if created.contains(id) {
                        if let (Some(path),Some(text))=(args["path"].as_str(),args["file_content"].as_str()) {
                            if !text.is_empty(){p.edits.push((abs(&p.cwd,path),text.into(),model.into(),ts));}
                        }
                    }
                    continue;
                }
                p.tool(
                    call["tool_name"].as_str().unwrap_or(""),
                    &json_arg(&call["input_json"]),
                    &p.cwd.clone(),
                    model,
                    ts,
                );
            }
        }
    }
    p
}

fn parse_file(s: &Source, f: &Path) -> Result<Parsed, std::io::Error> {
    let mut text = std::fs::read_to_string(f)?;
    if s.format == Format::Neovate {
        // Neovate deliberately omits cwd from messages; the official global
        // data.json indexes real paths, whereas the project directory is lossy.
        // Only a unique verified path-to-slug mapping can restore relative edits.
        if let Some(cwd) = neovate_cwd(&s.root, f) {
            text = text
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .map(|mut v| {
                    v["cwd"] = Value::String(cwd.clone());
                    v.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    let mut p = if s.format == Format::Auggie {
        parse_auggie(&serde_json::from_str(&text).unwrap_or_default())
    } else {
        parse_lines(&text, s.format)
    };
    if p.id.is_empty() {
        p.id = f.file_stem().unwrap_or_default().to_string_lossy().into();
    }
    p.edits.retain(|(path, _, _, _)| !path.is_empty());
    Ok(p)
}

fn neovate_cwd(base: &Path, file: &Path) -> Option<String> {
    let slug = file
        .strip_prefix(base.join("projects"))
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()?;
    let data: Value = std::fs::read_to_string(base.join("data.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    let re = regex::Regex::new("[^a-zA-Z0-9]+").ok()?;
    let matching: Vec<_> = data["projects"]
        .as_object()?
        .keys()
        .filter(|path| {
            re.replace_all(path.trim_matches('/'), "-")
                .trim_matches('-')
                .to_ascii_lowercase()
                == slug
        })
        .cloned()
        .collect();
    if matching.len() == 1 {
        matching.into_iter().next()
    } else {
        None
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    scan_cline(store, pm, st);
    scan_tabnine(store, pm, st);
    for s in sources() {
        let mut count = 0;
        for f in files(&s) {
            let key = format!("{}-cli:{}", s.src.harness(), f.display());
            let fp = fingerprint(&f);
            if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
                count += 1;
                continue;
            }
            let p = match parse_file(&s, &f) {
                Ok(p) => p,
                Err(e) => {
                    st.warnings.push(format!(
                        "{}: cannot read {}: {e}",
                        s.src.harness(),
                        f.display()
                    ));
                    continue;
                }
            };
            // Absolute tool paths also establish project membership when older logs omit cwd.
            if !pm.in_project(&p.cwd)
                && !p
                    .edits
                    .iter()
                    .any(|(path, _, _, _)| pm.to_rel(path).is_some())
            {
                continue;
            }
            let mut g = FileGroup {
                fp,
                ..Default::default()
            };
            for (path, text, model, ts) in &p.edits {
                crate::push_text(&mut g.evs, pm, path, text, s.src, model, &p.id, *ts);
            }
            g.sessions.push(Session {
                src: s.src,
                id: p.id.clone(),
                model: p.top_model(),
                start: p.start,
                end: p.end,
                tokens: p.tokens,
                branch: p.branch,
                prompts: p.prompts,
                reviewer: false,
                has_content: true,
                provider: None,
            });
            store.groups.insert(key, g);
            count += 1;
        }
        if count > 0 {
            st.notes.push(format!(
                "{}-cli: {count} sessions for this project; only successful tool results counted",
                s.src.harness()
            ));
        }
    }
}

pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<(Src, String), Activity> = HashMap::new();
    for s in sources() {
        for f in files(&s) {
            if let Ok(p) = parse_file(&s, &f) {
                if !p.cwd.is_empty() {
                    let e = out.entry((s.src, p.cwd.clone())).or_insert(Activity {
                        src: s.src,
                        cwd: p.cwd,
                        remote: None,
                        sessions: 0,
                        last: 0,
                    });
                    e.sessions += 1;
                    e.last = e.last.max(p.end);
                }
            }
        }
    }
    for c in cline_sessions() {
        if !c.cwd.is_empty() {
            let e = out.entry((Src::Cline, c.cwd.clone())).or_insert(Activity {
                src: Src::Cline,
                cwd: c.cwd,
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(c.end);
        }
    }
    for (f, cwd) in tabnine_files() {
        let v = crate::gemini::load(&f);
        let e = out.entry((Src::Tabnine, cwd.clone())).or_insert(Activity {
            src: Src::Tabnine,
            cwd,
            remote: None,
            sessions: 0,
            last: 0,
        });
        e.sessions += 1;
        e.last = e.last.max(any_ts(&v["lastUpdated"]).unwrap_or(0));
    }
    out.into_values().collect()
}

pub fn doctor() -> Vec<Health> {
    let mut out: Vec<_> = sources().iter().map(|s| {
        let parsed: Vec<_> = files(s).iter().filter_map(|f| parse_file(s, f).ok()).collect();
        Health { source: format!("{}-cli", s.src.harness()), path: s.root.display().to_string(), readable: !parsed.is_empty(),
            format: match s.format { Format::Auggie => "sessions/*.json exchange tool_use/tool_result_node", Format::Droid => "sessions/**/*.jsonl session_start + FactoryDroidMessage tool_use/tool_result", _ => "projects/**/*.jsonl native tool calls/results" }.into(),
            records: parsed.len() as u64, earliest: parsed.iter().map(|p| p.start).filter(|&t| t > 0).min().map(date_cn), latest: parsed.iter().map(|p| p.end).filter(|&t| t > 0).max().map(date_cn),
            note: creditia_core::tr!("已核对官方格式；仅通过构造记录测试，尚无本机真实会话核验。未记录模型时保留未知；缺少覆盖前原文或明确新建回执的整文件写入不计新增行。", "Official format verified; fixture tests only, no local real-session validation. Unrecorded models remain unknown; whole-file writes without prior content or explicit creation receipts do not produce added-line evidence."), ..Default::default() }
    }).collect();
    let cline = cline_sessions();
    out.push(Health { source: "cline-cli".into(), path: cline_root().display().to_string(), readable: cline.iter().any(|s| std::fs::File::open(&s.file).is_ok()), format: "data/db/sessions.db + data/sessions/<session>/*.messages.json".into(), records: cline.len() as u64, note: creditia_core::tr!("官方 Cline SDK 01b8a972 的原生工具调用/结果与逐消息 modelInfo；已通过构造记录测试，尚无本机真实会话核验。", "Official Cline SDK 01b8a972 native tool_use/tool_result and per-message modelInfo; fixture validated, no local real-session validation."), ..Default::default() });
    let tabnine = tabnine_files();
    out.push(Health { source: "tabnine-cli".into(), path: tabnine_root().display().to_string(), readable: tabnine.iter().any(|(p, _)| std::fs::File::open(p).is_ok()), format: "tmp/<project>/chats/session-*.json[l] + .project_root or projects.json".into(), records: tabnine.len() as u64, note: creditia_core::tr!("官方 legacy CLI 0.36.0；只读取成功 toolCalls。IDE Agent 与较新的 OpenCode 插件是其他入口。已通过构造记录测试，尚无本机真实会话核验。", "Official legacy CLI 0.36.0; successful toolCalls only. IDE Agent and newer OpenCode plugin are separate entrances. Fixture validated, no local real-session validation."), ..Default::default() });
    out
}

fn cline_root() -> PathBuf {
    root("CREDITIA_CLINE_CLI_DIR", Some("CLINE_DIR"), ".cline")
}
struct ClineSession {
    file: PathBuf,
    id: String,
    cwd: String,
    start: i64,
    end: i64,
}
fn cline_sessions() -> Vec<ClineSession> {
    let base = cline_root();
    let data = std::env::var("CLINE_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("data"));
    let dbdir = std::env::var("CLINE_DB_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("db"));
    let mut out = vec![];
    if let Ok(c) = crate::open_ro(&dbdir.join("sessions.db")) {
        if let Ok(mut q) = c.prepare("SELECT session_id, cwd, workspace_root, started_at, updated_at, messages_path FROM sessions") {
            let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, Option<String>>(5)?)));
            for (id, cwd, workspace, start, end, messages) in rows.into_iter().flatten().flatten() {
                let file = messages.filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| data.join("sessions").join(&id).join(format!("{id}.messages.json")));
                out.push(ClineSession { file, id, cwd: if cwd.is_empty() { workspace } else { cwd }, start: any_ts(&Value::String(start)).unwrap_or(0), end: any_ts(&Value::String(end)).unwrap_or(0) });
            }
        }
    }
    out
}
fn scan_cline(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for s in cline_sessions()
        .into_iter()
        .filter(|s| pm.in_project(&s.cwd))
    {
        let text = match std::fs::read_to_string(&s.file) {
            Ok(t) => t,
            Err(e) => {
                st.warnings
                    .push(format!("cline-cli: cannot read {}: {e}", s.file.display()));
                continue;
            }
        };
        let v: Value = serde_json::from_str(&text).unwrap_or_default();
        let lines = v["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let mut p = parse_lines(&lines, Format::Cline);
        // edit_file accepts relative paths. Metadata is authoritative for cwd.
        if p.cwd.is_empty() {
            let lines = v["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned()
                .map(|mut m| {
                    m["cwd"] = Value::String(s.cwd.clone());
                    m
                })
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            p = parse_lines(&lines, Format::Cline);
        }
        let mut g = FileGroup {
            fp: fingerprint(&s.file),
            ..Default::default()
        };
        for (path, text, model, ts) in &p.edits {
            crate::push_text(&mut g.evs, pm, path, text, Src::Cline, model, &s.id, *ts);
        }
        g.sessions.push(Session {
            src: Src::Cline,
            id: s.id.clone(),
            model: p.top_model(),
            start: if p.start > 0 { p.start } else { s.start },
            end: p.end.max(s.end),
            tokens: p.tokens,
            branch: None,
            prompts: p.prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
        store
            .groups
            .insert(format!("cline-cli:{}:{}", s.file.display(), s.id), g);
    }
}

// Tabnine's official legacy CLI 0.36.0 uses Gemini's recording codec, but a
// distinct config root and `tabnine` message type. Do not scan ~/.gemini here.
fn tabnine_root() -> PathBuf {
    root("CREDITIA_TABNINE_CLI_DIR", None, ".tabnine/agent")
}
fn tabnine_files() -> Vec<(PathBuf, String)> {
    let base = tabnine_root();
    let registry: Value = std::fs::read_to_string(base.join("projects.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut out = vec![];
    for project in std::fs::read_dir(base.join("tmp"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = project.path();
        let cwd = std::fs::read_to_string(path.join(".project_root"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                registry["projects"]
                    .as_object()?
                    .iter()
                    .find(|(_, v)| v.as_str() == project.file_name().to_str())
                    .map(|(k, _)| k.clone())
            });
        let Some(cwd) = cwd else {
            continue;
        };
        for e in walkdir::WalkDir::new(path.join("chats"))
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let f = e.into_path();
            if f.extension().is_some_and(|x| x == "jsonl" || x == "json")
                && !(f.extension().is_some_and(|x| x == "json")
                    && f.with_extension("jsonl").is_file())
            {
                out.push((f, cwd.clone()));
            }
        }
    }
    out.sort();
    out
}
fn scan_tabnine(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    for (f, cwd) in tabnine_files()
        .into_iter()
        .filter(|(_, cwd)| pm.in_project(cwd))
    {
        let key = format!("tabnine-cli:{}", f.display());
        let fp = fingerprint(&f);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let v = crate::gemini::load(&f);
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        crate::gemini::parse(Src::Tabnine, &v, &cwd, pm, &mut g);
        store.groups.insert(key, g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn lines(records: &[Value]) -> String {
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
    #[test]
    fn successful_qoder_edits_keep_their_real_project_boundary() {
        let rows = [
            json!({"sessionId":"s","cwd":"/repo","type":"assistant","message":{"model":"model-a","role":"assistant","content":[{"type":"tool_use","id":"outside","name":"Edit","input":{"file_path":"../other/x.rs","old_string":"old line","new_string":"outside generated line"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"outside","is_error":false}]}}),
        ];
        let p = parse_lines(&lines(&rows), Format::Anthropic);
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].0, "/other/x.rs");
        let pm = PathMap { known: ["x.rs".into()].into(), markers: vec!["/repo/".into()], excludes: vec![] };
        assert!(pm.to_rel(&p.edits[0].0).is_none());
    }
    #[test]
    fn successful_whole_writes_without_before_are_not_all_generated_lines() {
        let mut p=Parsed::default();
        for name in ["Write","write_file","Create","save-file","edit"] {
            p.tool(name,&json!({"path":"/repo/x.rs","file_path":"/repo/x.rs","content":"human context\nnew line","file_content":"human context\nnew line"}),"/repo","m",1);
        }
        p.tool("edit_file",&json!({"path":"/repo/x.rs","new_text":"human context\nnew line"}),"/repo","m",1);
        assert!(p.edits.is_empty());
    }
    #[test]
    fn auggie_save_file_needs_actual_creation_receipt_not_existing_identical_success() {
        let v=json!({"sessionId":"a","chatHistory":[{"exchange":{"model_id":"m","request_nodes":[{"ide_state_node":{"workspace_folders":[{"folder_root":"/repo"}]}}],"response_nodes":[{"tool_use":{"tool_use_id":"new","tool_name":"save-file","input_json":{"path":"new.rs","file_content":"new generated line"}}},{"tool_use":{"tool_use_id":"same","tool_name":"save-file","input_json":{"path":"human.rs","file_content":"retained human line"}}}]}},{"exchange":{"request_nodes":[{"tool_result_node":{"tool_use_id":"new","is_error":false,"content":"File saved.  Saved file {/repo/new.rs}"}},{"tool_result_node":{"tool_use_id":"same","is_error":false,"content":"File already exists and content is the same: {/repo/human.rs}"}}]}}]});
        let p=parse_auggie(&v);assert_eq!(p.edits,vec![("/repo/new.rs".into(),"new generated line".into(),"m".into(),0)]);
    }
    #[test]
    fn qoder_requires_explicit_success_and_preserves_per_call_model() {
        let rows = [
            json!({"sessionId":"s","cwd":"/repo","type":"assistant","message":{"model":"model-a","role":"assistant","content":[{"type":"tool_use","id":"ok","name":"Edit","input":{"file_path":"x.rs","old_string":"old line","new_string":"successful line"}},{"type":"tool_use","id":"bad","name":"Edit","input":{"file_path":"x.rs","old_string":"old line","new_string":"failed line"}},{"type":"tool_use","id":"missing","name":"Edit","input":{"file_path":"x.rs","old_string":"old line","new_string":"unconfirmed line"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"ok","is_error":false},{"type":"tool_result","tool_use_id":"bad","is_error":true}]}}),
        ];
        let p = parse_lines(&lines(&rows), Format::Anthropic);
        assert_eq!(
            p.edits,
            vec![(
                "/repo/x.rs".into(),
                "successful line\n".into(),
                "model-a".into(),
                0
            )]
        );
    }
    #[test]
    fn iflow_uses_record_status_not_missing_block_error_flag() {
        let rows = [
            json!({"sessionId":"s","cwd":"/repo","type":"assistant","message":{"model":"iflow-model","role":"assistant","content":[{"type":"tool_use","id":"ok","name":"edit","input":{"file_path":"x.rs","old_string":"keep","new_string":"keep\nadded line"}},{"type":"tool_use","id":"bad","name":"write_file","input":{"file_path":"x.rs","content":"denied line"}}]}}),
            json!({"type":"user","toolUseResult":{"status":"success"},"message":{"content":[{"type":"tool_result","tool_use_id":"ok"}]}}),
            json!({"type":"user","toolUseResult":{"status":"error"},"message":{"content":[{"type":"tool_result","tool_use_id":"bad"}]}}),
        ];
        let p = parse_lines(&lines(&rows), Format::Iflow);
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].1, "added line\n");
    }
    #[test]
    fn neovate_result_error_and_unmatched_calls_are_excluded() {
        let rows = [
            json!({"type":"message","role":"assistant","model":"n","content":[{"type":"tool_use","id":"yes","name":"edit","input":{"file_path":"/repo/x.rs","old_string":"old line","new_string":"written line"}},{"type":"tool_use","id":"no","name":"edit","input":{"file_path":"/repo/x.rs","old_string":"old line","new_string":"denied line"}}]}),
            json!({"type":"message","role":"tool","content":[{"type":"tool-result","toolCallId":"yes","result":{"llmContent":"saved"}},{"type":"tool-result","toolCallId":"no","result":{"isError":true,"llmContent":"denied"}}]}),
        ];
        let p = parse_lines(&lines(&rows), Format::Neovate);
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].1, "written line\n");
    }
    #[test]
    fn codebuddy_business_error_overrides_completed_status() {
        let rows = [
            json!({"type":"message","role":"assistant","cwd":"/repo","providerData":{"model":"cb-model"}}),
            json!({"type":"function_call","callId":"yes","name":"Edit","arguments":"{\"file_path\":\"/repo/x.rs\",\"old_string\":\"old line\",\"new_string\":\"written line\"}"}),
            json!({"type":"function_call_result","callId":"yes","status":"completed"}),
            json!({"type":"function_call","callId":"no","name":"Write","arguments":{"file_path":"/repo/x.rs","content":"failed line"}}),
            json!({"type":"function_call_result","callId":"no","status":"completed","providerData":{"toolResult":{"error":"write failed"}}}),
        ];
        let p = parse_lines(&lines(&rows), Format::Responses);
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].2, "cb-model");
    }
    #[test]
    fn auggie_pairs_later_exchange_results_and_numbered_replacements() {
        let v = json!({"sessionId":"a","agentState":{"modelId":"current-model-must-not-relabel-history"},"chatHistory":[{"exchange":{"model_id":"actual-model","request_message":"go","request_nodes":[{"ide_state_node":{"workspace_folders":[{"repository_root":"/repo"}]}}],"response_nodes":[{"tool_use":{"tool_use_id":"ok","tool_name":"str-replace-editor","input_json":"{\"command\":\"str_replace\",\"path\":\"x.rs\",\"old_str_1\":\"keep\",\"new_str_1\":\"keep\\nnew line\"}"}},{"tool_use":{"tool_use_id":"bad","tool_name":"save-file","input_json":{"path":"x.rs","file_content":"bad line"}}}]}},{"exchange":{"request_nodes":[{"tool_result_node":{"tool_use_id":"ok","is_error":false}},{"tool_result_node":{"tool_use_id":"bad","is_error":true}}]}}]});
        let p = parse_auggie(&v);
        assert_eq!(
            p.edits,
            vec![(
                "/repo/x.rs".into(),
                "new line\n".into(),
                "actual-model".into(),
                0
            )]
        );
    }
    #[test]
    fn droid_native_camel_case_receipts_and_models() {
        let rows = [
            json!({"type":"session_start","sessionId":"droid-session","title":"title","cwd":"/repo"}),
            json!({"type":"message","message":{"id":"m1","role":"assistant","modelId":"concrete-router-model","createdAt":1790000000000i64,"content":[{"type":"tool_use","id":"a","name":"Edit","input":{"file_path":"x.rs","old_str":"old line","new_str":"written line"}},{"type":"tool_use","id":"b","name":"Edit","input":{"file_path":"x.rs","old_str":"written line","new_str":"denied line"}}]}}),
            json!({"type":"message","message":{"id":"m2","role":"user","createdAt":1790000001000i64,"content":[{"type":"tool_result","toolUseId":"a","content":"created"},{"type":"tool_result","toolUseId":"b","isError":true,"content":"denied"}]}}),
        ];
        let p = parse_lines(&lines(&rows), Format::Droid);
        assert_eq!(p.id, "droid-session");
        assert_eq!(
            p.edits,
            vec![(
                "/repo/x.rs".into(),
                "written line\n".into(),
                "concrete-router-model".into(),
                1790000000
            )]
        );
    }
    #[test]
    fn cline_sdk_edit_file_uses_model_at_message_and_real_new_text() {
        let rows = [
            json!({"role":"assistant","cwd":"/repo","ts":1790000000000i64,"modelInfo":{"id":"cline-generation-model","provider":"anthropic"},"metrics":{"inputTokens":7,"outputTokens":3},"content":[{"type":"tool_use","id":"a","name":"edit_file","input":{"path":"x.rs","old_text":"keep","new_text":"keep\nnew line"}}]}),
            json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"a","content":"saved"}]}),
            json!({"role":"assistant","cwd":"/repo","content":[{"type":"tool_use","id":"b","name":"edit_file","input":{"path":"x.rs","insert_line":1,"new_text":"unknown model line"}}]}),
            json!({"role":"user","content":[{"type":"tool_result","tool_use_id":"b","is_error":false,"content":"saved"}]}),
        ];
        let p = parse_lines(&lines(&rows), Format::Cline);
        assert_eq!(p.edits.len(), 2);
        assert_eq!(p.edits[0].1, "new line\n");
        assert_eq!(p.edits[0].2, "cline-generation-model");
        assert!(p.edits[1].2.is_empty());
        assert_eq!(p.tokens, 10);
    }
    #[test]
    fn tabnine_native_type_requires_explicit_success() {
        let pm = PathMap {
            known: ["x.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let v = json!({"sessionId":"tn","startTime":"2026-10-01T00:00:00Z","messages":[{"type":"tabnine","model":"tn-model","toolCalls":[{"name":"replace","status":"success","args":{"file_path":"/repo/x.rs","old_string":"old line","new_string":"accepted line"}},{"name":"replace","status":"error","args":{"file_path":"/repo/x.rs","old_string":"old line","new_string":"failed line"}},{"name":"replace","args":{"file_path":"/repo/x.rs","old_string":"old line","new_string":"pending line"}},{"name":"write_file","status":"success","args":{"file_path":"/repo/x.rs","content":"human retained context"}}]}]});
        let mut g = FileGroup::default();
        crate::gemini::parse(Src::Tabnine, &v, "/repo", &pm, &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "tn-model");
        assert_eq!(g.evs[0].src, Src::Tabnine);
    }
    #[test]
    fn neovate_lossy_directory_must_match_unique_authoritative_project() {
        let base = std::env::temp_dir().join(format!("creditia-neovate-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("data.json"),
            json!({"projects":{"/Work/My Repo":{"history":[]}}}).to_string(),
        )
        .unwrap();
        assert_eq!(
            neovate_cwd(&base, &base.join("projects/work-my-repo/session.jsonl")),
            Some("/Work/My Repo".into())
        );
        std::fs::write(
            base.join("data.json"),
            json!({"projects":{"/Work/My Repo":{},"/work/my-repo":{}}}).to_string(),
        )
        .unwrap();
        assert_eq!(
            neovate_cwd(&base, &base.join("projects/work-my-repo/session.jsonl")),
            None
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
