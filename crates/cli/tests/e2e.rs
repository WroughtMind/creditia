//! 端到端：临时 HOME 里伪造 Codex / Claude Code / Cursor 日志，构造已知归因的测试仓库，跑真实二进制。
//! 所有数据源路径都通过环境变量重定向，不碰真实数据。

use creditia_core::hash::{ext_of, line_key, normalize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod native31;
mod native18;
mod native8;
mod native6;
mod native4;
mod native_hosts;

struct Env {
    root: PathBuf,
    repo: PathBuf,
    vars: HashMap<String, String>,
}

fn tmp() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("creditia-e2e-{}-{}-{id}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}

fn git(repo: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=Me", "-c", "user.email=me@example.com", "-c", "commit.gpgsign=false", "-c", "core.quotepath=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {:?} failed: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn commit(repo: &Path, msg: &str, email: &str, date: &str) -> String {
    commit_as(repo, msg, "X", email, date)
}

fn commit_as(repo: &Path, msg: &str, name: &str, email: &str, date: &str) -> String {
    git(repo, &["add", "-A"]);
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", &format!("user.name={name}"), "-c", &format!("user.email={email}"), "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg, "--date", date])
        .env("GIT_COMMITTER_DATE", date)
        // 装了盖章 hook 的仓库：提交时不去扫这台电脑上真实的 agent 日志
        .env("CREDITIA_HOOK_TIMEOUT", "0")
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_EMAIL", email)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    git(repo, &["rev-parse", "HEAD"])
}

fn setup() -> Env {
    let root = tmp();
    // 中文路径的仓库
    let repo = root.join("projects/示例项目");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    let mut vars = HashMap::new();
    vars.insert("HOME".into(), root.display().to_string());
    vars.insert("APPDATA".into(), root.join("AppData/Roaming").display().to_string());
    vars.insert("CREDITIA_HOME".into(), root.join("lib").display().to_string());
    vars.insert("CODEX_HOME".into(), root.join("codex").display().to_string());
    vars.insert("CLAUDE_CONFIG_DIR".into(), root.join("claude").display().to_string());
    vars.insert("CREDITIA_CURSOR_DIR".into(), root.join("cursor").display().to_string());
    vars.insert("CREDITIA_CURSOR_STORAGE".into(), root.join("cursor-storage").display().to_string());
    vars.insert("CREDITIA_GROK_DIR".into(), root.join("grok").display().to_string());
    vars.insert("CREDITIA_KIMI_DIR".into(), root.join("kimi").display().to_string());
    vars.insert("CREDITIA_GEMINI_DIR".into(), root.join("gemini").display().to_string());
    vars.insert("CREDITIA_QWEN_DIR".into(), root.join("qwen").display().to_string());
    vars.insert("CREDITIA_OPENCODE_DIR".into(), root.join("opencode").display().to_string());
    vars.insert("CREDITIA_VSCODE_STORAGE".into(), root.join("vscode").display().to_string());
    vars.insert("CREDITIA_GOOSE_DIR".into(), root.join("goose").display().to_string());
    vars.insert("CREDITIA_CONTINUE_DIR".into(), root.join("continue").display().to_string());
    vars.insert("CREDITIA_ZED_DIR".into(), root.join("zed").display().to_string());
    vars.insert("CREDITIA_AMAZONQ_DIR".into(), root.join("amazon-q").display().to_string());
    vars.insert("CREDITIA_ANTIGRAVITY_DIR".into(), root.join("agy").display().to_string());
    Env { root, repo, vars }
}

fn run(env: &Env, args: &[&str]) -> (bool, serde_json::Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(args).arg("--json").env_clear().envs(&env.vars).env("PATH", std::env::var("PATH").unwrap()).output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_else(|_| panic!("非 JSON 输出：{s}\n{}", String::from_utf8_lossy(&o.stderr)));
    (o.status.code() != Some(1), v)
}

#[test]
fn worktree_links_cannot_claim_to_unlink_an_automatic_path() {
    let env = setup();
    fs::write(env.repo.join("README.md"), "# worktree fixture\n").unwrap();
    commit(&env.repo, "init", "me@example.com", "2026-10-01T12:00:00+0800");
    let rs = env.repo.to_str().unwrap();
    let wt = env.root.join("linked-worktree");
    git(&env.repo, &["worktree", "add", "-b", "fixture", wt.to_str().unwrap()]);
    run(&env, &["init", "--repo", rs]);
    let (ok, v) = run(&env, &["analyze", "--repo", rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["link", wt.to_str().unwrap(), "--repo", rs]);
    assert!(ok, "{v}");
    let (_, v) = run(&env, &["paths", "--repo", rs]);
    assert!(v["paths"].as_array().unwrap().iter().any(|p| p["path"] == wt.canonicalize().unwrap().to_str().unwrap() && p["kind"] == "worktree"), "{v}");
    let (ok, v) = run(&env, &["unlink", wt.to_str().unwrap(), "--repo", rs]);
    assert!(!ok && v["error"].is_string(), "{v}");
}

#[test]
fn removed_forks_rejoin_their_own_project_identity() {
    let env = setup();
    fs::write(env.repo.join("README.md"), "# identity fixture\n").unwrap();
    commit(&env.repo, "init", "me@example.com", "2026-10-01T12:00:00+0800");
    git(&env.repo, &["remote", "add", "origin", "https://github.com/example/original.git"]);
    let fork = env.root.join("fork");
    git(&env.root, &["clone", env.repo.to_str().unwrap(), fork.to_str().unwrap()]);
    git(&fork, &["remote", "set-url", "origin", "https://github.com/example/fork.git"]);
    let analyze = |repo: &Path| {
        let (ok, v) = run(&env, &["analyze", "--repo", repo.to_str().unwrap()]);
        assert!(ok, "{v}");
    };
    for repo in [&env.repo, &fork] {
        let (ok, v) = run(&env, &["init", "--repo", repo.to_str().unwrap()]);
        assert!(ok, "{v}");
        analyze(repo);
    }
    let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
    let id = |repo: &Path| -> String { db.query_row("select id from projects where path=?1 and hidden=0", [repo.canonicalize().unwrap().to_str().unwrap()], |r| r.get(0)).unwrap() };
    let original_id = id(&env.repo);
    let fork_id = id(&fork);
    assert_ne!(original_id, fork_id);
    // Store unit tests exercise removal itself. Here run the real CLI/project_of restoration path.
    db.execute("update projects set hidden=1 where id=?1", [&original_id]).unwrap();
    analyze(&env.repo);
    assert_eq!(id(&env.repo), original_id);
    db.execute("update projects set hidden=1", []).unwrap();
    analyze(&fork);
    assert_eq!(id(&fork), fork_id);
    analyze(&env.repo);
    assert_eq!(id(&env.repo), original_id);
}

#[test]
fn public_agents_can_be_selected_launched_and_stamped_without_inventing_models() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let (ok, catalogue) = run(&env, &["agents"]);
    assert!(ok, "{catalogue}");
    assert_eq!(catalogue["count"], 78);
    for agent in catalogue["agents"].as_array().unwrap() {
        let id = agent["id"].as_str().unwrap();
        let expected = agent["harness"].as_str().unwrap();
        let (ok, identity) = run(&env, &["run", "--agent", id, "--", env!("CARGO_BIN_EXE_creditia-core"), "whoami"]);
        assert!(ok, "{id}: {identity}");
        assert_eq!(identity["agent"], expected, "{id}");
        assert!(identity["model"].is_null(), "{id}");
        let (ok, stamp) = run(&env, &["stamp", "--repo", &rs, "--agent", id]);
        assert!(ok, "{id}: {stamp}");
        assert_eq!(stamp["stamp"]["harness"], expected, "{id}");
        assert!(stamp["stamp"]["model"].is_null());
    }
    assert!(!env.repo.join(".git/hooks/prepare-commit-msg").exists());
    assert!(!env.root.join("lib/library.sqlite").exists());
    assert!(!run(&env, &["stamp", "--repo", &rs, "--agent", "not-registered"]).0);
    assert!(!run(&env, &["stamp", "--repo", &rs, "--agent", "codex", "--harness", "cursor"]).0);
    fs::remove_dir_all(env.root).unwrap();
}

#[cfg(unix)]
#[test]
fn run_preserves_stdio_arguments_exit_and_explicit_identity_over_parent_agent() {
    use std::io::Write;
    use std::process::Stdio;
    let env = setup();
    let mut child = Command::new(env!("CARGO_BIN_EXE_creditia-core"))
        .args(["run", "--agent", "deepagents", "--", "/bin/sh", "-c",
            "test \"$CREDITIA_HARNESS\" = deepagents && test -z \"$CREDITIA_MODEL\" || exit 99; cat; printf '%s' \"$1\"; printf 'child error' >&2; exit 7", "child", "--model=literal argument"])
        .env_clear().envs(&env.vars).env("PATH", std::env::var("PATH").unwrap())
        .env("CLAUDECODE", "1").env("CREDITIA_MODEL", "parent-model")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"input bytes\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(out.stdout, b"input bytes\n--model=literal argument");
    assert_eq!(out.stderr, b"child error");
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn hermes_openclaw_and_qoder_cn_native_history_reaches_final_attribution() {
    use serde_json::json;
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base = 1_789_603_200_i64;
    let date = |t| format!("@{t} +0000");
    fs::write(repo.join("README.md"), "# Native history fixture\n").unwrap();
    commit(&repo, "base", "me@example.com", &date(base - 60));
    let code = |tag: &str| (0..8).map(|n| format!("export const {tag}_{n} = {n} + 100;\n")).collect::<String>();
    let patch = |name: &str, text: &str| format!("--- /dev/null\n+++ {name}\n@@ -0,0 +1,8 @@\n{}", text.lines().map(|l| format!("+{l}\n")).collect::<String>());
    let htext = code("hermes"); let otext = code("openclaw"); let qtext = code("qoder_cn");
    for (name, text) in [("hermes.ts", &htext), ("openclaw.ts", &otext), ("qoder.ts", &qtext)] { fs::write(repo.join(name), text).unwrap(); }
    commit(&repo, "agent changes", "me@example.com", &date(base + 60));

    let hd = env.root.join(".hermes"); fs::create_dir_all(&hd).unwrap();
    let hdb = rusqlite::Connection::open(hd.join("state.db")).unwrap();
    hdb.execute_batch("create table sessions(id text,cwd text,started_at integer,model text); create table messages(id integer,session_id text,role text,content text,tool_calls text,tool_call_id text,timestamp integer,active integer,effect_disposition text);").unwrap();
    hdb.execute("insert into sessions values('hs',?1,?2,'mutable-final-model')", rusqlite::params![rs,base]).unwrap();
    let receipt = json!({"success":true,"resolved_path":format!("{rs}/hermes.ts"),"files_created":[format!("{rs}/hermes.ts")],"diff":patch("hermes.ts",&htext)});
    hdb.execute("insert into messages values(1,'hs','assistant',null,?1,null,?2,1,null)", rusqlite::params![json!([{"id":"hc","function":{"name":"patch"}}]).to_string(),base+1]).unwrap();
    hdb.execute("insert into messages values(2,'hs','tool',?1,null,'hc',?2,1,null)", rusqlite::params![receipt.to_string(),base+2]).unwrap();

    let od = env.root.join(".openclaw/agents/main/sessions"); fs::create_dir_all(&od).unwrap();
    let odb = rusqlite::Connection::open(od.join("openclaw-agent.sqlite")).unwrap();
    odb.execute_batch("create table transcript_events(session_id text,seq integer,event_json text,event_zstd blob,event_utf8_bytes integer);create table session_windows(session_id text,acp_owned integer);insert into session_windows values('os',0);").unwrap();
    let mut rows = [json!({"type":"session","id":"os","cwd":rs,"timestamp":base}),json!({"type":"message","id":"oa","timestamp":base+1,"message":{"role":"assistant","model":"actual-openclaw-model","content":[{"type":"toolCall","id":"oc","name":"edit"}]}}),json!({"type":"message","id":"or","timestamp":base+2,"message":{"role":"toolResult","toolCallId":"oc","isError":false,"details":{"changed":true,"patch":patch("openclaw.ts",&otext)}}})];
    for (seq,row) in rows.iter().enumerate() { odb.execute("insert into transcript_events values('os',?1,?2,null,null)",rusqlite::params![seq,row.to_string()]).unwrap(); }
    // This archive must not override the current database on a later retraction.
    fs::write(od.join("archived.jsonl"),rows.iter().map(|v|v.to_string()).collect::<Vec<_>>().join("\n")).unwrap();

    let qd=env.root.join("qoder-cn-custom/projects/demo");fs::create_dir_all(&qd).unwrap();
    env.vars.insert("QODERCN_CONFIG_DIR".into(),env.root.join("qoder-cn-custom").display().to_string());
    let qrows=[json!({"sessionId":"qs","cwd":rs,"type":"assistant","timestamp":base+1,"message":{"role":"assistant","model":"qoder-model","content":[{"type":"tool_use","id":"qc","name":"Edit","input":{"file_path":format!("{rs}/qoder.ts"),"old_string":"","new_string":qtext}}]}}),json!({"type":"user","timestamp":base+2,"message":{"content":[{"type":"tool_result","tool_use_id":"qc","is_error":false}]}})];
    fs::write(qd.join("qs.jsonl"),qrows.iter().map(|r|r.to_string()).collect::<Vec<_>>().join("\n")).unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]); assert!(ok,"{v}");
    let count = |source: &str| { let db=rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap(); db.query_row("select count(*) from evidence where src=?1",[source],|r|r.get::<_,i64>(0)).unwrap() };
    for round in 0..2 {
        let (ok,v)=run(&env,&["analyze","--repo",&rs]);assert!(ok,"{round}: {v}");
        for src in ["hermes-agent","openclaw","qoder"] {assert_eq!(count(src),8,"{src}, round {round}");}
        let credits:serde_json::Value=serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
        for src in ["hermes-agent","openclaw","qoder"] {
            let matching:Vec<_>=credits["breakdown"].as_array().unwrap().iter().filter(|b|b["contributor"].as_str().is_some_and(|id|id == format!("a:{src}") || id.starts_with(&format!("a:{src}/")))).collect();
            assert_eq!(matching.iter().map(|b|b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap_or(0)).sum::<u64>(),8,"{src}: {}",credits["breakdown"]);
        }
        assert!(!credits.to_string().contains("mutable-final-model"));
    }
    hdb.execute("update messages set active=0 where id=2",[]).unwrap();
    rows[2]["message"]["isError"]=json!(true);
    odb.execute("update transcript_events set event_json=?1 where seq=2",[rows[2].to_string()]).unwrap();
    let (ok,v)=run(&env,&["analyze","--repo",&rs]);assert!(ok,"{v}");
    assert_eq!(count("hermes-agent"),0);assert_eq!(count("openclaw"),0);assert_eq!(count("qoder"),8);
    drop(hdb);drop(odb);
    fs::remove_dir_all(env.root).unwrap();
}

/// Newly registered readers must produce attribution through the real CLI,
/// rather than only passing isolated parser tests or appearing in doctor.
#[test]
fn tabnine_native_ownership_survives_mapping_and_cache_removal() {
    use serde_json::json;
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let tn = env.root.join("tabnine-ide");
    fs::create_dir_all(&tn).unwrap();
    env.vars.insert("CREDITIA_TABNINE_IDE_DIR".into(), tn.display().to_string());
    let base = 1_789_603_200_i64;
    fs::write(repo.join("a.ts"), "// original human line\n").unwrap();
    commit(&repo, "base", "me@example.com", "2026-09-17T00:00:00Z");
    let additions = (0..8).map(|i| format!("export const nativeAdded{i} = {i};\n")).collect::<String>();
    let text = format!("// original human line\n{additions}");
    fs::write(repo.join("a.ts"), &text).unwrap();
    commit(&repo, "agent work", "me@example.com", "2026-09-17T00:01:00Z");
    let storage = env.root.join("opencode/storage");
    for dir in ["session/p", "message/ses_tabnine", "part/m"] { fs::create_dir_all(storage.join(dir)).unwrap(); }
    fs::write(storage.join("session/p/ses_tabnine.json"), json!({"id":"ses_tabnine","directory":rs,"time":{"created":base,"updated":base+30}}).to_string()).unwrap();
    fs::write(storage.join("message/ses_tabnine/m.json"), json!({"id":"m","role":"assistant","modelID":"actual-engine-model","tokens":{"input":100,"output":50},"time":{"created":base+20}}).to_string()).unwrap();
    fs::write(storage.join("part/m/t.json"), json!({"type":"tool","tool":"edit","state":{"status":"completed","input":{"filePath":"a.ts","oldString":"// original human line\n","newString":text},"time":{"end":base+30}}}).to_string()).unwrap();
    let (ok, v) = run(&env, &["init", "--repo", &rs]); assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]); assert!(ok, "{v}");
    fs::write(tn.join("sessions.json"), json!({"conversation":"ses_tabnine"}).to_string()).unwrap();
    fs::write(tn.join("conversations.json"), json!([{"conversationId":"conversation","conversationType":"agent","rootPath":rs,"modelId":"mutable-selection","messages":[]}]).to_string()).unwrap();
    for step in 0..3 {
        if step == 1 { fs::write(tn.join("sessions.json"), "{}").unwrap(); }
        if step == 2 { fs::remove_dir_all(env.root.join("lib/cache")).unwrap(); }
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]); assert!(ok, "step {step}: {v}");
        let c = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        assert_eq!(c.query_row("select count(*) from evidence where src='opencode'", [], |r| r.get::<_, i64>(0)).unwrap(), 0, "step {step}");
        assert_eq!(c.query_row("select count(*) from evidence where src='tabnine'", [], |r| r.get::<_, i64>(0)).unwrap(), 8, "step {step}");
        let sessions: Vec<_> = c.prepare("select src,model,tokens from sessions").unwrap().query_map([], |r| Ok((r.get::<_, String>(0)?,r.get::<_, String>(1)?,r.get::<_, i64>(2)?))).unwrap().map(Result::unwrap).collect();
        assert_eq!(sessions, vec![("tabnine".into(), "actual-engine-model".into(), 150)], "step {step}");
    }
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn zcode_and_acp_reach_discover_doctor_and_attribution() {
    use serde_json::json;
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base:i64 = 1_789_000_000;
    let date = |t| format!("@{t} +0800");
    fs::write(repo.join("README.md"), "# Reader integration fixture\n").unwrap();
    commit(&repo,"init","me@example.com",&date(base-86400));
    fs::create_dir_all(repo.join("src")).unwrap();
    let code = |tag:&str| (0..8).map(|n|format!("export const {tag}_{n} = {n} + 100;")).collect::<Vec<_>>().join("\n");
    let a=code("zcode_a");let b=code("zcode_b");let bad=code("rejected");let acp=code("cortex");
    fs::write(repo.join("src/zcode.ts"),format!("{a}\n{b}\n{bad}\n")).unwrap();
    fs::write(repo.join("src/acp.ts"),format!("{acp}\n")).unwrap();
    commit(&repo,"write source files","me@example.com",&date(base+60));

    let zd=env.root.join("zcode/cli/db");fs::create_dir_all(&zd).unwrap();
    env.vars.insert("CREDITIA_ZCODE_DIR".into(),env.root.join("zcode").display().to_string());
    let c=rusqlite::Connection::open(zd.join("db.sqlite")).unwrap();
    c.execute_batch("CREATE TABLE session(id TEXT,directory TEXT,time_created INTEGER,time_updated INTEGER); CREATE TABLE message(id TEXT,session_id TEXT,data TEXT,time_created INTEGER); CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,data TEXT,time_created INTEGER);").unwrap();
    c.execute("INSERT INTO session VALUES('native-session',?1,?2,?3)",rusqlite::params![rs,base*1000,(base+40)*1000]).unwrap();
    for (id,model,at) in [("m1","glm-a",base+10),("m2","glm-b",base+20)] {
        c.execute("INSERT INTO message VALUES(?1,'native-session',?2,?3)",rusqlite::params![id,json!({"role":"assistant","modelId":model,"providerId":"zhipu","tokens":{"total":100}}).to_string(),at*1000]).unwrap();
    }
    for (id,mid,status,text) in [("a","m1","completed",&a),("b","m2","completed",&b),("error","m2","error",&bad),("pending","m2","running",&bad)] {
        let lines: Vec<_> = text.lines().map(|l|format!("+{l}")).collect();
        c.execute("INSERT INTO part VALUES(?1,?2,'native-session',?3,?4)",rusqlite::params![id,mid,json!({"type":"tool","tool":"Write","state":{"status":status,"input":{"file_path":format!("{rs}/src/zcode.ts"),"content":text},"metadata":{"display":{"kind":"file_diff","filePath":format!("{rs}/src/zcode.ts"),"truncated":false,"structuredPatch":[{"lines":lines}]}}}}).to_string(),(base+30)*1000]).unwrap();
    }
    drop(c);
    let cap=env.root.join("captures");fs::create_dir_all(&cap).unwrap();
    env.vars.insert("CREDITIA_CAPTURE_DIR".into(),cap.display().to_string());
    let row=|direction:&str,message:serde_json::Value|json!({"direction":direction,"at":(base+35)*1000,"message":message});
    let rows=[
        json!({"type":"creditia-acp","version":1,"harness":"cortex-code","cwd":rs}),
        row("client",json!({"id":1,"method":"session/new","params":{"cwd":rs}})),
        row("agent",json!({"id":1,"result":{"sessionId":"acp-session","models":{"currentModelId":"cortex-model"}}})),
        row("agent",json!({"method":"session/update","params":{"sessionId":"acp-session","update":{"sessionUpdate":"tool_call","toolCallId":"edit-1","status":"in_progress","content":[{"type":"diff","path":format!("{rs}/src/acp.ts"),"oldText":null,"newText":acp}]}}})),
        row("agent",json!({"method":"session/update","params":{"sessionId":"acp-session","update":{"sessionUpdate":"tool_call_update","toolCallId":"edit-1","status":"completed"}}})),
    ];
    fs::write(cap.join("capture.jsonl"),rows.iter().map(|v|v.to_string()).collect::<Vec<_>>().join("\n")+"\n").unwrap();
    let (ok,discovered)=run(&env,&["discover"]);assert!(ok,"{discovered}");
    let summary=discovered.to_string();assert!(summary.contains("zcode"),"{discovered}");assert!(summary.contains("cortex-code"),"{discovered}");
    let (_,doctor)=run(&env,&["doctor"]);
    assert!(doctor["sources"].as_array().unwrap().iter().any(|s|s["source"]=="zcode"&&s["readable"]==true),"{doctor}");
    assert!(doctor["sources"].as_array().unwrap().iter().any(|s|s["source"]=="cortex-code-acp"&&s["scope"]=="capture"&&s["readable"]==true),"{doctor}");
    let (ok,v)=run(&env,&["init","--repo",&rs]);assert!(ok,"{v}");
    let (ok,v)=run(&env,&["analyze","--repo",&rs]);assert!(ok,"{v}");
    let read=||->serde_json::Value{serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap()};
    let credits=read();
    let verified=|id:&str|credits["breakdown"].as_array().unwrap().iter().find(|b|b["contributor"]==id).and_then(|b|b["byEvidence"]["verified"]["linesAdded"].as_u64()).unwrap_or(0);
    assert_eq!(verified("a:zcode/glm-a"),8,"{}",credits["breakdown"]);
    assert_eq!(verified("a:zcode/glm-b"),8,"{}",credits["breakdown"]);
    assert_eq!(verified("a:cortex-code/cortex-model"),8,"{}",credits["breakdown"]);
    let (ok,v)=run(&env,&["analyze","--repo",&rs]);assert!(ok,"{v}");
    assert_eq!(read()["totals"]["sessions"],credits["totals"]["sessions"],"incremental capture must not duplicate sessions");
    fs::remove_dir_all(env.root).unwrap();
}

#[cfg(unix)]
#[test]
fn acp_capture_preserves_stdio_child_flags_exit_code_and_private_fields() {
    use serde_json::json;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    let env=setup();
    let script=env.root.join("agent.py");
    fs::write(&script,r#"import sys, json
assert sys.argv[1:] == ['--json', '--harness', 'child-value']
for line in sys.stdin:
    v=json.loads(line)
    if v.get('method')=='authenticate':
        print(json.dumps({'jsonrpc':'2.0','id':v['id'],'result':{'echo':'AUTH-SECRET'}}),flush=True)
    elif v.get('method')=='session/new':
        print(json.dumps({'jsonrpc':'2.0','id':v['id'],'result':{'sessionId':'s','models':{'currentModelId':'test-model'}}}),flush=True)
    elif v.get('method')=='session/prompt':
        print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'s','update':{'sessionUpdate':'tool_call','toolCallId':'t','status':'completed','content':[{'type':'diff','path':'/demo/a.rs','oldText':None,'newText':'accepted_line'}]}}}),flush=True)
        print(json.dumps({'jsonrpc':'2.0','id':v['id'],'result':{'stopReason':'end_turn'}}),flush=True)
sys.exit(7)
"#).unwrap();
    let out=env.root.join("capture.jsonl");
    let mut child=Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(["capture","--harness","cortex-code","--out",out.to_str().unwrap(),"--","/usr/bin/python3","-u",script.to_str().unwrap(),"--json","--harness","child-value"])
        .env_clear().envs(&env.vars).env("PATH",std::env::var("PATH").unwrap()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let requests=[json!({"jsonrpc":"2.0","id":0,"method":"authenticate","params":{"token":"AUTH-SECRET"}}),json!({"jsonrpc":"2.0","id":1,"method":"session/new","params":{"cwd":"/demo","mcpServers":[{"env":[{"name":"KEY","value":"MCP-SECRET"}]}]}}),json!({"jsonrpc":"2.0","id":2,"method":"session/prompt","params":{"sessionId":"s","prompt":[{"type":"text","text":"PRIVATE-PROMPT"}]}})];
    let input=requests.iter().map(|v|v.to_string()).collect::<Vec<_>>().join("\n")+"\n";
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let result=child.wait_with_output().unwrap();
    assert_eq!(result.status.code(),Some(7),"{}",String::from_utf8_lossy(&result.stderr));
    let messages:Vec<serde_json::Value>=String::from_utf8(result.stdout).unwrap().lines().map(|s|serde_json::from_str(s).unwrap()).collect();
    assert_eq!(messages.len(),4);assert_eq!(messages[0]["result"]["echo"],"AUTH-SECRET");assert_eq!(messages[3]["result"]["stopReason"],"end_turn");
    let saved=fs::read_to_string(&out).unwrap();
    assert!(!saved.contains("AUTH-SECRET")&&!saved.contains("MCP-SECRET")&&!saved.contains("PRIVATE-PROMPT"),"capture retained unnecessary private fields");
    assert!(saved.contains("accepted_line"));
    assert_eq!(fs::metadata(&out).unwrap().permissions().mode()&0o777,0o600);
    // A user-chosen existing file is never overwritten, and the agent isn't launched.
    let second=Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(["capture","--harness","cortex-code","--out",out.to_str().unwrap(),"--","/no/such/agent"])
        .env_clear().envs(&env.vars).output().unwrap();
    assert_eq!(second.status.code(),Some(1));assert!(second.stdout.is_empty());assert_eq!(fs::read_to_string(&out).unwrap(),saved);
    fs::remove_dir_all(env.root).unwrap();
}

const AGENT_FILE: &str = "src/agent.rs\n";

fn codex_lines() -> Vec<String> {
    (0..30).map(|i| format!("pub fn codex_fn_{i}() -> u32 {{ {i} * 7 }}")).collect()
}
fn cursor_lines() -> Vec<String> {
    (0..20).map(|i| format!("export const cursorValue{i} = {i} * 3;")).collect()
}

/// 伪造 Codex：state_5.sqlite 的 threads 表 + rollout jsonl（含 add 和 unified_diff 两种变更）
fn fake_codex(env: &Env, ts: i64) {
    let base = env.root.join("codex");
    fs::create_dir_all(base.join("sessions/2026/09/01")).unwrap();
    let conn = rusqlite::Connection::open(base.join("state_5.sqlite")).unwrap();
    conn.execute_batch("create table threads(id text primary key, rollout_path text not null, created_at integer not null, updated_at integer not null, source text not null default '', model_provider text not null default '', cwd text not null, title text not null default '', sandbox_policy text not null default '', approval_mode text not null default '', tokens_used integer not null default 0, has_user_event integer not null default 0, archived integer not null default 0, archived_at integer, git_sha text, git_branch text, git_origin_url text, model text);").unwrap();
    let rollout = base.join("sessions/2026/09/01/rollout-2026-09-01T10-00-00-thread1.jsonl");
    conn.execute(
        "insert into threads(id,rollout_path,created_at,updated_at,cwd,tokens_used,git_branch,model) values(?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params!["thread1", rollout.display().to_string(), ts - 600, ts + 60, env.repo.display().to_string(), 12345, "codex/feature", "gpt-test"],
    )
    .unwrap();
    // 审阅线程：不计代码行
    conn.execute(
        "insert into threads(id,rollout_path,created_at,updated_at,cwd,tokens_used,git_branch,model) values(?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params!["review1", "/nonexistent.jsonl", ts, ts + 10, env.repo.display().to_string(), 10, "codex/feature", "codex-auto-review"],
    )
    .unwrap();
    let iso = |t: i64| {
        let (y, m, d) = creditia_core::util::civil_from_days(t.div_euclid(86400));
        let s = t.rem_euclid(86400);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", s / 3600, s / 60 % 60, s % 60)
    };
    let abs = env.repo.join("src/agent.rs").display().to_string();
    let content = codex_lines()[..20].join("\n") + "\n";
    let diff_added: String = codex_lines()[20..].iter().map(|l| format!("+{l}\n")).collect();
    let lines = [
        serde_json::json!({ "timestamp": iso(ts - 500), "type": "turn_context", "payload": { "model": "gpt-test", "cwd": env.repo.display().to_string() } }),
        serde_json::json!({ "timestamp": iso(ts - 400), "type": "event_msg", "payload": { "type": "task_started" } }),
        serde_json::json!({ "timestamp": iso(ts - 300), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "changes": { abs.clone(): { "type": "add", "content": content } } } } }),
        serde_json::json!({ "timestamp": iso(ts - 200), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "changes": { abs: { "type": "update", "unified_diff": format!("@@ -20,0 +21,10 @@\n{diff_added}"), "move_path": null } } } } }),
    ];
    fs::write(&rollout, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    let _ = AGENT_FILE;
}

/// 伪造 Claude Code：目录名编码过（中文变 ---），靠记录里的 cwd 识别；Write 一个文档
fn fake_claude(env: &Env, ts: i64) {
    let dir = env.root.join("claude/projects/-tmp-projects---");
    fs::create_dir_all(&dir).unwrap();
    let iso = |t: i64| {
        let (y, m, d) = creditia_core::util::civil_from_days(t.div_euclid(86400));
        let s = t.rem_euclid(86400);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", s / 3600, s / 60 % 60, s % 60)
    };
    let doc = (0..12).map(|i| format!("- 第 {i} 条说明，写得足够长以便哈希")).collect::<Vec<_>>().join("\n") + "\n";
    let lines = [
        serde_json::json!({ "type": "user", "cwd": env.repo.display().to_string(), "sessionId": "s1", "timestamp": iso(ts - 100), "message": { "role": "user", "content": "写个文档" } }),
        serde_json::json!({ "type": "assistant", "cwd": env.repo.display().to_string(), "sessionId": "s1", "timestamp": iso(ts - 50), "message": { "id": "m1", "model": "claude-test", "usage": { "input_tokens": 100, "output_tokens": 50 }, "content": [{ "type": "tool_use", "id": "write-doc", "name": "Write", "input": { "file_path": env.repo.join("docs/plan.md").display().to_string(), "content": doc } }] } }),
        serde_json::json!({ "type": "user", "cwd": env.repo.display().to_string(), "sessionId": "s1", "timestamp": iso(ts - 49), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "write-doc", "content": "File created successfully", "is_error": false }] }, "toolUseResult": { "type": "create", "filePath": env.repo.join("docs/plan.md").display().to_string(), "content": doc } }),
    ];
    fs::write(dir.join("s1.jsonl"), lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();
}

/// 伪造 Cursor：ai_code_hashes 表，按真实算法算哈希；其中 3 行是 Tab 补全
fn fake_cursor(env: &Env, ts: i64) {
    let dir = env.root.join("cursor/ai-tracking");
    fs::create_dir_all(&dir).unwrap();
    let conn = rusqlite::Connection::open(dir.join("ai-code-tracking.db")).unwrap();
    conn.execute_batch("create table ai_code_hashes(hash text primary key, source text not null, fileExtension text, fileName text, requestId text, conversationId text, timestamp integer, model text, createdAt integer not null);").unwrap();
    let rel = "web/app.ts";
    let abs = env.repo.join(rel).display().to_string();
    for (i, l) in cursor_lines().iter().enumerate() {
        let n = normalize(l, &ext_of(rel));
        let h = format!("{:x}", line_key(rel, &n));
        let source = if i < 3 { "tab" } else { "composer" };
        conn.execute("insert into ai_code_hashes(hash,source,fileExtension,fileName,requestId,conversationId,timestamp,model,createdAt) values(?1,?2,'ts',?3,'r','conv1',?4,?5,?4)", rusqlite::params![h, source, abs, (ts - 100) * 1000, if i % 2 == 0 { "grok-test" } else { "default" }]).unwrap();
    }
    // 对话库：default 回退到 grok-test
    let sdir = env.root.join("cursor-storage");
    fs::create_dir_all(&sdir).unwrap();
    let c2 = rusqlite::Connection::open(sdir.join("state.vscdb")).unwrap();
    c2.execute_batch("create table cursorDiskKV(key text primary key, value text);").unwrap();
    c2.execute("insert into cursorDiskKV values('composerData:conv1', ?1)", [r#"{"modelConfig":{"modelName":"grok-test"}}"#]).unwrap();
}

#[test]
fn end_to_end() {
    let env = setup();
    let repo = &env.repo;
    let base_ts: i64 = 1_787_000_000; // 2026-08
    let d = |t: i64| format!("@{t} +0800");

    // 1. 人写的第一个提交（主导者，无证据 → 未确认）
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/main.rs"), (0..10).map(|i| format!("fn human_written_{i}() {{}}")).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    fs::write(repo.join("package-lock.json"), (0..50).map(|i| format!("  \"dep-{i}\": \"1.0.{i}\",")).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    commit(repo, "init", "me@example.com", &d(base_ts - 86400));
    // 2. Codex 写的文件（逐行证据）
    fs::write(repo.join("src/agent.rs"), codex_lines().join("\n") + "\n").unwrap();
    commit(repo, "feat: agent file (#7)", "me@example.com", &d(base_ts));
    // 3. Cursor 写的文件（行级库 + Tab）
    fs::create_dir_all(repo.join("web")).unwrap();
    fs::write(repo.join("web/app.ts"), cursor_lines().join("\n") + "\n").unwrap();
    commit(repo, "cursor part", "me@example.com", &d(base_ts + 100));
    // 4. Claude 写的文档
    fs::create_dir_all(repo.join("docs")).unwrap();
    fs::write(repo.join("docs/plan.md"), (0..12).map(|i| format!("- 第 {i} 条说明，写得足够长以便哈希")).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    commit(repo, "docs", "me@example.com", &d(base_ts + 200));
    // 5. 别人的提交（agent 邮箱 → declared cursor）。放在所有本地会话的时间窗口之外，避免借用会话里的模型名
    fs::write(repo.join("web/other.ts"), (0..8).map(|i| format!("export function other{i}() {{ return {i}; }}")).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    commit(repo, "cloud agent", "cursoragent@cursor.com", &d(base_ts + 5000));
    // 6. 盖章提交
    fs::write(repo.join("src/stamped.rs"), (0..6).map(|i| format!("pub const STAMPED_{i}: u8 = {i};")).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    commit(repo, "stamped\n\nCreditia-Harness: kimi\nCreditia-Model: k3", "me@example.com", &d(base_ts + 6000));
    git(repo, &["tag", "v0.1.0"]);

    fake_codex(&env, base_ts);
    fake_claude(&env, base_ts + 200);
    fake_cursor(&env, base_ts + 100);

    // init
    let (ok, v) = run(&env, &["init", "--repo", &repo.display().to_string()]);
    assert!(ok, "{v}");
    assert_eq!(v["config"]["directors"][0]["emails"][0], "me@example.com");
    assert!(repo.join(".creditia/config.json").exists());

    // analyze
    let (ok, v) = run(&env, &["analyze", "--repo", &repo.display().to_string()]);
    assert!(ok, "{v}");
    assert_eq!(v["commits"], 6);
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let lines = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    // Codex：30 行全部逐行对上（add 20 + diff 10）
    assert_eq!(lines("a:codex/gpt-test"), 30, "{}", credits["breakdown"]);
    assert_eq!(verified("a:codex/gpt-test"), 30);
    // Cursor：17 行 composer（模型 default 回退到 grok-test）+ 3 行 Tab 归人
    assert_eq!(lines("a:cursor/grok-test"), 17, "{}", credits["breakdown"]);
    assert_eq!(credits["human"]["tabAssistedLines"], 3);
    // Claude：12 行文档
    assert_eq!(lines("a:claude-code/claude-test"), 12, "{}", credits["breakdown"]);
    // 云端 agent 邮箱 → declared cursor（无模型）
    assert_eq!(lines("a:cursor"), 8);
    // 盖章 → kimi/k3
    assert_eq!(lines("a:kimi/k3"), 6);
    // 人写的 10 行 → 未确认；lockfile 被排除；3 行 Tab 补全归主导者
    assert_eq!(lines("unattributed"), 10, "{}", credits["breakdown"]);
    assert_eq!(credits["totals"]["linesAdded"], 86);
    // 项：(#7) squash 识别为 pr:7
    assert!(credits["units"].as_array().unwrap().iter().any(|u| u["id"] == "pr:7"));
    // 版本
    assert_eq!(credits["versions"][0]["tag"], "v0.1.0");
    // 审阅线程记为 reviewer
    assert!(credits["contributors"].as_array().unwrap().iter().any(|c| c["model"] == "codex-auto-review" && c["roles"][0] == "reviewer"));
    // 隐私：输出里不含代码原文
    let txt = credits.to_string();
    assert!(!txt.contains("codex_fn_") && !txt.contains("cursorValue"));
    // 档案里不出现任何邮箱
    assert!(!txt.contains("@example.com") && !txt.contains("@cursor.com"), "{txt}");

    // 增量：加一个提交，只处理 1 个
    fs::write(repo.join("src/more.rs"), "pub fn more_fn_added_later() -> i32 { 42 }\n").unwrap();
    commit(repo, "more", "me@example.com", &d(base_ts + 7000));
    let (ok, v) = run(&env, &["update", "--repo", &repo.display().to_string()]);
    assert!(ok, "{v}");
    assert_eq!(v["newCommits"], 1);
    assert_eq!(v["commits"], 7);

    // blame：agent.rs 第 1 行是 Codex 写的
    let (ok, v) = run(&env, &["blame", "src/agent.rs:1", "--repo", &repo.display().to_string()]);
    assert!(ok, "{v}");
    assert_eq!(v["harness"], "codex");
    assert_eq!(v["model"], "gpt-test");
    assert_eq!(v["evidence"], "verified");

    // 证据库里有记录；projects 列出项目
    let (ok, v) = run(&env, &["projects"]);
    assert!(ok, "{v}");
    assert_eq!(v["projects"].as_array().unwrap().len(), 1);
    assert!(v["projects"][0]["evidence"].as_u64().unwrap() >= 59);
    // 跨项目汇总读每个项目最新的档案
    let (ok, v) = run(&env, &["all-credits"]);
    assert!(ok, "{v}");
    assert_eq!(v["projects"].as_array().unwrap().len(), 1);
    assert_eq!(v["projects"][0]["credits"]["totals"]["commits"], 7, "{v}");

    // doctor 能跑
    let (_, v) = run(&env, &["doctor"]);
    // Source registration is shared by discovery, collection and doctor. Check
    // identities instead of pinning the old source count as adapters are added.
    let sources=v["sources"].as_array().unwrap();
    for source in ["codex","cursor","zcode","muse","t3","omp","command-code","dsh","cortex-code"] {
        assert!(sources.iter().any(|s|s["source"]==source),"missing doctor source {source}: {v}");
    }

    // hook：安装后提交信息自动带 trailer（用盖章文件触发）
    let (ok, _) = run(&env, &["hook", "install", "--repo", &repo.display().to_string()]);
    assert!(ok);
    let (ok, _) = run(&env, &["stamp", "--harness", "grok", "--model", "grok-test", "--repo", &repo.display().to_string()]);
    assert!(ok);
    fs::write(repo.join("src/hooked.rs"), "pub fn hooked() {}\n").unwrap();
    commit(repo, "hooked commit", "me@example.com", &d(base_ts + 8000));
    let msg = git(repo, &["log", "-1", "--format=%B"]);
    assert!(msg.contains("Creditia-Harness: grok"), "{msg}");
    assert!(msg.contains("Creditia-Model: grok-test"), "{msg}");
    // hook 那一行先试数据目录里的固定入口，它指向这个核心
    let hook = fs::read_to_string(repo.join(".git/hooks/prepare-commit-msg")).unwrap();
    let shim = env.root.join("lib/bin/creditia-core");
    assert!(hook.contains(&shim.display().to_string()), "{hook}");
    assert_eq!(fs::canonicalize(&shim).unwrap(), fs::canonicalize(env!("CARGO_BIN_EXE_creditia-core")).unwrap());
    // 旧版写死路径的 hook：查一次接入状态就换成新的一行
    fs::write(repo.join(".git/hooks/prepare-commit-msg"), "#!/bin/sh\necho keep\n\"/old/place/creditia-core\" hook-trailers \"$1\" >/dev/null 2>&1 || true # creditia-stamp\n").unwrap();
    let (ok, _) = run(&env, &["integrations", "--repo", &repo.display().to_string()]);
    assert!(ok);
    let hook = fs::read_to_string(repo.join(".git/hooks/prepare-commit-msg")).unwrap();
    assert!(hook.contains("echo keep") && !hook.contains("/old/place") && hook.contains(&shim.display().to_string()), "{hook}");

    // 盖章 hook 记下这次提交里各工具写了几行：Codex 写的 30 行删掉再加回来，暂存区里 30 行都有证据
    fs::remove_file(repo.join("src/agent.rs")).unwrap();
    commit(repo, "drop agent", "me@example.com", &d(base_ts + 8100));
    fs::write(repo.join("src/agent.rs"), codex_lines().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/plain.rs"), "pub fn typed_by_hand_here() -> u8 { 1 }\n").unwrap();
    git(repo, &["add", "-A"]);
    let msgf = env.root.join("COMMIT_MSG");
    fs::write(&msgf, "restore agent\n").unwrap();
    let mut at = Env { root: env.root.clone(), repo: env.repo.clone(), vars: env.vars.clone() };
    at.vars.insert("GIT_AUTHOR_DATE".into(), d(base_ts + 8200));
    let (ok, v) = run(&at, &["hook-trailers", &msgf.display().to_string(), "--repo", &repo.display().to_string()]);
    assert!(ok, "{v}");
    let msg = fs::read_to_string(&msgf).unwrap();
    assert!(msg.contains("Creditia-Lines: codex/gpt-test 30 / 31"), "{msg}");
    assert!(!msg.contains("Creditia-Harness"), "{msg}");
    commit(repo, "restore agent", "me@example.com", &d(base_ts + 8200));

    // discover 能找到这个项目
    let (ok, v) = run(&env, &["discover"]);
    assert!(ok, "{v}");
    assert!(v["projects"].as_array().unwrap().iter().any(|p| p["name"] == "示例项目"), "{v}");

    // Claude Code 的日志被清理了：证据库把那个会话的证据补回来，文档仍是逐行证据
    fs::remove_dir_all(env.root.join("claude/projects")).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &repo.display().to_string(), "--fresh"]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let doc = credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == "a:claude-code/claude-test").unwrap();
    assert_eq!(doc["byEvidence"]["verified"]["linesAdded"], 12, "{doc}");

    let _ = fs::remove_dir_all(&env.root);
}

fn iso(ts: i64) -> String {
    let (y, m, d) = creditia_core::util::civil_from_days(ts.div_euclid(86400));
    let s = ts.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", s / 3600, s / 60 % 60, s % 60)
}

/// Gemini CLI、OpenCode、Cline 三种新数据源；口径开关；一个项目多处路径
#[test]
fn new_sources_levels_and_paths() {
    use sha2::{Digest, Sha256};
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_788_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let gem: Vec<String> = (0..12).map(|i| format!("def gemini_helper_{i}(x): return x * {i} + 1")).collect();
    let oc: Vec<String> = (0..10).map(|i| format!("pub fn opencode_part_{i}() -> usize {{ {i} + 100 }}")).collect();
    let rejected: Vec<String> = (0..5).map(|i| format!("pub fn rejected_attempt_{i}() -> usize {{ {i} }}")).collect();
    let cl: Vec<String> = (0..8).map(|i| format!("export const clineThing{i} = () => {i} * 2;")).collect();
    // 被拒绝的 Cline 编辑、报错的 Claude Code 编辑：内容后来由人自己写进了提交，不能记成 agent 的
    let cl_denied: Vec<String> = (0..6).map(|i| format!("export const handWrittenAfterDenial{i} = {i};")).collect();
    let cc_failed: Vec<String> = (0..6).map(|i| format!("pub fn hand_after_failed_edit_{i}() -> u8 {{ {i} }}")).collect();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# demo project for the new adapters\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/gem.py"), gem.join("\n") + "\n").unwrap();
    commit(&repo, "gemini work", "me@example.com", &d(base + 60));
    // 被拒绝的编辑和成功的编辑写在同一个文件里：被拒绝的那 5 行最后人自己也写了，不能算 OpenCode 的
    fs::write(repo.join("src/oc.rs"), [oc.clone(), rejected.clone()].concat().join("\n") + "\n").unwrap();
    commit(&repo, "opencode work", "me@example.com", &d(base + 160));
    fs::write(repo.join("src/cl.ts"), cl.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/denied.ts"), cl_denied.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/failed.rs"), cc_failed.join("\n") + "\n").unwrap();
    commit(&repo, "cline work", "me@example.com", &d(base + 260));

    // Gemini：目录名是项目路径的 SHA-256
    let hash: String = Sha256::digest(rs.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let gdir = env.root.join("gemini/tmp").join(&hash).join("chats");
    fs::create_dir_all(&gdir).unwrap();
    let chat = serde_json::json!({ "sessionId": "g1", "projectHash": hash, "startTime": iso(base - 600), "lastUpdated": iso(base),
        "messages": [
            { "type": "user", "timestamp": iso(base - 590), "content": "write helpers" },
            { "type": "gemini", "timestamp": iso(base - 500), "model": "gemini-test", "tokens": { "total": 4321 },
              "toolCalls": [{ "name": "write_file", "status": "success", "timestamp": iso(base - 500), "args": { "file_path": format!("{rs}/src/gem.py"), "content": gem.join("\n") } }] }
        ] });
    fs::write(gdir.join("session-1.json"), chat.to_string()).unwrap();

    // OpenCode：会话 → 消息 → 片段；渠道是自定义中转站
    let st = env.root.join("opencode/storage");
    fs::create_dir_all(st.join("session/p1")).unwrap();
    fs::create_dir_all(st.join("message/s1")).unwrap();
    fs::create_dir_all(st.join("part/m1")).unwrap();
    fs::write(st.join("session/p1/s1.json"), serde_json::json!({ "id": "s1", "directory": rs, "time": { "created": (base - 100) * 1000, "updated": (base + 100) * 1000 } }).to_string()).unwrap();
    fs::write(st.join("message/s1/m0.json"), serde_json::json!({ "id": "m0", "role": "user", "time": { "created": (base - 90) * 1000 } }).to_string()).unwrap();
    fs::write(st.join("message/s1/m1.json"), serde_json::json!({ "id": "m1", "role": "assistant", "modelID": "claude-opus-test", "providerID": "my-relay", "time": { "created": (base + 100) * 1000 }, "tokens": { "input": 100, "output": 50, "cache": { "read": 10, "write": 0 } }, "path": { "cwd": rs } }).to_string()).unwrap();
    fs::write(st.join("part/m1/a.json"), serde_json::json!({ "type": "tool", "tool": "write", "state": { "status": "completed", "input": { "filePath": format!("{rs}/src/oc.rs"), "content": oc.join("\n") }, "time": { "end": (base + 100) * 1000 } } }).to_string()).unwrap();
    fs::write(st.join("part/m1/b.json"), serde_json::json!({ "type": "tool", "tool": "edit", "state": { "status": "error", "input": { "filePath": format!("{rs}/src/oc.rs"), "oldString": "x", "newString": rejected.join("\n") }, "time": { "end": (base + 100) * 1000 } } }).to_string()).unwrap();

    // Cline：路径相对于工作目录；模型与渠道在 task_metadata
    let t = env.root.join("vscode/saoudrizwan.claude-dev/tasks/t1");
    fs::create_dir_all(&t).unwrap();
    fs::write(t.join("api_conversation_history.json"), serde_json::json!([
        { "role": "user", "content": [{ "type": "text", "text": format!("<environment_details>\n# Current Working Directory ({rs}) Files\nsrc/\n</environment_details>") }] },
        { "role": "user", "content": [{ "type": "text", "text": "[write_to_file for 'src/cl.ts'] Result:\nThe content was successfully saved to src/cl.ts.\n\n<final_file_content path=\"src/cl.ts\">\n// throws an error when the input failed validation\n</final_file_content>" }] },
        { "role": "user", "content": [{ "type": "text", "text": "[write_to_file for 'src/denied.ts'] Result:\nThe user denied this operation." }] }
    ]).to_string()).unwrap();
    fs::write(t.join("ui_messages.json"), serde_json::json!([
        { "ts": (base + 200) * 1000, "type": "say", "say": "task", "text": "add things" },
        { "ts": (base + 250) * 1000, "type": "ask", "ask": "tool", "text": serde_json::json!({ "tool": "newFileCreated", "path": "src/cl.ts", "content": cl.join("\n") }).to_string() },
        { "ts": (base + 255) * 1000, "type": "ask", "ask": "tool", "text": serde_json::json!({ "tool": "newFileCreated", "path": "src/denied.ts", "content": cl_denied.join("\n") }).to_string() }
    ]).to_string()).unwrap();

    // Claude Code：一次 Edit 报错（old_string 没找到）
    let cdir = env.root.join("claude/projects/demo");
    fs::create_dir_all(&cdir).unwrap();
    let lines = [
        serde_json::json!({ "type": "user", "cwd": rs, "sessionId": "cc1", "timestamp": iso(base + 220), "message": { "role": "user", "content": "edit it" } }),
        serde_json::json!({ "type": "assistant", "cwd": rs, "sessionId": "cc1", "timestamp": iso(base + 230), "message": { "id": "msg_01x", "model": "claude-test", "content": [{ "type": "tool_use", "id": "tu1", "name": "Edit", "input": { "file_path": format!("{rs}/src/failed.rs"), "old_string": "nope", "new_string": cc_failed.join("\n") } }] } }),
        serde_json::json!({ "type": "user", "cwd": rs, "sessionId": "cc1", "timestamp": iso(base + 231), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "tu1", "is_error": true, "content": "String to replace not found in file." }] } }),
    ];
    fs::write(cdir.join("cc1.jsonl"), lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    fs::write(t.join("task_metadata.json"), serde_json::json!({ "model_usage": [{ "ts": (base + 200) * 1000, "model_id": "claude-sonnet-test", "model_provider_id": "openrouter" }] }).to_string()).unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let b = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).cloned().unwrap_or_default();
    assert_eq!(b("a:gemini/gemini-test")["byEvidence"]["verified"]["linesAdded"], 12, "{}", credits["breakdown"]);
    assert_eq!(b("a:opencode/claude-opus-test")["byEvidence"]["verified"]["linesAdded"], 10, "{}", credits["breakdown"]);
    assert_eq!(b("a:cline/claude-sonnet-test")["byEvidence"]["verified"]["linesAdded"], 8, "{}", credits["breakdown"]);
    // 被拒绝 / 报错的编辑没有逐行证据：全仓库的逐行证据正好是 Gemini 12 + OpenCode 10 + Cline 8
    let verified_total: u64 = credits["breakdown"].as_array().unwrap().iter().map(|x| x["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap_or(0)).sum();
    assert_eq!(verified_total, 30, "{}", credits["breakdown"]);
    assert_eq!(b("a:claude-code/claude-test")["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap_or(0), 0);
    let c = |id: &str| credits["contributors"].as_array().unwrap().iter().find(|c| c["id"] == id).cloned().unwrap_or_default();
    assert_eq!(c("a:opencode/claude-opus-test")["provider"], "my-relay");
    assert_eq!(c("a:cline/claude-sonnet-test")["provider"], "openrouter");
    assert_eq!(c("a:gemini/gemini-test")["family"], "google");
    // 没有「主导者」
    assert!(credits.get("directors").is_none());
    assert!(!credits.to_string().contains("director"));
    // 会话里人发的指令
    assert!(credits["human"]["prompts"].as_i64().unwrap() >= 2);
    // standard：失败的那次编辑写的 5 行跟随同提交里的 OpenCode，记为推断（不是逐行证据，也不是声明）
    assert_eq!(b("a:opencode/claude-opus-test")["byEvidence"]["inferred"]["linesAdded"], 5);
    // strict：同提交延伸不算，这 5 行未确认
    let out = env.root.join("strict.json");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs, "--level", "strict", "--out", &out.display().to_string()]);
    assert!(ok, "{v}");
    let strict: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(strict["level"], "strict");
    let sb = |id: &str| strict["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(sb("a:opencode/claude-opus-test"), 10);

    // 一个项目多处路径：关联一个已经不存在的旧路径；paths 列出主仓库和它
    let (ok, v) = run(&env, &["link", "/old/machine/demo", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["paths", "--repo", &rs]);
    assert!(ok, "{v}");
    let ps: Vec<String> = v["paths"].as_array().unwrap().iter().map(|p| p["path"].as_str().unwrap().to_string()).collect();
    assert!(ps.contains(&rs) && ps.contains(&"/old/machine/demo".to_string()), "{v}");
    // 另一个仓库不能关联进来
    let other = env.root.join("other");
    fs::create_dir_all(&other).unwrap();
    git(&other, &["init", "-q"]);
    fs::write(other.join("a.txt"), "x\n").unwrap();
    commit(&other, "x", "me@example.com", &d(base));
    let (ok, _) = run(&env, &["link", &other.display().to_string(), "--repo", &rs]);
    assert!(!ok);
    let _ = fs::remove_dir_all(&env.root);
}

/// Grok、Kimi、Roo Code、Kilo Code 四种数据源；archive 标签不算版本，ignoreTags 可以自己加
#[test]
fn grok_kimi_roo_kilo_and_ignored_tags() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_789_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("pub fn {tag}_line_{i}() -> u32 {{ {i} * 11 }}")).collect() };
    let (grok_a, grok_b) = (lines("grok_write", 8), lines("grok_replace", 4));
    let (kimi_a, kimi_b) = (lines("kimi_write", 7), lines("kimi_edit", 5));
    let roo: Vec<String> = (0..4).map(|i| format!("export const rooValue{i} = {i} + 40;")).collect();
    let kilo: Vec<String> = (0..3).map(|i| format!("export const kiloValue{i} = {i} + 70;")).collect();

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# demo project for more adapters\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/grok_a.rs"), grok_a.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/grok_b.rs"), grok_b.join("\n") + "\n").unwrap();
    commit(&repo, "grok work", "me@example.com", &d(base + 60));
    git(&repo, &["tag", "v0.3.0"]);
    fs::write(repo.join("src/kimi_a.rs"), kimi_a.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/kimi_b.rs"), kimi_b.join("\n") + "\n").unwrap();
    commit(&repo, "kimi work", "me@example.com", &d(base + 160));
    git(&repo, &["tag", "archive/site-old"]);
    fs::write(repo.join("src/roo.ts"), roo.join("\n") + "\n").unwrap();
    commit(&repo, "roo work", "me@example.com", &d(base + 260));
    git(&repo, &["tag", "nightly-1"]);
    fs::write(repo.join("src/kilo.ts"), kilo.join("\n") + "\n").unwrap();
    commit(&repo, "kilo work", "me@example.com", &d(base + 360));

    // Grok：目录名是 URL 编码的 cwd；一次 write（参数是对象），一次 search_replace（参数是 JSON 字符串）
    let gs = env.root.join("grok/sessions").join(rs.replace('/', "%2F")).join("u1");
    fs::create_dir_all(&gs).unwrap();
    fs::write(gs.join("summary.json"), serde_json::json!({ "info": { "cwd": rs }, "current_model_id": "grok-test", "created_at": base - 300, "updated_at": base, "head_branch": "main" }).to_string()).unwrap();
    let hist = [
        serde_json::json!({ "role": "user", "content": "write it" }),
        serde_json::json!({ "role": "assistant", "model_id": "grok-test", "ts": base - 200, "tool_calls": [{ "name": "write", "args": { "file_path": format!("{rs}/src/grok_a.rs"), "content": grok_a.join("\n") } }] }),
        serde_json::json!({ "role": "assistant", "model_id": "grok-test", "ts": base - 100, "tool_calls": [{ "function": { "name": "search_replace", "arguments": serde_json::json!({ "file_path": format!("{rs}/src/grok_b.rs"), "old_string": "x", "new_string": grok_b.join("\n") }).to_string() } }] }),
    ];
    fs::write(gs.join("chat_history.jsonl"), hist.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    // Kimi：一个会话 state.json 里有 cwd（Write 包在 context.append_loop_event 里）；
    // 另一个会话没写 cwd，靠 session_index.jsonl 找回工作目录（直接的 tool.call Edit）
    let kb = env.root.join("kimi");
    let (s1, s2) = (kb.join("sessions/wd/session_k1"), kb.join("sessions/wd/session_k2"));
    for s in [&s1, &s2] {
        fs::create_dir_all(s.join("agents/main")).unwrap();
    }
    fs::write(s1.join("state.json"), serde_json::json!({ "cwd": rs }).to_string()).unwrap();
    fs::write(s2.join("state.json"), "{}").unwrap();
    fs::write(kb.join("session_index.jsonl"), serde_json::json!({ "sessionDir": s2.display().to_string(), "workDir": rs }).to_string() + "\n").unwrap();
    let w1 = [
        serde_json::json!({ "type": "turn.prompt", "time": base + 100 }),
        serde_json::json!({ "type": "llm.request", "time": base + 101, "payload": { "model": "kimi-test" } }),
        serde_json::json!({ "type": "usage.record", "time": base + 102, "payload": { "usage": { "inputOther": 100, "output": 40 } } }),
        serde_json::json!({ "type": "context.append_loop_event", "time": base + 103, "payload": { "event": { "type": "tool.call", "call": { "name": "Write", "args": { "path": format!("{rs}/src/kimi_a.rs"), "content": kimi_a.join("\n") } } } } }),
    ];
    let w2 = [
        serde_json::json!({ "type": "llm.request", "time": base + 120, "payload": { "model": "kimi-test" } }),
        serde_json::json!({ "type": "tool.call", "time": base + 121, "payload": { "name": "Edit", "args": { "file_path": format!("{rs}/src/kimi_b.rs"), "old_string": "x", "new_string": kimi_b.join("\n") } } }),
    ];
    fs::write(s1.join("agents/main/wire.jsonl"), w1.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    fs::write(s2.join("agents/main/wire.jsonl"), w2.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    // Roo Code 和 Kilo Code：和 Cline 同一种格式，放在各自的扩展目录里
    let task = |ext: &str, id: &str, result: &str, ui: serde_json::Value, model: &str| {
        let t = env.root.join("vscode").join(ext).join("tasks").join(id);
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("api_conversation_history.json"), serde_json::json!([
            { "role": "user", "content": [{ "type": "text", "text": format!("# Current Working Directory ({rs}) Files") }] },
            { "role": "user", "content": [{ "type": "text", "text": result }] }
        ]).to_string()).unwrap();
        fs::write(t.join("ui_messages.json"), ui.to_string()).unwrap();
        fs::write(t.join("task_metadata.json"), serde_json::json!({ "model_usage": [{ "model_id": model, "model_provider_id": "anthropic" }] }).to_string()).unwrap();
    };
    task("rooveterinaryinc.roo-cline", "r1", "[write_to_file for 'src/roo.ts'] Result:\nThe content was successfully saved to src/roo.ts.", serde_json::json!([
        { "ts": (base + 200) * 1000, "say": "task", "text": "add roo values" },
        { "ts": (base + 210) * 1000, "ask": "tool", "text": serde_json::json!({ "tool": "newFileCreated", "path": "src/roo.ts", "content": roo.join("\n") }).to_string() }
    ]), "roo-model");
    let diff = format!("--- a/src/kilo.ts\n+++ b/src/kilo.ts\n@@ -0,0 +1,3 @@\n{}\n", kilo.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
    task("kilocode.kilo-code", "k1", "[apply_diff for 'src/kilo.ts'] Result:\nChanges successfully applied to src/kilo.ts.", serde_json::json!([
        { "ts": (base + 300) * 1000, "say": "task", "text": "add kilo values" },
        { "ts": (base + 310) * 1000, "ask": "tool", "text": serde_json::json!({ "tool": "appliedDiff", "path": "src/kilo.ts", "diff": diff }).to_string() }
    ]), "kilo-model");

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let read = || -> serde_json::Value { serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap() };
    let credits = read();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:grok/grok-test"), 12, "{}", credits["breakdown"]);
    assert_eq!(verified("a:kimi/kimi-test"), 12, "{}", credits["breakdown"]);
    assert_eq!(verified("a:roo/roo-model"), 4, "{}", credits["breakdown"]);
    assert_eq!(verified("a:kilo/kilo-model"), 3, "{}", credits["breakdown"]);
    // 会话也记下了：Kimi 两个、Grok 一个
    let sessions = |h: &str| credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with(&format!("a:{h}/"))).count();
    assert_eq!(sessions("kimi"), 2, "{}", credits["sessions"]);
    assert_eq!(sessions("grok"), 1, "{}", credits["sessions"]);

    // 版本：archive/ 默认不算；nightly-1 默认算
    let tags = |c: &serde_json::Value| c["versions"].as_array().unwrap().iter().map(|v| v["tag"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(tags(&credits), ["v0.3.0", "nightly-1"]);
    // 自己加一条规则：nightly-* 也不算
    let cfg_path = repo.join(".creditia/config.json");
    let mut cfg: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg_path).unwrap()).unwrap();
    assert_eq!(cfg["ignoreTags"], serde_json::json!(["archive/**"]));
    cfg["ignoreTags"] = serde_json::json!(["archive/**", "nightly-*"]);
    fs::write(&cfg_path, cfg.to_string()).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(tags(&read()), ["v0.3.0"]);
    let _ = fs::remove_dir_all(&env.root);
}

/// Aider、GitHub Copilot（VS Code + 命令行）、Amp、TRAE CLI：各写一个文件，被拒绝 / 报错 / 没应用的编辑不算
#[test]
fn aider_copilot_amp_trae() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_790_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("value_{tag}_{i} = compute_{tag}({i}, {})", i * 13)).collect() };
    let (aider, vs, cli, amp, trae) = (lines("aider", 6), lines("vscode", 5), lines("cli", 4), lines("amp", 4), lines("trae", 3));
    let human = lines("human", 3);

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(".gitignore"), ".aider*\n").unwrap();
    fs::write(repo.join("README.md"), "# demo project for the newer readers\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/aider.py"), aider.join("\n") + "\n").unwrap();
    // --no-attribute-author：作者名里没有 (aider)，只能靠记录逐行认
    let aider_commit = commit(&repo, "feat: aider work", "me@example.com", &d(base + 60));
    fs::write(repo.join("src/vscode.ts"), vs.join("\n") + "\n").unwrap();
    commit(&repo, "copilot chat work", "me@example.com", &d(base + 160));
    fs::write(repo.join("src/cli.rs"), cli.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/human.rs"), human.join("\n") + "\n").unwrap();
    commit(&repo, "copilot cli work", "me@example.com", &d(base + 260));
    fs::write(repo.join("src/amp.ts"), amp.join("\n") + "\n").unwrap();
    commit(&repo, "amp work", "me@example.com", &d(base + 360));
    fs::write(repo.join("src/trae.go"), trae.join("\n") + "\n").unwrap();
    commit(&repo, "trae work", "me@example.com", &d(base + 460));

    // Aider：仓库根目录的 .aider.chat.history.md，开始时间是本地时间（+0800）
    let local = |t: i64| {
        let t = t + 8 * 3600;
        let (y, m, dd) = creditia_core::util::civil_from_days(t.div_euclid(86400));
        let s = t.rem_euclid(86400);
        format!("{y:04}-{m:02}-{dd:02} {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    };
    let history = format!(
        "\n# aider chat started at {}\n\n> Aider v0.86.1  \n> Main model: anthropic/claude-sonnet-4-5-20250929 with diff edit format  \n\n#### add the values  \n\nsrc/aider.py\n```python\n<<<<<<< SEARCH\n=======\n{}\n>>>>>>> REPLACE\n```\n\nsrc/human.rs\n```\n<<<<<<< SEARCH\n=======\n{}\n>>>>>>> REPLACE\n```\n\n> Tokens: 3.1k sent, 210 received.  \n> Applied edit to src/aider.py  \n> Commit {} feat: aider work  \n",
        local(base - 300),
        aider.join("\n"),
        human.join("\n"),
        &aider_commit[..7]
    );
    fs::write(repo.join(".aider.chat.history.md"), history).unwrap();

    // Copilot Chat（VS Code）：workspaceStorage 里的 chatSessions，增量 jsonl；workspace.json 的地址是百分号编码的
    let enc: String = rs.bytes().map(|b| if b.is_ascii_alphanumeric() || b"/-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let config_dir = if cfg!(target_os = "macos") { "Library/Application Support" } else if cfg!(target_os = "windows") { "AppData/Roaming" } else { ".config" };
    let ws = env.root.join(config_dir).join("Code/User/workspaceStorage/0123abcd");
    fs::create_dir_all(ws.join("chatSessions")).unwrap();
    fs::write(ws.join("workspace.json"), serde_json::json!({ "folder": format!("file://{enc}") }).to_string()).unwrap();
    let chat = [
        serde_json::json!({ "kind": 0, "v": { "sessionId": "vs1", "creationDate": (base + 100) * 1000, "requests": [] } }),
        serde_json::json!({ "kind": 2, "k": ["requests"], "v": [{ "timestamp": (base + 120) * 1000, "modelId": "copilot/gpt-5.2", "result": { "metadata": { "toolCallRounds": [{ "toolCalls": [
            { "id": "c1", "name": "create_file", "arguments": serde_json::json!({ "filePath": format!("{rs}/src/vscode.ts"), "content": vs.join("\n") }).to_string() },
            { "id": "c2", "name": "create_file", "arguments": serde_json::json!({ "filePath": format!("{rs}/src/human.rs"), "content": human.join("\n") }).to_string() }
        ] }], "toolCallResults": { "c2": { "content": [{ "value": "ERROR while calling tool: File already exists" }] } } } } }] }),
    ];
    fs::write(ws.join("chatSessions/vs1.jsonl"), chat.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    // Copilot 命令行：events.jsonl；被拒绝的编辑（human.rs）不算
    let cs = env.root.join(".copilot/session-state/cli1");
    fs::create_dir_all(&cs).unwrap();
    let ev = |t: &str, ts: i64, data: serde_json::Value| serde_json::json!({ "type": t, "timestamp": iso(ts), "id": "x", "parentId": null, "data": data }).to_string();
    let events = [
        ev("session.start", base + 200, serde_json::json!({ "sessionId": "cli1", "startTime": iso(base + 200), "selectedModel": "claude-sonnet-4.5", "context": { "cwd": rs } })),
        ev("user.message", base + 201, serde_json::json!({ "content": "write cli.rs" })),
        ev("tool.execution_start", base + 210, serde_json::json!({ "toolCallId": "t1", "toolName": "create", "arguments": { "path": "src/cli.rs", "file_text": cli.join("\n") } })),
        ev("tool.execution_complete", base + 211, serde_json::json!({ "toolCallId": "t1", "success": true, "model": "claude-sonnet-4.5" })),
        ev("tool.execution_start", base + 220, serde_json::json!({ "toolCallId": "t2", "toolName": "create", "arguments": { "path": format!("{rs}/src/human.rs"), "file_text": human.join("\n") } })),
        ev("tool.execution_complete", base + 221, serde_json::json!({ "toolCallId": "t2", "success": false, "error": { "message": "The user rejected this tool call." } })),
        ev("assistant.usage", base + 222, serde_json::json!({ "model": "claude-sonnet-4.5", "inputTokens": 900, "outputTokens": 100 })),
    ];
    fs::write(cs.join("events.jsonl"), events.join("\n") + "\n").unwrap();

    // Amp：threads/T-*.json；create_file 完成了，edit_file 被拒绝
    let at = env.root.join(".local/share/amp/threads");
    fs::create_dir_all(&at).unwrap();
    fs::write(at.join("T-amp1.json"), serde_json::json!({
        "v": 1, "id": "T-amp1", "created": (base + 300) * 1000,
        "env": { "initial": { "trees": [{ "uri": format!("file://{enc}") }], "tags": ["model:claude-opus-4-6"] } },
        "messages": [
            { "role": "user", "content": [{ "type": "text", "text": "write amp.ts" }], "meta": { "sentAt": (base + 300) * 1000 } },
            { "role": "assistant", "usage": { "model": "claude-opus-4-6", "inputTokens": 50, "outputTokens": 20, "timestamp": iso(base + 310) }, "content": [
                { "type": "tool_use", "id": "a1", "name": "create_file", "input": { "path": format!("{rs}/src/amp.ts"), "content": amp.join("\n") } },
                { "type": "tool_use", "id": "a2", "name": "edit_file", "input": { "path": format!("{rs}/src/human.rs"), "old_str": "", "new_str": human.join("\n") } }
            ] },
            { "role": "user", "content": [
                { "type": "tool_result", "toolUseID": "a1", "run": { "status": "done", "result": {} } },
                { "type": "tool_result", "toolUseID": "a2", "run": { "status": "rejected-by-user" } }
            ] }
        ]
    }).to_string()).unwrap();

    // TRAE CLI：Codex 格式的 rollout，放在 ~/.trae/cli/sessions 下
    let tr = env.root.join(".trae/cli/sessions/2026/09/22");
    fs::create_dir_all(&tr).unwrap();
    let rollout = [
        serde_json::json!({ "timestamp": iso(base + 400), "type": "session_meta", "payload": { "id": "trae1", "cwd": rs, "model_provider": "trae" } }),
        serde_json::json!({ "timestamp": iso(base + 401), "type": "turn_context", "payload": { "cwd": rs, "model": "doubao-seed-code" } }),
        serde_json::json!({ "timestamp": iso(base + 402), "type": "event_msg", "payload": { "type": "user_message", "message": "write trae.go" } }),
        serde_json::json!({ "timestamp": iso(base + 410), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "changes": { format!("{rs}/src/trae.go"): { "type": "add", "content": trae.join("\n") } } } } }),
    ];
    fs::write(tr.join("rollout-2026-09-22T10-00-00-trae1.jsonl"), rollout.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:aider/claude-sonnet-4-5"), 6, "{}", credits["breakdown"]);
    assert_eq!(verified("a:copilot/gpt-5.2"), 5, "{}", credits["breakdown"]);
    assert_eq!(verified("a:copilot/claude-sonnet-4-5"), 4, "{}", credits["breakdown"]);
    assert_eq!(verified("a:amp/claude-opus-4-6"), 4, "{}", credits["breakdown"]);
    assert_eq!(verified("a:trae/doubao-seed-code"), 3, "{}", credits["breakdown"]);
    // human.rs：没应用、报错、被拒绝的编辑都不算，记「未确认」
    let unattr = credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == "unattributed").map(|b| b["linesAdded"].as_u64().unwrap_or(0)).unwrap_or(0);
    assert!(unattr >= 3, "{}", credits["breakdown"]);
    let sessions = |h: &str| credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with(&format!("a:{h}/"))).count();
    for h in ["aider", "amp", "trae"] {
        assert_eq!(sessions(h), 1, "{h}: {}", credits["sessions"]);
    }
    assert_eq!(sessions("copilot"), 2, "{}", credits["sessions"]);

    // 英文模式：提醒、备注、摘要、doctor 都不带中文（项目名本身是中文，先去掉）
    let cjk = |s: &str| s.replace("示例项目", "").chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3000}'..='\u{303f}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c));
    let texts = |v: &serde_json::Value| -> Vec<String> { ["notes", "warnings", "tier"].iter().flat_map(|k| v[*k].as_array().cloned().unwrap_or_default()).filter_map(|x| x.as_str().map(String::from)).chain(v["summary"].as_str().map(String::from)).chain(v["tier"].as_str().map(String::from)).chain(v["sources"].as_array().into_iter().flatten().filter_map(|h| h["note"].as_str().map(String::from))).collect() };
    // App 要的进度：stderr 里一行一条，stdout 仍是完整的 JSON
    let o = Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(["analyze", "--repo", &rs, "--fresh", "--json"]).env_clear().envs(&env.vars).env("PATH", std::env::var("PATH").unwrap()).env("CREDITIA_PROGRESS", "1").output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    for stage in ["history", "logs", "attribute", "blame"] {
        assert!(err.lines().any(|l| l.starts_with("@progress ") && l.contains(&format!("\"stage\":\"{stage}\""))), "{stage}: {err}");
    }
    assert!(serde_json::from_slice::<serde_json::Value>(&o.stdout).is_ok());
    for args in [&["analyze", "--repo", rs.as_str(), "--lang", "en"][..], &["doctor", "--lang", "en"][..], &["discover", "--lang", "en"][..]] {
        let (_, v) = run(&env, args);
        let t = texts(&v);
        assert!(!t.is_empty() && t.iter().all(|s| !cjk(s)), "{args:?}: {t:?}");
    }
    let _ = fs::remove_dir_all(&env.root);
}

/// Crush：项目里的 .crush/crush.db（files 表逐版存内容），用户在两次编辑之间手改的不算；
/// 另一个文件只靠提交里的 `Assisted-by: Crush:模型` trailer 认
#[test]
fn crush_db_and_assisted_by() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_791_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("let {tag}_value_{i} = compute_{tag}({i}, {});", i * 17)).collect() };
    let (first, mine, second, trailer) = (lines("crush_first", 4), lines("by_hand", 2), lines("crush_second", 3), lines("assisted", 5));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(".gitignore"), ".crush/\n").unwrap();
    fs::write(repo.join("README.md"), "# crush demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    let all = [first.clone(), mine.clone(), second.clone()].concat();
    fs::write(repo.join("src/app.rs"), all.join("\n") + "\n").unwrap();
    commit(&repo, "crush work", "me@example.com", &d(base + 300));
    fs::write(repo.join("src/assisted.rs"), trailer.join("\n") + "\n").unwrap();
    commit(&repo, "more work\n\nAssisted-by: Crush:gpt-5", "me@example.com", &d(base + 400));

    // .crush/crush.db：v0 空文件 → v1 AI 写 first → v2 用户手改后的样子（加了 mine）→ 同一秒 v3 AI 写 second
    fs::create_dir_all(repo.join(".crush")).unwrap();
    let c = rusqlite::Connection::open(repo.join(".crush/crush.db")).unwrap();
    c.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, title TEXT, message_count INTEGER, prompt_tokens INTEGER, completion_tokens INTEGER, cost REAL, updated_at INTEGER, created_at INTEGER, summary_message_id TEXT, todos TEXT);
         CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT, role TEXT, parts TEXT, model TEXT, created_at INTEGER, updated_at INTEGER, finished_at INTEGER, provider TEXT, is_summary_message INTEGER);
         CREATE TABLE files (id TEXT PRIMARY KEY, session_id TEXT, path TEXT, content TEXT, version INTEGER, created_at INTEGER, updated_at INTEGER);",
    )
    .unwrap();
    let abs = format!("{rs}/src/app.rs");
    c.execute("INSERT INTO sessions VALUES ('cs1', NULL, 'build app', 6, 4000, 900, 0.1, ?1, ?2, NULL, NULL)", rusqlite::params![base + 250, base]).unwrap();
    let msgs = [("u1", "user", r#"[{"type":"text","data":{"text":"write app.rs"}}]"#, "", base + 10), ("a1", "assistant", "[]", "claude-sonnet-4-5", base + 20), ("u2", "user", r#"[{"type":"text","data":{"text":"add more"}}]"#, "", base + 150), ("a2", "assistant", "[]", "claude-sonnet-4-5", base + 160)];
    for (id, role, parts, model, ts) in msgs {
        c.execute("INSERT INTO messages VALUES (?1, 'cs1', ?2, ?3, ?4, ?5, ?5, ?5, 'anthropic', 0)", rusqlite::params![id, role, parts, model, ts]).unwrap();
    }
    let vs = [(0, String::new(), base + 30), (1, first.join("\n") + "\n", base + 30), (2, [first.clone(), mine.clone()].concat().join("\n") + "\n", base + 170), (3, all.join("\n") + "\n", base + 170)];
    for (v, content, ts) in vs {
        c.execute("INSERT INTO files VALUES (?1, 'cs1', ?2, ?3, ?4, ?5, ?5)", rusqlite::params![format!("f{v}"), abs, content, v, ts]).unwrap();
    }
    drop(c);
    // 全局登记表：discover 从这里找到项目
    let g = env.root.join(".local/share/crush");
    fs::create_dir_all(&g).unwrap();
    fs::write(g.join("projects.json"), serde_json::json!({ "projects": [{ "path": rs, "data_dir": format!("{rs}/.crush"), "last_accessed": "2026-09-22T10:00:00Z" }] }).to_string()).unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    // 用户手改的两行不在 AI 的证据里
    assert_eq!(verified("a:crush/claude-sonnet-4-5"), (first.len() + second.len()) as u64, "{}", credits["breakdown"]);
    let declared = credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == "a:crush/gpt-5").map(|b| b["linesAdded"].as_u64().unwrap_or(0)).unwrap_or(0);
    assert_eq!(declared, trailer.len() as u64, "{}", credits["breakdown"]);
    let s: Vec<&serde_json::Value> = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:crush/")).collect();
    assert_eq!(s.len(), 1, "{}", credits["sessions"]);

    let (_, v) = run(&env, &["discover"]);
    assert!(v.to_string().contains("crush"), "{v}");
    let (_, v) = run(&env, &["doctor"]);
    let h = v["sources"].as_array().unwrap().iter().find(|h| h["source"] == "crush").cloned().unwrap_or_default();
    assert_eq!(h["records"], 1, "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// Goose：sessions.db 里 toolRequest 配同 id 的 toolResponse，成功的 write / edit / shell 才算；报错的、别的项目的不算
#[test]
fn goose_sessions_db() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_792_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("def {tag}_step_{i}(x): return x * {} + {i}", i + 3)).collect() };
    let (written, edited, shell, failed) = (lines("goose_write", 4), lines("goose_edit", 2), lines("goose_shell", 3), lines("goose_failed", 2));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# goose demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/main.py"), [written.clone(), edited.clone(), failed.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/tool.py"), shell.join("\n") + "\n").unwrap();
    commit(&repo, "goose work", "me@example.com", &d(base + 600));

    let dir = env.root.join("goose/sessions");
    fs::create_dir_all(&dir).unwrap();
    let c = rusqlite::Connection::open(dir.join("sessions.db")).unwrap();
    c.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '', session_type TEXT NOT NULL DEFAULT 'user', working_dir TEXT NOT NULL, created_at TIMESTAMP, updated_at TIMESTAMP, total_tokens INTEGER, accumulated_total_tokens INTEGER, provider_name TEXT, model_config_json TEXT);
         CREATE TABLE messages (id INTEGER PRIMARY KEY AUTOINCREMENT, message_id TEXT, session_id TEXT NOT NULL, role TEXT NOT NULL, content_json TEXT NOT NULL, created_timestamp INTEGER NOT NULL, timestamp TIMESTAMP, tokens INTEGER, metadata_json TEXT);
         CREATE TABLE usage_ledger (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, created_timestamp INTEGER NOT NULL, model TEXT, input_tokens INTEGER, output_tokens INTEGER, total_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, cost REAL, cost_source TEXT, is_compaction INTEGER DEFAULT 0);",
    )
    .unwrap();
    let iso_sql = |t: i64| iso(t).replace('T', " ").trim_end_matches(".000Z").to_string();
    c.execute("INSERT INTO sessions (id, working_dir, created_at, updated_at, total_tokens, accumulated_total_tokens, provider_name, model_config_json) VALUES ('g1', ?1, ?2, ?3, 900, 5000, 'anthropic', '{\"model_name\":\"claude-sonnet-4-5\"}')", rusqlite::params![rs, iso_sql(base), iso_sql(base + 500)]).unwrap();
    c.execute("INSERT INTO sessions (id, working_dir, created_at, updated_at, model_config_json) VALUES ('other', '/somewhere/else', ?1, ?1, '{\"model_name\":\"gpt-5\"}')", rusqlite::params![iso_sql(base)]).unwrap();
    c.execute("INSERT INTO usage_ledger (session_id, created_timestamp, model) VALUES ('g1', ?1, 'claude-sonnet-4-5-20250929')", rusqlite::params![base + 15]).unwrap();
    let req = |id: &str, name: &str, args: serde_json::Value| serde_json::json!({ "type": "toolRequest", "id": id, "toolCall": { "status": "success", "value": { "name": name, "arguments": args } } });
    let res = |id: &str, err: bool| serde_json::json!({ "type": "toolResponse", "id": id, "toolResult": { "status": "success", "value": { "content": [{ "type": "text", "text": "ok" }], "isError": err } } });
    let main = format!("{rs}/src/main.py");
    let msgs = [
        ("user", serde_json::json!([{ "type": "text", "text": "write the module" }]), base + 10),
        ("assistant", serde_json::json!([req("t1", "developer__write", serde_json::json!({ "path": main, "content": written.join("\n") }))]), base + 20),
        ("user", serde_json::json!([res("t1", false)]), base + 21),
        ("assistant", serde_json::json!([req("t2", "edit", serde_json::json!({ "path": "src/main.py", "before": written[3], "after": format!("{}\n{}", written[3], edited.join("\n")) })), req("t3", "edit", serde_json::json!({ "path": main, "before": "missing", "after": failed.join("\n") }))]), base + 30),
        ("user", serde_json::json!([res("t2", false), res("t3", true)]), base + 31),
        ("assistant", serde_json::json!([req("t4", "shell", serde_json::json!({ "command": format!("cat > src/tool.py <<'EOF'\n{}\nEOF", shell.join("\n")) }))]), base + 40),
        ("user", serde_json::json!([res("t4", false)]), base + 41),
    ];
    for (role, content, ts) in msgs {
        c.execute("INSERT INTO messages (session_id, role, content_json, created_timestamp) VALUES ('g1', ?1, ?2, ?3)", rusqlite::params![role, content.to_string(), ts]).unwrap();
    }
    c.execute("INSERT INTO messages (session_id, role, content_json, created_timestamp) VALUES ('other', 'assistant', '[]', ?1)", rusqlite::params![base]).unwrap();
    drop(c);

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    // 报错的那次 edit（failed 两行）不算
    assert_eq!(verified("a:goose/claude-sonnet-4-5"), (written.len() + edited.len() + shell.len()) as u64, "{}", credits["breakdown"]);
    let s: Vec<&serde_json::Value> = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:goose/")).collect();
    assert_eq!(s.len(), 1, "{}", credits["sessions"]);
    let (_, v) = run(&env, &["doctor"]);
    let h = v["sources"].as_array().unwrap().iter().find(|h| h["source"] == "goose").cloned().unwrap_or_default();
    assert_eq!(h["records"], 2, "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// Continue：sessions/<id>.json 里 toolCallStates 为 done 的编辑才算；用户拒绝（canceled）、报错（errored）的不算
#[test]
fn continue_sessions() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_793_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("export const {tag}Item{i} = make{tag}({i}, {});", i * 11)).collect() };
    let (created, replaced, rejected, cli) = (lines("Created", 4), lines("Replaced", 2), lines("Rejected", 3), lines("Cli", 3));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# continue demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/a.ts"), [created.clone(), replaced.clone(), rejected.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/b.ts"), cli.join("\n") + "\n").unwrap();
    commit(&repo, "continue work", "me@example.com", &d(base + 600));

    let dir = env.root.join("continue/sessions");
    fs::create_dir_all(&dir).unwrap();
    let call = |id: &str, name: &str, args: serde_json::Value, status: &str| serde_json::json!({ "toolCallId": id, "toolCall": { "id": id, "type": "function", "function": { "name": name, "arguments": args.to_string() } }, "status": status, "parsedArgs": args });
    let uri = format!("file://{}", rs.replace(' ', "%20"));
    let ide = serde_json::json!({
        "sessionId": "c1", "title": "t", "workspaceDirectory": uri, "chatModelTitle": "Claude Sonnet 4.5",
        "usage": { "promptTokens": 3000, "completionTokens": 400, "totalCost": 0.1 },
        "history": [
            { "message": { "role": "user", "content": "make a.ts" }, "contextItems": [] },
            { "message": { "role": "assistant", "content": "" }, "contextItems": [], "promptLogs": [{ "modelTitle": "Claude Sonnet 4.5", "modelProvider": "anthropic", "prompt": "", "completion": "" }],
              "toolCallStates": [call("t1", "create_new_file", serde_json::json!({ "filepath": "src/a.ts", "contents": created.join("\n") }), "done")] },
            { "message": { "role": "tool", "content": "ok", "toolCallId": "t1" }, "contextItems": [] },
            { "message": { "role": "assistant", "content": "" }, "contextItems": [],
              "toolCallStates": [
                  call("t2", "single_find_and_replace", serde_json::json!({ "filepath": "src/a.ts", "old_string": created[3], "new_string": format!("{}\n{}", created[3], replaced.join("\n")) }), "done"),
                  call("t3", "edit_existing_file", serde_json::json!({ "filepath": "src/a.ts", "changes": format!("// ... existing code ...\n{}", rejected.join("\n")) }), "canceled"),
                  call("t4", "multi_edit", serde_json::json!({ "filepath": "src/a.ts", "edits": [{ "old_string": "zz", "new_string": rejected.join("\n") }] }), "errored"),
              ] },
        ]
    });
    fs::write(dir.join("c1.json"), ide.to_string()).unwrap();
    // 命令行（cn）：同样的格式，工具名是 Write / Edit
    let cn = serde_json::json!({
        "sessionId": "c2", "title": "t", "workspaceDirectory": rs, "chatModelTitle": "gpt-5",
        "history": [
            { "message": { "role": "user", "content": "make b.ts" }, "contextItems": [] },
            { "message": { "role": "assistant", "content": "" }, "contextItems": [], "toolCallStates": [call("w1", "Write", serde_json::json!({ "file_path": format!("{rs}/src/b.ts"), "content": cli.join("\n") }), "done")] },
        ]
    });
    fs::write(dir.join("c2.json"), cn.to_string()).unwrap();
    fs::write(dir.join("sessions.json"), serde_json::json!([{ "sessionId": "c1", "title": "t", "dateCreated": ((base + 10) * 1000).to_string(), "workspaceDirectory": uri }, { "sessionId": "c2", "title": "t", "dateCreated": ((base + 20) * 1000).to_string(), "workspaceDirectory": rs }]).to_string()).unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    // 被拒绝、报错的那几行没有逐行证据（和前面同一个改动块，只按块推断，不算 verified）
    assert_eq!(verified("a:continue/claude-sonnet-4-5"), (created.len() + replaced.len()) as u64, "{}", credits["breakdown"]);
    assert_eq!(verified("a:continue/gpt-5"), cli.len() as u64, "{}", credits["breakdown"]);
    let s = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:continue/")).count();
    assert_eq!(s, 2, "{}", credits["sessions"]);
    let _ = fs::remove_dir_all(&env.root);
}

/// Zed：threads.db 里 zstd 压缩的线程，edit_file / write_file 的结果带改前改后的内容；报错的不算，别的项目的线程不读
#[test]
fn zed_threads() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let root_name = repo.file_name().unwrap().to_string_lossy().to_string();
    let base: i64 = 1_794_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("pub fn {tag}_part_{i}() -> usize {{ {} }}", i * 19 + 1)).collect() };
    let (old, wrote, edited, failed) = (lines("kept", 2), lines("zed_write", 4), lines("zed_edit", 3), lines("zed_failed", 2));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/lib.rs"), old.join("\n") + "\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/new.rs"), [wrote.clone(), failed.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/lib.rs"), [old.clone(), edited.clone()].concat().join("\n") + "\n").unwrap();
    commit(&repo, "zed work", "me@example.com", &d(base + 600));

    let dir = env.root.join("zed/threads");
    fs::create_dir_all(&dir).unwrap();
    let c = rusqlite::Connection::open(dir.join("threads.db")).unwrap();
    c.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL, updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL, parent_id TEXT, folder_paths TEXT, folder_paths_order TEXT, created_at TEXT);").unwrap();
    let tool = |id: &str, name: &str, input: serde_json::Value| serde_json::json!({ "ToolUse": { "id": id, "name": name, "raw_input": input.to_string(), "input": { "type": "json", "value": input }, "is_input_complete": true } });
    let ok = |id: &str, name: &str, out: serde_json::Value| serde_json::json!({ "tool_use_id": id, "tool_name": name, "is_error": false, "content": "done", "output": out });
    let lib = format!("{root_name}/src/lib.rs");
    let both = [old.clone(), edited.clone()].concat().join("\n") + "\n";
    let new = format!("{root_name}/src/new.rs");
    let thread = serde_json::json!({
        "version": "0.3.0", "title": "t", "updated_at": iso(base + 300),
        "model": { "provider": "anthropic", "model": "claude-sonnet-4-5" },
        "cumulative_token_usage": { "input_tokens": 2000, "output_tokens": 300 },
        "messages": [
            { "User": { "id": "u1", "content": [{ "Text": "add things" }] } },
            { "Agent": {
                "content": [tool("t1", "write_file", serde_json::json!({ "path": new })), tool("t2", "edit_file", serde_json::json!({ "path": lib })), tool("t3", "edit_file", serde_json::json!({ "path": new }))],
                "tool_results": {
                    "t1": ok("t1", "write_file", serde_json::json!({ "input_path": new, "old_text": "", "new_text": wrote.join("\n") + "\n", "diff": "" })),
                    "t2": ok("t2", "edit_file", serde_json::json!({ "input_path": lib, "old_text": old.join("\n") + "\n", "new_text": both, "diff": "" })),
                    "t3": { "tool_use_id": "t3", "tool_name": "edit_file", "is_error": true, "content": "no match", "output": { "error": "no match", "input_path": new } }
                },
                "reasoning_details": null } }
        ]
    });
    let blob = zstd::stream::encode_all(thread.to_string().as_bytes(), 3).unwrap();
    let ts = |t: i64| iso(t).replace(".000Z", "Z");
    c.execute("INSERT INTO threads (id, summary, updated_at, data_type, data, folder_paths, folder_paths_order, created_at) VALUES ('z1', 't', ?1, 'zstd', ?2, ?3, '0', ?4)", rusqlite::params![ts(base + 300), blob, rs, ts(base + 5)]).unwrap();
    // 别的项目：解不开也不该去读
    c.execute("INSERT INTO threads (id, summary, updated_at, data_type, data, folder_paths, created_at) VALUES ('z2', 't', ?1, 'zstd', x'00', '/elsewhere/proj', ?1)", rusqlite::params![ts(base)]).unwrap();
    drop(c);

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    // 报错的 t3 没有证据：failed 两行跟在 write 那段后面，只按块推断
    assert_eq!(verified("a:zed/claude-sonnet-4-5"), (wrote.len() + edited.len()) as u64, "{}", credits["breakdown"]);
    let s = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:zed/")).count();
    assert_eq!(s, 1, "{}", credits["sessions"]);
    let (_, v) = run(&env, &["discover"]);
    assert!(v.to_string().contains("\"zed\""), "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// Amazon Q CLI：data.sqlite3 的 conversations 表（按目录存对话 JSON），fs_write 成功的才算；结果是 Error 的不算
#[test]
fn amazon_q_conversations() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_795_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("{tag}_setting_{i} = load_{tag}(\"{i}\", {})", i * 23)).collect() };
    let (created, appended, failed) = (lines("q_create", 4), lines("q_append", 2), lines("q_failed", 2));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# q demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/conf.py"), [created.clone(), appended.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/other.py"), failed.join("\n") + "\n").unwrap();
    commit(&repo, "q work", "me@example.com", &d(base + 600));

    let dir = env.root.join("amazon-q");
    fs::create_dir_all(&dir).unwrap();
    let c = rusqlite::Connection::open(dir.join("data.sqlite3")).unwrap();
    c.execute_batch("CREATE TABLE conversations (key TEXT PRIMARY KEY, value TEXT);").unwrap();
    let tool = |id: &str, args: serde_json::Value| serde_json::json!({ "id": id, "name": "fs_write", "orig_name": "fs_write", "args": args, "orig_args": args });
    let conv = serde_json::json!({
        "conversation_id": "qc1", "model": "claude-sonnet-4",
        "history": [
            { "user": { "additional_context": "", "env_context": { "env_state": { "current_working_directory": rs } }, "content": { "Prompt": { "prompt": "write conf.py" } }, "timestamp": null },
              "assistant": { "ToolUse": { "message_id": "m1", "content": "", "tool_uses": [
                  tool("a", serde_json::json!({ "command": "create", "path": format!("{rs}/src/conf.py"), "file_text": created.join("\n"), "summary": "s" })),
                  tool("b", serde_json::json!({ "command": "create", "path": "src/other.py", "file_text": failed.join("\n") })) ] } },
              "request_metadata": { "message_id": "m1", "request_start_timestamp_ms": (base + 20) * 1000, "stream_end_timestamp_ms": (base + 25) * 1000, "model_id": "claude-sonnet-4.5" } },
            { "user": { "additional_context": "", "env_context": {}, "content": { "ToolUseResults": { "tool_use_results": [{ "tool_use_id": "a", "content": [], "status": "Success" }, { "tool_use_id": "b", "content": [], "status": "Error" }] } }, "timestamp": null },
              "assistant": { "ToolUse": { "message_id": "m2", "content": "", "tool_uses": [tool("c", serde_json::json!({ "command": "append", "path": "src/conf.py", "new_str": appended.join("\n") }))] } },
              "request_metadata": { "message_id": "m2", "request_start_timestamp_ms": (base + 40) * 1000, "stream_end_timestamp_ms": (base + 45) * 1000, "model_id": "claude-sonnet-4.5" } },
            { "user": { "additional_context": "", "env_context": {}, "content": { "ToolUseResults": { "tool_use_results": [{ "tool_use_id": "c", "content": [], "status": "Success" }] } }, "timestamp": null },
              "assistant": { "Response": { "message_id": "m3", "content": "done" } },
              "request_metadata": { "message_id": "m3", "request_start_timestamp_ms": (base + 60) * 1000, "stream_end_timestamp_ms": (base + 61) * 1000, "model_id": "claude-sonnet-4.5" } }
        ]
    });
    c.execute("INSERT INTO conversations VALUES (?1, ?2)", rusqlite::params![rs, conv.to_string()]).unwrap();
    c.execute("INSERT INTO conversations VALUES ('/elsewhere', '{\"conversation_id\":\"x\",\"history\":[]}')", []).unwrap();
    drop(c);

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:amazon-q/claude-sonnet-4-5"), (created.len() + appended.len()) as u64, "{}", credits["breakdown"]);
    let s = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:amazon-q/")).count();
    assert_eq!(s, 1, "{}", credits["sessions"]);
    let _ = fs::remove_dir_all(&env.root);
}

/// OpenCode 新版的 SQLite：v1 表（session / message / part）和 v2 表（session_v2 / session_message）都读；
/// JSON 文件和库里同一个会话只算一次；Kilo 命令行（OpenCode 的分支）的 kilo.db 记成 Kilo Code
#[test]
fn opencode_sqlite_and_kilo_cli() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_796_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("export function {tag}Step{i}(x: number) {{ return x * {} }}", i + 7)).collect() };
    let (v1, v2, patch, kilo) = (lines("legacyTable", 3), lines("sessionV2", 4), lines("patched", 2), lines("kiloCli", 3));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# oc demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/v1.ts"), v1.join("\n") + "\n").unwrap();
    fs::write(repo.join("src/v2.ts"), [v2.clone(), patch.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/kilo.ts"), kilo.join("\n") + "\n").unwrap();
    commit(&repo, "oc work", "me@example.com", &d(base + 600));

    let ms = |t: i64| t * 1000;
    let schema = "CREATE TABLE session (id text PRIMARY KEY, project_id text NOT NULL, parent_id text, slug text NOT NULL, directory text NOT NULL, title text NOT NULL, version text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL);
        CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);
        CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, session_id text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);
        CREATE TABLE session_v2 (id text PRIMARY KEY, project_id text NOT NULL, slug text NOT NULL, directory text NOT NULL, version text NOT NULL, tokens_input integer DEFAULT 0 NOT NULL, tokens_output integer DEFAULT 0 NOT NULL, tokens_reasoning integer DEFAULT 0 NOT NULL, tokens_cache_read integer DEFAULT 0 NOT NULL, tokens_cache_write integer DEFAULT 0 NOT NULL, model text, time_created integer NOT NULL, time_updated integer NOT NULL);
        CREATE TABLE session_message (id text PRIMARY KEY, session_id text NOT NULL, type text NOT NULL, seq integer NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);";
    let v1_session = |c: &rusqlite::Connection, sid: &str, path: &str, model: &str, text: &[String]| {
        c.execute("INSERT INTO session VALUES (?1, 'p', NULL, 's', ?2, 't', '1', ?3, ?4)", rusqlite::params![sid, rs, ms(base), ms(base + 100)]).unwrap();
        c.execute("INSERT INTO message VALUES (?1, ?2, ?3, ?3, ?4)", rusqlite::params![format!("{sid}-u"), sid, ms(base + 5), serde_json::json!({ "role": "user", "time": { "created": ms(base + 5) } }).to_string()]).unwrap();
        c.execute("INSERT INTO message VALUES (?1, ?2, ?3, ?3, ?4)", rusqlite::params![format!("{sid}-a"), sid, ms(base + 10), serde_json::json!({ "role": "assistant", "modelID": model, "providerID": "anthropic", "time": { "created": ms(base + 10) }, "path": { "cwd": rs }, "tokens": { "input": 100, "output": 20 } }).to_string()]).unwrap();
        c.execute("INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?4, ?5)", rusqlite::params![format!("{sid}-p"), format!("{sid}-a"), sid, ms(base + 12), serde_json::json!({ "type": "tool", "tool": "write", "state": { "status": "completed", "input": { "filePath": format!("{rs}/{path}"), "content": text.join("\n") }, "time": { "end": ms(base + 12) } } }).to_string()]).unwrap();
    };

    // OpenCode：opencode.db 里一个 v1 会话、一个 v2 会话
    let oc = env.root.join("opencode");
    fs::create_dir_all(&oc).unwrap();
    let c = rusqlite::Connection::open(oc.join("opencode.db")).unwrap();
    c.execute_batch(schema).unwrap();
    v1_session(&c, "ses_v1", "src/v1.ts", "claude-sonnet-4-5", &v1);
    c.execute("INSERT INTO session_v2 (id, project_id, slug, directory, version, tokens_input, tokens_output, time_created, time_updated) VALUES ('ses_v2', 'p', 's', ?1, '2', 3000, 500, ?2, ?3)", rusqlite::params![rs, ms(base), ms(base + 200)]).unwrap();
    let tool = |name: &str, input: serde_json::Value| serde_json::json!({ "type": "tool", "id": name, "name": name, "state": { "status": "completed", "input": input, "content": [{ "type": "text", "text": "ok" }] }, "time": { "created": ms(base + 30), "completed": ms(base + 31) } });
    let assistant = serde_json::json!({ "agent": "build", "model": { "id": "gpt-5", "providerID": "openai" }, "time": { "created": ms(base + 30) }, "content": [
        tool("write", serde_json::json!({ "path": "src/v2.ts", "content": v2.join("\n") })),
        tool("patch", serde_json::json!({ "patchText": format!("*** Begin Patch\n*** Update File: src/v2.ts\n@@\n{}\n*** End Patch", patch.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n")) })),
        { "type": "tool", "id": "x", "name": "write", "state": { "status": "error", "input": { "path": "src/kilo.ts", "content": kilo.join("\n") }, "error": { "type": "x" } }, "time": { "created": ms(base + 32) } }
    ] });
    c.execute("INSERT INTO session_message VALUES ('m1', 'ses_v2', 'user', 1, ?1, ?1, ?2)", rusqlite::params![ms(base + 20), serde_json::json!({ "text": "go", "time": { "created": ms(base + 20) } }).to_string()]).unwrap();
    c.execute("INSERT INTO session_message VALUES ('m2', 'ses_v2', 'assistant', 2, ?1, ?1, ?2)", rusqlite::params![ms(base + 30), assistant.to_string()]).unwrap();
    drop(c);
    // 同一个 v1 会话也留在老的 JSON 文件里：只算一次
    let st = oc.join("storage");
    fs::create_dir_all(st.join("session/p")).unwrap();
    fs::write(st.join("session/p/ses_v1.json"), serde_json::json!({ "id": "ses_v1", "directory": rs, "time": { "created": ms(base), "updated": ms(base + 100) } }).to_string()).unwrap();

    // Kilo 命令行
    let k = env.root.join("kilo");
    fs::create_dir_all(&k).unwrap();
    let c = rusqlite::Connection::open(k.join("kilo.db")).unwrap();
    c.execute_batch(schema).unwrap();
    v1_session(&c, "ses_kilo", "src/kilo.ts", "claude-opus-4-6", &kilo);
    drop(c);
    let mut env = env;
    env.vars.insert("CREDITIA_KILO_CLI_DIR".into(), k.display().to_string());

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:opencode/claude-sonnet-4-5"), v1.len() as u64, "{}", credits["breakdown"]);
    assert_eq!(verified("a:opencode/gpt-5"), (v2.len() + patch.len()) as u64, "{}", credits["breakdown"]);
    assert_eq!(verified("a:kilo/claude-opus-4-6"), kilo.len() as u64, "{}", credits["breakdown"]);
    let sessions = |h: &str| credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with(&format!("a:{h}/"))).count();
    assert_eq!(sessions("opencode"), 2, "{}", credits["sessions"]);
    assert_eq!(sessions("kilo"), 1, "{}", credits["sessions"]);
    let (_, v) = run(&env, &["doctor"]);
    assert!(v["sources"].as_array().unwrap().iter().any(|h| h["source"] == "kilo-cli" && h["records"] == 1), "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// Antigravity CLI（agy）：brain/<对话>/…/transcript_full.jsonl；工具调用和结果按先后对上，结果不是 DONE 的不算；
/// 工作目录取 cache/conversation_metadata.json，模型取 USER_INPUT 里的模型设置变更
#[test]
fn antigravity_cli_transcript() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_796_000_000;
    let d = |t: i64| format!("@{t} +0800");
    // base 是 2026-11-30T00:53:20Z，下面的偏移都在一小时内
    let iso = |t: i64| { let s = t - base + 3200; format!("2026-11-30T00:{:02}:{:02}Z", s / 60, s % 60) };
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("{tag}_value_{i} = compute_{tag}({i}, {})", i * 31)).collect() };
    let (wrote, replaced, failed) = (lines("agy_write", 4), lines("agy_replace", 2), lines("agy_failed", 2));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# agy demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/calc.py"), [wrote.clone(), replaced.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/other.py"), failed.join("\n") + "\n").unwrap();
    commit(&repo, "agy work", "me@example.com", &d(base + 600));

    let root = env.root.join("agy");
    let id = "7d1e0c3a-5b6f-4a2e-9c1d-0e2f3a4b5c6d";
    let logs = root.join("brain").join(id).join(".system_generated/logs");
    fs::create_dir_all(&logs).unwrap();
    fs::create_dir_all(root.join("cache")).unwrap();
    fs::write(root.join("cache/conversation_metadata.json"), serde_json::json!({ "conversations": {
        id: { "summary": { "ID": id, "Title": "calc", "WorkspaceURIs": [format!("file://{rs}")], "Internal": false }, "is_internal": false }
    } }).to_string()).unwrap();
    let grown = std::iter::once(&wrote[3]).chain(&replaced).cloned().collect::<Vec<_>>().join("\n");
    let steps = [
        serde_json::json!({ "step_index": 0, "source": "USER_EXPLICIT", "type": "USER_INPUT", "status": "DONE", "created_at": iso(base),
            "content": "<USER_REQUEST>\nwrite calc.py\n</USER_REQUEST>\n<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Claude Sonnet 4.5 (Thinking). No need to comment on this change.\n</USER_SETTINGS_CHANGE>" }),
        serde_json::json!({ "step_index": 1, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": iso(base + 10), "tool_calls": [
            { "name": "write_to_file", "args": { "TargetFile": format!("{rs}/src/calc.py"), "CodeContent": wrote.join("\n"), "Overwrite": false, "EmptyFile": false } },
            { "name": "write_to_file", "args": { "TargetFile": format!("{rs}/src/other.py"), "CodeContent": failed.join("\n") } } ] }),
        serde_json::json!({ "step_index": 2, "source": "MODEL", "type": "CODE_ACTION", "status": "DONE", "created_at": iso(base + 11), "content": "Created file" }),
        serde_json::json!({ "step_index": 3, "source": "MODEL", "type": "CODE_ACTION", "status": "ERROR", "created_at": iso(base + 12), "content": "denied" }),
        serde_json::json!({ "step_index": 4, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": iso(base + 20), "tool_calls": [
            { "name": "replace_file_content", "args": { "TargetFile": "src/calc.py", "TargetContent": wrote[3], "ReplacementContent": grown, "StartLine": 4, "EndLine": 4 } } ] }),
        serde_json::json!({ "step_index": 5, "source": "MODEL", "type": "CODE_ACTION", "status": "DONE", "created_at": iso(base + 21), "content": "Edited file" }),
        serde_json::json!({ "step_index": 6, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": iso(base + 30), "content": "done" }),
    ];
    fs::write(logs.join("transcript_full.jsonl"), steps.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:antigravity/claude-sonnet-4-5"), (wrote.len() + replaced.len()) as u64, "{}", credits["breakdown"]);
    let s = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:antigravity/")).count();
    assert_eq!(s, 1, "{}", credits["sessions"]);
    let (_, v) = run(&env, &["discover"]);
    assert!(v.to_string().contains("\"antigravity\""), "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// Qwen Code 新版：projects/<目录>/chats/<会话>.jsonl，每条记录带 cwd；toolCallResult 为 success 的调用才算
#[test]
fn qwen_code_projects_jsonl() {
    let env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_796_100_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> Vec<String> { (0..n).map(|i| format!("{tag}_entry_{i} = parse_{tag}(\"{i}\", {})", i * 17)).collect() };
    let (wrote, edited, failed) = (lines("qw_write", 4), lines("qw_edit", 2), lines("qw_failed", 2));

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("README.md"), "# qwen demo\n").unwrap();
    commit(&repo, "init", "me@example.com", &d(base - 86400));
    fs::write(repo.join("src/parse.py"), [wrote.clone(), edited.clone()].concat().join("\n") + "\n").unwrap();
    fs::write(repo.join("src/other.py"), failed.join("\n") + "\n").unwrap();
    commit(&repo, "qwen work", "me@example.com", &d(base + 600));

    let chats = env.root.join("qwen/projects").join(rs.replace('/', "-")).join("chats");
    fs::create_dir_all(&chats).unwrap();
    let rec = |t: &str, off: i64, extra: serde_json::Value| {
        let mut v = serde_json::json!({ "uuid": format!("u{off}"), "parentUuid": null, "sessionId": "qs1", "type": t, "cwd": rs, "version": "0.9.0", "gitBranch": "main" });
        v["timestamp"] = serde_json::json!((base + off) * 1000);
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        v
    };
    let call = |id: &str, name: &str, args: serde_json::Value| serde_json::json!({ "functionCall": { "id": id, "name": name, "args": args } });
    let result = |off: i64, id: &str, status: &str| rec("tool_result", off, serde_json::json!({
        "message": { "role": "user", "parts": [{ "functionResponse": { "id": id, "name": "x", "response": { "output": "ok" } } }] },
        "toolCallResult": { "callId": id, "status": status } }));
    let recs = [
        rec("user", 0, serde_json::json!({ "message": { "role": "user", "parts": [{ "text": "write parse.py" }] } })),
        rec("assistant", 10, serde_json::json!({ "model": "qwen3-coder-plus", "usageMetadata": { "totalTokenCount": 900 }, "message": { "role": "model", "parts": [
            call("a", "write_file", serde_json::json!({ "file_path": format!("{rs}/src/parse.py"), "content": wrote.join("\n") })),
            call("b", "write_file", serde_json::json!({ "file_path": "src/other.py", "content": failed.join("\n") })) ] } })),
        result(11, "a", "success"),
        result(12, "b", "error"),
        rec("assistant", 20, serde_json::json!({ "model": "qwen3-coder-plus", "message": { "role": "model", "parts": [
            call("c", "edit", serde_json::json!({ "file_path": "src/parse.py", "old_string": wrote[3], "new_string": std::iter::once(&wrote[3]).chain(&edited).cloned().collect::<Vec<_>>().join("\n") })) ] } })),
        result(21, "c", "success"),
    ];
    fs::write(chats.join("qs1.jsonl"), recs.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap();
    let verified = |id: &str| credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    assert_eq!(verified("a:qwen/qwen3-coder-plus"), (wrote.len() + edited.len()) as u64, "{}", credits["breakdown"]);
    let s = credits["sessions"].as_array().unwrap().iter().filter(|s| s["contributor"].as_str().unwrap_or("").starts_with("a:qwen/")).count();
    assert_eq!(s, 1, "{}", credits["sessions"]);
    let (_, v) = run(&env, &["doctor"]);
    assert!(v["sources"].as_array().unwrap().iter().any(|h| h["source"] == "qwen" && h["records"] == 1), "{v}");
    let _ = fs::remove_dir_all(&env.root);
}

/// 两项接入：没决定过的列在 ask 里；装、卸都记下决定；core.hooksPath 指向别处时 hook 写到那里；不是 creditia 写的 workflow 不覆盖
#[test]
fn integrations_switches() {
    let env = setup();
    let repo = env.repo.display().to_string();
    fs::write(env.repo.join("a.txt"), "a\n").unwrap();
    commit(&env.repo, "init", "me@example.com", "2026-10-01T10:00:00Z");

    let (ok, v) = run(&env, &["init", "--repo", &repo]);
    assert!(ok, "{v}");
    assert_eq!(v["ask"], serde_json::json!(["hook", "release"]), "{v}");
    assert!(v.get("askRepo").is_none(), "内部字段不出现在 JSON 里：{v}");

    git(&env.repo, &["config", "core.hooksPath", ".githooks"]);
    let (ok, v) = run(&env, &["hook", "install", "--repo", &repo]);
    assert!(ok, "{v}");
    assert!(fs::read_to_string(env.repo.join(".githooks/prepare-commit-msg")).unwrap().contains("# creditia-stamp"));

    let (ok, v) = run(&env, &["release", "uninstall", "--repo", &repo]);
    assert!(ok, "{v}");
    let (_, v) = run(&env, &["integrations", "--repo", &repo]);
    assert_eq!((v["hook"]["installed"].as_bool(), v["hook"]["decided"].as_bool()), (Some(true), Some(true)), "{v}");
    assert_eq!((v["release"]["installed"].as_bool(), v["release"]["decided"].as_bool()), (Some(false), Some(false)), "{v}");
    let (_, v) = run(&env, &["analyze", "--repo", &repo]);
    assert!(v.get("ask").is_none(), "决定过就不再问：{v}");

    // 用户自己的同名 workflow 不覆盖；creditia 写的能装能删
    let wf = env.repo.join(".github/workflows/creditia.yml");
    fs::create_dir_all(wf.parent().unwrap()).unwrap();
    fs::write(&wf, "name: mine\n").unwrap();
    let (ok, v) = run(&env, &["release", "install", "--repo", &repo]);
    assert!(!ok, "{v}");
    assert_eq!(fs::read_to_string(&wf).unwrap(), "name: mine\n");
    fs::remove_file(&wf).unwrap();
    let (ok, v) = run(&env, &["release", "install", "--repo", &repo]);
    assert!(ok, "{v}");
    let text = fs::read_to_string(&wf).unwrap();
    assert!(text.starts_with("# creditia-release") && text.contains("release-notes: true") && text.contains("fetch-depth: 0"), "{text}");
    let (ok, _) = run(&env, &["release", "uninstall", "--repo", &repo]);
    assert!(ok && !wf.exists());

    // 先拨了开关（半份配置）再 init：不算已初始化，决定带过去
    fs::remove_file(env.repo.join(".creditia/config.json")).unwrap();
    let (ok, _) = run(&env, &["hook", "uninstall", "--repo", &repo]);
    assert!(ok);
    let (ok, v) = run(&env, &["init", "--repo", &repo]);
    assert!(ok, "{v}");
    assert_eq!(v["config"]["integrations"], serde_json::json!({ "hook": false }), "{v}");
    assert_eq!(v["ask"], serde_json::json!(["release"]), "{v}");

    let _ = fs::remove_dir_all(&env.root);
}

#[test]
fn collect_all_keeps_going_past_a_broken_project() {
    let env = setup();
    fs::write(env.repo.join("a.txt"), "a\n").unwrap();
    commit(&env.repo, "a", "me@example.com", "2026-07-01T10:00:00+08:00");
    let other = env.root.join("projects/other");
    fs::create_dir_all(&other).unwrap();
    git(&other, &["init", "-q", "-b", "main"]);
    fs::write(other.join("b.txt"), "b\n").unwrap();
    commit(&other, "b", "me@example.com", "2026-07-01T11:00:00+08:00");
    let (ok, v) = run(&env, &["collect", "--repo", env.repo.to_str().unwrap()]);
    assert!(ok, "{v}");
    let (ok, v) = run(&env, &["collect", "--repo", other.to_str().unwrap()]);
    assert!(ok, "{v}");

    // other 的缓存目录换成一个文件：它写不进去，另一个照收
    let broken = env.root.join("lib/cache").join(v["collected"][0]["project"].as_str().unwrap());
    fs::remove_dir_all(&broken).unwrap();
    fs::write(&broken, "").unwrap();
    let (ok, v) = run(&env, &["collect", "--all"]);
    assert!(ok, "{v}");
    let c = v["collected"].as_array().unwrap();
    assert_eq!(c.len(), 2, "{v}");
    assert_eq!(c.iter().filter(|x| x["error"].is_string()).count(), 1, "{v}");
    assert_eq!(c.iter().filter(|x| x["newEvidence"].is_u64()).count(), 1, "{v}");
    assert_eq!(v["warnings"].as_array().map(Vec::len), Some(1), "{v}");

    // 只收一个项目时出错照旧报错
    let (ok, _) = run(&env, &["collect", "--repo", other.to_str().unwrap()]);
    assert!(!ok);

    let _ = fs::remove_dir_all(&env.root);
}

#[test]
fn detached_checkout_analyzes_the_checked_out_commit() {
    let env = setup();
    fs::write(env.repo.join("hello.rs"), "pub fn hello() {}\n").unwrap();
    let head = commit(&env.repo, "initial", "me@example.com", "2026-09-01T10:00:00+00:00");
    git(&env.repo, &["checkout", "--detach", &head]);
    git(&env.repo, &["branch", "-D", "main"]);
    let (ok, report) = run(&env, &["analyze", "--repo", env.repo.to_str().unwrap()]);
    assert!(ok, "{report}");
    assert_eq!(report["head"], head);
    assert_eq!(report["commits"], 1);
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap()).unwrap();
    assert_eq!(raw["head"], head);
    assert_eq!(raw["files"].as_array().unwrap().len(), 1);
    let (ok, collected) = run(&env, &["collect", "--repo", env.repo.to_str().unwrap()]);
    assert!(ok, "{collected}");
}

#[test]
fn missing_configured_revision_is_an_error() {
    let env = setup();
    fs::write(env.repo.join("hello.rs"), "pub fn hello() {}\n").unwrap();
    commit(&env.repo, "initial", "me@example.com", "2026-09-01T10:00:00+00:00");
    fs::create_dir_all(env.repo.join(".creditia")).unwrap();
    fs::write(env.repo.join(".creditia/config.json"), r#"{"name":"test","repo":"","branch":"missing-release-branch"}"#).unwrap();
    for command in ["analyze", "collect"] {
        let (ok, report) = run(&env, &[command, "--repo", env.repo.to_str().unwrap()]);
        assert!(!ok, "{command} must reject a missing revision: {report}");
        assert!(report["error"].as_str().unwrap().contains("missing-release-branch"), "{report}");
    }
    assert!(!env.repo.join(".creditia/credits.json").exists());
}

#[test]
fn empty_repository_cannot_produce_a_successful_analysis() {
    let env = setup();
    let (ok, report) = run(&env, &["analyze", "--repo", env.repo.to_str().unwrap()]);
    assert!(!ok, "{report}");
    assert!(report["error"].is_string(), "{report}");
    assert!(!env.repo.join(".creditia/credits.json").exists());
}

/// 伪造 Claude Code 一次 Write：返回日志文件路径
fn claude_write(env: &Env, sid: &str, rel: &str, content: &str, ts: i64) -> PathBuf {
    let dir = env.root.join("claude/projects/-e2e-write");
    fs::create_dir_all(&dir).unwrap();
    let iso = |t: i64| {
        let (y, m, d) = creditia_core::util::civil_from_days(t.div_euclid(86400));
        let s = t.rem_euclid(86400);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", s / 3600, s / 60 % 60, s % 60)
    };
    let cwd = env.repo.display().to_string();
    let lines = [
        serde_json::json!({ "type": "user", "cwd": cwd, "sessionId": sid, "timestamp": iso(ts - 100), "message": { "role": "user", "content": "go" } }),
        serde_json::json!({ "type": "assistant", "cwd": cwd, "sessionId": sid, "timestamp": iso(ts - 50), "message": { "id": format!("msg_{sid}"), "model": "claude-test", "usage": { "input_tokens": 1, "output_tokens": 1 }, "content": [{ "type": "tool_use", "id": format!("tu_{sid}"), "name": "Write", "input": { "file_path": env.repo.join(rel).display().to_string(), "content": content } }] } }),
        serde_json::json!({ "type": "user", "cwd": cwd, "sessionId": sid, "timestamp": iso(ts), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": format!("tu_{sid}"), "content": "File created successfully" }] } }),
    ];
    let p = dir.join(format!("{sid}.jsonl"));
    fs::write(&p, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    p
}

fn verified_of(env: &Env, id: &str) -> u64 {
    let credits: serde_json::Value = serde_json::from_str(&fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap()).unwrap();
    credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap()).unwrap_or(0)
}

/// agent 新建的文件，提交之前先跑了一次分析（提交 hook 每次都这样）：提交以后这个文件的证据不能丢
#[test]
fn new_file_analyzed_before_commit_keeps_its_evidence() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let base: i64 = 1_787_000_000;
    let d = |t: i64| format!("@{t} +0800");
    fs::write(env.repo.join(".git/info/exclude"), ".creditia/\n").unwrap();
    fs::write(env.repo.join("README.md"), "# a readme line for the project\n").unwrap();
    commit(&env.repo, "init", "me@example.com", &d(base - 86400));
    let content: String = (0..20).map(|i| format!("pub fn written_by_agent_{i}() -> u32 {{ {i} * 11 }}\n")).collect();
    claude_write(&env, "s-new", "src/new.rs", &content, base);
    fs::create_dir_all(env.repo.join("src")).unwrap();
    fs::write(env.repo.join("src/new.rs"), &content).unwrap();
    run(&env, &["init", "--repo", &rs]);
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    commit(&env.repo, "add new.rs", "me@example.com", &d(base + 60));
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified_of(&env, "a:claude-code/claude-test"), 20);
}

/// 一次写入里重复的行：日志在的时候、删了日志再重建缓存以后，逐行证据一样多
#[test]
fn repeated_lines_survive_log_cleanup() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let base: i64 = 1_787_000_000;
    let d = |t: i64| format!("@{t} +0800");
    fs::write(env.repo.join(".git/info/exclude"), ".creditia/\n").unwrap();
    fs::write(env.repo.join("README.md"), "# a readme line for the project\n").unwrap();
    commit(&env.repo, "init", "me@example.com", &d(base - 86400));
    let content: String = (0..6).map(|i| format!("    let value_{i} = compute_something({i});\n")).collect::<String>() + &"        assert_eq!(result.status, Status::Ok);\n".repeat(10);
    let log = claude_write(&env, "s-dup", "tests/t.rs", &content, base);
    fs::create_dir_all(env.repo.join("tests")).unwrap();
    fs::write(env.repo.join("tests/t.rs"), &content).unwrap();
    commit(&env.repo, "tests", "me@example.com", &d(base + 60));
    run(&env, &["init", "--repo", &rs]);
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified_of(&env, "a:claude-code/claude-test"), 16);
    fs::remove_file(log).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs, "--fresh"]);
    assert!(ok, "{v}");
    assert_eq!(verified_of(&env, "a:claude-code/claude-test"), 16);
}

/// 团队仓库：别人的提交算那个人（写 git 名字，可以关），提交里的线索照样认成 agent；
/// 工具、贡献者、未确认合起来是全部新增行；authors 按提交者分，每位里面再按工具分
#[test]
fn team_repo_contributors() {
    let env = setup();
    let rs = env.repo.display().to_string();
    let base: i64 = 1_787_000_000;
    let d = |t: i64| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| -> String { (0..n).map(|i| format!("let {tag}_value_{i} = compute_{tag}({i});\n")).collect() };
    fs::write(env.repo.join(".git/info/exclude"), ".creditia/\n").unwrap();
    // 我：三个提交（是这个仓库的作者），最后一个没有任何 AI 证据
    fs::write(env.repo.join("README.md"), lines("readme", 2)).unwrap();
    commit_as(&env.repo, "init", "Me", "me@example.com", &d(base - 86400));
    fs::write(env.repo.join("mine.rs"), lines("mine", 3)).unwrap();
    commit_as(&env.repo, "mine", "Me", "me@example.com", &d(base));
    fs::write(env.repo.join("mine2.rs"), lines("mine_more", 1)).unwrap();
    commit_as(&env.repo, "more", "Me", "me@example.com", &d(base + 10));
    // Ana：没有线索 → 算 Ana
    fs::write(env.repo.join("ana.rs"), lines("ana", 6)).unwrap();
    commit_as(&env.repo, "ana's work", "Ana Lima", "ana@team.dev", &d(base + 100));
    // Bo：提交里写了 Co-authored-by Claude → 算 Claude Code，提交者还是 Bo
    fs::write(env.repo.join("bo.rs"), lines("bo", 4)).unwrap();
    commit_as(&env.repo, "bo's work\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>", "Bo", "bo@team.dev", &d(base + 200));

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(v["config"]["directors"][0]["emails"][0], "me@example.com");
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let read = || -> serde_json::Value { serde_json::from_str(&fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap()).unwrap() };
    let c = read();
    let total = c["totals"]["linesAdded"].as_u64().unwrap();
    assert_eq!(total, 16);
    let lines_of = |c: &serde_json::Value, id: &str| c["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).map(|b| b["linesAdded"].as_u64().unwrap()).unwrap_or(0);
    let sum: u64 = c["breakdown"].as_array().unwrap().iter().map(|b| b["linesAdded"].as_u64().unwrap()).sum();
    assert_eq!(sum, total, "{}", c["breakdown"]);
    let ana = c["contributors"].as_array().unwrap().iter().find(|x| x["kind"] == "human" && x["name"] == "Ana Lima").cloned().unwrap_or_else(|| panic!("{}", c["contributors"]));
    assert_eq!(lines_of(&c, ana["id"].as_str().unwrap()), 6);
    assert_eq!(lines_of(&c, "a:claude-code/claude-opus-5-5"), 4, "{}", c["breakdown"]);
    // 我自己没有证据的提交是未确认，不算「我手写」
    assert_eq!(lines_of(&c, "unattributed"), 6, "{}", c["breakdown"]);
    // authors：我、Ana、Bo，合计全部新增行；Bo 那一份是 Claude Code
    let au = c["authors"].as_array().unwrap();
    assert_eq!(au.iter().map(|a| a["linesAdded"].as_u64().unwrap()).sum::<u64>(), total, "{au:?}");
    assert!(au.iter().any(|a| a["me"] == true && a["linesAdded"] == 6), "{au:?}");
    let bo = au.iter().find(|a| a["name"] == "Bo").unwrap_or_else(|| panic!("{au:?}"));
    assert_eq!(bo["by"]["a:claude-code/claude-opus-5-5"], 4, "{bo}");
    // 终端那一行：名字和比例，分母是全部
    let tier = v["tier"].as_str().unwrap_or_default().to_string();
    assert!(tier.contains("Ana Lima 37.5%") && tier.contains("claude-code 25.0%"), "{tier}");

    // 关掉名字：credits.json 里一个名字都没有
    let cfg = env.repo.join(".creditia/config.json");
    let mut j: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    j["directors"][0]["id"] = serde_json::json!("h:private-full-name");
    j["directors"][0]["name"] = serde_json::json!("Private Full Name");
    j["display"] = serde_json::json!({ "hideNames": true });
    fs::write(&cfg, j.to_string()).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let s = fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap();
    assert!(!s.contains("Ana Lima") && !s.contains("\"Bo\""), "{s}");
    assert!(!s.contains("private-full-name") && !s.contains("Private Full Name"), "{s}");
    assert!(v["tier"].as_str().unwrap_or_default().contains("37.5%"), "{v}");
    // 别名也覆盖主导者的作者卡片；公开 ID 不携带原名字。
    j["display"]["hideNames"] = serde_json::json!(false);
    j["privacy"]["aliases"] = serde_json::json!({"Private Full Name": "Owner Alias", "Ana Lima": "A."});
    fs::write(&cfg, j.to_string()).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let s = fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap();
    assert!(!s.contains("Private Full Name") && !s.contains("private-full-name") && !s.contains("Ana Lima"), "{s}");
    let c = read();
    assert!(c["authors"].as_array().unwrap().iter().any(|a| a["me"] == true && a["name"] == "Owner Alias"));
}

/// 时区不写死：按天、作息按提交者当地时间；档案记下项目的时区；CREDITIA_TZ 可以指定
#[test]
fn days_follow_the_committers_timezone() {
    let mut env = setup();
    let rs = env.repo.display().to_string();
    // UTC 8 月 18 日 02:00 = 纽约（-05:00）8 月 17 日 21:00
    let ts: i64 = 1_787_018_400;
    fs::write(env.repo.join(".git/info/exclude"), ".creditia/\n").unwrap();
    fs::write(env.repo.join("a.rs"), "let late_night_value = compute_late(1);\n").unwrap();
    commit(&env.repo, "late", "me@example.com", &format!("@{ts} -0500"));
    run(&env, &["init", "--repo", &rs]);
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let c: serde_json::Value = serde_json::from_str(&fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap()).unwrap();
    assert_eq!(c["tzOffset"], -18000, "{}", c["tzOffset"]);
    assert_eq!(c["days"][0]["date"], "2026-08-17", "{}", c["days"]);
    assert_eq!(c["project"]["range"]["from"], "2026-08-17");
    assert!(c["generatedAt"].as_str().unwrap().ends_with("-05:00"), "{}", c["generatedAt"]);
    // 周一 21 点（ISO 周一 = 第 0 行）
    assert_eq!(c["clock"][0][21], 1, "{}", c["clock"]);
    // 指定 UTC：同一个提交算到 18 日
    env.vars.insert("CREDITIA_TZ".into(), "UTC".into());
    let (ok, v) = run(&env, &["analyze", "--repo", &rs, "--fresh"]);
    assert!(ok, "{v}");
    let c: serde_json::Value = serde_json::from_str(&fs::read_to_string(env.repo.join(".creditia/credits.json")).unwrap()).unwrap();
    assert_eq!(c["tzOffset"], 0);
    assert_eq!(c["days"][0]["date"], "2026-08-18", "{}", c["days"]);
}

/// 官方 SWE-agent 的提交补丁没有可靠墙钟时间：正常提交中只认完整历史里唯一的路径/行，
/// 不能把同一路径先添加、删除、再添加的相同文本任选一次归给 agent。
#[test]
fn swe_agent_submitted_patch_matches_unique_history_and_quoted_unicode_path() {
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let date = |offset: i64| format!("@{} +0800", 1_789_200_000 + offset);
    fs::write(repo.join("README.md"), "# SWE-agent history fixture\n").unwrap();
    let base = commit(&repo, "init", "me@example.com", &date(0));
    let rel = "src/中文.ts";
    fs::create_dir_all(repo.join("src")).unwrap();
    let repeated = "export const repeated_swe_a = resolve_a(42);\nexport const repeated_swe_b = resolve_b(73);\n";
    fs::write(repo.join(rel), repeated).unwrap();
    commit(&repo, "earlier matching text", "me@example.com", &date(60));
    fs::remove_file(repo.join(rel)).unwrap();
    commit(&repo, "remove earlier text", "me@example.com", &date(120));
    let unique = (0..8)
        .map(|n| format!("export const swe_unique_{n} = compute_unique_{n}({}, {});", n * 17, n + 501))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(repo.join(rel), format!("{repeated}{unique}\n")).unwrap();
    commit(&repo, "submitted SWE-agent work", "me@example.com", &date(180));

    // Let Git produce its actual C-style octal UTF-8 pathname quoting.
    let patch = git(&repo, &["-c", "core.quotepath=true", "diff", &base, "HEAD", "--", rel]);
    assert!(patch.contains(r#"+++ "b/src/\344\270\255\346\226\207.ts""#), "{patch}");
    let fixture = env.root.join("swe-agent");
    fs::create_dir_all(&fixture).unwrap();
    env.vars.insert("CREDITIA_SWE_AGENT_DIR".into(), fixture.display().to_string());
    let trajectory = serde_json::json!({
        "replay_config": { "env": { "repo": { "path": rs } }, "agent": { "model": { "name": "swe-test" } } },
        "trajectory": [],
        "history": [],
        "info": { "exit_status": "submitted", "submission": patch }
    });
    fs::write(fixture.join("submitted.traj"), trajectory.to_string()).unwrap();

    let (ok, result) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{result}");
    let (ok, result) = run(&env, &["analyze", "--repo", &rs, "--level", "strict"]);
    assert!(ok, "{result}");
    let credits: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo.join(".creditia/credits.json")).unwrap(),
    ).unwrap();
    assert_eq!(credits["level"], "strict");
    let swe = credits["breakdown"].as_array().unwrap().iter()
        .find(|b| b["contributor"] == "a:swe-agent/swe-test")
        .unwrap_or_else(|| panic!("SWE-agent patch did not reach attribution: {}", credits["breakdown"]));
    assert_eq!(swe["byEvidence"]["verified"]["linesAdded"], 8, "{swe}");
    assert_eq!(swe["linesAdded"], 8, "repeated untimed lines must remain unclaimed: {swe}");
    let verified: u64 = credits["breakdown"].as_array().unwrap().iter()
        .map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap_or(0)).sum();
    assert_eq!(verified, 8, "{}", credits["breakdown"]);
    assert_eq!(credits["totals"]["linesAdded"], 13, "both repeated additions must be present in history: {credits}");
    fs::remove_dir_all(env.root).unwrap();
}

/// Bolt / Lovable / Devin cloud native imports must reach final attribution
/// through the real CLI: cross-export duplicates, failures, retained human
/// context, unknown model/time, project boundaries and import updates.
#[test]
fn bolt_lovable_devin_cloud_native_imports_reach_final_attribution() {
    use serde_json::json;
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base: i64 = 1_789_000_000;
    let date = |t| format!("@{t} +0800");
    let lines = |tag: &str, n: usize| (0..n).map(|i| format!("export const {tag}_{i} = {i} + 7;")).collect::<Vec<_>>().join("\n");
    let bolt_added = lines("bolt", 8);
    let lov_added = lines("lovable", 8);
    let devin_added = lines("devin", 8);
    let failed_added = lines("never", 6);
    let bolt_base = "// human bolt base";
    let lov_base = "// human lovable base";
    let devin_base = "// human devin base";
    let bolt_full = format!("{bolt_base}\n{bolt_added}\n");
    let lov_full = format!("{lov_base}\n{lov_added}\n");
    let devin_full = format!("{devin_base}\n{devin_added}\n");
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/bolt.ts"), format!("{bolt_base}\n")).unwrap();
    fs::write(repo.join("src/lovable.ts"), format!("{lov_base}\n")).unwrap();
    fs::write(repo.join("src/devin.ts"), format!("{devin_base}\n")).unwrap();
    commit(&repo, "base", "me@example.com", &date(base - 86400));
    fs::write(repo.join("src/bolt.ts"), &bolt_full).unwrap();
    fs::write(repo.join("src/lovable.ts"), &lov_full).unwrap();
    fs::write(repo.join("src/devin.ts"), &devin_full).unwrap();
    fs::write(repo.join("src/failed.ts"), format!("{failed_added}\n")).unwrap();
    commit(&repo, "agent work", "me@example.com", &date(base + 120));

    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    // A real Bolt user maps the cloud workspace root through the config markers.
    let cfg_path = repo.join(".creditia/config.json");
    let mut cfg: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg_path).unwrap()).unwrap();
    cfg["markers"] = json!(["/home/project/"]);
    fs::write(&cfg_path, serde_json::to_string(&cfg).unwrap()).unwrap();

    let bolt_dir = env.root.join("bolt");
    let lov_dir = env.root.join("lovable");
    let devin_dir = env.root.join("devin");
    for d in [&bolt_dir, &lov_dir, &devin_dir] {
        fs::create_dir_all(d).unwrap();
    }
    env.vars.insert("CREDITIA_BOLT_DIR".into(), bolt_dir.display().to_string());
    env.vars.insert("CREDITIA_LOVABLE_DIR".into(), lov_dir.display().to_string());
    env.vars.insert("CREDITIA_DEVIN_DIR".into(), devin_dir.display().to_string());

    // Bolt: saved Write receipt joined with the turn-start commit and base body.
    let har_entry = |method: &str, path: String, body: serde_json::Value| json!({"request":{"method":method,"url":format!("https://bolt.new{path}")},"response":{"status":200,"content":{"mimeType":"application/json","text":body.to_string()}}});
    let mut start = har_entry("POST", "/api/chat".into(), json!({}));
    start["request"]["postData"] = json!({"mimeType":"application/json","text":json!({"projectId":"project-a","codeStorageRepo":{"sha":"base-sha"},"messages":[{"id":"u1","role":"user","content":"edit"}]}).to_string()});
    let mut apply = har_entry("POST", "/api/projects/project-a/apply-changes".into(), json!({"sha":"saved-sha","files":[{"state":"modified","path":"src/bolt.ts","content":bolt_full,"isBinary":false}]}));
    apply["request"]["postData"] = json!({"mimeType":"application/json","text":json!({"baseSha":"base-sha","sha":"saved-sha"}).to_string()});
    let chat = har_entry("GET", "/api/chats/project-a".into(), json!({"messages":[
        {"id":"u1","role":"user","content":"edit"},
        {"id":"a1","role":"assistant","annotations":[{"type":"bolt-merged","mergedSha":"saved-sha"},{"type":"metadata","userMessageId":"u1"}],"parts":[{"type":"tool-invocation","toolInvocation":{"toolCallId":"call-1","toolName":"Write","args":{"file_path":"/home/project/src/bolt.ts","content":bolt_full},"state":"result","result":"opaque"}}]}
    ]}));
    let baseline = json!({"request":{"method":"GET","url":"https://bolt.new/api/projects/project-a/files/base-sha/src/bolt~1ts?e"},"response":{"status":200,"content":{"mimeType":"text/plain","text":format!("{bolt_base}\n")}}});
    let bolt_har = json!({"log":{"version":"1.2","entries":[chat.clone(),apply.clone(),baseline.clone(),start.clone()]}}).to_string();

    // Lovable: successful trajectory events with a parent-bound model, plus a
    // rejected call whose lines must never count.
    // Official hunks carry one row per line; multi-line contents are invalid.
    let inline = |old: &str, new: &str| {
        use base64::Engine;
        let rows = old
            .lines()
            .map(|l| json!({"type":"remove","content":l}))
            .chain(new.lines().map(|l| json!({"type":"add","content":l})))
            .collect::<Vec<_>>();
        let hunks = json!([{"oldStart":1,"oldCount":old.lines().count(),"newStart":1,"newCount":new.lines().count(),"lines":rows}]);
        base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&hunks).unwrap())
    };
    let ts = (base + 30).to_string();
    let lev = |id: &str, parent: &str, payload: serde_json::Value| json!({"id":{"value":id},"parent":{"value":parent},"timestamp":ts,"payload":payload});
    let lov_path = format!("{rs}/src/lovable.ts");
    let fail_path = format!("{rs}/src/failed.ts");
    let lov_events = vec![
        lev("iteration", "", json!({"iterationStart":{}})),
        lev("inference", "iteration", json!({"agentOutputStart":{"model":{"modelName":"mutable-selection"}}})),
        lev("call", "inference", json!({"agentOutputToolCall":{"toolId":"lov-write","toolUseId":"provider-1","params":format!("{{\"file_path\":\"{lov_path}\"}}")}})),
        lev("model", "call", json!({"agentOutputEnd":{"model":{"modelName":"lov-model"}}})),
        lev("validated", "model", json!({"toolParametersValidated":{"toolCall":{"value":"call"},"params":format!("{{\"file_path\":\"{lov_path}\"}}")}})),
        lev("start", "validated", json!({"toolExecutionStart":{"toolCall":{"value":"call"}}})),
        lev("end", "start", json!({"toolExecutionEnd":{"toolCall":{"value":"call"},"output":{"changes":[{"edit":{"path":lov_path}}],"diffs":[{"filePath":lov_path,"content":{"inlineData":inline(lov_base, &lov_added)}}]}}})),
        lev("rej-call", "iteration", json!({"agentOutputToolCall":{"toolId":"lov-write","toolUseId":"provider-2","params":format!("{{\"file_path\":\"{fail_path}\"}}")}})),
        lev("rej-validated", "rej-call", json!({"toolParametersValidated":{"toolCall":{"value":"rej-call"},"params":format!("{{\"file_path\":\"{fail_path}\"}}")}})),
        lev("rejected", "rej-validated", json!({"toolRejection":{"toolCall":{"value":"rej-call"}}})),
    ];
    fs::write(lov_dir.join("trajectory.json"), json!({"events":lov_events}).to_string()).unwrap();

    // Devin cloud: native ACP loadPage page with a bound model, an own
    // timestamp, and a failed edit that vetoes its lines.
    let devin_page = json!({
        "updates": [
            {"sessionId":"devin-s","update":{"sessionUpdate":"user_message_chunk","content":[{"type":"text","text":"go"}],"messageId":"m1"}},
            {"sessionId":"devin-s","update":{"sessionUpdate":"agent_message_chunk","content":[{"type":"text","text":"working"}],"_meta":{"cognition.ai/model":"devin-model"}}},
            {"sessionId":"devin-s","update":{"sessionUpdate":"tool_call","toolCallId":"t1","title":"Edit","kind":"edit","status":"completed","content":[{"type":"diff","path":format!("{rs}/src/devin.ts"),"oldText":format!("{devin_base}\n"),"newText":devin_full}],"_meta":{"cognition.ai/timestamp":(base+40).to_string()}}},
            {"sessionId":"devin-s","update":{"sessionUpdate":"tool_call","toolCallId":"t2","title":"Edit","kind":"edit","status":"failed","content":[{"type":"diff","path":format!("{rs}/src/failed.ts"),"oldText":"","newText":format!("{failed_added}\n")}],"_meta":{"cognition.ai/timestamp":(base+50).to_string()}}}
        ],
        "coveredRange":{"start":null,"end":{"cursor":"c1","sortKey":"1"}},"hasMore":false,"replayFilterVersion":"v1"
    });
    fs::write(devin_dir.join("page.json"), devin_page.to_string()).unwrap();

    // Step 0 setup: duplicate Bolt exports across two files must stay one set.
    fs::write(bolt_dir.join("first.har"), &bolt_har).unwrap();
    fs::write(bolt_dir.join("copy.har"), &bolt_har).unwrap();
    let (_, doctor) = run(&env, &["doctor"]);
    let row = |name: &str| doctor["sources"].as_array().unwrap().iter().find(|s| s["source"] == name).cloned().unwrap_or_else(|| panic!("no {name}: {doctor}"));
    assert_eq!(row("bolt")["scope"], "edits", "{}", row("bolt"));
    assert_eq!(row("bolt")["readable"], true);
    assert_eq!(row("lovable")["scope"], "edits");
    assert_eq!(row("lovable")["readable"], true);
    assert_eq!(row("devin-acp")["scope"], "edits");
    assert_eq!(row("devin-acp")["readable"], true);
    assert_eq!(row("devin-cloud")["scope"], "sessions");

    let read_counts = || {
        let c = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let ev = |src: &str| c.query_row("select count(*) from evidence where src=?", [src], |r| r.get::<_, i64>(0)).unwrap();
        let sessions = |src: &str| c.query_row("select count(*) from sessions where src=?", [src], |r| r.get::<_, i64>(0)).unwrap();
        (ev("bolt"), ev("lovable"), ev("devin"), sessions("bolt"), sessions("lovable"), sessions("devin"))
    };
    let credits = || -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(repo.join(".creditia/credits.json")).unwrap()).unwrap()
    };
    let verified = |credits: &serde_json::Value, id: &str| {
        credits["breakdown"].as_array().unwrap().iter().find(|b| b["contributor"] == id).and_then(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64()).unwrap_or(0)
    };
    let failed_evidence = || {
        let c = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        c.query_row("select count(*) from evidence where rel='src/failed.ts'", [], |r| r.get::<_, i64>(0)).unwrap()
    };

    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let c = credits();
    assert_eq!(verified(&c, "a:bolt"), 8, "{:?} {}", read_counts(), c["breakdown"]);
    assert_eq!(verified(&c, "a:lovable/lov-model"), 8, "{}", c["breakdown"]);
    assert_eq!(verified(&c, "a:devin/devin-model"), 8, "{}", c["breakdown"]);
    assert_eq!(failed_evidence(), 0, "rejected/failed lines must not be evidence");
    assert_eq!(read_counts(), (8, 8, 8, 1, 1, 1));

    // Step 1: a second analyze must not duplicate sessions or evidence.
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(read_counts(), (8, 8, 8, 1, 1, 1), "incremental analyze must not duplicate");

    // Step 2: an updated export adds a second turn, while a conflicting stale
    // copy (its apply response body disagrees) suppresses the whole native set:
    // Both old and new lines stay unattributed while their receipts conflict.
    // An earlier successful analysis must not resurrect the rejected evidence.
    let bolt_next = lines("bolt_next", 6);
    let bolt_full2 = format!("{bolt_full}{bolt_next}\n");
    let mut chat2 = chat.clone();
    chat2["response"]["content"]["text"] = json!(json!({"messages":[
        {"id":"u1","role":"user","content":"edit"},
        {"id":"a1","role":"assistant","annotations":[{"type":"bolt-merged","mergedSha":"saved-sha"},{"type":"metadata","userMessageId":"u1"}],"parts":[{"type":"tool-invocation","toolInvocation":{"toolCallId":"call-1","toolName":"Write","args":{"file_path":"/home/project/src/bolt.ts","content":bolt_full},"state":"result","result":"opaque"}}]},
        {"id":"u2","role":"user","content":"more"},
        {"id":"a2","role":"assistant","annotations":[{"type":"bolt-merged","mergedSha":"saved-sha-2"},{"type":"metadata","userMessageId":"u2"}],"parts":[{"type":"tool-invocation","toolInvocation":{"toolCallId":"call-2","toolName":"Write","args":{"file_path":"/home/project/src/bolt.ts","content":bolt_full2},"state":"result","result":"opaque"}}]}
    ]}).to_string());
    let mut apply2 = apply.clone();
    apply2["request"]["url"] = json!("https://bolt.new/api/projects/project-a/apply-changes");
    apply2["request"]["postData"]["text"] = json!(json!({"baseSha":"saved-sha","sha":"saved-sha-2"}).to_string());
    apply2["response"]["content"]["text"] = json!(json!({"sha":"saved-sha-2","files":[{"state":"modified","path":"src/bolt.ts","content":bolt_full2,"isBinary":false}]}).to_string());
    let mut start2 = start.clone();
    start2["request"]["postData"]["text"] = json!(json!({"projectId":"project-a","codeStorageRepo":{"sha":"saved-sha"},"messages":[{"id":"u2","role":"user","content":"more"}]}).to_string());
    let two_turns = json!({"log":{"version":"1.2","entries":[chat2,apply.clone(),apply2,baseline.clone(),start.clone(),start2]}}).to_string();
    fs::write(bolt_dir.join("first.har"), &two_turns).unwrap();
    let mut conflicting: serde_json::Value = serde_json::from_str(&bolt_har).unwrap();
    let mut body: serde_json::Value = serde_json::from_str(conflicting["log"]["entries"][1]["response"]["content"]["text"].as_str().unwrap()).unwrap();
    body["files"][0]["content"] = json!("tampered committed body\n");
    conflicting["log"]["entries"][1]["response"]["content"]["text"] = json!(body.to_string());
    fs::write(bolt_dir.join("copy.har"), conflicting.to_string()).unwrap();
    fs::write(repo.join("src/bolt.ts"), &bolt_full2).unwrap();
    commit(&repo, "more agent work", "me@example.com", &date(base + 240));
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    let c = credits();
    assert_eq!(verified(&c, "a:bolt"), 0, "conflicting copies must retract prior attribution: {:?}", read_counts());
    assert_eq!(read_counts(), (0, 8, 8, 0, 1, 1));
    let (ok, v) = run(&env, &["analyze", "--fresh", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(&credits(), "a:bolt"), 0, "a fresh analysis must not restore rejected evidence");

    // Step 3: removing the conflicting copy rebuilds the native set; the second
    // turn's new lines are now attributed as well.
    fs::remove_file(bolt_dir.join("copy.har")).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(&credits(), "a:bolt"), 14, "{:?}", read_counts());
    assert_eq!(read_counts(), (14, 8, 8, 1, 1, 1));

    // Step 4: a failed copy of a previously accepted native call must retract
    // its stored rows, including when the CLI is asked for a fresh analysis.
    let mut failed_copy = devin_page.clone();
    failed_copy["updates"][2]["update"]["status"] = json!("failed");
    fs::write(devin_dir.join("failed-copy.json"), failed_copy.to_string()).unwrap();
    for args in [vec!["analyze", "--repo", &rs], vec!["analyze", "--fresh", "--repo", &rs]] {
        let (ok, v) = run(&env, &args);
        assert!(ok, "{v}");
        assert_eq!(verified(&credits(), "a:devin/devin-model"), 0);
        assert_eq!(read_counts(), (14, 8, 0, 1, 1, 0), "failed calls cannot return from the evidence library");
    }
    fs::remove_file(devin_dir.join("failed-copy.json")).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(read_counts(), (14, 8, 8, 1, 1, 1));

    // Correcting conflicting model observations must also remove the old
    // model's stored rows, rather than saving a second copy of the same write.
    let mut model_copy = devin_page.clone();
    model_copy["updates"][1]["update"]["_meta"]["cognition.ai/model"] = json!("other-model");
    fs::write(devin_dir.join("model-copy.json"), model_copy.to_string()).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(&credits(), "a:devin"), 8, "conflicting models remain unknown");
    assert_eq!(verified(&credits(), "a:devin/devin-model"), 0);
    assert_eq!(read_counts(), (14, 8, 8, 1, 1, 1));
    fs::remove_file(devin_dir.join("model-copy.json")).unwrap();

    // Lovable's explicit revert invalidates the currently captured branch.
    // It must revoke prior library evidence as well as the reader's cache.
    let revert = lev("later-revert", "end", json!({"revert":{"oldAgentHead":{"value":"end"}}}));
    fs::write(lov_dir.join("revert.json"), json!({"events":[revert]}).to_string()).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(read_counts(), (14, 0, 8, 1, 0, 1));
    assert_eq!(verified(&credits(), "a:lovable/lov-model"), 0);
    fs::remove_file(lov_dir.join("revert.json")).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(read_counts(), (14, 8, 8, 1, 1, 1));

    // Losing an import is not an explicit failure or revert. Historical
    // evidence remains usable when the native log itself is no longer present.
    fs::remove_file(lov_dir.join("trajectory.json")).unwrap();
    let (ok, v) = run(&env, &["analyze", "--fresh", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(&credits(), "a:lovable/lov-model"), 8);
    assert_eq!(read_counts(), (14, 8, 8, 1, 1, 1));
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn trae_native_encrypted_history_reaches_attribution_and_retracts_failed_results() {
    use serde_json::json;
    const SUCCESS: &[u8] = include_bytes!("fixtures/trae-native/success.db");
    const FAILED: &[u8] = include_bytes!("fixtures/trae-native/failed.db");
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    fs::write(repo.join("a.rs"), "unchanged line\nremoved line\n").unwrap();
    commit(&repo, "human base", "me@example.com", "@1720000000 +0000");
    fs::write(repo.join("a.rs"), "unchanged line\nactual native fixture line\n").unwrap();
    commit(&repo, "native edit", "me@example.com", "@1720000006 +0000");
    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let cfg_path = repo.join(".creditia/config.json");
    let mut cfg: serde_json::Value = serde_json::from_slice(&fs::read(&cfg_path).unwrap()).unwrap();
    cfg["markers"] = json!(["/private/tmp/creditia-trae-native-project/"]);
    fs::write(&cfg_path, serde_json::to_vec(&cfg).unwrap()).unwrap();
    let source = env.root.join("trae/ModularData/ai-agent/database.db");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, SUCCESS).unwrap();
    assert!(!SUCCESS.starts_with(b"SQLite format 3"), "the reference fixture must be encrypted");
    env.vars.insert("CREDITIA_TRAE_IDE_DIR".into(), env.root.join("trae").display().to_string());
    let (_, doctor) = run(&env, &["doctor"]);
    let native = doctor["sources"].as_array().unwrap().iter().find(|r| r["source"] == "trae-ide").unwrap();
    assert_eq!(native["scope"], "edits", "{native}");
    assert_eq!(native["readable"], true, "{native}");
    let verified = || {
        let credits: serde_json::Value = serde_json::from_slice(&fs::read(repo.join(".creditia/credits.json")).unwrap()).unwrap();
        credits["breakdown"].as_array().unwrap().iter()
            .filter(|b| b["contributor"] == "a:trae-ide")
            .filter_map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64()).sum::<u64>()
    };
    let rows = || {
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let mut q = db.prepare("select rel,key,model,session,ts from evidence where src='trae-ide'").unwrap();
        q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?)))
            .unwrap().collect::<Result<Vec<_>, _>>().unwrap()
    };
    for _ in 0..2 {
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(verified(), 1, "{v}");
        let saved = rows();
        assert_eq!(saved.len(), 1, "retained context and tool parameters are not evidence");
        assert_eq!(saved[0].0, "a.rs");
        assert_eq!(saved[0].1, line_key("a.rs", &normalize("actual native fixture line", &ext_of("a.rs"))) as i64);
        assert_eq!(saved[0].2, "", "historical configuration is not a proved execution model");
        assert_eq!(saved[0].3, "native-s");
        assert_eq!(saved[0].4, 1720000005, "use the tool's completion time");
        assert_eq!(fs::read(&source).unwrap(), SUCCESS);
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!PathBuf::from(format!("{}{suffix}", source.display())).exists(), "reading must not create a sidecar");
    }
    fs::write(&source, FAILED).unwrap();
    let (ok, v) = run(&env, &["analyze", "--fresh", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(), 0, "a failed native result revokes prior verified attribution");
    assert!(rows().is_empty());
    assert_eq!(fs::read(&source).unwrap(), FAILED);
    fs::write(&source, SUCCESS).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(), 1);

    // The default Trae and Trae CN roots can contain copies of the same history.
    // They must reconcile together, including a failed copy of an earlier call.
    env.vars.remove("CREDITIA_TRAE_IDE_DIR");
    env.vars.insert("APPDATA".into(), env.root.join("AppData").display().to_string());
    env.vars.insert("XDG_CONFIG_HOME".into(), env.root.join("config").display().to_string());
    #[cfg(target_os = "macos")]
    let app_base = env.root.join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let app_base = env.root.join("AppData");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let app_base = env.root.join("config");
    let mut default_paths = Vec::new();
    for name in ["Trae", "Trae CN"] {
        let path = app_base.join(name).join("ModularData/ai-agent/database.db");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, SUCCESS).unwrap();
        default_paths.push(path);
    }
    let (ok, v) = run(&env, &["analyze", "--fresh", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(), 1);
    assert_eq!(rows().len(), 1, "copies across default roots are one native call");
    let (_, doctor) = run(&env, &["doctor"]);
    let native = doctor["sources"].as_array().unwrap().iter().find(|r| r["source"] == "trae-ide").unwrap();
    assert_eq!(native["records"], 1, "session metadata also merges across roots");
    fs::write(&default_paths[1], FAILED).unwrap();
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(), 0, "a conflicting copy in the other root vetoes the call");
    assert!(rows().is_empty());
    assert_eq!(fs::read(&default_paths[0]).unwrap(), SUCCESS);
    assert_eq!(fs::read(&default_paths[1]).unwrap(), FAILED);
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn devin_partial_native_capture_preserves_uncovered_cli_edits_and_native_metadata() {
    use serde_json::json;
    let mut env = setup();
    let repo = env.repo.canonicalize().unwrap();
    let rs = repo.display().to_string();
    let base = 1720000000_i64;
    fs::write(repo.join("README.md"), "a neutral test repository\n").unwrap();
    commit(&repo, "base", "me@example.com", "@1720000000 +0000");
    let lines: Vec<_> = (0..8).map(|i| format!("export const cli_agent_{i} = {i} + 17;")).collect();
    fs::write(repo.join("a.ts"), format!("{}\n", lines.join("\n"))).unwrap();
    commit(&repo, "agent work", "me@example.com", "@1720000020 +0000");
    let (ok, v) = run(&env, &["init", "--repo", &rs]);
    assert!(ok, "{v}");
    let imports = env.root.join("devin");
    fs::create_dir_all(&imports).unwrap();
    env.vars.insert("CREDITIA_DEVIN_DIR".into(), imports.display().to_string());
    let patch = format!("apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: {rs}/a.ts\n{}\n*** End Patch\nPATCH", lines.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
    let hook = json!({"hook_event_name":"PostToolUse","tool_name":"exec","session_id":"partial-s","tool_input":{"command":patch},"tool_response":{"success":true}});
    fs::write(imports.join("cli.jsonl"), hook.to_string()).unwrap();
    let page = |user: &str, id: &str, path: &str, text: &str| json!({
        "updates":[
            {"sessionId":"partial-s","update":{"sessionUpdate":"user_message_chunk","messageId":user}},
            {"sessionId":"partial-s","update":{"sessionUpdate":"agent_message_chunk","_meta":{"cognition.ai/model":"native-model"}}},
            {"sessionId":"partial-s","update":{"sessionUpdate":"tool_call","toolCallId":id,"kind":"edit","status":"completed","content":[{"type":"diff","path":path,"oldText":"","newText":text}],"_meta":{"cognition.ai/timestamp":(base+10).to_string()}}}
        ],"coveredRange":{"start":null,"end":{"cursor":"end","sortKey":"1"}},"hasMore":false,"replayFilterVersion":"v1"
    });
    fs::write(imports.join("foreign.json"), page("u-foreign", "foreign-call", "/other-project/a.ts", "unrelated original native line\n").to_string()).unwrap();
    let verified = || {
        let c: serde_json::Value = serde_json::from_slice(&fs::read(repo.join(".creditia/credits.json")).unwrap()).unwrap();
        c["breakdown"].as_array().unwrap().iter().filter(|b| b["contributor"].as_str().unwrap_or("").starts_with("a:devin"))
            .filter_map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64()).sum::<u64>()
    };
    let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
    assert!(ok, "{v}");
    assert_eq!(verified(), 8, "unmapped native history must not erase valid CLI edits: {v}");
    fs::write(imports.join("native.json"), page("u-native", "mapped-call", &format!("{rs}/a.ts"), &format!("{}\n", lines[..4].join("\n"))).to_string()).unwrap();
    for _ in 0..2 {
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(verified(), 8, "native overlap and CLI-only rows must total eight: {v}");
        let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
        let count = |model: &str| db.query_row("select count(*) from evidence where src='devin' and model=?1", [model], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("native-model"), 4);
        assert_eq!(count(""), 4, "uncovered CLI edits remain available");
        let session: (i64, i64, i64) = db.query_row("select count(*),min(start),min(prompts) from sessions where src='devin'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(session, (1, base+10, 2), "native metadata must not be overwritten by the CLI's zero values");
    }
    fs::remove_dir_all(env.root).unwrap();
}

#[test]
fn relative_cloud_repository_conflicts_retract_persisted_attribution() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde_json::{json, Value};
    for source in ["lovable", "devin"] {
        let mut env = setup();
        let repo = env.repo.canonicalize().unwrap();
        let rs = repo.display().to_string();
        git(&repo, &["remote", "add", "origin", "https://github.com/acme/demo.git"]);
        fs::write(repo.join("a.rs"), "human base line\n").unwrap();
        commit(&repo, "base", "me@example.com", "@1720000000 +0000");
        let lines: Vec<_> = (0..8).map(|i| format!("pub const ADDED_{i}: i32 = {i} + 17;")).collect();
        let after = format!("human base line\n{}\n", lines.join("\n"));
        fs::write(repo.join("a.rs"), &after).unwrap();
        commit(&repo, "native edits", "me@example.com", "@1720000010 +0000");
        let (ok, v) = run(&env, &["init", "--repo", &rs]);
        assert!(ok, "{v}");
        let imports = env.root.join(source);
        fs::create_dir_all(&imports).unwrap();
        env.vars.insert(format!("CREDITIA_{}_DIR", source.to_uppercase()), imports.display().to_string());
        let har_entry = |path: &str, body: Value| json!({"request":{"method":"GET","url":format!("https://api.lovable.dev/{path}")},"response":{"status":200,"content":{"text":body.to_string()}}});
        let good = if source == "lovable" {
            let mut added = vec![json!({"type":"context","content":"human base line"})];
            added.extend(lines.iter().map(|line| json!({"type":"add","content":line})));
            let hunks = json!([{"oldStart":1,"oldCount":1,"newStart":1,"newCount":9,"lines":added}]);
            let event = |id: &str, parent: &str, payload: Value| json!({"id":{"value":id},"parent":{"value":parent},"timestamp":"2024-07-03T09:46:45Z","payload":payload});
            let params = json!({"file_path":"a.rs"}).to_string();
            let trace = vec![
                event("iteration", "", json!({"iterationStart":{}})),
                event("inference", "iteration", json!({"agentOutputStart":{}})),
                event("call", "inference", json!({"agentOutputToolCall":{"toolId":"lov-write","params":params}})),
                event("model", "call", json!({"agentOutputEnd":{"model":{"modelName":"actual-model"}}})),
                event("validated", "model", json!({"toolParametersValidated":{"toolCall":{"value":"call"},"params":params}})),
                event("start", "validated", json!({"toolExecutionStart":{"toolCall":{"value":"call"}}})),
                event("end", "start", json!({"toolExecutionEnd":{"toolCall":{"value":"call"},"output":{"changes":[{"edit":{"path":"a.rs"}}],"diffs":[{"filePath":"a.rs","content":{"inlineData":STANDARD.encode(hunks.to_string())}}]}}})),
            ];
            json!({"log":{"version":"1.2","entries":[
                har_entry("projects/p/trajectory?chain=view&filter=frontend&inclusive=true", json!({"events":trace})),
                har_entry("workspaces/w/projects/p/gitsync", json!({"synced":true,"config":{"repo_url":"https://github.com/acme/demo"}})),
            ]}})
        } else {
            json!({"updates":[
                {"sessionId":"relative-s","update":{"sessionUpdate":"user_message_chunk","messageId":"u"}},
                {"sessionId":"relative-s","update":{"sessionUpdate":"agent_message_chunk","_meta":{"cognition.ai/model":"actual-model","cognition.ai/owner":"acme","cognition.ai/repo":"demo"}}},
                {"sessionId":"relative-s","update":{"sessionUpdate":"tool_call","toolCallId":"c","kind":"edit","status":"completed","content":[{"type":"diff","path":"a.rs","oldText":"human base line\n","newText":after}],"_meta":{"cognition.ai/timestamp":"1720000005"}}},
            ],"coveredRange":{},"hasMore":false,"replayFilterVersion":"v1"})
        };
        let mut conflict = good.clone();
        let mut conflicts = Vec::new();
        if source == "lovable" {
            conflict["log"]["entries"].as_array_mut().unwrap().push(har_entry("workspaces/w/projects/p/gitsync", json!({"synced":true,"config":{"repo_url":"https://github.com/acme/other"}})));
            conflicts.push(conflict);
            let mut conflict = good.clone();
            let mut other_owner = good["log"]["entries"][0].clone();
            other_owner["request"]["url"] = json!("https://api.lovable.dev/projects/other/trajectory?chain=view&filter=frontend&inclusive=true");
            conflict["log"]["entries"].as_array_mut().unwrap().push(other_owner);
            conflicts.push(conflict);
        } else {
            conflict["updates"].as_array_mut().unwrap().push(json!({"sessionId":"relative-s","update":{"sessionUpdate":"agent_message_chunk","_meta":{"cognition.ai/owner":"acme","cognition.ai/repo":"other"}}}));
            conflicts.push(conflict);
        }
        let path = imports.join(if source == "lovable" { "network.har" } else { "page.json" });
        let verified = || {
            let c: Value = serde_json::from_slice(&fs::read(repo.join(".creditia/credits.json")).unwrap()).unwrap();
            c["breakdown"].as_array().unwrap().iter().filter(|b| b["contributor"].as_str().unwrap_or("").starts_with(&format!("a:{source}")))
                .filter_map(|b| b["byEvidence"]["verified"]["linesAdded"].as_u64()).sum::<u64>()
        };
        for conflict in conflicts {
            fs::write(&path, good.to_string()).unwrap();
            let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
            assert!(ok, "{v}");
            assert_eq!(verified(), 8, "{source}: the original repository binding is valid");
            fs::write(&path, conflict.to_string()).unwrap();
            let (ok, v) = run(&env, &["analyze", "--fresh", "--repo", &rs]);
            assert!(ok, "{v}");
            assert_eq!(verified(), 0, "{source}: a known contradictory binding must revoke attribution");
            let db = rusqlite::Connection::open(env.root.join("lib/library.sqlite")).unwrap();
            let count: i64 = db.query_row("select count(*) from evidence where src=?1", [source], |r| r.get(0)).unwrap();
            assert_eq!(count, 0, "{source}: rejected rows must not be restored from the library");
        }
        fs::write(&path, good.to_string()).unwrap();
        let (ok, v) = run(&env, &["analyze", "--repo", &rs]);
        assert!(ok, "{v}");
        assert_eq!(verified(), 8, "{source}: resolving the conflict restores the original evidence");
        fs::remove_dir_all(env.root).unwrap();
    }
}
