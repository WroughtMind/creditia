//! Junie CLI native events, verified from the official release 888.12 JAR.
//! https://github.com/JetBrains/junie/releases/tag/888.12
//! Read-only javap inspection, no Junie execution: SessionStore, SessionEvent,
//! SessionA2uxEvent, A2uxEvent, FileChangesBlockUpdatedEvent, FileChange,
//! TextFileContent, StepStatus, LlmResponseMetadataEvent, ModelUsage serializers.
//! The current A2UX codec uses `kind` discriminators (not legacy `method`).
//! Completed file-change blocks hold actual before/after contents. Approval,
//! in-progress, rejected/canceled/failed blocks and generic patch text do not count.

use crate::{added_lines, any_ts, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    std::env::var("CREDITIA_JUNIE_DIR")
        .ok()
        .or_else(|| std::env::var("JUNIE_HOME").ok())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".junie"))
}
fn files() -> Vec<PathBuf> {
    let mut out: Vec<_> = walkdir::WalkDir::new(home().join("sessions"))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name() == "events.jsonl")
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}
fn index() -> HashMap<String, Value> {
    std::fs::read_to_string(home().join("sessions/index.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| Some((v["sessionId"].as_str()?.to_string(), v)))
        .collect()
}
#[derive(Default)]
struct Parsed {
    cwd: String,
    start: i64,
    end: i64,
    prompts: i64,
    tokens: i64,
    models: HashSet<String>,
    edits: Vec<(String, String, String)>,
}
fn abs(cwd: &str, path: &str) -> String {
    let p = crate::file_uri_path(path);
    if Path::new(&p).is_absolute() || p.get(1..2) == Some(":") {
        p
    } else if cwd.is_empty() {
        String::new()
    } else {
        format!("{}/{p}", cwd.trim_end_matches('/'))
    }
}
fn parse(text: &str, summary: &Value) -> Parsed {
    let mut p = Parsed {
        cwd: summary["projectDir"].as_str().unwrap_or("").into(),
        start: any_ts(&summary["createdAt"]).unwrap_or(0),
        end: any_ts(&summary["updatedAt"]).unwrap_or(0),
        ..Default::default()
    };
    let mut generation_model = String::new();
    let mut seen = HashSet::new();
    for r in text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        if r["kind"] == "UserPromptEvent" {
            p.prompts += 1;
            generation_model.clear();
            continue;
        }
        if r["kind"] != "SessionA2uxEvent" {
            continue;
        }
        let e = &r["event"]["agentEvent"];
        match e["kind"].as_str() {
            Some("CurrentDirectoryUpdatedEvent") => {
                if let Some(cwd) = e["currentDirectory"].as_str() {
                    p.cwd = cwd.into();
                }
            }
            Some("LlmResponseMetadataEvent") => {
                let mut models = HashSet::new();
                for usage in e["modelUsage"].as_array().into_iter().flatten() {
                    if let Some(model) = usage["model"].as_str().filter(|s| !s.is_empty()) {
                        models.insert(model.to_string());
                        p.models.insert(model.to_string());
                    }
                    p.tokens += [
                        "inputTokens",
                        "cacheInputTokens",
                        "cacheCreateTokens",
                        "outputTokens",
                    ]
                    .iter()
                    .map(|k| usage[*k].as_i64().unwrap_or(0))
                    .sum::<i64>();
                }
                generation_model = if models.len() == 1 {
                    models.into_iter().next().unwrap_or_default()
                } else {
                    String::new()
                };
            }
            Some("FileChangesBlockUpdatedEvent") if e["status"] == "COMPLETED" => {
                for c in e["changes"].as_array().into_iter().flatten() {
                    if c["afterContent"]["kind"] != "TextFileContent" {
                        continue;
                    }
                    let Some(after) = c["afterContent"]["text"].as_str() else {
                        continue;
                    };
                    // A non-text previous body cannot safely be interpreted as
                    // an empty text file. A null previous body means creation.
                    let Some(previous) = c.get("beforeContent") else { continue; };
                    let before = if previous.is_null() {
                        ""
                    } else if c["beforeContent"]["kind"] == "TextFileContent" {
                        let Some(t) = c["beforeContent"]["text"].as_str() else {
                            continue;
                        };
                        t
                    } else {
                        continue;
                    };
                    let Some(path) = c["afterRelativePath"].as_str().filter(|s| !s.is_empty())
                    else {
                        continue;
                    };
                    let path = abs(&p.cwd, path);
                    if path.is_empty()
                        || !seen.insert((
                            e["stepId"].as_str().unwrap_or("").to_string(),
                            path.clone(),
                            before.to_string(),
                            after.to_string(),
                        ))
                    {
                        continue;
                    }
                    let added = added_lines(before, after);
                    if !added.is_empty() {
                        p.edits.push((path, added, generation_model.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    p
}
fn load(f: &Path, summaries: &HashMap<String, Value>) -> Result<(String, Parsed), String> {
    let id = f
        .parent()
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let text = std::fs::read_to_string(f).map_err(|e| e.to_string())?;
    let summary = summaries.get(&id).unwrap_or(&Value::Null);
    Ok((id, parse(&text, summary)))
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let summaries = index();
    for f in files() {
        let key = format!("junie-cli:{}", f.display());
        let fp = fingerprint(&f);
        // Index metadata can change separately from the append-only event log;
        // refresh the inexpensive summary even when file evidence is unchanged.
        let (id, p) = match load(&f, &summaries) {
            Ok(p) => p,
            Err(e) => {
                st.warnings
                    .push(format!("junie: cannot read {}: {e}", f.display()));
                continue;
            }
        };
        if !pm.in_project(&p.cwd) && !p.edits.iter().any(|e| pm.to_rel(&e.0).is_some()) {
            continue;
        }
        let mut g = FileGroup {
            fp,
            ..Default::default()
        };
        for (path, text, model) in &p.edits {
            crate::push_text(&mut g.evs, pm, path, text, Src::Junie, model, &id, p.start);
        }
        g.sessions.push(Session {
            src: Src::Junie,
            id,
            model: if p.models.len() == 1 {
                p.models.into_iter().next().unwrap_or_default()
            } else {
                String::new()
            },
            start: p.start,
            end: p.end,
            tokens: p.tokens,
            branch: None,
            prompts: p.prompts,
            reviewer: false,
            has_content: true,
            provider: None,
        });
        store.groups.insert(key, g);
    }
}
pub fn discover() -> Vec<Activity> {
    let summaries = index();
    let mut out: HashMap<String, Activity> = HashMap::new();
    for f in files() {
        if let Ok((_, p)) = load(&f, &summaries) {
            if p.cwd.is_empty() {
                continue;
            }
            let e = out.entry(p.cwd.clone()).or_insert(Activity {
                src: Src::Junie,
                cwd: p.cwd,
                remote: None,
                sessions: 0,
                last: 0,
            });
            e.sessions += 1;
            e.last = e.last.max(p.end);
        }
    }
    out.into_values().collect()
}
pub fn doctor() -> Health {
    let summaries = index();
    let parsed: Vec<_> = files()
        .iter()
        .filter_map(|f| load(f, &summaries).ok())
        .collect();
    Health{source:"junie-cli".into(),path:home().join("sessions").display().to_string(),readable:!parsed.is_empty(),format:"index.jsonl + <session>/events.jsonl; A2UX completed FileChangesBlockUpdatedEvent".into(),records:parsed.len()as u64,earliest:parsed.iter().map(|(_,p)|p.start).filter(|&t|t>0).min().map(date_cn),latest:parsed.iter().map(|(_,p)|p.end).filter(|&t|t>0).max().map(date_cn),note:creditia_core::tr!("已核对官方 888.12 序列化格式；实现已通过构造记录测试，尚未用真实会话验收。逐响应唯一模型关联随后完成的编辑；缺失或歧义时保留未知。事件没有逐编辑时间，采用会话创建时间。JetBrains IDE 历史是其他入口。", "Official release 888.12 serializer verified; implementation validated with fixtures, not yet with real sessions. Per-response unique model usage attaches to subsequent completed edits; absent or ambiguous models stay unknown. Event codec does not store edit timestamps; session creation time is used. JetBrains IDE history is a separate entrance."),..Default::default()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(e: Value) -> Value {
        json!({"kind":"SessionA2uxEvent","event":{"state":"IN_PROGRESS","agentEvent":e}})
    }
    #[test]
    fn actual_a2ux_completed_changes_model_switch_and_rejection() {
        let change = |status: &str, step: &str, after: &str| {
            event(
                json!({"kind":"FileChangesBlockUpdatedEvent","stepId":step,"status":status,"changes":[{"beforeContent":{"kind":"TextFileContent","text":"keep"},"afterContent":{"kind":"TextFileContent","text":after},"beforeRelativePath":"x.rs","afterRelativePath":"x.rs"}]}),
            )
        };
        let ok = change("COMPLETED", "a", "keep\nfirst new line");
        let rows = [
            event(json!({"kind":"CurrentDirectoryUpdatedEvent","currentDirectory":"/repo"})),
            json!({"kind":"UserPromptEvent","prompt":"go"}),
            event(
                json!({"kind":"LlmResponseMetadataEvent","modelUsage":[{"model":"model-a","inputTokens":5,"outputTokens":2}]}),
            ),
            change("IN_PROGRESS", "a", "keep\nfirst new line"),
            ok.clone(),
            ok,
            change("REJECTED", "bad", "denied line"),
            event(json!({"kind":"LlmResponseMetadataEvent","modelUsage":[{"model":"model-b"}]})),
            change("COMPLETED", "b", "keep\nsecond new line"),
            json!({"kind":"UserPromptEvent","prompt":"next"}),
            change("COMPLETED", "unknown", "keep\nunknown model line"),
        ];
        let p = parse(
            &rows
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
            &Value::Null,
        );
        assert_eq!(p.edits.len(), 3);
        assert_eq!(
            p.edits[0],
            (
                "/repo/x.rs".into(),
                "first new line\n".into(),
                "model-a".into()
            )
        );
        assert_eq!(p.edits[1].2, "model-b");
        assert!(p.edits[2].2.is_empty());
        assert_eq!(p.tokens, 7);
        assert_eq!(p.prompts, 2);
    }
    #[test]
    fn approval_and_binary_content_are_not_text_edits() {
        let v = event(
            json!({"kind":"FileChangesBlockUpdatedEvent","stepId":"a","status":"IN_PROGRESS","approvalRequest":{},"changes":[{"afterContent":{"kind":"TextFileContent","text":"proposed line"},"afterRelativePath":"x.rs"}]}),
        );
        assert!(parse(&v.to_string(), &json!({"projectDir":"/repo"}))
            .edits
            .is_empty());
        let missing=event(json!({"kind":"FileChangesBlockUpdatedEvent","stepId":"missing","status":"COMPLETED","changes":[{"afterContent":{"kind":"TextFileContent","text":"human context\nnew line"},"afterRelativePath":"x.rs"}]}));
        assert!(parse(&missing.to_string(),&json!({"projectDir":"/repo"})).edits.is_empty());
    }
}
