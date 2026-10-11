//! creditia-core：Rust 核心的命令行入口。所有命令非交互，支持 `--json`。
//! 退出码：0 成功，1 失败，2 部分成功（有警告）。

use creditia_cli::{analyze, project::detect_config};

use creditia_adapters as ad;
use creditia_core::model::Src;
use creditia_store::Library;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

type R<T> = Result<T, String>;

struct Args {
    cmd: String,
    pos: Vec<String>,
    flags: HashMap<String, String>,
    switches: Vec<String>,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut a = Args { cmd: raw.first().cloned().unwrap_or_default(), pos: vec![], flags: HashMap::new(), switches: vec![] };
    let mut i = 1;
    while i < raw.len() {
        let x = &raw[i];
        if let Some(k) = x.strip_prefix("--") {
            if let Some((k, v)) = k.split_once('=') {
                a.flags.insert(k.into(), v.into());
            } else if i + 1 < raw.len() && !raw[i + 1].starts_with("--") && !matches!(k, "json" | "fresh" | "force" | "github" | "all" | "yes" | "show") {
                a.flags.insert(k.into(), raw[i + 1].clone());
                i += 1;
            } else {
                a.switches.push(k.into());
            }
        } else {
            a.pos.push(x.clone());
        }
        i += 1;
    }
    a
}

impl Args {
    fn on(&self, k: &str) -> bool {
        self.switches.iter().any(|s| s == k)
    }
    fn repo(&self) -> R<PathBuf> {
        let p = self.flags.get("repo").map(PathBuf::from).unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
        let p = p.canonicalize().map_err(|_| format!("目录不存在：{}", p.display()))?;
        if !ad::git::git_ok(&p, &["rev-parse", "--git-dir"]) {
            return Err(format!("不是 git 仓库：{}", p.display()));
        }
        let top = ad::git::git(&p, &["rev-parse", "--show-toplevel"]);
        Ok(PathBuf::from(top.trim()))
    }
}

const USAGE: &str = "用法：creditia-core <命令> [选项]

  discover                         列出本机检测到 agent 活动的项目
  init      [--repo P] [--force]   在仓库写 .creditia/config.json
  analyze   [--repo P] [--fresh] [--github] [--out F] [--config F]   分析并写 .creditia/credits.json
  update    [--repo P]             增量更新（同 analyze）
  collect   [--repo P | --all]     只采集证据进证据库
  doctor                           检查各数据源
  blame     <路径>:<行号> [--repo P]  这一行是谁写的
  stamp     --harness H [--model M] [--share S]   给下一次提交盖章
  hook      install|uninstall [--repo P]           安装提交盖章 hook
  hook-trailers <msgfile>          （hook 调用）追加 Creditia-* trailer
  evidence  export [--out F] | import <文件>   多台电脑之间合并证据库
  link --path P [--repo P]         关联另一个工作副本或旧路径
  people --file F [--show]          保存人员署名
  projects                         作品库里的项目

所有命令支持 --json。退出码：0 成功，1 失败，2 有警告。";

fn main() {
    let a = parse_args();
    let json = a.on("json");
    let r: R<(Value, Vec<String>)> = match a.cmd.as_str() {
        "desktop-api" => cmd_desktop(&a),
        "link" => cmd_link(&a),
        "people" => cmd_people(&a),
        "discover" => cmd_discover(&a),
        "init" => cmd_init(&a),
        "analyze" | "update" => cmd_analyze(&a),
        "collect" => cmd_collect(&a),
        "doctor" => cmd_doctor(&a),
        "blame" => cmd_blame(&a),
        "stamp" => cmd_stamp(&a),
        "hook" => cmd_hook(&a),
        "hook-trailers" => cmd_hook_trailers(&a),
        "evidence" => cmd_evidence(&a),
        "projects" => cmd_projects(&a),
        "" | "help" | "-h" | "--help" => {
            println!("{USAGE}");
            return;
        }
        other => Err(format!("未知命令：{other}\n\n{USAGE}")),
    };
    match r {
        Ok((v, warnings)) => {
            if json {
                let mut v = v;
                if !warnings.is_empty() {
                    v["warnings"] = json!(warnings);
                }
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else if let Some(s) = v["summary"].as_str() {
                println!("{s}");
                for w in &warnings {
                    eprintln!("警告：{w}");
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            }
            std::process::exit(if warnings.is_empty() { 0 } else { 2 });
        }
        Err(e) => {
            if json {
                println!("{}", json!({ "error": e }));
            } else {
                eprintln!("错误：{e}");
            }
            std::process::exit(1);
        }
    }
}

// ───────────────────────── discover ─────────────────────────

fn cmd_discover(_a: &Args) -> R<(Value, Vec<String>)> {
    let mut acts: Vec<ad::Activity> = vec![];
    acts.extend(ad::codex::discover());
    acts.extend(ad::claude::discover());
    acts.extend(ad::cursor::discover());
    acts.extend(ad::grok::discover());
    acts.extend(ad::kimi::discover().into_iter().chain(ad::opencode::discover()).chain(ad::gemini::discover()).collect::<Vec<_>>());
    // 项目发现与分析共用身份；不同 fork 不因原始提交相同而合并。
    #[derive(Default)]
    struct P {
        id: String,
        origin: String,
        name: String,
        path: String,
        root: String,
        remotes: Vec<String>,
        tools: BTreeMap<String, u64>,
        last: i64,
        exists: bool,
    }
    let mut by_key: HashMap<String, P> = HashMap::new();
    let lib = Library::open().ok();
    let known: std::collections::HashSet<String> = lib.as_ref().and_then(|l| l.projects().ok()).unwrap_or_default().into_iter().map(|p| p.id).collect();
    for act in acts {
        let cwd = PathBuf::from(&act.cwd);
        let (key, p) = if cwd.is_dir() && ad::git::git_ok(&cwd, &["rev-parse", "--git-dir"]) {
            let main = ad::git::main_worktree(&cwd).unwrap_or(cwd.clone());
            let (root, remotes) = ad::git::identity(&main);
            let key = analyze::project_of(&main, &detect_config(&main)).id;
            let origin = ad::git::normalize_remote(ad::git::git(&main, &["remote", "get-url", "origin"]).trim());
            (key.clone(), P { id: key, origin, name: main.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), path: main.display().to_string(), root, remotes, exists: true, ..Default::default() })
        } else {
            let remote = act.remote.as_deref().map(ad::git::normalize_remote);
            let name = remote.as_deref().and_then(|r| r.rsplit('/').next()).map(String::from).unwrap_or_else(|| cwd.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(act.cwd.clone()));
            let key = remote.clone().unwrap_or(format!("path:{}", act.cwd));
            (key.clone(), P { id: key, origin: remote.clone().unwrap_or_default(), name, path: act.cwd.clone(), remotes: remote.into_iter().collect(), ..Default::default() })
        };
        let e = by_key.entry(key).or_insert(p);
        *e.tools.entry(act.src.harness().into()).or_default() += act.sessions;
        e.last = e.last.max(act.last);
    }
    // 不存在的目录如果远程地址和某个存在的项目一致，并进去
    let mut remote_to_keys: HashMap<String, Vec<String>> = HashMap::new();
    for (key, project) in &by_key {
        if project.exists && !project.origin.is_empty() { remote_to_keys.entry(project.origin.clone()).or_default().push(key.clone()); }
    }
    let mut merged: Vec<P> = vec![];
    let mut pending: Vec<P> = vec![];
    for (_, p) in by_key.drain() {
        if p.exists {
            merged.push(p);
        } else {
            pending.push(p);
        }
    }
    for p in pending {
        if let Some(keys) = remote_to_keys.get(&p.origin).filter(|keys| keys.len() == 1) {
            if let Some(t) = merged.iter_mut().find(|x| x.id == keys[0]) {
                for (s, n) in p.tools {
                    *t.tools.entry(s).or_default() += n;
                }
                t.last = t.last.max(p.last);
                continue;
            }
        }
        merged.push(p);
    }
    merged.sort_by(|a, b| b.last.cmp(&a.last));
    let status = |p: &P| if p.exists { p.path.clone() } else if PathBuf::from(&p.path).is_dir() { format!("{}  (不是 git 仓库)", p.path) } else { format!("{}  (目录已不存在)", p.path) };
    let list: Vec<Value> = merged
        .iter()
        .map(|p| {
            json!({
                "name": p.name, "path": p.path, "rootCommit": p.root, "remotes": p.remotes, "exists": p.exists,
                "tools": p.tools, "lastActive": creditia_core::util::date_cn(p.last),
                "id": p.id, "inLibrary": known.contains(&p.id),
            })
        })
        .collect();
    let summary = merged
        .iter()
        .map(|p| format!("{:<28} {}  {}  {}", p.name, creditia_core::util::date_cn(p.last), p.tools.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(" · "), status(p)))
        .collect::<Vec<_>>()
        .join("\n");
    Ok((json!({ "projects": list, "summary": summary }), vec![]))
}

// ───────────────────────── init ─────────────────────────

fn cmd_init(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let path = repo.join(".creditia/config.json");
    if path.exists() && !a.on("force") {
        return Err(format!("{} 已存在；加 --force 覆盖", path.display()));
    }
    let c = detect_config(&repo);
    c.save(&path)?;
    let v = serde_json::to_value(&c).unwrap();
    let summary = format!(
        "已写入 {}\n项目 {} · 分支 {}\n人员署名、关联路径和隐私选项由你确认。",
        path.display(),
        c.name,
        c.branch
    );
    Ok((json!({ "path": path, "config": v, "summary": summary }), vec![]))
}

// ───────────────────────── analyze / update ─────────────────────────

fn cmd_analyze(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let opts = analyze::Opts { branch: a.flags.get("branch").cloned(), fresh: a.on("fresh"), github: a.on("github"), out: a.flags.get("out").map(PathBuf::from), blame: None, config: a.flags.get("config").map(PathBuf::from) };
    let r = analyze::run(&repo, &opts)?;
    let summary = format!(
        "{}：{} 个提交（新增 {}），{} 项，{} 行；{:.1}s\n{}\n写入 {}",
        r.name,
        r.commits,
        r.new_commits,
        r.units,
        r.lines,
        r.secs,
        r.tier,
        r.out.display()
    );
    Ok((json!({ "project": r.name, "head": r.head, "commits": r.commits, "newCommits": r.new_commits, "units": r.units, "linesAdded": r.lines, "secs": r.secs, "out": r.out, "tier": r.tier, "notes": r.notes, "summary": summary }), r.warnings))
}

// ───────────────────────── collect ─────────────────────────

fn cmd_collect(a: &Args) -> R<(Value, Vec<String>)> {
    let mut lib = Library::open()?;
    let repos: Vec<PathBuf> = if a.on("all") { lib.projects()?.into_iter().map(|p| PathBuf::from(p.path)).filter(|p| p.is_dir()).collect() } else { vec![a.repo()?] };
    let mut out = vec![];
    let mut warnings = vec![];
    let cfgp = a.flags.get("config").map(PathBuf::from);
    for repo in repos {
        let (n, w, pid) = analyze::collect(&repo, &mut lib, cfgp.as_deref())?;
        warnings.extend(w);
        out.push(json!({ "repo": repo, "project": pid, "newEvidence": n }));
    }
    let summary = out.iter().map(|o| format!("{}：新增证据 {} 条", o["repo"].as_str().unwrap_or(""), o["newEvidence"])).collect::<Vec<_>>().join("\n");
    Ok((json!({ "collected": out, "summary": summary }), warnings))
}

// ───────────────────────── doctor ─────────────────────────

fn cmd_doctor(_a: &Args) -> R<(Value, Vec<String>)> {
    let hs = vec![ad::codex::doctor(), ad::claude::doctor(), ad::cursor::doctor(), ad::grok::doctor(), ad::kimi::doctor(), ad::opencode::doctor(), ad::gemini::doctor()];
    let git_ok = std::process::Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let gh_ok = std::process::Command::new("gh").args(["auth", "status"]).output().map(|o| o.status.success()).unwrap_or(false);
    let lib = Library::open().ok();
    let lib_path = lib.as_ref().map(|l| l.path().display().to_string()).unwrap_or_default();
    let n_proj = lib.as_ref().and_then(|l| l.projects().ok()).map(|p| p.len()).unwrap_or(0);
    let mut lines = vec![];
    for h in &hs {
        lines.push(format!(
            "{:<12} {}  {}{}{}",
            h.source,
            if !std::path::Path::new(&h.path).exists() { "没找到（没装或没用过）" } else if h.readable { "可读" } else { "读不了" },
            if h.records > 0 { format!("{} 条", h.records) } else { String::new() },
            match (&h.earliest, &h.latest) {
                (Some(a), Some(b)) => format!("  {a} → {b}"),
                _ => String::new(),
            },
            if h.note.is_empty() { String::new() } else { format!("  {}", h.note) }
        ));
    }
    lines.push(format!("git          {}", if git_ok { "可用" } else { "未找到" }));
    lines.push(format!("gh           {}", if gh_ok { "已登录" } else { "未登录或未安装（PR 元数据会缺少分支名）" }));
    lines.push(format!("作品库       {lib_path}  {n_proj} 个项目"));
    let warnings: Vec<String> = hs.iter().filter(|h| !h.readable && std::path::Path::new(&h.path).exists()).map(|h| format!("{} 的记录读不了：{}", h.source, h.path)).collect();
    Ok((json!({ "sources": hs, "git": git_ok, "gh": gh_ok, "library": { "path": lib_path, "projects": n_proj }, "summary": lines.join("\n") }), warnings))
}

// ───────────────────────── blame ─────────────────────────

fn cmd_blame(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let q = a.pos.first().ok_or("用法：creditia-core blame <路径>:<行号>")?;
    let (file, line) = q.rsplit_once(':').map(|(f, l)| (f.to_string(), l.parse::<usize>().unwrap_or(1))).unwrap_or((q.clone(), 1));
    // 路径允许是绝对路径或相对 cwd 的路径，统一成仓库内相对路径
    let abs = PathBuf::from(&file);
    let rel = if abs.is_absolute() { abs.strip_prefix(&repo).map(|p| p.display().to_string()).unwrap_or(file.clone()) } else {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        cwd.join(&file).canonicalize().ok().and_then(|p| p.strip_prefix(&repo).ok().map(|p| p.display().to_string())).unwrap_or(file.clone())
    };
    let opts = analyze::Opts { branch: None, fresh: false, github: false, out: None, blame: Some((rel.clone(), line)), config: a.flags.get("config").map(PathBuf::from) };
    let r = analyze::run(&repo, &opts)?;
    let b = r.blame.ok_or(format!("没找到 {rel}:{line}"))?;
    let who = b["harness"].as_str().map(harness_name).or(b["name"].as_str().map(String::from)).unwrap_or("未确认".into());
    let summary = format!(
        "{rel}:{line}\n作者    {who}{}\n提交    {} {}\n{}怎么认出来的  {}",
        b["model"].as_str().map(|m| format!("  {m}")).unwrap_or_default(),
        b["commit"].as_str().unwrap_or(""),
        b["date"].as_str().unwrap_or(""),
        b["unit"].as_str().map(|u| format!("PR/提交 {}  {}\n", u.strip_prefix("pr:").map(|n| format!("#{n}")).unwrap_or_else(|| u.trim_start_matches("c:").to_string()), b["unitTitle"].as_str().unwrap_or(""))).unwrap_or_default(),
        match b["evidence"].as_str().unwrap_or("") {
            "verified" => "编辑成功记录的路径、行哈希和时间匹配",
            "declared" => "提交盖章或明确的工具署名",
            _ => "没有证据",
        }
    );
    let mut v = b;
    v["summary"] = json!(summary);
    Ok((v, r.warnings))
}

// ───────────────────────── stamp / hook ─────────────────────────

fn git_dir(repo: &Path) -> R<PathBuf> {
    let d = ad::git::git(repo, &["rev-parse", "--git-dir"]).trim().to_string();
    if d.is_empty() {
        return Err("不是 git 仓库".into());
    }
    let p = PathBuf::from(&d);
    Ok(if p.is_absolute() { p } else { repo.join(p) })
}

fn cmd_stamp(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.flags.get("repo").map(PathBuf::from).unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    let harness = a.flags.get("harness").ok_or("需要 --harness")?.to_lowercase();
    let v = json!({ "harness": harness, "model": a.flags.get("model"), "share": a.flags.get("share") });
    let p = git_dir(&repo)?.join("creditia-stamp.json");
    std::fs::write(&p, v.to_string()).map_err(|e| e.to_string())?;
    Ok((json!({ "stamp": v, "path": p, "summary": format!("下一次提交将盖章：{}", v) }), vec![]))
}

const HOOK_MARK: &str = "# creditia-stamp";

fn cmd_hook(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let hook = git_dir(&repo)?.join("hooks/prepare-commit-msg");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let line = format!("\"{}\" hook-trailers \"$1\" --repo \"$(git rev-parse --show-toplevel)\" >/dev/null 2>&1 || true {HOOK_MARK}", exe.display());
    match a.pos.first().map(String::as_str) {
        Some("install") => {
            let existing = std::fs::read_to_string(&hook).unwrap_or_default();
            if existing.contains(HOOK_MARK) {
                return Ok((json!({ "path": hook, "summary": "hook 已安装" }), vec![]));
            }
            let content = if existing.is_empty() { format!("#!/bin/sh\n{line}\n") } else { format!("{}\n{line}\n", existing.trim_end()) };
            std::fs::create_dir_all(hook.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&hook, content).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755));
            }
            Ok((json!({ "path": hook, "summary": format!("已安装 {}", hook.display()) }), vec![]))
        }
        Some("uninstall") => {
            let existing = std::fs::read_to_string(&hook).unwrap_or_default();
            let kept: Vec<&str> = existing.lines().filter(|l| !l.contains(HOOK_MARK)).collect();
            if kept.iter().all(|l| l.trim().is_empty() || l.starts_with("#!")) {
                let _ = std::fs::remove_file(&hook);
            } else {
                std::fs::write(&hook, kept.join("\n") + "\n").map_err(|e| e.to_string())?;
            }
            Ok((json!({ "path": hook, "summary": "已卸载" }), vec![]))
        }
        _ => Err("用法：creditia-core hook install|uninstall".into()),
    }
}

/// 识别正在运行的 agent：先看盖章文件，再看环境变量
pub fn detect_agent(env: &HashMap<String, String>) -> Option<(String, Option<String>)> {
    let has = |k: &str| env.get(k).map(|v| !v.is_empty()).unwrap_or(false);
    if has("CLAUDECODE") || has("CLAUDE_CODE_ENTRYPOINT") {
        return Some(("claude-code".into(), env.get("ANTHROPIC_MODEL").cloned()));
    }
    if has("CODEX_SANDBOX") || has("CODEX_THREAD_ID") || env.keys().any(|k| k.starts_with("CODEX_") && k != "CODEX_HOME") {
        return Some(("codex".into(), None));
    }
    if has("CURSOR_AGENT") || has("CURSOR_TRACE_ID") {
        return Some(("cursor".into(), None));
    }
    if has("GROK_SESSION_ID") || has("GROK_AGENT") {
        return Some(("grok".into(), None));
    }
    if has("KIMI_SESSION_ID") || has("KIMI_CODE") {
        return Some(("kimi".into(), None));
    }
    None
}

fn cmd_hook_trailers(a: &Args) -> R<(Value, Vec<String>)> {
    let msgfile = a.pos.first().ok_or("需要提交信息文件路径")?;
    let repo = a.flags.get("repo").map(PathBuf::from).unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    let stamp_path = git_dir(&repo)?.join("creditia-stamp.json");
    let (harness, model, share) = if let Ok(s) = std::fs::read_to_string(&stamp_path) {
        let _ = std::fs::remove_file(&stamp_path);
        let v: Value = serde_json::from_str(&s).unwrap_or_default();
        (v["harness"].as_str().map(String::from), v["model"].as_str().map(String::from), v["share"].as_str().map(String::from))
    } else {
        let env: HashMap<String, String> = std::env::vars().collect();
        match detect_agent(&env) {
            Some((h, m)) => (Some(h), m, None),
            None => (None, None, None),
        }
    };
    let Some(h) = harness else { return Ok((json!({ "added": false, "summary": "" }), vec![])) };
    let msg = std::fs::read_to_string(msgfile).map_err(|e| e.to_string())?;
    if msg.contains("Creditia-Harness:") {
        return Ok((json!({ "added": false, "summary": "" }), vec![]));
    }
    // 在注释行之前插入 trailer（git 会在 # 开头的注释后截断）
    let (body, comments) = split_comments(&msg);
    let mut out = body.trim_end().to_string();
    out.push_str(if ends_with_trailers(&out) { "\n" } else { "\n\n" });
    out.push_str(&format!("Creditia-Harness: {h}\n"));
    if let Some(m) = &model {
        out.push_str(&format!("Creditia-Model: {m}\n"));
    }
    if let Some(s) = &share {
        out.push_str(&format!("Creditia-Share: {s}\n"));
    }
    if !comments.is_empty() {
        out.push('\n');
        out.push_str(&comments);
    }
    std::fs::write(msgfile, out).map_err(|e| e.to_string())?;
    Ok((json!({ "added": true, "harness": h, "model": model, "summary": "" }), vec![]))
}

/// 正文最后一段是不是已有的 trailer 块（如 Signed-off-by），是的话新 trailer 直接接在后面。
/// 只有一段时那是主题行，不算；`fix: xxx` 这类 conventional commit 主题也含 `: `，所以键必须只含 ASCII 字母数字和横线
fn ends_with_trailers(body: &str) -> bool {
    let Some((_, last)) = body.rsplit_once("\n\n") else { return false };
    let is_trailer = |l: &str| {
        let l = l.trim();
        l.is_empty() || l.split_once(": ").map(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).unwrap_or(false)
    };
    !last.trim().is_empty() && last.lines().all(is_trailer)
}

/// 把提交信息拆成正文和 git 注释块（末尾连续的 `#` 行，中间允许空行）
fn split_comments(msg: &str) -> (String, String) {
    let lines: Vec<&str> = msg.lines().collect();
    // 从后往前找：注释块 = 末尾那段只含 `#` 行和空行的区域
    let mut start = lines.len();
    while start > 0 && (lines[start - 1].starts_with('#') || lines[start - 1].trim().is_empty()) {
        start -= 1;
    }
    // 注释块前面的空行留给正文
    let body = lines[..start].join("\n");
    let comments = lines[start..].iter().filter(|l| l.starts_with('#')).cloned().collect::<Vec<_>>().join("\n");
    (body, comments)
}

// ───────────────────────── evidence：多台电脑合并 ─────────────────────────

fn cmd_evidence(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new))?;
    let proj = analyze::project_of(&repo, &cfg);
    let mut lib = Library::open()?;
    match a.pos.first().map(String::as_str) {
        Some("export") => {
            let (evs, sessions) = lib.load_evidence(&proj.id)?;
            let v = json!({ "creditia": "evidence", "version": 1, "project": proj.id, "name": proj.name, "rootCommit": proj.root_commit, "evidence": evs, "sessions": sessions });
            let out = a.flags.get("out").map(PathBuf::from).unwrap_or(PathBuf::from(format!("creditia-evidence-{}.json", proj.id)));
            std::fs::write(&out, serde_json::to_string(&v).unwrap()).map_err(|e| e.to_string())?;
            Ok((json!({ "path": out, "evidence": evs.len(), "sessions": sessions.len(), "summary": format!("已导出 {} 条证据、{} 个会话到 {}（只含路径、行哈希和元数据）", evs.len(), sessions.len(), out.display()) }), vec![]))
        }
        Some("import") => {
            let file = a.pos.get(1).ok_or("用法：creditia-core evidence import <文件>")?;
            let v: Value = serde_json::from_str(&std::fs::read_to_string(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if v["creditia"] != "evidence" {
                return Err("不是 creditia 的证据文件".into());
            }
            if v["rootCommit"].as_str().map(|r| !proj.root_commit.is_empty() && r != proj.root_commit).unwrap_or(false) {
                return Err("这份证据属于另一个仓库（根提交不同）".into());
            }
            let evs: Vec<creditia_core::model::Ev> = serde_json::from_value(v["evidence"].clone()).map_err(|e| e.to_string())?;
            let sessions: Vec<creditia_core::model::Session> = serde_json::from_value(v["sessions"].clone()).unwrap_or_default();
            let mut store = creditia_core::model::Store { version: creditia_core::model::STORE_VERSION, ..Default::default() };
            store.groups.insert("import".into(), creditia_core::model::FileGroup { fp: (0, 0), evs, sessions });
            lib.upsert_project(&proj)?;
            let n = lib.save_evidence(&proj.id, &store)?;
            Ok((json!({ "imported": n, "summary": format!("合并了 {n} 条新证据；下次 analyze 会用上") }), vec![]))
        }
        _ => Err("用法：creditia-core evidence export [--out F] | import <文件>".into()),
    }
}

// ───────────────────────── projects ─────────────────────────

fn cmd_projects(_a: &Args) -> R<(Value, Vec<String>)> {
    let lib = Library::open()?;
    let ps = lib.projects()?;
    let list: Vec<Value> = ps
        .iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "path": p.path, "remotes": p.remotes, "lastHead": p.last_head, "lastAnalyzed": p.last_analyzed.map(creditia_core::util::date_cn), "evidence": lib.evidence_count(&p.id) }))
        .collect();
    let summary = ps.iter().map(|p| format!("{:<28} {}  {}", p.name, p.last_analyzed.map(creditia_core::util::date_cn).unwrap_or("未分析".into()), p.path)).collect::<Vec<_>>().join("\n");
    Ok((json!({ "projects": list, "summary": if summary.is_empty() { "作品库为空；先 creditia-core analyze".into() } else { summary } }), vec![]))
}

#[allow(dead_code)]
fn _src(_: Src) {}


#[cfg(test)]
mod hook_tests {
    use super::ends_with_trailers;

    #[test]
    fn conventional_subject_is_not_a_trailer_block() {
        assert!(!ends_with_trailers("fix: 模型名归一"));
        assert!(!ends_with_trailers("feat: x\n\n说明: 这是正文"));
        assert!(ends_with_trailers("feat: x\n\nSigned-off-by: a <a@b.c>\nCo-authored-by: b <b@c.d>"));
    }
}

/// 终端里给人看的工具名
fn harness_name(h: &str) -> String {
    match h {
        "claude-code" => "Claude Code", "codex" => "Codex", "cursor" => "Cursor", "grok" => "Grok", "kimi" => "Kimi", "copilot" => "GitHub Copilot",
        other => return other.to_string(),
    }
    .to_string()
}

fn cmd_desktop(a: &Args) -> R<(Value, Vec<String>)> {
    let args: Value = serde_json::from_reader(std::io::stdin()).map_err(|e| e.to_string())?;
    let id = args["projectId"].as_str().unwrap_or("");
    let path = args["path"].as_str().unwrap_or("");
    let value = match a.pos.first().map(String::as_str) {
        Some("projects") => creditia_cli::desktop::projects()?,
        Some("library_credits") => creditia_cli::desktop::library_credits()?,
        Some("credits") => creditia_cli::desktop::credits(id)?,
        Some("config_of") => creditia_cli::desktop::config_of(id)?,
        Some("doctor") => creditia_cli::desktop::doctor(),
        Some("analyze_project") => creditia_cli::desktop::analyze_project(path, args["branch"].as_str())?,
        Some("set_people") => creditia_cli::desktop::set_people(id, serde_json::from_value(args["people"].clone()).map_err(|e| e.to_string())?, args["show"].as_bool().unwrap_or(false))?,
        Some("link_path") => creditia_cli::desktop::link_path(id, path)?,
        _ => return Err("未知桌面操作".into()),
    };
    Ok((value, vec![]))
}
fn cmd_link(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let config = analyze::load_config(&repo, None)?;
    let project = analyze::project_of(&repo, &config);
    Library::open()?.upsert_project(&project)?;
    let path = a.flags.get("path").ok_or("用法：creditia link --path <工作副本或旧路径>")?;
    Ok((creditia_cli::desktop::link_path(&project.id, path)?, vec![]))
}
fn cmd_people(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let config = analyze::load_config(&repo, None)?;
    let project = analyze::project_of(&repo, &config);
    Library::open()?.upsert_project(&project)?;
    let file = a.flags.get("file").ok_or("用法：creditia people --file <人员署名 JSON> [--show]")?;
    let people = serde_json::from_reader(std::fs::File::open(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok((creditia_cli::desktop::set_people(&project.id, people, a.on("show"))?, vec![]))
}
