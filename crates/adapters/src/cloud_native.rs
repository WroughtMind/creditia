//! Imports original cloud-client response recordings; no network requests or inferred snapshots.
//! Replit contract checked from the current public client; see SUPPORT_CLOUD_NATIVE.md.
use crate::trace_import as imp;
use crate::{any_ts, source_dir, Activity, Health, Stats};
use base64::{engine::general_purpose::STANDARD, Engine};
use creditia_core::{
    model::{FileGroup, PathMap, Src, Store},
    tr,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

#[derive(Clone)]
struct Block {
    raw: Value,
    content: Value,
    valid: bool,
}
fn bytes_json(v: &Value) -> Option<Value> {
    serde_json::from_slice(&STANDARD.decode(v.as_str()?).ok()?).ok()
}
fn absolute(p: &str) -> bool {
    std::path::Path::new(p).is_absolute() || p.as_bytes().get(1) == Some(&b':')
}
// BlockPatch uses JSON Pointer paths and ADD/REPLACE/REMOVE. A missing target or
// unsupported operation invalidates this block instead of inventing partial state.
fn patch(v: &mut Value, p: &Value) -> Option<()> {
    let op = match p["op"].as_str() {
        Some("BLOCK_PATCH_OP_ADD") => 1,
        Some("BLOCK_PATCH_OP_REPLACE") => 2,
        Some("BLOCK_PATCH_OP_REMOVE") => 3,
        _ => p["op"].as_u64()?,
    };
    if !(1..=3).contains(&op) {
        return None;
    }
    let path = p["path"].as_str()?;
    let (parent, leaf) = path.rsplit_once('/')?;
    let leaf = leaf.replace("~1", "/").replace("~0", "~");
    let target = v.pointer_mut(parent)?;
    let value = if op != 3 {
        bytes_json(&p["value"])?
    } else {
        Value::Null
    };
    match target {
        Value::Object(m) => {
            if op != 1 && !m.contains_key(&leaf) {
                return None;
            }
            if op == 3 {
                m.remove(&leaf);
            } else {
                m.insert(leaf, value);
            }
        }
        Value::Array(a) => {
            let index = if leaf == "-" && op == 1 {
                a.len()
            } else {
                leaf.parse::<usize>().ok()?
            };
            if op == 1 {
                if index > a.len() {
                    return None;
                }
                a.insert(index, value);
            } else {
                if index >= a.len() {
                    return None;
                }
                if op == 3 {
                    a.remove(index);
                } else {
                    a[index] = value;
                }
            }
        }
        _ => return None,
    }
    Some(())
}
fn cancelled_or_live(v: &Value) -> bool {
    matches!(
        v["lifecycle"].as_str(),
        Some("BLOCK_LIFECYCLE_LIVE" | "BLOCK_LIFECYCLE_CANCELLED")
    ) || matches!(v["lifecycle"].as_u64(), Some(1 | 3))
}
// Original protobuf-JSON SyncBlocksResponse bodies have no conversation envelope.
// Their snapshots carry agentId/turnId; route later id-based operations only after
// a snapshot establishes an unambiguous owner. No session model/cwd is invented.
type RawOwners = HashMap<String, Option<(String, String)>>;
fn raw_owners(rows: &[Value]) -> RawOwners {
    let mut owners = RawOwners::new();
    for b in rows
        .iter()
        .flat_map(|row| row["ops"].as_array().into_iter().flatten())
        .filter_map(|op| op.get("block"))
    {
        let (Some(id), Some(agent), Some(turn)) = (
            b["id"].as_str(),
            b["agentId"].as_str(),
            b["turnId"].as_str(),
        ) else {
            continue;
        };
        if id.is_empty() || agent.is_empty() || turn.is_empty() {
            continue;
        }
        let owner = (agent.to_owned(), turn.to_owned());
        let prior = owners
            .entry(id.into())
            .or_insert_with(|| Some(owner.clone()));
        if prior.as_ref() != Some(&owner) {
            *prior = None;
        }
    }
    owners
}
fn raw_recordings(rows: &[Value], known_owners: &RawOwners) -> Vec<Value> {
    let mut recordings = vec![];
    let mut groups: HashMap<(String, String), Vec<Value>> = HashMap::new();
    let mut owners = known_owners.clone();
    for row in rows {
        for op in row["ops"].as_array().into_iter().flatten() {
            if let Some(b) = op.get("block") {
                let (Some(id), Some(agent), Some(turn)) = (
                    b["id"].as_str(),
                    b["agentId"].as_str(),
                    b["turnId"].as_str(),
                ) else {
                    continue;
                };
                if id.is_empty() || agent.is_empty() || turn.is_empty() {
                    continue;
                }
                let owner = (agent.to_owned(), turn.to_owned());
                let old = owners
                    .entry(id.into())
                    .or_insert_with(|| Some(owner.clone()));
                if old.as_ref() != Some(&owner) {
                    *old = None;
                }
                groups.entry(owner).or_default().push(op.clone());
            } else if let Some(reset) = op.get("reset") {
                let turn = reset["turnId"].as_str().unwrap_or("");
                for owner in owners.values().flatten() {
                    if turn.is_empty() || turn == owner.1 {
                        groups.entry(owner.clone()).or_default().push(op.clone());
                    }
                }
            } else {
                let id = op["update"]["id"]
                    .as_str()
                    .or_else(|| op["close"]["id"].as_str());
                if let Some(Some(owner)) = id.and_then(|id| owners.get(id)) {
                    groups.entry(owner.clone()).or_default().push(op.clone());
                }
            }
        }
    }
    for ((agent, turn), mut ops) in groups {
        // An id collision across owners leaves all related snapshots unusable.
        ops.retain(|op| {
            op["block"]["id"]
                .as_str()
                .is_none_or(|id| owners.get(id).is_some_and(Option::is_some))
        });
        let recording = serde_json::json!({"version":1,"constants":{"replId":"raw-response","agentId":agent,"conversationId":format!("{agent}/{turn}")},"events":[{"stream":"blocks","key":format!("raw-response/{agent}/{turn}"),"type":"chateau.frontend.v1.SyncBlocksResponse","payload":{"ops":ops}}]});
        recordings.push(recording);
    }
    recordings
}
#[cfg(test)]
fn parse_replit(rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let mut recordings = raw_recordings(rows, &raw_owners(rows));
    recordings.extend_from_slice(rows);
    parse_recordings(&recordings, pm, g);
}
fn completed(b: &Block) -> bool {
    b.valid && b.content["result"]["status"] == "completed" && !cancelled_or_live(&b.raw["status"])
}
fn compatible_observation(observed: &Block, done: &Block) -> bool {
    if !observed.valid || observed.raw["kind"] != done.raw["kind"] {
        return false;
    }
    if completed(observed) {
        return observed.content == done.content;
    }
    // A cancelled/invalid/inconsistent final observation cannot be overruled by
    // a successful copy from another export. Only a compatible live prefix may
    // precede the completed version of the same native block.
    if observed.content["result"]["status"] != "live"
        || matches!(
            observed.raw["status"]["lifecycle"].as_str(),
            Some("BLOCK_LIFECYCLE_CANCELLED")
        )
        || observed.raw["status"]["lifecycle"].as_u64() == Some(3)
    {
        return false;
    }
    let (mut old, mut new) = (observed.content.clone(), done.content.clone());
    old.as_object_mut().map(|o| o.remove("result"));
    new.as_object_mut().map(|o| o.remove("result"));
    let field = match done.raw["kind"].as_str() {
        Some("CreateFile") => "chunks",
        Some("FileEdit") => "unifiedDiff",
        _ => return false,
    };
    let text = |v: &Value| -> Option<String> {
        if field == "chunks" {
            Some(
                v.as_array()?
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<Vec<_>>>()?
                    .concat(),
            )
        } else {
            v.as_str().map(str::to_owned)
        }
    };
    let (Some(a), Some(b)) = (text(&old[field]), text(&new[field])) else {
        return false;
    };
    old.as_object_mut().map(|o| o.remove(field));
    new.as_object_mut().map(|o| o.remove(field));
    old == new && b.starts_with(&a)
}
fn parse_recordings(rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    // Every export is replayed independently: concatenating overlapping traces
    // would apply ADD patches twice. Reconcile their final observations by the
    // vendor's repl/agent/turn/block identity before emitting any evidence.
    let mut observations: BTreeMap<(String, String), Vec<(String, Block)>> = BTreeMap::new();
    let mut unresolved = HashSet::new();
    let mut reset_views = vec![];
    for recording in rows {
        // This is Replit's own feed-recording envelope, not a Creditia JSON dialect.
        let c = &recording["constants"];
        let (Some(repl), Some(agent), Some(events)) = (
            c["replId"].as_str(),
            c["agentId"].as_str(),
            recording["events"].as_array(),
        ) else {
            continue;
        };
        if recording["version"] != 1 || repl.is_empty() || agent.is_empty() {
            continue;
        }
        let prefix = format!("{repl}/{agent}/");
        let mut blocks: HashMap<(String, String), Block> = HashMap::new();
        let mut resets = HashSet::new();
        for event in events {
            if event["stream"] != "blocks"
                || event["type"] != "chateau.frontend.v1.SyncBlocksResponse"
            {
                continue;
            }
            let Some(key) = event["key"].as_str().filter(|k| k.starts_with(&prefix)) else {
                continue;
            };
            for op in event["payload"]["ops"].as_array().into_iter().flatten() {
                if op.get("reset").is_some() {
                    blocks.retain(|(k, _), _| k != key);
                    resets.insert(key.to_owned());
                } else if let Some(raw) = op.get("block") {
                    let Some(id) = raw["id"].as_str().filter(|id| !id.is_empty()) else {
                        continue;
                    };
                    let content = bytes_json(&raw["content"]);
                    blocks.insert(
                        (key.into(), id.into()),
                        Block {
                            raw: raw.clone(),
                            valid: content.is_some(),
                            content: content.unwrap_or_default(),
                        },
                    );
                } else if let Some(update) = op.get("update") {
                    let Some(id) = update["id"].as_str() else {
                        continue;
                    };
                    let Some(b) = blocks.get_mut(&(key.into(), id.into())) else {
                        unresolved.insert((key.to_owned(), id.to_owned()));
                        continue;
                    };
                    for p in update["patches"].as_array().into_iter().flatten() {
                        if patch(&mut b.content, p).is_none() {
                            b.valid = false;
                        }
                    }
                    if let Some(status) = update.get("status") {
                        b.raw["status"] = status.clone();
                    }
                } else if let Some(close) = op.get("close") {
                    let Some(id) = close["id"].as_str() else {
                        continue;
                    };
                    if let Some(b) = blocks.get_mut(&(key.into(), id.into())) {
                        b.raw["open"] = Value::Bool(false);
                        if let Some(ts) = close.get("closedAt") {
                            b.raw["closedAt"] = ts.clone();
                        }
                    } else {
                        unresolved.insert((key.to_owned(), id.to_owned()));
                    }
                }
            }
        }
        let sid = c["conversationId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(agent);
        for key in resets {
            let ids: HashSet<_> = blocks
                .keys()
                .filter(|(k, _)| k == &key)
                .map(|(_, id)| id.clone())
                .collect();
            reset_views.push((key, ids));
        }
        for (identity, block) in blocks {
            observations
                .entry(identity)
                .or_default()
                .push((sid.to_owned(), block));
        }
    }
    let mut sessions: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for ((key, id), copies) in observations {
        if unresolved.contains(&(key.clone(), id.clone()))
            || reset_views
                .iter()
                .any(|(k, ids)| k == &key && !ids.contains(&id))
        {
            continue;
        }
        let Some((sid, b)) = copies.iter().find(|(_, b)| completed(b)) else {
            continue;
        };
        if copies
            .iter()
            .any(|(_, other)| !compatible_observation(other, b))
        {
            continue;
        }
        // Conflicting native completion timestamps remain unknown. Do not pick
        // an arbitrary export's time or a recording/session timestamp.
        let mut ts = any_ts(&b.raw["closedAt"]).unwrap_or(0);
        if copies
            .iter()
            .filter(|(_, b)| completed(b))
            .any(|(_, other)| any_ts(&other.raw["closedAt"]).unwrap_or(0) != ts)
        {
            ts = 0;
        }
        let session = if copies.iter().all(|(other, _)| other == sid) {
            sid.as_str()
        } else {
            key.as_str()
        };
        let Some(path) = b.content["path"].as_str().filter(|p| absolute(p)) else {
            continue;
        };
        let text = match b.raw["kind"].as_str() {
            Some("CreateFile") => {
                let Some(chunks) = b.content["chunks"].as_array() else {
                    continue;
                };
                let Some(chunks) = chunks.iter().map(Value::as_str).collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                chunks.concat()
            }
            Some("FileEdit") => {
                let Some(diff) = b.content["unifiedDiff"].as_str() else {
                    continue;
                };
                let files = imp::diff_files(diff);
                let [(destination, text)] = files.as_slice() else {
                    continue;
                };
                if destination != path
                    && !path
                        .replace('\\', "/")
                        .ends_with(&format!("/{destination}"))
                {
                    continue;
                }
                text.clone()
            }
            _ => continue,
        };
        let before = g.evs.len();
        imp::push_scoped(g, pm, "", path, &text, Src::Replit, "", session, ts);
        if g.evs.len() > before {
            let times = sessions.entry(session.to_owned()).or_insert((ts, ts));
            times.0 = times.0.min(ts);
            times.1 = times.1.max(ts);
        }
    }
    for (sid, (start, end)) in sessions {
        g.sessions
            .push(imp::session(Src::Replit, &sid, "", start, end, 0, true));
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    scan_root(&source_dir(Src::Replit), store, pm);
}
fn scan_root(root: &Path, store: &mut Store, pm: &PathMap) {
    let mut paths = imp::files(root);
    paths.sort();
    let prefix = "replit:native-import:";
    // Rebuild the import set as one group so deleting a duplicate/conflicting
    // export cannot leave cached evidence behind in an unrelated file group.
    store.groups.retain(|k, _| !k.starts_with(prefix));
    if paths.is_empty() {
        return;
    }
    let files: Vec<_> = paths.iter().map(|p| imp::read(p)).collect();
    let all_rows: Vec<_> = files.iter().flatten().cloned().collect();
    let owners = raw_owners(&all_rows);
    let mut rows = vec![];
    for file_rows in files {
        rows.extend(raw_recordings(&file_rows, &owners));
        rows.extend(file_rows.into_iter().filter(|v| v["version"] == 1));
    }
    let mut g = FileGroup::default();
    parse_recordings(&rows, pm, &mut g);
    store
        .groups
        .insert(format!("{prefix}{}", root.display()), g);
}
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Vec<Health> {
    let root = source_dir(Src::Replit);
    let records = imp::files(&root)
        .iter()
        .flat_map(|p| imp::read(p))
        .filter(|v| {
            (v["version"] == 1 && v["constants"]["replId"].is_string() && v["events"].is_array())
                || v["ops"].is_array()
        })
        .count();
    vec![imp::health(Src::Replit, &root, "Replit Chateau SyncBlocksResponse / feed recording v1", records,
        &tr!("读取官方原始 SyncBlocksResponse 或 feed-recording v1 中已完成的 CreateFile/FileEdit。只接受绝对路径；工作目录与逐次模型未知。公开客户端关闭内置录制，可手动保存已有响应供离线导入。", "Reads completed CreateFile/FileEdit from original SyncBlocksResponse bodies or feed-recording v1. Requires absolute paths; cwd and per-write model are unknown. Built-in recording is disabled in the public client; manually saved existing responses can be imported offline."), false)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn encoded(v: Value) -> String {
        STANDARD.encode(serde_json::to_vec(&v).unwrap())
    }
    fn pm() -> PathMap {
        PathMap {
            known: ["src/a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    fn recording(ops: Value) -> Value {
        json!({"version":1,"constants":{"replId":"r","agentId":"a","conversationId":"c"},"events":[{"stream":"blocks","key":"r/a/t","type":"chateau.frontend.v1.SyncBlocksResponse","payload":{"ops":ops}}]})
    }
    fn block(kind: &str, content: Value) -> Value {
        json!({"block":{"id":"b","kind":kind,"turnId":"t","content":encoded(content),"openedAt":"2026-10-17T00:00:00Z"}})
    }
    #[test]
    fn replit_completed_create_and_patch_updates_are_evidence() {
        let v = recording(
            json!([block("CreateFile",json!({"path":"/demo/src/a.rs","chunks":["real_","line\n"],"result":{"status":"live"}})),{"update":{"id":"b","patches":[{"op":"BLOCK_PATCH_OP_REPLACE","path":"/result/status","value":encoded(json!("completed"))}]}},{"close":{"id":"b","closedAt":"2026-10-17T00:00:02Z"}}]),
        );
        let mut g = FileGroup::default();
        parse_replit(&[v], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.sessions[0].model, "");
        assert!(g.evs[0].single_use);
    }
    #[test]
    fn replit_completed_edit_counts_only_added_lines() {
        let diff =
            "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n unchanged\n-old_line\n+new_line\n";
        let mut g = FileGroup::default();
        parse_replit(
            &[recording(json!([block(
                "FileEdit",
                json!({"path":"/demo/src/a.rs","unifiedDiff":diff,"result":{"status":"completed"}})
            )]))],
            &pm(),
            &mut g,
        );
        assert_eq!(g.evs.len(), 1);
    }
    #[test]
    fn replit_pending_cancelled_read_relative_and_bad_updates_are_rejected() {
        for (kind, path, status) in [
            ("CreateFile", "/demo/src/a.rs", "live"),
            ("CreateFile", "/demo/src/a.rs", "cancelled"),
            ("ReadFile", "/demo/src/a.rs", "completed"),
            ("CreateFile", "src/a.rs", "completed"),
            ("CreateFile", "/outside/src/a.rs", "completed"),
        ] {
            let mut g = FileGroup::default();
            parse_replit(
                &[recording(json!([block(
                    kind,
                    json!({"path":path,"chunks":["not_evidence\n"],"result":{"status":status}})
                )]))],
                &pm(),
                &mut g,
            );
            assert!(g.evs.is_empty());
        }
        let b = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["not_evidence\n"],"result":{"status":"completed"}}),
        );
        for op in [
            json!({"update":{"id":"b","status":{"lifecycle":"BLOCK_LIFECYCLE_CANCELLED"}}}),
            json!({"update":{"id":"b","patches":[{"op":"BLOCK_PATCH_OP_REPLACE","path":"/absent","value":encoded(json!("bad"))}]}}),
            json!({"reset":{"turnId":"t"}}),
        ] {
            let mut g = FileGroup::default();
            parse_replit(&[recording(json!([b.clone(), op]))], &pm(), &mut g);
            assert!(g.evs.is_empty());
        }
    }
    #[test]
    fn replit_requires_original_envelope_and_correct_stream_scope() {
        let mut v = recording(json!([block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["no\n"],"result":{"status":"completed"}})
        )]));
        v["events"][0]["key"] = json!("other/agent/t");
        let mut g = FileGroup::default();
        parse_replit(&[v], &pm(), &mut g);
        assert!(g.evs.is_empty());
        parse_replit(
            &[json!({"success":true,"path":"/demo/src/a.rs","content":"no\n"})],
            &pm(),
            &mut g,
        );
        assert!(g.evs.is_empty());
    }
    #[test]
    fn replit_original_response_needs_snapshot_identity_and_keeps_time_unknown() {
        let mut b = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["raw_response_line\n"],"result":{"status":"completed"}}),
        );
        b["block"]["agentId"] = json!("agent");
        b["block"]["turnId"] = json!("turn");
        let mut g = FileGroup::default();
        parse_replit(&[json!({"ops":[b.clone()]})], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].ts, 0);
        assert_eq!(g.evs[0].session, "agent/turn");
        b["block"].as_object_mut().unwrap().remove("agentId");
        let mut g = FileGroup::default();
        parse_replit(&[json!({"ops":[b]})], &pm(), &mut g);
        assert!(g.evs.is_empty());
    }
    #[test]
    fn replit_scan_reconciles_duplicate_exports_by_native_block_not_text() {
        let root = std::env::temp_dir().join(format!(
            "creditia-replit-dedup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let done = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["created_once_line\n"],"result":{"status":"completed"}}),
        );
        let write = |name: &str, v: &Value| {
            std::fs::write(root.join(name), serde_json::to_vec(v).unwrap()).unwrap()
        };
        write("first.json", &recording(json!([done.clone()])));
        write("duplicate.json", &recording(json!([done.clone()])));
        let mut store = Store::default();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            1
        );
        let mut second = done.clone();
        second["block"]["id"] = json!("another-native-block");
        write("different-block.json", &recording(json!([second])));
        scan_root(&root, &mut store, &pm());
        assert_eq!(
            store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
            2,
            "distinct block ids retain two occurrences of identical text"
        );
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            1
        );
        write(
            "duplicate.json",
            &recording(
                json!([done.clone(), {"update":{"id":"b","patches":[{"op":"BLOCK_PATCH_OP_REPLACE","path":"/result/status","value":encoded(json!("cancelled"))}]}}]),
            ),
        );
        scan_root(&root, &mut store, &pm());
        assert_eq!(
            store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
            1,
            "a cancelled copy vetoes the old successful copy"
        );
        std::fs::remove_file(root.join("duplicate.json")).unwrap();
        scan_root(&root, &mut store, &pm());
        assert_eq!(
            store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
            2,
            "removing a conflicting export rebuilds the whole set"
        );
        write("reset.json", &recording(json!([{"reset":{"turnId":"t"}}])));
        scan_root(&root, &mut store, &pm());
        assert_eq!(
            store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
            0,
            "a reset-only export cannot revive older snapshots"
        );
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replit_overlap_preserves_incremental_completion_without_double_applying_add() {
        let initial = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["created_"],"result":{"status":"live"}}),
        );
        let complete = recording(
            json!([initial.clone(), {"update":{"id":"b","patches":[{"op":"BLOCK_PATCH_OP_ADD","path":"/chunks/-","value":encoded(json!("once_line\n"))},{"op":"BLOCK_PATCH_OP_REPLACE","path":"/result/status","value":encoded(json!("completed"))}]}}]),
        );
        let mut g = FileGroup::default();
        parse_replit(
            &[recording(json!([initial])), complete.clone(), complete],
            &pm(),
            &mut g,
        );
        assert_eq!(
            g.evs.len(),
            1,
            "overlapping ADD operations replay in each trace, not repeatedly across copies"
        );
        assert_eq!(g.sessions.len(), 1);
        let mut expected = FileGroup::default();
        imp::push_scoped(
            &mut expected,
            &pm(),
            "",
            "/demo/src/a.rs",
            "created_once_line\n",
            Src::Replit,
            "",
            "c",
            0,
        );
        assert_eq!(g.evs[0].key, expected.evs[0].key);
    }
    #[test]
    fn replit_conflicting_completed_bodies_and_orphan_updates_are_not_ordered_by_filename() {
        let done = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["first_body_line\n"],"result":{"status":"completed"}}),
        );
        let different = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["different_body_line\n"],"result":{"status":"completed"}}),
        );
        for other in [
            recording(json!([different])),
            recording(
                json!([{"update":{"id":"b","status":{"lifecycle":"BLOCK_LIFECYCLE_CANCELLED"}}}]),
            ),
        ] {
            for rows in [
                vec![recording(json!([done.clone()])), other.clone()],
                vec![other.clone(), recording(json!([done.clone()]))],
            ] {
                let mut g = FileGroup::default();
                parse_replit(&rows, &pm(), &mut g);
                assert!(g.evs.is_empty());
                assert!(g.sessions.is_empty());
            }
        }
    }
    #[test]
    fn replit_raw_response_files_share_identity_and_orphan_conflicts() {
        let root = std::env::temp_dir().join(format!(
            "creditia-replit-raw-dedup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut done = block(
            "CreateFile",
            json!({"path":"/demo/src/a.rs","chunks":["raw_created_once_line\n"],"result":{"status":"completed"}}),
        );
        done["block"]["agentId"] = json!("agent");
        done["block"]["turnId"] = json!("turn");
        let write = |name: &str, v: &Value| {
            std::fs::write(root.join(name), serde_json::to_vec(v).unwrap()).unwrap()
        };
        write("first.json", &json!({"ops":[done.clone()]}));
        write("copy.json", &json!({"ops":[done]}));
        let mut store = Store::default();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        assert_eq!(
            store
                .groups
                .values()
                .map(|g| g.sessions.len())
                .sum::<usize>(),
            1
        );
        write(
            "copy.json",
            &json!({"ops":[{"update":{"id":"b","status":{"lifecycle":"BLOCK_LIFECYCLE_CANCELLED"}}}]}),
        );
        scan_root(&root, &mut store, &pm());
        assert_eq!(
            store.groups.values().map(|g| g.evs.len()).sum::<usize>(),
            0,
            "global snapshot identity routes an update-only response conservatively"
        );
        std::fs::remove_file(root.join("copy.json")).unwrap();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
