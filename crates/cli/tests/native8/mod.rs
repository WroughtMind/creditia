//! Official package formats, reconstructed without invoking a live model.
//! Exercises native files -> CLI -> durable evidence -> published credits data.
use super::*;
use serde_json::{json, Value};

fn code(prefix: &str) -> String {
    (0..8)
        .map(|i| format!("export const {prefix}_{i} = {i} + 891;\n"))
        .collect()
}
fn assert_lines(env: &Env, src: &str, model: &str, expected: i64) {
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    let count: i64 = db
        .query_row("select count(*) from evidence where src=?", [src], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, expected, "{src} durable evidence");
    let v: Value =
        serde_json::from_slice(&fs::read(env.repo.join(".creditia/credits.json")).unwrap())
            .unwrap();
    let key = if model.is_empty() {
        format!("a:{src}")
    } else {
        format!("a:{src}/{model}")
    };
    let lines: i64 = v["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["contributor"] == key)
        .map(|b| {
            b["byEvidence"]["verified"]["linesAdded"]
                .as_i64()
                .unwrap_or(0)
        })
        .sum();
    assert_eq!(lines, expected, "{src} final credits");
}

#[test]
fn firebender_native_database_subagents_forks_and_terminal_reconciliation() {
    let env = setup();
    let rs = env.repo.display().to_string();
    fs::write(env.repo.join("README.md"), "base\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let main = code("firebender");
    let child = code("firebender_child");
    fs::write(env.repo.join("fire.ts"), &main).unwrap();
    fs::write(env.repo.join("child.ts"), &child).unwrap();
    commit(&env.repo, "saved", "me@example.com", "2026-09-17T00:01:00Z");
    let path = env.repo.join(".idea/firebender/chat_history.db");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("create table schema_info(key text,value text);insert into schema_info values('version','2');create table chat_sessions(id text primary key,name text,created_at integer,updated_at integer,deleted_at integer,messages_json text,metadata_json text);create table subagent_conversations(tool_call_id text,session_id text,subagent_type text,description text,messages_json text,created_at integer,updated_at integer);").unwrap();
    let msgs = |cid: &str, file: &str, text: &str| {
        json!([
            {"role":"user","content":"make changes"},
            {"role":"assistant","tool_calls":[{"id":cid,"type":"function","function":{"name":"Write","arguments":json!({"file_path":file,"content":text}).to_string()}}]},
            {"role":"tool","name":"write","tool_call_id":cid,"content":format!("Wrote contents to {rs}/{file}.\n\nWhen you're done with your current set of changes to this file, you should call the read_lints tool with the specific file path and fix any newly introduced errors.")}
        ])
    };
    let original = msgs("write", "fire.ts", &main);
    let ms = 1789603201000i64;
    db.execute(
        "insert into chat_sessions values('s','name',?,?,null,?,'{}')",
        rusqlite::params![ms, ms + 2, original.to_string()],
    )
    .unwrap();
    db.execute(
        "insert into subagent_conversations values('spawn','s','task','description',?,?,?)",
        rusqlite::params![
            msgs("child-write", "child.ts", &child).to_string(),
            ms,
            ms + 1
        ],
    )
    .unwrap();
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for round in 0..5 {
        match round {
            1 => {
                db.execute(
                    "insert into chat_sessions values('fork','name',?,?,null,?,'{}')",
                    rusqlite::params![ms, ms + 3, original.to_string()],
                )
                .unwrap();
            }
            2 => {
                db.execute(
                    "update chat_sessions set messages_json='{' where id='fork'",
                    [],
                )
                .unwrap();
            }
            3 => {
                let mut failed = original.clone();
                failed.as_array_mut().unwrap().push(json!({"role":"tool","name":"write","tool_call_id":"write","failed":true,"content":"cancelled"}));
                db.execute(
                    "update chat_sessions set messages_json=?,updated_at=? where id='fork'",
                    rusqlite::params![failed.to_string(), ms + 4],
                )
                .unwrap();
            }
            4 => {
                db.execute("update chat_sessions set deleted_at=?", [ms + 5])
                    .unwrap();
            }
            _ => {}
        }
        let snapshot = fs::read(&path).unwrap();
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(
            fs::read(&path).unwrap(),
            snapshot,
            "native history is read-only"
        );
        assert_lines(
            &env,
            "firebender",
            "",
            match round {
                3 => 8,
                4 => 0,
                _ => 16,
            },
        );
    }
    drop(db);
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn moxby_native_receipt_joins_reject_stale_failed_and_foreign_results() {
    let mut env = setup();
    let rs = env.repo.display().to_string();
    fs::write(env.repo.join("README.md"), "base\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let text = code("moxby");
    fs::write(env.repo.join("native.ts"), &text).unwrap();
    commit(&env.repo, "saved", "me@example.com", "2026-09-17T00:01:00Z");
    let dir = env.root.join("moxby");
    fs::create_dir_all(&dir).unwrap();
    env.vars
        .insert("CREDITIA_MOXBY_DIR".into(), dir.display().to_string());
    let db = rusqlite::Connection::open(dir.join("moxby_chats.db")).unwrap();
    db.execute_batch("create table chat_threads(id text,chat_id text,created_at integer,updated_at integer);create table chat_turns(id text,chat_id text,thread_id text,model text);create table tool_calls(id text,chat_id text,thread_id text,tool_id text,result_event_id text,latest_event_id text,module_id text,harness_id text);create table tool_results(id text,tool_call_id text,chat_id text,thread_id text,turn_id text,event_id text,status text,result_json text,created_at integer,truncated integer,render_contract_id text,render_contract_version text);insert into chat_threads values('thread','chat',1789603201000,1789603202000);insert into chat_turns values('turn','chat','thread','moxby-model');").unwrap();
    let raw = json!({"path":env.repo.join("native.ts").display().to_string(),"status":"applied","replaced":1,"oldText":"","newText":text});
    for i in 0..7 {
        db.execute(
            "insert into tool_calls values(?,'chat','thread','file_edit',?,?,null,null)",
            rusqlite::params![format!("c{i}"), format!("r{i}"), format!("r{i}")],
        )
        .unwrap();
        db.execute("insert into tool_results values(?,?,'chat','thread','turn',?,'done',?,1789603202000,0,'file.edit.result','1')",rusqlite::params![format!("row{i}"),format!("c{i}"),format!("r{i}"),raw.to_string()]).unwrap();
    }
    db.execute_batch("update tool_calls set latest_event_id='pending' where id='c1';update tool_results set status='error' where tool_call_id='c2';update tool_results set truncated=1 where tool_call_id='c3';update tool_results set chat_id='other' where tool_call_id='c4';update tool_calls set module_id='external-mod' where id='c5';update tool_calls set harness_id='codex-cli' where id='c6';").unwrap();
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for round in 0..4 {
        if round == 1 {
            db.execute(
                "update tool_results set result_json='{' where tool_call_id='c0'",
                [],
            )
            .unwrap();
        }
        if round == 2 {
            db.execute(
                "update tool_results set result_json=?,status='error' where tool_call_id='c0'",
                [raw.to_string()],
            )
            .unwrap();
        }
        if round == 3 {
            db.execute("delete from tool_results where tool_call_id='c0'", [])
                .unwrap();
        }
        let snapshot = fs::read(dir.join("moxby_chats.db")).unwrap();
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(
            fs::read(dir.join("moxby_chats.db")).unwrap(),
            snapshot,
            "native history is read-only"
        );
        assert_lines(&env, "moxby", "moxby-model", if round < 2 { 8 } else { 0 });
    }
    drop(db);
    fs::remove_dir_all(env.root).unwrap();
}
