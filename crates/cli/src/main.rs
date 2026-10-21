//! creditia-core：Rust 核心的命令行入口。所有命令非交互，支持 `--json`。
//! 退出码：0 成功，1 失败，2 部分成功（有警告）。

mod analyze;

use creditia_adapters as ad;
use creditia_core::config::{Config, Director, Integrations};
use creditia_core::model::Src;
use creditia_core::tr;
use creditia_store::{Library, Project};
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
            } else if i + 1 < raw.len() && !raw[i + 1].starts_with("--") && !matches!(k, "json" | "fresh" | "force" | "github" | "all" | "yes" | "named") {
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
        let p = p.canonicalize().map_err(|_| tr!("目录不存在：{}", "no such folder: {}", p.display()))?;
        if !ad::git::git_ok(&p, &["rev-parse", "--git-dir"]) {
            return Err(tr!("不是 git 仓库：{}", "not a git repository: {}", p.display()));
        }
        // worktree 映射回主仓库
        Ok(ad::git::main_worktree(&p).unwrap_or(p))
    }
}

const USAGE_ZH: &str = "用法：creditia-core <命令> [选项]

  discover                         列出本机检测到 agent 活动的项目
  init      [--repo P] [--force]   在仓库写 .creditia/config.json
  analyze   [--repo P] [--fresh] [--github] [--level strict|standard|loose] [--out F] [--config F]
                                   分析并写 .creditia/credits.json（全程本地，不调用任何大模型）
  update    [--repo P]             增量更新（同 analyze）
  collect   [--repo P | --all]     只采集证据进证据库
  doctor                           检查各数据源
  blame     <路径>:<行号> [--repo P]  这一行是谁写的
  stamp     --harness H [--model M] [--share S]   给下一次提交盖章
  hook      install|uninstall|status [--repo P]    提交盖章 hook：提交时自动加 Creditia-* trailer（工具、模型、各工具行数）
  release   install|uninstall|status [--repo P]    跟着发版：写 .github/workflows/creditia.yml（Release 说明、PR 评论）
  integrations [--repo P]          上面两项的现状
  hook-trailers <msgfile>          （hook 调用）追加 Creditia-* trailer
  evidence  export [--out F] | import <文件>   多台电脑之间合并证据库
  projects                         本地已记录的项目
  all-credits                      本地每个项目最新的档案（跨项目汇总用）
  paths     [--repo P]             本项目登记过的全部路径（主仓库、worktree、临时克隆、旧路径）
  link      <路径> [--repo P]      把一个路径关联到本项目（别的电脑上的路径、已删除的临时目录）
  unlink    <路径> [--repo P]      取消关联
  whoami                           当前在哪个 agent 里运行（agent 给自己盖章用）

init / analyze 在终端里运行时，会问一次要不要装上面两项（答过就记进 .creditia/config.json，不再问）；
加 --json 时不问，把还没决定的列在 ask 里。

所有命令支持 --json 和 --lang zh|en（也可以设环境变量 CREDITIA_LANG）。退出码：0 成功，1 失败，2 有警告。";

const USAGE_EN: &str = "Usage: creditia-core <command> [options]

  discover                         list projects on this machine with agent activity
  init      [--repo P] [--force]   write .creditia/config.json in the repository
  analyze   [--repo P] [--fresh] [--github] [--level strict|standard|loose] [--out F] [--config F]
                                   analyze and write .creditia/credits.json (all local, never calls a model)
  update    [--repo P]             incremental update (same as analyze)
  collect   [--repo P | --all]     only collect evidence into the library
  doctor                           check every data source
  blame     <path>:<line> [--repo P]  which tool and model wrote this line
  stamp     --harness H [--model M] [--share S]   stamp the next commit
  hook      install|uninstall|status [--repo P]    commit stamps: Creditia-* trailers on commits (tool, model, lines per tool)
  release   install|uninstall|status [--repo P]    release credits: write .github/workflows/creditia.yml (release notes, PR comments)
  integrations [--repo P]          where those two stand
  hook-trailers <msgfile>          (called by the hook) append Creditia-* trailers
  evidence  export [--out F] | import <file>   merge evidence between machines
  projects                         projects in the library
  all-credits                      the latest credits of every project in the library (for the cross-project summary)
  paths     [--repo P]             every path recorded for this project (main, worktrees, temp clones, old paths)
  link      <path> [--repo P]      link a path to this project (a path on another machine, a deleted temp folder)
  unlink    <path> [--repo P]      remove a link
  whoami                           which agent this is running in (for agents stamping themselves)

Run in a terminal, init / analyze ask once whether to add those two (the answer goes into .creditia/config.json and isn't asked again);
with --json they don't ask and list the undecided ones under ask.

Every command takes --json and --lang zh|en (or set CREDITIA_LANG). Exit codes: 0 ok, 1 failed, 2 warnings.";

fn usage() -> &'static str {
    if creditia_core::i18n::en() { USAGE_EN } else { USAGE_ZH }
}

fn main() {
    let a = parse_args();
    if let Some(l) = a.flags.get("lang") {
        creditia_core::i18n::set(l);
    }
    let json = a.on("json");
    creditia_core::progress::init();
    refresh_shim(false);
    let r: R<(Value, Vec<String>)> = match a.cmd.as_str() {
        "discover" => cmd_discover(&a),
        "init" => cmd_init(&a),
        "analyze" | "update" => cmd_analyze(&a),
        "collect" => cmd_collect(&a),
        "doctor" => cmd_doctor(&a),
        "blame" => cmd_blame(&a),
        "stamp" => cmd_stamp(&a),
        "hook" => cmd_hook(&a),
        "release" => cmd_release(&a),
        "integrations" => cmd_integrations(&a),
        "hook-trailers" => cmd_hook_trailers(&a),
        "evidence" => cmd_evidence(&a),
        "projects" => cmd_projects(&a),
        "all-credits" => cmd_all_credits(&a),
        "paths" => cmd_paths(&a),
        "link" | "unlink" => cmd_link(&a),
        "whoami" => cmd_whoami(&a),
        "" | "help" | "-h" | "--help" => {
            println!("{}", usage());
            return;
        }
        other => Err(tr!("未知命令：{other}\n\n{}", "unknown command: {other}\n\n{}", usage())),
    };
    creditia_core::progress::done();
    match r {
        Ok((mut v, warnings)) => {
            let ask_repo = v.as_object_mut().and_then(|o| o.remove("askRepo")).and_then(|r| r.as_str().map(PathBuf::from));
            if json {
                if !warnings.is_empty() {
                    v["warnings"] = json!(warnings);
                }
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else if let Some(s) = v["summary"].as_str() {
                println!("{s}");
                for w in &warnings {
                    eprintln!("{}{w}", tr!("警告：", "warning: "));
                }
                if let (Some(repo), Some(todo)) = (ask_repo, v["ask"].as_array()) {
                    ask_integrations(&repo, todo);
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
                eprintln!("{}{e}", tr!("错误：", "error: "));
            }
            std::process::exit(1);
        }
    }
}

// ───────────────────────── discover ─────────────────────────

fn cmd_discover(a: &Args) -> R<(Value, Vec<String>)> {
    let mut acts: Vec<ad::Activity> = vec![];
    acts.extend(ad::codex::discover());
    acts.extend(ad::claude::discover());
    acts.extend(ad::cursor::discover());
    acts.extend(ad::grok::discover());
    acts.extend(ad::kimi::discover());
    acts.extend(ad::gemini::discover());
    acts.extend(ad::qwen::discover());
    acts.extend(ad::opencode::discover());
    acts.extend(ad::cline::discover());
    acts.extend(ad::copilot::discover());
    acts.extend(ad::amp::discover());
    acts.extend(ad::trae::discover());
    acts.extend(ad::crush::discover());
    acts.extend(ad::goose::discover());
    acts.extend(ad::cont::discover());
    acts.extend(ad::zed::discover());
    acts.extend(ad::amazonq::discover());
    acts.extend(ad::antigravity::discover());
    // 把 cwd 归到 git 仓库：存在的目录用根提交；不存在的用远程地址或目录名
    #[derive(Default)]
    struct P {
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
    let known: HashMap<String, Project> = lib.as_ref().and_then(|l| l.projects().ok()).unwrap_or_default().into_iter().map(|p| (p.root_commit.clone(), p)).collect();
    for act in acts {
        let cwd = PathBuf::from(&act.cwd);
        let (key, p) = if cwd.is_dir() && ad::git::git_ok(&cwd, &["rev-parse", "--git-dir"]) {
            let main = ad::git::main_worktree(&cwd).unwrap_or(cwd.clone());
            let (root, remotes) = ad::git::identity(&main);
            let key = if root.is_empty() { main.display().to_string() } else { root.clone() };
            (key, P { name: main.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), path: main.display().to_string(), root, remotes, exists: true, ..Default::default() })
        } else {
            let remote = act.remote.as_deref().map(ad::git::normalize_remote);
            let name = remote.as_deref().and_then(|r| r.rsplit('/').next()).map(String::from).unwrap_or_else(|| cwd.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(act.cwd.clone()));
            let key = remote.clone().unwrap_or(format!("path:{}", act.cwd));
            (key, P { name, path: act.cwd.clone(), remotes: remote.into_iter().collect(), ..Default::default() })
        };
        let e = by_key.entry(key).or_insert(p);
        *e.tools.entry(act.src.harness().into()).or_default() += act.sessions;
        e.last = e.last.max(act.last);
    }
    // 不存在的目录如果远程地址和某个存在的项目一致，并进去
    let remote_to_key: HashMap<String, String> = by_key.iter().filter(|(_, p)| p.exists).flat_map(|(k, p)| p.remotes.iter().map(move |r| (r.clone(), k.clone()))).collect();
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
        if let Some(k) = p.remotes.iter().find_map(|r| remote_to_key.get(r)) {
            if let Some(t) = merged.iter_mut().find(|x| &x.root == k || &x.path == k) {
                for (s, n) in p.tools {
                    *t.tools.entry(s).or_default() += n;
                }
                t.last = t.last.max(p.last);
                continue;
            }
        }
        merged.push(p);
    }
    merged.sort_by_key(|p| std::cmp::Reverse(p.last));
    // 已经不存在、又只有零星活动的临时目录默认不列（--all 列出）
    let show_all = a.on("all");
    merged.retain(|p| show_all || p.exists || p.tools.values().sum::<u64>() >= 3);
    let status = |p: &P| if p.exists { p.path.clone() } else if PathBuf::from(&p.path).is_dir() { tr!("{}  (不是 git 仓库)", "{}  (not a git repository)", p.path) } else { tr!("{}  (目录已不存在)", "{}  (folder no longer exists)", p.path) };
    let list: Vec<Value> = merged
        .iter()
        .map(|p| {
            json!({
                "name": p.name, "path": p.path, "rootCommit": p.root, "remotes": p.remotes, "exists": p.exists,
                "tools": p.tools, "lastActive": creditia_core::util::date_cn(p.last),
                "inLibrary": known.contains_key(&p.root),
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

/// 自动探测项目配置：主分支、主导者（最常见的人类提交者）、远程、路径关键词
pub fn detect_config(repo: &Path) -> Config {
    let mut c = Config { repo: repo.display().to_string(), name: project_name(repo), ..Default::default() };
    let cur = ad::git::git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim().to_string();
    c.branch = if !cur.is_empty() && cur != "HEAD" {
        cur
    } else if ad::git::git_ok(repo, &["rev-parse", "--verify", "main"]) {
        "main".into()
    } else {
        "master".into()
    };
    let (_, remotes) = ad::git::identity(repo);
    c.remotes = remotes.clone();
    // 主导者：把 (名字, 邮箱) 聚成"同一个人"——同名、同邮箱、同 GitHub 登录名（123+login@users.noreply）、
    // 名字等于某个邮箱的用户名，都视为同一人；提交最多的那个人是主导者，收集 TA 用过的全部邮箱
    let log = ad::git::git(repo, &["log", "--format=%aN%x00%aE", &c.branch]);
    let mut pairs: Vec<(String, String)> = vec![];
    let mut count: HashMap<(String, String), usize> = HashMap::new();
    for l in log.lines() {
        let Some((n, e)) = l.split_once('\0') else { continue };
        let e = e.to_lowercase();
        if c.agent_emails.contains_key(&e) || c.bot_emails.contains(&e) || e.ends_with("@cursor.com") || e.contains("[bot]") {
            continue;
        }
        let k = (n.to_string(), e);
        *count.entry(k.clone()).or_default() += 1;
        if !pairs.contains(&k) {
            pairs.push(k);
        }
    }
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        if p[i] != i {
            let r = find(p, p[i]);
            p[i] = r;
        }
        p[i]
    }
    let login = |e: &str| -> String {
        let local = e.split('@').next().unwrap_or("");
        if e.ends_with("@users.noreply.github.com") { local.split_once('+').map(|(_, l)| l).unwrap_or(local).to_string() } else { local.to_string() }
    };
    let norm = |s: &str| s.to_lowercase().replace([' ', '-', '_', '.'], "");
    let n = pairs.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            let (ni, ei) = &pairs[i];
            let (nj, ej) = &pairs[j];
            let same = norm(ni) == norm(nj) || ei == ej || login(ei) == login(ej) || norm(ni) == norm(&login(ej)) || norm(nj) == norm(&login(ei));
            if same {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    type Cluster = (usize, Vec<String>, HashMap<String, usize>);
    let mut clusters: HashMap<usize, Cluster> = HashMap::new();
    for (i, k) in pairs.iter().enumerate() {
        let r = find(&mut parent, i);
        let e = clusters.entry(r).or_default();
        let cnt = count[k];
        e.0 += cnt;
        if !e.1.contains(&k.1) {
            e.1.push(k.1.clone());
        }
        *e.2.entry(k.0.clone()).or_default() += cnt;
    }
    if let Some((_, (_, emails, names))) = clusters.into_iter().max_by_key(|(_, (n, _, _))| *n) {
        let name = names.into_iter().max_by_key(|(_, n)| *n).map(|(n, _)| n).unwrap_or_default();
        let mut emails = emails;
        for e in emails.clone() {
            emails.extend(ad::git::mailmap_emails(repo, &e));
        }
        emails.sort();
        emails.dedup();
        c.directors.push(Director { id: format!("h:{}", name.to_lowercase().replace(' ', "-")), name, emails });
    }
    // 把仓库里见过的 agent / bot 邮箱补进映射。只认确定的格式，不按邮箱里有没有「claude」「codex」这类字样去猜：
    // - GitHub 的 bot 账号：<数字>+<名字>[bot]@users.noreply.github.com，名字是已知的 agent 应用才算 agent，否则算 bot
    // - 各家官方的提交邮箱（default_agent_emails 里已有）
    const BOT_AGENTS: [(&str, &str); 6] = [("copilot-swe-agent", "copilot"), ("devin-ai-integration", "devin"), ("google-labs-jules", "jules"), ("cursor", "cursor"), ("claude", "claude-code"), ("codex", "codex")];
    for l in log.lines() {
        if let Some((_, e)) = l.split_once('\0') {
            let e = e.to_lowercase();
            if c.agent_emails.contains_key(&e) || c.bot_emails.contains(&e) {
                continue;
            }
            let Some(local) = e.strip_suffix("@users.noreply.github.com") else { continue };
            let Some(app) = local.split_once('+').map(|(_, n)| n).unwrap_or(local).strip_suffix("[bot]") else { continue };
            match BOT_AGENTS.iter().find(|(n, _)| *n == app) {
                Some((_, h)) => { c.agent_emails.insert(e.clone(), h.to_string()); }
                None => c.bot_emails.push(e.clone()),
            }
        }
    }
    c.markers = derive_markers(repo, &remotes);
    c.exclude_markers = detect_excludes(repo, &remotes);
    c
}

/// 项目名：有远程地址就用远程仓库名（克隆进什么目录都一样），否则用目录名
pub fn project_name(repo: &Path) -> String {
    let (_, remotes) = ad::git::identity(repo);
    let pick = remotes.iter().find(|r| r.contains("github.com")).or(remotes.first());
    pick.and_then(|r| r.rsplit('/').next()).map(String::from).filter(|s| !s.is_empty())
        .unwrap_or_else(|| repo.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
}

/// 判断日志路径是否属于本项目的关键词：目录名、各 worktree 目录、远程仓库名。
/// `/name/` 按目录边界匹配；`/name-` 匹配 Codex 等工具的临时克隆（如 `/tmp/demo-<任务>/`），
/// 同名前缀的无关目录（如 `demo-film`）由 `detect_excludes` 排除
pub fn derive_markers(repo: &Path, remotes: &[String]) -> Vec<String> {
    let mut m: Vec<String> = vec![];
    let mut push = |s: String| {
        let s = s.to_lowercase();
        if s.len() >= 4 && !m.contains(&s) {
            m.push(s);
        }
    };
    if let Some(n) = repo.file_name() {
        let n = n.to_string_lossy();
        push(format!("/{n}/"));
        push(format!("/{n}-"));
    }
    for w in ad::git::worktrees(repo) {
        push(format!("{}/", w.trim_end_matches('/')));
    }
    for r in remotes {
        if let Some(n) = r.rsplit('/').next() {
            push(format!("/{n}/"));
            push(format!("/{n}-"));
        }
    }
    m
}

/// 同级目录里名字以仓库名开头、但不是本仓库的克隆或 worktree 的目录（如 `demo-film`），自动排除
pub fn detect_excludes(repo: &Path, remotes: &[String]) -> Vec<String> {
    let (root, _) = ad::git::identity(repo);
    let mut prefixes: Vec<String> = vec![];
    if let Some(n) = repo.file_name() {
        prefixes.push(n.to_string_lossy().to_lowercase());
    }
    for r in remotes {
        if let Some(n) = r.rsplit('/').next() {
            prefixes.push(n.to_lowercase());
        }
    }
    let Some(parent) = repo.parent() else { return vec![] };
    let same_repo = |d: &Path| !root.is_empty() && ad::git::git_ok(d, &["rev-parse", "--git-dir"]) && ad::git::identity(d).0 == root;
    let mut out = vec![];
    for e in std::fs::read_dir(parent).into_iter().flatten().flatten() {
        let p = e.path();
        if p == repo || !p.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !prefixes.iter().any(|x| name.starts_with(x.as_str()) && name != *x) {
            continue;
        }
        // 本身是同一仓库的克隆 / worktree，或者下一级目录里有（如 `demo-worktrees/<分支>`），都算相关
        let related = same_repo(&p) || std::fs::read_dir(&p).into_iter().flatten().flatten().any(|s| s.path().is_dir() && same_repo(&s.path()));
        if !related {
            out.push(format!("/{name}/"));
        }
    }
    out.sort();
    out
}

fn cmd_init(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let path = repo.join(".creditia/config.json");
    // 只有接入开关、显示设置的半份配置（App 里先拨过开关）不算已初始化，那两段原样带过去
    if Config::load(&path).is_ok() && !a.on("force") {
        return Err(tr!("{} 已存在；加 --force 覆盖", "{} already exists; add --force to overwrite", path.display()));
    }
    let old: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut c = detect_config(&repo);
    c.integrations = Integrations::of(&repo);
    if let Ok(d) = serde_json::from_value(old["display"].clone()) {
        c.display = d;
    }
    c.save(&path)?;
    let v = serde_json::to_value(&c).unwrap();
    let who = c.directors.first().map(|d| d.name.clone()).unwrap_or_else(|| tr!("（未识别）", "(not found)"));
    let summary = tr!(
        "已写入 {}\n项目 {} · 分支 {} · 你的提交身份 {}\n可在该文件里调整你的邮箱、口径（level）、其他路径（paths）、隐私选项。",
        "Wrote {}\nproject {} · branch {} · your commit identity {}\nEdit that file to change your emails, the level, other paths (paths) and privacy options.",
        path.display(),
        c.name,
        c.branch,
        who
    );
    let mut out = json!({ "path": path, "config": v, "summary": summary });
    mark_ask(&repo, &mut out);
    Ok((out, vec![]))
}

// ───────────────────────── analyze / update ─────────────────────────

fn cmd_analyze(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let invoked = std::env::current_dir().ok().and_then(|p| p.canonicalize().ok());
    let opts = analyze::Opts { fresh: a.on("fresh"), github: a.on("github"), out: a.flags.get("out").map(PathBuf::from), blame: None, config: a.flags.get("config").map(PathBuf::from), level: a.flags.get("level").cloned(), invoked };
    let r = analyze::run(&repo, &opts)?;
    let summary = tr!(
        "{}：{} 个提交（新增 {}），{} 项，{} 行；{:.1}s\n{}\n写入 {}",
        "{}: {} commits ({} new), {} items, {} lines; {:.1}s\n{}\nwrote {}",
        r.name,
        r.commits,
        r.new_commits,
        r.units,
        r.lines,
        r.secs,
        r.tier,
        r.out.display()
    );
    let mut out = json!({ "project": r.name, "head": r.head, "commits": r.commits, "newCommits": r.new_commits, "units": r.units, "linesAdded": r.lines, "secs": r.secs, "out": r.out, "tier": r.tier, "notes": r.notes, "summary": summary });
    mark_ask(&repo, &mut out);
    Ok((out, r.warnings))
}

// ───────────────────────── collect ─────────────────────────

fn cmd_collect(a: &Args) -> R<(Value, Vec<String>)> {
    let mut lib = Library::open()?;
    let repos: Vec<PathBuf> = if a.on("all") { lib.projects()?.into_iter().map(|p| PathBuf::from(p.path)).filter(|p| p.is_dir()).collect() } else { vec![a.repo()?] };
    let mut out = vec![];
    let mut warnings = vec![];
    let cfgp = a.flags.get("config").map(PathBuf::from);
    let all = a.on("all");
    for repo in repos {
        match analyze::collect(&repo, &mut lib, cfgp.as_deref()) {
            Ok((n, w, pid)) => {
                warnings.extend(w);
                out.push(json!({ "repo": repo, "project": pid, "newEvidence": n }));
            }
            // --all 多半是后台定时跑的：一个项目收不成记下来，接着收别的
            Err(e) if all => {
                warnings.push(tr!("{}：{e}", "{}: {e}", repo.display()));
                out.push(json!({ "repo": repo, "error": e }));
            }
            Err(e) => return Err(e),
        }
    }
    let summary = out.iter().map(|o| match o["error"].as_str() {
        Some(e) => tr!("{}：没收成（{e}）", "{}: failed ({e})", o["repo"].as_str().unwrap_or("")),
        None => tr!("{}：新增证据 {} 条", "{}: {} new pieces of evidence", o["repo"].as_str().unwrap_or(""), o["newEvidence"]),
    }).collect::<Vec<_>>().join("\n");
    Ok((json!({ "collected": out, "summary": summary }), warnings))
}

// ───────────────────────── doctor ─────────────────────────

fn cmd_doctor(_a: &Args) -> R<(Value, Vec<String>)> {
    use creditia_core::model::Src;
use creditia_core::tr;
    let mut hs = vec![
        ad::codex::doctor(), ad::claude::doctor(), ad::cursor::doctor(), ad::grok::doctor(), ad::kimi::doctor(),
        ad::gemini::doctor(Src::Gemini), ad::qwen::doctor(),
        ad::cline::doctor(Src::Cline), ad::cline::doctor(Src::Roo), ad::cline::doctor(Src::Kilo),
        ad::aider::doctor(), ad::amp::doctor(), ad::trae::doctor(), ad::crush::doctor(), ad::goose::doctor(), ad::cont::doctor(), ad::zed::doctor(), ad::amazonq::doctor(), ad::antigravity::doctor(),
    ];
    hs.extend(ad::opencode::doctor());
    hs.extend(ad::copilot::doctor());
    hs.extend(ad::others::doctor());
    let git_ok = std::process::Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let gh_ok = std::process::Command::new("gh").args(["auth", "status"]).output().map(|o| o.status.success()).unwrap_or(false);
    let lib = Library::open().ok();
    let lib_path = lib.as_ref().map(|l| l.path().display().to_string()).unwrap_or_default();
    let n_proj = lib.as_ref().and_then(|l| l.projects().ok()).map(|p| p.len()).unwrap_or(0);
    let mut lines = vec![];
    for h in &hs {
        // 没装的工具：只说没找到，不列格式说明和打开失败的原文
        if !h.readable && !h.unsupported && !std::path::Path::new(&h.path).exists() {
            lines.push(format!("{:<15} {}", h.source, tr!("未找到", "not found")));
            continue;
        }
        lines.push(format!(
            "{:<15} {}  {}{}{}",
            h.source,
            if h.readable { tr!("可读", "readable") } else if h.unsupported { tr!("不支持", "unsupported") } else { tr!("不可读", "unreadable") },
            if h.records > 0 { tr!("{} 条", "{} records", h.records) } else { String::new() },
            match (&h.earliest, &h.latest) {
                (Some(a), Some(b)) => format!("  {a} → {b}"),
                _ => String::new(),
            },
            if h.note.is_empty() { String::new() } else { format!("  {}", h.note) }
        ));
    }
    lines.push(format!("{:<15} {}", "git", if git_ok { tr!("可用", "available") } else { tr!("未找到", "not found") }));
    lines.push(format!("{:<15} {}", "gh", if gh_ok { tr!("已登录", "signed in") } else { tr!("未登录或未安装（PR 元数据会缺少分支名）", "not signed in or not installed (PR metadata will lack branch names)") }));
    lines.push(format!("{:<15} {lib_path}  {}", tr!("本地项目库", "library"), if creditia_core::i18n::en() { format!("{n_proj} project{}", if n_proj == 1 { "" } else { "s" }) } else { format!("{n_proj} 个项目") }));
    // 没装的工具不算问题；装了却读不了才警告
    let warnings: Vec<String> = hs.iter().filter(|h| !h.readable && !h.unsupported && std::path::Path::new(&h.path).exists()).map(|h| tr!("{} 不可读：{}", "{} unreadable: {}", h.source, h.path)).collect();
    Ok((json!({ "sources": hs, "git": git_ok, "gh": gh_ok, "library": { "path": lib_path, "projects": n_proj }, "summary": lines.join("\n") }), warnings))
}

// ───────────────────────── blame ─────────────────────────

fn cmd_blame(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let q = a.pos.first().ok_or_else(|| tr!("用法：creditia-core blame <路径>:<行号>", "usage: creditia-core blame <path>:<line>"))?;
    let (file, line) = q.rsplit_once(':').map(|(f, l)| (f.to_string(), l.parse::<usize>().unwrap_or(1))).unwrap_or((q.clone(), 1));
    // 路径允许是绝对路径或相对 cwd 的路径，统一成仓库内相对路径
    let abs = PathBuf::from(&file);
    let rel = if abs.is_absolute() { abs.strip_prefix(&repo).map(|p| p.display().to_string()).unwrap_or(file.clone()) } else {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        cwd.join(&file).canonicalize().ok().and_then(|p| p.strip_prefix(&repo).ok().map(|p| p.display().to_string())).unwrap_or(file.clone())
    };
    let opts = analyze::Opts { fresh: false, github: false, out: None, blame: Some((rel.clone(), line)), config: a.flags.get("config").map(PathBuf::from), level: a.flags.get("level").cloned(), invoked: None };
    let r = analyze::run(&repo, &opts)?;
    let b = r.blame.ok_or_else(|| tr!("没找到 {rel}:{line}", "not found: {rel}:{line}"))?;
    let who = b["harness"].as_str().map(|h| h.to_string()).or(b["name"].as_str().map(String::from)).unwrap_or_else(|| tr!("未确认", "unattributed"));
    let summary = tr!(
        "{rel}:{line}\n作者  {who}{}\n提交  {} {}\n{}证据  {}",
        "{rel}:{line}\nwritten by  {who}{}\ncommit      {} {}\n{}evidence    {}",
        b["model"].as_str().map(|m| format!("  {m}")).unwrap_or_default(),
        b["commit"].as_str().unwrap_or(""),
        b["date"].as_str().unwrap_or(""),
        b["unit"].as_str().map(|u| tr!("项    {u}  {}\n", "item        {u}  {}\n", b["unitTitle"].as_str().unwrap_or(""))).unwrap_or_default(),
        b["evidence"].as_str().unwrap_or("")
    );
    let mut v = b;
    v["summary"] = json!(summary);
    Ok((v, r.warnings))
}

// ───────────────────────── stamp / hook ─────────────────────────

fn git_dir(repo: &Path) -> R<PathBuf> {
    let d = ad::git::git(repo, &["rev-parse", "--git-dir"]).trim().to_string();
    if d.is_empty() {
        return Err(tr!("不是 git 仓库", "not a git repository"));
    }
    let p = PathBuf::from(&d);
    Ok(if p.is_absolute() { p } else { repo.join(p) })
}

fn cmd_stamp(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.flags.get("repo").map(PathBuf::from).unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    let harness = a.flags.get("harness").ok_or_else(|| tr!("需要 --harness", "--harness is required"))?.to_lowercase();
    let v = json!({ "harness": harness, "model": a.flags.get("model"), "share": a.flags.get("share") });
    let p = git_dir(&repo)?.join("creditia-stamp.json");
    std::fs::write(&p, v.to_string()).map_err(|e| e.to_string())?;
    Ok((json!({ "stamp": v, "path": p, "summary": tr!("下一次提交将盖章：{}", "the next commit will be stamped: {}", v) }), vec![]))
}

const HOOK_MARK: &str = "# creditia-stamp";

/// prepare-commit-msg 的位置：设了 core.hooksPath 的仓库（husky 之类）用它指的目录
fn hook_path(repo: &Path) -> R<PathBuf> {
    git_dir(repo)?;
    let d = ad::git::git(repo, &["rev-parse", "--git-path", "hooks"]).trim().to_string();
    let p = PathBuf::from(&d);
    Ok(if p.is_absolute() { p } else { repo.join(p) }.join("prepare-commit-msg"))
}

/// hook 里的那一行。核心的位置会变（App 挪了位置、npm 升级换了目录），所以依次试：
/// 数据目录里的固定入口（每次运行核心都会把它指向自己）、装 hook 时的核心、PATH 里的 creditia-core
fn hook_line() -> R<String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let shim = shim_path();
    Ok(format!(
        "for c in \"{}\" \"{}\" creditia-core; do if command -v \"$c\" >/dev/null 2>&1; then \"$c\" hook-trailers \"$1\" --repo \"$(git rev-parse --show-toplevel)\" >/dev/null 2>&1; break; fi; done; true {HOOK_MARK}",
        shim.display(),
        exe.display()
    ))
}

/// 固定入口：数据目录下的 bin/creditia-core，指向最近运行过的核心
fn shim_path() -> PathBuf {
    creditia_store::data_dir().join("bin").join(if cfg!(windows) { "creditia-core.exe" } else { "creditia-core" })
}

/// 让固定入口指向正在运行的这个核心。Unix 上是符号链接；Windows 上不做（hook 里还有另外两个候选）。
/// 没装过 hook 的电脑上不创建（force 为假时只更新已有的）
fn refresh_shim(force: bool) {
    #[cfg(unix)]
    {
        let Ok(exe) = std::env::current_exe() else { return };
        let link = shim_path();
        if !force && std::fs::symlink_metadata(&link).is_err() {
            return;
        }
        if std::fs::read_link(&link).ok().as_deref() == Some(exe.as_path()) {
            return;
        }
        let _ = std::fs::create_dir_all(link.parent().unwrap());
        let tmp = link.with_extension("tmp");
        let _ = std::fs::remove_file(&tmp);
        if std::os::unix::fs::symlink(&exe, &tmp).is_ok() {
            let _ = std::fs::rename(&tmp, &link);
        }
    }
}

/// 旧版本装的 hook 写死了当时核心的路径：换成现在这一行
fn upgrade_hook(repo: &Path) {
    let (Ok(hook), Ok(line)) = (hook_path(repo), hook_line()) else { return };
    let Ok(text) = std::fs::read_to_string(&hook) else { return };
    if !text.contains(HOOK_MARK) || text.lines().any(|l| l == line) {
        return;
    }
    let out: Vec<&str> = text.lines().map(|l| if l.contains(HOOK_MARK) { line.as_str() } else { l }).collect();
    let _ = std::fs::write(&hook, out.join("\n") + "\n");
}

fn hook_installed(repo: &Path) -> bool {
    hook_path(repo).ok().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| t.contains(HOOK_MARK))
}

fn cmd_hook(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let r = hook_set(&repo, a.pos.first().map(String::as_str))?;
    Ok((r, vec![]))
}

/// install / uninstall / status。装和卸都算用户做了决定，记进 .creditia/config.json，之后不再问
fn hook_set(repo: &Path, act: Option<&str>) -> R<Value> {
    let hook = hook_path(repo)?;
    let line = hook_line()?;
    match act {
        Some("status") => {
            let on = hook_installed(repo);
            Ok(json!({ "installed": on, "decided": Integrations::of(repo).hook, "path": hook, "summary": if on { tr!("已安装：{}", "installed: {}", hook.display()) } else { tr!("没有安装", "not installed") } }))
        }
        Some("install") => {
            Integrations::record(repo, "hook", true)?;
            refresh_shim(true);
            let existing = std::fs::read_to_string(&hook).unwrap_or_default();
            if existing.contains(HOOK_MARK) {
                upgrade_hook(repo);
                return Ok(json!({ "installed": true, "path": hook, "summary": tr!("hook 已安装", "hook already installed") }));
            }
            let content = if existing.is_empty() { format!("#!/bin/sh\n{line}\n") } else { format!("{}\n{line}\n", existing.trim_end()) };
            std::fs::create_dir_all(hook.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&hook, content).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755));
            }
            Ok(json!({ "installed": true, "path": hook, "summary": tr!("已安装 {}", "installed {}", hook.display()) }))
        }
        Some("uninstall") => {
            Integrations::record(repo, "hook", false)?;
            let existing = std::fs::read_to_string(&hook).unwrap_or_default();
            if !existing.contains(HOOK_MARK) {
                return Ok(json!({ "installed": false, "path": hook, "summary": tr!("没有安装，不用卸载", "not installed; nothing to remove") }));
            }
            let kept: Vec<&str> = existing.lines().filter(|l| !l.contains(HOOK_MARK)).collect();
            if kept.iter().all(|l| l.trim().is_empty() || l.starts_with("#!")) {
                let _ = std::fs::remove_file(&hook);
            } else {
                std::fs::write(&hook, kept.join("\n") + "\n").map_err(|e| e.to_string())?;
            }
            Ok(json!({ "installed": false, "path": hook, "summary": tr!("已卸载", "uninstalled") }))
        }
        _ => Err(tr!("用法：creditia hook install|uninstall|status", "usage: creditia hook install|uninstall|status")),
    }
}

// ───────────────────────── release：跟着发版走 ─────────────────────────

const RELEASE_MARK: &str = "# creditia-release";

fn workflow_path(repo: &Path) -> PathBuf {
    repo.join(".github/workflows/creditia.yml")
}

fn release_installed(repo: &Path) -> bool {
    std::fs::read_to_string(workflow_path(repo)).is_ok_and(|t| t.contains(RELEASE_MARK))
}

/// 写进用户仓库的 workflow。CI 里没有 agent 日志，用的是仓库里的 credits.json（creditia publish 写出）
fn release_workflow(lang: &str) -> String {
    let head = if lang == "en" {
        "# Written by `creditia release install`; `creditia release uninstall` removes it.\n# When a release is published, its credits go into the release notes; pull requests get a comment with their credits.\n# CI has no agent logs: this reads the line counts that commit stamps (`creditia hook install`) put on each commit, or credits.json if the repo has one."
    } else {
        "# 由 creditia release install 写入；creditia release uninstall 删除。\n# 发 Release 时把这一版的署名写进说明；PR 上评论这个 PR 的署名。\n# CI 里没有 agent 日志：读提交盖章（creditia hook install）在每个提交上记下的各工具行数；仓库里有 credits.json 时优先用它。"
    };
    format!(
        "{RELEASE_MARK}\n{head}\nname: creditia\non:\n  pull_request:\n  release:\n    types: [published]\npermissions:\n  contents: write\n  pull-requests: write\njobs:\n  credits:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n      - uses: WroughtMind/creditia@main\n        with:\n          release-notes: true\n          pr-comment: true\n          lang: {lang}\n"
    )
}

fn cmd_release(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    Ok((release_set(&repo, a.pos.first().map(String::as_str))?, vec![]))
}

fn release_set(repo: &Path, act: Option<&str>) -> R<Value> {
    let wf = workflow_path(repo);
    let lang = if creditia_core::i18n::en() { "en" } else { "zh" };
    match act {
        Some("status") => {
            let on = release_installed(repo);
            Ok(json!({ "installed": on, "decided": Integrations::of(repo).release, "path": wf, "summary": if on { tr!("已接入：{}", "on: {}", wf.display()) } else { tr!("没有接入", "off") } }))
        }
        Some("install") => {
            let existing = std::fs::read_to_string(&wf).ok();
            if existing.as_deref().is_some_and(|t| !t.contains(RELEASE_MARK)) {
                return Err(tr!("{} 已存在，而且不是 creditia 写的；没有覆盖", "{} already exists and wasn't written by creditia; left it alone", wf.display()));
            }
            Integrations::record(repo, "release", true)?;
            std::fs::create_dir_all(wf.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&wf, release_workflow(lang)).map_err(|e| e.to_string())?;
            let next = if hook_installed(repo) || repo.join("credits.json").exists() {
                String::new()
            } else {
                tr!("\n提交盖章还没打开：打开后，之后的提交会自动带上各工具写的行数，PR 评论和 Release 说明就有内容了（creditia hook install）。", "\nCommit stamps are off: turn them on and new commits carry each tool's line count, which is what PR comments and release notes read (creditia hook install).")
            };
            Ok(json!({ "installed": true, "path": wf, "summary": tr!(
                "已写入 {}\n提交并推送后生效。它读每个提交上的盖章（各工具写了几行）；仓库根目录有 credits.json 时优先用它。{next}",
                "Wrote {}\nCommit and push it to turn it on. It reads the stamps on each commit (lines per tool), or credits.json at the repo root if there is one.{next}",
                wf.display()
            ) }))
        }
        Some("uninstall") => {
            Integrations::record(repo, "release", false)?;
            if !release_installed(repo) {
                return Ok(json!({ "installed": false, "path": wf, "summary": tr!("没有接入，不用删除", "not on; nothing to remove") }));
            }
            std::fs::remove_file(&wf).map_err(|e| e.to_string())?;
            Ok(json!({ "installed": false, "path": wf, "summary": tr!("已删除 {}（提交后生效）", "removed {} (commit to apply)", wf.display()) }))
        }
        _ => Err(tr!("用法：creditia release install|uninstall|status", "usage: creditia release install|uninstall|status")),
    }
}

/// 两项接入的现状（App 的开关读这个）
fn cmd_integrations(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let d = Integrations::of(&repo);
    if hook_installed(&repo) {
        upgrade_hook(&repo);
    }
    let github = ad::git::git(&repo, &["remote", "-v"]).contains("github.com");
    let v = json!({
        "hook": { "installed": hook_installed(&repo), "decided": d.hook, "path": hook_path(&repo)? },
        "release": { "installed": release_installed(&repo), "decided": d.release, "path": workflow_path(&repo), "github": github, "published": repo.join("credits.json").exists() },
    });
    let on = |b: bool| if b { tr!("开", "on") } else { tr!("关", "off") };
    let summary = tr!("提交盖章：{}\n跟着发版：{}", "Commit stamps: {}\nRelease credits: {}", on(hook_installed(&repo)), on(release_installed(&repo)));
    let mut v = v;
    v["summary"] = json!(summary);
    Ok((v, vec![]))
}

/// init / analyze 之后还没决定的接入列在 ask 里：--json（agent、App）交给调用方问用户，终端里由 ask_integrations 直接问
fn mark_ask(repo: &Path, v: &mut Value) {
    let d = Integrations::of(repo);
    let todo: Vec<&str> = [("hook", d.hook), ("release", d.release)].into_iter().filter(|(_, x)| x.is_none()).map(|(k, _)| k).collect();
    if !todo.is_empty() {
        v["ask"] = json!(todo);
        v["askRepo"] = json!(repo);
    }
}

/// 在终端里逐项问（结果先打出来，再问）。不是终端就不问
fn ask_integrations(repo: &Path, todo: &[Value]) {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }
    eprintln!("{}", tr!(
        "\n这个项目还有两项可以接入，你来决定（以后随时能改：App 里每个项目都有开关，或者 creditia hook / creditia release）。",
        "\nTwo optional add-ons for this project — your call (change it any time: each project has switches in the App, or use creditia hook / creditia release)."
    ));
    for k in todo.iter().filter_map(Value::as_str) {
        let q = if k == "hook" {
            tr!(
                "\n· 提交盖章：在 git 的 prepare-commit-msg hook 里加一行。在 agent 里提交时，提交信息末尾自动加上 Creditia-Harness / Creditia-Model；有 AI 代码时再加一行 Creditia-Lines（各工具行数，不含代码）。写入 Git 提交，无需依赖本地日志，PR 评论和 Release 说明也读它。只影响这个仓库。\n  装吗？[y/N] ",
                "\n· Commit stamps: adds one line to git's prepare-commit-msg hook. Commits made inside an agent get Creditia-Harness / Creditia-Model trailers, and commits with AI code get a Creditia-Lines trailer (lines per tool, no code). Recorded in Git commits without relying on local logs; PR comments and release notes read them too. Only this repository.\n  Install? [y/N] "
            )
        } else {
            tr!(
                "\n· 跟着发版：写 .github/workflows/creditia.yml（要你提交）。发 Release 时把这一版的署名写进说明，PR 上评论这个 PR 的署名。读的是提交盖章记下的行数，或仓库里的 credits.json。\n  加吗？[y/N] ",
                "\n· Release credits: writes .github/workflows/creditia.yml (you commit it). Published releases get their credits in the notes; pull requests get a comment with theirs. It reads the line counts from commit stamps, or credits.json in the repo.\n  Add it? [y/N] "
            )
        };
        eprint!("{q}");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return;
        }
        let yes = matches!(line.trim().to_lowercase().as_str(), "y" | "yes" | "是" | "好");
        let r = if k == "hook" { hook_set(repo, Some(if yes { "install" } else { "uninstall" })) } else { release_set(repo, Some(if yes { "install" } else { "uninstall" })) };
        match r {
            Ok(r) if yes => eprintln!("  {}", r["summary"].as_str().unwrap_or("").replace('\n', "\n  ")),
            Ok(_) => eprintln!("  {}", tr!("不装。之后想要：creditia {} install", "Skipped. To add it later: creditia {} install", k)),
            Err(e) => eprintln!("  {e}"),
        }
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
    // Antigravity（agy 命令行和编辑器）给它跑的命令设 ANTIGRAVITY_AGENT=1 和 ANTIGRAVITY_TRAJECTORY_ID
    if has("ANTIGRAVITY_AGENT") || has("ANTIGRAVITY_TRAJECTORY_ID") {
        return Some(("antigravity".into(), None));
    }
    if has("GEMINI_CLI") {
        return Some(("gemini".into(), env.get("GEMINI_MODEL").cloned()));
    }
    if has("QWEN_CODE") {
        return Some(("qwen".into(), None));
    }
    if has("OPENCODE") || has("OPENCODE_SESSION") {
        return Some(("opencode".into(), None));
    }
    // Crush 给它跑的命令设 CRUSH=1、AI_AGENT=crush
    if has("CRUSH") || env.get("AI_AGENT").map(|v| v == "crush").unwrap_or(false) {
        return Some(("crush".into(), None));
    }
    if has("AIDER_MODEL") || has("AIDER_CHAT_HISTORY_FILE") {
        return Some(("aider".into(), env.get("AIDER_MODEL").cloned()));
    }
    None
}

fn cmd_hook_trailers(a: &Args) -> R<(Value, Vec<String>)> {
    let msgfile = a.pos.first().ok_or_else(|| tr!("需要提交信息文件路径", "the commit message file path is required"))?;
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
    let msg = std::fs::read_to_string(msgfile).map_err(|e| e.to_string())?;
    // 这次提交里各工具写了几行：本机日志里有证据的才算。CI 里没有日志，PR 评论和 Release 说明读这一行。
    // 扫日志最多等几秒，等不到就不写，不耽误提交
    let lines = if msg.contains("Creditia-Lines:") { None } else { staged_lines(&repo) };
    let harness = harness.filter(|_| !msg.contains("Creditia-Harness:"));
    if harness.is_none() && lines.is_none() {
        return Ok((json!({ "added": false, "summary": "" }), vec![]));
    }
    // 在注释行之前插入 trailer（git 会在 # 开头的注释后截断）
    let (body, comments) = split_comments(&msg);
    let mut out = body.trim_end().to_string();
    out.push_str(if ends_with_trailers(&out) { "\n" } else { "\n\n" });
    if let Some(h) = &harness {
        out.push_str(&format!("Creditia-Harness: {h}\n"));
        if let Some(m) = &model {
            out.push_str(&format!("Creditia-Model: {m}\n"));
        }
        if let Some(s) = &share {
            out.push_str(&format!("Creditia-Share: {s}\n"));
        }
    }
    if let Some(l) = &lines {
        out.push_str(&format!("Creditia-Lines: {l}\n"));
    }
    if !comments.is_empty() {
        out.push('\n');
        out.push_str(&comments);
    }
    std::fs::write(msgfile, out).map_err(|e| e.to_string())?;
    Ok((json!({ "added": true, "harness": harness, "model": model, "lines": lines, "summary": "" }), vec![]))
}

/// `Creditia-Lines` 的值：`工具/模型 行数, 工具 行数 / 暂存区新增总行数`。没有 AI 写的行、或超时，返回 None
fn staged_lines(repo: &Path) -> Option<String> {
    let secs = std::env::var("CREDITIA_HOOK_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(8u64);
    if secs == 0 {
        return None;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let repo = repo.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(analyze::staged(&repo));
    });
    let (by, total) = rx.recv_timeout(std::time::Duration::from_secs(secs)).ok()?.ok()?;
    if by.is_empty() {
        return None;
    }
    Some(lines_value(&by, total))
}

fn lines_value(by: &[(String, usize)], total: usize) -> String {
    format!("{} / {total}", by.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "))
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
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new));
    let mut lib = Library::open()?;
    let proj = analyze::project_of(&repo, &cfg, Some(&lib));
    match a.pos.first().map(String::as_str) {
        Some("export") => {
            let (evs, sessions) = lib.load_evidence(&proj.id)?;
            let v = json!({ "creditia": "evidence", "version": 1, "project": proj.id, "name": proj.name, "rootCommit": proj.root_commit, "evidence": evs, "sessions": sessions });
            let out = a.flags.get("out").map(PathBuf::from).unwrap_or(PathBuf::from(format!("creditia-evidence-{}.json", proj.id)));
            std::fs::write(&out, serde_json::to_string(&v).unwrap()).map_err(|e| e.to_string())?;
            Ok((json!({ "path": out, "evidence": evs.len(), "sessions": sessions.len(), "summary": tr!("已导出 {} 条证据、{} 个会话到 {}（只含路径、行哈希和元数据）", "exported {} pieces of evidence and {} sessions to {} (paths, line hashes and metadata only)", evs.len(), sessions.len(), out.display()) }), vec![]))
        }
        Some("import") => {
            let file = a.pos.get(1).ok_or_else(|| tr!("用法：creditia-core evidence import <文件>", "usage: creditia-core evidence import <file>"))?;
            let v: Value = serde_json::from_str(&std::fs::read_to_string(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if v["creditia"] != "evidence" {
                return Err(tr!("不是 creditia 的证据文件", "not a creditia evidence file"));
            }
            if v["rootCommit"].as_str().map(|r| !proj.root_commit.is_empty() && r != proj.root_commit).unwrap_or(false) {
                return Err(tr!("这份证据属于另一个仓库（根提交不同）", "this evidence belongs to another repository (different root commit)"));
            }
            let evs: Vec<creditia_core::model::Ev> = serde_json::from_value(v["evidence"].clone()).map_err(|e| e.to_string())?;
            let sessions: Vec<creditia_core::model::Session> = serde_json::from_value(v["sessions"].clone()).unwrap_or_default();
            let mut store = creditia_core::model::Store { version: creditia_core::model::STORE_VERSION, ..Default::default() };
            store.groups.insert("import".into(), creditia_core::model::FileGroup { fp: (0, 0), evs, sessions });
            lib.upsert_project(&proj)?;
            let n = lib.save_evidence(&proj.id, &store)?;
            Ok((json!({ "imported": n, "summary": tr!("合并了 {n} 条新证据；下次 analyze 会用上", "merged {n} new pieces of evidence; the next analyze will use them") }), vec![]))
        }
        _ => Err(tr!("用法：creditia-core evidence export [--out F] | import <文件>", "usage: creditia-core evidence export [--out F] | import <file>")),
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
    let summary = ps.iter().map(|p| format!("{:<28} {}  {}", p.name, p.last_analyzed.map(creditia_core::util::date_cn).unwrap_or_else(|| tr!("未分析", "not analyzed")), p.path)).collect::<Vec<_>>().join("\n");
    Ok((json!({ "projects": list, "summary": if summary.is_empty() { tr!("本地项目记录为空；先 creditia-core analyze", "the library is empty; run creditia-core analyze first") } else { summary } }), vec![]))
}

/// 作品库里每个项目最新的档案（仓库里的 .creditia/credits.json，读不到用作品库里的快照）：跨项目汇总用
fn cmd_all_credits(_a: &Args) -> R<(Value, Vec<String>)> {
    let lib = Library::open()?;
    let mut list = vec![];
    for p in lib.projects()? {
        let live = std::fs::read_to_string(Path::new(&p.path).join(".creditia/credits.json")).ok();
        let text = match live {
            Some(t) => Some(t),
            None => lib.snapshot_json(&p.id)?,
        };
        if let Some(v) = text.and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            list.push(json!({ "id": p.id, "name": p.name, "credits": v }));
        }
    }
    let n = list.len();
    Ok((json!({ "projects": list, "summary": tr!("{n} 个项目有档案", "{n} projects have credits") }), vec![]))
}

// ───────────────────────── 路径：一个项目在多处 ─────────────────────────

fn cmd_paths(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new));
    let lib = Library::open()?;
    let proj = analyze::project_of(&repo, &cfg, Some(&lib));
    let mut list: Vec<Value> = lib.paths(&proj.id)?.into_iter().map(|(p, k)| json!({ "path": p, "kind": k, "exists": Path::new(&p).is_dir() })).collect();
    for p in &cfg.paths {
        list.push(json!({ "path": p, "kind": "config", "exists": Path::new(p).is_dir() }));
    }
    let summary = list.iter().map(|v| format!("{:<9} {}{}", v["kind"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or(""), if v["exists"] == true { String::new() } else { tr!("  （目录已不在）", "  (folder no longer exists)") })).collect::<Vec<_>>().join("\n");
    Ok((json!({ "project": proj.id, "paths": list, "summary": if summary.is_empty() { tr!("还没有登记过路径；先 analyze", "no paths recorded yet; run analyze first") } else { summary } }), vec![]))
}

fn cmd_link(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let target = a.pos.first().ok_or_else(|| tr!("用法：creditia-core link <路径>", "usage: creditia-core link <path>"))?;
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new));
    let lib = Library::open()?;
    let proj = analyze::project_of(&repo, &cfg, Some(&lib));
    lib.upsert_project(&proj)?;
    if a.cmd == "unlink" {
        let n = lib.remove_path(&proj.id, target)?;
        return Ok((json!({ "removed": n, "summary": if n > 0 { tr!("已取消关联 {target}", "unlinked {target}") } else { tr!("{target} 本来就没有关联", "{target} was not linked") } }), vec![]));
    }
    // 目录还在的话，确认它真是同一个仓库（根提交一致）
    let p = Path::new(target);
    if p.is_dir() && ad::git::git_ok(p, &["rev-parse", "--git-dir"]) {
        let (root, _) = ad::git::identity(p);
        if !root.is_empty() && !proj.root_commit.is_empty() && root != proj.root_commit {
            return Err(tr!("{target} 是另一个仓库（根提交不同），没有关联", "{target} is another repository (different root commit); not linked"));
        }
    }
    lib.add_path(&proj.id, target.trim_end_matches('/'), "manual")?;
    Ok((json!({ "linked": target, "project": proj.id, "summary": tr!("已把 {target} 关联到 {}；下次 analyze 起，日志里这个路径下的改动也算本项目", "linked {target} to {}; from the next analyze, changes under this path in the logs count for this project", proj.name) }), vec![]))
}

fn cmd_whoami(_a: &Args) -> R<(Value, Vec<String>)> {
    let env: HashMap<String, String> = std::env::vars().collect();
    match detect_agent(&env) {
        Some((h, m)) => Ok((json!({ "agent": h, "model": m, "summary": format!("{h}{}", m.as_deref().map(|m| format!(" · {m}")).unwrap_or_default()) }), vec![])),
        None => Ok((json!({ "agent": null, "summary": tr!("没有检测到 agent（可能是人在终端里直接运行）", "no agent detected (probably run directly in a terminal)") }), vec![])),
    }
}

#[allow(dead_code)]
fn _src(_: Src) {}


#[cfg(test)]
mod hook_tests {
    use super::{detect_agent, ends_with_trailers};
    use std::collections::HashMap;

    #[test]
    fn detects_antigravity() {
        let env: HashMap<String, String> = [("ANTIGRAVITY_AGENT".to_string(), "1".to_string())].into();
        assert_eq!(detect_agent(&env), Some(("antigravity".to_string(), None)));
        let env: HashMap<String, String> = [("ANTIGRAVITY_TRAJECTORY_ID".to_string(), "t1".to_string()), ("GEMINI_CLI".to_string(), "1".to_string())].into();
        assert_eq!(detect_agent(&env).map(|(h, _)| h), Some("antigravity".to_string()));
    }

    #[test]
    fn conventional_subject_is_not_a_trailer_block() {
        assert!(!ends_with_trailers("fix: 模型名归一"));
        assert!(!ends_with_trailers("feat: x\n\n说明: 这是正文"));
        assert!(ends_with_trailers("feat: x\n\nSigned-off-by: a <a@b.c>\nCo-authored-by: b <b@c.d>"));
    }
}
