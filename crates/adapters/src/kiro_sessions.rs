//! Kiro V3 native persistence, statically verified from official stable 2.32.0.
//! The ELF embeds a zstd KAS tar (not executed); @kiro/agent/dist/server/acp-server.js
//! contains M3/Qir Zod schemas, SessionPersistence.appendMessages/snapshotFile,
//! tool artifact conversion and FsAppend/StrReplace/Write handlers. Native
//! tool_result.success follows terminal Success/Accepted after the write promise.
//! PendingAction's legacy model-facing success is deliberately NOT sufficient.
//! session.modelId is mutable; model_route stores classifier categories, not
//! execution model IDs. Neither is assigned to historical edits.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

fn root() -> PathBuf {
    std::env::var("CREDITIA_KIRO_SESSIONS_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("CREDITIA_KIRO_DIR")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| crate::home().join(".kiro"))
                .join("sessions")
        })
}
fn files() -> Vec<PathBuf> {
    let mut out: Vec<_> = walkdir::WalkDir::new(root())
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name() == "session.json")
        .map(|e| e.into_path())
        .filter(|p| {
            p.parent()
                .is_some_and(|p| p.join("messages.jsonl").is_file())
        })
        .collect();
    out.sort();
    out
}
fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn rows(path: &Path) -> Result<Vec<Value>, String> {
    std::fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
        .collect()
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn pct(t: &str) -> Option<String> {
    let b = t.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
fn safe_relative(p: &str) -> bool {
    !p.is_empty()
        && !p.contains('\\')
        && Path::new(p)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
// URI helper mirrors native GSu/jSu. Only immutable snapshots in THIS session
// are read; live-file fallback URIs never prove a historical before/after pair.
fn snapshot(dir: &Path, sid: &str, uri: &str) -> Option<String> {
    let (base, query) = uri.split_once('?')?;
    if !base.starts_with("kiro-snapshot:")
        && !base.starts_with("kiro-snapshot-v2:")
        && !base.starts_with("kiro-session-snapshot:")
    {
        return None;
    }
    let q = pct(query)?;
    let params: HashMap<_, _> = q
        .split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| Some((pct(k)?, pct(v)?)))
        .collect::<Option<_>>()?;
    let authority = base
        .split_once("://")
        .map(|(_, p)| p.split('/').next().unwrap_or(""))
        .unwrap_or("");
    let (session, id) = if authority.is_empty() {
        (
            params.get("sessionId").map(String::as_str).unwrap_or(sid),
            params.get("snapshotId")?.as_str(),
        )
    } else {
        authority.split_once(':').unwrap_or((sid, authority))
    };
    if session != sid || !safe_relative(id) || id.contains('/') {
        return None;
    }
    let rel = params.get("originalPath")?;
    if !safe_relative(rel) {
        return None;
    }
    let bucket = dir.join("snapshots").join(id);
    let path = bucket.join(rel);
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(bucket.canonicalize().ok()?) {
        return None;
    }
    let text = std::fs::read_to_string(canonical).ok()?;
    let hash = std::fs::read_to_string(bucket.join(".hash")).ok()?;
    (hash == format!("{:x}", Sha256::digest(text.as_bytes()))).then_some(text)
}
#[derive(Default)]
struct Parsed {
    id: String,
    roots: Vec<String>,
    start: i64,
    end: i64,
    prompts: i64,
    edits: Vec<(String, String, i64)>,
}
fn absolute(path: &str, roots: &[String]) -> String {
    let path = crate::file_uri_path(path);
    if Path::new(&path).is_absolute() || path.get(1..2) == Some(":") {
        path
    } else if roots.len() == 1 {
        format!("{}/{path}", roots[0].trim_end_matches('/'))
    } else {
        String::new()
    }
}
fn parse(meta: &Value, messages: Vec<Value>, dir: &Path) -> Result<Parsed, String> {
    if meta["schemaVersion"] != "1.0.0"
        || s(meta, "id").is_empty()
        || !meta["workspacePaths"].is_array()
    {
        return Err("unrecognized Kiro KAS session metadata".into());
    }
    let mut p = Parsed {
        id: s(meta, "id").into(),
        roots: meta["workspacePaths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(crate::file_uri_path)
            .collect(),
        start: any_ts(&meta["createdAt"]).unwrap_or(0),
        end: any_ts(&meta["lastModifiedAt"]).unwrap_or(0),
        ..Default::default()
    };
    // Native checkpoint_revert removes the target itself and every later row;
    // summarization only hides context and does not undo actual file writes.
    let mut active: Vec<Value> = Vec::new();
    for row in messages {
        let b = &row["payload"];
        if b["type"] == "tombstone" && b["kind"] == "checkpoint_revert" {
            if let Some(i) = active
                .iter()
                .position(|r| r["id"] == b["effectiveFromMessageId"])
            {
                active.truncate(i);
            }
        } else {
            active.push(row);
        }
    }
    let fork = s(meta, "forkedAtMessageId");
    let from = if fork.is_empty() {
        0
    } else {
        active
            .iter()
            .position(|r| r["id"] == fork)
            .map(|i| i + 1)
            .ok_or("Kiro fork boundary missing from history")?
    };
    let reverted: HashSet<_> = active
        .iter()
        .filter(|r| r["payload"]["type"] == "tool_revert")
        .filter_map(|r| r["payload"]["toolCallId"].as_str())
        .collect();
    let mut calls: HashMap<String, Value> = HashMap::new();
    let mut seen = HashSet::new();
    for row in &active[from..] {
        let b = &row["payload"];
        let ts = any_ts(&row["timestamp"]).unwrap_or(p.start);
        if ts > 0 {
            p.end = p.end.max(ts);
        }
        match s(b, "type") {
            "user" if b["source"] != "hook" && b["source"] != "steer" => p.prompts += 1,
            "tool_call" if !s(b, "toolCallId").is_empty() => {
                calls.insert(s(b, "toolCallId").into(), b.clone());
            }
            "tool_result" if b["success"] == true => {
                let id = s(b, "toolCallId");
                if id.is_empty() || reverted.contains(id) || seen.contains(id) {
                    continue;
                }
                let Some(call) = calls.get(id) else {
                    continue;
                };
                if s(call, "executionId") != s(b, "executionId") {
                    continue;
                }
                if !matches!(s(call, "status"), "completed" | "approved") {
                    continue;
                }
                if matches!(s(&call["_meta"]["kiro"], "toolOrigin"), "client" | "acp") {
                    continue;
                }
                let args = &call["args"];
                let name = s(call, "toolName");
                if s(args, "path").is_empty() {
                    continue;
                }
                let path = absolute(s(args, "path"), &p.roots);
                if path.is_empty() {
                    continue;
                }
                let checkpoint = &b["_meta"]["kiro"]["checkpoint"];
                let diff = snapshot(dir, &p.id, s(checkpoint, "original"))
                    .zip(snapshot(dir, &p.id, s(checkpoint, "modified")))
                    .map(|(before, after)| added_lines(&before, &after));
                let text = if let Some(diff) = diff {
                    diff
                } else {
                    match name {
                        "str_replace" => match (args["oldStr"].as_str(), args["newStr"].as_str()) {
                            (Some(old), Some(new)) if !old.is_empty() => added_lines(old, new),
                            _ => String::new(),
                        },
                        "fs_append" => args["text"].as_str().unwrap_or("").into(),
                        // No creation inference from tool name or generic success.
                        // Overwrite and create need native immutable before/after.
                        _ => String::new(),
                    }
                };
                if !text.is_empty() {
                    p.edits.push((path, text, ts));
                }
                seen.insert(id.to_string());
            }
            _ => {}
        }
    }
    Ok(p)
}
fn load(file: &Path) -> Result<Parsed, String> {
    let dir = file.parent().ok_or("missing Kiro session parent")?;
    let meta = read(file)?;
    let mut messages = rows(&dir.join("messages.jsonl"))?;
    // Sub executions have their own call/result IDs and executionId. They are
    // appended after main only for parsing; no cross-stream model inference.
    let mut subs: Vec<_> = std::fs::read_dir(dir.join("sub-executions"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|s| s == "jsonl"))
        .collect();
    subs.sort();
    // Forks copy inherited sub-execution files unchanged. Checkpoint reverts
    // can also remove a parent sub-agent invocation. Until the relationship is
    // unambiguous, retain main receipts and exclude those detached sub streams.
    let detached = !s(&meta, "forkedAtMessageId").is_empty()
        || messages.iter().any(|r| {
            r["payload"]["type"] == "tombstone" && r["payload"]["kind"] == "checkpoint_revert"
        });
    if !detached {
        for sub in subs {
            messages.extend(rows(&sub)?);
        }
    }
    parse(&meta, messages, dir)
}
fn fp(file: &Path) -> (u64, u64) {
    walkdir::WalkDir::new(file.parent().unwrap_or(file))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| fingerprint(e.path()))
        .fold((0, 0), |(a, b), (c, d)| (a.max(c), b.wrapping_add(d)))
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for file in files() {
        let key = format!("kiro-kas:{}", file.display());
        let fp = fp(&file);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let p = match load(&file) {
            Ok(p) => p,
            Err(e) => {
                st.warnings
                    .push(format!("kiro-kas: {}: {e}", file.display()));
                continue;
            }
        };
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for (path, text, ts) in &p.edits {
            crate::push_text(&mut g.evs, pm, path, text, Src::Kiro, "", &p.id, *ts);
        }
        if g.evs.is_empty() && !p.roots.iter().any(|r| pm.in_project(r)) {
            continue;
        }
        g.sessions.push(Session {
            src: Src::Kiro,
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
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    files()
        .iter()
        .filter_map(|f| load(f).ok())
        .flat_map(|p| {
            p.roots.into_iter().map(move |cwd| Activity {
                src: Src::Kiro,
                cwd,
                remote: None,
                sessions: 1,
                last: p.end,
            })
        })
        .collect()
}
pub(crate) fn session_ids() -> HashSet<String> {
    files()
        .iter()
        .filter_map(|f| load(f).ok())
        .map(|p| p.id)
        .collect()
}
pub fn doctor() -> Health {
    let all = files();
    let parsed: Vec<_> = all.iter().filter_map(|f| load(f).ok()).collect();
    Health { source:"kiro-kas-v3".into(),path:root().display().to_string(),readable:!parsed.is_empty(),
        format:"KAS session.json 1.0.0 + messages.jsonl + sub-executions; immutable snapshots".into(),records:parsed.len()as u64,
        earliest:parsed.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_local),latest:parsed.iter().map(|p|p.end).filter(|&t|t>0).max().map(date_local),
        note:creditia_core::tr!("官方 2.32.0 内嵌 KAS 服务已静态核对。成功回执按调用/执行 ID 关联；拒绝、失败、未完成、撤销和 fork 继承部分不计。替换/追加读取实际新增；整文件写入要求完整且哈希一致的前后快照。当前选择模型不回填，逐编辑模型未知。构造记录验证，尚无真实会话验收。", "Official 2.32.0 embedded KAS service inspected statically. Successful receipts match call/execution IDs; denied, failed, pending, reverted and inherited fork edits are excluded. Replacements/appends use added text; whole-file writes require immutable checksum-verified snapshots. Mutable selected models are not backfilled; per-edit models remain unknown. Fixture validated; no real-session acceptance yet."),..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn meta() -> Value {
        json!({"schemaVersion":"1.0.0","id":"s","workspacePaths":["/repo"],"createdAt":"2026-10-17T00:00:00Z","modelId":"mutable-must-not-use"})
    }
    fn call(id: &str, name: &str, args: Value) -> Value {
        json!({"id":format!("{id}-call"),"timestamp":"2026-10-17T00:00:01Z","payload":{"type":"tool_call","toolCallId":id,"toolName":name,"args":args,"status":"completed","executionId":"e"}})
    }
    fn result(id: &str, ok: bool) -> Value {
        json!({"id":format!("{id}-result"),"timestamp":"2026-10-17T00:00:02Z","payload":{"type":"tool_result","toolCallId":id,"success":ok,"executionId":"e"}})
    }
    #[test]
    fn native_success_not_pending_failed_reverted_or_whole_overwrite() {
        let a = call(
            "a",
            "str_replace",
            json!({"path":"a.rs","oldStr":"human context","newStr":"human context\nadded"}),
        );
        let mut pending = call("p", "fs_append", json!({"path":"a.rs","text":"pending"}));
        pending["payload"]["status"] = json!("awaiting_approval");
        let rows = vec![
            a,
            result("a", true),
            pending,
            result("p", true),
            call("f", "fs_append", json!({"path":"a.rs","text":"failed"})),
            result("f", false),
            call(
                "w",
                "fs_write",
                json!({"path":"a.rs","text":"human context\nadded"}),
            ),
            result("w", true),
            call("r", "fs_append", json!({"path":"a.rs","text":"reverted"})),
            result("r", true),
            json!({"payload":{"type":"tool_revert","toolCallId":"r"}}),
        ];
        let p = parse(&meta(), rows, Path::new("/missing")).unwrap();
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].1, "added\n");
    }
    #[test]
    fn fork_and_checkpoint_revert_remove_inherited_and_undone_rows() {
        let mut m = meta();
        m["forkedAtMessageId"] = json!("a-result");
        let rows = vec![
            call("a", "fs_append", json!({"path":"a","text":"inherited"})),
            result("a", true),
            call("b", "fs_append", json!({"path":"a","text":"undone"})),
            result("b", true),
            json!({"payload":{"type":"tombstone","kind":"checkpoint_revert","effectiveFromMessageId":"b-call"}}),
            call("c", "fs_append", json!({"path":"a","text":"new"})),
            result("c", true),
        ];
        let p = parse(&m, rows, Path::new("/missing")).unwrap();
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].1, "new");
    }
    #[test]
    fn snapshot_checksum_and_path_boundary() {
        let dir = std::env::temp_dir().join(format!("creditia-kiro-kas-{}", std::process::id()));
        let bucket = dir.join("snapshots/1234");
        std::fs::create_dir_all(bucket.join("src")).unwrap();
        std::fs::write(bucket.join("src/a"), "before").unwrap();
        std::fs::write(
            bucket.join(".hash"),
            format!("{:x}", Sha256::digest(b"before")),
        )
        .unwrap();
        let uri = "kiro-snapshot-v2://s:1234/?originalPath=src%2Fa";
        assert_eq!(snapshot(&dir, "s", uri).as_deref(), Some("before"));
        assert!(snapshot(&dir, "other", uri).is_none());
        std::fs::write(bucket.join("src/a"), "tampered").unwrap();
        assert!(snapshot(&dir, "s", uri).is_none());
        assert!(snapshot(
            &dir,
            "s",
            "kiro-snapshot-v2://s:1234/?originalPath=..%2Fsecret"
        )
        .is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn whole_write_uses_immutable_saved_difference() {
        let dir = std::env::temp_dir().join(format!("creditia-kiro-whole-{}", std::process::id()));
        for (id, text) in [
            ("before", "human context"),
            ("after", "human context\nactual saved addition"),
        ] {
            let bucket = dir.join("snapshots").join(id);
            std::fs::create_dir_all(&bucket).unwrap();
            std::fs::write(bucket.join("a.rs"), text).unwrap();
            std::fs::write(
                bucket.join(".hash"),
                format!("{:x}", Sha256::digest(text.as_bytes())),
            )
            .unwrap();
        }
        let c = call(
            "w",
            "fs_write",
            json!({"path":"/repo/a.rs","text":"candidate must not count"}),
        );
        let mut r = result("w", true);
        r["payload"]["_meta"] = json!({"kiro":{"checkpoint":{"original":"kiro-snapshot-v2://s:before/?originalPath=a.rs","modified":"kiro-snapshot-v2://s:after/?originalPath=a.rs"}}});
        let p = parse(&meta(), vec![c, r], &dir).unwrap();
        assert_eq!(p.edits.len(), 1);
        assert_eq!(p.edits[0].1, "actual saved addition\n");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
