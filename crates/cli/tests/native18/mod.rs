//! Shapes reconstructed from the pinned official distributions in SUPPORT_AGENT18.md.
//! No real model invocation is claimed: exercise the binary, durable store and final credits.
use super::*;
use serde_json::{json, Value};
fn write(p: impl AsRef<Path>, v: &Value) {
    let p = p.as_ref();
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, v.to_string()).unwrap();
}
fn jsonl(p: impl AsRef<Path>, v: &[Value]) {
    let p = p.as_ref();
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, v.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
}
fn db(p: impl AsRef<Path>, sql: &str) -> rusqlite::Connection {
    let p = p.as_ref();
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(p).unwrap();
    db.execute_batch(sql).unwrap();
    db
}
#[test]
fn sarvam_rollouts_attribute_confirmed_patches_and_retract_failed_final_receipts() {
    let mut env = setup();
    let rs = env.repo.display().to_string();
    let dir = env.root.join("sarvam");
    env.vars
        .insert("CREDITIA_SARVAM_CODE_DIR".into(), dir.display().to_string());
    fs::write(env.repo.join("README.md"), "base\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let code = (0..8)
        .map(|i| format!("export const sarvam_{i} = {i} + 402;\n"))
        .collect::<String>();
    fs::write(env.repo.join("native.ts"), &code).unwrap();
    commit(&env.repo, "saved", "me@example.com", "2026-09-17T00:01:00Z");
    let p = dir.join("sessions/2026/09/17/rollout-s.jsonl");
    let mut rows = vec![
        json!({"timestamp":"2026-09-17T00:00:01Z","type":"session_meta","payload":{"id":"sarvam-session","cwd":rs,"timestamp":"2026-09-17T00:00:01Z","originator":"sarvam-code","cli_version":"0.55.0"}}),
        json!({"type":"turn_context","payload":{"turn_id":"turn","model":"sarvam-model"}}),
        json!({"timestamp":"2026-09-17T00:00:02Z","type":"event_msg","payload":{"type":"patch_apply_end","call_id":"call","success":true,"changes":{"native.ts":{"type":"add","content":code}}}}),
    ];
    jsonl(&p, &rows);
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for round in 0..3 {
        if round == 1 {
            let mut fork = rows.clone();
            fork[0]["payload"]["id"] = json!("sarvam-fork");
            jsonl(dir.join("archived_sessions/fork.jsonl"), &fork);
        }
        if round == 2 {
            rows[2]["payload"]["success"] = json!(false);
            jsonl(&p, &rows);
            rows[0]["payload"]["id"] = json!("sarvam-fork");
            jsonl(dir.join("archived_sessions/fork.jsonl"), &rows);
        }
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let count: i64 = db
            .query_row(
                "select count(*) from evidence where src='sarvam-code'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, if round == 2 { 0 } else { 8 });
        let credits: Value =
            serde_json::from_slice(&fs::read(env.repo.join(".creditia/credits.json")).unwrap())
                .unwrap();
        let lines: i64 = credits["breakdown"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["contributor"] == "a:sarvam-code/sarvam-model")
            .map(|b| {
                b["byEvidence"]["verified"]["linesAdded"]
                    .as_i64()
                    .unwrap_or(0)
            })
            .sum();
        assert_eq!(lines, if round == 2 { 0 } else { 8 });
    }
    fs::remove_dir_all(env.root).unwrap();
}
#[test]
fn native18_reaches_credits_and_reconciles_receipts_snapshots_and_duplicate_histories() {
    let mut env = setup();
    let repo = env.repo.clone();
    let rs = repo.display().to_string();
    let base = 1789603200i64;
    let date = |t| {
        format!(
            "2026-09-17T00:{:02}:{:02}Z",
            (t - base) / 60,
            (t - base) % 60
        )
    };
    fs::write(repo.join("README.md"), "Native format integration\n").unwrap();
    commit(&repo, "base", "me@example.com", &date(base));
    let names = [
        "posit-assistant",
        "tinycloud",
        "zencoder",
        "codestudio",
        "terramind",
        "zenflow",
    ];
    let mut code = HashMap::new();
    for name in names {
        let text = (0..8)
            .map(|i| format!("export const {}_{i} = {i} + 101;\n", name.replace('-', "_")))
            .collect::<String>();
        fs::write(repo.join(format!("{name}.ts")), &text).unwrap();
        code.insert(name, text);
        env.vars.insert(
            format!("CREDITIA_{}_DIR", name.to_uppercase().replace('-', "_")),
            env.root.join(name).display().to_string(),
        );
    }
    commit(&repo, "agent changes", "me@example.com", &date(base + 60));
    let path = |name: &str| repo.join(format!("{name}.ts")).display().to_string();
    let ms = (base + 1) * 1000;
    let pp = env
        .root
        .join("posit-assistant/workspace/conversations/s/conversation.json");
    write(
        env.root.join("posit-assistant/workspace/workspace.json"),
        &json!({"path":rs}),
    );
    let mut posit = json!({"schemaVersion":"3","root":{"id":"posit-session","timestamp":ms},"messages":[{"id":"a","parentId":"posit-session","isActive":true,"timestamp":ms,"lmMessageIds":[0,1]}]});
    let mut plog = vec![
        json!({"id":0,"message":{"role":"assistant","providerOptions":{"providerMetadata":{"positai":{"modelId":"posit-model"}}},"content":[{"type":"tool-call","toolCallId":"p-write","toolName":"write","input":{"file_path":"posit-assistant.ts","content":code["posit-assistant"]}}]}}),
        json!({"id":1,"message":{"role":"tool","content":[{"type":"tool-result","toolCallId":"p-write","toolName":"write","output":{"type":"text","value":"Created file: posit-assistant.ts"}}]}}),
    ];
    write(&pp, &posit);
    jsonl(pp.with_file_name("lm-messages.jsonl"), &plog);
    let tp = env
        .root
        .join("tinycloud/projects/repo/sessions/tiny-session.jsonl");
    let mut tiny = vec![
        json!({"id":"a","timestamp":ms,"message":{"role":"assistant","model":"tiny-model","content":[{"type":"toolCall","id":"t-write","name":"write","arguments":{"path":path("tinycloud"),"content":code["tinycloud"]}}]}}),
        json!({"id":"b","timestamp":ms,"message":{"role":"toolResult","toolCallId":"t-write","toolName":"write","isError":false,"details":{"path":path("tinycloud"),"bytes":code["tinycloud"].len()},"content":[{"type":"text","text":format!("Wrote {} bytes to {}",code["tinycloud"].len(),path("tinycloud"))}]}}),
    ];
    jsonl(&tp, &tiny);
    let zp = env.root.join("zencoder/sessions/zen-session.jsonl");
    let diff = format!(
        "--- /dev/null\n+++ zencoder.ts\n@@ -0,0 +1,8 @@\n{}",
        code["zencoder"]
            .lines()
            .map(|s| format!("+{s}\n"))
            .collect::<String>()
    );
    let mut zen = vec![
        json!({"version":"1.0","id":"zen-session","createdAt":date(base),"modelId":"mutable-wrong"}),
        json!({"role":"assistant","createdAt":date(base+1),"modelDetails":{"model":{"id":"zen-model"}},"content":[{"type":"tool-call","toolCallId":"z-write","toolName":"Write","input":{"file_path":path("zencoder"),"content":"preview is not actual output"}}]}),
        json!({"role":"tool","createdAt":date(base+2),"content":[{"type":"tool-result","toolCallId":"z-write","toolName":"Write","isError":false,"content":[{"type":"diff","path":path("zencoder"),"diff":diff}]}]}),
    ];
    jsonl(&zp, &zen);
    let cp = env
        .root
        .join("codestudio/workspaceStorage/ws/chatEditingSessions/studio-session/state.json");
    let cd = cp.parent().unwrap();
    write(
        env.root
            .join("codestudio/workspaceStorage/ws/workspace.json"),
        &json!({"folder":format!("file://{rs}")}),
    );
    write(
        env.root
            .join("codestudio/workspaceStorage/ws/chatSessions/studio-session.json"),
        &json!({"requests":[{"requestId":"r1","timestamp":ms,"modelId":"studio-first"},{"requestId":"r2","timestamp":ms+1000,"modelId":"studio-second"}]}),
    );
    fs::create_dir_all(cd.join("contents")).unwrap();
    fs::write(cd.join("contents/0000000"), "").unwrap();
    fs::write(cd.join("contents/1111111"), &code["codestudio"]).unwrap();
    let uri = json!({"scheme":"file","authority":"","path":path("codestudio")});
    let lines = code["codestudio"].lines().collect::<Vec<_>>();
    let first = lines[..4].join("\n") + "\n";
    let second = lines[4..].join("\n") + "\n";
    let mut studio = json!({"version":2,"initialFileContents":[[format!("file://{}",path("codestudio")),"0000000"]],"recentSnapshot":{"entries":[{"resource":uri,"state":1,"currentHash":"1111111"}]},"timeline":{"currentEpoch":3,"operations":[{"type":"create","uri":uri,"epoch":0,"initialContent":"","requestId":"r1"},{"type":"textEdit","uri":uri,"epoch":1,"requestId":"r1","edits":[{"range":{"startLineNumber":1,"startColumn":1,"endLineNumber":1,"endColumn":1},"text":first}]},{"type":"textEdit","uri":uri,"epoch":2,"requestId":"r2","edits":[{"range":{"startLineNumber":5,"startColumn":1,"endLineNumber":5,"endColumn":1},"text":second}]}]}});
    write(&cp, &studio);
    let td=db(env.root.join("terramind/data/agents.db"),"create table projects(id text,path text);create table chats(id text,project_id text,worktree_path text);create table sub_chats(id text,chat_id text,messages text,execution_env text,created_at integer,updated_at integer);create table harness_tool_executions(id text,sub_chat_id text,step_id text,tool_call_id text,registration_id text,normalized_arguments text,output text,started_at integer,settled_at integer,location text,state text);");
    td.execute("insert into projects values('p',?)", [&rs])
        .unwrap();
    td.execute("insert into chats values('c','p',null)", [])
        .unwrap();
    td.execute(
        "insert into sub_chats values('terra-session','c',?,'local',?,?)",
        rusqlite::params![
            json!([{"id":"terra-step","role":"assistant","metadata":{"model":"terra-model"}}])
                .to_string(),
            base,
            base + 2
        ],
    )
    .unwrap();
    td.execute("insert into harness_tool_executions values('j','terra-session','terra-step','t','native:Write',?,?,?,?, 'local','settled')",rusqlite::params![json!({"file_path":path("terramind"),"content":code["terramind"]}).to_string(),format!("Successfully wrote {} bytes to {}",code["terramind"].len(),path("terramind")),ms,ms+1]).unwrap();
    let zd=db(env.root.join("zenflow/db.sqlite"),"create table projects(id blob,git_repo_path text);create table tasks(id blob,project_id blob,container_ref text);create table execution_processes(id blob,task_id blob,started_at text,completed_at text,run_reason text,dropped integer);create table execution_process_normalized_logs(execution_id blob,logs text);create table executor_sessions(execution_process_id blob,session_id text);");
    zd.execute("insert into projects values(x'01',?)", [&rs])
        .unwrap();
    zd.execute("insert into tasks values(x'02',x'01',null)", [])
        .unwrap();
    zd.execute(
        "insert into execution_processes values(x'03',x'02',?,?,'codingagent',0)",
        [date(base + 1), date(base + 2)],
    )
    .unwrap();
    zd.execute(
        "insert into executor_sessions values(x'03','flow-session')",
        [],
    )
    .unwrap();
    let entry = json!({"type":"NORMALIZED_ENTRY","content":{"timestamp":date(base+1),"metadata":{"tool_call_id":"flow-write"},"entry_type":{"type":"tool_use","tool_name":"Write","status":{"status":"created"},"action_type":{"action":"file_edit","path":path("zenflow"),"changes":[{"action":"write","content":code["zenflow"]}]}}}});
    let flow=vec![json!({"JsonPatch":[{"op":"add","path":"/entries/-","value":entry}]}),json!({"JsonPatch":[{"op":"replace","path":"/entries/0/content/entry_type/status","value":{"status":"success"}}]})].into_iter().map(|v|v.to_string()).collect::<Vec<_>>().join("\n");
    zd.execute(
        "insert into execution_process_normalized_logs values(x'03',?)",
        [&flow],
    )
    .unwrap();
    assert!(run(&env, &["init", "--repo", &rs]).0);
    let counts = || {
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        names.map(|src| {
            db.query_row("select count(*) from evidence where src=?", [src], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
        })
    };
    for round in 0..3 {
        if round == 1 {
            // Native V3 -> official V2 export shape, same conversation identity.
            posit["schemaVersion"] = json!("2");
            posit["root"]["activeChildId"] = json!("a");
            posit["messages"] = json!([{"id":"a","timestamp":ms,"message":{"lmMessage":[plog[0]["message"],plog[1]["message"]]}}]);
            write(&pp, &posit);
            let mut exported = posit.clone();
            exported["root"]["id"] = json!("posit-export-new-id");
            write(
                repo.join(".creditia/imports/posit-assistant/export.json"),
                &exported,
            );
            let mut fork = zen.clone();
            fork[0]["id"] = json!("zen-fork");
            fork[0]["parentId"] = json!("zen-session");
            jsonl(env.root.join("zencoder/sessions/zen-fork.jsonl"), &fork);
            fs::create_dir_all(env.root.join("tinycloud/sessions")).unwrap();
            fs::copy(&tp, env.root.join("tinycloud/sessions/tiny-session.jsonl")).unwrap();
        }
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(counts(), [8; 6], "round {round}: {v}");
        let credits: Value =
            serde_json::from_slice(&fs::read(repo.join(".creditia/credits.json")).unwrap())
                .unwrap();
        for name in names {
            let total: i64 = credits["breakdown"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|b| {
                    b["contributor"].as_str().is_some_and(|s| {
                        s == format!("a:{name}") || s.starts_with(&format!("a:{name}/"))
                    })
                })
                .map(|b| {
                    b["byEvidence"]["verified"]["linesAdded"]
                        .as_i64()
                        .unwrap_or(0)
                })
                .sum();
            assert_eq!(total, 8, "{name}: {credits}");
        }
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let mut q=db.prepare("select model,count(*) from evidence where src='codestudio' group by model order by model").unwrap();
        let models = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            models,
            vec![("studio-first".into(), 4), ("studio-second".into(), 4)]
        );
    }
    // Dropped executions revoke Zenflow evidence; restoring the execution restores it.
    zd.execute("update execution_processes set dropped=1", [])
        .unwrap();
    assert!(run(&env, &["analyze", "--repo", &rs]).0);
    assert_eq!(counts(), [8, 8, 8, 8, 8, 0]);
    zd.execute("update execution_processes set dropped=0", [])
        .unwrap();
    assert!(run(&env, &["analyze", "--repo", &rs]).0);
    assert_eq!(counts(), [8; 6]);
    // Malformed active data/snapshot must retain prior evidence and surface a warning.
    fs::write(&pp, "{").unwrap();
    fs::write(cd.join("contents/1111111"), "manual mismatch").unwrap();
    assert!(run(&env, &["analyze", "--repo", &rs]).0);
    assert_eq!(counts(), [8; 6]);
    // Fully readable failure/rejection snapshots retract successful evidence on the next run.
    plog[1]["message"]["content"][0]["output"]["type"] = json!("error-text");
    posit["messages"][0]["message"]["lmMessage"][1] = plog[1]["message"].clone();
    write(&pp, &posit);
    let mut exported = posit.clone();
    exported["root"]["id"] = json!("posit-export-new-id");
    write(
        repo.join(".creditia/imports/posit-assistant/export.json"),
        &exported,
    );
    tiny[1]["message"]["isError"] = json!(true);
    jsonl(&tp, &tiny);
    jsonl(
        env.root.join("tinycloud/sessions/tiny-session.jsonl"),
        &tiny,
    );
    zen[2]["content"][0]["isError"] = json!(true);
    jsonl(&zp, &zen);
    zen[0]["id"] = json!("zen-fork");
    jsonl(env.root.join("zencoder/sessions/zen-fork.jsonl"), &zen);
    studio["recentSnapshot"]["entries"][0]["state"] = json!(2);
    write(&cp, &studio);
    td.execute("update harness_tool_executions set state='uncertain'", [])
        .unwrap();
    zd.execute("update execution_process_normalized_logs set logs=?",[format!("{flow}\n{}",json!({"JsonPatch":[{"op":"replace","path":"/entries/0/content/entry_type/status","value":{"status":"failed"}}]}))]).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(counts(), [0; 6], "{v}");
    drop((td, zd));
    fs::remove_dir_all(env.root).unwrap();
}
