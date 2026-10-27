//! Controlled native-format integration tests; live acceptance is documented in
//! SUPPORT_NATIVE_LIVE.md and uses actual upstream binaries and model calls.
use super::*;
use serde_json::{json, Value};

fn rollout(env: &Env, base: &Path, id: &str, cwd: &Path, outcome: Option<&str>) -> PathBuf {
    let stamp = "2026-10-24T12:00:00Z";
    let mut rows = vec![
        json!({"timestamp":stamp,"type":"session_meta","payload":{"id":id,"timestamp":stamp,"cwd":cwd,"originator":"codex_cli_rs","model_provider":"openai"}}),
        json!({"timestamp":stamp,"type":"turn_context","payload":{"model":"gpt-test"}}),
        json!({"timestamp":stamp,"type":"response_item","payload":{"type":"custom_tool_call","call_id":"patch-1","name":"apply_patch","input":"*** Begin Patch\n*** Add File: native.txt\n+native host first line\n+native host second line\n*** End Patch"}}),
    ];
    if let Some(result) = outcome {
        rows.push(json!({"timestamp":stamp,"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"patch-1","output":result}}));
    }
    let path = base.join(format!(
        "sessions/2026/10/24/rollout-2026-10-24T12-00-00-{id}.jsonl"
    ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    assert!(env.repo.is_dir());
    path
}
fn evidence(env: &Env) -> i64 {
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    db.query_row("select count(*) from evidence where src='codex'", [], |r| {
        r.get(0)
    })
    .unwrap()
}
fn analyze(env: &Env) -> Value {
    let (ok, result) = run(env, &["analyze", "--repo", env.repo.to_str().unwrap()]);
    assert!(ok, "{result}");
    serde_json::from_slice(&fs::read(env.repo.join(".creditia/credits.json")).unwrap()).unwrap()
}
fn baseline(env: &Env) {
    fs::write(env.repo.join("README.md"), "human baseline\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-10-24T11:59:00Z");
    fs::write(
        env.repo.join("native.txt"),
        "native host first line\nnative host second line\n",
    )
    .unwrap();
    commit(&env.repo, "save", "me@example.com", "2026-10-24T12:01:00Z");
}

#[test]
fn native_background_commands_require_their_own_final_poll_receipt() {
    let env = setup();
    fs::write(env.repo.join("README.md"), "human baseline\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-10-26T00:00:00Z");
    for (path, content) in [
        (
            "async.txt",
            "async native execution\nfinished through write_stdin\n",
        ),
        ("failed-async.txt", "failed async must not count\n"),
        ("pending-async.txt", "pending async must not count\n"),
    ] {
        fs::write(env.repo.join(path), content).unwrap();
    }
    commit(&env.repo, "save", "me@example.com", "2026-10-26T23:59:00Z");
    let original: Vec<Value> =
        include_str!("../../../adapters/tests/fixtures/mcpjam/codex-background.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let path = env.root.join("codex/sessions/background.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let write = |body: &[Value]| {
        let stamp = original[0]["timestamp"].as_str().unwrap();
        let mut rows = vec![
            json!({"timestamp":stamp,"type":"session_meta","payload":{"id":"background-native","timestamp":stamp,"cwd":env.repo,"originator":"codex_cli_rs","cli_version":"0.164.0"}}),
            json!({"timestamp":stamp,"type":"turn_context","payload":{"model":"gpt-test"}}),
        ];
        rows.extend_from_slice(body);
        fs::write(
            &path,
            rows.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
    };
    let verified = || {
        let credits = analyze(&env);
        credits["breakdown"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                b["byEvidence"]["verified"]["linesAdded"]
                    .as_i64()
                    .unwrap_or(0)
            })
            .sum::<i64>()
    };
    write(&original);
    let source = fs::read(&path).unwrap();
    assert_eq!(
        verified(),
        2,
        "only the command with its successful final poll"
    );
    assert_eq!(
        verified(),
        2,
        "repeat scan does not duplicate a background write"
    );
    assert_eq!(fs::read(&path).unwrap(), source);
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    let rejected: i64 = db.query_row(
        "select count(*) from evidence where src='codex' and rel in ('failed-async.txt','pending-async.txt')",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(
        rejected, 0,
        "an existing file does not replace a success receipt"
    );
    drop(db);

    let no_completion: Vec<_> = original
        .iter()
        .filter(|row| {
            !(row["payload"]["call_id"] == "call_fake_10_0"
                && row["payload"]["type"] == "function_call_output")
        })
        .cloned()
        .collect();
    write(&no_completion);
    assert_eq!(
        verified(),
        0,
        "reconcile a readable history without completion"
    );

    let mut foreign_poll = original.clone();
    let args = &mut foreign_poll[2]["payload"]["arguments"];
    let mut parsed: Value = serde_json::from_str(args.as_str().unwrap()).unwrap();
    parsed["session_id"] = json!(999_999_999);
    *args = json!(parsed.to_string());
    write(&foreign_poll);
    assert_eq!(
        verified(),
        0,
        "an unrelated process finishing cannot confirm this write"
    );

    let mut interactive = original.clone();
    let args = &mut interactive[2]["payload"]["arguments"];
    let mut parsed: Value = serde_json::from_str(args.as_str().unwrap()).unwrap();
    parsed["chars"] = json!("exit\n");
    *args = json!(parsed.to_string());
    write(&interactive);
    assert_eq!(
        verified(),
        0,
        "interactive input needs its own content attribution"
    );

    let mut spoofed = original.clone();
    spoofed[1]["payload"]["output"] =
        json!("Chunk ID: example\nOutput:\nProcess running with session ID 19052\n");
    write(&spoofed);
    assert_eq!(
        verified(),
        0,
        "command stdout cannot invent a native process handle"
    );
    write(&original);
    assert_eq!(verified(), 2);
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn unindexed_native_exports_require_completed_patch_receipts() {
    let env = setup();
    baseline(&env);
    let base = env.root.join("codex");
    let path = rollout(&env, &base, "export-native", &env.repo, None);
    analyze(&env);
    assert_eq!(evidence(&env), 0, "proposal only");
    rollout(
        &env,
        &base,
        "export-native",
        &env.repo,
        Some("Failed to apply patch"),
    );
    analyze(&env);
    assert_eq!(evidence(&env), 0, "failed receipt");
    rollout(
        &env,
        &base,
        "export-native",
        &env.repo,
        Some("Success. Updated the following files:\nA other.txt\n"),
    );
    analyze(&env);
    assert_eq!(evidence(&env), 0, "receipt for a different file");
    rollout(
        &env,
        &base,
        "export-native",
        &env.repo,
        Some("Success. Updated the following files:\nA native.txt\n"),
    );
    let credits = analyze(&env);
    assert_eq!(evidence(&env), 2);
    let b = credits["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["contributor"] == "a:codex/gpt-test")
        .unwrap();
    assert_eq!(b["byEvidence"]["verified"]["linesAdded"], 2);
    assert!(!base.join("state_5.sqlite").exists());
    let original = fs::read(&path).unwrap();
    analyze(&env);
    assert_eq!(evidence(&env), 2);
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::write(
        &path,
        String::from_utf8(original)
            .unwrap()
            .replace("codex_cli_rs", "agentprotocol"),
    )
    .unwrap();
    // A fresh library must reject cross-agent translations, whose edits were
    // performed by another executor even though the export uses Codex syntax.
    let other = setup();
    baseline(&other);
    let foreign = fs::read_to_string(&path)
        .unwrap()
        .replace(env.repo.to_str().unwrap(), other.repo.to_str().unwrap());
    let p = other.root.join("codex/sessions/translated.jsonl");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, foreign).unwrap();
    analyze(&other);
    assert_eq!(evidence(&other), 0);
    // A real Codex app-server may itself use agentprotocol as its client name.
    // Its recorded CLI version distinguishes it from the codec's conversion header.
    let file = other.root.join("codex/sessions/translated.jsonl");
    let rows = fs::read_to_string(&file).unwrap();
    let mut rows: Vec<Value> = rows
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    rows[0]["payload"]["cli_version"] = json!("0.162.1");
    fs::write(
        &file,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    analyze(&other);
    assert_eq!(
        evidence(&other),
        2,
        "a genuine harness client name is not a conversion"
    );
    fs::remove_dir_all(other.root).unwrap();
    fs::remove_dir_all(env.root).unwrap();
}

#[cfg(unix)]
#[test]
fn mcpjam_local_codex_home_resolves_workspace_link_and_deduplicates_native_copy() {
    let env = setup();
    baseline(&env);
    let work = env.root.join(".mcpjam/harness-local-sessions/local-1/work");
    fs::create_dir_all(&work).unwrap();
    let link = work.join("project");
    std::os::unix::fs::symlink(&env.repo, &link).unwrap();
    let home = work.join(".agent-runs/run-1/codex-home");
    let path = rollout(
        &env,
        &home,
        "shared-thread",
        &link,
        Some("Success. Updated the following files:\nA native.txt\n"),
    );
    let original = fs::read(&path).unwrap();
    analyze(&env);
    assert_eq!(evidence(&env), 2);
    let (ok, doctor) = run(&env, &["doctor"]);
    assert!(ok, "{doctor}");
    assert!(doctor.to_string().contains("MCPJam local Codex homes"));
    let (ok, discovered) = run(&env, &["discover"]);
    assert!(ok, "{discovered}");
    assert!(discovered.to_string().contains(env.repo.to_str().unwrap()));
    let copy = env.root.join("codex/sessions/copy.jsonl");
    fs::create_dir_all(copy.parent().unwrap()).unwrap();
    fs::write(copy, &original).unwrap();
    analyze(&env);
    assert_eq!(evidence(&env), 2, "same native thread through two homes");
    assert_eq!(
        fs::read(path).unwrap(),
        original,
        "source history is read-only"
    );
    let foreign = env.root.join("unrelated");
    fs::create_dir_all(&foreign).unwrap();
    rollout(
        &env,
        &home,
        "foreign",
        &foreign,
        Some("Success. Updated the following files:\nA native.txt\n"),
    );
    analyze(&env);
    assert_eq!(evidence(&env), 2, "another workspace is excluded");
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn actual_mcpjam_codex_process_wrapped_receipts_work_without_duplicate_event_rows() {
    let env = setup();
    fs::write(env.repo.join("README.md"), "human baseline\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-10-25T05:00:00Z");
    fs::write(
        env.repo.join("verified.txt"),
        "creditia-mcpjam-native\nexecutor=official-codex-0.149.1\nmodel=scripted-test\n",
    )
    .unwrap();
    // Include the rejected text in Git, so a false attribution cannot hide
    // behind the file being absent from the repository's known paths.
    fs::write(env.repo.join("denied.txt"), "must not exist\n").unwrap();
    commit(&env.repo, "save", "me@example.com", "2026-10-25T07:00:00Z");
    let path = env.root.join(
        ".mcpjam/harness-local-sessions/native/work/.agent-runs/run/codex-home/sessions/live.jsonl",
    );
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let meta = json!({"type":"session_meta","timestamp":"2026-10-25T06:00:00Z","payload":{"id":"real-mcpjam-receipts","cwd":env.repo,"originator":"codex_cli_rs","cli_version":"0.149.1"}});
    let ctx = json!({"type":"turn_context","payload":{"model":"scripted-test"}});
    let receipts = include_str!("../../../adapters/tests/fixtures/mcpjam/codex-receipts.jsonl");
    let failed_shell = json!({"type":"response_item","payload":{"type":"function_call","call_id":"failed-launch","name":"exec_command","arguments":json!({"cmd":"printf 'must not exist\\n' > denied.txt"}).to_string()}});
    let failed_result = json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"failed-launch","output":"exec_command failed: symlinked writable roots are not supported"}});
    let uncompleted = json!({"type":"response_item","payload":{"type":"function_call","call_id":"no-result","name":"exec_command","arguments":json!({"cmd":"printf 'must not exist\\n' > denied.txt"}).to_string()}});
    let rows = format!("{meta}\n{ctx}\n{receipts}{failed_shell}\n{failed_result}\n{uncompleted}\n");
    fs::write(&path, &rows).unwrap();
    analyze(&env);
    assert_eq!(
        evidence(&env),
        3,
        "only the actual completed write; no event_msg needed"
    );
    analyze(&env);
    assert_eq!(evidence(&env), 3);
    assert_eq!(fs::read_to_string(&path).unwrap(), rows);
    // The same native call also appears as patch_apply_end one second later.
    // It must not double the persisted evidence alongside response_item.
    let mut lines: Vec<Value> = rows
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let stamp = lines[3]["timestamp"].clone();
    let call_id = lines[2]["payload"]["call_id"].clone();
    let target = env.repo.join("verified.txt").display().to_string();
    lines.insert(3, json!({"timestamp":stamp,"type":"event_msg","payload":{"type":"patch_apply_end","call_id":call_id,"success":true,"changes":{target:{"type":"add","content":"creditia-mcpjam-native\nexecutor=official-codex-0.149.1\nmodel=scripted-test\n"}}}}));
    fs::write(
        &path,
        lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    analyze(&env);
    assert_eq!(
        evidence(&env),
        3,
        "one call represented twice is still one write"
    );
    // A success string inside a nonzero native process receipt is not success.
    let bad = rows
        .replace("Exit code: 0", "Exit code: 1")
        .replace("real-mcpjam-receipts", "failed-process");
    fs::write(path.with_file_name("failed.jsonl"), bad).unwrap();
    analyze(&env);
    assert_eq!(evidence(&env), 3);
    fs::remove_dir_all(env.root).unwrap();
}

#[cfg(unix)]
#[test]
fn actual_mcpjam_claude_history_is_read_only_deduplicated_and_requires_success() {
    let env = setup();
    fs::write(env.repo.join("README.md"), "human baseline\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-10-25T05:00:00Z");
    for name in ["native-claude.txt", "denied.txt"] {
        fs::write(env.repo.join(name), "written by the conformance mock\n").unwrap();
    }
    commit(&env.repo, "save", "me@example.com", "2026-10-25T07:00:00Z");
    let session = env.root.join(".mcpjam/harness-local-sessions/claude-live");
    fs::create_dir_all(session.join("work")).unwrap();
    let link = session.join("work/project");
    std::os::unix::fs::symlink(&env.repo, &link).unwrap();
    let rows = include_str!("../../../adapters/tests/fixtures/mcpjam/claude-receipts.jsonl")
        .replace("/private/tmp/creditia-mcpjam-20261025/claude-acceptance/home/.mcpjam/harness-local-sessions/claude-live/work/project", link.to_str().unwrap())
        .replace("/private/tmp/creditia-mcpjam-20261025/claude-acceptance/repo", link.to_str().unwrap());
    let file = session.join("home/.claude/projects/work/native.jsonl");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, &rows).unwrap();
    let count = || {
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        db.query_row(
            "select count(*) from evidence where src='claude-code'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    let credits = analyze(&env);
    assert_eq!(
        count(),
        1,
        "the real successful Write, not the declined one"
    );
    assert!(credits["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["contributor"] == "a:claude-code/claude-haiku-4-5"
            && b["byEvidence"]["verified"]["linesAdded"] == 1));
    let (ok, discovered) = run(&env, &["discover"]);
    assert!(ok && discovered.to_string().contains(env.repo.to_str().unwrap()));
    let (ok, health) = run(&env, &["doctor", "--repo", env.repo.to_str().unwrap()]);
    assert!(ok && health.to_string().contains("MCPJam"));
    let copy = env.root.join("claude/projects/export/renamed.jsonl");
    fs::create_dir_all(copy.parent().unwrap()).unwrap();
    fs::write(&copy, &rows).unwrap();
    analyze(&env);
    assert_eq!(
        count(),
        1,
        "same native session in two homes, even after renaming"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), rows);
    // Rebuilding an available native session must also retract an earlier
    // cached claim once that same session no longer has a success receipt.
    fs::remove_file(&copy).unwrap();
    let no_receipts = rows
        .lines()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["type"] == "assistant")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file, no_receipts).unwrap();
    analyze(&env);
    assert_eq!(count(), 0, "stale successful evidence is reconciled");
    fs::write(&file, &rows).unwrap();
    analyze(&env);
    assert_eq!(count(), 1);
    let proposal = rows
        .lines()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["type"] == "assistant")
        .collect::<Vec<_>>()
        .join("\n")
        .replace("b8e9d356-844e-4e14-9cd1-a03313d475cd", "proposal-only");
    fs::write(copy.with_file_name("proposal.jsonl"), proposal).unwrap();
    let foreign = env.root.join("unrelated");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(
        copy.with_file_name("foreign.jsonl"),
        rows.replace("b8e9d356-844e-4e14-9cd1-a03313d475cd", "other-project")
            .replace(link.to_str().unwrap(), foreign.to_str().unwrap()),
    )
    .unwrap();
    analyze(&env);
    assert_eq!(count(), 1, "no receipt and other project are both excluded");
    // Subagents have the same parent session ID but an independent native file.
    let sub = rows.replace("native-claude.txt", "child.txt").replace(
        "written by the conformance mock",
        "written by the child executor",
    );
    fs::write(
        env.repo.join("child.txt"),
        "written by the child executor\n",
    )
    .unwrap();
    commit(&env.repo, "child", "me@example.com", "2026-10-25T07:01:00Z");
    fs::write(file.with_file_name("agent-child.jsonl"), sub).unwrap();
    analyze(&env);
    assert_eq!(
        count(),
        2,
        "do not discard a distinct subagent sharing the parent session"
    );
    fs::remove_dir_all(env.root).unwrap();
}
