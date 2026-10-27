//! Read-only imports of official Jules activities, Devin v3 session responses / CLI hooks,
//! Warp run responses and v0 v2 message metadata. Never contacts an API or installs hooks.
//! Schemas checked 2026-10-17: docs.devin.ai/v3-openapi.yaml;
//! jules.google/docs/api/reference/{activities,sources}/;
//! docs.warp.dev/openapi.json; v0.app/docs/api/v2/reference/messages/get-message.
use crate::trace_import as imp;
use crate::{any_ts, source_dir, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

const SOURCES: [Src; 5] = [
    Src::Devin,
    Src::Jules,
    Src::Replit,
    Src::V0,
    Src::Warp,
];
fn items<'a>(v: &'a Value, key: &str) -> Vec<&'a Value> {
    v[key]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_else(|| vec![v])
}
fn pr_remote(url: &str) -> String {
    url.split("/pull/")
        .next()
        .unwrap_or(url)
        .trim_end_matches('/')
        .into()
}
fn remote(src: Src, v: &Value) -> String {
    match src {
        Src::Devin => v["pull_requests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["pr_url"].as_str())
            .next()
            .map(pr_remote)
            .unwrap_or_default(),
        Src::Warp => v["artifacts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a["artifact_type"] == "PULL_REQUEST")
            .filter_map(|a| a["data"]["url"].as_str())
            .next()
            .map(pr_remote)
            .unwrap_or_default(),
        _ => String::new(),
    }
}
fn jules_sources(root: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for p in imp::files(root) {
        for v in imp::read(&p) {
            for s in items(&v, "sources") {
                if let (Some(name), Some(owner), Some(repo)) = (
                    s["name"].as_str(),
                    s["githubRepo"]["owner"].as_str(),
                    s["githubRepo"]["repo"].as_str(),
                ) {
                    out.insert(name.into(), format!("https://github.com/{owner}/{repo}"));
                }
            }
        }
    }
    out
}
fn parse_jules(rows: &[Value], sources: &HashMap<String, String>, pm: &PathMap, g: &mut FileGroup) {
    let mut sessions: HashMap<String, (i64, i64, i64)> = HashMap::new();
    for v in rows {
        for a in items(v, "activities") {
            let Some(name) = a["name"]
                .as_str()
                .filter(|s| s.starts_with("sessions/") && s.contains("/activities/"))
            else {
                continue;
            };
            let id = name.split("/activities/").next().unwrap_or(name);
            let ts = any_ts(&a["createTime"]).unwrap_or(0);
            let mut relevant = false;
            // A changeSet artifact is a produced patch. Plans/progress text and bash output aren't file edits.
            if a["originator"] == "agent" && a.get("sessionFailed").is_none() {
                for artifact in a["artifacts"].as_array().into_iter().flatten() {
                    let c = &artifact["changeSet"];
                    let Some(repo) = c["source"]
                        .as_str()
                        .and_then(|s| sources.get(s))
                        .filter(|s| pm.in_project(s))
                    else {
                        continue;
                    };
                    relevant = true;
                    if let Some(diff) = c["gitPatch"]["unidiffPatch"].as_str() {
                        for (p, t) in imp::diff_files(diff) {
                            imp::push_scoped(g, pm, repo, &p, &t, Src::Jules, "", id, ts);
                        }
                    }
                }
            }
            if relevant {
                let e = sessions.entry(id.into()).or_insert((ts, ts, 0));
                if ts > 0 {
                    e.0 = e.0.min(ts);
                    e.1 = e.1.max(ts);
                }
                if a.get("userMessaged").is_some() {
                    e.2 += 1;
                }
            }
        }
    }
    for (id, (start, end, prompts)) in sessions {
        g.sessions.push(imp::session(
            Src::Jules,
            &id,
            "",
            start,
            end,
            prompts,
            !g.evs.is_empty(),
        ));
    }
}
fn parse_devin_hooks(rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    for v in rows {
        if v["hook_event_name"] != "PostToolUse"
            || v["tool_response"]["success"] != true
            || v["tool_response"]["error"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        let Some(id) = v["session_id"].as_str() else {
            continue;
        };
        // Official hooks leave edit input tool-specific. Only known apply_patch syntax in an exec command is parsed.
        if v["tool_name"] != "exec" {
            continue;
        }
        let Some(cmd) = v["tool_input"]["command"].as_str().filter(|c| {
            c.trim_start()
                .strip_prefix("apply_patch")
                .is_some_and(|tail| tail.starts_with(char::is_whitespace))
        }) else {
            continue;
        };
        let before = g.evs.len();
        for (p, t) in crate::apply_patch_files(cmd) {
            imp::push_scoped(g, pm, "", &p, &t, Src::Devin, "", id, 0);
        }
        if g.evs.len() > before {
            g.sessions
                .push(imp::session(Src::Devin, id, "", 0, 0, 0, true));
        }
    }
}
/// Native imports replace only matching line occurrences. Other CLI edits from
/// the same session remain useful, including paths outside a partial capture.
fn dedup_devin_hooks(group: &mut FileGroup, native: &FileGroup) {
    let mut covered = HashMap::new();
    for ev in &native.evs {
        *covered.entry((ev.session.clone(), ev.rel.clone(), ev.key)).or_insert(0usize) += 1;
    }
    group.evs.retain(|ev| {
        if let Some(count) = covered.get_mut(&(ev.session.clone(), ev.rel.clone(), ev.key)) {
            if *count > 0 {
                *count -= 1;
                return false;
            }
        }
        true
    });
    // A native session with mapped evidence owns its metadata even when the
    // CLI still supplies uncovered lines. Keep one session record overall.
    group.sessions.retain(|session| {
        !(native.sessions.iter().any(|s| s.id == session.id)
            && native.evs.iter().any(|ev| ev.session == session.id))
    });
}

fn parse_metadata(src: Src, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let mut seen_messages = std::collections::HashSet::new();
    for v in rows {
        if src == Src::V0 {
            // v2 file-edit parts have operation/path/time but do not contain edit text. Preserve metadata only.
            for m in items(v, "messages") {
                if m["chatId"].as_str().is_none()
                    || m["id"].as_str().is_none()
                    || m["role"] != "assistant"
                {
                    continue;
                }
                if !seen_messages.insert(m["id"].as_str().unwrap_or("")) {
                    continue;
                }
                let scoped = m["parts"].as_array().into_iter().flatten().any(|p| {
                    p["type"] == "file-edit"
                        && p["path"].as_str().is_some_and(|p| pm.to_rel(p).is_some())
                });
                if !scoped {
                    continue;
                }
                let mut s = imp::session(
                    src,
                    m["chatId"].as_str().unwrap_or(""),
                    m["usage"]["model"].as_str().unwrap_or(""),
                    any_ts(&m["createdAt"]).unwrap_or(0),
                    any_ts(&m["updatedAt"]).unwrap_or(0),
                    0,
                    false,
                );
                s.tokens = m["usage"]["tokens"]["total"].as_i64().unwrap_or(0);
                g.sessions.push(s);
            }
            continue;
        }
        for r in items(v, if src == Src::Devin { "items" } else { "runs" }) {
            let key = if src == Src::Devin {
                "session_id"
            } else {
                "run_id"
            };
            let Some(id) = r[key].as_str() else { continue };
            let repo = remote(src, r);
            if repo.is_empty() || !pm.in_project(&repo) {
                continue;
            }
            let model = if src == Src::Warp {
                r["agent_config"]["model_id"].as_str().unwrap_or("")
            } else {
                ""
            };
            g.sessions.push(imp::session(
                src,
                id,
                model,
                any_ts(&r["created_at"]).unwrap_or(0),
                any_ts(&r["updated_at"]).unwrap_or(0),
                0,
                false,
            ));
        }
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let native_warp = crate::warp::native_session_aliases(Some(pm));
    for src in SOURCES {
        // Replit uses cloud_native's official Timeline schema. Bolt/Lovable have
        // their own verified native readers and must not be double-counted here.
        if matches!(src, Src::Replit) {
            continue;
        }
        let root = source_dir(src);
        let sources = if src == Src::Jules {
            jules_sources(&root)
        } else {
            HashMap::new()
        };
        let rows: Vec<Value> = imp::files(&root)
            .iter()
            .flat_map(|p| imp::read(p))
            .collect();
        let mut g = FileGroup::default();
        if src == Src::Jules {
            parse_jules(&rows, &sources, pm, &mut g);
        } else {
            parse_metadata(src, &rows, pm, &mut g);
            if src == Src::Devin {
                parse_devin_hooks(&rows, pm, &mut g);
            }
        }
        // Multiple downloaded pages can contain the same session; don't inflate totals.
        let mut sessions: HashMap<String, creditia_core::model::Session> = HashMap::new();
        for s in g.sessions.drain(..) {
            if let Some(prev) = sessions.get_mut(&s.id) {
                if s.start > 0 {
                    prev.start = if prev.start == 0 {
                        s.start
                    } else {
                        prev.start.min(s.start)
                    };
                }
                prev.end = prev.end.max(s.end);
                prev.tokens = if src == Src::V0 {
                    prev.tokens + s.tokens
                } else {
                    prev.tokens.max(s.tokens)
                };
                prev.prompts = prev.prompts.max(s.prompts);
                prev.has_content |= s.has_content;
                if !s.model.is_empty() {
                    prev.model = s.model;
                }
            } else {
                sessions.insert(s.id.clone(), s);
            }
        }
        g.sessions = sessions.into_values().collect();
        if src == Src::Warp {
            g.sessions.retain(|s| !native_warp.contains(&s.id));
        }
        if src == Src::Devin {
            let native = crate::devin_cloud::native_evidence(&root, pm);
            dedup_devin_hooks(&mut g, &native);
        }
        let prefix = format!("{}:import:", src.harness());
        store.groups.retain(|k, _| !k.starts_with(&prefix));
        if !g.sessions.is_empty() {
            st.notes.push(format!(
                "{}: {} project sessions from official exports",
                src.harness(),
                g.sessions.len()
            ));
        }
        store
            .groups
            .insert(format!("{prefix}{}", root.display()), g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    let native_warp = crate::warp::native_session_aliases(None);
    for src in [Src::Jules, Src::Devin, Src::Warp] {
        let root = source_dir(src);
        if src == Src::Jules {
            for remote in jules_sources(&root).into_values() {
                out.push(imp::activity(src, remote.clone(), Some(remote), 0));
            }
        } else {
            for p in imp::files(&root) {
                for v in imp::read(&p) {
                    for r in items(&v, if src == Src::Devin { "items" } else { "runs" }) {
                        if src == Src::Warp
                            && r["run_id"].as_str().is_some_and(|id| native_warp.contains(id))
                        {
                            continue;
                        }
                        let repo = remote(src, r);
                        if !repo.is_empty() {
                            out.push(imp::activity(
                                src,
                                repo.clone(),
                                Some(repo),
                                any_ts(&r["updated_at"]).unwrap_or(0),
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}
pub fn doctor() -> Vec<Health> {
    let mut out:Vec<_>=SOURCES.into_iter().filter(|src| *src != Src::Replit).map(|src|{
        let root=source_dir(src);let rows:Vec<Value>=imp::files(&root).iter().flat_map(|p|imp::read(p)).collect();
        let records=rows.iter().map(|v|match src{
            Src::Jules=>items(v,"activities").iter().filter(|a|a["name"].as_str().is_some_and(|s|s.contains("/activities/"))).count(),
            Src::Devin=>items(v,"items").iter().filter(|a|a["session_id"].is_string()).count(),
            Src::Warp=>items(v,"runs").iter().filter(|a|a["run_id"].is_string()).count(),
            Src::V0=>items(v,"messages").iter().filter(|a|a["chatId"].is_string()&&a["id"].is_string()).count(),
            _=>0,
        }).sum();
        let(format,note,unsupported)=match src{
            Src::Jules=>("Jules v1alpha sources + activities JSON","Reads agent changeSet.gitPatch.unidiffPatch artifacts. Export sources responses alongside activities for repository identity; activities alone cannot bind relative paths to a repository. No model field is published, so model remains unknown.",false),
            Src::Devin=>("Devin v3 session JSON / CLI PostToolUse JSONL","Cloud API: session/PR/time metadata only. Official v1-v3 messages have no successful file-tool results or model identifier; attachments expose URLs, not edit results, and structured_output is unconstrained (devin_mode is not a model). CLI: success=true PostToolUse exec apply_patch supports absolute paths only; only matching line occurrences covered by mapped native ACP evidence are deduplicated; uncovered CLI edits remain. Devin Desktop Cascade uses its separate adapter.",false),
            Src::Warp=>("Warp Oz run JSON","Reads run/session time and agent_config.model_id when associated with a PR repository. Matching readable native sessions take precedence. Native SQLite and official ConversationData protobufs use the separate Warp reader. Oz generic action input/output objects and CLI Markdown are not inferred as edits.",false),
            Src::V0=>("v0 v2 message JSON","Reads usage.model, tokens and times when file-edit parts contain project-resolvable paths. Official v2 file-edit schema permits path/operation/time only, no content/diff. Get Chat Files returns current contents without a historical message/snapshot parameter; tool-call inputs/outputs are unconstrained, and bash exitCode is documented as currently omitted. Neither proves per-edit success. Relative paths alone and v1 generated/included snapshots cannot establish local agent authorship.",false),
            _=>unreachable!(),
        };
        imp::health(src,&root,format,records,note,unsupported)
    }).collect();
    if let Some(cloud) = out.iter_mut().find(|h| h.source == "devin") {
        cloud.source = "devin-cloud".into();
    }
    if let Some(oz) = out.iter_mut().find(|h| h.source == "warp") {
        oz.source = "warp-oz".into();
    }
    let root = source_dir(Src::Devin);
    let records = imp::files(&root)
        .iter()
        .flat_map(|p| imp::read(p))
        .filter(|v| {
            v["hook_event_name"] == "PostToolUse"
                && v["tool_response"]["success"] == true
                && v["tool_name"] == "exec"
        })
        .count();
    let mut cli=imp::health(Src::Devin,&root,"Devin CLI PostToolUse JSONL",records,"Successful exec apply_patch hooks only; absolute paths required. Missing execution time stays unknown and needs a unique non-conflicting Git history match.",false);
    cli.source = "devin-cli".into();
    out.push(cli);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["a.py".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        }
    }
    #[test]
    fn jules_real_patch_requires_agent_and_source_binding() {
        let rows = vec![
            json!({"name":"sessions/s/activities/a","originator":"agent","createTime":"2026-10-17T01:00:00Z","artifacts":[{"changeSet":{"source":"sources/github-org-demo","gitPatch":{"unidiffPatch":"diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1 +1 @@\n-old_line\n+new_line\n"}}}]}),
        ];
        let mut g = FileGroup::default();
        parse_jules(&rows, &HashMap::new(), &pm(), &mut g);
        assert!(g.evs.is_empty());
        let sources = [(
            "sources/github-org-demo".into(),
            "https://github.com/org/demo".into(),
        )]
        .into_iter()
        .collect();
        parse_jules(&rows, &sources, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].session, "sessions/s");
        assert_eq!(g.evs[0].model, "");
    }
    #[test]
    fn devin_cloud_mode_and_message_are_never_line_or_model_evidence() {
        let rows = vec![
            json!({"session_id":"devin-s","devin_mode":"swe-2-max","created_at":1780000000,"updated_at":1780000100,"messages":[{"source":"devin","message":"I wrote code"}],"pull_requests":[{"pr_url":"https://github.com/org/demo/pull/1"}]}),
        ];
        let mut g = FileGroup::default();
        parse_metadata(Src::Devin, &rows, &pm(), &mut g);
        assert_eq!(g.sessions.len(), 1);
        assert!(g.evs.is_empty());
        assert_eq!(g.sessions[0].model, "");
        assert!(!g.sessions[0].has_content);
    }
    #[test]
    fn devin_hook_pre_and_failed_do_not_count() {
        let mut v = json!({"hook_event_name":"PostToolUse","tool_name":"exec","session_id":"s","tool_input":{"command":"apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: /demo/a.py\n+new_line\n*** End Patch\nPATCH"},"tool_response":{"success":false,"error":"failed"}});
        let mut g = FileGroup::default();
        parse_devin_hooks(&[v.clone()], &pm(), &mut g);
        assert!(g.evs.is_empty());
        v["tool_response"] = json!({"success":true,"error":null});
        parse_devin_hooks(&[v], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
    }
    #[test]
    fn devin_native_precedence_preserves_uncovered_cli_lines_and_occurrences() {
        let hook = json!({"hook_event_name":"PostToolUse","tool_name":"exec","session_id":"s","tool_input":{"command":"apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: /demo/a.py\n+native_covered_line\n+cli_only_line\n*** End Patch\nPATCH"},"tool_response":{"success":true}});
        let mut cli = FileGroup::default();
        parse_devin_hooks(std::slice::from_ref(&hook), &pm(), &mut cli);
        assert_eq!(cli.evs.len(), 2);
        // No actual native line evidence cannot suppress a valid hook.
        dedup_devin_hooks(&mut cli, &FileGroup::default());
        assert_eq!(cli.evs.len(), 2);
        let mut native = FileGroup::default();
        native.evs.push(cli.evs[0].clone());
        native.sessions.push(imp::session(Src::Devin, "s", "", 0, 0, 0, true));
        let remaining_key = cli.evs[1].key;
        dedup_devin_hooks(&mut cli, &native);
        assert_eq!(cli.evs.len(), 1);
        assert_eq!(cli.evs[0].key, remaining_key);
        assert!(cli.sessions.is_empty());
        native.evs = cli.evs.clone();
        dedup_devin_hooks(&mut cli, &native);
        assert!(cli.evs.is_empty());
        assert!(cli.sessions.is_empty());
        // One native occurrence cannot erase two separate CLI occurrences.
        let mut cli = FileGroup::default();
        parse_devin_hooks(&[hook.clone(), hook], &pm(), &mut cli);
        let mut native = FileGroup::default();
        native.evs.push(cli.evs[0].clone());
        dedup_devin_hooks(&mut cli, &native);
        assert_eq!(cli.evs.len(), 3);
    }
    #[test]
    fn partial_devin_native_body_owns_metadata_without_discarding_cli_only_lines() {
        let lines = (0..8).map(|n| format!("+line_number_{n}\n")).collect::<String>();
        let hook = json!({"hook_event_name":"PostToolUse","tool_name":"exec","session_id":"s","tool_input":{"command":format!("apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: /demo/a.py\n{lines}*** End Patch\nPATCH")},"tool_response":{"success":true}});
        let mut cli = FileGroup::default();
        parse_devin_hooks(&[hook], &pm(), &mut cli);
        assert_eq!(cli.evs.len(), 8);
        dedup_devin_hooks(&mut cli, &FileGroup::default());
        assert_eq!(cli.evs.len(), 8);
        assert_eq!(cli.sessions.len(), 1);
        // Native metadata without mapped line evidence cannot take ownership.
        let mut native = FileGroup::default();
        native.sessions.push(imp::session(Src::Devin, "s", "", 123, 456, 2, true));
        dedup_devin_hooks(&mut cli, &native);
        assert_eq!(cli.evs.len(), 8);
        assert_eq!(cli.sessions.len(), 1);
        native.evs = cli.evs[..4].to_vec();
        dedup_devin_hooks(&mut cli, &native);
        assert_eq!(cli.evs.len(), 4);
        assert!(cli.sessions.is_empty(), "one native session owns metadata even when CLI supplements four lines");
    }
    #[test]
    fn v0_paths_are_metadata_without_text() {
        let v = json!({"id":"m","chatId":"c","role":"assistant","createdAt":"2026-10-17T01:00:00Z","parts":[{"type":"file-edit","operation":"create","path":"/demo/a.py"}],"usage":{"model":"v0-pro","tokens":{"total":42}}});
        let mut g = FileGroup::default();
        parse_metadata(Src::V0, &[v], &pm(), &mut g);
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(g.sessions[0].tokens, 42);
        assert!(g.evs.is_empty());
    }
}
