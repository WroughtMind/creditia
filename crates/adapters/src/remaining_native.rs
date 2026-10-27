//! Native histories verified from official Cortex 1.0.73, Nova 1.1.54,
//! Pool 1.0.16 and DimCode 0.5.19 distribution artifacts. See SUPPORT_REMAINING.md.
use crate::{any_ts, Activity, Health, Stats};
use creditia_core::{
    model::{fingerprint, FileGroup, PathMap, Src, Store},
    tr,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

const SOURCES: [Src; 5] = [
    Src::Cortex,
    Src::Nova,
    Src::Poolside,
    Src::DimCode,
    Src::Corust,
];
fn env(k: &str) -> Option<PathBuf> {
    std::env::var(k)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}
fn override_dir(src: Src) -> Option<PathBuf> {
    env(&format!(
        "CREDITIA_{}_DIR",
        src.harness().to_uppercase().replace('-', "_")
    ))
}
pub fn data_dir(src: Src) -> PathBuf {
    if let Some(p) = override_dir(src) {
        return p;
    }
    match src {
        Src::Cortex => crate::home().join(".snowflake/cortex/conversations"),
        Src::Nova => nova_home(),
        Src::Poolside => env("XDG_CONFIG_HOME")
            .unwrap_or_else(|| crate::home().join(".config"))
            .join("poolside/trajectories"),
        Src::DimCode => {
            let root = env("DIMCODE_HOME")
                .map(|p| {
                    if p.file_name().is_some_and(|n| n == "v2") {
                        p
                    } else {
                        p.join("v2")
                    }
                })
                .unwrap_or_else(|| crate::home().join(".dimcode/v2"));
            root.join("dimcode.sqlite")
        }
        Src::Corust => env("CORUST_HOME").unwrap_or_else(|| crate::home().join(".corust-agent")),
        _ => unreachable!("not a remaining native source"),
    }
}
fn nova_home() -> PathBuf {
    let p = crate::home().join(".nova");
    if p.is_dir() || !crate::home().join(".compass").is_dir() {
        p
    } else {
        crate::home().join(".compass")
    }
}
fn string<'a>(v: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|k| v[*k].as_str()).unwrap_or("")
}
fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn read_json(p: &Path) -> Value {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn jsonl(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
fn paths(root: &Path, suffix: &str) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.into()];
    }
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(suffix))
        .map(|e| e.into_path())
        .collect()
}
#[derive(Clone)]
struct Record {
    src: Src,
    id: String,
    cwd: String,
    model: String,
    start: i64,
    end: i64,
    rows: Vec<Value>,
}
impl Record {
    fn add_session(&self, g: &mut FileGroup) {
        let prompts = self
            .rows
            .iter()
            .filter(|v| {
                if self.src == Src::Poolside {
                    return !v["session_input"].is_null() || !v["session_start"].is_null();
                }
                if v["role"] != "user" {
                    return false;
                }
                if self.src == Src::Cortex {
                    if v["content"].is_string() {
                        return true;
                    }
                    return array(&v["content"])
                        .iter()
                        .any(|c| c["type"] == "text" && c["is_user_prompt"] == true);
                }
                true
            })
            .count() as i64;
        g.sessions.push(crate::trace_import::session(
            self.src,
            &self.id,
            &self.model,
            self.start,
            self.end,
            prompts,
            !self.rows.is_empty(),
        ));
    }
}
fn native_files(src: Src, roots: &[String]) -> Vec<PathBuf> {
    let mut out = match src {
        Src::Cortex => paths(&data_dir(src), ".json")
            .into_iter()
            .filter(|p| {
                !p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .contains(".back.")
            })
            .collect(),
        Src::Nova => {
            let mut candidates = if override_dir(src).is_some() {
                paths(&data_dir(src), ".json")
            } else {
                paths(&data_dir(src).join("projects"), ".json")
                    .into_iter()
                    .filter(|p| {
                        p.parent()
                            .and_then(Path::file_name)
                            .is_some_and(|s| s == "chats")
                            || p.file_name().is_some_and(|s| s == "session.json")
                    })
                    .collect()
            };
            if override_dir(src).is_none() {
                for root in roots {
                    for name in [".nova", ".compass"] {
                        let local = Path::new(root).join(name);
                        candidates.extend(paths(&local.join("chats"), ".json"));
                        if local.join("session.json").is_file() {
                            candidates.push(local.join("session.json"));
                        }
                    }
                }
            }
            if override_dir(src).is_none() {
                candidates.extend(paths(&data_dir(src).join("acp-sessions"), ".json"));
            }
            candidates
        }
        Src::Poolside => paths(&data_dir(src), ".ndjson"),
        Src::DimCode => {
            if data_dir(src).is_file() {
                vec![data_dir(src)]
            } else {
                vec![]
            }
        }
        _ => vec![],
    };
    out.sort();
    out.dedup();
    out
}
fn records(src: Src, p: &Path) -> Vec<Record> {
    match src {
        Src::Cortex => {
            let v = read_json(p);
            let id = string(&v, &["session_id", "sessionId"]);
            let cwd = string(&v, &["working_directory", "workingDirectory"]);
            if id.is_empty() || cwd.is_empty() {
                return vec![];
            }
            let hist = p.with_file_name(format!(
                "{}.history.jsonl",
                p.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let rows = if hist.exists() {
                jsonl(&hist)
            } else if v["history"].is_array() {
                array(&v["history"]).to_vec()
            } else {
                array(&v["messages"]).to_vec()
            };
            // Native reader W1_/Os$ keeps the final occurrence of each message id.
            let last: HashMap<_, _> = rows
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v["id"].as_str().map(|id| (id.to_owned(), i)))
                .collect();
            let rows = rows
                .into_iter()
                .enumerate()
                .filter(|(i, v)| v["id"].as_str().is_some_and(|id| last.get(id) == Some(i)))
                .map(|(_, v)| v)
                .collect();
            vec![Record {
                src,
                id: id.into(),
                cwd: cwd.into(),
                model: String::new(),
                start: any_ts(&v["created_at"])
                    .or_else(|| any_ts(&v["createdAt"]))
                    .unwrap_or(0),
                end: any_ts(&v["last_updated"])
                    .or_else(|| any_ts(&v["lastUpdated"]))
                    .unwrap_or(0),
                rows,
            }]
        }
        Src::Nova => {
            let v = read_json(p);
            let (id, cwd, model, start, end, rows) = if v["version"] == "1.0"
                && v["messages"].is_array()
                && v["startedAt"].is_string()
                && v["endedAt"].is_string()
            {
                (
                    string(&v, &["sessionId"]),
                    string(&v, &["projectPath"]),
                    string(&v, &["model"]),
                    &v["startedAt"],
                    &v["endedAt"],
                    &v["messages"],
                )
            } else if v["workingDir"].is_string()
                && v["conversationHistory"].is_array()
                && v["createdAt"].is_string()
                && v["lastActivity"].is_string()
            {
                // Native ACPSessionManager.snapshot(): this is an existing vendor
                // disk store, independently of Creditia's future ACP capture.
                (
                    string(&v, &["id"]),
                    string(&v, &["workingDir"]),
                    "",
                    &v["createdAt"],
                    &v["lastActivity"],
                    &v["conversationHistory"],
                )
            } else if v["state"]["conversationHistory"].is_array()
                && v["projectPath"].is_string()
                && v["createdAt"].is_string()
                && v["updatedAt"].is_string()
            {
                (
                    string(&v, &["id"]),
                    string(&v, &["projectPath"]),
                    string(&v["state"], &["currentModel"]),
                    &v["createdAt"],
                    &v["updatedAt"],
                    &v["state"]["conversationHistory"],
                )
            } else {
                return vec![];
            };
            if id.is_empty() || cwd.is_empty() {
                return vec![];
            }
            vec![Record {
                src,
                id: id.into(),
                cwd: cwd.into(),
                model: model.into(),
                start: any_ts(start).unwrap_or(0),
                end: any_ts(end).unwrap_or(0),
                rows: array(rows).to_vec(),
            }]
        }
        Src::Poolside => {
            let rows = jsonl(p);
            let Some(start) = rows
                .iter()
                .find(|v| v["type"] == "session.start" && v["session_start"].is_object())
            else {
                return vec![];
            };
            let cwd = array(&start["session_start"]["working_directories"])
                .first()
                .and_then(Value::as_str)
                .unwrap_or("");
            if cwd.is_empty() {
                return vec![];
            }
            let id = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            vec![Record {
                src,
                id,
                cwd: cwd.into(),
                model: String::new(),
                start: any_ts(&start["timestamp"]).unwrap_or(0),
                end: rows
                    .iter()
                    .filter_map(|v| any_ts(&v["timestamp"]))
                    .max()
                    .unwrap_or(0),
                rows,
            }]
        }
        Src::DimCode => dim_records(p),
        _ => vec![],
    }
}
fn dim_records(p: &Path) -> Vec<Record> {
    let Ok(db) = crate::open_ro(p) else {
        return vec![];
    };
    let Ok(mut q) = db.prepare("SELECT sessionId,cwd,createdAt,updatedAt FROM sessions") else {
        return vec![];
    };
    let sessions: Vec<_> = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    let mut out = vec![];
    for (id, cwd, start, end) in sessions {
        let mut rows = vec![];
        if let Ok(mut q)=db.prepare("SELECT messageId,role,parts,toolMetadata,metadata,createdAt FROM messages WHERE sessionId=? ORDER BY orderKey,messageId") {
            for (mid,role,parts,tool,meta,ts) in q.query_map([&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?.unwrap_or_default(),r.get::<_,Option<String>>(4)?.unwrap_or_default(),r.get::<_,String>(5)?))).into_iter().flatten().flatten() {
                rows.push(serde_json::json!({"id":mid,"role":role,"parts":serde_json::from_str::<Value>(&parts).unwrap_or_default(),"toolMetadata":serde_json::from_str::<Value>(&tool).unwrap_or_default(),"metadata":serde_json::from_str::<Value>(&meta).unwrap_or_default(),"createdAt":ts}));
            }
        }
        // A run may switch models. Only a single distinct ledger model is safe
        // for that run's assistant tool calls; never borrow session_states' last model.
        let mut models = HashMap::new();
        if let Ok(mut q)=db.prepare("SELECT runId,MIN(modelId) FROM usage_ledger WHERE sessionId=? AND runId IS NOT NULL GROUP BY runId HAVING COUNT(DISTINCT modelId)=1") {
            for (run,model) in q.query_map([&id],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).into_iter().flatten().flatten() { models.insert(run,model); }
        }
        for row in &mut rows {
            if let Some(m) = row["metadata"]["runId"]
                .as_str()
                .and_then(|r| models.get(r))
            {
                row["_creditia_model"] = Value::String(m.clone());
            }
        }
        out.push(Record {
            src: Src::DimCode,
            id,
            cwd,
            model: String::new(),
            start: any_ts(&Value::String(start)).unwrap_or(0),
            end: any_ts(&Value::String(end)).unwrap_or(0),
            rows,
        });
    }
    out
}
fn content_text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.into();
    }
    array(v)
        .iter()
        .filter_map(|v| v["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn push(r: &Record, g: &mut FileGroup, pm: &PathMap, path: &str, text: &str, model: &str, ts: i64) {
    crate::trace_import::push_scoped(g, pm, &r.cwd, path, text, r.src, model, &r.id, ts);
}
fn cortex_edits(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    let mut calls: HashMap<String, Value> = HashMap::new();
    let mut done: HashSet<String> = HashSet::new();
    for row in &r.rows {
        for part in array(&row["content"]) {
            if row["role"] == "assistant" && part["type"] == "tool_use" {
                let c = &part["tool_use"];
                if let Some(id) = c["tool_use_id"].as_str() {
                    calls.insert(id.into(), c.clone());
                }
            } else if row["role"] == "user" && part["type"] == "tool_result" {
                let result = &part["tool_result"];
                let id = string(result, &["tool_use_id"]);
                if result["status"] != "success" || !done.insert(id.to_owned()) {
                    continue;
                }
                let Some(c) = calls.get(id) else {
                    continue;
                };
                if result["name"].as_str().is_some_and(|n| c["name"] != n) {
                    continue;
                }
                let args = &c["input"];
                let text = content_text(&result["content"]);
                let ts = any_ts(&row["user_sent_time"])
                    .or_else(|| any_ts(&row["userSentTime"]))
                    .unwrap_or(r.end);
                match c["name"].as_str().unwrap_or("") {
                    "edit" if text.starts_with("File edited successfully: ") => {
                        if let (Some(path), Some(old), Some(new)) = (
                            args["file_path"].as_str(),
                            args["old_string"].as_str(),
                            args["new_string"].as_str(),
                        ) {
                            push(r, g, pm, path, &crate::added_lines(old, new), "", ts);
                        }
                    }
                    "apply_patch" if text.starts_with("Applied patch successfully (") => {
                        for (path, new) in
                            crate::apply_patch_files(args["input"].as_str().unwrap_or(""))
                        {
                            push(r, g, pm, &path, &new, "", ts);
                        }
                    }
                    _ => {} // write overwrites lack a retained old snapshot; do not invent an empty before.
                }
            }
        }
    }
}
fn nova_changes(v: &Value) -> Option<String> {
    let changes = v["changes"].as_array()?;
    let mut out = String::new();
    let strip = |a: &Value| -> Option<String> {
        let mut text = String::new();
        for line in a.as_array()? {
            let (n, s) = line.as_str()?.split_once(": ")?;
            if n.is_empty() || !n.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            text.push_str(s);
            text.push('\n');
        }
        Some(text)
    };
    for c in changes {
        out.push_str(&crate::added_lines(
            &strip(&c["before"])?,
            &strip(&c["after"])?,
        ));
    }
    Some(out)
}
fn nova_edits(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    let mut calls: HashMap<String, (Value, String)> = HashMap::new();
    let mut done: HashSet<String> = HashSet::new();
    for row in &r.rows {
        // Native ACP snapshots store completed tools as assistant parts with
        // state.input/state.output, not Anthropic blocks. Their per-tool model
        // is absent; the mutable session.model must never backfill these lines.
        if row["role"] == "assistant" {
            for part in array(&row["parts"]) {
                if part["type"] != "tool"
                    || part["state"]["status"] != "completed"
                    || !matches!(part["tool"].as_str(), Some("edit_file" | "write_file"))
                {
                    continue;
                }
                let id = string(part, &["callId"]);
                let args = &part["state"]["input"];
                let Ok(result) =
                    serde_json::from_str::<Value>(part["state"]["output"].as_str().unwrap_or(""))
                else {
                    continue;
                };
                if id.is_empty()
                    || result["success"] != true
                    || result["dryRun"] == true
                    || args["dryRun"] == true
                    || args["dry_run"] == true
                    || result["execution"]["applied"] == false
                    || !done.insert(id.into())
                {
                    continue;
                }
                let path = string(&result, &["path"]);
                if path.is_empty() || args["path"] != path {
                    continue;
                }
                if let Some(text) = nova_changes(&result) {
                    push(
                        r,
                        g,
                        pm,
                        path,
                        &text,
                        "",
                        any_ts(&row["timestamp"]).unwrap_or(r.end),
                    );
                }
            }
        }

        for p in array(&row["contentBlocks"]) {
            if row["role"] == "assistant" && p["type"] == "tool_use" {
                if let Some(id) = p["id"].as_str() {
                    calls.insert(id.into(), (p.clone(), string(row, &["model"]).into()));
                }
            } else if row["role"] == "user" && p["type"] == "tool_result" && p["is_error"] != true {
                let id = string(p, &["tool_use_id"]);
                let Some((call, model)) = calls.get(id) else {
                    continue;
                };
                let Ok(result) = serde_json::from_str::<Value>(&content_text(&p["content"])) else {
                    continue;
                };
                let args = &call["input"];
                if result["success"] != true
                    || result["dryRun"] == true
                    || args["dryRun"] == true
                    || args["dry_run"] == true
                    || result["execution"]["applied"] == false
                    || !done.insert(id.to_owned())
                {
                    continue;
                }
                if !matches!(call["name"].as_str(), Some("edit_file" | "write_file")) {
                    continue;
                }
                let path = string(&result, &["path"]);
                if path.is_empty() || args["path"] != path {
                    continue;
                }
                let Some(text) = nova_changes(&result) else {
                    continue;
                };
                push(
                    r,
                    g,
                    pm,
                    path,
                    &text,
                    model,
                    any_ts(&row["timestamp"]).unwrap_or(r.end),
                );
            }
        }
    }
}
fn dim_edits(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    let mut calls: HashMap<String, (Value, String)> = HashMap::new();
    let mut done: HashSet<String> = HashSet::new();
    for row in &r.rows {
        for p in array(&row["parts"]) {
            if row["role"] == "assistant" && p["type"] == "tool_use" {
                if let Some(id) = p["id"].as_str() {
                    calls.insert(
                        id.into(),
                        (p.clone(), string(row, &["_creditia_model"]).into()),
                    );
                }
            } else if row["role"] == "tool_result"
                && p["type"] == "tool_result"
                && p["is_error"] != true
                && row["toolMetadata"]["status"] == "success"
            {
                let id = string(p, &["tool_use_id"]);
                let Some((call, model)) = calls.get(id) else {
                    continue;
                };
                if call["name"] != "edit"
                    || row["toolMetadata"]["toolName"] != "edit"
                    || !done.insert(id.to_owned())
                {
                    continue;
                }
                let result = &p["structuredContent"];
                let path = string(result, &["path"]);
                if path.is_empty()
                    || call["input"]["path"] != path
                    || !result["revision"].is_string()
                {
                    continue;
                }
                // The executed edit result is an actual before/after hunk diff,
                // including fuzzy match normalization; do not diff guessed args.
                let text = crate::added_from_diff(result["diff"].as_str().unwrap_or(""));
                push(
                    r,
                    g,
                    pm,
                    path,
                    &text,
                    model,
                    any_ts(&row["createdAt"]).unwrap_or(r.end),
                );
            }
        }
    }
}
fn pool_edits(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    let mut models: HashMap<String, String> = HashMap::new();
    let mut done: HashSet<String> = HashSet::new();
    for v in &r.rows {
        let step = string(v, &["step_id"]);
        if !step.is_empty() {
            if let Some(model) =
                v["tool_call_inference_start"]["chat_completion_request"]["model"].as_str()
            {
                models.insert(step.into(), model.into());
            }
        }
        if v["type"] != "tool_call.result" || v["is_transient"] == true {
            continue;
        }
        let result = &v["tool_call_result"];
        let id = string(result, &["id"]);
        if id.is_empty()
            || result["is_error"] == true
            || !string(result, &["execution_error_kind"]).is_empty()
            || !done.insert(id.into())
        {
            continue;
        }
        let edit = &result["edit_tool_result"];
        // Result payload contains actual entire before/after, rather than approval previews.
        if let (Some(path), Some(old), Some(new)) = (
            edit["path"].as_str(),
            edit["old_contents"].as_str(),
            edit["new_contents"].as_str(),
        ) {
            push(
                r,
                g,
                pm,
                path,
                &crate::added_lines(old, new),
                models.get(step).map(String::as_str).unwrap_or(""),
                any_ts(&v["timestamp"]).unwrap_or(r.end),
            );
        }
    }
}
fn parse(r: &Record, pm: &PathMap, g: &mut FileGroup) {
    if !pm.in_project(&r.cwd) {
        return;
    }
    r.add_session(g);
    match r.src {
        Src::Cortex => cortex_edits(r, pm, g),
        Src::Nova => nova_edits(r, pm, g),
        Src::DimCode => dim_edits(r, pm, g),
        Src::Poolside => pool_edits(r, pm, g),
        _ => {}
    }
}
fn fp(src: Src, p: &Path) -> (u64, u64) {
    let mut f = fingerprint(p);
    let companion = match src {
        Src::Cortex => Some(p.with_file_name(format!(
            "{}.history.jsonl",
            p.file_stem().unwrap_or_default().to_string_lossy()
        ))),
        Src::DimCode => Some(PathBuf::from(format!("{}-wal", p.display()))),
        _ => None,
    };
    if let Some(c) = companion {
        let b = fingerprint(&c);
        f.0 = f.0.wrapping_add(b.0);
        f.1 = f.1.wrapping_add(b.1);
    }
    f
}
pub fn scan(store: &mut Store, pm: &PathMap, roots: &[String], _st: &mut Stats) {
    for src in SOURCES {
        let prefix = format!("{}:native:", src.harness());
        let files = native_files(src, roots);
        let keys: HashSet<_> = files
            .iter()
            .map(|p| format!("{prefix}{}", p.display()))
            .collect();
        store
            .groups
            .retain(|k, _| !k.starts_with(&prefix) || keys.contains(k));
        for p in files {
            let key = format!("{prefix}{}", p.display());
            let fingerprint = fp(src, &p);
            if store.groups.get(&key).is_some_and(|g| g.fp == fingerprint) {
                continue;
            }
            let mut g = FileGroup {
                fp: fingerprint,
                ..Default::default()
            };
            for r in records(src, &p) {
                parse(&r, pm, &mut g);
            }
            store.groups.insert(key, g);
        }
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for src in SOURCES {
        for p in native_files(src, &[]) {
            for r in records(src, &p) {
                out.push(crate::trace_import::activity(src, r.cwd, None, r.end));
            }
        }
    }
    out
}
pub fn doctor() -> Vec<Health> {
    SOURCES.into_iter().map(|src| {
        let count=native_files(src,&[]).iter().map(|p|records(src,p).len()).sum();
        let (format,note,unsupported)=match src {
            Src::Cortex=>("Cortex split conversation JSON + history JSONL",tr!("读取原生会话及已成功的 edit/apply_patch；覆盖写入缺少旧快照时不生成新增行，磁盘消息不保存逐条模型。","Reads native sessions and successful edit/apply_patch; overwrites without old snapshots produce no added lines, and persisted messages omit per-message models."),false),
            Src::Nova=>("Nova native archives, session JSON and ACP snapshots",tr!("读取原生聊天档案、session.json 和 ACP 磁盘快照及成功工具真实 changes；排除预览和失败。只用逐消息模型；ACP 快照未保存逐工具模型时留空。项目本地档案在扫描项目时一并读取。","Reads native archives, session.json and ACP disk snapshots with successful tool changes; excludes previews and failures. Uses per-message models only; ACP tools without saved models stay unknown. Project-local archives are included during scans."),false),
            Src::Poolside=>("Pool trajectory Event NDJSON",tr!("读取原生事件流及成功 edit_tool_result 的真实前后文本，模型取同一步的推理请求；未知旧文本的覆盖写入不猜新增行。","Reads native events and actual before/after text from successful edit_tool_result, with the same step's inference model; overwrite additions without known old text are not guessed."),false),
            Src::DimCode=>("DimCode native SQLite sessions/messages",tr!("只读读取原生会话和成功 edit 的真实 diff；仅在同一 run 的账本只包含一个模型时归因该模型。覆盖写入缺少旧文本时不猜新增行。","Reads native sessions and successful edit diffs read-only; assigns a model only when that run's ledger contains one distinct model. Overwrite additions without old text are not guessed."),false),
            _=>("Corust 0.6.0 native persistence unavailable",tr!("已检查官方 0.6.0 原生二进制：load_session 明确未实现，未发现可核实的会话持久化序列化；工作区模型偏好和记忆不能还原旧编辑。","Inspected the official 0.6.0 binary: load_session explicitly is not implemented, and no verifiable session persistence serialization was found; workspace model preferences and memories cannot reconstruct past edits."),true),
        };
        crate::trace_import::health(src,&data_dir(src),format,count,&note,unsupported)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["sub/a.rs".into(), "a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    fn record(src: Src, rows: Vec<Value>) -> Record {
        Record {
            src,
            id: "native-session".into(),
            cwd: "/demo/sub".into(),
            model: "final-model-is-not-evidence".into(),
            start: 1,
            end: 10,
            rows,
        }
    }
    fn cortex_call(id: &str, new: &str) -> Value {
        json!({"id":format!("a-{id}"),"role":"assistant","content":[{"type":"tool_use","tool_use":{"tool_use_id":id,"name":"edit","input":{"file_path":"a.rs","old_string":"retained context\nprevious line","new_string":format!("retained context\n{new}")}}}]})
    }
    fn cortex_result(id: &str, status: &str, text: &str) -> Value {
        json!({"id":format!("u-{id}"),"role":"user","user_sent_time":10,"content":[{"type":"tool_result","tool_result":{"tool_use_id":id,"name":"edit","status":status,"content":[{"type":"text","text":text}]}}]})
    }
    #[test]
    fn cortex_nested_receipt_pairing_scopes_paths_and_rejects_errors_pending_and_fake_success() {
        let r = record(
            Src::Cortex,
            vec![
                cortex_call("ok", "actual new line"),
                cortex_result("ok", "success", "File edited successfully: /demo/sub/a.rs"),
                cortex_call("fail", "failed new line"),
                cortex_result("fail", "error", "File edited successfully: x"),
                cortex_call("pending", "pending new line"),
                cortex_call("fake", "fake new line"),
                cortex_result("fake", "success", "approved"),
                cortex_result("ok", "success", "File edited successfully: duplicate"),
            ],
        );
        let mut g = FileGroup::default();
        parse(&r, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "sub/a.rs");
        assert!(g.evs[0].model.is_empty());
        assert!(g.evs[0].single_use);
    }
    #[test]
    fn cortex_reads_split_jsonl_last_message_id_and_legacy_without_mutating_source() {
        let root =
            std::env::temp_dir().join(format!("creditia-cortex-native-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let p = root.join("s.json");
        std::fs::write(&p,json!({"session_id":"s","working_directory":"/demo/sub","created_at":1,"last_updated":5,"history":[cortex_call("old","not in split")] }).to_string()).unwrap();
        let mut old = cortex_call("ok", "stale superseded line");
        old["id"] = json!("a-ok");
        let current = cortex_call("ok", "current persisted line");
        let result = cortex_result("ok", "success", "File edited successfully: /demo/sub/a.rs");
        let hist = root.join("s.history.jsonl");
        std::fs::write(&hist, format!("{}\n{}\n{}\n", old, current, result)).unwrap();
        let before = std::fs::read(&hist).unwrap();
        let rec = records(Src::Cortex, &p);
        assert_eq!(rec[0].rows.len(), 2);
        let mut g = FileGroup::default();
        parse(&rec[0], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(before, std::fs::read(&hist).unwrap());
        let first = fp(Src::Cortex, &p);
        std::fs::write(&hist, format!("{}\n{}\n", current, result)).unwrap();
        assert_ne!(first, fp(Src::Cortex, &p));
        std::fs::remove_dir_all(root).unwrap();
    }
    fn nova_call(id: &str, model: &str, dry: bool) -> Value {
        json!({"role":"assistant","model":model,"contentBlocks":[{"type":"tool_use","id":id,"name":"edit_file","input":{"path":"a.rs","dryRun":dry,"edits":[{"oldText":"before","newText":"after"}]}}]})
    }
    fn nova_result(id: &str, success: bool, dry: bool, new: &str) -> Value {
        json!({"role":"user","timestamp":9,"contentBlocks":[{"type":"tool_result","tool_use_id":id,"content":json!({"path":"a.rs","success":success,"dryRun":dry,"execution":{"applied":success&&!dry},"changes":[{"before":["4: retained context","5: previous line"],"after":["4: retained context",format!("5: {new}")]}]}).to_string()}]})
    }
    #[test]
    fn nova_native_changes_preserve_actual_models_and_exclude_dry_run_failure_and_missing_before() {
        let mut missing = nova_result("missing", true, false, "missing-before line");
        let mut parsed: Value =
            serde_json::from_str(missing["contentBlocks"][0]["content"].as_str().unwrap()).unwrap();
        parsed["changes"][0]
            .as_object_mut()
            .unwrap()
            .remove("before");
        missing["contentBlocks"][0]["content"] = json!(parsed.to_string());
        let r = record(
            Src::Nova,
            vec![
                nova_call("a", "model-a", false),
                nova_result("a", true, false, "written by model a"),
                nova_call("b", "model-b", false),
                nova_result("b", true, false, "written by model b"),
                nova_call("preview", "model-c", true),
                nova_result("preview", true, true, "preview new line"),
                nova_call("fail", "model-c", false),
                nova_result("fail", false, false, "failed new line"),
                nova_call("missing", "model-c", false),
                missing,
            ],
        );
        let mut g = FileGroup::default();
        parse(&r, &pm(), &mut g);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "model-a");
        assert_eq!(g.evs[1].model, "model-b");
        assert!(g.evs.iter().all(|e| e.rel == "sub/a.rs"));
    }
    #[test]
    fn nova_reads_vendor_acp_disk_snapshot_completed_tools_and_ignores_final_model() {
        let p =
            std::env::temp_dir().join(format!("creditia-nova-native-{}.json", std::process::id()));
        let response = |new: &str, dry: bool| {
            json!({"path":"a.rs","operation":"edit","success":true,"dryRun":dry,"execution":{"applied":!dry},"changes":[{"before":["1: retained context","2: prior actual text"],"after":["1: retained context",format!("2: {new}")]}]}).to_string()
        };
        let tool = |id: &str, status: &str, dry: bool| json!({"id":id,"role":"assistant","timestamp":"2026-01-01T00:00:04Z","parts":[{"type":"tool","callId":id,"tool":"edit_file","state":{"status":status,"input":{"path":"a.rs","dryRun":dry},"output":response(id,dry)}}]});
        let native = json!({"id":"existing-acp-session","workingDir":"/demo/sub","createdAt":"2026-01-01T00:00:00Z","lastActivity":"2026-01-01T00:00:05Z","model":{"modelId":"final-switched-model"},"conversationHistory":[{"role":"user","parts":[{"type":"text","text":"edit this file"}]},tool("actual persisted line","completed",false),tool("dry preview line","completed",true),tool("failed line","error",false),tool("pending line","pending",false)]});
        std::fs::write(&p, native.to_string()).unwrap();
        let rs = records(Src::Nova, &p);
        assert_eq!(rs.len(), 1);
        let mut g = FileGroup::default();
        parse(&rs[0], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "sub/a.rs");
        assert!(g.evs[0].model.is_empty());
        assert_eq!(g.sessions[0].prompts, 1);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn pool_uses_actual_result_old_contents_and_same_step_model_only() {
        let infer = |step: &str, model: &str| json!({"type":"tool_call.inference.start","step_id":step,"tool_call_inference_start":{"chat_completion_request":{"model":model}}});
        let result = |id: &str, step: &str, error: bool| json!({"type":"tool_call.result","step_id":step,"timestamp":8,"tool_call_result":{"id":id,"is_error":error,"edit_tool_result":{"path":"a.rs","old_contents":"retained context\nold line","new_contents":format!("retained context\nnew line {id}")}}});
        let mut absent = result("absent", "three", false);
        absent["tool_call_result"]["edit_tool_result"]
            .as_object_mut()
            .unwrap()
            .remove("old_contents");
        let r = record(
            Src::Poolside,
            vec![
                infer("one", "model-a"),
                result("a", "one", false),
                infer("two", "model-b"),
                result("b", "two", false),
                result("error", "two", true),
                absent,
                result("unattributed", "three", false),
                json!({"type":"tool_call.approval","tool_call_result":{"id":"approval","edit_tool_result":{"path":"a.rs","old_contents":"","new_contents":"approved only line"}}}),
            ],
        );
        let mut g = FileGroup::default();
        parse(&r, &pm(), &mut g);
        assert_eq!(g.evs.len(), 3);
        assert_eq!(g.evs[0].model, "model-a");
        assert_eq!(g.evs[1].model, "model-b");
        assert!(g.evs[2].model.is_empty());
    }
    #[test]
    fn dim_native_sqlite_results_use_actual_diff_and_unique_run_ledger_models() {
        let p =
            std::env::temp_dir().join(format!("creditia-dim-native-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let db = rusqlite::Connection::open(&p).unwrap();
        db.execute_batch("CREATE TABLE sessions(sessionId TEXT,cwd TEXT,createdAt TEXT,updatedAt TEXT);CREATE TABLE messages(messageId TEXT,sessionId TEXT,role TEXT,parts TEXT,toolMetadata TEXT,metadata TEXT,orderKey TEXT,createdAt TEXT);CREATE TABLE usage_ledger(sessionId TEXT,runId TEXT,modelId TEXT);INSERT INTO sessions VALUES('s','/demo/sub','2026-01-01T00:00:00Z','2026-01-01T00:00:05Z');INSERT INTO usage_ledger VALUES('s','r1','model-one'),('s','r2','model-two'),('s','r2','fallback-model');").unwrap();
        for (i, run, status) in [
            (1, "r1", "success"),
            (2, "r2", "success"),
            (3, "r1", "error"),
        ] {
            let id = format!("t{i}");
            let call = json!([{"type":"tool_use","id":id,"name":"edit","input":{"path":"a.rs","oldText":"not the fuzzy actual before","newText":"not the fuzzy actual after"}}]);
            let result = json!([{"type":"tool_result","tool_use_id":id,"is_error":status=="error","structuredContent":{"path":"a.rs","revision":"sha256","diff":format!("@@ -1,2 +1,2 @@\n retained context\n-old actual line\n+new actual line {i}\n")}}]);
            for (n, role, parts, tool) in [
                (0, "assistant", call, json!({})),
                (
                    1,
                    "tool_result",
                    result,
                    json!({"status":status,"toolName":"edit"}),
                ),
            ] {
                db.execute(
                    "INSERT INTO messages VALUES(?,?,?,?,?,?,?,?)",
                    rusqlite::params![
                        format!("m{i}{n}"),
                        "s",
                        role,
                        parts.to_string(),
                        tool.to_string(),
                        json!({"runId":run}).to_string(),
                        format!("{i}{n}"),
                        "2026-01-01T00:00:04Z"
                    ],
                )
                .unwrap();
            }
        }
        drop(db);
        let rs = dim_records(&p);
        let mut g = FileGroup::default();
        parse(&rs[0], &pm(), &mut g);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "model-one");
        assert!(g.evs[1].model.is_empty());
        assert!(g.evs.iter().all(|e| e.rel == "sub/a.rs"));
        std::fs::remove_file(p).unwrap();
    }
}
