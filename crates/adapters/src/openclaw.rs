//! OpenClaw native SQLite transcript_events and legacy session JSONL.
//! Contract: openclaw/openclaw 1197ee98, openclaw-agent-schema.sql,
//! transcript-payload-read.ts and sessions/tools/{edit,file-diff}.ts.
//! Never read meeting transcripts as coding history or attribute ACP-owned sessions.
use crate::{trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    std::env::var_os("CREDITIA_OPENCLAW_DIR")
        .or_else(|| std::env::var_os("OPENCLAW_STATE_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".openclaw"))
}

fn files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let mut all: Vec<_> = walkdir::WalkDir::new(root.join("agents"))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && (e.file_name() == "openclaw-agent.sqlite"
                    || e.path().extension().is_some_and(|e| e == "jsonl"))
        })
        .map(|e| e.into_path())
        .collect();
    // Current DB is authoritative for each agent; archived JSONL must not revive old records.
    let owners: Vec<_> = all
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "openclaw-agent.sqlite"))
        .filter_map(|p| p.parent()?.parent().map(Path::to_path_buf))
        .collect();
    all.retain(|p| {
        p.extension().is_some_and(|x| x != "jsonl") || !owners.iter().any(|o| p.starts_with(o))
    });
    all.sort();
    all
}

fn sqlite(path: &Path) -> Result<Vec<Vec<Value>>, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let columns: HashSet<String> = db
        .prepare("pragma table_info(transcript_events)")
        .and_then(|mut s| s.query_map([], |r| r.get(1))?.collect())
        .map_err(|e| e.to_string())?;
    if !["session_id", "seq", "event_json"]
        .iter()
        .all(|c| columns.contains(*c))
    {
        return Err("unrecognized OpenClaw transcript_events schema".into());
    }
    let windows: HashSet<String> = db
        .prepare("pragma table_info(session_windows)")
        .and_then(|mut s| s.query_map([], |r| r.get(1))?.collect())
        .map_err(|e| e.to_string())?;
    let owned = if windows.contains("acp_owned") {
        " where session_id not in (select session_id from session_windows where acp_owned=1)"
    } else {
        ""
    };
    let compressed = if columns.contains("event_zstd") && columns.contains("event_utf8_bytes") {
        "event_zstd,event_utf8_bytes"
    } else {
        "null,null"
    };
    let sql=format!("select session_id,event_json,{compressed} from transcript_events{owned} order by session_id,seq");
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut sessions: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    while let Some(r) = rows.next().map_err(|e| e.to_string())? {
        let id: String = r.get(0).map_err(|e| e.to_string())?;
        let raw = if let Some(s) = r.get::<_, Option<String>>(1).map_err(|e| e.to_string())? {
            s
        } else {
            let blob: Vec<u8> = r.get(2).map_err(|e| e.to_string())?;
            let len: usize = r.get(3).map_err(|e| e.to_string())?;
            // Upstream schema enforces 4 MiB on both compressed and decoded payloads.
            if len == 0 || len > 4 * 1024 * 1024 || blob.is_empty() || blob.len() > 4 * 1024 * 1024
            {
                return Err("invalid OpenClaw compressed transcript bounds".into());
            }
            let decoder =
                zstd::stream::read::Decoder::new(blob.as_slice()).map_err(|e| e.to_string())?;
            let mut bytes = vec![];
            decoder
                .take(len as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() != len {
                return Err("OpenClaw transcript length mismatch".into());
            }
            String::from_utf8(bytes).map_err(|e| e.to_string())?
        };
        sessions
            .entry(id)
            .or_default()
            .push(serde_json::from_str(&raw).map_err(|e| e.to_string())?);
    }
    Ok(sessions.into_values().collect())
}

fn load(path: &Path) -> Result<Vec<Vec<Value>>, String> {
    if path.extension().is_some_and(|x| x == "sqlite") {
        sqlite(path)
    } else {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut rows = vec![];
        for line in text.lines().filter(|s| !s.trim().is_empty()) {
            rows.push(serde_json::from_str(line).map_err(|e| e.to_string())?);
        }
        Ok(vec![rows])
    }
}

fn parse(rows: &[Value], pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let Some(header) = rows.iter().find(|r| r["type"] == "session") else {
        return g;
    };
    let (Some(id), Some(cwd)) = (header["id"].as_str(), header["cwd"].as_str()) else {
        return g;
    };
    if id.is_empty() || !pm.in_project(cwd) {
        return g;
    }
    let start = crate::any_ts(&header["timestamp"]).unwrap_or(0);
    let superseded: HashSet<&str> = rows
        .iter()
        .filter_map(|r| r["supersedesEntryId"].as_str())
        .collect();
    let mut entries = HashSet::new();
    let mut pending: HashMap<String, (String, i64)> = HashMap::new();
    let mut finished = HashSet::new();
    let mut end = start;
    let mut prompts = 0;
    let mut tokens = 0;
    for row in rows {
        if row["type"] != "message" {
            continue;
        }
        if let Some(eid) = row["id"].as_str() {
            if superseded.contains(eid) || !entries.insert(eid) {
                continue;
            }
        }
        let ts = crate::any_ts(&row["timestamp"]).unwrap_or(0);
        if header["parentSession"].is_string() && (ts == 0 || ts < start) {
            continue;
        }
        end = end.max(ts);
        let m = &row["message"];
        if m["role"] == "user" {
            prompts += 1;
        }
        if m["role"] == "assistant" {
            let model = m["model"].as_str().unwrap_or("");
            tokens += m["usage"]["totalTokens"].as_i64().unwrap_or(0);
            for call in m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["type"] == "toolCall" && c["name"] == "edit")
            {
                if let Some(cid) = call["id"].as_str() {
                    pending.insert(cid.into(), (model.into(), ts));
                }
            }
        }
        if m["role"] != "toolResult" {
            continue;
        }
        let Some(cid) = m["toolCallId"].as_str() else {
            continue;
        };
        let Some((model, call_ts)) = pending.remove(cid) else {
            continue;
        };
        if !finished.insert(cid) || m["isError"] != false || m["details"]["changed"] == false {
            continue;
        }
        // Unified patch is the actual saved diff; proposed old/new arguments are not used.
        for (path, added) in imp::diff_files(m["details"]["patch"].as_str().unwrap_or("")) {
            imp::push_scoped(
                &mut g,
                pm,
                cwd,
                &path,
                &added,
                Src::Openclaw,
                &model,
                id,
                if ts > 0 { ts } else { call_ts },
            );
        }
    }
    g.reconciled_sessions.push((Src::Openclaw, id.into()));
    let mut s = imp::session(
        Src::Openclaw,
        id,
        "",
        start,
        end,
        prompts,
        !g.evs.is_empty(),
    );
    s.tokens = tokens;
    g.sessions.push(s);
    g
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let mut sessions: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut conflicts = HashSet::new();
    for file in files(&root()) {
        match load(&file) {
            Ok(all) => {
                for rows in all {
                    if let Some(id) = rows
                        .iter()
                        .find(|r| r["type"] == "session")
                        .and_then(|r| r["id"].as_str())
                        .map(str::to_string)
                    {
                        if sessions.get(&id).is_some_and(|old| old != &rows) {
                            conflicts.insert(id.clone());
                        }
                        sessions.entry(id).or_insert(rows);
                    }
                }
            }
            Err(e) => {
                st.warnings
                    .push(format!("OpenClaw {}: {e}", file.display()));
            }
        }
    }
    // Missing/unreadable history is recovered from the evidence library, never fabricated.
    store
        .groups
        .retain(|k, _| !k.starts_with("openclaw:native:"));
    for (id, rows) in sessions {
        let mut g = parse(&rows, pm);
        if conflicts.contains(&id) {
            g.evs.clear();
            st.warnings
                .push(format!("OpenClaw conflicting copies for session {id}"));
        }
        store.groups.insert(format!("openclaw:native:{id}"), g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut ids = HashSet::new();
    let mut out = vec![];
    for file in files(&root()) {
        for rows in load(&file).unwrap_or_default() {
            if let Some(h) = rows.iter().find(|r| r["type"] == "session") {
                if let (Some(id), Some(cwd)) = (h["id"].as_str(), h["cwd"].as_str()) {
                    if ids.insert(id.to_string()) {
                        out.push(imp::activity(
                            Src::Openclaw,
                            cwd.into(),
                            None,
                            crate::any_ts(&h["timestamp"]).unwrap_or(0),
                        ));
                    }
                }
            }
        }
    }
    out
}
pub fn doctor() -> Health {
    let root = root();
    let mut count = 0;
    let mut errors = vec![];
    for file in files(&root) {
        match load(&file) {
            Ok(s) => {
                count += s
                    .iter()
                    .filter(|rows| rows.iter().any(|r| r["type"] == "session"))
                    .count()
            }
            Err(e) => errors.push(e),
        }
    }
    let note = if errors.is_empty() {
        "Successful edit receipts with actual unified patches. Legacy records without patch have session metadata only. ACP-owned sessions excluded. Contract-tested, account runtime validation pending.".into()
    } else {
        errors.join("; ")
    };
    imp::health(
        Src::Openclaw,
        &root,
        "native transcript_events / legacy session JSONL",
        count,
        &note,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn rows() -> Vec<Value> {
        vec![
            json!({"type":"session","id":"s","cwd":"/repo","timestamp":1000}),
            json!({"type":"message","id":"m1","timestamp":1001,"message":{"role":"assistant","model":"actual-model","content":[{"type":"toolCall","id":"c","name":"edit","arguments":{"path":"wrong.rs","newText":"wrong"}}]}}),
            json!({"type":"message","id":"m2","timestamp":1002,"message":{"role":"toolResult","toolCallId":"c","isError":false,"details":{"changed":true,"patch":"--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-old_line(42);\n+actual_line(73);\n"}}}),
        ]
    }
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into(),
            markers: vec!["/repo/".into()],
            excludes: vec![],
        }
    }
    #[test]
    fn actual_patch_and_model_are_used_and_failure_retracts() {
        let mut r = rows();
        let g = parse(&r, &pm());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "a.rs");
        assert_eq!(g.evs[0].model, "actual-model");
        r[2]["message"]["isError"] = json!(true);
        let g = parse(&r, &pm());
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions.len(), 1);
    }
    #[test]
    fn copied_and_forked_messages_do_not_gain_credit() {
        let mut r = rows();
        r.extend(r[1..].to_vec());
        assert_eq!(parse(&r, &pm()).evs.len(), 1);
        r[0]["parentSession"] = json!("parent");
        r[0]["timestamp"] = json!(2000);
        assert!(parse(&r, &pm()).evs.is_empty());
    }
    #[test]
    fn sqlite_compression_roundtrip_is_read_only() {
        let path =
            std::env::temp_dir().join(format!("creditia-openclaw-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("create table transcript_events(session_id text,seq integer,event_json text,event_zstd blob,event_utf8_bytes integer);create table session_windows(session_id text,acp_owned integer);insert into session_windows values('s',0);").unwrap();
        for (i, v) in rows().iter().enumerate() {
            let s = v.to_string();
            let compressed = zstd::encode_all(s.as_bytes(), 1).unwrap();
            db.execute(
                "insert into transcript_events values('s',?1,null,?2,?3)",
                rusqlite::params![i, compressed, s.len()],
            )
            .unwrap();
        }
        drop(db);
        let before = std::fs::read(&path).unwrap();
        let all = sqlite(&path).unwrap();
        assert_eq!(parse(&all[0], &pm()).evs.len(), 1);
        assert_eq!(before, std::fs::read(&path).unwrap());
        assert!(!path.with_extension("sqlite-wal").exists());
        std::fs::remove_file(path).unwrap();
    }
}
