//! Import the official SWE-agent .traj, mini-SWE-agent 1.1, and OpenHands 0.60 event serialization.
//! Contracts: SWE-agent/SWE-agent sweagent/agent/agents.py and sweagent/types.py;
//! SWE-agent/mini-swe-agent src/minisweagent/{agents/default,models/utils/actions_toolcall}.py;
//! OpenHands/OpenHands tag 0.60.0 openhands/events/{action/files,observation/files,serialization/event}.py.
use crate::trace_import as imp;
use crate::{any_ts, source_dir, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::Path;

const SOURCES: [Src; 3] = [Src::OpenHands, Src::SweAgent, Src::MiniSweAgent];

/// A literal heredoc has a known destination and exact bytes. Do not treat arbitrary shell
/// command text (read output, Python scripts, nested examples) as evidence of writing every path it mentions.
fn literal_shell_write(cmd: &str) -> Option<(String, String)> {
    let re=regex::Regex::new(r#"(?s)^\s*(?:cat\s*>{1,2}\s*|tee(?:\s+-a)?\s+)(?:"([^"]+)"|'([^']+)'|([^\s<>]+))\s*<<\s*(?:'([A-Za-z_][A-Za-z0-9_]*)'|"([A-Za-z_][A-Za-z0-9_]*)")\s*\n(.*)$"#).unwrap();
    let c = re.captures(cmd)?;
    let path = c.get(1).or(c.get(2)).or(c.get(3))?.as_str();
    if path.contains(['$', '`']) {
        return None;
    }
    let delimiter = c.get(4).or(c.get(5))?.as_str();
    let tail = c.get(6)?.as_str().trim_end_matches('\n');
    let body = tail.strip_suffix(delimiter)?.strip_suffix('\n')?;
    if body.lines().any(|line| line == delimiter) {
        return None;
    }
    Some((path.into(), format!("{body}\n")))
}

fn swe_config(v: &Value) -> Value {
    if let Some(s) = v["replay_config"].as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else {
        v["replay_config"].clone()
    }
}
fn context(src: Src, v: &Value) -> (String, String) {
    match src {
        Src::MiniSweAgent => (
            v["info"]["config"]["environment"]["cwd"]
                .as_str()
                .unwrap_or("")
                .into(),
            v["info"]["config"]["model"]["model_name"]
                .as_str()
                .unwrap_or("")
                .into(),
        ),
        Src::SweAgent => {
            let c = swe_config(v);
            (
                c["env"]["repo"]["path"]
                    .as_str()
                    .or(c["env"]["repo"]["github_url"].as_str())
                    .unwrap_or("")
                    .into(),
                c["agent"]["model"]["name"].as_str().unwrap_or("").into(),
            )
        }
        _ => (String::new(), String::new()),
    }
}

fn parse_mini(path: &Path, v: &Value, pm: &PathMap, g: &mut FileGroup) {
    if v["trajectory_format"] != "mini-swe-agent-1.1" {
        return;
    }
    let (cwd, model) = context(Src::MiniSweAgent, v);
    if !pm.in_project(&cwd) {
        return;
    }
    let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let messages = v["messages"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut pending = std::collections::HashMap::new();
    let mut start = 0;
    let mut end = 0;
    let mut prompts = 0;
    for m in messages {
        let ts = any_ts(&m["extra"]["timestamp"]).unwrap_or(0);
        if ts > 0 {
            if start == 0 {
                start = ts;
            }
            end = end.max(ts);
        }
        if m["role"] == "assistant" {
            for a in m["extra"]["actions"].as_array().into_iter().flatten() {
                if let (Some(call), Some(cmd)) = (a["tool_call_id"].as_str(), a["command"].as_str())
                {
                    pending.insert(call, cmd);
                }
            }
        } else if m["role"] == "tool" {
            let Some(cmd) = m["tool_call_id"].as_str().and_then(|id| pending.remove(id)) else {
                continue;
            };
            if m["extra"]["returncode"].as_i64() != Some(0)
                || m["extra"]["exception_info"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            {
                continue;
            }
            if cmd
                .trim_start()
                .strip_prefix("apply_patch")
                .is_some_and(|tail| tail.starts_with(char::is_whitespace))
            {
                for (p, t) in crate::apply_patch_files(cmd) {
                    imp::push_scoped(g, pm, &cwd, &p, &t, Src::MiniSweAgent, &model, id, ts);
                }
            }
            // Only a literal file redirection / tee heredoc is a supported shell edit.
            // Reading a file, an unpaired request, and unsuccessful shell calls cannot attribute code.
            if let Some((path, text)) = literal_shell_write(cmd) {
                imp::push_scoped(g, pm, &cwd, &path, &text, Src::MiniSweAgent, &model, id, ts);
            }
        } else if m["role"] == "user" && m.get("extra").is_none() {
            prompts += 1;
        }
    }
    let mut session = imp::session(
        Src::MiniSweAgent,
        id,
        &model,
        start,
        end,
        prompts,
        !g.evs.is_empty(),
    );
    session.tokens = v["info"]["model_stats"]["tokens_sent"]
        .as_i64()
        .unwrap_or(0)
        + v["info"]["model_stats"]["tokens_received"]
            .as_i64()
            .unwrap_or(0);
    g.sessions.push(session);
}

fn parse_swe(path: &Path, v: &Value, pm: &PathMap, g: &mut FileGroup) {
    if !v["trajectory"].is_array() || !v["history"].is_array() || v["info"].is_null() {
        return;
    }
    let (cwd, model) = context(Src::SweAgent, v);
    if !pm.in_project(&cwd) {
        return;
    }
    let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    // This is the actual patch read from model.patch by handle_submission, not the proposed action text.
    if v["info"]["exit_status"]
        .as_str()
        .is_some_and(|s| s == "submitted" || s.starts_with("submitted ("))
    {
        if let Some(diff) = v["info"]["submission"].as_str() {
            for (p, t) in imp::diff_files(diff) {
                imp::push_scoped(g, pm, &cwd, &p, &t, Src::SweAgent, &model, id, 0);
            }
        }
    }
    let mut s = imp::session(Src::SweAgent, id, &model, 0, 0, 1, !g.evs.is_empty());
    s.tokens = v["info"]["model_stats"]["tokens_sent"]
        .as_i64()
        .unwrap_or(0)
        + v["info"]["model_stats"]["tokens_received"]
            .as_i64()
            .unwrap_or(0);
    g.sessions.push(s);
}

fn parse_openhands(path: &Path, rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let events: Vec<&Value> = rows
        .iter()
        .flat_map(|v| {
            v["trajectory"]
                .as_array()
                .or(v["events"].as_array())
                .map(|a| a.iter().collect())
                .unwrap_or_else(|| vec![v])
        })
        .collect();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let id = if stem.parse::<u64>().is_ok() {
        path.parent()
            .and_then(|p| {
                if p.file_name().is_some_and(|s| s == "events") {
                    p.parent()
                } else {
                    Some(p)
                }
            })
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };
    let mut actions = std::collections::HashMap::new();
    let mut start = 0;
    let mut end = 0;
    let mut prompts = 0;
    for e in &events {
        if e["source"] == "agent" && e["action"].is_string() {
            if let Some(id) = e["id"].as_i64() {
                actions.insert(id, *e);
            }
        }
    }
    for e in events {
        let ts = any_ts(&e["timestamp"]).unwrap_or(0);
        if ts > 0 {
            if start == 0 {
                start = ts;
            }
            end = end.max(ts);
        }
        if e["source"] == "user" && e["action"] == "message" {
            prompts += 1;
        }
        let Some(a) = e["cause"].as_i64().and_then(|c| actions.get(&c)) else {
            continue;
        };
        let kind = e["observation"].as_str().unwrap_or("");
        if !matches!(kind, "write" | "edit") || a["action"] != kind || e["success"] == false {
            continue;
        }
        let content = e["content"].as_str().unwrap_or("");
        if content.contains("Changes are NOT applied") {
            continue;
        }
        let p = e["extras"]["path"].as_str().unwrap_or("");
        if p != a["args"]["path"].as_str().unwrap_or("") {
            continue;
        }
        // Model identity is optional in the published event format; never derive it from a product name.
        let model = a["llm_metrics"]["token_usages"]
            .as_array()
            .and_then(|a| a.last())
            .and_then(|u| u["model"].as_str())
            .unwrap_or("");
        if kind == "edit" {
            if let (Some(old), Some(new)) = (
                e["extras"]["old_content"].as_str(),
                e["extras"]["new_content"].as_str(),
            ) {
                imp::push_scoped(
                    g,
                    pm,
                    "",
                    p,
                    &crate::added_lines(old, new),
                    Src::OpenHands,
                    model,
                    id,
                    ts,
                );
            }
        } else if let Some(text) = a["args"]["content"].as_str() {
            imp::push_scoped(g, pm, "", p, text, Src::OpenHands, model, id, ts);
        }
    }
    if !g.evs.is_empty() {
        let model = g.evs.last().map(|e| e.model.clone()).unwrap_or_default();
        g.sessions.push(imp::session(
            Src::OpenHands,
            id,
            &model,
            start,
            end,
            prompts,
            true,
        ));
    }
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    for src in SOURCES {
        if src == Src::OpenHands {
            scan_openhands(store, pm, st);
            continue;
        }
        imp::scan_files(src, &source_dir(src), store, pm, st, |p, rows, pm, g| {
            for v in rows {
                match src {
                    Src::SweAgent => parse_swe(p, v, pm, g),
                    Src::MiniSweAgent => parse_mini(p, v, pm, g),
                    _ => {}
                }
            }
        });
    }
}
fn scan_openhands(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let root = source_dir(Src::OpenHands);
    let mut bundles: std::collections::HashMap<std::path::PathBuf, Vec<Value>> =
        std::collections::HashMap::new();
    // Native 0.60 storage has one numbered JSON file per event. Pair only within the same events directory.
    for p in imp::files(&root) {
        let rows = imp::read(&p);
        let key = if p
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.parse::<u64>().is_ok())
            && rows
                .iter()
                .all(|v| v["action"].is_string() || v["observation"].is_string())
        {
            p.parent().unwrap_or(&root).join("0.json")
        } else {
            p
        };
        bundles.entry(key).or_default().extend(rows);
    }
    let prefix = "openhands:import:";
    let keys: std::collections::HashSet<String> = bundles
        .keys()
        .map(|p| format!("{prefix}{}", p.display()))
        .collect();
    store
        .groups
        .retain(|k, _| !k.starts_with(prefix) || keys.contains(k));
    for (p, mut rows) in bundles {
        rows.sort_by_key(|e| e["id"].as_i64().unwrap_or(0));
        let mut g = FileGroup::default();
        parse_openhands(&p, &rows, pm, &mut g);
        if !g.sessions.is_empty() {
            st.notes
                .push(format!("openhands: {} project event stream", p.display()));
        }
        store.groups.insert(format!("{prefix}{}", p.display()), g);
    }
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for src in SOURCES {
        for p in imp::files(&source_dir(src)) {
            for v in imp::read(&p) {
                let (cwd, _) = context(src, &v);
                if !cwd.is_empty() {
                    let remote = cwd.starts_with("https://").then(|| cwd.clone());
                    out.push(imp::activity(src, cwd, remote, 0));
                }
            }
        }
    }
    out
}
pub fn doctor() -> Vec<Health> {
    SOURCES.into_iter().map(|src| {let root=source_dir(src);let n=imp::files(&root).iter().filter(|p|!imp::read(p).is_empty()).count();imp::health(src,&root,match src{Src::MiniSweAgent=>"mini-swe-agent-1.1 .traj.json",Src::SweAgent=>"SWE-agent .traj + replay_config",_=>"OpenHands 0.60 serialized event JSON/JSONL"},n,match src{Src::MiniSweAgent=>"Imports official 1.1 toolcall trajectories. Requires environment.cwd; only matched tool_call_id with returncode=0, no exception and explicit shell write/apply_patch. Other shell edits are not inferred.",Src::SweAgent=>"Imports submitted info.submission unified patches, scoped by replay_config.env.repo; model from replay_config.agent.model.name. Trajectories do not provide reliable wall timestamps; unfinished runs stay metadata-only.",_=>"Imports version 0.60 event serialization: agent write/edit paired with matching successful file observation by cause and path. Error observations and unapplied edits are excluded. Modern SDK trajectory format is not assumed compatible."},false)}).collect()
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
    fn mini_requires_paired_success() {
        let v = json!({"trajectory_format":"mini-swe-agent-1.1","info":{"config":{"environment":{"cwd":"/demo"},"model":{"model_name":"openai/gpt-4.1"}}},"messages":[{"role":"assistant","extra":{"actions":[{"command":"cat > a.py <<'EOF'\naccepted_line\nEOF","tool_call_id":"ok"},{"command":"cat > a.py <<'EOF'\nrejected_line\nEOF","tool_call_id":"bad"}]}},{"role":"tool","tool_call_id":"bad","extra":{"returncode":1}},{"role":"tool","tool_call_id":"ok","extra":{"returncode":0,"timestamp":1780000000,"exception_info":""}}]});
        let mut g = FileGroup::default();
        parse_mini(Path::new("s.traj.json"), &v, &pm(), &mut g);
        assert!(g.evs.iter().any(|e| e.model == "openai/gpt-4.1"));
        assert!(!g
            .evs
            .iter()
            .any(|e| e.key == creditia_core::hash::line_key("a.py", "rejected_line")));
    }
    #[test]
    fn swe_only_actual_submitted_patch() {
        let mut v = json!({"replay_config":{"env":{"repo":{"github_url":"https://github.com/org/demo"}},"agent":{"model":{"name":"gpt-4.1"}}},"trajectory":[],"history":[],"info":{"exit_status":"failed","submission":"diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -0,0 +1 @@\n+actual_line\n"}});
        let mut g = FileGroup::default();
        parse_swe(Path::new("s.traj"), &v, &pm(), &mut g);
        assert!(g.evs.is_empty());
        v["info"]["exit_status"] = json!("submitted");
        let mut g = FileGroup::default();
        parse_swe(Path::new("s.traj"), &v, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "gpt-4.1");
    }
    #[test]
    fn openhands_requires_matching_success_observation() {
        let rows = vec![
            json!({"id":1,"source":"agent","action":"write","args":{"path":"/demo/a.py","content":"accepted_line"}}),
            json!({"observation":"error","cause":1,"content":"failure","extras":{"path":"/demo/a.py"}}),
        ];
        let mut g = FileGroup::default();
        parse_openhands(Path::new("session/events.json"), &rows, &pm(), &mut g);
        assert!(g.evs.is_empty());
        let mut good = rows;
        good.push(json!({"observation":"write","cause":1,"extras":{"path":"/demo/a.py"}}));
        parse_openhands(Path::new("session/events.json"), &good, &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
    }
}
