//! 端到端：临时 HOME 里伪造 Codex / Claude Code / Cursor 日志，构造已知归因的测试仓库，跑真实二进制。
//! 所有数据源路径都通过环境变量重定向，不碰真实数据。

use creditia_core::hash::{ext_of, line_key, normalize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Env {
    root: PathBuf,
    repo: PathBuf,
    vars: HashMap<String, String>,
}

fn tmp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("creditia-e2e-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
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
    git(repo, &["add", "-A"]);
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=X", "-c", &format!("user.email={email}"), "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg, "--date", date])
        .env("GIT_COMMITTER_DATE", date)
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
    vars.insert("CREDITIA_HOME".into(), root.join("lib").display().to_string());
    vars.insert("CODEX_HOME".into(), root.join("codex").display().to_string());
    vars.insert("CLAUDE_CONFIG_DIR".into(), root.join("claude").display().to_string());
    vars.insert("CREDITIA_CURSOR_DIR".into(), root.join("cursor").display().to_string());
    vars.insert("CREDITIA_CURSOR_STORAGE".into(), root.join("cursor-storage").display().to_string());
    vars.insert("CREDITIA_GROK_DIR".into(), root.join("grok").display().to_string());
    vars.insert("CREDITIA_KIMI_DIR".into(), root.join("kimi").display().to_string());
    Env { root, repo, vars }
}

fn run(env: &Env, args: &[&str]) -> (bool, serde_json::Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(args).arg("--json").env_clear().envs(&env.vars).env("PATH", std::env::var("PATH").unwrap()).output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_else(|_| panic!("非 JSON 输出：{s}\n{}", String::from_utf8_lossy(&o.stderr)));
    (o.status.code() != Some(1), v)
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
        serde_json::json!({ "timestamp": iso(ts - 300), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "status":"completed", "changes": { abs.clone(): { "type": "add", "content": content } } } } }),
        serde_json::json!({ "timestamp": iso(ts - 200), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "status":"completed", "changes": { abs: { "type": "update", "unified_diff": format!("@@ -20,0 +21,10 @@\n{diff_added}"), "move_path": null } } } } }),
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
        serde_json::json!({ "type": "assistant", "cwd": env.repo.display().to_string(), "sessionId": "s1", "timestamp": iso(ts - 50), "message": { "id": "m1", "model": "claude-test", "usage": { "input_tokens": 100, "output_tokens": 50 }, "content": [{ "type": "tool_use", "id":"tool1", "name": "Write", "input": { "file_path": env.repo.join("docs/plan.md").display().to_string(), "content": doc } }] } }),
    ];
    let mut lines=lines.to_vec();
    lines.push(serde_json::json!({"type":"user","cwd":env.repo.display().to_string(),"sessionId":"s1","timestamp":iso(ts-40),"message":{"content":[{"type":"tool_result","tool_use_id":"tool1","content":"ok","is_error":false}]}}));
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
    // 消息库：requestId 精确对应模型；对话级模型不参与推断
    let sdir = env.root.join("cursor-storage");
    fs::create_dir_all(&sdir).unwrap();
    let c2 = rusqlite::Connection::open(sdir.join("state.vscdb")).unwrap();
    c2.execute_batch("create table cursorDiskKV(key text primary key, value text);").unwrap();
    c2.execute("insert into cursorDiskKV values('bubbleId:conv1:b1', ?1)", [r#"{"requestId":"r","modelInfo":{"modelName":"grok-test"}}"#]).unwrap();
}

#[test]
fn end_to_end() {
    let env = setup();
    let repo = &env.repo;
    let base_ts: i64 = 1_787_000_000; // 2026-08
    let d = |t: i64| format!("@{t} +0800");

    // 1. 人写的第一个提交（没有逐行证据 → 未确认）
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
    assert_eq!(v["config"]["people"], serde_json::json!([]));
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
    // Cursor：17 行 composer（模型 default 由同请求记录补齐）+ 3 行 Tab 保留未确认
    assert_eq!(lines("a:cursor/grok-test"), 17, "{}", credits["breakdown"]);
    assert_eq!(credits["human"]["tabAssistedLines"], 3);
    // Claude：12 行文档
    assert_eq!(lines("a:claude-code/claude-test"), 12, "{}", credits["breakdown"]);
    // 云端 agent 邮箱 → declared cursor（无模型）
    assert_eq!(lines("a:cursor"), 8);
    // 盖章 → kimi/k3
    assert_eq!(lines("a:kimi/k3"), 6);
    // 人写的 10 行 → 未确认；lockfile 被排除；3 行 Tab 补全不猜测键盘使用者
    assert_eq!(lines("unattributed"), 13, "{}", credits["breakdown"]);
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

    // doctor 能跑
    let (_, v) = run(&env, &["doctor"]);
    for source in ["codex","claude-code","cursor","grok","kimi","opencode","gemini"] { assert!(v["sources"].as_array().unwrap().iter().any(|s|s["source"]==source)); }

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

    // discover 能找到这个项目
    let (ok, v) = run(&env, &["discover"]);
    assert!(ok, "{v}");
    assert!(v["projects"].as_array().unwrap().iter().any(|p| p["name"] == "示例项目"), "{v}");

    let _ = fs::remove_dir_all(&env.root);
}
