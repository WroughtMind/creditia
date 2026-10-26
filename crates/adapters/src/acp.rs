//! Opt-in ACP stdio capture. Only completed diff / successful fs writes become evidence.
//! https://agentclientprotocol.com/protocol/v1/tool-calls
//! https://agentclientprotocol.com/protocol/v1/file-system
//! Capture never changes protocol messages, capabilities, authentication or permissions.

use crate::{Activity, Health, Stats};
use creditia_core::model::{Ev, FileGroup, PathMap, Session, Src, Store};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// These entries have an official ACP launch entry. This is capture support, not a
/// claim that their private historical stores or every released version were tested.
pub const CAPTURE_SOURCES: &[Src] = &[
    Src::Cortex,
    Src::Bob,
    Src::Poolside,
    Src::Autohand,
    Src::VtCode,
    Src::Kimchi,
    Src::Nova,
    Src::GlmAgent,
    Src::Dirac,
    Src::Corust,
    Src::Crow,
    Src::DimCode,
    Src::MinionCode,
    Src::SiGit,
    Src::Stakpak,
];

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rpc_key(direction: &str, id: &Value) -> Option<String> {
    if id.is_string() || id.is_number() {
        Some(format!("{direction}:{id}"))
    } else {
        None
    }
}

fn model_options(v: &Value) -> Value {
    Value::Array(v.as_array().into_iter().flatten().filter(|v| v["category"] == "model" || v["id"] == "model")
        .filter_map(|v| Some(json!({"id": v["id"].as_str()?, "category": "model", "currentValue": v["currentValue"].as_str()?}))).collect())
}

/// Retain only attribution fields; never write authentication, prompt text,
/// terminal commands, MCP server environment, permission payloads or raw tool output.
pub struct Recorder {
    file: File,
    pending: HashMap<String, String>,
}

impl Recorder {
    pub fn new(file: File) -> Self {
        Self {
            file,
            pending: HashMap::new(),
        }
    }

    fn sanitize(&mut self, direction: &str, v: &Value) -> Option<Value> {
        let id = &v["id"];
        if !id.is_null() && rpc_key(direction, id).is_none() {
            return None;
        }
        if let Some(method) = v["method"].as_str() {
            let p = &v["params"];
            let sid = p["sessionId"].as_str();
            let params = match (direction, method) {
                ("client", "session/new") => json!({"cwd":p["cwd"].as_str()?}),
                ("client", "session/load" | "session/resume") => {
                    json!({"sessionId":sid?,"cwd":p["cwd"].as_str()?})
                }
                ("client", "session/prompt") => json!({"sessionId":sid?}),
                ("client", "session/set_model") => {
                    json!({"sessionId":sid?,"modelId":p["modelId"].as_str()?})
                }
                ("client", "session/set_config_option") => json!({"sessionId":sid?}),
                ("agent", "fs/read_text_file") => {
                    json!({"sessionId":sid?,"path":p["path"].as_str()?,"line":p["line"].as_u64(),"limit":p["limit"].as_u64()})
                }
                ("agent", "fs/write_text_file") => {
                    json!({"sessionId":sid?,"path":p["path"].as_str()?,"content":p["content"].as_str()?})
                }
                ("agent", "session/update") => {
                    let u = &p["update"];
                    let update = match u["sessionUpdate"].as_str()? {
                        "tool_call" | "tool_call_update" => {
                            let mut x = json!({"sessionUpdate":u["sessionUpdate"].as_str()?,"toolCallId":u["toolCallId"].as_str()?});
                            for key in ["status", "kind"] {
                                if let Some(s) = u[key].as_str() {
                                    x[key] = s.into();
                                }
                            }
                            if let Some(content) = u["content"].as_array() {
                                x["content"] = Value::Array(content.iter().filter(|c| c["type"] == "diff").filter_map(|c| {
                                    let old=c.get("oldText")?;if !old.is_null()&&!old.is_string(){return None;}
                                    Some(json!({"type":"diff","path":c["path"].as_str()?,"oldText":old,"newText":c["newText"].as_str()?}))
                                }).collect());
                            }
                            x
                        }
                        "config_option_update" => {
                            json!({"sessionUpdate":"config_option_update","configOptions":model_options(&u["configOptions"])})
                        }
                        "current_model_update" => {
                            json!({"sessionUpdate":"current_model_update","currentModelId":u["currentModelId"].as_str()?})
                        }
                        _ => return None,
                    };
                    json!({"sessionId":sid?,"update":update})
                }
                _ => return None,
            };
            if let Some(key) = rpc_key(direction, id) {
                self.pending.insert(key, method.into());
            }
            let mut out = json!({"jsonrpc":"2.0","method":method,"params":params});
            if !id.is_null() {
                out["id"] = id.clone();
            }
            return Some(out);
        }
        let other = if direction == "agent" {
            "client"
        } else {
            "agent"
        };
        let method = self.pending.remove(&rpc_key(other, id)?)?;
        if !v["error"].is_null() {
            return Some(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":v["error"]["code"].as_i64()}}),
            );
        }
        let r = v.get("result").filter(|r| r.is_object())?;
        let result = match method.as_str() {
            "session/new"
            | "session/load"
            | "session/resume"
            | "session/set_model"
            | "session/set_config_option" => {
                json!({"sessionId":r["sessionId"].as_str(),"models":{"currentModelId":r["models"]["currentModelId"].as_str()},"configOptions":model_options(&r["configOptions"])})
            }
            "fs/read_text_file" => json!({"content":r["content"].as_str()?}),
            "fs/write_text_file" | "session/prompt" => json!({}),
            _ => return None,
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }

    pub fn record(&mut self, direction: &str, bytes: &[u8]) -> Result<(), String> {
        let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
            return Ok(());
        };
        if let Some(message) = self.sanitize(direction, &v) {
            serde_json::to_writer(
                &mut self.file,
                &json!({"direction":direction,"at":now_ms(),"message":message}),
            )
            .map_err(|e| e.to_string())?;
            self.file
                .write_all(b"\n")
                .and_then(|_| self.file.flush())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Run as an ACP agent command in an existing ACP client. Stdout is protocol only.
/// Exit status belongs to the child. Captures are local and explicitly opted into.
pub fn capture(src: Src, command: &[String], out: Option<&Path>) -> Result<i32, String> {
    let (exe, args) = command
        .split_first()
        .ok_or("capture requires an ACP agent command after --")?;
    let path = out.map(PathBuf::from).unwrap_or_else(|| {
        crate::registry::capture_dir().join(format!(
            "{}-{}-{}.jsonl",
            src.harness(),
            now_ms(),
            std::process::id()
        ))
    });
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(parent)
            .map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, &json!({"type":"creditia-acp","version":1,"harness":src.harness(),"cwd":cwd,"startedAt":now_ms()})).map_err(|e| e.to_string())?;
    file.write_all(b"\n")
        .and_then(|_| file.flush())
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("{exe}: {e}"))?;
    let mut input = child.stdin.take().ok_or("agent stdin unavailable")?;
    let output = child.stdout.take().ok_or("agent stdout unavailable")?;
    let recorder = Arc::new(Mutex::new(Recorder::new(file)));
    let input_recorder = recorder.clone();
    let (input_errors, receive_error) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut line = Vec::new();
        loop {
            line.clear();
            match stdin.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Err(e) => {
                    let _ = input_errors.send(e.to_string());
                    break;
                }
                Ok(_) => {
                    let result = input_recorder
                        .lock()
                        .map_err(|_| "capture lock poisoned".to_string())
                        .and_then(|mut r| r.record("client", &line));
                    if let Err(e) = result {
                        let _ = input_errors.send(e);
                        break;
                    }
                    if input.write_all(&line).and_then(|_| input.flush()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut reader = BufReader::new(output);
    let mut stdout = std::io::stdout().lock();
    let mut line = Vec::new();
    let transfer = (|| -> Result<(), String> {
        loop {
            line.clear();
            if reader
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                break;
            }
            recorder
                .lock()
                .map_err(|_| "capture lock poisoned".to_string())?
                .record("agent", &line)?;
            stdout
                .write_all(&line)
                .and_then(|_| stdout.flush())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(e) = transfer {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if let Ok(e) = receive_error.try_recv() {
        return Err(e);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    #[cfg(not(unix))]
    {
        Ok(status.code().unwrap_or(1))
    }
}

#[derive(Default)]
struct State {
    cwd: String,
    model: String,
    start: i64,
    end: i64,
    prompts: i64,
    replaying: bool,
}

struct Pending {
    method: String,
    params: Value,
    model: String,
    sequence: usize,
}
#[derive(Default)]
struct Tool {
    model: String,
    status: String,
    content: Vec<Value>,
    ts: i64,
}

struct Parsed {
    src: Src,
    groups: HashMap<String, FileGroup>,
    activities: Vec<Activity>,
    calls: Vec<CapturedCall>,
}

struct CapturedCall {
    sid: String,
    id: String,
    evs: Vec<Ev>,
}

fn model_from(v: &Value) -> Option<String> {
    model_options(&v["configOptions"])
        .as_array()?
        .iter()
        .find_map(|v| v["currentValue"].as_str().map(String::from))
        .or_else(|| v["models"]["currentModelId"].as_str().map(String::from))
}

fn absolute(cwd: &str, path: &str) -> String {
    if Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':') {
        path.into()
    } else {
        format!("{}/{}", cwd.replace('\\', "/").trim_end_matches('/'), path)
    }
}

fn parse(rows: &[Value], pm: &PathMap) -> Option<Parsed> {
    let header = rows.first()?;
    if header["type"] != "creditia-acp" || header["version"] != 1 {
        return None;
    }
    let src = Src::from_harness(header["harness"].as_str()?)?;
    let mut states: HashMap<String, State> = HashMap::new();
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut tools: HashMap<(String, String), Tool> = HashMap::new();
    let mut prior: HashMap<(String, String), String> = HashMap::new();
    // Match an echoed filesystem transition once, not every equal line in a session.
    let mut last_fs: HashMap<(String, String), (String, String, usize)> = HashMap::new();
    let mut echoed_fs = HashSet::new();
    let mut groups: HashMap<String, FileGroup> = HashMap::new();
    let mut calls = vec![];
    let capture_id = header.to_string();
    for (sequence, row) in rows[1..].iter().enumerate() {
        let direction = row["direction"].as_str().unwrap_or("");
        let at = crate::any_ts(&row["at"]).unwrap_or(0);
        let v = &row["message"];
        let p = &v["params"];
        let sid = p["sessionId"].as_str().unwrap_or("").to_string();
        if !sid.is_empty() {
            let state = states.entry(sid.clone()).or_default();
            if state.cwd.is_empty() {
                state.cwd = header["cwd"].as_str().unwrap_or("").into();
            }
            if at > 0 {
                state.start = if state.start == 0 {
                    at
                } else {
                    state.start.min(at)
                };
                state.end = state.end.max(at);
            }
        }
        if let Some(method) = v["method"].as_str() {
            if direction == "agent" && method == "session/update" {
                if sid.is_empty() {
                    continue;
                }
                let state = states.entry(sid.clone()).or_default();
                let u = &p["update"];
                match u["sessionUpdate"].as_str() {
                    Some("config_option_update") => {
                        if let Some(model) = model_from(u) {
                            state.model = model;
                        }
                    }
                    Some("current_model_update") => {
                        if let Some(model) = u["currentModelId"].as_str() {
                            state.model = model.into();
                        }
                    }
                    Some("tool_call" | "tool_call_update") => {
                        let Some(id) = u["toolCallId"].as_str() else {
                            continue;
                        };
                        let tool = tools
                            .entry((sid.clone(), id.into()))
                            .or_insert_with(|| Tool {
                                model: state.model.clone(),
                                ..Default::default()
                            });
                        let already_completed = tool.status == "completed";
                        if let Some(status) = u["status"].as_str() {
                            tool.status = status.into();
                        }
                        if let Some(content) = u["content"].as_array() {
                            tool.content = content.clone();
                        }
                        tool.ts = if state.replaying { 0 } else { at };
                        if tool.status == "completed" && !already_completed && !state.replaying {
                            for diff in &tool.content {
                                if let (Some(path), Some(text)) =
                                    (diff["path"].as_str(), diff["newText"].as_str())
                                {
                                    let key = (sid.clone(), absolute(&state.cwd, path));
                                    if let Some((old, new, index)) = last_fs.remove(&key) {
                                        let before = diff.get("oldText").and_then(|v| {
                                            if v.is_null() {
                                                Some("")
                                            } else {
                                                v.as_str()
                                            }
                                        });
                                        if before == Some(old.as_str()) && new == text {
                                            echoed_fs.insert(index);
                                        }
                                    }
                                    prior.insert(key, text.into());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if direction == "client" && matches!(method, "session/load" | "session/resume") {
                if let Some(cwd) = p["cwd"].as_str() {
                    states.entry(sid.clone()).or_default().cwd = cwd.into();
                }
                states.entry(sid.clone()).or_default().replaying = method == "session/load";
            }
            if let Some(key) = rpc_key(direction, &v["id"]) {
                pending.insert(
                    key,
                    Pending {
                        method: method.into(),
                        params: p.clone(),
                        model: states
                            .get(&sid)
                            .map(|s| s.model.clone())
                            .unwrap_or_default(),
                        sequence,
                    },
                );
            }
            continue;
        }
        let other = if direction == "agent" {
            "client"
        } else {
            "agent"
        };
        let Some(key) = rpc_key(other, &v["id"]) else {
            continue;
        };
        let Some(req) = pending.remove(&key) else {
            continue;
        };
        if v.get("error").is_some() || v.get("result").is_none() {
            if req.method == "session/load" {
                if let Some(s) = req.params["sessionId"]
                    .as_str()
                    .and_then(|s| states.get_mut(s))
                {
                    s.replaying = false;
                }
            }
            continue;
        }
        let r = &v["result"];
        if !r.is_object() {
            continue;
        }
        let sid = req.params["sessionId"]
            .as_str()
            .or_else(|| r["sessionId"].as_str())
            .unwrap_or("")
            .to_string();
        if sid.is_empty() {
            continue;
        }
        let state = states.entry(sid.clone()).or_default();
        if at > 0 {
            state.start = if state.start == 0 {
                at
            } else {
                state.start.min(at)
            };
            state.end = state.end.max(at);
        }
        match req.method.as_str() {
            "session/new" | "session/load" | "session/resume" => {
                state.replaying = false;
                if let Some(cwd) = req.params["cwd"].as_str() {
                    state.cwd = cwd.into();
                }
                if let Some(model) = model_from(r) {
                    state.model = model;
                }
            }
            "session/set_model" => {
                if let Some(model) = req.params["modelId"].as_str() {
                    state.model = model.into();
                }
            }
            "session/set_config_option" => {
                if let Some(model) = model_from(r) {
                    state.model = model;
                }
            }
            "session/prompt" => {
                state.prompts += 1;
            }
            "fs/read_text_file"
                if req.params["line"].is_null() && req.params["limit"].is_null() =>
            {
                if let (Some(path), Some(text)) =
                    (req.params["path"].as_str(), r["content"].as_str())
                {
                    let key = (sid.clone(), absolute(&state.cwd, path));
                    last_fs.remove(&key);
                    prior.insert(key, text.into());
                }
            }
            "fs/write_text_file" => {
                if let (Some(path), Some(text)) =
                    (req.params["path"].as_str(), req.params["content"].as_str())
                {
                    let path = absolute(&state.cwd, path);
                    let old = prior.insert((sid.clone(), path.clone()), text.into());
                    // A write response doesn't identify a new file. Unknown before
                    // content cannot be treated as empty; wait for an explicit diff.
                    if let Some(old) = old {
                        last_fs.insert(
                            (sid.clone(), path.clone()),
                            (old.clone(), text.into(), calls.len()),
                        );
                        let mut g = FileGroup::default();
                        crate::trace_import::push_scoped(
                            &mut g,
                            pm,
                            "",
                            &path,
                            &crate::added_lines(&old, text),
                            src,
                            &req.model,
                            &sid,
                            at,
                        );
                        calls.push(CapturedCall {
                            sid: sid.clone(),
                            id: format!("fs:{capture_id}:{}", req.sequence),
                            evs: g.evs,
                        });
                    } else {
                        last_fs.remove(&(sid.clone(), path));
                    }
                }
            }
            _ => {}
        }
    }
    let mut tools: Vec<_> = tools.into_iter().collect();
    tools.sort_by(|a, b| (a.1.ts, &a.0).cmp(&(b.1.ts, &b.0)));
    for ((sid, call_id), tool) in tools {
        if tool.status != "completed" {
            continue;
        }
        let cwd = states.get(&sid).map(|s| s.cwd.as_str()).unwrap_or("");
        let mut g = FileGroup::default();
        for diff in tool.content.iter().filter(|c| c["type"] == "diff") {
            let (Some(path), Some(new)) = (diff["path"].as_str(), diff["newText"].as_str()) else {
                continue;
            };
            // oldText:null denotes a new file in ACP. Missing oldText is not proof of an empty file.
            let Some(old) = diff.get("oldText").and_then(|old| {
                if old.is_null() {
                    Some("")
                } else {
                    old.as_str()
                }
            }) else {
                continue;
            };
            crate::trace_import::push_scoped(
                &mut g,
                pm,
                cwd,
                path,
                &crate::added_lines(old, new),
                src,
                &tool.model,
                &sid,
                tool.ts,
            );
        }
        calls.push(CapturedCall {
            sid: sid.clone(),
            id: format!("tool:{call_id}"),
            evs: g.evs,
        });
    }
    for (index, call) in calls.iter_mut().enumerate() {
        if echoed_fs.contains(&index) {
            call.evs.clear();
        }
    }
    for call in &calls {
        groups
            .entry(call.sid.clone())
            .or_default()
            .evs
            .extend(call.evs.iter().cloned());
    }
    let mut activities = vec![];
    for (sid, s) in states {
        if !s.cwd.is_empty() {
            activities.push(Activity {
                src,
                cwd: s.cwd.clone(),
                remote: None,
                sessions: 1,
                last: s.end,
            });
        }
        let g = groups.entry(sid.clone()).or_default();
        if !pm.in_project(&s.cwd) && g.evs.is_empty() {
            continue;
        }
        g.sessions.push(Session {
            src,
            id: sid,
            model: s.model,
            start: s.start,
            end: s.end,
            tokens: 0,
            prompts: s.prompts,
            branch: None,
            reviewer: false,
            has_content: !g.evs.is_empty(),
            provider: None,
        });
    }
    Some(Parsed {
        src,
        groups,
        activities,
        calls,
    })
}

fn files() -> Vec<PathBuf> {
    let mut paths: Vec<_> = walkdir::WalkDir::new(crate::registry::capture_dir())
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|s| s == "jsonl"))
        .map(|e| e.into_path())
        .collect();
    paths.sort();
    paths
}

fn read(path: &Path) -> std::io::Result<Vec<Value>> {
    BufReader::new(File::open(path)?)
        .lines()
        .filter_map(|line| match line {
            Ok(s) if s.trim().is_empty() => None,
            Ok(s) => Some(
                serde_json::from_str(&s)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            ),
            Err(e) => Some(Err(e)),
        })
        .collect()
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    scan_paths(&files(), store, pm, st);
}

fn event_key(e: &Ev) -> (Src, String, String, u32) {
    (e.src, e.session.clone(), e.rel.clone(), e.key)
}

fn scan_paths(paths: &[PathBuf], store: &mut Store, pm: &PathMap, st: &mut Stats) {
    // Rebuild captures so a subsequently discovered native session takes precedence.
    store.groups.retain(|key, _| !key.starts_with("acp:"));
    let native_sessions: HashSet<_> = store
        .groups
        .values()
        .flat_map(|g| g.sessions.iter().map(|s| (s.src, s.id.clone())))
        .collect();
    // A capture's timestamp is receipt time, not the native execution clock.
    // With no shared native call ID, conservatively overlap per-session line
    // occurrences instead of treating a one-second delay as a second write.
    // Different known models remain separate; unknown models cannot disprove
    // overlap. Native evidence keeps its actual time and model.
    let mut native_counts: HashMap<_, std::collections::BTreeMap<String, usize>> = HashMap::new();
    for e in store.groups.values().flat_map(|g| &g.evs) {
        *native_counts
            .entry(event_key(e))
            .or_default()
            .entry(e.model.clone())
            .or_default() += 1;
    }
    let mut seen_calls = HashSet::new();
    let mut pending_evidence = vec![];
    let mut recordings: HashMap<String, Vec<Value>> = HashMap::new();
    for path in paths {
        let rows = match read(path) {
            Ok(v) => v,
            Err(e) => {
                st.warnings.push(format!("ACP {}: {e}", path.display()));
                continue;
            }
        };
        let Some(header) = rows
            .first()
            .filter(|h| h["type"] == "creditia-acp" && h["version"] == 1)
        else {
            st.warnings.push(format!(
                "ACP {}: unsupported capture header",
                path.display()
            ));
            continue;
        };
        let recording = recordings.entry(header.to_string()).or_default();
        if rows.len() > recording.len() {
            *recording = rows;
        }
    }
    let mut recordings: Vec<_> = recordings.into_iter().collect();
    recordings.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, rows) in recordings {
        let Some(mut parsed) = parse(&rows, pm) else {
            continue;
        };
        for g in parsed.groups.values_mut() {
            g.evs.clear();
        }
        for call in parsed.calls {
            if !seen_calls.insert((parsed.src, call.sid.clone(), call.id)) {
                continue;
            }
            for e in call.evs {
                pending_evidence.push(e);
            }
        }
        for (sid, mut g) in parsed.groups {
            if native_sessions.contains(&(parsed.src, sid.clone())) {
                g.sessions.clear();
            }
            if g.sessions.is_empty() && g.evs.is_empty() {
                continue;
            }
            let group = store
                .groups
                .entry(format!("acp:{}:{sid}", parsed.src.harness()))
                .or_default();
            group.evs.extend(g.evs);
            if let Some(s) = g.sessions.pop() {
                if let Some(existing) = group.sessions.first_mut() {
                    existing.start = existing.start.min(s.start);
                    existing.end = existing.end.max(s.end);
                    existing.prompts += s.prompts;
                    existing.has_content |= s.has_content;
                    if !s.model.is_empty() {
                        existing.model = s.model;
                    }
                } else {
                    group.sessions.push(s);
                }
            }
        }
    }
    // Reserve exact known-model matches before unknown capture models can use
    // a native occurrence. This applies across all recordings, not one file.
    pending_evidence.sort_by_key(|e| e.model.is_empty());
    for e in pending_evidence {
        if let Some(models) = native_counts.get_mut(&event_key(&e)) {
            let overlap = if models.get(&e.model).is_some_and(|&n| n > 0) {
                Some(e.model.clone())
            } else if e.model.is_empty() {
                models.iter().find(|(_, n)| **n > 0).map(|(m, _)| m.clone())
            } else {
                models.get("").filter(|&&n| n > 0).map(|_| String::new())
            };
            if let Some(model) = overlap {
                *models.get_mut(&model).unwrap() -= 1;
                continue;
            }
        }
        store
            .groups
            .entry(format!("acp:{}:{}", e.src.harness(), e.session))
            .or_default()
            .evs
            .push(e);
    }
}

fn empty_map() -> PathMap {
    PathMap {
        known: HashSet::new(),
        markers: vec![],
        excludes: vec![],
    }
}

pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<(Src, String), Activity> = HashMap::new();
    for path in files() {
        if let Some(parsed) = read(&path).ok().and_then(|v| parse(&v, &empty_map())) {
            for a in parsed.activities {
                let e = out.entry((a.src, a.cwd.clone())).or_insert(Activity {
                    sessions: 0,
                    ..a.clone()
                });
                e.sessions += 1;
                e.last = e.last.max(a.last);
            }
        }
    }
    out.into_values().collect()
}

pub fn doctor() -> Vec<Health> {
    let mut counts: HashMap<Src, u64> = HashMap::new();
    for path in files() {
        if let Some(p) = read(&path).ok().and_then(|v| parse(&v, &empty_map())) {
            *counts.entry(p.src).or_default() += p.activities.len() as u64;
        }
    }
    let mut sources = CAPTURE_SOURCES.to_vec();
    for src in counts.keys() {
        if !sources.contains(src) {
            sources.push(*src);
        }
    }
    sources.into_iter().map(|src|Health{source:format!("{}-acp",src.harness()),path:crate::registry::capture_dir().display().to_string(),readable:counts.get(&src).copied().unwrap_or(0)>0,format:"Creditia ACP capture v1".into(),records:counts.get(&src).copied().unwrap_or(0),note:"Opt-in ACP capture: completed diff and successful fs/write_text_file only. Use creditia capture --harness <tool> -- <agent ACP command> in your ACP client. Does not read private past history; no model is inferred when the protocol omits it; context-window usage is not billed tokens.".into(),..Default::default()}).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(direction: &str, message: Value) -> Value {
        json!({"direction":direction,"at":1780000000000i64,"message":message})
    }
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    fn header() -> Value {
        json!({"type":"creditia-acp","version":1,"harness":"cortex-code","cwd":"/demo"})
    }
    fn update(id: &str, status: &str, old: Value, new: &str) -> Value {
        row(
            "agent",
            json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"tool_call","toolCallId":id,"kind":"edit","status":status,"content":[{"type":"diff","path":"/demo/a.rs","oldText":old,"newText":new}]}}}),
        )
    }
    #[test]
    fn completed_partial_updates_keep_model_at_call_and_ignore_failed() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
            ),
            row(
                "agent",
                json!({"id":1,"result":{"sessionId":"s","models":{"currentModelId":"model-a"}}}),
            ),
            update(
                "ok",
                "in_progress",
                json!("unchanged_line"),
                "unchanged_line\naccepted_line",
            ),
            row(
                "agent",
                json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"config_option_update","configOptions":[{"id":"model","currentValue":"model-b"}]}}}),
            ),
            row(
                "agent",
                json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"tool_call_update","toolCallId":"ok","status":"completed"}}}),
            ),
            update("failed", "failed", Value::Null, "failed_line"),
            update("pending", "pending", Value::Null, "pending_line"),
        ];
        let p = parse(&rows, &pm()).unwrap();
        let g = &p.groups["s"];
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "model-a");
        assert_eq!(g.sessions[0].model, "model-b");
        assert_eq!(g.sessions[0].tokens, 0);
    }
    #[test]
    fn filesystem_receipts_require_success_and_use_prior_full_read() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
            ),
            row("agent", json!({"id":1,"result":{"sessionId":"s"}})),
            row(
                "agent",
                json!({"id":7,"method":"fs/read_text_file","params":{"sessionId":"s","path":"/demo/a.rs"}}),
            ),
            row(
                "client",
                json!({"id":7,"result":{"content":"unchanged_line"}}),
            ),
            row(
                "agent",
                json!({"id":8,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"unchanged_line\naccepted_line"}}),
            ),
            row("client", json!({"id":8,"result":{}})),
            row(
                "agent",
                json!({"id":9,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"failed_line"}}),
            ),
            row("client", json!({"id":9,"error":{"code":-1}})),
        ];
        let p = parse(&rows, &pm()).unwrap();
        assert_eq!(p.groups["s"].evs.len(), 1);
        assert_eq!(p.groups["s"].evs[0].model, "");
        let mut partial = rows.clone();
        partial[3]["message"]["params"]["line"] = json!(1);
        partial[3]["message"]["params"]["limit"] = json!(1);
        assert!(
            parse(&partial, &pm()).unwrap().groups["s"].evs.is_empty(),
            "partial read cannot prove full before content"
        );
    }
    #[test]
    fn call_identity_preserves_repeated_lines_and_models_but_deduplicates_copies() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
            ),
            row(
                "agent",
                json!({"id":1,"result":{"sessionId":"s","models":{"currentModelId":"model-a"}}}),
            ),
            update(
                "first",
                "completed",
                Value::Null,
                "repeated_line\nrepeated_line",
            ),
            row(
                "agent",
                json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"config_option_update","configOptions":[{"id":"model","currentValue":"model-b"}]}}}),
            ),
            update("second", "completed", Value::Null, "repeated_line"),
        ];
        let root = std::env::temp_dir().join(format!(
            "creditia-acp-repeat-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let text = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let a = root.join("a.jsonl");
        let b = root.join("copy.jsonl");
        std::fs::write(&a, &text).unwrap();
        std::fs::write(&b, &text).unwrap();
        let parsed = parse(&rows, &pm()).unwrap();
        let mut store = Store::default();
        store.groups.insert(
            "native".into(),
            FileGroup {
                evs: vec![parsed.groups["s"].evs[0].clone()],
                ..Default::default()
            },
        );
        let mut stats = Stats::default();
        scan_paths(&[a, b], &mut store, &pm(), &mut stats);
        let evs: Vec<_> = store.groups.values().flat_map(|g| &g.evs).collect();
        assert!(stats.warnings.is_empty());
        assert_eq!(evs.len(), 3);
        assert_eq!(evs.iter().filter(|e| e.model == "model-a").count(), 2);
        assert_eq!(evs.iter().filter(|e| e.model == "model-b").count(), 1);
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn loaded_tool_history_does_not_invent_an_execution_time() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/load","params":{"sessionId":"s","cwd":"/demo"}}),
            ),
            update("old-call", "completed", Value::Null, "past_line"),
            row("agent", json!({"id":1,"result":{}})),
        ];
        let parsed = parse(&rows, &pm()).unwrap();
        assert_eq!(parsed.groups["s"].evs[0].ts, 0);
    }
    #[test]
    fn native_overlap_does_not_require_matching_receipt_time_or_model_availability() {
        for (native_model, native_time) in [("m", 1_780_000_099), ("", 1_780_000_099), ("m", 0)] {
            let rows = vec![
                header(),
                row(
                    "client",
                    json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
                ),
                row(
                    "agent",
                    json!({"id":1,"result":{"sessionId":"s","models":{"currentModelId":"m"}}}),
                ),
                update("one", "completed", Value::Null, "same_written_line"),
            ];
            let p = std::env::temp_dir().join(format!(
                "creditia-acp-native-delay-{}-{}-{}.jsonl",
                std::process::id(),
                native_model,
                native_time
            ));
            std::fs::write(
                &p,
                rows.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
            let parsed = parse(&rows, &pm()).unwrap();
            let mut native = parsed.groups["s"].evs[0].clone();
            native.ts = native_time;
            native.model = native_model.into();
            let mut store = Store::default();
            store
                .groups
                .entry("native".into())
                .or_default()
                .evs
                .push(native);
            scan_paths(
                std::slice::from_ref(&p),
                &mut store,
                &pm(),
                &mut Stats::default(),
            );
            assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
            std::fs::remove_file(p).unwrap();
        }
    }
    #[test]
    fn known_models_reserve_native_occurrences_before_unknown_models() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
            ),
            row("agent", json!({"id":1,"result":{"sessionId":"s"}})),
            update("unknown", "completed", Value::Null, "same_written_line"),
            row(
                "agent",
                json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"current_model_update","currentModelId":"a-model"}}}),
            ),
            update("known", "completed", Value::Null, "same_written_line"),
        ];
        let p = std::env::temp_dir().join(format!(
            "creditia-acp-unknown-overlap-{}.jsonl",
            std::process::id()
        ));
        std::fs::write(
            &p,
            rows.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        let parsed = parse(&rows, &pm()).unwrap();
        let first = Ev {
            model: "b-model".into(),
            ts: 1_780_000_100,
            ..parsed.groups["s"].evs[0].clone()
        };
        let second = Ev {
            model: "a-model".into(),
            ts: 1_780_000_200,
            ..first.clone()
        };
        let mut store = Store::default();
        store.groups.entry("native".into()).or_default().evs = vec![first, second];
        scan_paths(
            std::slice::from_ref(&p),
            &mut store,
            &pm(),
            &mut Stats::default(),
        );
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 2);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn filesystem_and_tool_diff_do_not_multiply_the_same_write() {
        let rows = vec![
            header(),
            row(
                "client",
                json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
            ),
            row("agent", json!({"id":1,"result":{"sessionId":"s"}})),
            row(
                "agent",
                json!({"id":7,"method":"fs/read_text_file","params":{"sessionId":"s","path":"/demo/a.rs"}}),
            ),
            row("client", json!({"id":7,"result":{"content":"old_line"}})),
            row(
                "agent",
                json!({"id":8,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"old_line\naccepted_line"}}),
            ),
            row("client", json!({"id":8,"result":{}})),
            update(
                "tool",
                "completed",
                json!("old_line"),
                "old_line\naccepted_line",
            ),
        ];
        let parsed = parse(&rows, &pm()).unwrap();
        assert_eq!(parsed.groups["s"].evs.len(), 1);
    }
    #[test]
    fn a_later_filesystem_readd_is_not_an_echo_of_an_old_tool_diff() {
        for second_model in ["first", "second"] {
            let rows = vec![
                header(),
                row(
                    "client",
                    json!({"id":1,"method":"session/new","params":{"cwd":"/demo"}}),
                ),
                row(
                    "agent",
                    json!({"id":1,"result":{"sessionId":"s","models":{"currentModelId":"first"}}}),
                ),
                update("first", "completed", Value::Null, "repeated_line"),
                row(
                    "agent",
                    json!({"id":2,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":""}}),
                ),
                row("client", json!({"id":2,"result":{}})),
                row(
                    "agent",
                    json!({"method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"current_model_update","currentModelId":second_model}}}),
                ),
                row(
                    "agent",
                    json!({"id":3,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"repeated_line"}}),
                ),
                row("client", json!({"id":3,"result":{}})),
            ];
            let p = parse(&rows, &pm()).unwrap();
            let g = &p.groups["s"];
            assert_eq!(
                g.evs.len(),
                2,
                "separate writes survive even within the same timestamp/model"
            );
            assert!(g.evs.iter().any(|e| e.model == second_model));
        }
    }
    #[test]
    fn sanitization_keeps_edit_fields_without_auth_prompts_or_server_credentials() {
        let f = std::env::temp_dir().join(format!(
            "creditia-acp-unit-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut rec = Recorder::new(File::create(&f).unwrap());
        assert!(rec
            .sanitize(
                "client",
                &json!({"id":0,"method":"authenticate","params":{"token":"SECRET"}})
            )
            .is_none());
        assert!(rec
            .sanitize("agent", &json!({"id":0,"result":{"token":"SECRET"}}))
            .is_none());
        let new=rec.sanitize("client",&json!({"id":1,"method":"session/new","params":{"cwd":"/demo","mcpServers":[{"env":[{"value":"SECRET"}]}]}})).unwrap();
        assert!(!new.to_string().contains("SECRET"));
        let prompt=rec.sanitize("client",&json!({"id":2,"method":"session/prompt","params":{"sessionId":"s","prompt":[{"text":"PRIVATE PROMPT"}]}})).unwrap();
        assert!(!prompt.to_string().contains("PRIVATE"));
        assert!(rec
            .sanitize(
                "agent",
                &json!({"method":"session/request_permission","params":{"secret":"SECRET"}})
            )
            .is_none());
        std::fs::remove_file(f).unwrap();
    }
    #[test]
    fn malformed_container_fields_and_nonobject_write_results_are_not_recorded() {
        let f = std::env::temp_dir().join(format!(
            "creditia-acp-types-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut rec = Recorder::new(File::create(&f).unwrap());
        assert!(rec
            .sanitize(
                "client",
                &json!({"id":{"token":"SECRET"},"method":"session/new","params":{"cwd":"/demo"}})
            )
            .is_none());
        assert!(rec
            .sanitize(
                "client",
                &json!({"id":1,"method":"session/new","params":{"cwd":{"token":"SECRET"}}})
            )
            .is_none());
        assert!(rec.sanitize("agent",&json!({"id":2,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":{"token":"SECRET"}}})).is_none());
        assert!(rec.sanitize("agent",&json!({"id":3,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"new_line"}})).is_some());
        assert!(rec
            .sanitize("client", &json!({"id":3,"result":null}))
            .is_none());
        let rows = vec![
            header(),
            row(
                "agent",
                json!({"id":4,"method":"fs/write_text_file","params":{"sessionId":"s","path":"/demo/a.rs","content":"new_line"}}),
            ),
            row("client", json!({"id":4,"result":false})),
        ];
        let parsed = parse(&rows, &pm()).unwrap();
        assert!(parsed.groups.values().all(|g| g.evs.is_empty()));
        std::fs::remove_file(f).unwrap();
    }
    #[test]
    fn a_corrupt_capture_is_refused_instead_of_dropping_rows() {
        let f = std::env::temp_dir().join(format!(
            "creditia-acp-corrupt-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::write(&f, format!("{}\n{{broken\n", header())).unwrap();
        assert_eq!(
            read(&f).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        std::fs::remove_file(f).unwrap();
    }
}
