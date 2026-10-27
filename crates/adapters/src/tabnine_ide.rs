//! Tabnine IDE Agent, official console.tabnine.com/update/vscode/version
//! 3.366.0 VSIX inspected read-only (never run). opencode-agent/server/index.js:
//! store/sessionMap.ts maps conversation IDs to actual engine sessions;
//! store/conversations.ts atomically persists rich assistant.parts. The native
//! translator emits tool_result only on OpenCode completed/error; the stream
//! reducer sets tool-*.state=output-available/output-error and persists matching
//! data-toolmeta.diff. Generic UI text and mutable conversation.modelId are not
//! editing evidence or historical execution models. The Marketplace's old
//! 3.335.0 CHAT_CONVERSATIONS format is a separate text-only chat feature.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    // Native dataDir.ts returns this override unchanged: the final store dir.
    std::env::var("CREDITIA_TABNINE_IDE_DIR")
        .ok()
        .or_else(|| std::env::var("TABNINE_DATA_DIR").ok())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".tabnine/opencode-ide"))
}
fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
/// Confirmed host ownership for native OpenCode engine session IDs.
pub fn session_ids() -> HashSet<String> {
    read(&root().join("sessions.json"))
        .ok()
        .and_then(|v| v.as_object().cloned())
        .into_iter()
        .flatten()
        .filter_map(|(_, v)| {
            v.as_str()
                .filter(|s| !s.is_empty() && !s.starts_with("ses_mock_"))
                .map(String::from)
        })
        .collect()
}
#[derive(Default)]
struct Parsed {
    id: String,
    cwd: String,
    start: i64,
    end: i64,
    prompts: i64,
    edits: Vec<(String, String)>,
}
fn abs(cwd: &str, path: &str) -> String {
    let path = crate::file_uri_path(path);
    if Path::new(&path).is_absolute() || path.get(1..2) == Some(":") {
        path
    } else if !cwd.is_empty() && !path.is_empty() {
        format!("{}/{path}", cwd.trim_end_matches('/'))
    } else {
        String::new()
    }
}
// metadata.diff is the engine's actual completed edit diff. Reconcile removed
// and added lines too, so re-emitted unchanged context never becomes new code.
pub(crate) fn diff_added(diff: &str) -> Option<String> {
    let mut before = String::new();
    let mut after = String::new();
    let mut hunk = false;
    let mut saw_hunk = false;
    for line in diff.lines() {
        if line.starts_with("@@") {
            hunk = true;
            saw_hunk = true;
            continue;
        }
        if line.starts_with("diff ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            hunk = false;
            continue;
        }
        if !hunk {
            continue;
        }
        if let Some(t) = line.strip_prefix('-') {
            before.push_str(t);
            before.push('\n');
        } else if let Some(t) = line.strip_prefix('+') {
            after.push_str(t);
            after.push('\n');
        } else if let Some(t) = line.strip_prefix(' ') {
            before.push_str(t);
            before.push('\n');
            after.push_str(t);
            after.push('\n');
        }
    }
    saw_hunk.then(|| added_lines(&before, &after))
}
fn parse(c: &Value, map: &Value) -> Option<Parsed> {
    let cid = c["conversationId"].as_str()?;
    let id = map[cid].as_str()?;
    if id.is_empty() || id.starts_with("ses_mock_") || c["conversationType"] != "agent" {
        return None;
    }
    let mut p = Parsed {
        id: id.into(),
        cwd: c["rootPath"]
            .as_str()
            .map(crate::file_uri_path)
            .unwrap_or_default(),
        start: any_ts(&c["createdAt"]).unwrap_or(0),
        end: any_ts(&c["updatedAt"]).unwrap_or(0),
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for message in c["messages"].as_array().into_iter().flatten() {
        if message["role"] == "user" {
            p.prompts += 1;
            continue;
        }
        if message["role"] != "assistant" {
            continue;
        }
        let parts: Vec<_> = message["parts"].as_array().into_iter().flatten().collect();
        let meta: HashMap<_, _> = parts
            .iter()
            .filter(|v| v["type"] == "data-toolmeta")
            .filter_map(|v| Some((v["id"].as_str()?, &v["data"])))
            .collect();
        for t in parts.iter().filter(|t| t["state"] == "output-available") {
            let Some(id) = t["toolCallId"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let Some(name) = t["toolName"]
                .as_str()
                .or_else(|| t["type"].as_str()?.strip_prefix("tool-"))
            else {
                continue;
            };
            let name = name.strip_prefix("\u{21b3} ").unwrap_or(name);
            if !matches!(name, "edit" | "write") {
                continue;
            }
            if !matches!(name, "edit" | "write" | "multiedit") {
                continue;
            }
            let args = &t["input"];
            let path = args["filePath"]
                .as_str()
                .or(args["path"].as_str())
                .unwrap_or("");
            let path = abs(&p.cwd, path);
            if path.is_empty() {
                continue;
            }
            let m = meta.get(id).copied().unwrap_or(&Value::Null);
            if t["isError"] == true
                || t["errorText"].is_string()
                || m["exit"].as_i64().is_some_and(|n| n != 0)
            {
                continue;
            }
            let text = if let Some(diff) = m["diff"].as_str().and_then(diff_added) {
                diff
            } else if name == "edit" {
                match (args["oldString"].as_str(), args["newString"].as_str()) {
                    (Some(old), Some(new)) if !old.is_empty() => added_lines(old, new),
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            if !text.is_empty() {
                p.edits.push((path, text));
            }
        }
    }
    Some(p)
}
fn load() -> Result<Vec<Parsed>, String> {
    let map = read(&root().join("sessions.json"))?;
    let conversations = read(&root().join("conversations.json"))?;
    Ok(conversations
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| parse(c, &map))
        .collect())
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    if !root().join("conversations.json").is_file() {
        return;
    }
    let p = match load() {
        Ok(p) => p,
        Err(e) => {
            st.warnings.push(format!("tabnine-ide: {e}"));
            return;
        }
    };
    let a = fingerprint(&root().join("conversations.json"));
    let b = fingerprint(&root().join("sessions.json"));
    let fp = (a.0.max(b.0), a.1 + b.1);
    for p in p {
        let key = format!("tabnine-ide:{}:{}", root().display(), p.id);
        let native_session = store
            .groups
            .get(&format!("tabnine:{}", p.id))
            .is_some_and(|g| {
                g.sessions
                    .iter()
                    .any(|s| s.src == Src::Tabnine && s.id == p.id)
            });
        if store
            .groups
            .get(&format!("tabnine:{}", p.id))
            .is_some_and(|g| !g.evs.is_empty())
        {
            store.groups.remove(&key);
            continue;
        }
        if native_session {
            if let Some(g) = store.groups.get_mut(&key) {
                g.sessions.clear();
            }
        }
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for (path, text) in &p.edits {
            crate::push_text(&mut g.evs, pm, path, text, Src::Tabnine, "", &p.id, 0);
        }
        if g.evs.is_empty() && !pm.in_project(&p.cwd) {
            continue;
        }
        if !native_session {
            g.sessions.push(Session {
                src: Src::Tabnine,
                id: p.id,
                model: String::new(),
                start: p.start,
                end: p.end,
                tokens: 0,
                prompts: p.prompts,
                branch: None,
                reviewer: false,
                has_content: true,
                provider: None,
            });
        }
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    let native = crate::opencode::tabnine_native_session_ids();
    load()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.cwd.is_empty() && !native.contains(&p.id))
        .map(|p| Activity {
            src: Src::Tabnine,
            cwd: p.cwd,
            remote: None,
            sessions: 1,
            last: p.end,
        })
        .collect()
}
pub fn doctor() -> Health {
    let p = load().unwrap_or_default();
    let mut ids = crate::opencode::tabnine_native_session_ids();
    ids.extend(p.iter().map(|p| p.id.clone()));
    Health {source:"tabnine-ide".into(),path:root().display().to_string(),readable:!ids.is_empty(),
        format:"OpenCode native engine + conversations.json rich tool parts + sessions.json map".into(),records:ids.len()as u64,
        earliest:p.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_local),latest:p.iter().map(|p|p.end).filter(|&t|t>0).max().map(date_local),
        note:creditia_core::tr!("已只读核对官方公开 3.366.0 VSIX：原生会话映射关联底层 completed 工具记录，保留实际模型、时间与 token；底层缺失时读取 UI output-available 的实际 diff 或 old/new，模型和执行时间未知。失败、审批中、无差分整文覆盖不计，可变选择模型不回填。构造记录验证，尚无真实会话验收；旧 Marketplace 纯聊天不算编辑。", "Official public 3.366.0 VSIX inspected read-only. Mapped native completed tools retain actual model, time and tokens. UI output-available actual diffs or old/new edits provide fallback with unknown model/time. Errors, pending tools and unbounded overwrites are excluded; mutable model selections are not backfilled. Fixture validated, no real-session acceptance; old Marketplace text chat is not edit evidence."),..Default::default() }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn terminal_native_tool_parts_not_pending_errors_or_whole_content() {
        let c = json!({"conversationId":"c","conversationType":"agent","rootPath":"/repo","modelId":"mutable","messages":[{"role":"assistant","parts":[
            {"type":"tool-edit","state":"output-available","input":{"filePath":"a","oldString":"old","newString":"missing id must skip only this part"}},
            {"type":"tool-edit","toolCallId":"ok","state":"output-available","input":{"filePath":"a","oldString":"keep","newString":"keep\nadded"}},
            {"type":"tool-edit","toolCallId":"pending","state":"input-available","input":{"filePath":"a","oldString":"old","newString":"not saved"}},
            {"type":"tool-edit","toolCallId":"err","state":"output-error","input":{"filePath":"a","oldString":"old","newString":"failed"}},
            {"type":"tool-write","toolCallId":"whole","state":"output-available","input":{"filePath":"a","content":"keep\nadded"}},
            {"type":"tool-write","toolCallId":"diff","state":"output-available","input":{"filePath":"b","content":"ignored candidate"}},
            {"type":"data-toolmeta","id":"diff","data":{"diff":"--- b\n+++ b\n@@ -1 +1,2 @@\n-keep\n+keep\n+actual\n"}}
        ]}]});
        let map = json!({"c":"ses_real"});
        let p = parse(&c, &map).unwrap();
        assert_eq!(p.edits.len(), 2);
        assert_eq!(p.edits[0].1, "added\n");
        assert_eq!(p.edits[1].1, "actual\n");
        assert!(parse(&c, &json!({"c":"ses_mock_c"})).is_none());
        assert!(parse(&c, &json!({})).is_none());
    }
    #[test]
    fn final_header_keeps_prior_actual_hunk_and_unchanged_lines_cancel() {
        let diff="--- a\n+++ a\n@@ -1,2 +1,3 @@\n keep\n-context\n+context\n+actual\ndiff --git next next\n--- next\n+++ next\n";
        assert_eq!(diff_added(diff).as_deref(), Some("actual\n"));
        assert!(diff_added("candidate code without a diff hunk").is_none());
    }
}
