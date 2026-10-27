//! Reasonix flat transcripts, append/replace event logs, and schema-2 message DAGs.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
fn default_root() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::home().join("AppData/Roaming"))
            .join("reasonix")
    } else {
        crate::home().join(".reasonix")
    }
}
fn root() -> PathBuf {
    std::env::var_os("CREDITIA_REASONIX_DIR")
        .or_else(|| std::env::var_os("REASONIX_STATE_HOME"))
        .or_else(|| std::env::var_os("REASONIX_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(default_root)
}
fn paths() -> Vec<PathBuf> {
    let mut out = n::files(&root(), |p| {
        p.extension().and_then(|s| s.to_str()) == Some("jsonl")
            && p.parent()
                .is_some_and(|p| p.components().any(|c| c.as_os_str() == "sessions"))
    });
    out.retain(|p| {
        let name = p.file_name().unwrap().to_string_lossy();
        ![".wire.jsonl", ".adjudication.jsonl", ".display.jsonl"]
            .iter()
            .any(|s| name.ends_with(s))
    });
    for p in &mut out {
        if let Some(base) = p.to_str().and_then(|s| s.strip_suffix(".events.jsonl")) {
            *p = PathBuf::from(format!("{base}.jsonl"));
        }
    }
    out.sort();
    out.dedup();
    out
}
fn meta(path: &Path) -> Value {
    n::json(&PathBuf::from(format!("{}.meta", path.display())))
        .or_else(|_| n::json(&path.with_extension("meta.json")))
        .unwrap_or(Value::Null)
}
fn workspace(v: &Value) -> &str {
    v["workspace_root"]
        .as_str()
        .or_else(|| v["workspace"].as_str())
        .unwrap_or("")
}
#[derive(Default)]
struct Head {
    leaf: String,
    order: usize,
    at: (i64, u32),
    retired: bool,
}
fn order_time(v: &Value) -> (i64, u32) {
    // Go writes RFC3339Nano. Keep fractions when two heads change in one second.
    let fraction = v
        .as_str()
        .and_then(|s| s.split_once('.'))
        .map(|(_, tail)| {
            tail.chars()
                .take_while(char::is_ascii_digit)
                .take(9)
                .collect::<String>()
        })
        .unwrap_or_default();
    let nanos = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<9}").parse().unwrap_or(0)
    };
    (crate::any_ts(v).unwrap_or(0), nanos)
}
fn replay(rows: &[Value]) -> Result<Vec<Value>, String> {
    if rows.iter().all(|r| r["role"].is_string()) {
        return Ok(rows.to_vec());
    }
    if rows.first().and_then(|r| r["schema_version"].as_u64()) == Some(2) {
        let mut nodes: HashMap<String, (String, Value)> = HashMap::new();
        let mut heads: HashMap<String, Head> = HashMap::new();
        let mut selected = String::new();
        let mut patches = HashMap::new();
        let mut redactions = HashMap::new();
        for (i, r) in rows.iter().enumerate() {
            if r["schema_version"] != 2 {
                return Err("unsupported mixed Reasonix DAG version".into());
            }
            let h = r["head"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("main");
            match n::text(&r["type"]) {
                "message" => {
                    let id = n::text(&r["id"]);
                    let msgs = r["msgs"].as_array().ok_or("DAG message missing msgs")?;
                    if id.is_empty() || msgs.len() != 1 {
                        return Err("invalid DAG message".into());
                    }
                    if nodes.contains_key(id) {
                        continue;
                    }
                    nodes.insert(id.into(), (n::text(&r["parent"]).into(), msgs[0].clone()));
                    let head = heads.entry(h.into()).or_default();
                    head.leaf = id.into();
                    head.order = i;
                    head.at = order_time(&r["at"]);
                }
                "patch" => {
                    if !nodes.contains_key(n::text(&r["target"])) {
                        continue;
                    }
                    if let Some(m) = r["msgs"].as_array().filter(|a| a.len() == 1) {
                        patches.insert(n::text(&r["target"]).to_string(), m[0].clone());
                    } else {
                        return Err("invalid Reasonix patch payload".into());
                    }
                }
                "redact" => {
                    for (id, msgs) in r["targets"].as_object().into_iter().flatten() {
                        if let Some(m) = msgs.as_array().filter(|a| a.len() == 1) {
                            redactions.insert(id.clone(), m[0].clone());
                        } else {
                            return Err("invalid Reasonix redaction payload".into());
                        }
                    }
                }
                "fork" => {
                    heads.entry(n::text(&r["new_head"]).into()).or_insert(Head {
                        leaf: n::text(&r["from"]).into(),
                        order: i,
                        at: order_time(&r["at"]),
                        retired: false,
                    });
                }
                "rewind" => {
                    let head = heads.entry(h.into()).or_default();
                    head.leaf = n::text(&r["to"]).into();
                    head.order = i;
                    head.at = order_time(&r["at"]);
                }
                "select" => selected = h.into(),
                "retire" => heads.entry(h.into()).or_default().retired = true,
                "system" | "log" | "writer" | "rename" | "turn_begin" | "turn_end"
                | "compaction" | "checkpoint" => {}
                other => return Err(format!("unsupported Reasonix DAG event: {other}")),
            }
        }
        let head = heads
            .get(&selected)
            .filter(|h| !h.retired)
            .or_else(|| {
                heads
                    .values()
                    .filter(|h| !h.retired)
                    .max_by_key(|h| (h.at, h.order))
            })
            .or_else(|| heads.values().max_by_key(|h| (h.at, h.order)));
        let mut id = head.map(|h| h.leaf.as_str()).unwrap_or("");
        let mut seen = HashSet::new();
        let mut out = vec![];
        while !id.is_empty() && seen.insert(id) {
            let Some((parent, msg)) = nodes.get(id) else {
                break;
            };
            out.push(
                redactions
                    .get(id)
                    .or_else(|| patches.get(id))
                    .unwrap_or(msg)
                    .clone(),
            );
            id = parent;
        }
        out.reverse();
        return Ok(out);
    }
    let mut out = vec![];
    for r in rows {
        if !matches!(r["schema_version"].as_u64(), Some(1 | 3 | 4 | 5)) {
            return Err("unsupported Reasonix event version".into());
        }
        let msgs = r["messages"].as_array().cloned().unwrap_or_default();
        match n::text(&r["type"]) {
            "replace" => out = msgs,
            "append" => {
                if r["message_index"].as_u64().unwrap_or(0) != out.len() as u64 {
                    return Err("Reasonix append base does not match".into());
                }
                out.extend(msgs);
            }
            _ => return Err("unknown Reasonix event".into()),
        }
    }
    Ok(out)
}
fn parse(rows: &[Value], metadata: &Value, id: &str, pm: &PathMap) -> FileGroup {
    let mut g = FileGroup::default();
    let mut calls = HashMap::new();
    let mut seen = HashSet::new();
    let cwd = workspace(metadata);
    let start = crate::any_ts(&metadata["created_at"]).unwrap_or(0);
    let mut end = start;
    let mut prompts = 0;
    for r in rows {
        if !r["agent_composition"].is_null() {
            continue;
        }
        let ts = crate::any_ts(&r["createdAt"]).unwrap_or(start);
        end = end.max(ts);
        // External forks duplicate their parent's prefix; only the child's new messages belong here.
        if metadata["parent_id"].is_string() && (ts < start || r["createdAt"].is_null()) {
            continue;
        }
        if r["role"] == "user" && r["host_authored"] != true {
            prompts += 1;
        }
        if r["role"] == "assistant" && r["local_only"] != true {
            for c in r["tool_calls"].as_array().into_iter().flatten() {
                let cid = n::text(&c["id"]);
                if !cid.is_empty() {
                    calls.insert(cid, (c, n::text(&r["modelRef"])));
                }
            }
        }
        if r["role"] != "tool" {
            continue;
        }
        let cid = n::text(&r["tool_call_id"]);
        let Some((call, model)) = calls.remove(cid) else {
            continue;
        };
        if !seen.insert(cid) || !r["tool_failure"].is_null() {
            continue;
        }
        let args: Value = serde_json::from_str(n::text(&call["arguments"])).unwrap_or(Value::Null);
        let receipt = r["raw_content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(n::text(&r["content"]));
        let path = n::text(&args["path"]);
        let resolved = if Path::new(path).is_absolute() {
            path.to_string()
        } else {
            Path::new(cwd).join(path).to_string_lossy().into_owned()
        };
        let added = match n::text(&call["name"]) {
            "write_file"
                if receipt
                    == format!(
                        "wrote {} bytes to {resolved}",
                        n::text(&args["content"]).len()
                    ) =>
            {
                n::text(&args["content"]).into()
            }
            "edit_file" | "multi_edit" if receipt.starts_with(&format!("edited {resolved}")) => {
                let Some((_, actual)) =
                    receipt.split_once("\nActual replacement receipt after write:\n")
                else {
                    continue;
                };
                if actual.contains("truncated") || actual.contains("omitted]") {
                    continue;
                }
                actual
                    .lines()
                    .filter_map(|l| l.strip_prefix('+'))
                    .filter(|l| *l != "<empty>")
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => continue,
        };
        imp::push_scoped(&mut g, pm, cwd, path, &added, Src::Reasonix, model, id, ts);
    }
    n::session(&mut g, pm, Src::Reasonix, id, cwd, start, end, prompts, 0);
    g
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let events = path.with_extension("events.jsonl");
    let source = if events.is_file() {
        events.as_path()
    } else {
        path
    };
    let rows = replay(&n::jsonl(source)?)?;
    let metadata = meta(path);
    if metadata["superseded"] == true {
        return Ok(FileGroup {
            reconciled_sessions: vec![(
                Src::Reasonix,
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .into(),
            )],
            ..Default::default()
        });
    }
    Ok(parse(
        &rows,
        &metadata,
        path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
        pm,
    ))
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Reasonix, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    paths()
        .into_iter()
        .filter_map(|p| {
            let m = meta(&p);
            let cwd = workspace(&m);
            (!cwd.is_empty()).then(|| {
                imp::activity(
                    Src::Reasonix,
                    cwd.into(),
                    None,
                    crate::any_ts(&m["updated_at"]).unwrap_or(0),
                )
            })
        })
        .collect()
}
pub fn doctor() -> Health {
    imp::health(Src::Reasonix,&root(),"native flat / event log / schema-2 DAG",paths().len(),"Paired successful write_file and complete actual edit receipts; per-message modelRef. Unknown models stay unknown. Superseded snapshots are excluded.",false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actual_receipt_and_failure() {
        let mut rows = vec![
            json!({"role":"assistant","modelRef":"m","tool_calls":[{"id":"c","name":"edit_file","arguments":"{\"path\":\"src/main.rs\",\"new_string\":\"proposal\"}"}]}),
            json!({"role":"tool","tool_call_id":"c","content":"edited /demo/src/main.rs\nActual replacement receipt after write:\n@@ replacement 1 of 1 (1 occurrence(s)) @@\n-old\n+fn actual() {}\n"}),
        ];
        let m = json!({"workspace":"/demo"});
        assert_eq!(parse(&rows, &m, "s", &n::project()).evs.len(), 1);
        rows[1]["tool_failure"] = json!({"reason":"error"});
        assert!(parse(&rows, &m, "s", &n::project()).evs.is_empty());
    }
    #[test]
    fn dag_rewind_redaction_and_unknown_version() {
        let rows = vec![
            json!({"schema_version":2,"type":"message","id":"a","msgs":[{"role":"assistant"}]}),
            json!({"schema_version":2,"type":"message","id":"b","parent":"a","msgs":[{"role":"tool"}]}),
            json!({"schema_version":2,"type":"redact","targets":{"a":[{"role":"user"}]}}),
            json!({"schema_version":2,"type":"rewind","to":"a"}),
        ];
        assert_eq!(replay(&rows).unwrap(), vec![json!({"role":"user"})]);
        assert!(replay(&[json!({"schema_version":99})]).is_err());
        let rows = vec![
            json!({"schema_version":2,"type":"message","id":"a","head":"newer","at":"2026-10-23T01:00:00.100Z","msgs":[{"role":"user","content":"newer"}]}),
            json!({"schema_version":2,"type":"message","id":"b","head":"older","at":"2026-10-23T01:00:00.050Z","msgs":[{"role":"user","content":"older"}]}),
        ];
        assert_eq!(replay(&rows).unwrap()[0]["content"], "newer");
    }
}
