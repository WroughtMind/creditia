//! Native persistence fixtures follow the pinned serializers in SUPPORT_AGENT31.md.
//! Exercise the real binary, SQLite evidence store and final credits, not parser counts alone.
use super::*;
use serde_json::{json, Value};

fn write(path: impl AsRef<Path>, value: &Value) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn jsonl(path: impl AsRef<Path>, rows: &[Value]) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
}
fn db(path: impl AsRef<Path>, schema: &str) -> rusqlite::Connection {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(schema).unwrap();
    db
}
fn fxlog(path: impl AsRef<Path>, rows: &[Value]) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut output = String::new();
    for (i, row) in rows.iter().enumerate() {
        let mut row = row.clone();
        row["v"] = json!(1);
        row["seq"] = json!(i + 1);
        let mut text = row.to_string();
        text.pop();
        let mut crc = !0u32;
        for b in text.bytes() {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0x82f63b78u32.wrapping_mul(crc & 1));
            }
        }
        output.push_str(&format!("{text},\"crc\":\"{:08x}\"}}\n", !crc));
    }
    fs::write(path, output).unwrap();
}

fn msgpack(value: &Value) -> Vec<u8> {
    fn packed(v: &Value) -> rmpv::Value {
        match v {
            Value::Null => rmpv::Value::Nil,
            Value::Bool(v) => (*v).into(),
            Value::Number(v) => v.as_i64().unwrap().into(),
            Value::String(v) => v.as_str().into(),
            Value::Array(v) => rmpv::Value::Array(v.iter().map(packed).collect()),
            Value::Object(v) => {
                let fields = rmpv::Value::Map(
                    v.iter()
                        .map(|(k, v)| (k.as_str().into(), packed(v)))
                        .collect(),
                );
                if matches!(v.get("type").and_then(Value::as_str), Some("ai" | "tool")) {
                    let (module, name) = if v["type"] == "ai" {
                        ("ai", "AIMessage")
                    } else {
                        ("tool", "ToolMessage")
                    };
                    let tuple = rmpv::Value::Array(vec![
                        format!("langchain_core.messages.{module}").into(),
                        name.into(),
                        fields,
                        "model_validate_json".into(),
                    ]);
                    let mut encoded = vec![];
                    rmpv::encode::write_value(&mut encoded, &tuple).unwrap();
                    rmpv::Value::Ext(5, encoded)
                } else {
                    fields
                }
            }
        }
    }
    let mut encoded = vec![];
    rmpv::encode::write_value(&mut encoded, &packed(value)).unwrap();
    encoded
}

fn gzip(path: impl AsRef<Path>, rows: &[Value]) {
    use std::io::Write;
    let mut output = flate2::write::GzEncoder::new(
        fs::File::create(path).unwrap(),
        flate2::Compression::default(),
    );
    output
        .write_all(
            rows.iter()
                .map(|r| format!("{r}\n"))
                .collect::<String>()
                .as_bytes(),
        )
        .unwrap();
    output.finish().unwrap();
}

#[test]
fn native31_histories_reach_persisted_and_final_credits_then_retract_failed_receipts() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base = 1_789_603_200_i64;
    let date = |t| format!("@{t} +0000");
    fs::write(repo.join("README.md"), "# Native history integration\n").unwrap();
    commit(&repo, "base", "me@example.com", &date(base - 60));
    let names = [
        "aider-desk",
        "dexto",
        "pochi",
        "forgecode",
        "kode",
        "mux",
        "reasonix",
        "jazz",
        "deepagents",
        "fx",
        "astrbot",
        "codearts-agent",
        "adal",
    ];
    let codes: HashMap<_, _> = names
        .iter()
        .map(|name| {
            (
                *name,
                (0..8)
                    .map(|n| format!("export const {}_{n} = {n} + 100;\n", name.replace('-', "_")))
                    .collect::<String>(),
            )
        })
        .collect();
    for name in names {
        fs::write(repo.join(format!("{name}.ts")), &codes[name]).unwrap();
    }
    commit(&repo, "native changes", "me@example.com", &date(base + 60));
    let patch = |name: &str| {
        format!(
            "--- /dev/null\n+++ {name}.ts\n@@ -0,0 +1,8 @@\n{}",
            codes[name]
                .lines()
                .map(|l| format!("+{l}\n"))
                .collect::<String>()
        )
    };
    let ms = (base + 1) * 1000;

    let ap = repo.join(".aider-desk/tasks/a/context.json");
    write(
        ap.with_file_name("settings.json"),
        &json!({"model":"mutable-model"}),
    );
    let mut aider = json!({"version":2,"contextMessages":[{"role":"assistant","timestamp":ms,"usageReport":{"model":"aider-model"},"content":[{"type":"tool-call","toolCallId":"c","toolName":"power---file_write","input":{"filePath":"aider-desk.ts","content":codes["aider-desk"]}}]},{"role":"tool","timestamp":ms+1000,"content":[{"type":"tool-result","toolCallId":"c","toolName":"power---file_write","output":{"type":"text","value":"Successfully created 'aider-desk.ts'."}}]}]});
    write(&ap, &aider);

    let dd=db(env.root.join(".dexto/database/default.db"),"create table kv_store(key text primary key,value text);create table list_store(key text,sequence integer,value text);");
    dd.execute(
        "insert into kv_store values('session:d',?)",
        [json!({"id":"d","workspaceId":"w","createdAt":base*1000}).to_string()],
    )
    .unwrap();
    dd.execute(
        "insert into kv_store values('workspace:item:w',?)",
        [json!({"path":rs}).to_string()],
    )
    .unwrap();
    let dc = json!({"role":"assistant","model":"dexto-model","timestamp":ms,"toolCalls":[{"id":"c","function":{"name":"write_file","arguments":json!({"file_path":"dexto.ts","content":codes["dexto"]}).to_string()}}]});
    let mut dr = json!({"role":"tool","name":"write_file","toolCallId":"c","success":true,"timestamp":ms+1000,"content":[{"type":"text","text":json!({"success":true,"path":"dexto.ts","bytes_written":codes["dexto"].len()}).to_string()}]});
    for (i, v) in [&dc, &dr].into_iter().enumerate() {
        dd.execute(
            "insert into list_store values('messages:d',?,?)",
            rusqlite::params![i, v.to_string()],
        )
        .unwrap();
    }

    let pd=db(env.root.join(".pochi/materialized.db"),"create table tasks(id text,cwd text,parentId text,createdAt integer,updatedAt integer);create table messages(taskId text,data text);");
    pd.execute(
        "insert into tasks values('p',?,null,?,?)",
        rusqlite::params![rs, base * 1000, ms + 1000],
    )
    .unwrap();
    let mut pochi = json!({"role":"assistant","metadata":{"finishedAt":ms+1000},"parts":[{"type":"tool-writeToFile","state":"output-available","toolCallId":"c","input":{"path":"pochi.ts","content":"this proposal is not evidence"},"output":{"success":true,"_meta":{"edit":patch("pochi")}}}]});
    pd.execute("insert into messages values('p',?)", [pochi.to_string()])
        .unwrap();
    // Pochi root forks have no parentId, but keep the old message timestamps.
    pd.execute(
        "insert into tasks values('fork',?,null,?,?)",
        rusqlite::params![rs, ms + 10000, ms + 10000],
    )
    .unwrap();
    pd.execute("insert into messages values('fork',?)", [pochi.to_string()])
        .unwrap();
    fs::copy(
        env.root.join(".pochi/materialized.db"),
        env.root.join(".pochi/backup.sqlite"),
    )
    .unwrap();

    let fd = db(
        env.root.join(".forge/.forge.db"),
        "create table conversations(conversation_id text,created_at text,context text);",
    );
    let mut forge = json!({"messages":[{"message":{"text":{"role":"assistant","model":"forge-model","tool_calls":[{"call_id":"c","name":"fs_write","arguments":{"file_path":format!("{rs}/forgecode.ts"),"content":codes["forgecode"]}}]}}},{"message":{"tool":{"call_id":"c","name":"fs_write","output":{"is_error":false,"values":[{"text":format!("<file_created path=\"{rs}/forgecode.ts\">Created file</file_created>")}]}}}}]});
    fd.execute(
        "insert into conversations values('f',datetime(?,'unixepoch'),?)",
        rusqlite::params![base + 1, forge.to_string()],
    )
    .unwrap();

    let kp = env.root.join(".kode/projects/demo/k.jsonl");
    let mut kode = vec![
        json!({"sessionId":"k","cwd":rs,"type":"assistant","timestamp":ms,"message":{"model":"kode-model","content":[{"type":"tool_use","id":"c","name":"Write","input":{"file_path":"kode.ts","content":"proposal"}}]}}),
        json!({"sessionId":"k","cwd":rs,"type":"user","timestamp":ms+1000,"message":{"content":[{"type":"tool_result","tool_use_id":"c","is_error":false}]},"toolUseResult":{"type":"create","filePath":"kode.ts","content":codes["kode"]}}),
    ];
    let first_half = codes["kode"].lines().take(4).collect::<Vec<_>>().join("\n");
    let last_half = codes["kode"]
        .lines()
        .skip(4)
        .map(|l| format!("+{l}"))
        .collect::<Vec<_>>();
    kode[1]["toolUseResult"]["content"] = json!(first_half);
    let mut kode_child = kode.clone();
    for row in &mut kode_child {
        row["agentId"] = json!("worker");
        row["isSidechain"] = json!(true);
    }
    kode_child[0]["message"]["content"][0]["name"] = json!("Edit");
    kode_child[1]["toolUseResult"] =
        json!({"filePath":"kode.ts","structuredPatch":[{"lines":last_half}]});
    let kchild = kp.parent().unwrap().join("k/subagents/agent-worker.jsonl");
    jsonl(&kp, &kode);
    jsonl(&kchild, &kode_child);

    let mp = env.root.join(".mux/sessions/m/chat.jsonl");
    write(
        env.root.join(".mux/config.json"),
        &json!({"projects":[[rs,{"workspaces":[{"id":"m","path":rs}]}]]}),
    );
    let mut mux = json!({"id":"m1","role":"assistant","metadata":{"model":"mux-model","timestamp":ms},"parts":[{"type":"dynamic-tool","state":"output-available","toolName":"file_edit_insert","input":{"path":"mux.ts"},"output":{"success":true,"ui_only":{"file_edit":{"diff":patch("mux")}}}}]});
    jsonl(&mp, &[mux.clone()]);
    jsonl(mp.with_file_name("chat-archive.jsonl"), &[mux.clone()]);
    // A copied fork transcript must not create a second set of edits.
    let fork = mp.parent().unwrap().parent().unwrap().join("fork");
    jsonl(fork.join("chat.jsonl"), &[mux.clone()]);
    write(
        fork.join("session-usage.json"),
        &json!({"version":1,"forkedAt":ms}),
    );

    let rp = env.root.join(".reasonix/sessions/r.jsonl");
    write(
        rp.with_extension("meta.json"),
        &json!({"workspace":rs,"created_at":base,"updated_at":base+2}),
    );
    let mut reasonix = vec![
        json!({"role":"assistant","modelRef":"reasonix-model","createdAt":base+1,"tool_calls":[{"id":"c","name":"write_file","arguments":json!({"path":"reasonix.ts","content":codes["reasonix"]}).to_string()}]}),
        json!({"role":"tool","tool_call_id":"c","createdAt":base+2,"content":format!("wrote {} bytes to {rs}/reasonix.ts",codes["reasonix"].len())}),
    ];
    jsonl(&rp, &reasonix);

    let jp = env.root.join(".jazz/history/a/j.jsonl");
    let mut jazz = vec![
        json!({"type":"conversation","version":4,"agentId":"a","conversationId":"j","startedAt":ms}),
        json!({"type":"message","id":"jc","at":ms,"message":{"role":"assistant","tool_calls":[{"id":"c","function":{"name":"edit_file","arguments":"{}"}}]}}),
        json!({"type":"message","id":"jr","at":ms+1000,"message":{"role":"tool","tool_call_id":"c","content":json!({"path":format!("{rs}/jazz.ts"),"diff":patch("jazz")}).to_string()}}),
    ];
    jsonl(&jp, &jazz);

    let deep=db(env.root.join(".deepagents/.state/sessions.db"),"create table checkpoints(thread_id text,checkpoint_ns text,checkpoint_id text,type text,checkpoint blob,metadata blob);create table writes(thread_id text,checkpoint_ns text,checkpoint_id text,task_id text,idx integer,channel text,type text,value blob);");
    let mut checkpoint = json!({"channel_values":{"messages":[{"id":"a","type":"ai","response_metadata":{"model_name":"deep-model"},"tool_calls":[{"id":"c","name":"write_file","args":{"file_path":"deepagents.ts","content":codes["deepagents"]}}]},{"id":"b","type":"tool","name":"write_file","tool_call_id":"c","status":"success","content":"Updated file deepagents.ts"}]}});
    deep.execute(
        "insert into checkpoints values('deep','','001','json',?,?)",
        rusqlite::params![
            serde_json::to_vec(&checkpoint).unwrap(),
            serde_json::to_vec(&json!({"cwd":rs,"updated_at":ms+1000})).unwrap()
        ],
    )
    .unwrap();

    let fp = env.root.join(".fx/sessions/v2/fx/log.jsonl");
    let mut fx = vec![
        json!({"kind":"session_created","id":"fx","workspace":rs,"ts":ms}),
        json!({"kind":"set","key":"prefs","value":{"model":"fx-model"}}),
        json!({"kind":"item","type":"tool_call","ts":ms,"data":{"call_id":"c","tool_name":"write_file","arguments_json":"{}"}}),
        json!({"kind":"item","type":"tool_result","ts":ms+1000,"data":{"call_id":"c","tool_name":"write_file","status":"success","committed_file_presentation":{"path":"fx.ts","after_content":codes["fx"],"previous_content":"","truncated":false}}}),
    ];
    fxlog(&fp, &fx);

    let astr = db(
        env.root.join(".astrbot/data/data_v4.db"),
        "create table conversations(conversation_id text,content text,updated_at text);",
    );
    let mut astrbot = vec![
        json!({"role":"assistant","tool_calls":[{"id":"c","function":{"name":"astrbot_file_write_tool","arguments":json!({"path":format!("{rs}/astrbot.ts"),"content":codes["astrbot"]}).to_string()}}]}),
        json!({"role":"tool","tool_call_id":"c","content":format!("File written successfully: {rs}/astrbot.ts")}),
    ];
    astr.execute(
        "insert into conversations values('astr',?,datetime(?,'unixepoch'))",
        rusqlite::params![serde_json::to_string(&astrbot).unwrap(), base + 2],
    )
    .unwrap();

    let ca=db(env.root.join(".codeartsdoer/cli-data/opencode.db"),"create table session(id text,directory text,time_created integer,time_updated integer);create table message(id text,session_id text,time_created integer,data text);create table part(id text,message_id text,session_id text,time_created integer,data text);");
    ca.execute(
        "insert into session values('ca',?,?,?)",
        rusqlite::params![rs, ms, ms + 1000],
    )
    .unwrap();
    ca.execute(
        "insert into message values('m','ca',?,?)",
        rusqlite::params![
            ms,
            json!({"role":"assistant","modelID":"codearts-model"}).to_string()
        ],
    )
    .unwrap();
    let mut capart = json!({"type":"tool","tool":"edit","state":{"status":"completed","input":{"filePath":"codearts-agent.ts"},"metadata":{"diff":patch("codearts-agent")},"time":{"end":ms+1000}}});
    ca.execute(
        "insert into part values('p','m','ca',?,?)",
        rusqlite::params![ms + 1000, capart.to_string()],
    )
    .unwrap();

    let adp = repo.join(".creditia/imports/adal/run.ndjson");
    let mut adal = vec![
        json!({"type":"tool_call","id":"c","name":"replace_by_string","args":{"file_path":"adal.ts","old_string":"old","new_string":codes["adal"]}}),
        json!({"type":"tool_result","id":"c","name":"replace_by_string","status":"completed","output":"Edited adal.ts"}),
        json!({"type":"complete","session_id":"adal","exit_code":0,"model":"mutable-model"}),
    ];
    jsonl(&adp, &adal);

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let count = |src: &str| {
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        db.query_row("select count(*) from evidence where src=?", [src], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
    };
    for round in 0..4 {
        if round == 1 {
            jsonl(
                rp.with_extension("events.jsonl"),
                &[
                    json!({"schema_version":1,"type":"replace","messages":[reasonix[0]]}),
                    json!({"schema_version":3,"type":"append","message_index":1,"messages":[reasonix[1]]}),
                ],
            );
            gzip(jp.with_extension("jsonl.gz"), &jazz);
            fs::remove_file(&jp).unwrap();
            deep.execute(
                "update checkpoints set type='msgpack',checkpoint=?",
                [msgpack(&checkpoint)],
            )
            .unwrap();
            use sha2::{Digest, Sha256};
            let bytes = serde_json::to_vec(&fx[3]["data"]).unwrap();
            let hash = format!("{:x}", Sha256::digest(&bytes));
            fs::create_dir_all(fp.parent().unwrap().join("blobs")).unwrap();
            fs::write(fp.parent().unwrap().join("blobs").join(&hash), bytes).unwrap();
            let mut external = fx.clone();
            external[3]["data"] = json!({"$blob":hash});
            fxlog(&fp, &external);
        } else if round == 2 {
            jsonl(
                rp.with_extension("events.jsonl"),
                &[
                    json!({"schema_version":2,"type":"message","id":"a","msgs":[reasonix[0]]}),
                    json!({"schema_version":2,"type":"message","id":"b","parent":"a","msgs":[reasonix[1]]}),
                ],
            );
        } else if round == 3 {
            jsonl(
                rp.with_extension("events.jsonl"),
                &[
                    json!({"schema_version":4,"type":"replace","messages":[reasonix[0]]}),
                    json!({"schema_version":5,"type":"append","message_index":1,"messages":[reasonix[1]]}),
                ],
            );
        }
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "round {round}: {v}");
        let credits: Value =
            serde_json::from_slice(&fs::read(repo.join(".creditia/credits.json")).unwrap())
                .unwrap();
        for src in names {
            assert_eq!(count(src), 8, "{src}, round {round}: {v}");
            let lines = credits["breakdown"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|b| {
                    b["contributor"].as_str().is_some_and(|id| {
                        id == format!("a:{src}") || id.starts_with(&format!("a:{src}/"))
                    })
                })
                .map(|b| {
                    b["byEvidence"]["verified"]["linesAdded"]
                        .as_u64()
                        .unwrap_or(0)
                })
                .sum::<u64>();
            assert_eq!(lines, 8, "{src}: {}", credits["breakdown"]);
        }
        assert!(!credits.to_string().contains("mutable-model"));
    }
    // A current native receipt changing to failure must retract old stored evidence.
    aider["contextMessages"][1]["content"][0]["output"]["value"] = json!("denied");
    write(&ap, &aider);
    dr["success"] = json!(false);
    dd.execute(
        "update list_store set value=? where sequence=1",
        [dr.to_string()],
    )
    .unwrap();
    pochi["parts"][0]["output"]["success"] = json!(false);
    // The current native completion is newer than the unchanged backup;
    // conflicting snapshots at the same native progress cannot establish retraction.
    pochi["metadata"]["finishedAt"] = json!(ms + 2000);
    pd.execute(
        "update messages set data=? where taskId='p'",
        [pochi.to_string()],
    )
    .unwrap();
    pd.execute("update tasks set updatedAt=? where id='p'", [ms + 2000])
        .unwrap();
    forge["messages"][1]["message"]["tool"]["output"]["is_error"] = json!(true);
    fd.execute("update conversations set context=?", [forge.to_string()])
        .unwrap();
    kode[1]["message"]["content"][0]["is_error"] = json!(true);
    jsonl(&kp, &kode);
    kode_child[1]["message"]["content"][0]["is_error"] = json!(true);
    jsonl(&kchild, &kode_child);
    mux["parts"][0]["output"]["success"] = json!(false);
    jsonl(&mp, &[mux]);
    reasonix[1]["tool_failure"] = json!({"error":"denied"});
    jsonl(
        rp.with_extension("events.jsonl"),
        &[json!({"schema_version":5,"type":"replace","messages":reasonix})],
    );
    jazz[2]["message"]["content"] = json!({"error":"denied"}).to_string().into();
    gzip(jp.with_extension("jsonl.gz"), &jazz);
    checkpoint["channel_values"]["messages"][1]["status"] = json!("error");
    deep.execute(
        "update checkpoints set type='json',checkpoint=?",
        [serde_json::to_vec(&checkpoint).unwrap()],
    )
    .unwrap();
    fx[3]["data"]["status"] = json!("failure");
    fxlog(&fp, &fx);
    astrbot[1]["content"] = json!("Error writing file: denied");
    astr.execute(
        "update conversations set content=?",
        [serde_json::to_string(&astrbot).unwrap()],
    )
    .unwrap();
    capart["state"]["status"] = json!("error");
    ca.execute("update part set data=?", [capart.to_string()])
        .unwrap();
    adal[1]["status"] = json!("denied");
    jsonl(&adp, &adal);
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    for src in names {
        assert_eq!(count(src), 0, "retracted {src}");
    }
    drop((dd, pd, fd, deep, astr, ca));
    fs::remove_dir_all(env.root).unwrap();
}
