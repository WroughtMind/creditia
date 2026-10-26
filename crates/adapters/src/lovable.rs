//! Offline original Lovable trajectory events. See SUPPORT_LOVABLE.md for the
//! verified public reducer-to-timeline-to-hunk-viewer chain and remaining limits.
use crate::trace_import as imp;
use crate::{added_lines, any_ts, source_dir, Activity, Health, Stats};
use base64::{engine::general_purpose::STANDARD, Engine};
use creditia_core::{
    model::{FileGroup, PathMap, Src, Store},
    tr,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

fn id(v: &Value) -> Option<&str> {
    v["value"].as_str().filter(|s| !s.is_empty())
}
fn event(v: &Value) -> bool {
    id(&v["id"]).is_some() && v["payload"].as_object().is_some_and(|p| p.len() == 1)
}
fn safe_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '?', '#', '\0'])
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn remote(v: &Value) -> Option<String> {
    if v["synced"] != true {
        return None;
    }
    let url = v["config"]["repo_url"]
        .as_str()?
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let (host, path) = url.strip_prefix("https://")?.split_once('/')?;
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || !safe_rel(path)
        || path.split('/').count() < 2
        || path.contains('%')
    {
        return None;
    }
    Some(url.to_owned())
}
fn har_body(e: &Value) -> Option<Value> {
    if e["request"]["method"] != "GET" || !(200..300).contains(&e["response"]["status"].as_u64()?) {
        return None;
    }
    let c = &e["response"]["content"];
    let text = c["text"].as_str()?;
    let text = match c["encoding"].as_str() {
        None => text.to_owned(),
        Some("base64") => String::from_utf8(STANDARD.decode(text).ok()?).ok()?,
        _ => return None,
    };
    serde_json::from_str(&text).ok()
}
fn view_query(url: &str) -> bool {
    let Some((_, query)) = url.split_once('?') else {
        return false;
    };
    let fields: Vec<_> = query
        .split('#')
        .next()
        .unwrap_or("")
        .split('&')
        .filter_map(|s| s.split_once('='))
        .collect();
    [
        ("chain", "view"),
        ("filter", "frontend"),
        ("inclusive", "true"),
    ]
    .iter()
    .all(|(key, value)| {
        let found: Vec<_> = fields.iter().filter(|(k, _)| k == key).collect();
        found.len() == 1 && found[0].1 == *value
    })
}
#[derive(Default)]
struct Capture {
    rows: Vec<Value>,
    // Event/project identity comes from the original trajectory request URL,
    // never from filenames or a synthetic export wrapper.
    owners: BTreeMap<String, Option<String>>,
    repos: BTreeMap<String, Option<String>>,
    // Keep original project bindings for retraction even when contradictory
    // observations make the positive mapping unusable.
    observed_owners: BTreeMap<String, HashSet<String>>,
    observed_repos: BTreeMap<String, HashSet<String>>,
}
fn observe(map: &mut BTreeMap<String, Option<String>>, key: String, value: Option<String>) {
    let old = map.entry(key).or_insert_with(|| value.clone());
    if *old != value {
        *old = None;
    }
}
fn captures(rows: &[Value]) -> Capture {
    let mut c = Capture::default();
    for row in rows {
        let Some(entries) = row["log"]["entries"].as_array() else {
            c.rows.push(row.clone());
            continue;
        };
        for e in entries {
            let Some(url) = e["request"]["url"].as_str() else {
                continue;
            };
            let Some(path) = url.strip_prefix("https://api.lovable.dev/") else {
                continue;
            };
            let path = path.split(['?', '#']).next().unwrap_or("");
            let parts: Vec<_> = path.split('/').collect();
            let Some(body) = har_body(e) else {
                continue;
            };
            match parts.as_slice() {
                ["projects", project, "trajectory"]
                    if safe_rel(project) && !project.contains('%') && view_query(url) =>
                {
                    let Some(events) = body["events"].as_array() else {
                        continue;
                    };
                    for event in events {
                        if let Some(key) = id(&event["id"]) {
                            c.observed_owners.entry(key.into()).or_default().insert((*project).into());
                            observe(&mut c.owners, key.into(), Some((*project).into()));
                        }
                    }
                    c.rows.push(body);
                }
                ["workspaces", workspace, "projects", project, "gitsync"]
                    if safe_rel(workspace) && safe_rel(project) && !project.contains('%') =>
                {
                    let repo = remote(&body);
                    if let Some(repo) = &repo {
                        c.observed_repos.entry((*project).into()).or_default().insert(repo.clone());
                    }
                    observe(&mut c.repos, (*project).into(), repo);
                }
                _ => {}
            }
        }
    }
    c
}
fn events(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .flat_map(|v| {
            if let Some(a) = v["events"].as_array() {
                a.clone()
            } else if id(&v["id"]).is_some() {
                vec![v.clone()]
            } else {
                vec![]
            }
        })
        .collect()
}
type Graph = BTreeMap<String, Value>;
fn graph(rows: &[Value]) -> (Graph, HashSet<String>) {
    let mut graph = Graph::new();
    let mut bad = HashSet::new();
    for e in events(rows) {
        let Some(key) = id(&e["id"]).map(str::to_owned) else {
            continue;
        };
        if !event(&e) {
            bad.insert(key.clone());
        }
        if graph.get(&key).is_some_and(|old| old != &e) {
            bad.insert(key.clone());
        }
        graph.insert(key, e);
    }
    (graph, bad)
}
// Parent links, never array order or timestamps, bind a call to its inference.
fn ancestors<'a>(g: &'a Graph, bad: &HashSet<String>, start: &str) -> Option<Vec<&'a Value>> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    let mut next = Some(start);
    while let Some(key) = next {
        if bad.contains(key) || !seen.insert(key) {
            return None;
        }
        let Some(e) = g.get(key) else {
            break;
        };
        if e.get("copiedFrom").is_some()
            || e["payload"].get("threadForkConfig").is_some()
            || e["payload"].get("revert").is_some()
        {
            return None;
        }
        out.push(e);
        next = id(&e["parent"]);
    }
    Some(out)
}
fn has(e: &Value, kind: &str) -> bool {
    e["payload"].get(kind).is_some()
}

// Preserve identity from every native observation, before graph conflict
// resolution discards copies. A changed path, tool or parent must not turn an
// explicit conflict into "missing history" and resurrect old library evidence.
fn observed_edit_sessions(g: &BTreeMap<&str, Vec<&Value>>, start: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut pending = vec![start];
    let mut sessions = HashSet::new();
    while let Some(key) = pending.pop() {
        if !seen.insert(key) {
            continue;
        }
        for e in g.get(key).into_iter().flatten() {
            if has(e, "iterationStart") {
                if let Some(iteration) = id(&e["id"]) {
                    sessions.insert(iteration.to_owned());
                }
            }
            if let Some(thread) = e["payload"]["threadStart"]["threadId"].as_str().filter(|s| !s.is_empty()) {
                sessions.insert(thread.to_owned());
            }
            if let Some(parent) = id(&e["parent"]) {
                pending.push(parent);
            }
        }
    }
    sessions
}

/// The actual official HunkDiffViewer and ActivityView row structure. A malformed
/// hunk invalidates the entire file, including any earlier valid additions.
fn hunk_text(v: &Value) -> Option<String> {
    let mut text = String::new();
    let (mut old_end, mut new_end) = (0u64, 0u64);
    for h in v.as_array()? {
        let o = h.as_object()?;
        if o.len() != 5 {
            return None;
        }
        let (os, oc, ns, nc) = (
            h["oldStart"].as_u64()?,
            h["oldCount"].as_u64()?,
            h["newStart"].as_u64()?,
            h["newCount"].as_u64()?,
        );
        if (oc > 0 && os == 0) || (nc > 0 && ns == 0) || os < old_end || ns < new_end {
            return None;
        }
        old_end = os.checked_add(oc)?;
        new_end = ns.checked_add(nc)?;
        let (mut old_count, mut new_count) = (0u64, 0u64);
        let (mut old, mut new) = (String::new(), String::new());
        for l in h["lines"].as_array()? {
            if l.as_object()?.len() != 2 {
                return None;
            }
            let content = l["content"].as_str()?;
            if content.contains(['\n', '\r']) {
                return None;
            }
            match l["type"].as_str()? {
                "add" => {
                    new_count += 1;
                    new.push_str(content);
                    new.push('\n');
                }
                "remove" => {
                    old_count += 1;
                    old.push_str(content);
                    old.push('\n');
                }
                "context" => {
                    old_count += 1;
                    new_count += 1;
                }
                _ => return None,
            }
        }
        if old_count != oc || new_count != nc {
            return None;
        }
        text.push_str(&added_lines(&old, &new));
    }
    Some(text)
}
fn write_paths(tool: &str, params: &Value) -> Vec<String> {
    match tool {
        "lov-write" | "write" | "lov-line-replace" | "line_replace" | "lov-search-replace"
        | "search_replace" => params["file_path"]
            .as_str()
            .map(|s| vec![s.into()])
            .unwrap_or_default(),
        "lov-apply-patch" | "apply_patch" => params["patch"]
            .as_str()
            .map(crate::apply_patch_files)
            .unwrap_or_default()
            .into_iter()
            .map(|(p, _)| p)
            .collect(),
        _ => vec![],
    }
}
fn call_info(
    g: &Graph,
    bad: &HashSet<String>,
    call_id: &str,
    end_id: &str,
) -> Option<(String, String, Value)> {
    let call_chain = ancestors(g, bad, call_id)?;
    let call = call_chain.first()?;
    let c = &call["payload"]["agentOutputToolCall"];
    let tool = c["toolId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| c["toolName"].as_str())?;
    let result_chain = ancestors(g, bad, end_id)?;
    let call_pos = result_chain
        .iter()
        .position(|e| id(&e["id"]) == Some(call_id))?;
    // A conflicting rejection/error observation must not be overruled by a copy
    // of the successful output. All imported exports share native call identity.
    for (key, e) in g {
        for kind in ["toolRejection", "toolExecutionEnd"] {
            let t = &e["payload"][kind];
            if id(&t["toolCall"]) == Some(call_id)
                && (bad.contains(key)
                    || kind == "toolRejection"
                    || t.get("output").is_none()
                    || t.get("hardError").is_some()
                    || t.get("softError").is_some())
            {
                return None;
            }
        }
    }
    let mut validated = None;
    for e in &result_chain[..call_pos] {
        let v = &e["payload"]["toolParametersValidated"];
        if id(&v["toolCall"]) == Some(call_id) {
            let p: Value = serde_json::from_str(v["params"].as_str()?).ok()?;
            if validated.as_ref().is_some_and(|old| old != &p) {
                return None;
            }
            validated = Some(p);
        }
    }
    let params = validated?;
    if write_paths(tool, &params).is_empty() {
        return None;
    }
    let iteration = call_chain.iter().find(|e| has(e, "iterationStart"))?;
    let session = call_chain
        .iter()
        .find_map(|e| {
            e["payload"]["threadStart"]["threadId"]
                .as_str()
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| id(&iteration["id"]).unwrap())
        .to_owned();
    let output_start = call_chain.iter().position(|e| has(e, "agentOutputStart"));
    let mut model = String::new();
    if let Some(start) = output_start {
        // Do not cross an inference end/start, iteration, user message or run.
        let interrupted = call_chain[1..start].iter().any(|e| {
            [
                "agentOutputEnd",
                "iterationStart",
                "iterationEnd",
                "agentStart",
                "userMessage",
            ]
            .iter()
            .any(|k| has(e, k))
        });
        let between = &result_chain[..call_pos];
        if !interrupted
            && !between.iter().any(|e| {
                [
                    "agentOutputStart",
                    "iterationStart",
                    "iterationEnd",
                    "agentStart",
                    "userMessage",
                ]
                .iter()
                .any(|k| has(e, k))
            })
        {
            let ends: Vec<_> = between
                .iter()
                .filter(|e| has(e, "agentOutputEnd"))
                .collect();
            if ends.len() == 1 {
                model = ends[0]["payload"]["agentOutputEnd"]["model"]["modelName"]
                    .as_str()
                    .unwrap_or("")
                    .into();
            }
        }
    }
    Some((session, model, params))
}
fn parse(rows: &[Value], pm: &PathMap, group: &mut FileGroup) {
    let capture = captures(rows);
    let (g, mut bad) = graph(&capture.rows);
    let observations = events(&capture.rows);
    let mut identities: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for e in observations.iter().filter(|e| event(e)) {
        identities.entry(id(&e["id"]).unwrap()).or_default().push(e);
    }
    let mut rebuilt: Vec<_> = observations.iter().filter_map(|e| {
        let key = id(&e["id"])?;
        let call = &e["payload"]["agentOutputToolCall"];
        let tool = call["toolId"].as_str().or_else(|| call["toolName"].as_str())?;
        let params: Value = serde_json::from_str(call["params"].as_str()?).ok()?;
        let scoped = write_paths(tool, &params).iter().any(|path| {
            if Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':') {
                pm.to_rel(path).is_some()
            } else {
                safe_rel(path) && pm.known.contains(path)
                    && capture.observed_owners.get(key).into_iter().flatten()
                        .filter_map(|project| capture.observed_repos.get(project)).flatten()
                        .any(|remote| pm.in_project(remote)
                            && pm.to_rel(&format!("{remote}/{path}")).as_deref() == Some(path))
            }
        });
        if !scoped {
            return None;
        }
        Some(observed_edit_sessions(&identities, key))
    }).flatten().map(|session| (Src::Lovable, session)).collect::<HashSet<_>>().into_iter().collect();
    rebuilt.sort_by(|a, b| a.1.cmp(&b.1));
    group.reconciled_sessions = rebuilt;
    bad.extend(
        capture
            .owners
            .iter()
            .filter(|(_, owner)| owner.is_none())
            .map(|(id, _)| id.clone()),
    );
    // The raw history body has no selected head. A revert can point outside a
    // paginated capture, so do not count that capture's possibly stale branch.
    if g.values().any(|e| has(e, "revert")) {
        return;
    }
    let mut results: BTreeMap<String, Vec<(&str, &Value)>> = BTreeMap::new();
    for (key, e) in &g {
        if let Some(call) = id(&e["payload"]["toolExecutionEnd"]["toolCall"]) {
            results.entry(call.into()).or_default().push((key, e));
        }
    }
    let mut sessions: BTreeMap<String, (i64, i64, HashSet<String>)> = BTreeMap::new();
    for (call, ends) in results {
        // Multiple distinct native completion events are ambiguous, even when
        // their text is identical. Repeated exports of one event are already merged.
        if ends.len() != 1 {
            continue;
        }
        let (key, e) = ends[0];
        let result = &e["payload"]["toolExecutionEnd"];
        if result.get("hardError").is_some() || result.get("softError").is_some() {
            continue;
        }
        let Some(output) = result.get("output") else {
            continue;
        };
        let Some((session, model, params)) = call_info(&g, &bad, &call, key) else {
            continue;
        };
        let call_data = &g[&call]["payload"]["agentOutputToolCall"];
        let tool = call_data["toolId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| call_data["toolName"].as_str())
            .unwrap_or("");
        let paths = write_paths(tool, &params);
        let repo = capture
            .owners
            .get(&call)
            .and_then(Option::as_deref)
            .and_then(|project| {
                let chain = ancestors(&g, &bad, key)?;
                let end = chain.iter().position(|e| id(&e["id"]) == Some(&call))?;
                if chain[..=end].iter().any(|e| {
                    capture
                        .owners
                        .get(id(&e["id"]).unwrap())
                        .and_then(Option::as_deref)
                        != Some(project)
                }) {
                    return None;
                }
                capture
                    .repos
                    .get(project)
                    .and_then(Option::as_deref)
                    .filter(|remote| pm.in_project(remote))
            });
        let ts = any_ts(&e["timestamp"]).unwrap_or(0);
        let before = group.evs.len();
        for d in output["diffs"].as_array().into_iter().flatten() {
            let Some(path) = d["filePath"].as_str() else {
                continue;
            };
            if output["diffs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|other| other["filePath"] == path)
                .count()
                != 1
                || !paths.iter().any(|p| p == path)
                || d["tooLarge"] == true
                || d["content"].get("blobRef").is_some()
            {
                continue;
            }
            let changed = output["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| {
                    c.as_object().is_some_and(|o| o.len() == 1)
                        && ["addition", "edit", "spliceEdit"]
                            .iter()
                            .any(|k| c[k]["path"] == path)
                })
                .count();
            if changed != 1 {
                continue;
            }
            let Some(inline) = d["content"]["inlineData"].as_str() else {
                continue;
            };
            let Some(hunks) = STANDARD
                .decode(inline)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            let Some(text) = hunk_text(&hunks) else {
                continue;
            };
            // Calls are reconciled by native identity above; the emitted rows
            // retain their native thread/iteration session association.
            if Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':') {
                imp::push_scoped(
                    group,
                    pm,
                    "",
                    path,
                    &text,
                    Src::Lovable,
                    &model,
                    &session,
                    ts,
                );
            } else if let Some(repo) = repo.filter(|_| safe_rel(path) && pm.known.contains(path)) {
                // This is a repository-relative path, not a guessed local cwd.
                // to_rel additionally applies the project's explicit excludes.
                if pm.to_rel(&format!("{repo}/{path}")).as_deref() == Some(path) {
                    crate::push_rel(
                        &mut group.evs,
                        path,
                        &text,
                        Src::Lovable,
                        &model,
                        &session,
                        ts,
                    );
                }
            }
        }
        if group.evs.len() > before {
            let s = sessions.entry(session).or_insert((ts, ts, HashSet::new()));
            if ts > 0 {
                if s.0 == 0 {
                    s.0 = ts;
                }
                s.0 = s.0.min(ts);
                s.1 = s.1.max(ts);
            }
            s.2.insert(model);
        }
    }
    for (id, (start, end, models)) in sessions {
        let model = if models.len() == 1 {
            models.into_iter().next().unwrap()
        } else {
            String::new()
        };
        group
            .sessions
            .push(imp::session(Src::Lovable, &id, &model, start, end, 0, true));
    }
}
pub fn scan(store: &mut Store, pm: &PathMap, _stats: &mut Stats) {
    scan_root(&source_dir(Src::Lovable), store, pm);
}
fn scan_root(root: &Path, store: &mut Store, pm: &PathMap) {
    let prefix = "lovable:native-import:";
    store.groups.retain(|key, _| !key.starts_with(prefix));
    let paths = imp::files(root);
    if paths.is_empty() {
        return;
    }
    let rows: Vec<_> = paths.iter().flat_map(|p| imp::read(p)).collect();
    let mut g = FileGroup::default();
    parse(&rows, pm, &mut g);
    store
        .groups
        .insert(format!("{prefix}{}", root.display()), g);
}
pub fn discover() -> Vec<Activity> {
    let rows: Vec<_> = imp::files(&source_dir(Src::Lovable))
        .iter()
        .flat_map(|p| imp::read(p))
        .collect();
    let capture = captures(&rows);
    let mut seen = HashSet::new();
    capture
        .repos
        .into_iter()
        .filter_map(|(project, repo)| {
            let repo = repo?;
            (capture
                .owners
                .values()
                .any(|p| p.as_deref() == Some(&project))
                && seen.insert(repo.clone()))
            .then(|| imp::activity(Src::Lovable, String::new(), Some(repo), 0))
        })
        .collect()
}
pub fn doctor() -> Vec<Health> {
    let root = source_dir(Src::Lovable);
    let rows: Vec<_> = imp::files(&root)
        .iter()
        .flat_map(|p| imp::read(p))
        .collect();
    let records = events(&captures(&rows).rows)
        .iter()
        .filter(|v| event(v))
        .count();
    vec![imp::health(Src::Lovable,&root,"Lovable trajectory EventWrapper / original HAR",records,
        &tr!("离线读取 trajectory 成功写入与官方 Hunk[]。相对路径须由原始 HAR 中同项目的 trajectory 与已同步 gitsync.repo_url 绑定本仓库；无映射仅接受绝对路径。模型由父链推理区间确认；未知正文、blob、回退/分叉不猜测。公开客户端链路已核实，尚无真实账户验收。", "Reads successful trajectory writes and official Hunk[] offline. Relative paths require same-project trajectory and synced gitsync.repo_url in an original HAR, matched to this repository; otherwise absolute paths only. Model requires a parent-linked inference. Unknown bodies, blobs and revert/fork are not inferred. Public-client chain verified; no real-account acceptance capture yet."),false)]
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
    fn ev(id: &str, parent: &str, payload: Value) -> Value {
        json!({"id":{"value":id},"parent":{"value":parent},"timestamp":"2026-10-17T00:00:02Z","payload":payload})
    }
    fn hunks() -> Value {
        json!([{"oldStart":1,"oldCount":2,"newStart":1,"newCount":2,"lines":[{"type":"context","content":"unchanged"},{"type":"remove","content":"old_line"},{"type":"add","content":"new_line"}]}])
    }
    fn trace() -> Vec<Value> {
        vec![
            ev("iteration", "", json!({"iterationStart":{}})),
            ev(
                "inference",
                "iteration",
                json!({"agentOutputStart":{"model":{"modelName":"selection_is_not_actual"}}}),
            ),
            ev(
                "call",
                "inference",
                json!({"agentOutputToolCall":{"toolId":"lov-write","toolUseId":"provider-call","params":"{\"file_path\":\"/demo/a.rs\"}"}}),
            ),
            ev(
                "model",
                "call",
                json!({"agentOutputEnd":{"model":{"modelName":"actual_model"}}}),
            ),
            ev(
                "validated",
                "model",
                json!({"toolParametersValidated":{"toolCall":{"value":"call"},"params":"{\"file_path\":\"/demo/a.rs\"}"}}),
            ),
            ev(
                "start",
                "validated",
                json!({"toolExecutionStart":{"toolCall":{"value":"call"}}}),
            ),
            ev(
                "end",
                "start",
                json!({"toolExecutionEnd":{"toolCall":{"value":"call"},"output":{"changes":[{"edit":{"path":"/demo/a.rs"}}],"diffs":[{"filePath":"/demo/a.rs","content":{"inlineData":STANDARD.encode(serde_json::to_vec(&hunks()).unwrap())}}]}}}),
            ),
        ]
    }
    fn parsed(rows: Vec<Value>) -> FileGroup {
        let mut g = FileGroup::default();
        parse(&rows, &pm(), &mut g);
        g
    }
    #[test]
    fn known_official_hunk_shape_and_parent_bound_model() {
        let g = parsed(vec![json!({"events":trace()})]);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.sessions[0].model, "actual_model");
        assert_eq!(g.evs[0].model, "actual_model");
        assert_eq!(g.evs[0].session, "iteration");
    }
    #[test]
    fn conflicting_observations_keep_the_original_session_for_reconciliation() {
        for kind in ["path", "tool", "parent"] {
            let mut rows = trace();
            let mut conflict = rows[if kind == "parent" { 1 } else { 2 }].clone();
            match kind {
                "path" => conflict["payload"]["agentOutputToolCall"]["params"] = json!("{\"file_path\":\"/unrelated/a.rs\"}"),
                "tool" => conflict["payload"]["agentOutputToolCall"]["toolId"] = json!("read"),
                _ => conflict["parent"] = json!({"value":"different-missing-root"}),
            }
            rows.push(conflict);
            let g = parsed(rows);
            assert!(g.evs.is_empty(), "conflicting {kind} cannot certify edits");
            assert!(g.reconciled_sessions.contains(&(Src::Lovable, "iteration".into())), "original {kind} observation must revoke stored evidence");
        }
    }
    #[test]
    fn hunk_structure_counts_coordinates_and_multiline_are_checked() {
        for bad in [
            json!("@@ -1 +1 @@\n+not_a_hunk"),
            json!([{"lines":[{"added":true,"content":"invented"}]}]),
        ] {
            assert!(hunk_text(&bad).is_none());
        }
        let mut h = hunks();
        h[0]["newCount"] = json!(3);
        assert!(hunk_text(&h).is_none());
        let mut h = hunks();
        h[0]["lines"][2]["content"] = json!("one\ntwo");
        assert!(hunk_text(&h).is_none());
        let mut h = hunks();
        h.as_array_mut().unwrap().push(hunks()[0].clone());
        assert!(hunk_text(&h).is_none());
    }
    #[test]
    fn requests_pending_errors_preview_and_mismatched_changes_are_rejected() {
        for kind in ["hardError", "softError"] {
            let mut t = trace();
            t[6]["payload"]["toolExecutionEnd"]
                .as_object_mut()
                .unwrap()
                .remove("output");
            t[6]["payload"]["toolExecutionEnd"][kind] = json!({});
            assert!(parsed(t).evs.is_empty());
        }
        let mut t = trace();
        t.pop();
        assert!(parsed(t).evs.is_empty());
        for tool in ["lov-view", "propose_edit", "preview"] {
            let mut t = trace();
            t[2]["payload"]["agentOutputToolCall"]["toolId"] = json!(tool);
            assert!(parsed(t).evs.is_empty());
        }
        let mut t = trace();
        t[6]["payload"]["toolExecutionEnd"]["output"]["changes"][0]["edit"]["path"] =
            json!("/elsewhere/a.rs");
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[6]["payload"]["toolExecutionEnd"]["output"]
            .as_object_mut()
            .unwrap()
            .remove("changes");
        assert!(parsed(t).evs.is_empty());
    }
    #[test]
    fn model_missing_or_other_inference_is_unknown() {
        let mut t = trace();
        t[3]["payload"]["agentOutputEnd"] = json!({});
        assert_eq!(parsed(t).evs[0].model, "");
        let mut t = trace();
        t[3]["payload"] = json!({"agentOutputStart":{"model":{"modelName":"other_model"}}});
        assert_eq!(parsed(t).evs[0].model, "");
        let mut t = trace();
        t[6].as_object_mut().unwrap().remove("timestamp");
        assert_eq!(parsed(t).evs[0].ts, 0);
    }
    #[test]
    fn repeated_exports_deduplicate_native_ids_but_distinct_calls_remain() {
        let mut t = trace();
        t.extend(trace());
        assert_eq!(parsed(t).evs.len(), 1);
        let mut t = trace();
        let other: Vec<Value> = trace()
            .into_iter()
            .map(|mut e| {
                for pointer in [
                    "/id/value",
                    "/parent/value",
                    "/payload/toolExecutionEnd/toolCall/value",
                    "/payload/toolExecutionStart/toolCall/value",
                    "/payload/toolParametersValidated/toolCall/value",
                ] {
                    if let Some(v) = e.pointer_mut(pointer) {
                        if let Some(s) = v.as_str().filter(|s| !s.is_empty() && *s != "iteration") {
                            *v = json!(format!("{s}2"));
                        }
                    }
                }
                e
            })
            .collect();
        t.extend(other);
        assert_eq!(parsed(t).evs.len(), 2);
    }
    #[test]
    fn conflicting_duplicate_rejection_fork_revert_and_cycles_veto() {
        let mut t = trace();
        let mut e = t[6].clone();
        e["timestamp"] = json!("2026-10-18T00:00:00Z");
        t.push(e);
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t.push(ev(
            "rejected",
            "call",
            json!({"toolRejection":{"toolCall":{"value":"call"}}}),
        ));
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[0]["payload"] = json!({"revert":{"from":{"value":"earlier"}}});
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[2]["copiedFrom"] = json!({"value":"another-call"});
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[0]["parent"] = json!({"value":"call"});
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        let mut bad = t[6].clone();
        bad["payload"]["noop"] = json!({});
        t.push(bad);
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t.push(ev(
            "later-revert",
            "iteration",
            json!({"revert":{"oldAgentHead":{"value":"end"}}}),
        ));
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[0]["payload"] = json!({"threadForkConfig":{"forkPoint":{"value":"earlier"}}});
        assert!(parsed(t).evs.is_empty());
    }
    #[test]
    fn unknown_wholefile_blob_and_relative_body_are_not_line_evidence() {
        let mut t = trace();
        t[6]["payload"]["toolExecutionEnd"]["output"]["diffs"][0]["content"] =
            json!({"inlineData":STANDARD.encode("whole file body")});
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        t[6]["payload"]["toolExecutionEnd"]["output"]["diffs"][0]["content"] =
            json!({"blobRef":"some-blob"});
        assert!(parsed(t).evs.is_empty());
        let t = trace()
            .into_iter()
            .map(|e| {
                serde_json::from_str(
                    &serde_json::to_string(&e)
                        .unwrap()
                        .replace("/demo/a.rs", "a.rs"),
                )
                .unwrap()
            })
            .collect();
        assert!(parsed(t).evs.is_empty());
    }
    #[test]
    fn ordering_pages_and_second_completion_do_not_change_identity() {
        let mut t = trace();
        t.reverse();
        assert_eq!(parsed(t).evs.len(), 1);
        let t = trace();
        assert_eq!(
            parsed(vec![json!({"events":&t[4..]}), json!({"events":&t[..4]})])
                .evs
                .len(),
            1
        );
        let mut t = trace();
        let mut second = t[6].clone();
        second["id"]["value"] = json!("second-end");
        t.push(second);
        assert!(parsed(t).evs.is_empty());
        let mut t = trace();
        let duplicate = t[6]["payload"]["toolExecutionEnd"]["output"]["diffs"][0].clone();
        t[6]["payload"]["toolExecutionEnd"]["output"]["diffs"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(parsed(t).evs.is_empty());
    }
    #[test]
    fn import_rebuild_removes_duplicates_and_conflicts() {
        let root = std::env::temp_dir().join(format!(
            "creditia-lovable-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("first.json");
        let copy = root.join("copy.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({"events":trace()})).unwrap(),
        )
        .unwrap();
        std::fs::copy(&path, &copy).unwrap();
        let mut store = Store::default();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        let mut bad = trace();
        bad[6]["timestamp"] = json!("2026-10-18T00:00:00Z");
        std::fs::write(&copy, serde_json::to_vec(&json!({"events":bad})).unwrap()).unwrap();
        scan_root(&root, &mut store, &pm());
        assert!(store.groups.values().all(|g| g.evs.is_empty()));
        std::fs::remove_file(&copy).unwrap();
        scan_root(&root, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        std::fs::remove_file(&path).unwrap();
        scan_root(&root, &mut store, &pm());
        assert!(store.groups.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn remove_add_reemission_is_not_new_authorship() {
        let h = json!([{"oldStart":1,"oldCount":1,"newStart":1,"newCount":2,"lines":[{"type":"remove","content":"human_original_line"},{"type":"add","content":"human_original_line"},{"type":"add","content":"actual_added"}]}]);
        assert_eq!(hunk_text(&h).as_deref(), Some("actual_added\n"));
        let mut t = trace();
        t[6]["payload"]["toolExecutionEnd"]["output"]["diffs"][0]["content"]["inlineData"] =
            json!(STANDARD.encode(serde_json::to_vec(&h).unwrap()));
        assert_eq!(parsed(t).evs.len(), 1);
    }
    fn har_entry(path: &str, body: Value) -> Value {
        json!({"request":{"method":"GET","url":format!("https://api.lovable.dev/{path}")},"response":{"status":200,"content":{"text":serde_json::to_string(&body).unwrap()}}})
    }
    fn relative_har() -> Value {
        let t: Vec<Value> = trace()
            .into_iter()
            .map(|e| {
                serde_json::from_str(
                    &serde_json::to_string(&e)
                        .unwrap()
                        .replace("/demo/a.rs", "a.rs"),
                )
                .unwrap()
            })
            .collect();
        json!({"log":{"version":"1.2","entries":[har_entry("projects/p/trajectory?chain=view&filter=frontend&inclusive=true&head_id=head",json!({"events":t})),har_entry("workspaces/w/projects/p/gitsync",json!({"synced":true,"config":{"repo_url":"https://github.com/acme/demo","owner_name":"acme","repo_name":"demo","provider_type":"github"}}))]}})
    }
    #[test]
    fn original_har_binds_relative_paths_to_the_matching_synced_repository() {
        let g = parsed(vec![relative_har()]);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.evs[0].rel, "a.rs");
        assert_eq!(g.evs[0].model, "actual_model");
        let mut h = relative_har();
        let s = h["log"]["entries"][0]["response"]["content"]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        h["log"]["entries"][0]["response"]["content"]["encoding"] = json!("base64");
        h["log"]["entries"][0]["response"]["content"]["text"] = json!(STANDARD.encode(s));
        assert_eq!(parsed(vec![h]).evs.len(), 1);
        let h=relative_har();let entries=h["log"]["entries"].as_array().unwrap();
        assert_eq!(parsed(vec![json!({"log":{"entries":[entries[1]]}}),json!({"log":{"entries":[entries[0]]}})]).evs.len(),1);
        let mut excluded=pm();excluded.excludes.push("/a.rs/".into());
        let mut g=FileGroup::default();parse(&[relative_har()],&excluded,&mut g);assert!(g.evs.is_empty());
    }
    #[test]
    fn conflicting_har_bindings_still_reconcile_the_observed_project() {
        for kind in ["repository", "owner", "unsynced"] {
            let mut h = relative_har();
            let entries = h["log"]["entries"].as_array_mut().unwrap();
            let conflict = match kind {
                "owner" => {
                    let mut entry = entries[0].clone();
                    entry["request"]["url"] = json!("https://api.lovable.dev/projects/other/trajectory?chain=view&filter=frontend&inclusive=true");
                    entry
                }
                "unsynced" => har_entry("workspaces/w/projects/p/gitsync", json!({"synced":false})),
                _ => har_entry("workspaces/w/projects/p/gitsync", json!({"synced":true,"config":{"repo_url":"https://github.com/acme/other"}})),
            };
            entries.push(conflict);
            let g = parsed(vec![h]);
            assert!(g.evs.is_empty(), "conflicting {kind} is not positive evidence");
            assert!(g.reconciled_sessions.contains(&(Src::Lovable, "iteration".into())), "the observed project must still retract its old evidence after {kind} conflicts");
        }
        let mut foreign = relative_har();
        foreign["log"]["entries"][1] = har_entry("workspaces/w/projects/p/gitsync", json!({"synced":true,"config":{"repo_url":"https://github.com/acme/other"}}));
        assert!(parsed(vec![foreign]).reconciled_sessions.is_empty(), "an unrelated repository cannot retract this project's evidence");
    }
    #[test]
    fn relative_paths_require_exact_original_project_remote_and_view_context() {
        for (pointer,value) in [
            ("/log/entries/1/request/url",json!("https://api.lovable.dev/workspaces/w/projects/another/gitsync")),
            ("/log/entries/0/request/url",json!("https://foreign.example/projects/p/trajectory?chain=view&filter=frontend&inclusive=true")),
            ("/log/entries/0/request/url",json!("https://api.lovable.dev/projects/p/trajectory?chain=agent&filter=frontend&inclusive=true")),
            ("/log/entries/0/response/status",json!(403)),
        ] {let mut h=relative_har();*h.pointer_mut(pointer).unwrap()=value;assert!(parsed(vec![h]).evs.is_empty());}
        for body in [
            json!({"synced":false,"config":{"repo_url":"https://github.com/acme/demo"}}),
            json!({"synced":true,"config":{"repo_url":"https://github.com/acme/other"}}),
        ] {
            let mut h = relative_har();
            h["log"]["entries"][1] = har_entry("workspaces/w/projects/p/gitsync", body);
            assert!(parsed(vec![h]).evs.is_empty());
        }
        let mut h = relative_har();
        h["log"]["entries"].as_array_mut().unwrap().pop();
        assert!(parsed(vec![h]).evs.is_empty());
        let mut h = relative_har();
        h["log"]["entries"].as_array_mut().unwrap().push(har_entry(
            "workspaces/w/projects/p/gitsync",
            json!({"synced":true,"config":{"repo_url":"https://github.com/acme/other"}}),
        ));
        assert!(parsed(vec![h]).evs.is_empty());
        assert!(!safe_rel("../a.rs"));
        assert!(!safe_rel("sub/../a.rs"));
        assert!(!safe_rel("/a.rs"));
        let h = serde_json::from_str(
            &serde_json::to_string(&relative_har())
                .unwrap()
                .replace("a.rs", "sub/a.rs"),
        )
        .unwrap();
        assert!(parsed(vec![h]).evs.is_empty());
    }
    #[test]
    fn original_har_extension_is_scanned_as_a_real_import_file() {
        let root=std::env::temp_dir().join(format!("creditia-lovable-har-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let path=root.join("network.har");std::fs::write(&path,serde_json::to_vec(&relative_har()).unwrap()).unwrap();
        let mut store=Store::default();scan_root(&root,&mut store,&pm());
        assert_eq!(store.groups.values().map(|g|g.evs.len()).sum::<usize>(),1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
