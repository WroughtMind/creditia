//! Reconstructed fixtures from the native formats cited in SUPPORT_AGENT6.md.
//! Runs the actual CLI and checks durable evidence and final credits.
use super::*;
use serde_json::{json, Value};

fn code(prefix: &str) -> String {
    (0..8)
        .map(|i| format!("export const {prefix}_{i} = {i} + 739;\n"))
        .collect()
}
fn assert_lines(env: &Env, src: &str, model: &str, expected: i64) {
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    let count: i64 = db
        .query_row(
            "select count(*) from evidence where src=? and model=?",
            [src, model],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, expected, "{src}/{model} durable evidence");
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
    assert_eq!(lines, expected, "{src}/{model} final credits");
}

fn rovo_pair(cid: &str, tool: &str, args: Value, result: &str, model: &str) -> Vec<Value> {
    vec![
        json!({"kind":"response","model_name":model,"timestamp":"2026-09-17T00:00:01Z","parts":[{"part_kind":"tool-call","tool_name":tool,"tool_call_id":cid,"args":args.to_string()}]}),
        json!({"kind":"request","parts":[{"part_kind":"tool-return","tool_name":tool,"tool_call_id":cid,"content":result,"timestamp":"2026-09-17T00:00:02Z"}]}),
    ]
}

#[test]
fn rovodev_native_receipts_forks_pruning_and_reconciliation() {
    let env = setup();
    let rs = env.repo.display().to_string();
    fs::write(env.repo.join("README.md"), "base\n").unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let created = code("rovo_create");
    let edited = code("rovo_edit");
    let excluded = code("rovo_excluded");
    for (file, text) in [
        ("create.ts", &created),
        ("edit.ts", &edited),
        ("excluded.ts", &excluded),
    ] {
        fs::write(env.repo.join(file), text).unwrap();
    }
    commit(&env.repo, "saved", "me@example.com", "2026-09-17T00:01:00Z");
    // Native project-local sessions are discovered without an override.
    let dir = env.repo.join(".rovodev/sessions");
    let path = dir.join("original/session_context.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rows = rovo_pair(
        "create-call",
        "create_file",
        json!({"file_path":"create.ts","initial_content":format!("!U!SHALL!NOT!PARSE!{created}")}),
        "Successfully created create.ts.",
        "rovo-model-a",
    );
    let diff = format!(
        "Successfully replaced code in edit.ts:\n\n--- edit.ts\n+++ edit.ts\n@@ -0,0 +1,8 @@\n{}",
        edited
            .lines()
            .map(|l| format!("+{l}\n"))
            .collect::<String>()
    );
    rows.extend(rovo_pair(
        "edit-call",
        "find_and_replace_code",
        json!({"file_path":"edit.ts","find":"","replace":excluded}),
        &diff,
        "rovo-model-b",
    ));
    rows.extend(rovo_pair(
        "failed-call",
        "create_file",
        json!({"file_path":"excluded.ts","initial_content":excluded}),
        "File excluded.ts already exists. To overwrite, use overwrite=True.",
        "rovo-model-a",
    ));
    rows.extend(rovo_pair(
        "outside-call",
        "create_file",
        json!({"file_path":"../other/excluded.ts","initial_content":excluded}),
        "Successfully created ../other/excluded.ts.",
        "rovo-model-a",
    ));
    rows.extend(
        rovo_pair(
            "unpaired-call",
            "create_file",
            json!({"file_path":"excluded.ts","initial_content":excluded}),
            "Successfully created excluded.ts.",
            "rovo-model-a",
        )
        .into_iter()
        .take(1),
    );
    let original = json!({"id":"native-session","workspace_path":rs,"timestamp":1789603200,"prompts":["make changes"],"message_history":rows});
    fs::write(&path, original.to_string()).unwrap();
    assert!(run(&env, &["init", "--repo", &rs]).0);
    let fork = dir.join("fork/session_context.json");
    for round in 0..6 {
        match round {
            1 => {
                fs::create_dir_all(fork.parent().unwrap()).unwrap();
                let mut v = original.clone();
                v["id"] = json!("fork-session");
                fs::write(&fork, v.to_string()).unwrap();
            }
            2 => {
                // /prune removes tool results. This does not undo successful writes.
                let mut pruned = original.clone();
                pruned["message_history"] = json!([]);
                fs::write(&path, pruned.to_string()).unwrap();
                fs::remove_file(&fork).unwrap();
            }
            3 => {
                let mut broken = original.clone();
                broken["message_history"][0]["parts"][0]["args"] = json!("{");
                fs::write(&path, broken.to_string()).unwrap();
            }
            4 => {
                let mut failed = original.clone();
                failed["message_history"].as_array_mut().unwrap().push(json!({"kind":"request","parts":[{"part_kind":"retry-prompt","tool_name":"create_file","tool_call_id":"create-call","content":"write failed","timestamp":"2026-09-17T00:00:03Z"}]}));
                fs::write(&path, failed.to_string()).unwrap();
            }
            5 => {
                fs::remove_file(&path).unwrap();
            }
            _ => {}
        }
        let snapshot = fs::read(&path).ok();
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(
            fs::read(&path).ok(),
            snapshot,
            "native history is read-only"
        );
        assert_lines(
            &env,
            "rovodev",
            "rovo-model-a",
            if round < 4 { 8 } else { 0 },
        );
        assert_lines(&env, "rovodev", "rovo-model-b", 8);
    }
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn lingma_acceptance_requires_matching_native_content_versions() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let baseline = "export const human_baseline = 17;\n";
    fs::write(env.repo.join("saved.ts"), baseline).unwrap();
    commit(&env.repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let after = format!("{baseline}{}", code("lingma_saved"));
    let excluded = code("lingma_excluded");
    fs::write(env.repo.join("saved.ts"), &after).unwrap();
    fs::write(env.repo.join("excluded.ts"), &excluded).unwrap();
    commit(&env.repo, "saved", "me@example.com", "2026-09-17T00:01:00Z");
    let dir = env.root.join(".lingma/cache");
    fs::create_dir_all(dir.join("db")).unwrap();
    fs::create_dir_all(dir.join("workingSpace")).unwrap();
    let path = dir.join("db/local.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE chat_session(session_id text,project_uri text,gmt_modified integer);CREATE TABLE chat_working_space_file(item_id text primary key,session_id text,file_id text,content_type text,type text,key text,status text,version text,local_id text,extra text,gmt_create integer,gmt_modified integer);").unwrap();
    db.execute(
        "INSERT INTO chat_session VALUES('session',?,1789603202000)",
        [&rs],
    )
    .unwrap();
    let extra = json!({"CHAT_MODE":"agent","diffInfo":{"sourceMd5":"9d8852a497d594add100eb2eaee5f731","targetMd5":"0d6f7868d0844be42f6fc14fb3b1528d"}});
    for i in 0..10 {
        let id = format!("row{i}");
        let (file, text) = if i <= 1 {
            ("saved.ts", if i == 0 { baseline } else { &after })
        } else {
            ("excluded.ts", excluded.as_str())
        };
        let key = dir.join(format!("workingSpace/{id}__{file}"));
        fs::write(&key, text).unwrap();
        let mut e = if i <= 1 {
            extra.clone()
        } else {
            json!({"CHAT_MODE":"agent","diffInfo":{"sourceMd5":"d41d8cd98f00b204e9800998ecf8427e","targetMd5":"6bb5c079199fc3dc9deadbc857a38629"}})
        };
        if i == 5 {
            e["diffInfo"]["targetMd5"] = json!("mismatched");
        }
        if i == 6 {
            e["LINGMA_TAG::CONTENT_USE_WORKING_SPACE_LOCAL"] = json!(true);
        }
        if i == 7 {
            e["diffInfo"]["sourceMd5"] = json!("missing-base-version");
        }
        if i == 8 {
            e["CHAT_MODE"] = json!("edit");
        }
        if i == 9 {
            e["diffInfo"]["targetMd5"] = json!("6bb5c079199fc3dc9deadbc857a38629");
        }
        let native_key = if i == 9 {
            format!("C:\\old-cache\\other\\{id}__{file}")
        } else {
            format!("C:\\old-cache\\workingSpace\\{id}__{file}")
        };
        let destination = if i == 4 {
            env.root.join(file)
        } else {
            env.repo.join(file)
        };
        db.execute("INSERT INTO chat_working_space_file VALUES(?,'session',?,'FULL','LOCAL_FILE',?,?,?,?,?,1789603201000,1789603202000)",rusqlite::params![id,destination.display().to_string(),native_key,if i==2 {"APPLIED"} else if i==3 {"REJECTED"} else {"ACCEPTED"},if i==0 {"0"} else {"1"},if i==0 {""} else {"row0"},e.to_string()]).unwrap();
    }
    assert!(run(&env, &["init", "--repo", &rs]).0);
    let saved_cache = dir.join("workingSpace/row1__saved.ts");
    for round in 0..7 {
        match round {
            1 => {
                fs::write(&saved_cache, "later manual content").unwrap();
            }
            2 => {
                fs::remove_file(&saved_cache).unwrap();
            }
            3 => {
                fs::write(&saved_cache, &after).unwrap();
            }
            4 => {
                db.execute(
                    "UPDATE chat_working_space_file SET extra='{' WHERE item_id='row1'",
                    [],
                )
                .unwrap();
            }
            5 => {
                db.execute(
                    "UPDATE chat_working_space_file SET status='REJECTED' WHERE item_id='row1'",
                    [],
                )
                .unwrap();
            }
            6 => {
                db.execute(
                    "DELETE FROM chat_working_space_file WHERE item_id='row1'",
                    [],
                )
                .unwrap();
            }
            _ => {}
        }
        let db_before = fs::read(&path).unwrap();
        let cache_before = fs::read(&saved_cache).ok();
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(fs::read(&path).unwrap(), db_before);
        assert_eq!(fs::read(&saved_cache).ok(), cache_before);
        assert_lines(&env, "lingma", "", if round < 5 { 8 } else { 0 });
    }
    drop(db);
    fs::remove_dir_all(env.root).unwrap();
}

/// Optional acceptance against the byte-identical public native database.
/// Download instructions and SHA256s: SUPPORT_AGENT3.md. No chat text is read.
#[test]
#[ignore = "requires the pinned public Lingma database and workingSpace files"]
fn lingma_public_native_history_acceptance() {
    use sha2::{Digest, Sha256};
    let cache = PathBuf::from(
        std::env::var_os("CREDITIA_TEST_LINGMA_CACHE").expect("native cache directory"),
    );
    let original_db = fs::read(cache.join("db/local.db")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&original_db)),
        "aa5abdf2d05a3ca7eaf54134d3d4f91fa9388a3ba01f0729f272a1bb48c00f1a"
    );
    let snapshot =
        cache.join("workingSpace/846c1557-ab2b-4bd5-9dac-b96a66be3b98__persistent-memory.js");
    let original_content = fs::read(&snapshot).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&original_content)),
        "53b0da6da39c5ed978348606f105ebc4ab468006e697ec373b5b1e604d01abce"
    );
    let mut env = setup();
    let moved = env
        .repo
        .with_file_name("autonomous-elasticsearch-evolution-agent");
    fs::rename(&env.repo, &moved).unwrap();
    env.repo = moved;
    env.vars
        .insert("CREDITIA_LINGMA_DIR".into(), cache.display().to_string());
    let rs = env.repo.display().to_string();
    // Only the comparison Git repository is reconstructed. The database,
    // Windows paths, statuses, timestamps and native snapshot remain untouched.
    fs::write(env.repo.join("README.md"), "Native history acceptance\n").unwrap();
    commit(
        &env.repo,
        "before native edit",
        "me@example.com",
        "@1771593700 +0000",
    );
    fs::write(env.repo.join("persistent-memory.js"), &original_content).unwrap();
    commit(
        &env.repo,
        "native saved snapshot",
        "me@example.com",
        "@1771593810 +0000",
    );
    assert!(run(&env, &["init", "--repo", &rs]).0);
    for _ in 0..2 {
        let (ok, result) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{result}");
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let counts: (i64, i64) = db
            .query_row(
                "SELECT count(*),count(distinct session) FROM evidence WHERE src='lingma'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (308, 1));
        let credits: Value =
            serde_json::from_slice(&fs::read(env.repo.join(".creditia/credits.json")).unwrap())
                .unwrap();
        let lingma: Vec<_> = credits["breakdown"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["contributor"] == "a:lingma")
            .collect();
        assert_eq!(lingma.len(), 1);
        assert_eq!(lingma[0]["byEvidence"]["verified"]["linesAdded"], 119);
        assert_eq!(lingma[0]["byEvidence"]["declared"]["linesAdded"], 95);
        assert_eq!(fs::read(cache.join("db/local.db")).unwrap(), original_db);
        assert_eq!(fs::read(&snapshot).unwrap(), original_content);
    }
    fs::remove_dir_all(env.root).unwrap();
}
