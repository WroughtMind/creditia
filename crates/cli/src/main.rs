//! creditia-core：Rust 核心的命令行入口。所有命令非交互，支持 `--json`。
//! 退出码：0 成功，1 失败，2 部分成功（有警告）。

mod analyze;

use creditia_adapters as ad;
use creditia_core::config::{Config, Director};
use creditia_core::model::Src;
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
        let p = p.canonicalize().map_err(|_| format!("目录不存在：{}", p.display()))?;
        if !ad::git::git_ok(&p, &["rev-parse", "--git-dir"]) {
            return Err(format!("不是 git 仓库：{}", p.display()));
        }
        // worktree 映射回主仓库
        Ok(ad::git::main_worktree(&p).unwrap_or(p))
    }
}

const USAGE: &str = "用法：creditia-core <命令> [选项]

  discover                         列出本机检测到 agent 活动的项目
  init      [--repo P] [--force]   在仓库写 .creditia/config.json
  analyze   [--repo P] [--fresh] [--github] [--level strict|standard|loose] [--out F] [--config F]
                                   分析并写 .creditia/credits.json（全程本地，不调用任何大模型）
  update    [--repo P]             增量更新（同 analyze）
  collect   [--repo P | --all]     只采集证据进证据库
  doctor                           检查各数据源
  blame     <路径>:<行号> [--repo P]  这一行是谁写的
  stamp     --harness H [--model M] [--share S]   给下一次提交盖章
  hook      install|uninstall [--repo P]           安装提交盖章 hook
  hook-trailers <msgfile>          （hook 调用）追加 Creditia-* trailer
  evidence  export [--out F] | import <文件>   多台电脑之间合并证据库
  projects                         作品库里的项目
  paths     [--repo P]             本项目登记过的全部路径（主仓库、worktree、临时克隆、旧路径）
  link      <路径> [--repo P]      把一个路径关联到本项目（别的电脑上的路径、已删除的临时目录）
  unlink    <路径> [--repo P]      取消关联
  whoami                           当前在哪个 agent 里运行（agent 给自己盖章用）

所有命令支持 --json。退出码：0 成功，1 失败，2 有警告。";

fn main() {
    let a = parse_args();
    let json = a.on("json");
    let r: R<(Value, Vec<String>)> = match a.cmd.as_str() {
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
        "paths" => cmd_paths(&a),
        "link" | "unlink" => cmd_link(&a),
        "whoami" => cmd_whoami(&a),
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

fn cmd_discover(a: &Args) -> R<(Value, Vec<String>)> {
    let mut acts: Vec<ad::Activity> = vec![];
    acts.extend(ad::codex::discover());
    acts.extend(ad::claude::discover());
    acts.extend(ad::cursor::discover());
    acts.extend(ad::grok::discover());
    acts.extend(ad::kimi::discover());
    acts.extend(ad::gemini::discover());
    acts.extend(ad::opencode::discover());
    acts.extend(ad::cline::discover());
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
    merged.sort_by(|a, b| b.last.cmp(&a.last));
    // 已经不存在、又只有零星活动的临时目录默认不列（--all 列出）
    let show_all = a.on("all");
    merged.retain(|p| show_all || p.exists || p.tools.values().sum::<u64>() >= 3);
    let status = |p: &P| if p.exists { p.path.clone() } else if PathBuf::from(&p.path).is_dir() { format!("{}  (不是 git 仓库)", p.path) } else { format!("{}  (目录已不存在)", p.path) };
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
    let mut c = Config::default();
    c.repo = repo.display().to_string();
    c.name = project_name(repo);
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
    let mut clusters: HashMap<usize, (usize, Vec<String>, HashMap<String, usize>)> = HashMap::new();
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
    // 把仓库里见过的 agent 邮箱补进映射：邮箱用户名里带工具名的（codex-acceptance@、cursoragent@…）按工具归类，[bot] 归 bot
    for l in log.lines() {
        if let Some((_, e)) = l.split_once('\0') {
            let e = e.to_lowercase();
            if c.agent_emails.contains_key(&e) || c.bot_emails.contains(&e) {
                continue;
            }
            let local = e.split('@').next().unwrap_or("");
            let h = if e.contains("[bot]") {
                c.bot_emails.push(e.clone());
                continue;
            } else if local.contains("codex") || e.ends_with("@openai.com") {
                "codex"
            } else if local.contains("cursor") || e.ends_with("@cursor.com") {
                "cursor"
            } else if local.contains("claude") || e.ends_with("@anthropic.com") {
                "claude-code"
            } else if local.contains("copilot") {
                "github-copilot"
            } else if local.contains("devin") {
                "devin"
            } else if local.contains("kimi") {
                "kimi"
            } else if local.contains("grok") {
                "grok"
            } else if local.contains("gemini") || local.contains("jules") {
                "gemini"
            } else {
                continue;
            };
            c.agent_emails.insert(e, h.into());
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
    if path.exists() && !a.on("force") {
        return Err(format!("{} 已存在；加 --force 覆盖", path.display()));
    }
    let c = detect_config(&repo);
    c.save(&path)?;
    let v = serde_json::to_value(&c).unwrap();
    let summary = format!(
        "已写入 {}\n项目 {} · 分支 {} · 你的提交身份 {}\n可在该文件里调整你的邮箱、口径（level）、其他路径（paths）、隐私选项。",
        path.display(),
        c.name,
        c.branch,
        c.directors.first().map(|d| d.name.as_str()).unwrap_or("（未识别）")
    );
    Ok((json!({ "path": path, "config": v, "summary": summary }), vec![]))
}

// ───────────────────────── analyze / update ─────────────────────────

fn cmd_analyze(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let invoked = std::env::current_dir().ok().and_then(|p| p.canonicalize().ok());
    let opts = analyze::Opts { fresh: a.on("fresh"), github: a.on("github"), out: a.flags.get("out").map(PathBuf::from), blame: None, config: a.flags.get("config").map(PathBuf::from), level: a.flags.get("level").cloned(), invoked };
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
    use creditia_core::model::Src;
    let hs = vec![
        ad::codex::doctor(), ad::claude::doctor(), ad::cursor::doctor(), ad::grok::doctor(), ad::kimi::doctor(),
        ad::gemini::doctor(Src::Gemini), ad::gemini::doctor(Src::Qwen), ad::opencode::doctor(),
        ad::cline::doctor(Src::Cline), ad::cline::doctor(Src::Roo), ad::cline::doctor(Src::Kilo),
    ];
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
            if h.readable { "可读" } else { "不可读" },
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
    // 没装的工具不算问题；装了却读不了才警告
    let warnings: Vec<String> = hs.iter().filter(|h| !h.readable && std::path::Path::new(&h.path).exists()).map(|h| format!("{} 不可读：{}", h.source, h.path)).collect();
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
    let opts = analyze::Opts { fresh: false, github: false, out: None, blame: Some((rel.clone(), line)), config: a.flags.get("config").map(PathBuf::from), level: a.flags.get("level").cloned(), invoked: None };
    let r = analyze::run(&repo, &opts)?;
    let b = r.blame.ok_or(format!("没找到 {rel}:{line}"))?;
    let who = b["harness"].as_str().map(|h| h.to_string()).or(b["name"].as_str().map(String::from)).unwrap_or("未确认".into());
    let summary = format!(
        "{rel}:{line}\n作者  {who}{}\n提交  {} {}\n{}证据  {}",
        b["model"].as_str().map(|m| format!("  {m}")).unwrap_or_default(),
        b["commit"].as_str().unwrap_or(""),
        b["date"].as_str().unwrap_or(""),
        b["unit"].as_str().map(|u| format!("项    {u}  {}\n", b["unitTitle"].as_str().unwrap_or(""))).unwrap_or_default(),
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
    if has("GEMINI_CLI") {
        return Some(("gemini".into(), env.get("GEMINI_MODEL").cloned()));
    }
    if has("QWEN_CODE") {
        return Some(("qwen".into(), None));
    }
    if has("OPENCODE") || has("OPENCODE_SESSION") {
        return Some(("opencode".into(), None));
    }
    if has("AIDER_MODEL") || has("AIDER_CHAT_HISTORY_FILE") {
        return Some(("aider".into(), env.get("AIDER_MODEL").cloned()));
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
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new));
    let mut lib = Library::open()?;
    let proj = analyze::project_of(&repo, &cfg, Some(&lib));
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
    let summary = list.iter().map(|v| format!("{:<9} {}{}", v["kind"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or(""), if v["exists"] == true { "" } else { "  （目录已不在）" })).collect::<Vec<_>>().join("\n");
    Ok((json!({ "project": proj.id, "paths": list, "summary": if summary.is_empty() { "还没有登记过路径；先 analyze".into() } else { summary } }), vec![]))
}

fn cmd_link(a: &Args) -> R<(Value, Vec<String>)> {
    let repo = a.repo()?;
    let target = a.pos.first().ok_or("用法：creditia-core link <路径>")?;
    let cfg = analyze::load_config(&repo, a.flags.get("config").map(Path::new));
    let lib = Library::open()?;
    let proj = analyze::project_of(&repo, &cfg, Some(&lib));
    lib.upsert_project(&proj)?;
    if a.cmd == "unlink" {
        let n = lib.remove_path(&proj.id, target)?;
        return Ok((json!({ "removed": n, "summary": if n > 0 { format!("已取消关联 {target}") } else { format!("{target} 本来就没有关联") } }), vec![]));
    }
    // 目录还在的话，确认它真是同一个仓库（根提交一致）
    let p = Path::new(target);
    if p.is_dir() && ad::git::git_ok(p, &["rev-parse", "--git-dir"]) {
        let (root, _) = ad::git::identity(p);
        if !root.is_empty() && !proj.root_commit.is_empty() && root != proj.root_commit {
            return Err(format!("{target} 是另一个仓库（根提交不同），没有关联"));
        }
    }
    lib.add_path(&proj.id, target.trim_end_matches('/'), "manual")?;
    Ok((json!({ "linked": target, "project": proj.id, "summary": format!("已把 {target} 关联到 {}；下次 analyze 起，日志里这个路径下的改动也算本项目", proj.name) }), vec![]))
}

fn cmd_whoami(_a: &Args) -> R<(Value, Vec<String>)> {
    let env: HashMap<String, String> = std::env::vars().collect();
    match detect_agent(&env) {
        Some((h, m)) => Ok((json!({ "agent": h, "model": m, "summary": format!("{h}{}", m.as_deref().map(|m| format!(" · {m}")).unwrap_or_default()) }), vec![])),
        None => Ok((json!({ "agent": null, "summary": "没有检测到 agent（可能是人在终端里直接运行）" }), vec![])),
    }
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
