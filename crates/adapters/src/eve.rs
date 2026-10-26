//! Eve local OTLP v4 file receipts joined to native Workflow checkpoints.
//! A sandbox path/appRoot is not a repository identity. See SUPPORT_AGENT4.md.
use crate::{
    eve_codec::{context_after, Graph},
    native_history as n, trace_import as imp, Activity, Health, Stats,
};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
};
mod local;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    remote: String,
    checkout: String,
    local: bool,
    patch_tools: BTreeSet<String>,
}
type Bindings = HashMap<String, Option<Binding>>;

fn roots(projects: &[String]) -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("CREDITIA_EVE_DIR") {
        return vec![p.into()];
    }
    let mut roots = vec![crate::home().join(".eve")];
    roots.extend(projects.iter().map(|p| Path::new(p).join(".eve")));
    roots.sort();
    roots.dedup();
    roots
}
fn traces(root: &Path) -> Vec<PathBuf> {
    let directory = root.join("traces");
    n::files(&directory, |p| {
        (p.file_name()
            .is_some_and(|s| s.to_string_lossy().ends_with(".otlp.json"))
            && p.parent()
                .and_then(Path::file_name)
                .is_some_and(|s| s == "segments"))
            // `eve traces --json` exports retain native spans but use plain
            // attributes and startTimeNs/endTimeNs instead of OTLP envelopes.
            || (p.parent() == Some(directory.as_path())
                && p.extension().is_some_and(|s| s == "json"))
    })
}
fn clean_absolute(s: &str) -> Option<String> {
    if !s.starts_with('/') || s.contains(['\\', '\0']) {
        return None;
    }
    let mut parts = vec![];
    for p in s.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(p),
        }
    }
    Some(format!("/{}", parts.join("/")))
}
fn repo_component(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn binding(channel: &Value, patch_tools: &BTreeSet<String>) -> Option<Binding> {
    // Other adapters may expose similarly named state. Only the native GitHub
    // channel's default route is understood here; custom route names are skipped.
    if channel["kind"] != "channel:github" {
        return None;
    }
    let state = &channel["state"];
    let owner = state["owner"].as_str()?;
    let repo = state["repo"].as_str()?;
    if !repo_component(owner) || !repo_component(repo) || state["repositoryId"].as_u64()? == 0 {
        return None;
    }
    let checkout = clean_absolute(state["checkoutPath"].as_str()?)?;
    if checkout == "/" {
        return None;
    }
    Some(Binding {
        remote: format!("https://github.com/{owner}/{repo}"),
        checkout,
        local: false,
        patch_tools: patch_tools.clone(),
    })
}

fn patch_tools(bundle: Option<Value>, revision: Option<Value>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(bundle) = bundle else { return names };
    // A mutable current-development manifest cannot prove an old tool's origin.
    if bundle["source"]["kind"] != "disk" {
        return names;
    }
    let Some(app) = bundle["source"]["appRoot"]
        .as_str()
        .filter(|s| Path::new(s).is_absolute())
    else {
        return names;
    };
    let directory = Path::new(app).join(".eve/compile");
    let Ok(bytes) = std::fs::read(directory.join("compiled-agent-manifest.json")) else {
        return names;
    };
    let Ok(metadata) = n::json(&directory.join("compile-metadata.json")) else {
        return names;
    };
    let hash = n::text(&metadata["discovery"]["sourceGraphHash"]);
    let Some(revision) = revision.as_ref().and_then(Value::as_str) else {
        return names;
    };
    let loader = bundle["source"]["moduleMapLoaderPath"]
        .as_str()
        .map(|path| format!(":authored-source:{path}"))
        .unwrap_or_default();
    let version = format!("disk:{app}{loader}:{hash}");
    // appRoot can be recompiled between turns. Only the exact source graph
    // recorded in that turn may establish the tool's implementation.
    if hash.is_empty()
        || metadata["kind"] != "eve-compile-metadata"
        || metadata["version"] != 6
        || metadata["status"] != "ready"
        || metadata["compile"]["manifest"]["sha256"] != format!("{:x}", Sha256::digest(&bytes))
        || !(revision == version
            || revision
                .strip_prefix(&format!("{version}:mtime-"))
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric())))
    {
        return names;
    }
    let Ok(manifest) = serde_json::from_slice::<Value>(&bytes) else {
        return names;
    };
    if manifest["kind"] != "eve-agent-compiled-manifest" || manifest["version"] != 54 {
        return names;
    }
    let node = match bundle["nodeId"].as_str() {
        None | Some("__root__") => &manifest,
        Some(id) => match manifest["subagents"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|n| n["nodeId"] == id)
            .and_then(|n| n.get("agent"))
        {
            Some(n) => n,
            None => return names,
        },
    };
    for t in node["tools"].as_array().into_iter().flatten() {
        if t["hasExecute"] != true || t["sourceKind"] != "module" {
            continue;
        }
        let (Some(name), Some(source)) = (t["name"].as_str(), t["sourceId"].as_str()) else {
            continue;
        };
        let backing = &node["bindings"][source]["backing"];
        if backing["kind"] != "filesystem" {
            continue;
        }
        let Some(file) = backing["sourcePath"].as_str() else {
            continue;
        };
        for mount in node["extensionMounts"].as_array().into_iter().flatten() {
            let Some(root) = mount["sourceRoot"].as_str() else {
                continue;
            };
            if mount["packageName"] == "eve-code"
                && backing["mountId"] == mount["mountId"]
                && matches!(
                    file.strip_prefix(&format!("{}/", root.trim_end_matches('/'))),
                    Some("tools/apply_patch.ts" | "tools/apply_patch.js")
                )
            {
                names.insert(name.into());
            }
        }
    }
    names
}

fn contexts(root: &Path, warnings: &mut Vec<String>) -> Bindings {
    let mut out = Bindings::new();
    for path in n::files(&root.join(".workflow-data/steps"), |p| {
        p.extension().is_some_and(|s| s == "json")
    }) {
        let result = (|| -> Result<(), String> {
            let step = n::json(&path)?;
            let name = n::text(&step["stepName"]);
            let packaged = name
                .strip_prefix("step//eve@")
                .and_then(|s| s.strip_suffix("//turnStep"))
                .is_some_and(|version| {
                    !version.is_empty()
                        && version
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b".-+".contains(&c))
                });
            if step["status"] != "completed"
                || !(packaged
                    || name.ends_with("/execution/session/turn-step.ts//turnStep")
                    || name.ends_with("/execution/session/turn-step.js//turnStep"))
            {
                return Ok(());
            }
            let input = Graph::read(&step["input"])?;
            // Workflow stores StepArguments { args, closureVars, thisVal }.
            // Selecting args first avoids decoding opaque runtime capabilities.
            let Some(id) = input.get(&["args", "0", "sessionState", "sessionId"])? else {
                return Err("missing native session ID".into());
            };
            let id = id
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("invalid native session ID")?;
            if input.get(&["args", "0", "sessionState", "version"])? != Some(Value::from(2)) {
                return Err("unsupported Eve session version".into());
            }
            let channel = input
                .get(&["args", "0", "serializedContext", "eve.channel"])?
                .unwrap_or(Value::Null);
            let output = Graph::read(&step["output"])?;
            let next = context_after(&channel, &output, "eve.channel")?;
            let revision_key = "eve.sessionDynamicToolRuntimeRevision";
            let revision = input
                .get(&["args", "0", "serializedContext", revision_key])?
                .unwrap_or(Value::Null);
            let bundle = input.get(&["args", "0", "serializedContext", "eve.bundle"])?;
            let revision = context_after(&revision, &output, revision_key)?;
            let tools = patch_tools(bundle.clone(), revision.clone());
            let local = local::binding(root, id, bundle.as_ref(), revision.as_ref());
            let candidates = [Some(channel), next]
                .into_iter()
                .flatten()
                .filter_map(|channel| binding(&channel, &tools))
                .chain(local);
            for b in candidates {
                let entry = out.entry(id.into()).or_insert_with(|| Some(b.clone()));
                if let Some(prior) = entry {
                    if prior.remote != b.remote
                        || prior.checkout != b.checkout
                        || prior.local != b.local
                    {
                        *entry = None
                    } else {
                        prior.patch_tools = prior
                            .patch_tools
                            .intersection(&b.patch_tools)
                            .cloned()
                            .collect()
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            warnings.push(format!("eve: {}: {e}", path.display()));
        }
    }
    out
}

fn attributes(v: &Value) -> HashMap<String, Value> {
    if let Some(fields) = v.as_object() {
        return fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    }
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|kv| {
            let key = kv["key"].as_str()?;
            let value = ["stringValue", "boolValue", "intValue", "doubleValue"]
                .iter()
                .find_map(|k| kv["value"].get(k))?;
            Some((key.into(), value.clone()))
        })
        .collect()
}
fn nanos(v: &Value) -> Option<i64> {
    v.as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| v.as_i64())
        .map(|n| n / 1_000_000_000)
}
fn new_file_receipt(args: &Value, result: &Value) -> Result<Option<(String, String)>, String> {
    // Overwrites also contain human baseline text. A receipt alone proves
    // attribution only for new files, and repository binding is checked by read.
    if result["existed"] != false {
        return Ok(None);
    }
    let (Some(target), Some(text)) = (
        result["path"].as_str().and_then(clean_absolute),
        args["content"].as_str(),
    ) else {
        return Err("invalid native write_file receipt".into());
    };
    Ok(Some((target, text.into())))
}
fn read(
    path: &Path,
    pm: &PathMap,
    bindings: &Bindings,
    remotes: &BTreeSet<String>,
    warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<FileGroup, String> {
    let v = n::json(path)?;
    let spans: Vec<&Value> = if let Some(resources) = v["resourceSpans"].as_array() {
        resources
            .iter()
            .flat_map(|r| r["scopeSpans"].as_array().into_iter().flatten())
            .flat_map(|s| s["spans"].as_array().into_iter().flatten())
            .collect()
    } else if let Some(traces) = v.as_array() {
        let mut spans = vec![];
        for trace in traces {
            if trace["traceId"].as_str().is_none_or(str::is_empty) {
                return Err("invalid Eve traces export".into());
            }
            spans.extend(trace["spans"].as_array().ok_or("missing exported spans")?);
        }
        spans
    } else {
        return Err("expected native OTLP or Eve traces export".into());
    };
    let mut g = FileGroup::default();
    for span in spans {
        let a = attributes(&span["attributes"]);
        let get = |key: &str| a.get(key).and_then(Value::as_str).unwrap_or("");
        if get("gen_ai.operation.name") != "execute_tool"
            || !matches!(a.get("agent.trace.schema.version"), Some(Value::String(s)) if s == "4")
                && a.get("agent.trace.schema.version") != Some(&Value::from(4))
        {
            continue;
        }
        let session = get("agent.run.id");
        let call = get("gen_ai.tool.call.id");
        if session.is_empty() || call.is_empty() {
            continue;
        }
        let tool = get("gen_ai.tool.name");
        let native_write =
            tool == "write_file" && a.get("agent.tool.is_framework") == Some(&Value::Bool(true));
        let Some(Some(binding)) = bindings.get(session) else {
            if tool == "write_file" {
                warnings.borrow_mut().push(format!("eve: {} session {session}: no verified native repository binding for tool receipt", path.display()));
            }
            continue;
        };
        // PathMap also has basename markers for local worktrees. A cloud
        // checkout must match the full Git remote, including its owner.
        if !binding.local && !remotes.contains(&crate::git::normalize_remote(&binding.remote))
            || !pm.in_project(&binding.remote)
        {
            continue;
        }
        let patch = binding.patch_tools.contains(tool);
        if !native_write && !patch {
            continue;
        }
        let id = format!("tool:{}:{session}:{call}", binding.remote);
        if span["status"]["code"] == 2
            || span["statusCode"] == 2
            || span["status"]["code"] == "STATUS_CODE_ERROR"
            // Durable actions and direct invocation spans use different native
            // outcome fields in the same Eve release. Either failure is binding.
            || ["agent.action.outcome", "eve.tool.outcome"].iter().any(|key| {
                matches!(get(key), "failed" | "invalid-input" | "denied" | "cancelled")
            })
        {
            g.reconciled_sessions.push((Src::Eve, id));
            continue;
        }
        let (Some(start), Some(end)) = (
            nanos(
                span.get("startTimeUnixNano")
                    .unwrap_or(&span["startTimeNs"]),
            ),
            nanos(span.get("endTimeUnixNano").unwrap_or(&span["endTimeNs"])),
        ) else {
            continue;
        };
        if start <= 0 || end < start {
            continue;
        }
        let args: Value = serde_json::from_str(get("gen_ai.tool.call.arguments"))
            .map_err(|e| format!("invalid native tool arguments: {e}"))?;
        let result: Value = serde_json::from_str(get("gen_ai.tool.call.result"))
            .map_err(|e| format!("missing or invalid native tool receipt: {e}"))?;
        let before = g.evs.len();
        if native_write {
            let Some((target, text)) = new_file_receipt(&args, &result)? else {
                continue;
            };
            let Some(rel) = target.strip_prefix(&format!("{}/", binding.checkout)) else {
                continue;
            };
            imp::push_scoped(
                &mut g,
                pm,
                &binding.remote,
                rel,
                &text,
                Src::Eve,
                "",
                &id,
                end,
            );
        } else {
            let Some(root) = args["root"].as_str().and_then(clean_absolute) else {
                continue;
            };
            let prefix = if root == binding.checkout {
                ""
            } else if let Some(rel) = root.strip_prefix(&format!("{}/", binding.checkout)) {
                rel
            } else {
                continue;
            };
            let files = result["files"]
                .as_array()
                .ok_or("missing native patch receipt files")?;
            if !result["diagnostics"].is_array() {
                return Err("invalid native patch receipt".into());
            }
            let text = args["patchText"]
                .as_str()
                .ok_or("missing native patchText")?;
            for (target, text) in crate::apply_patch_files(text) {
                // The receipt contains post-move paths. No proposal-only file,
                // absolute path, or traversal may contribute.
                if target.is_empty()
                    || target.starts_with('/')
                    || target.contains('\\')
                    || target.split('/').any(|p| p == "..")
                {
                    continue;
                }
                if !files.iter().any(|f| {
                    f["path"] == target
                        && matches!(f["operation"].as_str(), Some("add" | "update" | "move"))
                }) {
                    continue;
                }
                let rel = if prefix.is_empty() {
                    target
                } else {
                    format!("{prefix}/{target}")
                };
                imp::push_scoped(
                    &mut g,
                    pm,
                    &binding.remote,
                    &rel,
                    &text,
                    Src::Eve,
                    "",
                    &id,
                    end,
                );
            }
        }
        g.reconciled_sessions.push((Src::Eve, id.clone()));
        // Per-call identity also makes copies of immutable trace segments idempotent.
        g.sessions.push(imp::session(
            Src::Eve,
            &id,
            "",
            start,
            end,
            0,
            g.evs.len() > before,
        ));
    }
    Ok(g)
}

pub fn scan(store: &mut Store, pm: &PathMap, projects: &[String], st: &mut Stats) {
    let roots = roots(projects);
    let mut bindings = vec![];
    let mut files = vec![];
    for root in roots {
        let mut contexts = contexts(&root, &mut st.warnings);
        // Local sandboxes must be inside an exact selected checkout. Basename
        // markers alone can match an unrelated repository with the same name.
        contexts.retain(|_, b| {
            b.as_ref().is_none_or(|b| {
                !b.local
                    || projects.iter().any(|p| {
                        Path::new(p)
                            .canonicalize()
                            .ok()
                            .is_some_and(|p| Path::new(&b.remote).starts_with(p))
                    })
            })
        });
        bindings.push((root.clone(), contexts));
        files.extend(traces(&root));
    }
    files.sort();
    files.dedup();
    let remotes = if files.is_empty() {
        BTreeSet::new()
    } else {
        projects
            .iter()
            .flat_map(|p| {
                crate::git::git(Path::new(p), &["remote", "-v"])
                    .lines()
                    .filter_map(|l| l.split_whitespace().nth(1))
                    .map(crate::git::normalize_remote)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let warnings = std::cell::RefCell::new(vec![]);
    n::scan(Src::Eve, store, pm, st, files, |path, pm| {
        let (_, b) = bindings
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .ok_or("missing Eve store")?;
        read(path, pm, b, &remotes, &warnings)
    });
    st.warnings.extend(warnings.into_inner());
}
pub fn discover() -> Vec<Activity> {
    roots(&[])
        .iter()
        .flat_map(|root| contexts(root, &mut vec![]).into_values().flatten())
        .map(|b| {
            imp::activity(
                Src::Eve,
                b.remote.clone(),
                (!b.local).then_some(b.remote),
                0,
            )
        })
        .collect()
}
pub fn doctor() -> Health {
    let root = &roots(&[])[0];
    imp::health(Src::Eve, root, "OTLP / traces JSON + Workflow checkpoints", traces(root).len(), "GitHub-bound new write_file receipts and eve-code patches; local eve dev self-modification requires the run's immutable generation, verified manifest and exact official sandbox source hash. Save eve traces --json under traces/. Unbound virtual files, encrypted or ambiguous checkpoints, existing-file overwrites, shell edits and missing bodies are excluded. Model remains unknown.", false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_local_write_receipt_matches_native_readback() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eve/live-local");
        let record = |name: &str, digest: &str| {
            let bytes = std::fs::read(root.join(name)).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), digest);
            let v: Value = serde_json::from_slice(&bytes).unwrap();
            attributes(&v["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["attributes"])
        };
        let write = record(
            "write_file.otlp.json",
            "b24c49f22ad51e6871225c07c0890b8e178af2fe5452cd5c3912c5cd41c1a1e5",
        );
        let read = record(
            "read_file.otlp.json",
            "0b96c236c58c357b7c531fb0e8c039bfc1ec1a07d6003cf96b399bacecf1387b",
        );
        assert_eq!(write["agent.run.id"], read["agent.run.id"]);
        assert_eq!(write["agent.action.outcome"], "completed");
        assert_eq!(read["agent.action.outcome"], "completed");
        let args =
            serde_json::from_str(write["gen_ai.tool.call.arguments"].as_str().unwrap()).unwrap();
        let result =
            serde_json::from_str(write["gen_ai.tool.call.result"].as_str().unwrap()).unwrap();
        let (path, content) = new_file_receipt(&args, &result).unwrap().unwrap();
        let readback: Value =
            serde_json::from_str(read["gen_ai.tool.call.result"].as_str().unwrap()).unwrap();
        assert_eq!(path, "/source/verified-write.txt");
        assert_eq!(readback["path"], path);
        assert_eq!(readback["totalLines"], 3);
        assert_eq!(readback["truncated"], false);
        assert_eq!(
            readback["content"],
            content
                .lines()
                .enumerate()
                .map(|(i, l)| format!("{}: {l}", i + 1))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn official_ci_new_file_receipt_matches_real_readback_without_inventing_binding() {
        let directory =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eve/official-ci");
        let mut records = vec![];
        for (file, digest) in [
            (
                "write.otlp.json",
                "6c849c051ccd19712042c2f8e4edd22c86b4c5df57ec388bf2ff7fb00e53fb12",
            ),
            (
                "read.otlp.json",
                "7dd2d88a63dff02fb522456ff9f41a07c19d6964b679b0898098673b7f182a7f",
            ),
        ] {
            let bytes = std::fs::read(directory.join(file)).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), digest);
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            let span = &value["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
            let a = attributes(&span["attributes"]);
            assert_eq!(a["agent.framework.name"], "eve");
            assert_eq!(a["agent.framework.version"], "0.83.0");
            assert_eq!(a["agent.action.outcome"], "completed");
            assert_eq!(a["agent.tool.is_framework"], true);
            assert_ne!(span["status"]["code"], 2);
            records.push(a);
        }
        assert_eq!(records[0]["agent.run.id"], records[1]["agent.run.id"]);
        assert_eq!(records[0]["gen_ai.tool.name"], "write_file");
        assert_eq!(records[1]["gen_ai.tool.name"], "read_file");
        let args: Value =
            serde_json::from_str(records[0]["gen_ai.tool.call.arguments"].as_str().unwrap())
                .unwrap();
        let result: Value =
            serde_json::from_str(records[0]["gen_ai.tool.call.result"].as_str().unwrap()).unwrap();
        let readback: Value =
            serde_json::from_str(records[1]["gen_ai.tool.call.result"].as_str().unwrap()).unwrap();
        let (target, text) = new_file_receipt(&args, &result).unwrap().unwrap();
        assert_eq!(target, "/workspace/notes/handoff.txt");
        assert_eq!(text, "sandbox-file-tools-ok-Q2H");
        assert_eq!(readback["path"], target);
        assert_eq!(readback["content"], format!("1: {text}"));
        assert_eq!(readback["totalLines"], 1);
        assert_eq!(readback["truncated"], false);

        // This real run has no GitHub checkpoint. Do not manufacture one from
        // the CI source repository or treat /workspace as the selected project.
        let warnings = std::cell::RefCell::new(vec![]);
        let group = read(
            &directory.join("write.otlp.json"),
            &n::project(),
            &Bindings::new(),
            &BTreeSet::new(),
            &warnings,
        )
        .unwrap();
        assert!(group.evs.is_empty());
        assert!(group.sessions.is_empty());
        assert_eq!(warnings.borrow().len(), 1);
        assert!(warnings.borrow()[0].contains("no verified native repository binding"));
    }

    /// Original public runs, fetched separately to keep user context and
    /// unrelated runtime credentials out of our repository. See SUPPORT_AGENT3.md.
    #[test]
    #[ignore = "requires the pinned public Eve checkpoints and trace export"]
    fn public_native_history_decodes_without_inventing_repository_binding() {
        let directory = PathBuf::from(
            std::env::var_os("CREDITIA_TEST_EVE_HISTORY_DIR").expect("public history directory"),
        );
        let temp = std::env::temp_dir().join(format!(
            "creditia-eve-public-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp.join(".workflow-data/steps")).unwrap();
        for (file, digest) in [
            (
                "checkpoint0.json",
                "be31282ab307535f65554f11f7c5ba9a95e0ddb12f4ae8195e90611756135ab8",
            ),
            (
                "checkpoint1.json",
                "ba8330ddac39306d2b50342ce5242f8a33b3d706f5f7a3c949393e5055d41b07",
            ),
        ] {
            let bytes = std::fs::read(directory.join(file)).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), digest);
            let step: Value = serde_json::from_slice(&bytes).unwrap();
            let input = Graph::read(&step["input"]).unwrap();
            assert_eq!(
                input
                    .get(&["args", "0", "sessionState", "version"])
                    .unwrap(),
                Some(Value::from(1))
            );
            assert_eq!(
                input
                    .get(&["args", "0", "serializedContext", "eve.channel", "kind"])
                    .unwrap(),
                Some(Value::from("http"))
            );
            assert!(Graph::read(&step["output"])
                .unwrap()
                .get(&["action"])
                .unwrap()
                .is_some());
            std::fs::write(temp.join(".workflow-data/steps").join(file), &bytes).unwrap();
            assert_eq!(std::fs::read(directory.join(file)).unwrap(), bytes);
        }
        let mut warnings = vec![];
        assert!(contexts(&temp, &mut warnings).is_empty());
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .all(|w| w.contains("unsupported Eve session version")));

        let trace = directory.join("golden-trace.json");
        let bytes = std::fs::read(&trace).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "f887c86c2c7893e096df05b3af3591af108c3e55d3593f7a523cea7b133526ec"
        );
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v[0]["spans"].as_array().unwrap().len(), 27);
        // The real benchmark wrote an existing sandbox file, but exports no
        // GitHub checkpoint. It must not become local repository evidence.
        let warnings = std::cell::RefCell::new(vec![]);
        let group = read(
            &trace,
            &n::project(),
            &Bindings::new(),
            &BTreeSet::new(),
            &warnings,
        )
        .unwrap();
        assert!(group.evs.is_empty());
        assert!(group.reconciled_sessions.is_empty());
        assert_eq!(warnings.borrow().len(), 1);
        assert!(warnings.borrow()[0].contains("no verified native repository binding"));
        assert_eq!(std::fs::read(trace).unwrap(), bytes);
        std::fs::remove_dir_all(temp).unwrap();
    }
}
