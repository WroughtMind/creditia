//! Offline Bolt HAR reader. A saved server merge and exact before/after bodies,
//! rather than the chat's reloaded `complete` status, establish a write.
//! See SUPPORT_BOLT.md for the public shipping-client contract and limitations.
use crate::trace_import as imp;
use crate::{added_lines, source_dir, Activity, Health, Stats};
use base64::{engine::general_purpose::STANDARD, Engine};
use creditia_core::{
    model::{FileGroup, PathMap, Src, Store},
    tr,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

type FileKey = (String, String, String); // project, immutable commit SHA, API-relative path
#[derive(Default)]
struct Capture {
    bodies: HashMap<FileKey, Option<String>>,
    receipts: BTreeMap<(String, String), Vec<(String, Value)>>,
    calls: BTreeMap<(String, String), Vec<(String, Value)>>,
    prompts: HashMap<String, HashSet<String>>,
    conflicting_calls: HashSet<(String, String)>,
    turn_bases: HashMap<(String, String), Option<String>>,
    call_users: HashMap<(String, String), HashSet<String>>,
}
fn text_body(e: &Value) -> Option<String> {
    let c = &e["response"]["content"];
    let s = c["text"].as_str()?;
    match c["encoding"].as_str() {
        None => Some(s.into()),
        Some("base64") => String::from_utf8(STANDARD.decode(s).ok()?).ok(),
        _ => None,
    }
}
fn json_body(e: &Value) -> Option<Value> {
    serde_json::from_str(&text_body(e)?).ok()
}
fn endpoint(e: &Value) -> Option<Vec<String>> {
    // Do not treat an arbitrary website's similarly named paths as Bolt traffic.
    let url = e["request"]["url"].as_str()?;
    let path = url
        .strip_prefix("https://bolt.new/")?
        .split(['?', '#'])
        .next()?;
    path.split('/').map(decode_segment).collect()
}
fn decode_segment(s: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let b = s.as_bytes()[i];
        if b == b'%' {
            bytes.push(u8::from_str_radix(s.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            bytes.push(b);
            i += 1;
        }
    }
    String::from_utf8(bytes).ok()
}
fn safe_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn tool_rel(path: &str) -> Option<&str> {
    // Exact roots handled by the current client's toProjectRelativePath. These
    // identify API paths only; they NEVER bypass PathMap's project membership.
    if let Some(rel) = path.strip_prefix("/home/project/") {
        return safe_rel(rel).then_some(rel);
    }
    for prefix in ["/tmp/cc-agent/", "/workspace/bolt-code/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let (agent, rel) = rest.split_once("/project/")?;
            return (safe_rel(agent) && !agent.contains('/') && safe_rel(rel)).then_some(rel);
        }
    }
    None
}
fn observe_body(c: &mut Capture, key: FileKey, body: String) {
    let previous = c.bodies.entry(key).or_insert_with(|| Some(body.clone()));
    if previous.as_ref() != Some(&body) {
        *previous = None;
    }
}
fn entries(v: &Value) -> impl Iterator<Item = &Value> {
    v["log"]["entries"].as_array().into_iter().flatten()
}
fn collect(rows: &[Value]) -> Capture {
    let mut c = Capture::default();
    for e in rows.iter().flat_map(entries) {
        let Some(parts) = endpoint(e) else { continue };
        let p: Vec<_> = parts.iter().map(String::as_str).collect();
        let status = e["response"]["status"].as_u64().unwrap_or(0);
        if !(200..300).contains(&status) {
            continue;
        }
        match p.as_slice() {
            ["api", "chat"] if e["request"]["method"] == "POST" => {
                let Some(request) = e["request"]["postData"]["text"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                else {
                    continue;
                };
                let (Some(project), Some(base), Some(user)) = (
                    request["projectId"].as_str(),
                    request["codeStorageRepo"]["sha"].as_str(),
                    request["messages"]
                        .as_array()
                        .and_then(|a| a.last())
                        .filter(|m| m["role"] == "user")
                        .and_then(|m| m["id"].as_str()),
                ) else {
                    continue;
                };
                if project.is_empty() || base.is_empty() || user.is_empty() {
                    continue;
                }
                let previous = c
                    .turn_bases
                    .entry((project.into(), user.into()))
                    .or_insert_with(|| Some(base.into()));
                if previous.as_deref() != Some(base) {
                    *previous = None;
                }
            }

            ["api", "chats", project] if e["request"]["method"] == "GET" => {
                let Some(body) = json_body(e) else { continue };
                for message in body["messages"].as_array().into_iter().flatten() {
                    let Some(id) = message["id"].as_str().filter(|s| !s.is_empty()) else {
                        continue;
                    };
                    if message["role"] == "user" {
                        c.prompts
                            .entry((*project).into())
                            .or_default()
                            .insert(id.into());
                        continue;
                    }
                    if message["role"] != "assistant" {
                        continue;
                    }
                    let merges: HashSet<_> = message["annotations"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|v| v["type"] == "bolt-merged")
                        .filter_map(|v| v["mergedSha"].as_str().filter(|s| !s.is_empty()))
                        .collect();
                    // A contradictory/missing merge cannot certify tool proposals.
                    let sha = if merges.len() == 1 {
                        *merges.iter().next().unwrap()
                    } else {
                        ""
                    };
                    let users: HashSet<_> = message["annotations"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|a| a["type"] == "metadata")
                        .filter_map(|a| a["userMessageId"].as_str().filter(|s| !s.is_empty()))
                        .collect();
                    for call in message["parts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|p| p["type"] == "tool-invocation")
                        .map(|p| &p["toolInvocation"])
                        .chain(message["toolInvocations"].as_array().into_iter().flatten())
                    {
                        let Some(call_id) = call["toolCallId"].as_str().filter(|s| !s.is_empty())
                        else {
                            continue;
                        };
                        c.call_users
                            .entry(((*project).into(), call_id.into()))
                            .or_default()
                            .extend(users.iter().map(|s| s.to_string()));
                        if merges.len() > 1 || users.len() > 1 {
                            c.conflicting_calls
                                .insert(((*project).into(), call_id.into()));
                        }
                        c.calls
                            .entry(((*project).into(), call_id.into()))
                            .or_default()
                            .push((sha.into(), call.clone()));
                    }
                }
            }
            ["api", "projects", project, "apply-changes"] if e["request"]["method"] == "POST" => {
                let Some(request) = e["request"]["postData"]["text"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                else {
                    continue;
                };
                let (Some(base), Some(sha), Some(body)) = (
                    request["baseSha"].as_str(),
                    request["sha"].as_str(),
                    json_body(e),
                ) else {
                    continue;
                };
                if base.is_empty()
                    || sha.is_empty()
                    || body["sha"] != sha
                    || !body["files"].is_array()
                {
                    continue;
                }
                for file in body["files"].as_array().into_iter().flatten() {
                    if let (Some(path), Some(content)) =
                        (file["path"].as_str(), file["content"].as_str())
                    {
                        if safe_rel(path)
                            && file["isBinary"] != true
                            && !matches!(file["state"].as_str(), Some("deleted" | "unmerged"))
                        {
                            observe_body(
                                &mut c,
                                ((*project).into(), sha.into(), path.into()),
                                content.into(),
                            );
                        }
                    }
                }
                c.receipts
                    .entry(((*project).into(), sha.into()))
                    .or_default()
                    .push((base.into(), body));
            }
            ["api", "projects", project, "files", sha, path @ ..]
                if e["request"]["method"] == "GET" =>
            {
                // fileUrl applies routerEncode (~ -> ~0, . -> ~1) before URL encoding.
                let path = path.join("/").replace("~1", ".").replace("~0", "~");
                let mime = e["response"]["content"]["mimeType"]
                    .as_str()
                    .unwrap_or("")
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim();
                // This endpoint serves bytes, not a JSON content envelope. Avoid
                // decoding binary captures as source text merely because UTF-8 works.
                if !safe_rel(&path)
                    || !(mime.starts_with("text/")
                        || matches!(
                            mime,
                            "application/json"
                                | "application/javascript"
                                | "application/typescript"
                        ))
                {
                    continue;
                }
                if let Some(body) = text_body(e) {
                    observe_body(&mut c, ((*project).into(), (*sha).into(), path), body);
                }
            }
            _ => {}
        }
    }
    c
}
fn failed(call: &Value) -> bool {
    let r = &call["result"];
    r["type"] == "error"
        || r["isError"] == true
        || r["is_error"] == true
        || r["success"] == false
        || r["ok"] == false
        || matches!(r["status"].as_str(), Some("error" | "failed" | "cancelled"))
        || call["error"].is_object()
        || call["error"].is_string()
}
fn replace(before: &str, edit: &Value) -> Option<String> {
    let (old, new) = (edit["old_string"].as_str()?, edit["new_string"].as_str()?);
    let count = before.matches(old).count();
    if old.is_empty() || count == 0 || count > 1 && edit["replace_all"] != true {
        return None;
    }
    Some(if edit["replace_all"] == true {
        before.replace(old, new)
    } else {
        before.replacen(old, new, 1)
    })
}
fn proposed_after(before: &str, call: &Value) -> Option<String> {
    let args = &call["args"];
    match call["toolName"].as_str()? {
        "Write" => args["content"]
            .as_str()
            .or_else(|| args["file_text"].as_str())
            .map(str::to_owned),
        "Edit" => replace(before, args),
        "MultiEdit" => {
            let edits = args["edits"].as_array().filter(|a| !a.is_empty())?;
            edits
                .iter()
                .try_fold(before.to_owned(), |body, e| replace(&body, e))
        }
        _ => None,
    }
}
fn parse(rows: &[Value], pm: &PathMap, g: &mut FileGroup) {
    let c = collect(rows);
    let mut rebuilt: Vec<_> = c.calls.iter()
        .filter(|(_, observations)| observations.iter().any(|(_, call)| {
            matches!(call["toolName"].as_str(), Some("Write" | "Edit" | "MultiEdit"))
                && call["args"]["file_path"].as_str().or_else(|| call["args"]["path"].as_str())
                    .is_some_and(|path| pm.to_rel(path).is_some())
        }))
        .map(|((project, _), _)| (Src::Bolt, project.clone()))
        .collect::<HashSet<_>>().into_iter().collect();
    rebuilt.sort_by(|a, b| a.1.cmp(&b.1));
    g.reconciled_sessions = rebuilt;
    let mut used = HashSet::new();
    let mut emitted = HashSet::new();
    for ((project, call_id), observations) in &c.calls {
        if c.conflicting_calls
            .contains(&(project.clone(), call_id.clone()))
        {
            continue;
        }
        let Some(users) = c
            .call_users
            .get(&(project.clone(), call_id.clone()))
            .filter(|s| s.len() == 1)
        else {
            continue;
        };
        let user = users.iter().next().unwrap();
        let Some(Some(turn_base)) = c.turn_bases.get(&(project.clone(), user.clone())) else {
            continue;
        };
        let Some((sha, call)) = observations.iter().find(|(sha, call)| {
            !sha.is_empty() && call["state"] == "result" && call.get("result").is_some()
        }) else {
            continue;
        };
        // Repeated exports/parts are observations of the same native tool ID.
        // Failed, altered or differently committed copies cannot inflate evidence.
        if observations.iter().any(|(other_sha, other)| {
            failed(other)
                || (!other_sha.is_empty() && other_sha != sha)
                || other["toolName"] != call["toolName"]
                || other["args"] != call["args"]
        }) {
            continue;
        }
        let Some(path) = call["args"]["file_path"]
            .as_str()
            .or_else(|| call["args"]["path"].as_str())
        else {
            continue;
        };
        let Some(rel) = tool_rel(path) else { continue };
        let Some(Some(after)) = c.bodies.get(&(project.clone(), sha.clone(), rel.into())) else {
            continue;
        };
        let Some(receipts) = c.receipts.get(&(project.clone(), sha.clone())) else {
            continue;
        };
        let mut evidence = None;
        let mut conflict = false;
        for (base, receipt) in receipts {
            if base != turn_base {
                continue;
            }
            let candidates: Vec<_> = receipt["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["path"] == rel)
                .collect();
            if candidates.len() != 1 {
                continue;
            }
            let file = candidates[0];
            if file["isBinary"] == true
                || matches!(
                    file["state"].as_str(),
                    Some("deleted" | "unmerged" | "renamed")
                )
                || file["content"] != after.as_str()
            {
                continue;
            }
            let Some(Some(before)) = c.bodies.get(&(project.clone(), base.clone(), rel.into()))
            else {
                continue;
            };
            if proposed_after(before, call).as_ref() != Some(after) {
                continue;
            }
            let added = added_lines(before, after);
            if evidence.as_ref().is_some_and(|previous| previous != &added) {
                conflict = true;
            }
            evidence = Some(added);
        }
        if conflict {
            continue;
        }
        let Some(added) = evidence.filter(|s| !s.is_empty()) else {
            continue;
        };
        if !emitted.insert((project.clone(), sha.clone(), rel.to_owned())) {
            continue;
        }
        let prior = g.evs.len();
        imp::push_scoped(g, pm, "", path, &added, Src::Bolt, "", project, 0);
        if g.evs.len() > prior {
            used.insert(project.clone());
        }
    }
    for project in used {
        g.sessions.push(imp::session(
            Src::Bolt,
            &project,
            "",
            0,
            0,
            c.prompts.get(&project).map_or(0, |s| s.len() as i64),
            true,
        ));
    }
}
fn scan_root(root: &Path, store: &mut Store, pm: &PathMap) {
    let prefix = "bolt:native-import:";
    store.groups.retain(|k, _| !k.starts_with(prefix));
    let rows: Vec<_> = imp::files(root).iter().flat_map(|p| imp::read(p)).collect();
    if rows.is_empty() {
        return;
    }
    let mut g = FileGroup::default();
    parse(&rows, pm, &mut g);
    store
        .groups
        .insert(format!("{prefix}{}", root.display()), g);
}
pub fn scan(store: &mut Store, pm: &PathMap, _st: &mut Stats) {
    scan_root(&source_dir(Src::Bolt), store, pm);
}
// A cloud project ID and a file parent do not establish a filesystem cwd.
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Vec<Health> {
    let root = source_dir(Src::Bolt);
    let rows: Vec<_> = imp::files(&root)
        .iter()
        .flat_map(|p| imp::read(p))
        .collect();
    let c = collect(&rows);
    vec![imp::health(Src::Bolt, &root, "Bolt original HAR: turn-start + chat + bolt-merged + apply-changes + base file", c.receipts.values().map(Vec::len).sum(),
        &tr!("离线读取原始 HAR：将本轮起始提交、聊天工具正文、bolt-merged 回执、apply-changes 提交正文和同一 baseSha 的原文交叉核对；仅计验证过的新增行。需要绝对工具路径匹配项目，模型与执行时间未知；缺基线的新文件暂不计。", "Offline original HAR reader joins turn-start commit, tool text, bolt-merged, apply-changes commit bodies and the same baseSha's original file. Only verified added lines count. Absolute tool paths must match the project; model and execution time are unknown. New files without a baseline are skipped."), false)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pm() -> PathMap {
        PathMap {
            known: ["src/a.rs".into()].into_iter().collect(),
            markers: vec!["/home/project/".into()],
            excludes: vec![],
        }
    }
    fn exchange(method: &str, path: &str, body: Value) -> Value {
        json!({"request":{"method":method,"url":format!("https://bolt.new{path}")},"response":{"status":200,"content":{"mimeType":"application/json","text":body.to_string()}}})
    }
    fn fixture(tool: &str, args: Value, before: &str, after: &str) -> Value {
        let chat = exchange(
            "GET",
            "/api/chats/project-a",
            json!({"agent":"claude","model":"mutable-final-selection","editedAt":"2026-10-17T01:00:00Z","messages":[{"id":"u1","role":"user","content":"edit"},{"id":"a1","role":"assistant","status":"complete","createdAt":"2026-10-17T01:00:00Z","annotations":[{"type":"bolt-merged","mergedSha":"saved-sha"},{"type":"metadata","userMessageId":"u1"}],"parts":[{"type":"tool-invocation","toolInvocation":{"toolCallId":"call-1","toolName":tool,"args":args,"state":"result","result":"opaque server tool output"}}]}]}),
        );
        let mut apply = exchange(
            "POST",
            "/api/projects/project-a/apply-changes",
            json!({"sha":"saved-sha","files":[{"state":"modified","path":"src/a.rs","content":after,"isBinary":false}]}),
        );
        apply["request"]["postData"] = json!({"mimeType":"application/json","text":json!({"baseSha":"base-sha","sha":"saved-sha"}).to_string()});
        let baseline = json!({"request":{"method":"GET","url":"https://bolt.new/api/projects/project-a/files/base-sha/src/a~1rs?e"},"response":{"status":200,"content":{"mimeType":"text/plain","text":before}}});
        {
            let mut start = exchange("POST", "/api/chat", json!({}));
            start["request"]["postData"] = json!({"mimeType":"application/json","text":json!({"projectId":"project-a","codeStorageRepo":{"sha":"base-sha"},"messages":[{"id":"u1","role":"user","content":"edit"}]}).to_string()});
            json!({"log":{"version":"1.2","entries":[chat,apply,baseline,start]}})
        }
    }
    fn write_fixture() -> Value {
        fixture(
            "Write",
            json!({"file_path":"/home/project/src/a.rs","content":"human kept line\nai added line\n"}),
            "human kept line\n",
            "human kept line\nai added line\n",
        )
    }
    fn read_fixture(v: Value) -> FileGroup {
        let mut g = FileGroup::default();
        parse(&[v], &pm(), &mut g);
        g
    }
    fn chat_mut(v: &mut Value) -> Value {
        serde_json::from_str(
            v["log"]["entries"][0]["response"]["content"]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    }
    fn set_chat(v: &mut Value, chat: Value) {
        v["log"]["entries"][0]["response"]["content"]["text"] = json!(chat.to_string());
    }
    #[test]
    fn saved_write_subtracts_existing_human_context_and_keeps_metadata_unknown() {
        let g = read_fixture(write_fixture());
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(
            g.evs[0].key,
            creditia_core::hash::line_key(
                "src/a.rs",
                &creditia_core::hash::normalize("ai added line", "rs")
            )
        );
        assert_eq!(g.evs[0].ts, 0);
        assert_eq!(g.evs[0].model, "");
        assert_eq!(g.sessions[0].prompts, 1);
        assert_eq!(g.sessions[0].start, 0);
        assert_eq!(g.sessions[0].model, "");
    }
    #[test]
    fn completed_edit_and_multiedit_are_replayed_against_immutable_base() {
        let g = read_fixture(fixture(
            "Edit",
            json!({"file_path":"/home/project/src/a.rs","old_string":"old content","new_string":"new ai content"}),
            "human kept line\nold content\n",
            "human kept line\nnew ai content\n",
        ));
        assert_eq!(g.evs.len(), 1);
        let g = read_fixture(fixture(
            "MultiEdit",
            json!({"file_path":"/home/project/src/a.rs","edits":[{"old_string":"first old","new_string":"first added"},{"old_string":"second old","new_string":"second added"}]}),
            "human kept\nfirst old\nsecond old\n",
            "human kept\nfirst added\nsecond added\n",
        ));
        assert_eq!(g.evs.len(), 2);
    }
    #[test]
    fn earlier_apply_response_can_supply_the_exact_base_without_a_file_fetch() {
        let mut v = write_fixture();
        let mut earlier = v["log"]["entries"][1].clone();
        earlier["request"]["postData"]["text"] =
            json!(json!({"baseSha":"parent-sha","sha":"base-sha"}).to_string());
        earlier["response"]["content"]["text"]=json!(json!({"sha":"base-sha","files":[{"state":"modified","path":"src/a.rs","content":"human kept line\n","isBinary":false}]}).to_string());
        v["log"]["entries"][2] = earlier;
        assert_eq!(read_fixture(v).evs.len(), 1);
    }
    #[test]
    fn captured_turn_start_must_match_the_same_users_exact_base() {
        for mode in ["missing", "old-base", "different-user", "different-project"] {
            let mut v = write_fixture();
            if mode == "missing" {
                v["log"]["entries"][3]["response"]["status"] = json!(500);
            } else {
                let mut start: Value = serde_json::from_str(
                    v["log"]["entries"][3]["request"]["postData"]["text"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
                match mode {
                    "old-base" => start["codeStorageRepo"]["sha"] = json!("other-sha"),
                    "different-user" => start["messages"][0]["id"] = json!("other-user"),
                    _ => start["projectId"] = json!("other-project"),
                }
                v["log"]["entries"][3]["request"]["postData"]["text"] = json!(start.to_string());
            }
            assert!(read_fixture(v).evs.is_empty(), "{mode}");
        }
    }
    #[test]
    fn same_text_saved_in_distinct_commits_retains_distinct_occurrences() {
        let a = write_fixture();
        let mut b = a.clone();
        let mut chat = chat_mut(&mut b);
        chat["messages"][1]["parts"][0]["toolInvocation"]["toolCallId"] = json!("second-call");
        chat["messages"][1]["annotations"][0]["mergedSha"] = json!("second-sha");
        set_chat(&mut b, chat);
        b["log"]["entries"][1]["request"]["postData"]["text"] =
            json!(json!({"baseSha":"base-sha","sha":"second-sha"}).to_string());
        let mut receipt: Value = serde_json::from_str(
            b["log"]["entries"][1]["response"]["content"]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        receipt["sha"] = json!("second-sha");
        b["log"]["entries"][1]["response"]["content"]["text"] = json!(receipt.to_string());
        let mut g = FileGroup::default();
        parse(&[a, b], &pm(), &mut g);
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.sessions.len(), 1);
    }
    #[test]
    fn incomplete_failure_read_preview_and_generic_success_are_not_evidence() {
        for mode in [
            "no-merge",
            "preview",
            "error",
            "mcp-error",
            "read",
            "generic-success",
        ] {
            let mut v = write_fixture();
            let mut chat = chat_mut(&mut v);
            let message = &mut chat["messages"][1];
            match mode {
                "no-merge" => message["annotations"] = json!([]),
                "preview" => message["parts"][0]["toolInvocation"]["state"] = json!("call"),
                "error" => {
                    message["parts"][0]["toolInvocation"]["result"] =
                        json!({"type":"error","message":"write failed"})
                }
                "mcp-error" => {
                    message["parts"][0]["toolInvocation"]["result"] = json!({"isError":true})
                }
                "read" => message["parts"][0]["toolInvocation"]["toolName"] = json!("Read"),
                _ => {
                    message["parts"][0]["toolInvocation"]["result"] = json!({"success":true});
                    v["log"]["entries"][1]["response"]["status"] = json!(409);
                }
            }
            set_chat(&mut v, chat);
            assert!(read_fixture(v).evs.is_empty(), "{mode}");
        }
    }
    #[test]
    fn missing_mismatched_or_binary_receipts_and_baselines_are_rejected() {
        for mode in [
            "no-base",
            "different-project",
            "wrong-sha",
            "wrong-request-sha",
            "binary",
            "deleted",
            "unmerged",
            "different-body",
            "binary-baseline",
            "wrong-origin",
        ] {
            let mut v = write_fixture();
            match mode {
                "no-base" => v["log"]["entries"][2]["response"]["status"] = json!(404),
                "different-project" => {
                    v["log"]["entries"][1]["request"]["url"] =
                        json!("https://bolt.new/api/projects/project-b/apply-changes")
                }
                "wrong-request-sha" => {
                    v["log"]["entries"][1]["request"]["postData"]["text"] =
                        json!("{\"baseSha\":\"other\",\"sha\":\"wrong\"}")
                }
                "binary-baseline" => {
                    v["log"]["entries"][2]["response"]["content"]["mimeType"] = json!("image/png")
                }
                "wrong-origin" => {
                    v["log"]["entries"][1]["request"]["url"] =
                        json!("https://example.org/api/projects/project-a/apply-changes")
                }
                _ => {
                    let mut body: Value = serde_json::from_str(
                        v["log"]["entries"][1]["response"]["content"]["text"]
                            .as_str()
                            .unwrap(),
                    )
                    .unwrap();
                    match mode {
                        "wrong-sha" => body["sha"] = json!("wrong"),
                        "binary" => body["files"][0]["isBinary"] = json!(true),
                        "deleted" => body["files"][0]["state"] = json!("deleted"),
                        "unmerged" => body["files"][0]["state"] = json!("unmerged"),
                        _ => body["files"][0]["content"] = json!("different committed line"),
                    }
                    v["log"]["entries"][1]["response"]["content"]["text"] = json!(body.to_string());
                }
            }
            assert!(read_fixture(v).evs.is_empty(), "{mode}");
        }
    }
    #[test]
    fn absolute_protocol_path_must_still_match_pathmap() {
        for path in [
            "src/a.rs",
            "/other/src/a.rs",
            "/home/project/../other/src/a.rs",
        ] {
            let g = read_fixture(fixture(
                "Write",
                json!({"file_path":path,"content":"ai added line\n"}),
                "",
                "ai added line\n",
            ));
            assert!(g.evs.is_empty(), "{path}");
        }
        let mut g = FileGroup::default();
        parse(
            &[write_fixture()],
            &PathMap {
                markers: vec!["/real-project/".into()],
                ..pm()
            },
            &mut g,
        );
        assert!(g.evs.is_empty());
        assert_eq!(
            tool_rel("/tmp/cc-agent/abc/project/src/a.rs"),
            Some("src/a.rs")
        );
        assert_eq!(
            tool_rel("/workspace/bolt-code/abc/project/src/a.rs"),
            Some("src/a.rs")
        );
    }
    #[test]
    fn server_manual_changes_cannot_be_attributed_to_the_tool_proposal() {
        let g = read_fixture(fixture(
            "Write",
            json!({"file_path":"/home/project/src/a.rs","content":"ai added line\n"}),
            "human kept line\n",
            "ai added line\nmanual added line\n",
        ));
        assert!(g.evs.is_empty());
        let g = read_fixture(fixture(
            "Edit",
            json!({"file_path":"/home/project/src/a.rs","old_string":"missing old","new_string":"ai added line"}),
            "human kept line\n",
            "ai added line\n",
        ));
        assert!(g.evs.is_empty());
        let g = read_fixture(fixture(
            "Write",
            json!({"file_path":"/home/project/src/a.rs","content":"human kept line\n"}),
            "human kept line\n",
            "human kept line\n",
        ));
        assert!(g.evs.is_empty());
    }
    #[test]
    fn overlapping_exports_and_duplicate_legacy_parts_do_not_multiply_evidence() {
        let mut v = write_fixture();
        let mut chat = chat_mut(&mut v);
        chat["messages"][1]["toolInvocations"] =
            json!([chat["messages"][1]["parts"][0]["toolInvocation"].clone()]);
        set_chat(&mut v, chat);
        let mut g = FileGroup::default();
        parse(&[v.clone(), v], &pm(), &mut g);
        assert_eq!(g.evs.len(), 1);
        assert_eq!(g.sessions.len(), 1);
    }
    #[test]
    fn same_commit_file_is_emitted_once_across_distinct_calls() {
        let mut v = write_fixture();
        let mut chat = chat_mut(&mut v);
        let mut duplicate = chat["messages"][1]["parts"][0].clone();
        duplicate["toolInvocation"]["toolCallId"] = json!("second-native-call");
        chat["messages"][1]["parts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        set_chat(&mut v, chat);
        assert_eq!(read_fixture(v).evs.len(), 1);
    }
    #[test]
    fn conflicting_export_bodies_or_failure_observations_suppress_the_native_identity() {
        for mode in ["failure", "baseline", "after", "args", "merges"] {
            let a = write_fixture();
            let mut b = a.clone();
            match mode {
                "baseline" => {
                    b["log"]["entries"][2]["response"]["content"]["text"] =
                        json!("other baseline\n")
                }
                "after" => {
                    let mut body: Value = serde_json::from_str(
                        b["log"]["entries"][1]["response"]["content"]["text"]
                            .as_str()
                            .unwrap(),
                    )
                    .unwrap();
                    body["files"][0]["content"] = json!("conflicting after\n");
                    b["log"]["entries"][1]["response"]["content"]["text"] = json!(body.to_string());
                }
                _ => {
                    let mut chat = chat_mut(&mut b);
                    if mode == "merges" {
                        chat["messages"][1]["annotations"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"type":"bolt-merged","mergedSha":"contradictory-sha"}));
                    } else if mode == "failure" {
                        chat["messages"][1]["parts"][0]["toolInvocation"]["result"] =
                            json!({"type":"error"})
                    } else {
                        chat["messages"][1]["parts"][0]["toolInvocation"]["args"]["content"] =
                            json!("changed proposal")
                    };
                    set_chat(&mut b, chat);
                }
            }
            let mut g = FileGroup::default();
            parse(&[a, b], &pm(), &mut g);
            assert!(g.evs.is_empty(), "{mode}");
        }
    }
    #[test]
    fn base64_and_percent_encoded_native_paths_are_supported() {
        let mut v = write_fixture();
        let raw = v["log"]["entries"][2]["response"]["content"]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        v["log"]["entries"][2]["response"]["content"]["text"] = json!(STANDARD.encode(raw));
        v["log"]["entries"][2]["response"]["content"]["encoding"] = json!("base64");
        v["log"]["entries"][2]["request"]["url"] =
            json!("https://bolt.new/api/projects/project-a/files/base-sha/src%2Fa~1rs?e");
        assert_eq!(read_fixture(v).evs.len(), 1);
    }
    #[test]
    fn scan_rebuilds_har_set_after_duplicates_changes_and_deletion() {
        let dir = std::env::temp_dir().join(format!("creditia-bolt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.har"), dir.join("b.har"));
        let v = write_fixture();
        std::fs::write(&a, v.to_string()).unwrap();
        std::fs::write(&b, v.to_string()).unwrap();
        let mut store = Store::default();
        scan_root(&dir, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        let mut bad = v;
        bad["log"]["entries"][2]["response"]["content"]["text"] = json!("conflicting base");
        std::fs::write(&b, bad.to_string()).unwrap();
        scan_root(&dir, &mut store, &pm());
        assert!(store.groups.values().all(|g| g.evs.is_empty()));
        std::fs::remove_file(&b).unwrap();
        scan_root(&dir, &mut store, &pm());
        assert_eq!(store.groups.values().map(|g| g.evs.len()).sum::<usize>(), 1);
        std::fs::remove_file(&a).unwrap();
        scan_root(&dir, &mut store, &pm());
        assert!(store.groups.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
