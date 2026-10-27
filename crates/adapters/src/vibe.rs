//! Mistral Vibe session metadata + messages.jsonl, verified upstream 4ae5c59.
//! PersistedToolResult is emitted only for a successful response (agent_loop_hooks.py).
//! Historical messages have no model or timestamp: never relabel all edits with
//! metadata.config.active_model, which is overwritten on every save.
use crate::{any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
pub fn data_dir() -> PathBuf {
    std::env::var("CREDITIA_MISTRAL_VIBE_DIR")
        .ok()
        .or_else(|| std::env::var("VIBE_HOME").ok())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".vibe"))
}
fn root() -> PathBuf {
    data_dir().join("logs/session")
}
fn read(p: &Path) -> Value {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn files() -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root())
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && (e.file_name() == "meta.json"
                    || e.file_name().to_string_lossy().starts_with("session_")
                        && e.path().extension().is_some_and(|e| e == "json"))
        })
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}
fn parts(p: &Path) -> (Value, Vec<Value>) {
    let v = read(p);
    if v["metadata"].is_object() {
        (
            v["metadata"].clone(),
            v["messages"].as_array().cloned().unwrap_or_default(),
        )
    } else {
        (
            v,
            crate::pi::read_jsonl(&p.with_file_name("messages.jsonl")),
        )
    }
}
fn cwd(meta: &Value) -> &str {
    meta["environment"]["working_directory"]
        .as_str()
        .unwrap_or("")
}
fn model(meta: &Value) -> (String, Option<String>) {
    let cfg = &meta["config"];
    let active = cfg["active_model"].as_str().unwrap_or("");
    let m = cfg["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["alias"] == active || m["name"] == active);
    m.map(|m| {
        (
            m["name"].as_str().unwrap_or(active).into(),
            m["provider"].as_str().map(str::to_string),
        )
    })
    .unwrap_or((active.into(), None))
}
fn parse(meta: &Value, rows: &[Value], pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    if !pm.in_project(cwd(meta)) {
        return g;
    }
    let id = meta["session_id"].as_str().unwrap_or("");
    let start = any_ts(&meta["start_time"]).unwrap_or(0);
    let end = any_ts(&meta["end_time"]).unwrap_or(start);
    let mut calls: HashMap<&str, (&str, Value)> = HashMap::new();
    let (mut prompts, mut assistant) = (0, false);
    for m in rows {
        if m["role"] == "assistant" {
            assistant = true;
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
        } else if m["role"] == "user" && m["injected"] != true {
            prompts += 1;
        }
        if m["role"] != "tool" {
            continue;
        }
        let Some((name, _args)) = calls.remove(m["tool_call_id"].as_str().unwrap_or("")) else {
            continue;
        };
        let r = &m["tool_result"];
        if !r.is_object() || r["cancelled"] == true {
            continue;
        }
        let o = &r["output"];
        let edit = match name {
            "write_file" => o["file_path"]
                .as_str()
                .zip(o["content"].as_str())
                .map(|(p, s)| (p, s.into())),
            "edit" => o["file"]
                .as_str()
                .zip(o["new_string"].as_str())
                .map(|(p, s)| {
                    (
                        p,
                        crate::added_lines(o["old_string"].as_str().unwrap_or(""), s),
                    )
                }),
            _ => None,
        };
        if let Some((path, s)) = edit {
            crate::push_text(
                &mut g.evs,
                pm,
                &crate::pi::abs(cwd(meta), path),
                &s,
                Src::Vibe,
                "",
                id,
                end,
            );
        }
    }
    if assistant {
        let (model, provider) = model(meta);
        let stats = &meta["stats"];
        let tokens = stats["session_total_llm_tokens"]
            .as_i64()
            .unwrap_or_else(|| {
                stats["session_prompt_tokens"].as_i64().unwrap_or(0)
                    + stats["session_completion_tokens"].as_i64().unwrap_or(0)
            });
        g.sessions.push(Session {
            src: Src::Vibe,
            id: id.into(),
            model,
            start,
            end,
            tokens,
            branch: meta["git_branch"].as_str().map(str::to_string),
            prompts,
            reviewer: false,
            has_content: true,
            provider,
        });
    }
    g
}
pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    let files = files();
    let prefix = "mistral-vibe:session:";
    let keys: std::collections::HashSet<String> = files
        .iter()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(prefix) || keys.contains(k));
    for p in files {
        let a = fingerprint(&p);
        let b = fingerprint(&p.with_file_name("messages.jsonl"));
        let fp = (a.0.max(b.0), a.1 + b.1);
        let key = format!("{prefix}{}", p.display());
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let (meta, rows) = parts(&p);
        let mut g = parse(&meta, &rows, pm);
        g.fp = fp;
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    for p in files() {
        let (m, _) = parts(&p);
        let cwd = cwd(&m);
        if cwd.is_empty() {
            continue;
        }
        let last = any_ts(&m["end_time"])
            .or_else(|| any_ts(&m["start_time"]))
            .unwrap_or(0);
        let e = out.entry(cwd.into()).or_insert(Activity {
            src: Src::Vibe,
            cwd: cwd.into(),
            remote: None,
            sessions: 0,
            last: 0,
        });
        e.sessions += 1;
        e.last = e.last.max(last);
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    Health{source:"mistral-vibe".into(),path:root().display().to_string(),readable:root().is_dir(),format:"meta.json + messages.jsonl; legacy session_*.json".into(),records:discover().iter().map(|a|a.sessions).sum(),note:creditia_core::tr!("成功编辑取持久化工具结果；旧日志没有逐调用模型与时间，编辑模型留空，会话模型仅为末次配置。尚未真实运行验收。", "Successful edits come from persisted tool results. Historical logs lack per-call model and time: edit models remain unknown; the session model is the final configuration. Runtime validation remains pending."),..Default::default()}
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn successful_output_only_and_no_final_model_backfill() {
        let meta = json!({"session_id":"s","environment":{"working_directory":"/repo"},"config":{"active_model":"second","models":[{"alias":"second","name":"model-2","provider":"p"}]}});
        let rows = vec![
            json!({"role":"assistant","tool_calls":[{"id":"ok","function":{"name":"edit","arguments":"{}"}},{"id":"failed","function":{"name":"write_file","arguments":"{}"}},{"id":"cancelled","function":{"name":"write_file","arguments":"{}"}}]}),
            json!({"role":"tool","tool_call_id":"ok","tool_result":{"output":{"file":"/repo/a.rs","old_string":"keep\nold","new_string":"keep\nlet actual = true;"},"cancelled":false}}),
            json!({"role":"tool","tool_call_id":"failed","content":"<tool_error>permission denied</tool_error>"}),
            json!({"role":"tool","tool_call_id":"cancelled","tool_result":{"cancelled":true,"output":{"file_path":"/repo/a.rs","content":"let cancelled = true;"}}}),
        ];
        let pm = PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        };
        let g = parse(&meta, &rows, &pm);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.sessions[0].model, "model-2");
    }
}
