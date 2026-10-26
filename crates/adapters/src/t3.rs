//! T3 Code host, verified pingdotgg/t3code cfafb4b. v2 projections preserve
//! driver, native session identity, per-run model and completed file-change diff.
//! Run LAST: prefer native adapters for (executor, native session id). Rebuild
//! host groups even when DB is unchanged so newly available native logs win.
use crate::{any_ts, open_ro, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
pub fn data_dir() -> PathBuf {
    ["CREDITIA_T3_DIR", "CREDITIA_T3_CODE_DIR", "T3CODE_HOME"]
        .iter()
        .find_map(|k| {
            std::env::var(k)
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| crate::home().join(".t3"))
}
fn dbs() -> Vec<PathBuf> {
    let root = data_dir();
    [
        "userdata/statev2.sqlite",
        "userdata/state.sqlite",
        "dev/statev2.sqlite",
        "dev/state.sqlite",
    ]
    .iter()
    .map(|p| root.join(p))
    .filter(|p| p.is_file())
    .collect()
}
fn payloads(c: &Connection, table: &str) -> Vec<Value> {
    let Ok(mut q) = c.prepare(&format!("SELECT payload_json FROM {table}")) else {
        return vec![];
    };
    q.query_map([], |r| r.get::<_, String>(0))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect()
}
fn keyed(rows: Vec<Value>) -> HashMap<String, Value> {
    rows.into_iter()
        .filter_map(|v| Some((v["id"].as_str()?.to_string(), v)))
        .collect()
}
fn src(driver: &str) -> Option<Src> {
    match driver {
        "codex" => Some(Src::Codex),
        "claudeAgent" => Some(Src::Claude),
        "muse" => Some(Src::Muse),
        "opencode" => Some(Src::OpenCode),
        "pi" => Some(Src::Pi),
        "omp" => Some(Src::Omp),
        "gemini" => Some(Src::Gemini),
        _ => None,
    }
}
fn projects(c: &Connection) -> HashMap<String, String> {
    let Ok(mut q) = c.prepare("SELECT project_id,workspace_root FROM projection_projects") else {
        return HashMap::new();
    };
    q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .into_iter()
        .flatten()
        .flatten()
        .collect()
}
fn cwd<'a>(
    pt: &Value,
    session: Option<&'a Value>,
    thread: Option<&'a Value>,
    projects: &'a HashMap<String, String>,
) -> &'a str {
    let _ = pt;
    session
        .and_then(|s| s["cwd"].as_str())
        .or_else(|| thread.and_then(|t| t["worktreePath"].as_str()))
        .or_else(|| {
            thread.and_then(|t| {
                projects
                    .get(t["projectId"].as_str().unwrap_or(""))
                    .map(String::as_str)
            })
        })
        .unwrap_or("")
}
fn native_id(pt: &Value) -> &str {
    pt["nativeThreadRef"]["nativeId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or(pt["providerSessionId"].as_str())
        .unwrap_or("")
}
fn parse_v2(c: &Connection, pm: &PathMap, native: &HashSet<(Src, String)>) -> FileGroup {
    let pts = payloads(c, "orchestration_v2_projection_provider_threads");
    let ss = keyed(payloads(c, "orchestration_v2_projection_provider_sessions"));
    let threads = keyed(payloads(c, "orchestration_v2_projection_threads"));
    let runs = keyed(payloads(c, "orchestration_v2_projection_runs"));
    let items = payloads(c, "orchestration_v2_projection_turn_items");
    let projects = projects(c);
    let mut g = FileGroup::default();
    let mut claimed = HashSet::new();
    for pt in pts {
        let Some(executor) = src(pt["driver"].as_str().unwrap_or("")) else {
            continue;
        };
        let id = native_id(&pt);
        if id.is_empty()
            || native.contains(&(executor, id.into()))
            || !claimed.insert((executor, id.to_string()))
        {
            continue;
        }
        let pid = pt["id"].as_str().unwrap_or("");
        let sid = pt["providerSessionId"].as_str().unwrap_or("");
        let s = ss.get(sid);
        let t = threads.get(pt["appThreadId"].as_str().unwrap_or(""));
        let cwd = cwd(&pt, s, t, &projects);
        if !pm.in_project(cwd) {
            continue;
        }
        let run_model = |run_id: &str| {
            runs.get(run_id)
                .and_then(|r| r["modelSelection"]["model"].as_str())
                .unwrap_or("")
        };
        let rs: Vec<_> = runs
            .values()
            .filter(|r| r["providerThreadId"] == pid)
            .collect();
        let model = rs
            .iter()
            .max_by_key(|r| r["ordinal"].as_u64().unwrap_or(0))
            .and_then(|r| r["modelSelection"]["model"].as_str())
            .or_else(|| s.and_then(|s| s["model"].as_str()))
            .unwrap_or("");
        let start = any_ts(&pt["createdAt"])
            .or_else(|| s.and_then(|s| any_ts(&s["createdAt"])))
            .unwrap_or(0);
        let end = any_ts(&pt["updatedAt"]).unwrap_or(start);
        for item in items
            .iter()
            .filter(|i| i["providerThreadId"] == pid && i["status"] == "completed")
        {
            let ts = any_ts(&item["completedAt"])
                .or_else(|| any_ts(&item["updatedAt"]))
                .unwrap_or(end);
            // A later model selection must never label an earlier run. Unknown
            // run identity leaves the edit model empty rather than guessing.
            let model = run_model(item["runId"].as_str().unwrap_or(""));
            if item["type"] == "file_change" {
                let p = item["fileName"].as_str().unwrap_or("");
                if p.is_empty() {
                    continue;
                }
                let text = if let Some(n) = item["newStr"].as_str() {
                    crate::added_lines(item["oldStr"].as_str().unwrap_or(""), n)
                } else {
                    crate::added_from_diff(item["diffStr"].as_str().unwrap_or(""))
                };
                crate::push_text(
                    &mut g.evs,
                    pm,
                    &crate::pi::abs(cwd, p),
                    &text,
                    executor,
                    model,
                    id,
                    ts,
                );
            } else if item["type"] == "command_execution"
                && item["exitCode"] == 0
                && item["outputIndicatesFailure"] != true
            {
                if let Some(cmd) = item["input"].as_str() {
                    for rel in crate::shell_targets(cmd, cwd, pm) {
                        crate::push_rel(&mut g.evs, &rel, cmd, executor, model, id, ts);
                    }
                }
            }
        }
        g.sessions.push(Session {
            src: executor,
            id: id.into(),
            model: model.into(),
            start,
            end,
            tokens: 0,
            branch: t.and_then(|t| t["branch"].as_str()).map(str::to_string),
            prompts: rs.len() as i64,
            reviewer: false,
            has_content: !rs.is_empty(),
            provider: None,
        });
    }
    g
}
fn legacy(c: &Connection, pm: &PathMap, native: &HashSet<(Src, String)>) -> FileGroup {
    let sql="SELECT s.provider_name,s.provider_session_id,t.model,t.created_at,t.updated_at,COALESCE(t.worktree_path,p.workspace_root),t.branch FROM projection_thread_sessions s JOIN projection_threads t USING(thread_id) JOIN projection_projects p USING(project_id)";
    let Ok(mut q) = c.prepare(sql) else {
        return FileGroup::default();
    };
    let mut g = FileGroup::default();
    for (driver, id, model, start, end, cwd, branch) in q
        .query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })
        .into_iter()
        .flatten()
        .flatten()
    {
        let Some(src) = src(&driver) else {
            continue;
        };
        if id.is_empty() || !pm.in_project(&cwd) || native.contains(&(src, id.clone())) {
            continue;
        }
        let parsed: Value = serde_json::from_str(&model).unwrap_or_default();
        let model = parsed["model"].as_str().unwrap_or(&model).into();
        g.sessions.push(Session {
            src,
            id,
            model,
            start: any_ts(&Value::String(start)).unwrap_or(0),
            end: any_ts(&Value::String(end)).unwrap_or(0),
            tokens: 0,
            branch,
            prompts: 0,
            reviewer: false,
            has_content: false,
            provider: None,
        });
    }
    g
}
fn native_sessions(store: &Store) -> HashSet<(Src, String)> {
    store
        .groups
        .iter()
        .filter(|(key, _)| !key.starts_with("acp:") && !key.starts_with("t3-code:host:"))
        .flat_map(|(_, g)| g.sessions.iter())
        .map(|s| (s.src, s.id.clone()))
        .collect()
}
pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    let prefix = "t3-code:host:";
    store.groups.retain(|k, _| !k.starts_with(prefix));
    let mut native = native_sessions(store);
    for p in dbs() {
        let Ok(c) = open_ro(&p) else {
            continue;
        };
        let mut g = parse_v2(&c, pm, &native);
        if c.query_row("SELECT 1 FROM sqlite_master WHERE type='table' AND name='orchestration_v2_projection_provider_threads'", [], |_| Ok(())).is_err() {
            g = legacy(&c, pm, &native);
        }
        for s in &g.sessions {
            native.insert((s.src, s.id.clone()));
        }
        store.groups.insert(format!("{prefix}{}", p.display()), g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    let mut seen = HashSet::new();
    for p in dbs() {
        let Ok(c) = open_ro(&p) else {
            continue;
        };
        let ss = keyed(payloads(
            &c,
            "orchestration_v2_projection_provider_sessions",
        ));
        let threads = keyed(payloads(&c, "orchestration_v2_projection_threads"));
        let projects = projects(&c);
        for pt in payloads(&c, "orchestration_v2_projection_provider_threads") {
            let sid = pt["providerSessionId"].as_str().unwrap_or("");
            let id = native_id(&pt);
            if !seen.insert((
                pt["driver"].as_str().unwrap_or("").to_string(),
                id.to_string(),
            )) {
                continue;
            }
            let s = ss.get(sid);
            let t = threads.get(pt["appThreadId"].as_str().unwrap_or(""));
            let cwd = cwd(&pt, s, t, &projects);
            if cwd.is_empty() {
                continue;
            }
            let e = out.entry(cwd.into()).or_insert(Activity {
                src: Src::T3,
                cwd: cwd.into(),
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(any_ts(&pt["updatedAt"]).unwrap_or(0));
        }
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    let v2=dbs().iter().filter_map(|p|open_ro(p).ok()).any(|c|c.query_row("SELECT 1 FROM sqlite_master WHERE type='table' AND name='orchestration_v2_projection_provider_threads'",[], |_|Ok(())).is_ok());
    Health{source:Src::T3.harness().into(),path:data_dir().join("userdata").display().to_string(),readable:!discover().is_empty(),format:if v2 {"statev2.sqlite v2 projections"} else {"legacy state.sqlite sessions"}.into(),records:discover().iter().map(|a|a.sessions).sum(),note:creditia_core::tr!("执行者取 driver，编辑模型取 run；原生日志按执行者与 nativeThreadRef 优先去重。v2 只认 completed 文件变更或零退出命令；旧 v1 当前只读会话。尚未真实运行验收。", "Executor comes from driver and edit model from its run. Native executor/session logs take precedence by nativeThreadRef. V2 accepts only completed file changes or zero-exit commands; v1 currently supplies sessions only. Runtime validation remains pending."),..Default::default()}
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE projection_projects(project_id TEXT,workspace_root TEXT);")
            .unwrap();
        for table in [
            "threads",
            "provider_sessions",
            "provider_threads",
            "runs",
            "turn_items",
        ] {
            c.execute_batch(&format!(
                "CREATE TABLE orchestration_v2_projection_{table}(payload_json TEXT)"
            ))
            .unwrap();
        }
        c.execute(
            "INSERT INTO projection_projects VALUES('project','/repo')",
            [],
        )
        .unwrap();
        let put = |t: &str, v: Value| {
            c.execute(
                &format!("INSERT INTO orchestration_v2_projection_{t} VALUES(?1)"),
                [v.to_string()],
            )
            .unwrap();
        };
        put("threads", json!({"id":"t","projectId":"project"}));
        put(
            "provider_sessions",
            json!({"id":"ps","cwd":"/repo","model":"last"}),
        );
        put(
            "provider_threads",
            json!({"id":"pt","driver":"muse","providerSessionId":"ps","appThreadId":"t","nativeThreadRef":{"nativeId":"native"}}),
        );
        for (id, m, ord) in [("r1", "first", 1), ("r2", "second", 2)] {
            put(
                "runs",
                json!({"id":id,"providerThreadId":"pt","ordinal":ord,"modelSelection":{"model":m}}),
            );
        }
        for (status, run, line) in [
            ("completed", "r1", "let first = 1;"),
            ("failed", "r1", "let failed = 1;"),
            ("cancelled", "r2", "let denied = 1;"),
            ("completed", "r2", "let second = 2;"),
        ] {
            put(
                "turn_items",
                json!({"providerThreadId":"pt","runId":run,"status":status,"type":"file_change","fileName":"a.rs","oldStr":"unchanged","newStr":format!("unchanged\n{line}")}),
            );
        }
        c
    }
    #[test]
    fn host_executor_model_switch_failed_edits_and_native_dedup() {
        let c = fixture();
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse_v2(&c, &pm, &HashSet::new());
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "first");
        assert_eq!(g.evs[1].model, "second");
        assert!(g.evs.iter().all(|e| e.src == Src::Muse));
        let native = [(Src::Muse, "native".into())].into_iter().collect();
        assert!(parse_v2(&c, &pm, &native).sessions.is_empty());
    }
    #[test]
    fn previous_acp_metadata_cannot_suppress_real_host_edits() {
        let c = fixture();
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let mut previous = parse_v2(&c, &pm, &HashSet::new());
        previous.evs.clear();
        let mut store = Store::default();
        store.groups.insert("acp:old-session".into(), previous);
        let g = parse_v2(&c, &pm, &native_sessions(&store));
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.sessions[0].id, "native");
    }
}
