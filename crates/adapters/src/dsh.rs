//! DSH native v0-v4 Session JSONL. Verified against deepseek-ai/deepseek-harness local
//! checkout (2026-10-17): session-format-v{0-to-1,1-to-2,3-to-4}/src/codec.ts,
//! v3-to-v4/src/tool-role.ts, session-persistence-jsonl, fs/tool-fs and home-paths.
//! Stream chunks are not edit evidence: only assistant tool calls with matched, explicitly
//! successful tool/result messages are accepted. Seeded inherited history is skipped.
use crate::trace_import as imp;
use crate::{any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var("CREDITIA_DSH_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
    {
        return if p.join("sessions").is_dir() {
            p.join("sessions")
        } else {
            p
        };
    }
    let root = std::env::var("DSH_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            if let Some(p) = s.strip_prefix("~/").or(s.strip_prefix("~\\")) {
                crate::home().join(p)
            } else {
                PathBuf::from(s)
            }
        })
        .unwrap_or_else(|| crate::home().join(".dsh"));
    root.join("sessions")
}
fn generation(p: &Path) -> Option<u64> {
    let n = p.file_name()?.to_str()?;
    let n = n
        .strip_suffix(".zstd")
        .or(n.strip_suffix(".zst"))
        .unwrap_or(n);
    if n == "session.jsonl" {
        return Some(0);
    }
    let s = n.strip_prefix("session.v")?.strip_suffix(".jsonl")?;
    if s.starts_with('0') {
        return None;
    }
    s.parse().ok()
}
fn selected(root: &Path) -> Vec<(PathBuf, u64)> {
    let entries = if root.is_file() {
        vec![root.to_path_buf()]
    } else {
        walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect()
    };
    let mut dirs: HashMap<PathBuf, (PathBuf, u64)> = HashMap::new();
    for p in entries {
        let Some(v) = generation(&p) else { continue };
        let dir = p.parent().unwrap_or(root).to_path_buf();
        let entry = dirs.entry(dir).or_insert((p.clone(), v));
        if v > entry.1 || v == entry.1 && p > entry.0 {
            *entry = (p, v);
        }
    }
    dirs.into_values().collect()
}
fn read(p: &Path) -> Result<Vec<Value>, String> {
    let raw = std::fs::read(p).map_err(|e| e.to_string())?;
    let bytes = if matches!(p.extension().and_then(|s| s.to_str()), Some("zst" | "zstd")) {
        zstd::stream::decode_all(&raw[..]).map_err(|e| e.to_string())?
    } else {
        raw
    };
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    text.lines()
        .filter(|s| !s.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect()
}
fn ts(v: &Value) -> i64 {
    any_ts(v).unwrap_or(0)
}
fn parse(rows: &[Value], version: u64, pm: &PathMap, g: &mut FileGroup) -> Result<(), String> {
    let h = rows.first().ok_or("empty session")?;
    if version > 4 || h["type"] != "session" || h["version"].as_u64() != Some(version) {
        return Err("unsupported or mismatched Session generation".into());
    }
    let id = h["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("missing Session id")?;
    let initial = h["cwd"].as_str().unwrap_or("");
    if !pm.in_project(initial) {
        return Ok(());
    }
    let cut = if version < 2 {
        h["seedLength"].as_u64().unwrap_or(0)
    } else if h["isSeeded"] == true {
        rows.iter()
            .filter(|e| e["type"] == "session/end-seed" && e["data"]["inherited"] == true)
            .filter_map(|e| e["seq"].as_u64())
            .max()
            .ok_or("seeded Session lacks inherited boundary")?
    } else {
        0
    };
    let mut cwd = initial.to_string();
    let mut pending: HashMap<String, (String, Value, String, String, String)> = HashMap::new();
    let mut model = String::new();
    let mut provider = None;
    let mut end = ts(&h["createdAt"]);
    let mut prompts = 0;
    let mut tokens = 0;
    for e in rows.iter().skip(1) {
        if e["seq"].as_u64().is_some_and(|s| s < cut) {
            continue;
        }
        let time = ts(&e["time"]);
        end = end.max(time);
        let d = &e["data"];
        let m = &d["message"];
        match e["type"].as_str() {
            Some("working-directory/change") => {
                if let Some(p) = d["cwd"].as_str() {
                    cwd = p.into();
                }
            }
            Some("user/message") if m["source"]["kind"] == "user" => prompts += 1,
            Some("assistant/message") => {
                if m["source"]["kind"] != "model" {
                    continue;
                }
                let current = m["source"]["model"].as_str().unwrap_or("");
                let vendor = m["source"]["provider"].as_str().unwrap_or("");
                if !current.is_empty() {
                    model = current.into();
                }
                if !vendor.is_empty() {
                    provider = Some(vendor.into());
                }
                let u = &d["usage"];
                tokens += u["totalTokens"].as_i64().unwrap_or_else(|| {
                    [
                        "inputTokens",
                        "outputTokens",
                        "cacheReadTokens",
                        "cacheWriteTokens",
                    ]
                    .iter()
                    .filter_map(|k| u[k].as_i64())
                    .sum()
                });
                for b in m["content"].as_array().into_iter().flatten() {
                    if b["type"] != "tool-call" {
                        continue;
                    }
                    if let (Some(call), Some(name), Some(args)) = (
                        b["id"].as_str(),
                        b["name"].as_str(),
                        b["arguments"].as_str(),
                    ) {
                        if let Ok(a) = serde_json::from_str(args) {
                            pending.insert(
                                call.into(),
                                (name.into(), a, current.into(), vendor.into(), cwd.clone()),
                            );
                        }
                    }
                }
            }
            Some("tool/result") => {
                if d.get("error").is_some() || m["source"]["kind"] != "tool" {
                    continue;
                }
                let Some(call) = m["source"]["callId"].as_str() else {
                    continue;
                };
                let success = if version == 4 {
                    m["role"] == "tool" && m["toolCallId"] == call && m["isError"] == false
                } else {
                    m["role"] == "user"
                        && m["content"].as_array().is_some_and(|a| {
                            a.len() == 1
                                && a[0]["type"] == "tool-result"
                                && a[0]["toolCallId"] == call
                                && a[0]["isError"] == false
                        })
                };
                let Some((name, a, model, _vendor, call_cwd)) = pending.remove(call) else {
                    continue;
                };
                if !success {
                    continue;
                }
                let path = d["meta"]["path"]
                    .as_str()
                    .or(a["file_path"].as_str())
                    .or(a["path"].as_str())
                    .unwrap_or("");
                if let Some(diffs) = d["meta"]["diffs"].as_array().filter(|a| !a.is_empty()) {
                    for diff in diffs {
                        if let (Some(old), Some(new)) =
                            (diff["oldText"].as_str(), diff["newText"].as_str())
                        {
                            let path = diff["path"].as_str().unwrap_or(path);
                            imp::push_scoped(
                                g,
                                pm,
                                &call_cwd,
                                path,
                                &crate::added_lines(old, new),
                                Src::Dsh,
                                &model,
                                id,
                                time,
                            );
                        }
                    }
                    continue;
                }
                let text = match name.as_str() {
                    "write" if d["meta"]["operation"] == "create" => {
                        a["content"].as_str().map(String::from)
                    }
                    "edit" => a["old_string"]
                        .as_str()
                        .zip(a["new_string"].as_str())
                        .map(|(old, new)| crate::added_lines(old, new)),
                    "str_replace_editor" => match a["command"].as_str() {
                        Some("create") => a["file_text"].as_str().map(String::from),
                        Some("str_replace") => a["old_str"].as_str().map(|old| {
                            crate::added_lines(old, a["new_str"].as_str().unwrap_or(""))
                        }),
                        Some("insert") => a["new_str"].as_str().map(String::from),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(text) = text {
                    imp::push_scoped(g, pm, &call_cwd, path, &text, Src::Dsh, &model, id, time);
                }
            }
            _ => {}
        }
    }
    let mut s = imp::session(
        Src::Dsh,
        id,
        &model,
        ts(&h["createdAt"]),
        end,
        prompts,
        !g.evs.is_empty(),
    );
    s.tokens = tokens;
    s.provider = provider;
    g.sessions.push(s);
    Ok(())
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let root = data_dir();
    let files = selected(&root);
    let prefix = "dsh:native:";
    let keys: std::collections::HashSet<String> = files
        .iter()
        .map(|(p, _)| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(prefix) || keys.contains(k));
    for (p, v) in files {
        let key = format!("{prefix}{}", p.display());
        let fp = fingerprint(&p);
        if store.groups.get(&key).is_some_and(|g| g.fp == fp) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        let result = read(&p).and_then(|r| parse(&r, v, pm, &mut g));
        if let Err(e) = result {
            st.warnings.push(format!("dsh: {}: {e}", p.display()));
            g.evs.clear();
            g.sessions.clear();
        }
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    selected(&data_dir())
        .into_iter()
        .filter_map(|(p, v)| {
            let r = read(&p).ok()?;
            let h = r.first()?;
            if v > 4 || h["type"] != "session" || h["version"].as_u64() != Some(v) {
                return None;
            }
            let cwd = h["cwd"].as_str()?.into();
            Some(imp::activity(
                Src::Dsh,
                cwd,
                None,
                r.iter().map(|e| ts(&e["time"])).max().unwrap_or(0),
            ))
        })
        .collect()
}
pub fn doctor() -> Vec<Health> {
    let root = data_dir();
    let files = selected(&root);
    let valid = files
        .iter()
        .filter(|(p, v)| {
            *v <= 4
                && read(p)
                    .ok()
                    .and_then(|r| r.first().cloned())
                    .is_some_and(|h| h["type"] == "session" && h["version"].as_u64() == Some(*v))
        })
        .count();
    vec![imp::health(Src::Dsh,&root,"DSH Session v0-v4 JSONL / JSONL.zstd",valid,"Reads native assistant tool calls paired by callId with successful tool/result messages. Supports v0-v3 result wrappers and v4 tool-role messages, per-call model/provider, usage, canonical highest generation and seeded-history exclusion. Stream chunks and unconfirmed shell/plugin edits are not counted. Default is $DSH_HOME/sessions (normally ~/.dsh/sessions); custom storage roots can be selected with CREDITIA_DSH_DIR. Future generations are refused rather than falling back to stale history.",false)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    fn rows(v: u64) -> Vec<Value> {
        let mut result = json!({"role":"tool","source":{"kind":"tool","callId":"c"},"toolCallId":"c","isError":false,"content":[{"type":"text","text":"Updated file"}]});
        if v < 4 {
            result = json!({"role":"user","source":{"kind":"tool","callId":"c"},"content":[{"type":"tool-result","toolCallId":"c","isError":false,"content":[]}]});
        }
        vec![
            json!({"type":"session","version":v,"id":"s","cwd":"/demo","createdAt":1780000000000i64,"isSeeded":false}),
            json!({"type":"assistant/message","seq":0,"time":1780000001000i64,"data":{"message":{"role":"assistant","source":{"kind":"model","model":"deepseek-v4-flash","provider":"deepseek-official"},"content":[{"type":"tool-call","id":"c","name":"edit","arguments":"{\"file_path\":\"a.rs\",\"old_string\":\"keep_line\",\"new_string\":\"keep_line\\naccepted_line\"}"}]},"usage":{"totalTokens":23}}}),
            json!({"type":"tool/result","seq":1,"time":1780000002000i64,"data":{"message":result,"meta":{"path":"/demo/a.rs"}}}),
        ]
    }
    #[test]
    fn all_released_generations_pair_real_results() {
        for v in 0..=4 {
            let mut g = FileGroup::default();
            parse(&rows(v), v, &pm(), &mut g).unwrap();
            assert_eq!(g.evs.len(), 1);
            assert_eq!(g.evs[0].model, "deepseek-v4-flash");
            assert_eq!(g.sessions[0].tokens, 23);
            assert_eq!(g.sessions[0].provider.as_deref(), Some("deepseek-official"));
        }
    }
    #[test]
    fn failed_unmatched_and_inherited_edits_do_not_count() {
        let mut r = rows(4);
        r[2]["data"]["message"]["isError"] = json!(true);
        let mut g = FileGroup::default();
        parse(&r, 4, &pm(), &mut g).unwrap();
        assert!(g.evs.is_empty());
        r = rows(4);
        r[0]["isSeeded"] = json!(true);
        r.push(json!({"type":"session/end-seed","seq":2,"data":{"inherited":true}}));
        let mut g = FileGroup::default();
        parse(&r, 4, &pm(), &mut g).unwrap();
        assert!(g.evs.is_empty());
        assert_eq!(g.sessions[0].tokens, 0);
    }
    #[test]
    fn full_file_write_uses_observed_diff_and_never_claims_existing_context() {
        let mut r = rows(4);
        r[1]["data"]["message"]["content"][0]["name"] = json!("write");
        r[1]["data"]["message"]["content"][0]["arguments"] = json!(
            json!({"file_path":"a.rs","content":"existing_human_line\nadded_line"}).to_string()
        );
        r[2]["data"]["meta"] = json!({"path":"/demo/a.rs","operation":"update","diffs":[{"path":"/demo/a.rs","oldText":"existing_human_line","newText":"existing_human_line\nadded_line"}]});
        let mut g = FileGroup::default();
        parse(&r, 4, &pm(), &mut g).unwrap();
        assert_eq!(g.evs.len(), 1);
        r[2]["data"]["meta"] = json!({"path":"/demo/a.rs","operation":"update","diffs":[]});
        let mut g = FileGroup::default();
        parse(&r, 4, &pm(), &mut g).unwrap();
        assert!(g.evs.is_empty());
        r[2]["data"]["meta"]["operation"] = json!("create");
        let mut g = FileGroup::default();
        parse(&r, 4, &pm(), &mut g).unwrap();
        assert_eq!(g.evs.len(), 2);
    }
    #[test]
    fn canonical_generation_and_zstd() {
        assert_eq!(generation(Path::new("session.v4.jsonl.zstd")), Some(4));
        assert_eq!(generation(Path::new("session.v04.jsonl")), None);
        assert_eq!(generation(Path::new("session.v4.jsonl.tmp")), None);
        let root = std::env::temp_dir().join(format!("creditia-dsh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let p = root.join("session.v4.jsonl.zstd");
        let text = rows(4)
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&p, zstd::stream::encode_all(text.as_bytes(), 1).unwrap()).unwrap();
        std::fs::write(root.join("session.v3.jsonl"), "{}").unwrap();
        assert_eq!(selected(&root), vec![(p.clone(), 4)]);
        assert_eq!(read(&p).unwrap().len(), 3);
        std::fs::write(root.join("session.v5.jsonl"), "{}").unwrap();
        assert_eq!(selected(&root)[0].1, 5);
        std::fs::remove_dir_all(root).unwrap();
    }
}
