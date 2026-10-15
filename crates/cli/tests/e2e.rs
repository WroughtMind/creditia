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
    vars.insert("CREDITIA_GEMINI_DIR".into(), root.join("gemini").display().to_string());
    vars.insert("CREDITIA_QWEN_DIR".into(), root.join("qwen").display().to_string());
    vars.insert("CREDITIA_OPENCODE_DIR".into(), root.join("opencode").display().to_string());
    vars.insert("CREDITIA_VSCODE_STORAGE".into(), root.join("vscode").display().to_string());
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
        serde_json::json!({ "type": "assistant", "cwd": env.repo.display().to_string(), "sessionId": "s1", "timestamp": iso(ts - 50), "message": { "id": "m1", "model": "claude-test", "usage": { "input_tokens": 100, "output_tokens": 50 }, "content": [{ "type": "tool_use", "name": "Write", "input": { "file_path": env.repo.join("docs/plan.md").display().to_string(), "content": doc } }] } }),
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

    // doctor 能跑
    let (_, v) = run(&env, &["doctor"]);
    // 11 个老数据源 + Aider、Amp、TRAE CLI、Crush、Copilot（VS Code、命令行）；没装的 Windsurf、TRAE IDE 不列
    assert_eq!(v["sources"].as_array().unwrap().len(), 17);

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
    let ws = env.root.join(".config/Code/User/workspaceStorage/0123abcd");
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
