//! Vercel fx v2 native journals: CRC32C-framed events and SHA-256 addressed JSON pieces.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_FX_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".fx"))
}
fn paths() -> Vec<PathBuf> {
    n::files(&root().join("sessions/v2"), |p| {
        p.file_name().is_some_and(|s| s == "log.jsonl")
    })
}
fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0x82f63b78u32.wrapping_mul(crc & 1));
        }
    }
    !crc
}
fn journal(path: &Path) -> Result<Vec<Value>, String> {
    let bytes = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut rows = vec![];
    for (sequence, line) in bytes.split_inclusive('\n').enumerate() {
        if !line.ends_with('\n') {
            break;
        }
        let (payload, tail) = line
            .rsplit_once(",\"crc\":\"")
            .ok_or("fx journal missing CRC frame")?;
        let digest = tail
            .strip_suffix("\"}\n")
            .ok_or("fx journal bad CRC frame")?;
        if digest.len() != 8
            || u32::from_str_radix(digest, 16).ok() != Some(crc32c(payload.as_bytes()))
        {
            return Err("fx journal CRC mismatch".into());
        }
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if row["v"] != 1 || row["seq"].as_u64() != Some(sequence as u64 + 1) {
            return Err("unsupported fx journal version/sequence".into());
        }
        rows.push(row);
    }
    Ok(rows)
}
fn piece(dir: &Path, value: &Value) -> Result<Value, String> {
    if let Some(hash) = value["$blob"].as_str() {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid fx blob hash".into());
        }
        let bytes = std::fs::read(dir.join("blobs").join(hash)).map_err(|e| e.to_string())?;
        if format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err("fx blob checksum mismatch".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    } else {
        Ok(value.clone())
    }
}
fn parse(rows: &[Value], dir: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let header = rows.first().ok_or("empty fx journal")?;
    if header["kind"] != "session_created" {
        return Err("fx session header missing".into());
    }
    let id = n::text(&header["id"]);
    let mut cwd = n::text(&header["workspace"]).to_string();
    let start = crate::any_ts(&header["ts"]).unwrap_or(0);
    let fork_seq = header["forked_from"]["seq"].as_u64().unwrap_or(0);
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let mut model = String::new();
    let mut end = start;
    let mut prompts = 0;
    for row in rows {
        if row["kind"] == "set" {
            if row["key"] == "prefs" {
                model = n::text(&row["value"]["model"]).into();
            }
            if row["key"] == "workspace" {
                if let Some(s) = row["value"].as_str() {
                    cwd = s.into();
                }
            }
            continue;
        }
        if row["kind"] != "item" {
            continue;
        }
        let data = piece(dir, &row["data"])?;
        let ts = crate::any_ts(&row["ts"]).unwrap_or(start);
        end = end.max(ts);
        if row["seq"].as_u64().unwrap_or(0) <= fork_seq {
            continue;
        }
        match n::text(&row["type"]) {
            "user" => prompts += 1,
            "tool_call" | "tool_running" => {
                let cid = n::text(&data["call_id"]);
                if !cid.is_empty()
                    && data["argument_integrity"] != "invalid"
                    && data["provenance"] != "provider_native"
                {
                    calls.insert(cid.to_string(), (data, model.clone()));
                }
            }
            "tool_result" => {
                let cid = n::text(&data["call_id"]);
                let Some((call, model)) = calls.remove(cid) else {
                    continue;
                };
                if data["status"] != "success"
                    || data["provider_native"] == true
                    || data["review_feedback"] == true
                    || data["tool_name"] != call["tool_name"]
                    || !seen.insert(cid.to_string())
                {
                    continue;
                }
                if !matches!(n::text(&call["tool_name"]), "write_file" | "edit_file") {
                    continue;
                }
                let presentation = &data["committed_file_presentation"];
                let path = n::text(&presentation["path"]);
                if path.is_empty() {
                    continue;
                }
                let added = if presentation["after_content"].is_string()
                    && presentation["content_handle"].is_null()
                {
                    crate::added_lines(
                        n::text(&presentation["previous_content"]),
                        n::text(&presentation["after_content"]),
                    )
                } else if presentation["truncated"] == false {
                    presentation["lines"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|l| l["kind"] == "addition")
                        .filter_map(|l| l["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    continue;
                };
                imp::push_scoped(&mut g, pm, &cwd, path, &added, Src::Fx, &model, id, ts);
            }
            _ => {}
        }
    }
    n::session(&mut g, pm, Src::Fx, id, &cwd, start, end, prompts, 0);
    Ok(g)
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    parse(
        &journal(path)?,
        path.parent().ok_or("missing fx session directory")?,
        pm,
    )
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Fx, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    paths()
        .iter()
        .filter_map(|p| {
            let rows = journal(p).ok()?;
            let h = rows.first()?;
            Some(imp::activity(
                Src::Fx,
                n::text(&h["workspace"]).into(),
                None,
                rows.iter()
                    .filter_map(|r| crate::any_ts(&r["ts"]))
                    .max()
                    .unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Fx,&root(),"native v2 CRC32C journal + SHA-256 pieces",paths().len(),"Successful committed_file_presentation additions. Truncated preview-only receipts and legacy v1 binary event frames are excluded; complete inline post-save content takes precedence.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tampered_journal_and_external_blob_are_rejected() {
        let dir = std::env::temp_dir().join(format!(
            "creditia-fx-codec-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("blobs")).unwrap();
        let payload="{\"v\":1,\"seq\":1,\"ts\":1727400000000,\"kind\":\"session_created\",\"id\":\"a\",\"workspace\":\"/demo\"";
        let frame = format!(
            "{payload},\"crc\":\"{:08x}\"}}\n",
            crc32c(payload.as_bytes())
        );
        let path = dir.join("log.jsonl");
        std::fs::write(&path, &frame).unwrap();
        assert_eq!(journal(&path).unwrap().len(), 1);
        std::fs::write(&path, frame.replace("/demo", "/other")).unwrap();
        assert!(journal(&path).unwrap_err().contains("CRC"));
        let bytes = b"{\"saved\":true}";
        let hash = format!("{:x}", Sha256::digest(bytes));
        std::fs::write(dir.join("blobs").join(&hash), bytes).unwrap();
        assert_eq!(piece(&dir, &json!({"$blob":hash})).unwrap()["saved"], true);
        std::fs::write(dir.join("blobs").join(&hash), b"tampered").unwrap();
        assert!(piece(&dir, &json!({"$blob":hash})).is_err());
        assert!(piece(&dir, &json!({"$blob":"../secrets"})).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn committed_postimage_not_requested_text() {
        assert_eq!(crc32c(b"123456789"), 0xe3069283);
        let mut rows = vec![
            json!({"kind":"session_created","id":"s","workspace":"/demo"}),
            json!({"kind":"item","seq":2,"type":"tool_call","data":{"call_id":"c","tool_name":"write_file","arguments_json":"{}"}}),
            json!({"kind":"item","seq":3,"type":"tool_result","data":{"call_id":"c","tool_name":"write_file","status":"success","committed_file_presentation":{"path":"src/main.rs","truncated":false,"lines":[{"kind":"context","text":"not evidence"},{"kind":"addition","text":"fn actual() {}"}]}}}),
        ];
        assert_eq!(
            parse(&rows, Path::new("/missing"), &n::project())
                .unwrap()
                .evs
                .len(),
            1
        );
        rows[2]["data"]["status"] = json!("failure");
        assert!(parse(&rows, Path::new("/missing"), &n::project())
            .unwrap()
            .evs
            .is_empty());
    }
}
