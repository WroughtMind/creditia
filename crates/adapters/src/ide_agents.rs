//! Official IDE storage checked 2026-10-17:
//! CoStrict zgsm-ai/costrict dd38f540, apiMessages.ts, WriteToFileTool.ts,
//! DiffViewProvider.pushToolWriteResult (JSON {path,operation:created|modified}).
//! Amazon Q IDE aws/language-servers 452d26cd, agenticChat/tools/chatDb/{chatDb,util}.ts:
//! ~/.aws/amazonq/history/chat-history-*.json is LokiJS, tabs[].conversations[].messages.
//! Q IDE records fsWrite/fileText and fsReplace/diffs oldStr/newStr. A tab's
//! current modelId is mutable, so it is deliberately not assigned to old calls.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn read(path: &Path) -> Result<Value, String> {
    let t = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&t).map_err(|e| e.to_string())
}
fn abs(cwd: &str, path: &str) -> String {
    let path = crate::file_uri_path(path);
    if Path::new(&path).is_absolute() || path.get(1..2) == Some(":") {
        path
    } else if cwd.is_empty() {
        String::new()
    } else {
        format!("{}/{path}", cwd.trim_end_matches('/'))
    }
}
fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").into()
}

fn costrict_roots() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("CREDITIA_COSTRICT_IDE_DIR") {
        return vec![PathBuf::from(p).join("tasks")];
    }
    if let Ok(p) = std::env::var("CREDITIA_VSCODE_STORAGE") {
        return vec![PathBuf::from(p).join("zgsm-ai.zgsm/tasks")];
    }
    crate::vscode_user_dirs()
        .iter()
        .map(|p| p.join("globalStorage/zgsm-ai.zgsm/tasks"))
        .filter(|p| p.is_dir())
        .collect()
}
fn costrict_tasks() -> Vec<PathBuf> {
    let mut out: Vec<_> = costrict_roots()
        .iter()
        .flat_map(|p| {
            std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("api_conversation_history.json").is_file())
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort();
    out
}
fn result_text(v: &Value) -> String {
    if let Some(t) = v.as_str() {
        t.to_string()
    } else {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Default)]
struct Parsed {
    id: String,
    cwd: String,
    start: i64,
    end: i64,
    prompts: i64,
    edits: Vec<(String, String, String, i64)>,
}
impl Parsed {
    fn time(&mut self, ts: i64) {
        if ts > 0 {
            self.start = if self.start == 0 {
                ts
            } else {
                self.start.min(ts)
            };
            self.end = self.end.max(ts);
        }
    }
    fn group(&self, src: Src, pm: &PathMap) -> FileGroup {
        let mut g = FileGroup::default();
        for (path, text, model, ts) in &self.edits {
            if !path.is_empty() {
                crate::push_text(&mut g.evs, pm, path, text, src, model, &self.id, *ts);
            }
        }
        let models: HashSet<_> = self
            .edits
            .iter()
            .map(|e| e.2.as_str())
            .filter(|m| !m.is_empty())
            .collect();
        g.sessions.push(Session {
            src,
            id: self.id.clone(),
            model: if models.len() == 1 {
                models.into_iter().next().unwrap_or("").into()
            } else {
                String::new()
            },
            start: self.start,
            end: self.end,
            tokens: 0,
            branch: None,
            prompts: self.prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
        g
    }
}

fn parse_costrict(v: &Value, id: &str) -> Parsed {
    let mut p = Parsed {
        id: id.into(),
        ..Default::default()
    };
    let messages: Vec<&Value> = v.as_array().into_iter().flatten().collect();
    let mut ok = HashSet::new();
    let mut created = HashSet::new();
    for m in &messages {
        for b in m["content"].as_array().into_iter().flatten() {
            if b["type"] != "tool_result" || b["is_error"] == true {
                continue;
            }
            let text = result_text(&b["content"]);
            // edit_file may append ` (N replacements)` to the JSON receipt.
            let output: Value = serde_json::Deserializer::from_str(&text)
                .into_iter()
                .next()
                .and_then(Result::ok)
                .unwrap_or_default();
            // The successful native write receipt is structured, never a mere
            // proposal/UI ask. Failed, denied, partial and missing results skip.
            if matches!(output["operation"].as_str(), Some("created" | "modified"))
                && output["path"].is_string()
            {
                if let Some(id) = b["tool_use_id"].as_str().filter(|s| !s.is_empty()) {
                    ok.insert(id.to_string());
                    if output["operation"] == "created" {
                        created.insert(id.to_string());
                    }
                }
            }
        }
        let text = result_text(&m["content"]);
        if p.cwd.is_empty() {
            if let Some(rest) = text.split("Current Working Directory (").nth(1) {
                if let Some(cwd) = rest.split(')').next() {
                    p.cwd = cwd.replace('\\', "/");
                }
            }
        }
    }
    for m in messages {
        let ts = any_ts(&m["ts"])
            .or_else(|| any_ts(&m["timestamp"]))
            .unwrap_or(0);
        p.time(ts);
        if m["role"] == "user"
            && m["content"]
                .as_array()
                .is_some_and(|a| a.iter().any(|b| b["type"] == "text"))
        {
            p.prompts += 1;
        }
        let model = m["model"].as_str().unwrap_or("").to_string();
        for b in m["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "tool_use")
        {
            if !ok.contains(b["id"].as_str().unwrap_or("")) {
                continue;
            }
            let a = &b["input"];
            let name = b["name"].as_str().unwrap_or("");
            let path = crate::path_arg(a).unwrap_or_default();
            let text = match name {
                "write_to_file" if created.contains(b["id"].as_str().unwrap_or("")) => {
                    a["content"].as_str().map(String::from)
                }
                "edit_file"
                    if ["old_string", "oldString", "old_str"]
                        .iter()
                        .any(|k| a[*k].is_string()) =>
                {
                    crate::new_text(name, a)
                }
                "apply_diff" => a["diff"].as_str().map(crate::added_from_diff),
                _ => None,
            };
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                p.edits.push((abs(&p.cwd, &path), text, model.clone(), ts));
            }
        }
    }
    p
}

fn q_dir() -> PathBuf {
    std::env::var("CREDITIA_AMAZONQ_IDE_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".aws/amazonq/history"))
}
fn q_files() -> Vec<PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir(q_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("chat-history-"))
                && p.extension().is_some_and(|e| e == "json")
        })
        .collect();
    out.sort();
    out
}
fn parse_q(v: &Value) -> Vec<Parsed> {
    let mut out = vec![];
    for collection in v["collections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["name"] == "tabs")
    {
        for tab in collection["data"].as_array().into_iter().flatten() {
            for conversation in tab["conversations"].as_array().into_iter().flatten() {
                let mut p = Parsed {
                    id: s(conversation, "conversationId"),
                    ..Default::default()
                };
                let messages: Vec<&Value> = conversation["messages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .collect();
                let mut ok = HashSet::new();
                for m in &messages {
                    for r in m["userInputMessageContext"]["toolResults"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|r| r["status"] == "success")
                    {
                        if let Some(id) = r["toolUseId"].as_str().filter(|s| !s.is_empty()) {
                            ok.insert(id.to_string());
                        }
                    }
                }
                for m in messages {
                    let ts = any_ts(&m["timestamp"]).unwrap_or(0);
                    p.time(ts);
                    if m["type"] == "prompt" && m["body"].as_str().is_some_and(|s| !s.is_empty()) {
                        p.prompts += 1;
                    }
                    for call in m["toolUses"].as_array().into_iter().flatten() {
                        if !ok.contains(call["toolUseId"].as_str().unwrap_or("")) {
                            continue;
                        }
                        let a = &call["input"];
                        let path = s(a, "path");
                        let text = match call["name"].as_str() {
                            Some("fsWrite") if a["command"] == "append" => {
                                a["fileText"].as_str().map(String::from)
                            }
                            Some("fsReplace") => Some(
                                a["diffs"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter(|d| d["oldStr"].is_string())
                                    .map(|d| {
                                        added_lines(
                                            d["oldStr"].as_str().unwrap_or(""),
                                            d["newStr"].as_str().unwrap_or(""),
                                        )
                                    })
                                    .collect::<String>(),
                            ),
                            _ => None,
                        };
                        if let Some(text) = text.filter(|t| !t.is_empty()) {
                            p.edits.push((abs("", &path), text, String::new(), ts));
                        }
                    }
                }
                if !p.id.is_empty() {
                    out.push(p);
                }
            }
        }
    }
    out
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for task in costrict_tasks() {
        let f = task.join("api_conversation_history.json");
        let v = match read(&f) {
            Ok(v) => v,
            Err(e) => {
                st.warnings
                    .push(format!("costrict-ide: cannot read {}: {e}", f.display()));
                continue;
            }
        };
        let p = parse_costrict(&v, &task.file_name().unwrap_or_default().to_string_lossy());
        if !pm.in_project(&p.cwd) && !p.edits.iter().any(|e| pm.to_rel(&e.0).is_some()) {
            continue;
        }
        let mut g = p.group(Src::Costrict, pm);
        g.fp = fingerprint(&f);
        store
            .groups
            .insert(format!("costrict-ide:{}", f.display()), g);
    }
    for f in q_files() {
        let v = match read(&f) {
            Ok(v) => v,
            Err(e) => {
                st.warnings
                    .push(format!("amazon-q-ide: cannot read {}: {e}", f.display()));
                continue;
            }
        };
        for p in parse_q(&v)
            .into_iter()
            .filter(|p| p.edits.iter().any(|e| pm.to_rel(&e.0).is_some()))
        {
            let mut g = p.group(Src::AmazonQ, pm);
            g.fp = fingerprint(&f);
            store
                .groups
                .insert(format!("amazon-q-ide:{}:{}", f.display(), p.id), g);
        }
    }
}

pub fn discover() -> Vec<Activity> {
    let mut out: HashMap<String, Activity> = HashMap::new();
    for task in costrict_tasks() {
        if let Ok(v) = read(&task.join("api_conversation_history.json")) {
            let p = parse_costrict(&v, "");
            if p.cwd.is_empty() {
                continue;
            }
            let e = out.entry(p.cwd.clone()).or_insert(Activity {
                src: Src::Costrict,
                cwd: p.cwd,
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(p.end);
        }
    }
    // Q's native history filenames hash workspace identities; it does not
    // persist plaintext workspace paths. Don't invent a project from a filename.
    out.into_values().collect()
}

pub fn doctor() -> Vec<Health> {
    let tasks = costrict_tasks();
    let q: Vec<_> = q_files().iter().filter_map(|p| read(p).ok()).collect();
    let mut out = vec![Health { source: "costrict-ide".into(), path: costrict_roots().first().map(|p| p.display().to_string()).unwrap_or_default(), readable: tasks.iter().any(|p| read(p).is_ok()), format: "globalStorage/zgsm-ai.zgsm/tasks/*/api_conversation_history.json".into(), records: tasks.len() as u64, note: creditia_core::tr!("只读取原生结构化 created/modified 成功回执。历史未记录的模型保留未知；已通过构造记录测试，尚无本机真实会话核验。", "Native structured created/modified receipts only. Missing historical model stays unknown; fixture validated, no local real-session validation."), ..Default::default() },
        Health { source: "amazon-q-ide".into(), path: q_dir().display().to_string(), readable: !q.is_empty(), format: "LokiJS chat-history-*.json / tabs / conversations / messages".into(), records: q.iter().map(|v| parse_q(v).len() as u64).sum(), note: creditia_core::tr!("fsWrite/fsReplace 要求匹配成功结果与项目内绝对路径。历史没有逐调用模型或明文项目根目录，模型保留未知。已通过构造记录测试，尚无本机真实会话核验。", "fsWrite/fsReplace require matching success results and absolute repository paths. History does not retain per-call model IDs or plaintext workspace roots; model remains unknown. Fixture validated, no local real-session validation."), ..Default::default() }];
    out.push(crate::tabnine_ide::doctor());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn costrict_native_receipt_required_and_context_lines_removed() {
        let v = json!([
            {"role":"user","content":[{"type":"text","text":"Current Working Directory (/repo)"}]},
            {"role":"assistant","model":"actual-model","content":[{"type":"tool_use","id":"ok","name":"edit_file","input":{"file_path":"x.rs","old_string":"keep","new_string":"keep\nnew line"}},{"type":"tool_use","id":"bad","name":"write_to_file","input":{"path":"x.rs","content":"failed line"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"ok","content":"{\"path\":\"x.rs\",\"operation\":\"modified\"}"},{"type":"tool_result","tool_use_id":"bad","is_error":true,"content":"denied"}]}
        ]);
        let p = parse_costrict(&v, "s");
        assert_eq!(
            p.edits,
            vec![(
                "/repo/x.rs".into(),
                "new line\n".into(),
                "actual-model".into(),
                0
            )]
        );
    }
    #[test]
    fn q_ide_never_uses_current_tab_model_for_old_successful_edits() {
        let v = json!({"collections":[{"name":"tabs","data":[{"modelId":"mutable-current-model","conversations":[{"conversationId":"q-ide","messages":[
            {"type":"answer","toolUses":[{"toolUseId":"a","name":"fsWrite","input":{"path":"/repo/x.rs","command":"append","fileText":"written line"}},{"toolUseId":"b","name":"fsReplace","input":{"path":"/repo/x.rs","diffs":[{"oldStr":"written line","newStr":"written line\nadded line"}]}},{"toolUseId":"c","name":"fsWrite","input":{"path":"/repo/x.rs","command":"append","fileText":"denied line"}}]},
            {"type":"prompt","userInputMessageContext":{"toolResults":[{"toolUseId":"a","status":"success"},{"toolUseId":"b","status":"success"},{"toolUseId":"c","status":"error"}]}}
        ]}]}]}]});
        let parsed = parse_q(&v);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].edits.len(), 2);
        assert_eq!(parsed[0].edits[1].1, "added line\n");
        assert!(parsed[0].edits.iter().all(|e| e.2.is_empty()));
    }
}
