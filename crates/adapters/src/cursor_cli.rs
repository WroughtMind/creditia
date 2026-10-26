//! Cursor Agent CLI's real local store, checked read-only on 2026-10-17.
//! Installed official version: ~/.local/share/cursor-agent/versions/2026.08.11-e8db854.
//! 3363.index.js SqliteBlobStore: blobs(id,data), meta(key,value), key "0" is
//! hex-encoded UTF-8 JSON metadata. index.js AgentConversationTurnStructure keeps
//! model identity as encrypted_model; lastUsedModel is mutable session metadata
//! and must never label historical calls. No model decryption/guessing here.
//! Real observed successful Write/StrReplace results carry exact before/after
//! text under providerOptions.cursor.highLevelToolCallResult.output.success.
//! Only these persisted successful receipts produce evidence, once per toolCallId.

use crate::{added_lines, any_ts, open_ro, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn roots() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("CREDITIA_CURSOR_CLI_DIR") {
        return vec![PathBuf::from(p)];
    }
    let base = crate::source_dir(Src::Cursor);
    vec![base.join("chats"), base.join("acp-sessions")]
}
fn dbs() -> Vec<PathBuf> {
    let mut out: Vec<_> = roots()
        .iter()
        .flat_map(|p| {
            walkdir::WalkDir::new(p)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && e.file_name() == "store.db")
                .map(|e| e.into_path())
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort();
    out.dedup();
    out
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    s.as_bytes()
        .chunks(2)
        .map(|b| {
            std::str::from_utf8(b)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
        })
        .collect()
}
#[derive(Default)]
struct Parsed {
    id: String,
    start: i64,
    prompts: i64,
    edits: Vec<(String, String)>,
}

fn parse_messages(metadata: &Value, messages: &[Value]) -> Parsed {
    let mut p = Parsed {
        id: metadata["agentId"].as_str().unwrap_or("").into(),
        start: any_ts(&metadata["createdAt"]).unwrap_or(0),
        ..Default::default()
    };
    let mut seen = HashSet::new();
    let mut seen_prompts = HashSet::new();
    for m in messages {
        if m["role"] == "user" {
            if let Some(id) = m["id"].as_str() {
                if seen_prompts.insert(id) {
                    p.prompts += 1;
                }
            }
        }
        let result = &m["providerOptions"]["cursor"]["highLevelToolCallResult"];
        if result["isError"] != false || !result["output"]["success"].is_object() {
            continue;
        }
        let success = &result["output"]["success"];
        let Some(path) = success["path"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(after) = success["afterFullFileContent"].as_str() else {
            continue;
        };
        let content = m["content"].as_array().into_iter().flatten();
        for block in content.filter(|b| {
            b["type"] == "tool-result"
                && matches!(b["toolName"].as_str(), Some("Write" | "StrReplace"))
        }) {
            let Some(id) = block["toolCallId"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let text = match block["toolName"].as_str() {
                Some("StrReplace") => {
                    let Some(before) = success["beforeFullFileContent"].as_str() else {
                        continue;
                    };
                    added_lines(before, after)
                }
                Some("Write") => {
                    let Some(before) = success["beforeFullFileContent"].as_str() else { continue; };
                    added_lines(before, after)
                },
                _ => continue,
            };
            if !text.is_empty() && (Path::new(path).is_absolute() || path.get(1..2) == Some(":")) {
                p.edits.push((crate::file_uri_path(path), text));
            }
        }
    }
    p
}

fn read_db(db: &Path) -> Result<Parsed, String> {
    let c = open_ro(db).map_err(|e| e.to_string())?;
    let meta: String = c
        .query_row("SELECT value FROM meta WHERE key = '0'", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let metadata: Value = unhex(&meta)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| "unrecognized metadata encoding".to_string())?;
    let mut q = c
        .prepare("SELECT data FROM blobs ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| r.get::<_, Vec<u8>>(0))
        .map_err(|e| e.to_string())?;
    // Binary protobuf blobs include root prompt references and encrypted model
    // slots; only canonical JSON messages are accepted as edit receipts.
    let messages: Vec<Value> = rows
        .flatten()
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .filter(|v: &Value| v["role"].is_string() && v["content"].is_array())
        .collect();
    let mut p = parse_messages(&metadata, &messages);
    if p.id.is_empty() {
        p.id = db
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .into();
    }
    Ok(p)
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut count = 0;
    for db in dbs() {
        let key = format!("cursor-cli:{}", db.display());
        let a = fingerprint(&db);
        let b = fingerprint(&PathBuf::from(format!("{}-wal", db.display())));
        let fp = (a.0.max(b.0), a.1 + b.1);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            count += 1;
            continue;
        }
        let p = match read_db(&db) {
            Ok(p) => p,
            Err(e) => {
                st.warnings
                    .push(format!("cursor-cli: cannot read {}: {e}", db.display()));
                continue;
            }
        };
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for (path, text) in &p.edits {
            crate::push_text(&mut g.evs, pm, path, text, Src::Cursor, "", &p.id, p.start);
        }
        if g.evs.is_empty() {
            continue;
        }
        g.sessions.push(Session {
            src: Src::Cursor,
            id: p.id,
            model: String::new(),
            start: p.start,
            end: p.start,
            tokens: 0,
            branch: None,
            prompts: p.prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
        store.groups.insert(key, g);
        count += 1;
    }
    if count > 0 {
        st.notes.push(format!("cursor-cli: {count} native sessions; successful before/after edit receipts. Historical per-call models remain unknown (encrypted model slots)."));
    }
}

fn repo_of(path: &str) -> Option<String> {
    Path::new(path)
        .ancestors()
        .find(|p| p.join(".git").exists())
        .map(|p| p.display().to_string())
}
pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    for db in dbs() {
        if let Ok(p) = read_db(&db) {
            let repos: HashSet<String> = p.edits.iter().filter_map(|e| repo_of(&e.0)).collect();
            for cwd in repos {
                let e = out.entry(cwd.clone()).or_insert(Activity {
                    src: Src::Cursor,
                    cwd,
                    remote: None,
                    sessions: 0,
                    last: 0,
                });
                e.sessions += 1;
                e.last = e.last.max(p.start);
            }
        }
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    let files = dbs();
    let parsed: Vec<_> = files.iter().filter_map(|p| read_db(p).ok()).collect();
    Health{source:"cursor-cli".into(),path:roots().iter().map(|p|p.display().to_string()).collect::<Vec<_>>().join("; "),readable:!parsed.is_empty(),format:"store.db blobs + hex-JSON meta; successful highLevelToolCallResult before/after receipts".into(),records:parsed.len() as u64,earliest:parsed.iter().map(|p|p.start).filter(|&t|t>0).min().map(date_cn),latest:parsed.iter().map(|p|p.start).filter(|&t|t>0).max().map(date_cn),note:creditia_core::tr!("发现 {} 个库，{} 个可读。已用官方 CLI 2026.08.11-e8db854 与本机真实格式核对。历史模型已加密，不使用可变 lastUsedModel 回填。缺少逐编辑时间，采用会话创建时间。", "{} databases found, {} readable. Real local format verified against official CLI 2026.08.11-e8db854. Historical models encrypted; mutable lastUsedModel ignored. Individual edit times unavailable; session creation time is used.",files.len(),parsed.len()),..Default::default()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actual_success_receipts_and_snapshot_dedup() {
        let ok = json!({"role":"tool","content":[{"type":"tool-result","toolCallId":"call-a","toolName":"StrReplace","result":"display only"}],"providerOptions":{"cursor":{"highLevelToolCallResult":{"isError":false,"output":{"success":{"path":"/repo/x.rs","beforeFullFileContent":"keep","afterFullFileContent":"keep\nadded line"}}}}}});
        let denied = json!({"role":"tool","content":[{"type":"tool-result","toolCallId":"call-b","toolName":"Write"}],"providerOptions":{"cursor":{"highLevelToolCallResult":{"isError":true,"output":{"rejected":{}}}}}});
        let p = parse_messages(
            &json!({"agentId":"cli-session","createdAt":1790000000000i64,"lastUsedModel":{"modelId":"current-must-not-backfill"}}),
            &[ok.clone(), ok, denied],
        );
        assert_eq!(p.id, "cli-session");
        assert_eq!(p.edits, vec![("/repo/x.rs".into(), "added line\n".into())]);
    }
    #[test]
    fn result_without_explicit_success_does_not_count() {
        let v = json!({"role":"tool","content":[{"type":"tool-result","toolCallId":"pending","toolName":"Write","result":"success"}],"providerOptions":{"cursor":{"highLevelToolCallResult":{"isError":false,"output":{"failure":{"message":"failed"}}}}}});
        assert!(parse_messages(&Value::Null, &[v]).edits.is_empty());
        assert_eq!(unhex("7b7d"), Some(b"{}".to_vec()));
        assert!(unhex("7bgd").is_none());
        let overwrite=json!({"role":"tool","content":[{"type":"tool-result","toolCallId":"overwrite","toolName":"Write"}],"providerOptions":{"cursor":{"highLevelToolCallResult":{"isError":false,"output":{"success":{"path":"/repo/x.rs","afterFullFileContent":"human context\nnew line"}}}}}});
        assert!(parse_messages(&Value::Null,&[overwrite]).edits.is_empty());
    }
}
