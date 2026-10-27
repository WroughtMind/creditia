//! Independently verified native formats, not generic name registration.
//! GLM e083941: session JSON + concrete success receipts; siGit 706b49d:
//! JSONL + cwd/model sidecar; crow 94510f6: completed subtool actual diffs.
//! Kimchi 50687f8: native Pi JSONL with successful multi-edit results.
//! VTCode eec794a / Minion 26e373e / Stakpak 760cd2b, Autohand d01ef07,
//! Dirac 3615882 and IBM Bob 2.0.8: native successful edits are parsed in
//! longtail_edits.rs, with source-specific receipts and retained-model limits.
use crate::{any_ts, open_ro, Activity, Health, Stats};
#[path = "longtail_edits.rs"]
mod edits;
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
fn env(k: &str) -> Option<PathBuf> {
    std::env::var(k)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}
pub fn data_dir(src: Src) -> PathBuf {
    let key = format!(
        "CREDITIA_{}_DIR",
        src.harness().to_uppercase().replace('-', "_")
    );
    if let Some(p) = env(&key) {
        return p;
    }
    match src {
        Src::GlmAgent => env("ACP_GLM_SESSION_DIR").unwrap_or_else(|| {
            env("XDG_STATE_HOME")
                .unwrap_or_else(|| crate::home().join(".local/state"))
                .join("glm-acp-agent/sessions")
        }),
        Src::SiGit => env("SIGIT_CONFIG_DIR")
            .unwrap_or_else(|| crate::home().join(".config/sigit"))
            .join("sessions"),
        Src::Crow => crate::home().join(".agents/crow/crow.db"),
        Src::Bob => crate::home().join(".bob/db/bob.db"),
        Src::Kimchi => crate::home().join(".config/kimchi/harness/sessions"),
        Src::Autohand => env("AUTOHAND_HOME")
            .unwrap_or_else(|| crate::home().join(".autohand"))
            .join("sessions"),
        Src::MinionCode => crate::home().join(".minion-code/sessions"),
        Src::Dirac => env("DIRAC_DATA_DIR").unwrap_or_else(|| {
            env("DIRAC_DIR")
                .unwrap_or_else(|| crate::home().join(".dirac"))
                .join("data")
        }),
        Src::VtCode => {
            if let Some(p) = env("VT_SESSION_DIR") {
                return p;
            }
            if let Some(p) = env("XDG_STATE_HOME") {
                return p.join("vtcode/sessions");
            }
            #[cfg(target_os = "macos")]
            {
                crate::home().join("Library/Application Support/com.vinhnx.vtcode/state/sessions")
            }
            #[cfg(target_os = "windows")]
            {
                env("APPDATA")
                    .unwrap_or_default()
                    .join("vinhnx/vtcode/state/sessions")
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                crate::home().join(".local/state/vtcode/sessions")
            }
        }
        Src::Stakpak => crate::home().join(".stakpak/data/local.db"),
        _ => crate::source_dir(src),
    }
}
fn read(p: &Path) -> Value {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn json_files(src: Src, root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|s| s == if src == Src::SiGit { "jsonl" } else { "json" })
                && !e.file_name().to_string_lossy().ends_with(".meta.json")
        })
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}
struct Record {
    src: Src,
    id: String,
    cwd: String,
    model: String,
    provider: Option<String>,
    start: i64,
    end: i64,
    rows: Vec<Value>,
}
fn dirac_record(v: &Value) -> Option<Record> {
    // Goals summarize separate task conversations and must not double-count them.
    if v["runKind"] == "goal" {
        return None;
    }
    Some(Record {
        src: Src::Dirac,
        id: v["id"].as_str()?.into(),
        cwd: v["cwdOnTaskInitialization"]
            .as_str()
            .or_else(|| v["workspaceRootPath"].as_str())?
            .into(),
        model: v["modelId"].as_str().unwrap_or("").into(),
        provider: None,
        start: any_ts(&v["ts"]).unwrap_or(0),
        end: any_ts(&v["ts"]).unwrap_or(0),
        rows: vec![],
    })
}
fn records(src: Src) -> Vec<(PathBuf, Record)> {
    records_at(src, &data_dir(src))
}
fn records_at(src: Src, root: &Path) -> Vec<(PathBuf, Record)> {
    if src == Src::Dirac {
        let p = root.join("state/taskHistory.json");
        return read(&p)
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(dirac_record)
            .map(|mut r| {
                let id_path = Path::new(&r.id);
                if id_path.components().count() == 1 && id_path.file_name().is_some() {
                    r.rows = edits::dirac_history(&root.join("tasks").join(&r.id));
                }
                (p.clone(), r)
            })
            .collect();
    }
    json_files(src, root)
        .into_iter()
        .filter_map(|p| {
            let v = if src == Src::SiGit {
                read(&p.with_extension("meta.json"))
            } else {
                read(&p)
            };
            let id = match src {
                Src::GlmAgent | Src::Autohand => v["sessionId"].as_str().map(str::to_string),
                Src::MinionCode => v["metadata"]["session_id"].as_str().map(str::to_string),
                _ => p.file_stem().and_then(|s| s.to_str()).map(str::to_string),
            }?;
            let cwd = match src {
                Src::GlmAgent | Src::SiGit => v["cwd"].as_str(),
                Src::MinionCode => v["metadata"]["project_path"].as_str(),
                Src::Autohand => v["projectPath"].as_str(),
                Src::VtCode => v["metadata"]["workspace_path"].as_str(),
                _ => None,
            }?
            .to_string();
            let model = match src {
                Src::GlmAgent | Src::Autohand => v["model"].as_str(),
                Src::SiGit => v["model_id"].as_str(),
                Src::VtCode => v["metadata"]["model"].as_str(),
                _ => None,
            }
            .unwrap_or("")
            .to_string();
            let start = match src {
                Src::VtCode => any_ts(&v["started_at"]),
                Src::Autohand => any_ts(&v["createdAt"]),
                Src::MinionCode => any_ts(&v["metadata"]["created_at"]),
                _ => None,
            }
            .unwrap_or(0);
            let end = match src {
                Src::GlmAgent => any_ts(&v["updatedAt"]),
                Src::Autohand => any_ts(&v["lastActiveAt"]),
                Src::VtCode => any_ts(&v["ended_at"]),
                Src::MinionCode => any_ts(&v["metadata"]["updated_at"]),
                _ => None,
            }
            .unwrap_or(start);
            let mut rows = if src == Src::MinionCode
                && v["agent_history"].is_array()
                && v["agent_history"].as_array().is_some_and(|r| !r.is_empty())
            {
                v["agent_history"].as_array().cloned().unwrap_or_default()
            } else if src == Src::Autohand {
                crate::pi::read_jsonl(&p.with_file_name("conversation.jsonl"))
            } else if src == Src::SiGit {
                crate::pi::read_jsonl(&p)
            } else {
                v["messages"].as_array().cloned().unwrap_or_default()
            };
            if src == Src::Autohand && v["branch"].is_object() {
                rows.retain(|m| any_ts(&m["timestamp"]).is_some_and(|ts| ts >= start));
            }
            Some((
                p,
                Record {
                    src,
                    id,
                    cwd,
                    model,
                    provider: v["metadata"]["provider"].as_str().map(str::to_string),
                    start,
                    end,
                    rows,
                },
            ))
        })
        .collect()
}
fn text(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
fn parse(r: &Record, pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    if !pm.in_project(&r.cwd) {
        return g;
    }
    let mut calls: HashMap<&str, (&str, Value)> = HashMap::new();
    let mut prompts = 0;
    let mut has_content = false;
    for m in &r.rows {
        if m["role"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("user"))
        {
            prompts += 1;
        }
        if m["role"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("assistant"))
        {
            has_content = true;
            for c in m["tool_calls"].as_array().into_iter().flatten() {
                let f = &c["function"];
                let a = f["arguments"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                if let (Some(id), Some(name)) = (c["id"].as_str(), f["name"].as_str()) {
                    calls.insert(id, (name, a));
                }
            }
        }
        if m["role"] != "tool" {
            continue;
        }
        let Some((name, a)) = calls.remove(m["tool_call_id"].as_str().unwrap_or("")) else {
            continue;
        };
        let receipt = text(&m["content"]);
        let ok = match (r.src, name) {
            (Src::GlmAgent, "edit_file") => receipt.starts_with("File edited successfully: "),
            (Src::SiGit, "create_file") => receipt.starts_with("Created file: "),
            (Src::SiGit, "edit_file") => receipt.starts_with("Edited file: "),
            (Src::SiGit, "multi_edit") => {
                receipt.starts_with("Applied ") && receipt.contains(" edits to ")
            }
            _ => false,
        };
        if !ok {
            continue;
        }
        let Some(path) = a["path"].as_str() else {
            continue;
        };
        let text = if name == "create_file" {
            a["content"].as_str().unwrap_or("").into()
        } else if name == "multi_edit" {
            a["edits"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|e| {
                    crate::added_lines(
                        e["old_text"].as_str().unwrap_or(""),
                        e["new_text"].as_str().unwrap_or(""),
                    )
                })
                .collect()
        } else {
            crate::added_lines(
                a["old_text"].as_str().unwrap_or(""),
                a["new_text"].as_str().unwrap_or(""),
            )
        };
        // Both stores persist the final selection, not a model on each message.
        crate::push_text(
            &mut g.evs,
            pm,
            &crate::pi::abs(&r.cwd, path),
            &text,
            r.src,
            "",
            &r.id,
            r.end,
        );
    }
    if matches!(r.src, Src::VtCode | Src::MinionCode | Src::Autohand) {
        edits::openai(r, pm, &mut g, &mut Default::default());
    }
    if r.src == Src::Dirac {
        edits::dirac(r, pm, &mut g);
    }
    g.sessions.push(Session {
        src: r.src,
        id: r.id.clone(),
        model: r.model.clone(),
        start: r.start,
        end: r.end,
        tokens: 0,
        branch: None,
        prompts,
        reviewer: false,
        has_content,
        provider: r.provider.clone(),
    });
    g
}
fn crow(c: &rusqlite::Connection, pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let Ok(mut q) =
        c.prepare("SELECT agent_id,session_id,cwd,model_identifier,created_at FROM agents")
    else {
        return g;
    };
    let agents: Vec<(String, String, String, String, String)> = q
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    for (id, _session, cwd, model, created) in agents {
        if !pm.in_project(&cwd) {
            continue;
        }
        let start = any_ts(&json!(created)).unwrap_or(0);
        let mut end = start;
        if let Ok(mut q)=c.prepare("SELECT acp_payload,created_at FROM subtool_calls WHERE agent_id=?1 AND status='completed' AND result_kind='diff' AND error IS NULL ORDER BY id"){
            for (raw, time) in q.query_map([&id], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, String>(1)?))).into_iter().flatten().flatten() {
                let v: Value = serde_json::from_str(&raw).unwrap_or_default();
                let time = any_ts(&json!(time)).unwrap_or(start);
                end = end.max(time);
                if v["content"] != "diff" { continue; }
                if let (Some(p), Some(n)) = (v["path"].as_str(), v["new_text"].as_str()) {
                    let added = crate::added_lines(v["old_text"].as_str().unwrap_or(""), n);
                    crate::push_text(&mut g.evs, pm, &crate::pi::abs(&cwd,p), &added, Src::Crow, "", &id, time);
                }
            }
        }
        let (prompts,tokens,last):(i64,i64,Option<String>)=c.query_row("SELECT SUM(CASE WHEN role='user' THEN 1 ELSE 0 END),COALESCE(SUM(total_tokens),0),MAX(created_at) FROM messages WHERE agent_id=?1",[&id],|r|Ok((r.get::<_,Option<i64>>(0)?.unwrap_or(0),r.get(1)?,r.get(2)?))).unwrap_or_default();
        end = end.max(last.and_then(|s| any_ts(&json!(s))).unwrap_or(start));
        g.sessions.push(Session {
            src: Src::Crow,
            id,
            model,
            start,
            end,
            tokens,
            branch: None,
            prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
    }
    g
}
fn stakpak(c: &rusqlite::Connection, pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let Ok(mut q) = c.prepare("SELECT id,cwd,created_at,updated_at FROM sessions") else {
        return g;
    };
    for (id, cwd, start, end) in q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .into_iter()
        .flatten()
        .flatten()
    {
        if !pm.in_project(&cwd) {
            continue;
        }
        let mut seen = std::collections::HashSet::new();
        let mut prompts = 0;
        let mut has_content = false;
        let mut model = String::new();
        if let Ok(mut q) =
            c.prepare("SELECT state FROM checkpoints WHERE session_id=? ORDER BY created_at,id")
        {
            for raw in q
                .query_map([&id], |r| r.get::<_, String>(0))
                .into_iter()
                .flatten()
                .flatten()
            {
                let v: Value = serde_json::from_str(&raw).unwrap_or_default();
                let r = Record {
                    src: Src::Stakpak,
                    id: id.clone(),
                    cwd: cwd.clone(),
                    model: String::new(),
                    provider: None,
                    start: 0,
                    end: 0,
                    rows: v["messages"].as_array().cloned().unwrap_or_default(),
                };
                prompts = prompts.max(r.rows.iter().filter(|m| m["role"] == "user").count() as i64);
                has_content |= r.rows.iter().any(|m| m["role"] == "assistant");
                if let Some(m) = r.rows.iter().rev().find_map(|m| m["model"]["id"].as_str()) {
                    model = m.into();
                }
                edits::openai(&r, pm, &mut g, &mut seen);
            }
        }
        g.sessions.push(Session {
            src: Src::Stakpak,
            id,
            model,
            start: any_ts(&json!(start)).unwrap_or(0),
            end: any_ts(&json!(end)).unwrap_or(0),
            tokens: 0,
            branch: None,
            prompts,
            reviewer: false,
            has_content,
            provider: None,
        });
    }
    g
}
fn bob(c: &rusqlite::Connection, pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let Ok(mut q) =
        c.prepare("SELECT id,project_id,env,created_at,updated_at,git_branch FROM tasks")
    else {
        return g;
    };
    for (id, cwd, env, start, end, branch) in q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .into_iter()
        .flatten()
        .flatten()
    {
        if !pm.in_project(&cwd) {
            continue;
        }
        let e: Value = env
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let (prompts, has_content) = c
            .query_row(
                "SELECT SUM(role='user'),SUM(role='assistant') FROM messages WHERE task_id=?",
                [&id],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        r.get::<_, Option<i64>>(1)?.unwrap_or(0) > 0,
                    ))
                },
            )
            .unwrap_or((0, false));
        let rows: Vec<Value> = c
            .prepare("SELECT data FROM messages WHERE task_id=? ORDER BY created_at,id")
            .ok()
            .into_iter()
            .flat_map(|mut q| {
                q.query_map([&id], |r| r.get::<_, String>(0))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .collect::<Vec<_>>()
            })
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect();
        edits::bob(
            &Record {
                src: Src::Bob,
                id: id.clone(),
                cwd: cwd.clone(),
                model: String::new(),
                provider: None,
                start: 0,
                end: 0,
                rows,
            },
            pm,
            &mut g,
        );
        g.sessions.push(Session {
            src: Src::Bob,
            id,
            model: e["model"]
                .as_str()
                .or_else(|| e["model"]["id"].as_str())
                .unwrap_or("")
                .into(),
            start: any_ts(&json!(start)).unwrap_or(0),
            end: any_ts(&json!(end)).unwrap_or(0),
            tokens: 0,
            branch,
            prompts,
            reviewer: false,
            has_content,
            provider: None,
        });
    }
    g
}
pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    let prefix = "longtail:native:";
    store.groups.retain(|k, _| !k.starts_with(prefix));
    for src in [
        Src::GlmAgent,
        Src::SiGit,
        Src::VtCode,
        Src::MinionCode,
        Src::Autohand,
        Src::Dirac,
    ] {
        for (p, r) in records(src) {
            let mut g = parse(&r, pm);
            g.fp = fingerprint(&p);
            store.groups.insert(
                format!("{prefix}{}:{}:{}", src.harness(), p.display(), r.id),
                g,
            );
        }
    }
    for p in crate::pi::jsonl_files(&data_dir(Src::Kimchi)) {
        let rows = crate::pi::read_jsonl(&p);
        let mut g = crate::pi::parse(Src::Kimchi, &rows, pm);
        g.fp = fingerprint(&p);
        store
            .groups
            .insert(format!("{prefix}kimchi:{}", p.display()), g);
    }
    for src in [Src::Crow, Src::Stakpak, Src::Bob] {
        let p = data_dir(src);
        if let Ok(c) = open_ro(&p) {
            let g = if src == Src::Crow {
                crow(&c, pm)
            } else if src == Src::Bob {
                bob(&c, pm)
            } else {
                stakpak(&c, pm)
            };
            store
                .groups
                .insert(format!("{prefix}{}:{}", src.harness(), p.display()), g);
        }
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<(Src, String), Activity> = HashMap::new();
    for src in [
        Src::GlmAgent,
        Src::SiGit,
        Src::VtCode,
        Src::MinionCode,
        Src::Autohand,
        Src::Dirac,
    ] {
        for (_, r) in records(src) {
            let e = out.entry((src, r.cwd.clone())).or_insert(Activity {
                src,
                cwd: r.cwd,
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(r.end);
        }
    }
    for p in crate::pi::jsonl_files(&data_dir(Src::Kimchi)) {
        let rows = crate::pi::read_jsonl(&p);
        if let Some(h) = rows.iter().find(|r| r["type"] == "session") {
            if let Some(cwd) = h["cwd"].as_str() {
                let last = rows
                    .iter()
                    .filter_map(|r| any_ts(&r["timestamp"]))
                    .max()
                    .unwrap_or(0);
                let e = out.entry((Src::Kimchi, cwd.into())).or_insert(Activity {
                    src: Src::Kimchi,
                    cwd: cwd.into(),
                    remote: None,
                    sessions: 0,
                    last: 0,
                });
                e.sessions += 1;
                e.last = e.last.max(last);
            }
        }
    }
    for (src, sql) in [
        (
            Src::Crow,
            "SELECT cwd,COUNT(*),MAX(created_at) FROM agents GROUP BY cwd",
        ),
        (
            Src::Stakpak,
            "SELECT cwd,COUNT(*),MAX(updated_at) FROM sessions GROUP BY cwd",
        ),
        (Src::Bob,"SELECT project_id,COUNT(*),CAST(MAX(updated_at) AS TEXT) FROM tasks GROUP BY project_id"),
    ] {
        if let Ok(c) = open_ro(&data_dir(src)) {
            if let Ok(mut q) = c.prepare(sql) {
                for (cwd, n, time) in q
                    .query_map([], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, u64>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    })
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    out.insert(
                        (src, cwd.clone()),
                        Activity {
                            src,
                            cwd,
                            remote: None,
                            sessions: n,
                            last: time.and_then(|s| any_ts(&json!(s))).unwrap_or(0),
                        },
                    );
                }
            }
        }
    }
    out.into_values().collect()
}
pub fn doctor() -> Vec<Health> {
    let activities = discover();
    [
        Src::GlmAgent,
        Src::SiGit,
        Src::Crow,
        Src::Bob,
        Src::Kimchi,
        Src::Autohand,
        Src::Dirac,
        Src::VtCode,
        Src::MinionCode,
        Src::Stakpak,
    ]
    .into_iter()
    .map(|src| Health {
        source: src.harness().into(),
        path: data_dir(src).display().to_string(),
        readable: data_dir(src).exists(),
        format: match src {
            Src::Crow => "crow.db agents/messages/subtool_calls",
            Src::Bob => "bob.db tasks/messages",
            Src::Kimchi => "native Pi SessionManager JSONL",
            Src::Autohand => "metadata.json + conversation.jsonl",
            Src::Dirac => "taskHistory + native API baseline/operations",
            Src::GlmAgent => "persisted session JSON v4",
            Src::SiGit => "session JSONL + meta sidecar",
            Src::VtCode => "SessionSnapshot JSON",
            Src::MinionCode => "SessionStorage JSON",
            _ => "local.db sessions/checkpoints",
        }
        .into(),
        records: activities
            .iter()
            .filter(|a| a.src == src)
            .map(|a| a.sessions)
            .sum(),
        note: match src {
            Src::Kimchi => creditia_core::tr!("原生 Pi 消息配对成功结果，保留每条 assistant 模型；独立核实 Kimchi 固定路径与 multi-edit 参数。", "Native Pi messages require matching successful results and preserve each assistant model. Kimchi's fixed store path and multi-edit contract were independently verified."),
            Src::GlmAgent | Src::SiGit => {
                creditia_core::tr!("原生成功回执配对产生行证据；历史无逐消息模型，行模型留空，会话保留最后选择。", "Concrete native success receipts produce line evidence. Historical messages lack per-call models, so edit models remain unknown and sessions retain the final selection.")
            }
            Src::Crow => {
                creditia_core::tr!("原生 completed subtool actual diff，失败/错误结果不算；agent模型可变，行模型留空，会话保留最后选择。", "Native completed subtool actual diffs exclude failed/error results. Stored agent models are mutable; edit models remain unknown and sessions retain the stored selection.")
            }
            Src::Dirac => creditia_core::tr!("回放原生API baseline与增量操作；写入需Saved成功回执，edit_file取实际结果diff；排除拒绝、人工改写与格式化，保留逐消息模型。", "Replays native API baselines and operations. Writes require Saved receipts; edit_file uses actual result diffs. Refusals, manual edits and formatting changes are excluded; per-message models are preserved."),
            Src::Bob => creditia_core::tr!("读取原生消息落盘后的_meta.changes实际before/after，排除isError和审批展示；消息不存逐次模型，行模型留空。", "Reads actual post-write before/after snapshots in native message _meta.changes, excluding isError and approval previews. Messages omit per-call models, so edit models remain unknown."),
            Src::VtCode => creditia_core::tr!("原生SessionSnapshot配对write_file实际字节数与diff；识别被compact删除的成功字段，排除错误、跳过及复用结果；逐次模型未保存。", "Pairs native SessionSnapshot write_file calls with actual byte counts and diffs, accounting for compacted-away success flags. Errors, skips and reused results are excluded; per-call models were not retained."),
            Src::MinionCode => creditia_core::tr!("读取原生agent_history的file_write/file_edit/multi_edit明确成功回执；仅有UI对话或已压缩丢失工具时不猜编辑，模型与时间未知则留空。", "Reads concrete file_write/file_edit/multi_edit success receipts from native agent_history. UI-only or compacted-away tool records never imply edits; unknown models and timestamps remain unknown."),
            Src::Autohand => creditia_core::tr!("读取git_apply_patch实际git apply成功回执；普通write/append/apply_patch在preview模式也返回相似结果且未保存模式，故排除。", "Reads git_apply_patch receipts after actual successful git apply. Ordinary write/append/apply_patch results are ambiguous in preview mode because that mode is not persisted, so they are excluded."),
            _ => creditia_core::tr!("原生checkpoint配对create/str_replace的明确成功回执与实际diff，失败不算，跨checkpoint按调用去重；仅保留实际消息存在的模型与时间。", "Pairs native checkpoint create/str_replace calls with concrete success receipts and actual diffs, excluding errors and deduplicating calls across checkpoints. Models and timestamps are retained only when present on actual messages."),
        },
        ..Default::default()
    })
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    #[test]
    fn six_sources_native_stores_reach_successful_edit_evidence() {
        let temp = std::env::temp_dir().join(format!(
            "creditia-native-edits-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let call = |id: &str, name: &str, args: Value| json!({"id":id,"function":{"name":name,"arguments":args.to_string()}});
        let openai_rows = |name: &str, args: Value, receipt: &str| {
            vec![
                json!({"role":"assistant","tool_calls":[call("ok",name,args)]}),
                json!({"role":"tool","tool_call_id":"ok","content":receipt}),
            ]
        };
        let vt = temp.join("vt");
        std::fs::create_dir_all(&vt).unwrap();
        let rows = vec![
            json!({"role":"Assistant","tool_calls":[call("ok","write_file",json!({"path":"a.rs","content":"vt_actual_line"}))]}),
            json!({"role":"Tool","tool_call_id":"ok","content":json!({"path":"a.rs","mode":"create_new","bytes_written":14,"diff":[{"path":"a.rs","content":"@@ -0,0 +1 @@\n+vt_actual_line"}]}).to_string()}),
        ];
        std::fs::write(
            vt.join("vt-session.json"),
            json!({"metadata":{"workspace_path":"/repo/","model":"final"},"messages":rows})
                .to_string(),
        )
        .unwrap();
        let minion = temp.join("minion");
        std::fs::create_dir_all(&minion).unwrap();
        std::fs::write(minion.join("minion.json"),json!({"metadata":{"session_id":"minion","project_path":"/repo/"},"messages":[{"role":"user","content":"UI-only"}],"agent_history":openai_rows("file_edit",json!({"file_path":"a.rs","old_string":"old","new_string":"minion_actual_line"}),"The file /repo/a.rs has been updated. Here's the result of the edit:\n1 | minion_actual_line")}).to_string()).unwrap();
        let autohand = temp.join("autohand/session");
        std::fs::create_dir_all(&autohand).unwrap();
        std::fs::write(
            autohand.join("metadata.json"),
            json!({"sessionId":"auto","projectPath":"/repo/","model":"final"}).to_string(),
        )
        .unwrap();
        let auto_rows = [
            json!({"role":"assistant","toolCalls":[{"id":"ok","tool":"git_apply_patch","args":{"patch":"--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +1 @@\n+autohand_actual_line"}}]}),
            json!({"role":"tool","tool_call_id":"ok","content":"Applied git patch."}),
        ];
        std::fs::write(
            autohand.join("conversation.jsonl"),
            auto_rows
                .iter()
                .map(|r| r.to_string() + "\n")
                .collect::<String>(),
        )
        .unwrap();
        let dirac = temp.join("dirac");
        std::fs::create_dir_all(dirac.join("state")).unwrap();
        std::fs::create_dir_all(dirac.join("tasks/d")).unwrap();
        std::fs::write(
            dirac.join("state/taskHistory.json"),
            json!([{"id":"d","cwdOnTaskInitialization":"/repo/","ts":1780000000000_i64}])
                .to_string(),
        )
        .unwrap();
        std::fs::write(dirac.join("tasks/d/api_conversation_history.json"),json!([{"role":"assistant","modelInfo":{"modelId":"per-message"},"content":[{"type":"tool_use","id":"ok","name":"edit_file","input":{"files":[{"path":"a.rs","edits":[{"anchor":"A","edit_type":"replace","text":"dirac_actual_line"}]}]}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"ok","content":"Applied 1 edit(s) successfully (+1, -1 lines).\n\nDiff (unanchored):\n\n-old\n+dirac_actual_line"}]}]).to_string()).unwrap();
        for (src, root) in [
            (Src::VtCode, vt),
            (Src::MinionCode, minion),
            (Src::Autohand, temp.join("autohand")),
            (Src::Dirac, dirac),
        ] {
            let rows = records_at(src, &root);
            assert_eq!(rows.len(), 1, "{}", src.harness());
            let g = parse(&rows[0].1, &pm());
            assert_eq!(g.evs.len(), 1, "{}", src.harness());
            assert_eq!(g.sessions.len(), 1);
        }
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE sessions(id TEXT,cwd TEXT,created_at TEXT,updated_at TEXT);CREATE TABLE checkpoints(id TEXT,session_id TEXT,state TEXT,created_at TEXT);INSERT INTO sessions VALUES('st','/repo/','2026-05-28T00:00:00Z','2026-05-28T00:01:00Z');").unwrap();
        let state=json!({"messages":openai_rows("create",json!({"path":"a.rs","file_text":"stakpak_actual_line"}),"Successfully created file /repo/a.rs with 1 lines")}).to_string();
        c.execute(
            "INSERT INTO checkpoints VALUES('a','st',?1,'2026-05-28T00:00:00Z')",
            [&state],
        )
        .unwrap();
        c.execute(
            "INSERT INTO checkpoints VALUES('b','st',?1,'2026-05-28T00:01:00Z')",
            [&state],
        )
        .unwrap();
        let g = stakpak(&c, &pm());
        assert_eq!(g.evs.len(), 1);
        assert!(g.sessions[0].has_content);
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tasks(id TEXT,project_id TEXT,env TEXT,created_at INTEGER,updated_at INTEGER,git_branch TEXT);CREATE TABLE messages(id TEXT,task_id TEXT,role TEXT,data TEXT,created_at INTEGER);INSERT INTO tasks VALUES('bob','/repo/','{\"model\":{\"id\":\"recorded-final\"}}',1780000000000,1780000010000,'main');").unwrap();
        let actual=json!({"role":"tool","toolUsage":{"signature":{"id":"ok","name":"write_file"}},"_meta":{"timestamp":1780000000000_i64,"changes":{"file:///repo/a.rs":{"before":"old","after":"bob_actual_line"}}}}).to_string();
        c.execute(
            "INSERT INTO messages VALUES('ok','bob','tool',?1,1780000000000)",
            [actual],
        )
        .unwrap();
        let g = bob(&c, &pm());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.sessions[0].model, "recorded-final");
        assert!(g.evs[0].model.is_empty());
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn bob_native_tasks_without_edit_results_remain_metadata_only() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tasks(id TEXT,project_id TEXT,env TEXT,created_at INTEGER,updated_at INTEGER,git_branch TEXT);CREATE TABLE messages(task_id TEXT,role TEXT);INSERT INTO tasks VALUES('bob-1','/repo/','{\"model\":\"saved-model\"}',1780000000000,1780000010000,'main');INSERT INTO messages VALUES('bob-1','user'),('bob-1','assistant');").unwrap();
        let g = bob(&c, &pm());
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(g.sessions[0].model, "saved-model");
        assert_eq!(g.sessions[0].prompts, 1);
        assert!(g.sessions[0].has_content);
        assert!(g.evs.is_empty());
    }
    #[test]
    fn dirac_native_history_does_not_treat_presentation_diffs_as_writes() {
        let v = json!({"id":"task-1", "ts":1780000000000_i64,"cwdOnTaskInitialization":"/repo/","modelId":"saved-model","tokensIn":10,"tokensOut":20});
        let r = dirac_record(&v).unwrap();
        let g = parse(&r, &pm());
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(g.sessions[0].model, "saved-model");
        assert_eq!(g.sessions[0].start, 1780000000);
        assert!(g.evs.is_empty());
        let mut goal = v.clone();
        goal["runKind"] = json!("goal");
        assert!(dirac_record(&goal).is_none());
    }
    #[test]
    fn glm_sigit_real_receipts_reject_errors_and_keep_model_boundary() {
        for src in [Src::GlmAgent, Src::SiGit] {
            let name = if src == Src::GlmAgent {
                "edit_file"
            } else {
                "create_file"
            };
            let success = if src == Src::GlmAgent {
                "File edited successfully: a.rs"
            } else {
                "Created file: /repo/a.rs (10 bytes)"
            };
            let r = Record {
                src,
                id: "s".into(),
                cwd: "/repo".into(),
                model: "last-model".into(),
                provider: None,
                start: 0,
                end: 0,
                rows: vec![
                    json!({"role":"assistant","tool_calls":[{"id":"ok","function":{"name":name,"arguments":json!({"path":"a.rs","content":"let confirmed = true;","old_text":"human_original_line","new_text":"human_original_line\nlet confirmed = true;"}).to_string()}},{"id":"bad","function":{"name":name,"arguments":json!({"path":"a.rs","content":"let failed = true;","old_text":"old","new_text":"let failed = true;"}).to_string()}}]}),
                    json!({"role":"tool","tool_call_id":"bad","content":"Error: denied"}),
                    json!({"role":"tool","tool_call_id":"ok","content":success}),
                ],
            };
            let g = parse(&r, &pm());
            assert_eq!(g.evs.len(), 1);
            assert_eq!(g.evs[0].model, "");
            assert_eq!(g.sessions[0].model, "last-model");
        }
    }
    #[test]
    fn crow_failed_or_dry_run_diffs_are_not_writes() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE agents(agent_id TEXT,session_id TEXT,cwd TEXT,model_identifier TEXT,created_at TEXT);CREATE TABLE messages(agent_id TEXT,role TEXT,total_tokens INTEGER,created_at TEXT);CREATE TABLE subtool_calls(id INTEGER,agent_id TEXT,status TEXT,result_kind TEXT,error TEXT,acp_payload TEXT,created_at TEXT);INSERT INTO agents VALUES('a','s','/repo','real-model','2026-10-17T00:00:00Z');").unwrap();
        let payload=json!({"content":"diff","path":"/repo/a.rs","old_text":"keep\nold","new_text":"keep\nlet actual = 1;"}).to_string();
        for (status, kind) in [
            ("completed", "diff"),
            ("failed", "diff"),
            ("completed", "rewrite"),
        ] {
            c.execute(
                "INSERT INTO subtool_calls VALUES(1,'a',?1,?2,NULL,?3,'2026-10-17T00:00:01Z')",
                [status, kind, &payload],
            )
            .unwrap();
        }
        let g = crow(&c, &pm());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.sessions[0].model, "real-model");
    }
    #[test]
    fn kimchi_native_pi_multi_edit_success_and_failure() {
        let rows = vec![
            json!({"type":"session","id":"kimchi-s","cwd":"/repo"}),
            json!({"type":"message","message":{"role":"assistant","model":"route-model-a","content":[{"type":"toolCall","id":"ok","name":"edit","arguments":{"path":"a.rs","edits":[{"oldText":"keep\nold","newText":"keep\nlet actual = true;"}]}},{"type":"toolCall","id":"bad","name":"write","arguments":{"path":"a.rs","content":"let failed = true;"}}]}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"bad","isError":true}}),
            json!({"type":"message","message":{"role":"toolResult","toolCallId":"ok","isError":false}}),
        ];
        let g = crate::pi::parse(Src::Kimchi, &rows, &pm());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].src, Src::Kimchi);
        assert_eq!(g.evs[0].model, "route-model-a");
    }
}
