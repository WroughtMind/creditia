//! Read a paginated lineage without assigning a parent's edits to its fork.
//! References name immutable rollout IDs (including the `_rollout-id` suffix
//! after a revert), and bound both the bytes and the exclusive ordinal.
use super::*;
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};

pub(super) fn snapshot(path: &Path, owner: &str) -> Result<(String, (u64, u64)), String> {
    let mut path = path.to_path_buf();
    let mut end: Option<(u64, usize)> = None;
    let mut seen = HashSet::new();
    let mut parts = vec![];
    let mut head = None;
    let mut hash = DefaultHasher::new();
    "codex-owned-lineage-v1".hash(&mut hash);
    let mut bytes = 0;
    loop {
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        if !seen.insert(canonical.clone()) {
            return Err("history_base cycle detected".into());
        }
        let before = fingerprint(&path);
        let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
        let mut raw = if path.extension().is_some_and(|e| e == "zst") {
            zstd::stream::decode_all(&raw[..]).map_err(|e| e.to_string())?
        } else {
            raw
        };
        if fingerprint(&path) != before {
            return Err("rollout changed while reading lineage".into());
        }
        if let Some((_, offset)) = end {
            if offset == 0 || offset > raw.len() || raw[offset - 1] != b'\n' {
                return Err("history_base byte boundary is outside a complete record".into());
            }
            raw.truncate(offset);
        }
        canonical.hash(&mut hash);
        before.hash(&mut hash);
        end.hash(&mut hash);
        raw.hash(&mut hash);
        bytes += raw.len() as u64;
        let text = String::from_utf8(raw).map_err(|e| e.to_string())?;
        let rows = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<Value>(l).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let first = rows.first().ok_or("empty rollout snapshot")?;
        let meta = &first["payload"];
        if head.is_none() {
            // Preserve legacy indexed rollouts that predate session_meta.
            if first["type"] != "session_meta" {
                if end.is_some() {
                    return Err("history_base source has no session metadata".into());
                }
                return Ok((text, (hash.finish(), bytes)));
            }
            if meta["id"] != owner {
                return Err("rollout belongs to another thread".into());
            }
            head = Some(first.to_string());
        } else if first["type"] != "session_meta" {
            return Err("history_base source has no session metadata".into());
        }
        let base = &meta["history_base"];
        let inherited = if base.is_null() {
            None
        } else {
            Some((
                base["thread_id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or("invalid history_base rollout ID")?,
                base["end_ordinal_exclusive"]
                    .as_u64()
                    .ok_or("invalid history_base ordinal")?,
                usize::try_from(
                    base["end_byte_offset"]
                        .as_u64()
                        .ok_or("invalid history_base byte offset")?,
                )
                .map_err(|e| e.to_string())?,
            ))
        };
        if meta["history_mode"] == "paginated" || inherited.is_some() || end.is_some() {
            if meta["history_mode"] != "paginated" {
                return Err("history_base source is not paginated".into());
            }
            let mut next = inherited.map_or(0, |(_, ordinal, _)| ordinal);
            for row in &rows {
                if row["ordinal"].as_u64() != Some(next) {
                    return Err(format!("paginated rollout ordinal gap at {next}"));
                }
                next = next.checked_add(1).ok_or("rollout ordinal overflow")?;
            }
            if end.is_some_and(|(ordinal, _)| next != ordinal) {
                return Err("history_base byte and ordinal boundaries disagree".into());
            }
        }
        if meta["id"] == owner {
            let start = meta["subagent_history_start_ordinal"].as_u64().unwrap_or(0);
            // Copied subagent prefixes retain the parent's ordinals, but are
            // context, not executions by this child.
            let own = rows
                .iter()
                .filter(|r| {
                    r["type"] == "session_meta"
                        || start == 0
                        || r["ordinal"].as_u64().is_some_and(|n| n >= start)
                })
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(own);
        }
        let Some((id, ordinal, offset)) = inherited else {
            break;
        };
        let root = canonical
            .ancestors()
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| n == "sessions" || n == "archived_sessions")
            })
            .and_then(Path::parent)
            .ok_or("history_base outside managed session roots")?;
        let files = rollout_files(root);
        let suffix = format!("-{id}.jsonl");
        let reverted = format!("_{id}.jsonl");
        let candidates: Vec<_> = files
            .iter()
            .filter(|(name, _)| name.ends_with(&suffix) || name.ends_with(&reverted))
            .collect();
        if candidates.len() != 1 {
            return Err(format!(
                "history_base {id}: missing or ambiguous source rollout"
            ));
        }
        path = candidates[0].1.clone();
        end = Some((ordinal, offset));
    }
    parts.reverse();
    let mut text = head.unwrap_or_default();
    for part in parts {
        text.push('\n');
        text.push_str(&part);
    }
    Ok((text, (hash.finish(), bytes)))
}
