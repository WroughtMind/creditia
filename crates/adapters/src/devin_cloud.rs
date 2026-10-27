//! Offline Devin cloud ACP session-update reader. The web client replays history
//! through WebSocket `_cognition.ai/session/loadPage` responses, `session/update`
//! notifications and `_cognition.ai/session_update_batch` batches; every shape
//! below is that native envelope, never a re-derived export. See SUPPORT_DEVIN.md.
use crate::trace_import as imp;
use crate::{added_lines, any_ts, source_dir, Activity, Health, Stats};
use creditia_core::{
    model::{FileGroup, PathMap, Src, Store},
    tr,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

const META: &str = "cognition.ai/";

fn mkey(key: &str) -> String {
    format!("{META}{key}")
}

fn meta_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v["_meta"].get(mkey(key)).and_then(Value::as_str)
}

fn safe_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Accept the native envelopes only: a loadPage result (bare or under a JSON-RPC
/// `result`), a `session/update` notification, or the batched ext notification.
fn native_rows(v: &Value) -> Vec<(String, Value)> {
    let page = |body: &Value| -> Option<Vec<(String, Value)>> {
        let updates = body["updates"].as_array()?;
        let mut out = vec![];
        for u in updates {
            if let (Some(session), Some(update)) = (
                u["sessionId"].as_str().filter(|s| !s.is_empty()),
                u["update"].as_object(),
            ) {
                if update["sessionUpdate"].as_str().is_some() {
                    out.push((session.to_owned(), u["update"].clone()));
                }
            }
        }
        (!out.is_empty()
            && body["coveredRange"].is_object()
            && body["hasMore"].is_boolean()
            && body["replayFilterVersion"].is_string())
        .then_some(out)
    };
    let mut out = vec![];
    if let Some(rows) = page(v) {
        out.extend(rows);
    }
    if v["method"] == "session/update" {
        if let (Some(session), Some(update)) = (
            v["params"]["sessionId"].as_str().filter(|s| !s.is_empty()),
            v["params"]["update"].as_object(),
        ) {
            if update["sessionUpdate"].as_str().is_some() {
                out.push((session.into(), v["params"]["update"].clone()));
            }
        }
    }
    if v["method"] == "ext/notification"
        && v["params"]["method"] == "_cognition.ai/session_update_batch"
    {
        for u in v["params"]["params"]["updates"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let (Some(session), Some(update)) = (
                u["sessionId"].as_str().filter(|s| !s.is_empty()),
                u["update"].as_object(),
            ) {
                if update["sessionUpdate"].as_str().is_some() {
                    out.push((session.into(), u["update"].clone()));
                }
            }
        }
    }
    // A captured JSON-RPC response frame carries the same page under `result`.
    if let Some(result) = v.get("result").filter(|r| r.is_object()) {
        out.extend(native_rows(result));
    }
    out
}

fn read_rows(root: &Path) -> Vec<Vec<Value>> {
    imp::files(root)
        .iter()
        .map(|p| imp::read(p))
        .collect::<Vec<_>>()
}

#[derive(Default)]
struct Call {
    kind: Option<String>,
    status: Option<String>,
    // The latest non-empty content array replaces earlier ones, matching the
    // client's tool_call_update field replacement.
    content: Option<Vec<Value>>,
    ts: Option<i64>,
}

fn collect_calls(files: &[Vec<Value>]) -> BTreeMap<(String, String), Vec<Call>> {
    // One merged observation sequence per import file; cross-file ordering is
    // never inferred, so overlapping pages reconcile by native identity only.
    let mut calls: BTreeMap<(String, String), Vec<Call>> = BTreeMap::new();
    for rows in files {
        let mut file_calls: BTreeMap<(String, String), Call> = BTreeMap::new();
        for (session, update) in rows.iter().flat_map(native_rows) {
            let kind = update["sessionUpdate"].as_str().unwrap_or_default();
            if !matches!(kind, "tool_call" | "tool_call_update") {
                continue;
            }
            let Some(id) = update["toolCallId"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            let call = file_calls.entry((session.clone(), id.into())).or_default();
            // tool_call_update replaces exactly the fields it carries.
            if let Some(k) = update["kind"].as_str() {
                call.kind = Some(k.into());
            }
            if let Some(s) = update["status"].as_str() {
                call.status = Some(s.into());
            }
            if update
                .get("content")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
            {
                call.content = update["content"].as_array().cloned();
            }
            if let Some(t) = meta_str(&update, "timestamp") {
                call.ts = any_ts(&Value::String(t.into()));
            }
        }
        for (key, call) in file_calls {
            calls.entry(key).or_default().push(call);
        }
    }
    calls
}

/// Reconcile overlapping observations of one native call. A failed final status
/// vetoes; differing contents or kinds suppress the call entirely.
fn reconcile(observations: &[Call]) -> Option<Call> {
    let mut kinds: HashSet<&str> = HashSet::new();
    let mut completed = false;
    let mut failed = false;
    let mut content: Option<&Vec<Value>> = None;
    let mut times: HashSet<i64> = HashSet::new();
    for call in observations {
        if let Some(k) = call.kind.as_deref() {
            kinds.insert(k);
        }
        match call.status.as_deref() {
            Some("completed") => completed = true,
            Some("failed") => failed = true,
            _ => {}
        }
        if let Some(c) = call.content.as_ref() {
            if content.is_some_and(|other| other != c) {
                return None;
            }
            content = Some(c);
        }
        if let Some(t) = call.ts {
            times.insert(t);
        }
    }
    if kinds.len() != 1 || !kinds.contains("edit") {
        return None;
    }
    if failed || !completed {
        return None;
    }
    Some(Call {
        kind: Some("edit".into()),
        status: Some("completed".into()),
        content: content.cloned(),
        ts: (times.len() == 1).then(|| *times.iter().next().unwrap()),
    })
}

/// Preserve the session's raw PR-card repository identities for revocation.
/// Conflicts must not erase a previously observed binding to this project.
fn session_repo_observations(files: &[Vec<Value>]) -> HashMap<String, HashSet<String>> {
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for rows in files {
        for (session, update) in rows.iter().flat_map(native_rows) {
            if update["sessionUpdate"] != "agent_message_chunk" {
                continue;
            }
            let meta = &update["_meta"];
            let pr_url = || {
                meta.get(mkey("prUrl"))
                    .and_then(Value::as_str)
                    .and_then(|url| {
                        let base = url.split("/pull/").next()?.trim_end_matches('/');
                        (!base.is_empty()).then_some(base.to_owned())
                    })
            };
            let remote = match (
                meta.get(mkey("owner")).and_then(Value::as_str),
                meta.get(mkey("repo")).and_then(Value::as_str),
            ) {
                (Some(owner), Some(repo))
                    if !owner.is_empty()
                        && !repo.is_empty()
                        && owner
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        && repo
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) =>
                {
                    Some(format!("https://github.com/{owner}/{repo}"))
                }
                _ => pr_url(),
            };
            if let Some(remote) = remote {
                out.entry(session).or_default().insert(remote);
            }
        }
    }
    out
}

/// Positive relative-path evidence requires one unique repository identity.
fn session_repos(files: &[Vec<Value>]) -> HashMap<String, Option<String>> {
    session_repo_observations(files)
        .into_iter()
        .map(|(session, repos)| {
            let single = if repos.len() == 1 {
                repos.into_iter().next()
            } else {
                None
            };
            (session, single)
        })
        .collect()
}

/// Distinct user message ids per session; only sessions with mapped evidence
/// report this count.
fn session_prompts(files: &[Vec<Value>]) -> HashMap<String, usize> {
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for rows in files {
        for (session, update) in rows.iter().flat_map(native_rows) {
            if update["sessionUpdate"] == "user_message_chunk" {
                if let Some(id) = update["messageId"].as_str().filter(|s| !s.is_empty()) {
                    out.entry(session).or_default().insert(id.into());
                }
            }
        }
    }
    out.into_iter().map(|(s, ids)| (s, ids.len())).collect()
}

/// A call's model must agree in every observed segment and file. An update
/// arriving in another turn is ambiguous rather than a new model assignment.
fn turn_models(files: &[Vec<Value>]) -> HashMap<(String, String), String> {
    #[derive(Default)]
    struct Turn {
        id: Option<String>,
        models: HashSet<String>,
        calls: HashSet<String>,
    }
    type Binding = Option<(String, String)>;
    let mut observations: HashMap<(String, String), Vec<Binding>> = HashMap::new();
    let mut flush = |session: &str, turn: &Turn| {
        let binding = turn.id.as_ref().and_then(|id| {
            (turn.models.len() == 1)
                .then(|| (id.clone(), turn.models.iter().next().unwrap().clone()))
        });
        for call in &turn.calls {
            observations
                .entry((session.into(), call.clone()))
                .or_default()
                .push(binding.clone());
        }
    };
    for rows in files {
        // Interleaved sessions do not close each other's turns.
        let mut turns: HashMap<String, Turn> = HashMap::new();
        for (session, update) in rows.iter().flat_map(native_rows) {
            let turn = turns.entry(session.clone()).or_default();
            let kind = update["sessionUpdate"].as_str().unwrap_or_default();
            if kind == "user_message_chunk" {
                let id = update["messageId"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned);
                // Several chunks of one user message form one turn. Missing
                // identities cannot establish a boundary across captures.
                if id.is_none() || id != turn.id {
                    flush(&session, turn);
                    *turn = Turn {
                        id,
                        ..Turn::default()
                    };
                }
            } else if kind == "agent_message_chunk" {
                if let Some(model) = meta_str(&update, "model").filter(|m| !m.is_empty()) {
                    turn.models.insert(model.into());
                }
            } else if matches!(kind, "tool_call" | "tool_call_update") {
                if let Some(id) = update["toolCallId"].as_str().filter(|id| !id.is_empty()) {
                    turn.calls.insert(id.into());
                }
            }
        }
        for (session, turn) in turns {
            flush(&session, &turn);
        }
    }
    observations
        .into_iter()
        .filter_map(|(key, bindings)| {
            let first = bindings.first()?.as_ref()?;
            bindings
                .iter()
                .all(|binding| binding.as_ref() == Some(first))
                .then(|| (key, first.1.clone()))
        })
        .collect()
}

/// One project boundary for emitted lines and native reconciliation authority.
fn project_path(path: &str, remote: Option<&str>, pm: &PathMap) -> Option<String> {
    if Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':') {
        // Match push_scoped's lexical normalization before PathMap: remote and
        // deleted paths cannot be resolved through this machine's filesystem.
        let absolute = path.replace('\\', "/");
        let mut parts: Vec<&str> = vec![];
        for part in absolute.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.last().is_some_and(|p| !p.ends_with(':')) {
                        parts.pop();
                    }
                }
                _ => parts.push(part),
            }
        }
        let resolved = format!(
            "{}{}",
            if absolute.starts_with('/') { "/" } else { "" },
            parts.join("/")
        );
        return pm.to_rel(&resolved);
    }
    if safe_rel(path) && pm.known.contains(path) {
        if let Some(remote) = remote.filter(|remote| pm.in_project(remote)) {
            return pm
                .to_rel(&format!("{remote}/{path}"))
                .filter(|rel| rel == path);
        }
    }
    None
}

fn parse(files: &[Vec<Value>], pm: &PathMap, group: &mut FileGroup) {
    let calls = collect_calls(files);
    let repos = session_repos(files);
    let repo_observations = session_repo_observations(files);
    // Preserve project identity from every raw observation, even when later
    // field replacement, failed status or conflicting copies reject the edit.
    let updates: Vec<_> = files
        .iter()
        .flat_map(|rows| rows.iter().flat_map(native_rows))
        .filter(|(_, update)| {
            matches!(
                update["sessionUpdate"].as_str(),
                Some("tool_call" | "tool_call_update")
            )
        })
        .collect();
    let edits: HashSet<_> = updates
        .iter()
        .filter(|(_, update)| update["kind"] == "edit")
        .filter_map(|(session, update)| {
            update["toolCallId"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(|id| (session.as_str(), id))
        })
        .collect();
    let mut reconciled: Vec<_> = updates
        .iter()
        .filter(|(session, update)| {
            update["toolCallId"]
                .as_str()
                .is_some_and(|id| edits.contains(&(session.as_str(), id)))
                && update["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|diff| diff["type"] == "diff")
                    .filter_map(|diff| diff["path"].as_str())
                    .any(|path| {
                        // Absolute paths identify themselves. For relative
                        // revocation only, any raw project-bound repository
                        // survives a later conflicting PR-card observation.
                        project_path(path, None, pm).is_some()
                            || repo_observations.get(session).is_some_and(|remotes| {
                                remotes
                                    .iter()
                                    .any(|remote| project_path(path, Some(remote), pm).is_some())
                            })
                    })
        })
        .map(|(session, _)| session.clone())
        .collect();
    reconciled.sort();
    reconciled.dedup();
    group
        .reconciled_sessions
        .extend(reconciled.into_iter().map(|session| (Src::Devin, session)));
    let prompts = session_prompts(files);
    let models = turn_models(files);
    let mut used_sessions: HashSet<String> = HashSet::new();
    let mut sessions: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for ((session, id), observations) in &calls {
        let Some(call) = reconcile(observations) else {
            continue;
        };
        // Success is the native final status; a failed observation vetoes.
        let content = call.content.as_deref().unwrap_or(&[]);
        let ts = call.ts.unwrap_or(0);
        let model = models
            .get(&(session.clone(), id.clone()))
            .cloned()
            .unwrap_or_default();
        let before = group.evs.len();
        for diff in content.iter().filter(|c| c["type"] == "diff") {
            let Some(path) = diff["path"].as_str().filter(|p| !p.is_empty()) else {
                continue;
            };
            // A contents key is the authoritative body reference; the inline text
            // beside it is not independent proof. Skip referenced bodies.
            if meta_str(diff, "contentsKey").is_some()
                || meta_str(diff, "prevContentsKey").is_some()
            {
                continue;
            }
            if content
                .iter()
                .filter(|c| c["type"] == "diff" && c["path"].as_str() == Some(path))
                .count()
                != 1
            {
                continue;
            }
            let Some(new_text) = diff["newText"].as_str() else {
                continue;
            };
            let old_text = diff.get("oldText").and_then(Value::as_str).unwrap_or("");
            let added = added_lines(old_text, new_text);
            if added.is_empty() {
                continue;
            }
            if let Some(rel) = project_path(path, repos.get(session).and_then(Option::as_deref), pm)
            {
                crate::push_rel(
                    &mut group.evs,
                    &rel,
                    &added,
                    Src::Devin,
                    &model,
                    session,
                    ts,
                );
            }
        }
        if group.evs.len() > before {
            used_sessions.insert(session.clone());
            let span = sessions.entry(session.clone()).or_insert((ts, ts));
            if ts > 0 {
                span.0 = if span.0 == 0 { ts } else { span.0.min(ts) };
                span.1 = span.1.max(ts);
            }
        }
    }
    for session in used_sessions {
        let (start, end) = sessions.remove(&session).unwrap_or((0, 0));
        group.sessions.push(imp::session(
            Src::Devin,
            &session,
            "",
            start,
            end,
            prompts.get(&session).copied().unwrap_or(0) as i64,
            true,
        ));
    }
}

/// Use the exact same path, content and conflict checks for CLI precedence as
/// for the native reader; merely seeing a successful edit does not cover it.
pub(crate) fn native_evidence(root: &Path, pm: &PathMap) -> FileGroup {
    let mut group = FileGroup::default();
    parse(&read_rows(root), pm, &mut group);
    group
}

fn scan_root(root: &Path, store: &mut Store, pm: &PathMap) {
    let prefix = "devin-cloud:native-import:";
    store.groups.retain(|k, _| !k.starts_with(prefix));
    let files = read_rows(root);
    if files.iter().flatten().next().is_none() {
        return;
    }
    let mut g = FileGroup::default();
    parse(&files, pm, &mut g);
    store
        .groups
        .insert(format!("{prefix}{}", root.display()), g);
}

pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    scan_root(&source_dir(Src::Devin), store, pm);
}

pub fn discover() -> Vec<Activity> {
    let root = source_dir(Src::Devin);
    let repos = session_repos(&read_rows(&root));
    let mut seen = HashSet::new();
    let mut out = vec![];
    for remote in repos.into_values().flatten() {
        // The session's own PR messages provide the remote; a cloud session id
        // is not a filesystem cwd.
        if seen.insert(remote.clone()) {
            out.push(imp::activity(Src::Devin, String::new(), Some(remote), 0));
        }
    }
    out
}

pub fn doctor() -> Vec<Health> {
    let root = source_dir(Src::Devin);
    let files = read_rows(&root);
    let records = files
        .iter()
        .flat_map(|rows| rows.iter().flat_map(native_rows))
        .count();
    let mut h = imp::health(
        Src::Devin,
        &root,
        "Devin cloud ACP session updates (loadPage / session_update_batch)",
        records,
        &tr!(
            "离线读取网页客户端原生 ACP 更新（loadPage 响应、session/update 与 _cognition.ai/session_update_batch 批次）。只认最终状态 completed 且 kind=edit 的 diff 正文；引用 contentsKey/prevContentsKey 的正文跳过。模型要求该工具的全部观察都绑定同一 user messageId 和唯一 cognition.ai/model，缺失、冲突或跨回合保持未知；时间取工具自身 cognition.ai/timestamp，缺失保持未知。会话号即原生 sessionId；CLI hook 仅移除当前项目中已被原生证据覆盖的行。",
            "Offline native ACP updates from the web client (loadPage responses, session/update and _cognition.ai/session_update_batch batches). Only final-status completed kind=edit diff bodies count; bodies referenced by contentsKey/prevContentsKey are skipped. Model requires every observation to agree on one user messageId and cognition.ai/model; missing, conflicting or cross-turn bindings stay unknown; time is the tool's own cognition.ai/timestamp, unknown when missing. Sessions use native sessionIds; only CLI-hook line occurrences covered by actual native project evidence are deduplicated."
        ),
        false,
    );
    h.source = "devin-acp".into();
    vec![h]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pm() -> PathMap {
        PathMap {
            known: ["a.rs".into()].into_iter().collect(),
            markers: vec![
                "/home/ubuntu/repos/demo/".into(),
                "github.com/acme/demo".into(),
            ],
            excludes: vec![],
        }
    }
    fn update(kind: &str, fields: Value) -> Value {
        let mut u = json!({"sessionUpdate": kind});
        for (k, v) in fields.as_object().unwrap() {
            u[k] = v.clone();
        }
        u
    }
    fn page(session: &str, updates: Vec<Value>) -> Value {
        json!({"updates": updates.iter().map(|u| json!({"sessionId": session, "update": u})).collect::<Vec<_>>(), "coveredRange": {"start": null, "end": {"cursor": "c1", "sortKey": "1"}}, "hasMore": false, "replayFilterVersion": "v1"})
    }
    fn session_rows() -> Vec<Value> {
        vec![
            update(
                "user_message_chunk",
                json!({"content": [{"type": "text", "text": "edit it"}], "messageId": "m1"}),
            ),
            update(
                "agent_message_chunk",
                json!({"content": [{"type": "text", "text": "doing"}], "_meta": {"cognition.ai/model": "devin-2.0", "cognition.ai/timestamp": "2026-10-17T01:00:00Z"}}),
            ),
            update(
                "tool_call",
                json!({
                    "toolCallId": "call-1",
                    "title": "Edit src/a.rs",
                    "kind": "edit",
                    "status": "in_progress",
                    "_meta": {"cognition.ai/timestamp": "2026-10-17T01:00:05Z"}
                }),
            ),
            update(
                "tool_call_update",
                json!({
                    "toolCallId": "call-1",
                    "status": "completed",
                    "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "human kept\nold line\n", "newText": "human kept\nnew ai line\n"}]
                }),
            ),
        ]
    }
    fn parsed(files: Vec<Vec<Value>>) -> FileGroup {
        let mut g = FileGroup::default();
        parse(&files, &pm(), &mut g);
        g
    }
    #[test]
    fn completed_edit_diff_becomes_evidence_with_turn_model_and_own_time() {
        let g = parsed(vec![vec![page("s1", session_rows())]]);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "a.rs");
        assert_eq!(g.evs[0].model, "devin-2.0");
        assert_eq!(g.evs[0].ts, 1_792_198_805);
        assert_eq!(g.evs[0].session, "s1");
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(g.sessions[0].prompts, 1);
    }
    #[test]
    fn create_without_oldtext_counts_all_added_lines() {
        let rows = vec![
            session_rows()[0].clone(),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "newText": "brand new line\n"}]}),
            ),
        ];
        let g = parsed(vec![vec![page("s", rows)]]);
        assert_eq!(g.evs.len(), 1);
    }
    #[test]
    fn failed_read_and_unfinished_calls_never_become_evidence() {
        for (kind, status) in [
            ("edit", "failed"),
            ("edit", "in_progress"),
            ("read", "completed"),
            ("execute", "completed"),
        ] {
            let rows = vec![
                session_rows()[0].clone(),
                update(
                    "tool_call",
                    json!({"toolCallId": "c", "kind": kind, "status": status, "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "x\n", "newText": "x\nfirst long added line\n"}]}),
                ),
            ];
            assert!(
                parsed(vec![vec![page("s", rows)]]).evs.is_empty(),
                "{kind}/{status}"
            );
        }
        // A failed later observation vetoes an earlier completed one.
        let rows = vec![
            session_rows()[0].clone(),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "x\n", "newText": "x\nfirst long added line\n"}]}),
            ),
            update(
                "tool_call_update",
                json!({"toolCallId": "c", "status": "failed"}),
            ),
        ];
        assert!(parsed(vec![vec![page("s", rows)]]).evs.is_empty());
    }
    #[test]
    fn referenced_contents_keys_and_duplicate_paths_are_skipped() {
        for meta in [
            json!({"cognition.ai/contentsKey": "key-1"}),
            json!({"cognition.ai/prevContentsKey": "key-2"}),
        ] {
            let mut rows = session_rows();
            rows[3]["content"][0]["_meta"] = meta;
            assert!(parsed(vec![vec![page("s", rows)]]).evs.is_empty());
        }
        let rows = vec![
            session_rows()[0].clone(),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [
                    {"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "a\n", "newText": "a\nb\n"},
                    {"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "a\n", "newText": "a\nc\n"}
                ]}),
            ),
        ];
        assert!(parsed(vec![vec![page("s", rows)]]).evs.is_empty());
    }
    #[test]
    fn batched_and_single_notifications_and_jsonrpc_results_are_native_shapes() {
        let updates: Vec<Value> = session_rows();
        let batch = json!({"method": "ext/notification", "params": {"method": "_cognition.ai/session_update_batch", "params": {"updates": updates.iter().map(|u| json!({"sessionId": "s1", "update": u})).collect::<Vec<_>>()}}});
        let single = json!({"method": "session/update", "params": {"sessionId": "s1", "update": updates[3]}});
        let rpc = json!({"jsonrpc": "2.0", "id": 7, "result": page("s1", vec![updates[0].clone(), updates[2].clone(), updates[3].clone()])});
        // user+agent come from the batch, the completed diff from either extra shape
        let files = vec![vec![batch.clone(), single, rpc.clone()]];
        let g = parsed(files);
        assert_eq!(g.evs.len(), 1);
        // The jsonrpc result's extra copy of the same native call id merges.
        let g = parsed(vec![vec![batch, rpc]]);
        assert_eq!(g.evs.len(), 1);
    }
    #[test]
    fn repeated_imports_merge_and_conflicting_copies_suppress() {
        let good = vec![page("s1", session_rows())];
        let mut conflicting = good.clone();
        let mut rows = session_rows();
        rows[3]["content"][0]["newText"] = json!("human kept\nconflicting line\n");
        conflicting[0] = page("s1", rows);
        assert_eq!(parsed(vec![good.clone(), good.clone()]).evs.len(), 1);
        assert!(parsed(vec![good, conflicting]).evs.is_empty());
    }
    #[test]
    fn relative_paths_need_the_sessions_own_repository_binding() {
        let rows: Vec<Value> = session_rows()
            .into_iter()
            .map(|u| {
                serde_json::from_str(
                    &serde_json::to_string(&u)
                        .unwrap()
                        .replace("/home/ubuntu/repos/demo/a.rs", "a.rs"),
                )
                .unwrap()
            })
            .collect();
        assert!(parsed(vec![vec![page("s1", rows.clone())]]).evs.is_empty());
        let mut with_repo = vec![update(
            "agent_message_chunk",
            json!({"content": [{"type": "text", "text": "pr"}], "_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "demo"}}),
        )];
        with_repo.extend(rows.clone());
        let g = parsed(vec![vec![page("s1", with_repo)]]);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "a.rs");
        // A second, different repository in the same session blocks mapping.
        let mut two = vec![update(
            "agent_message_chunk",
            json!({"content": [{"type": "text", "text": "pr"}], "_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "demo"}}),
        )];
        two.push(update(
            "agent_message_chunk",
            json!({"content": [{"type": "text", "text": "pr"}], "_meta": {"cognition.ai/prUrl": "https://github.com/acme/other/pull/1"}}),
        ));
        two.extend(rows);
        assert!(parsed(vec![vec![page("s1", two)]]).evs.is_empty());
        // Traversal stays out.
        let mut bad = vec![update(
            "agent_message_chunk",
            json!({"_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "demo"}}),
        )];
        bad.push(update("tool_call", json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "../a.rs", "oldText": "x\n", "newText": "x\nfirst long added line\n"}]})));
        assert!(parsed(vec![vec![page("s1", bad)]]).evs.is_empty());
    }
    #[test]
    fn model_requires_one_model_in_the_calls_turn_segment() {
        let two_models = vec![
            session_rows()[0].clone(),
            update(
                "agent_message_chunk",
                json!({"_meta": {"cognition.ai/model": "model-a"}}),
            ),
            update(
                "agent_message_chunk",
                json!({"_meta": {"cognition.ai/model": "model-b"}}),
            ),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "x\n", "newText": "x\nfirst long added line\n"}]}),
            ),
        ];
        assert_eq!(parsed(vec![vec![page("s", two_models)]]).evs[0].model, "");
        // A model from a previous turn must not leak into the next one.
        let next_turn = vec![
            update("user_message_chunk", json!({"messageId": "m1"})),
            update(
                "agent_message_chunk",
                json!({"_meta": {"cognition.ai/model": "model-a"}}),
            ),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "x\n", "newText": "x\nfirst long added line\n"}]}),
            ),
            update("user_message_chunk", json!({"messageId": "m2"})),
            update(
                "tool_call",
                json!({"toolCallId": "c2", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "/home/ubuntu/repos/demo/a.rs", "oldText": "x\n", "newText": "x\nsecond long added line\n"}]}),
            ),
        ];
        let g = parsed(vec![vec![page("s", next_turn)]]);
        assert_eq!(g.evs[0].model, "model-a");
        assert_eq!(g.evs[1].model, "");
        assert_eq!(g.sessions[0].prompts, 2);
    }
    #[test]
    fn model_conflicts_and_incomplete_copies_stay_unknown_in_every_file_order() {
        let good = vec![page("s", session_rows())];
        let mut other = session_rows();
        other[1]["_meta"]["cognition.ai/model"] = json!("other-model");
        let conflicting = vec![page("s", other)];
        let incomplete = vec![page("s", vec![session_rows()[3].clone()])];
        for second in [conflicting, incomplete] {
            for files in [
                vec![good.clone(), second.clone()],
                vec![second, good.clone()],
            ] {
                let g = parsed(files);
                assert_eq!(g.evs.len(), 1);
                assert_eq!(
                    g.evs[0].model, "",
                    "conflicting or incomplete observations must not pick a model"
                );
            }
        }
    }
    #[test]
    fn late_update_does_not_borrow_the_following_turns_model() {
        let mut rows = session_rows();
        rows.insert(3, update("user_message_chunk", json!({"messageId": "m2"})));
        rows.insert(
            4,
            update(
                "agent_message_chunk",
                json!({"_meta": {"cognition.ai/model": "next-turn-model"}}),
            ),
        );
        let g = parsed(vec![vec![page("s", rows)]]);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].model, "");
    }
    #[test]
    fn model_without_a_known_user_turn_stays_unknown() {
        let rows = session_rows().into_iter().skip(1).collect();
        assert_eq!(parsed(vec![vec![page("s", rows)]]).evs[0].model, "");
    }
    #[test]
    fn native_session_precedence_requires_actual_project_line_evidence() {
        let root = std::env::temp_dir().join(format!(
            "creditia-devin-precedence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for case in ["outside-project", "unchanged", "duplicate-path"] {
            let mut rows = session_rows();
            match case {
                "outside-project" => rows[3]["content"][0]["path"] = json!("/unrelated/a.rs"),
                "unchanged" => {
                    rows[3]["content"][0]["newText"] = rows[3]["content"][0]["oldText"].clone()
                }
                _ => {
                    let diff = rows[3]["content"][0].clone();
                    rows[3]["content"].as_array_mut().unwrap().push(diff);
                }
            }
            std::fs::write(
                root.join("page.json"),
                serde_json::to_vec(&page("s", rows)).unwrap(),
            )
            .unwrap();
            assert!(
                native_evidence(&root, &pm()).evs.is_empty(),
                "{case} must not suppress CLI evidence"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_evidence_lists_only_verifiably_mapped_sessions() {
        let root = std::env::temp_dir().join(format!(
            "creditia-devin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("page.json"),
            serde_json::to_vec(&page("s1", session_rows())).unwrap(),
        )
        .unwrap();
        assert_eq!(
            native_evidence(&root, &pm())
                .sessions
                .iter()
                .map(|s| s.id.clone())
                .collect::<HashSet<_>>(),
            ["s1".to_owned()].into_iter().collect::<HashSet<_>>()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reconciliation_authority_requires_a_project_bound_diff_path() {
        for path in [
            Some("/unrelated/a.rs"),
            Some("a.rs"),
            Some("/home/ubuntu/repos/demo/../../other/a.rs"),
            None,
        ] {
            let mut rows = session_rows();
            if let Some(path) = path {
                rows[3]["content"][0]["path"] = json!(path);
            } else {
                rows[3]["content"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("path");
            }
            let g = parsed(vec![vec![page("s", rows)]]);
            assert!(
                g.reconciled_sessions.is_empty(),
                "unbound path {path:?} cannot revoke project history"
            );
        }
        // Known edit kind without any body/path is not project identity.
        let g = parsed(vec![vec![page(
            "s",
            vec![update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "failed"}),
            )],
        )]]);
        assert!(g.reconciled_sessions.is_empty());
    }
    #[test]
    fn conflicting_repository_cards_retract_relative_evidence_in_this_project() {
        let mut rows = session_rows();
        rows[3]["content"][0]["path"] = json!("a.rs");
        rows.push(update(
            "agent_message_chunk",
            json!({"_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "demo"}}),
        ));
        let original = parsed(vec![vec![page("s", rows.clone())]]);
        assert_eq!(original.evs.len(), 1);
        rows.push(update(
            "agent_message_chunk",
            json!({"_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "other"}}),
        ));
        let conflicting = parsed(vec![vec![page("s", rows)]]);
        assert!(conflicting.evs.is_empty());
        assert_eq!(conflicting.reconciled_sessions, vec![(Src::Devin, "s".into())], "repository conflicts revoke previous local evidence instead of reviving it from the library");
        // Repositories from another project cannot assert authority here.
        let foreign = vec![
            update(
                "agent_message_chunk",
                json!({"_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "other"}}),
            ),
            update(
                "tool_call",
                json!({"toolCallId": "c", "kind": "edit", "status": "completed", "content": [{"type": "diff", "path": "a.rs", "newText": "foreign project line\n"}]}),
            ),
        ];
        assert!(parsed(vec![vec![page("s", foreign)]])
            .reconciled_sessions
            .is_empty());
    }
    #[test]
    fn native_edit_sessions_are_reconciled_even_when_conflicts_retract_lines() {
        let good = vec![page("s", session_rows())];
        let mut bad = session_rows();
        bad[3]["status"] = json!("failed");
        let g = parsed(vec![good, vec![page("s", bad)]]);
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions, vec![(Src::Devin, "s".into())]);
        // Identity survives replacement by a later failed/out-of-project body.
        let mut replaced = session_rows();
        replaced.push(update("tool_call_update", json!({"toolCallId": "call-1", "status": "failed", "content": [{"type": "diff", "path": "/other/a.rs", "newText": "other project line\n"}]})));
        let g = parsed(vec![vec![page("s", replaced)]]);
        assert!(g.evs.is_empty());
        assert_eq!(g.reconciled_sessions, vec![(Src::Devin, "s".into())]);
        // Relative paths need the session's own repository even for a failure.
        let mut relative = session_rows();
        relative[3]["status"] = json!("failed");
        relative[3]["content"][0]["path"] = json!("a.rs");
        relative.push(update(
            "agent_message_chunk",
            json!({"_meta": {"cognition.ai/owner": "acme", "cognition.ai/repo": "demo"}}),
        ));
        assert_eq!(
            parsed(vec![vec![page("s", relative)]]).reconciled_sessions,
            vec![(Src::Devin, "s".into())]
        );
        for files in [
            vec![],
            vec![vec![json!({"not": "native"})]],
            vec![vec![page(
                "read",
                vec![update(
                    "tool_call",
                    json!({"toolCallId": "r", "kind": "read", "status": "completed"}),
                )],
            )]],
        ] {
            assert!(parsed(files).reconciled_sessions.is_empty());
        }
    }
    #[test]
    fn scan_rebuilds_the_import_set_on_changes_and_deletion() {
        let root = std::env::temp_dir().join(format!(
            "creditia-devin-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("a.json"),
            serde_json::to_vec(&page("s1", session_rows())).unwrap(),
        )
        .unwrap();
        let mut store = Store::default();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        std::fs::write(
            root.join("a.json"),
            serde_json::to_vec(&page("s1", vec![session_rows()[0].clone()])).unwrap(),
        )
        .unwrap();
        scan_root(&root, &mut store, &pm());
        assert!(store.groups.values().all(|g| g.evs.is_empty()));
        std::fs::remove_file(root.join("a.json")).unwrap();
        scan_root(&root, &mut store, &pm());
        assert!(store.groups.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
