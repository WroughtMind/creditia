//! Shared desktop operations used by both the Tauri and GPUI frontends.
//!
//! These functions are blocking. Run discovery, analysis and Git operations on a
//! background executor. The existing Rust CLI remains the analysis worker, and
//! existing project/config/library formats and machine-readable errors stay intact.
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

mod analysis;
mod settings;
pub mod collection;
mod background;
mod projects;
pub use projects::{project_archives, project_details, set_project_config, blame, evidence_archive, import_evidence, doctor};
pub use background::BackgroundService;
pub use collection::{collect_info, collect_now, collect_round, set_collect, set_collect_agent};
pub use settings::{app_settings, edit_app_settings, ui_preferences, patch_ui_preferences, import_legacy_preferences, set_legacy_preference, legacy_preferences, migrate_legacy_with_helper};
pub use analysis::{analysis_running, cancel_analyze, run_analysis, run_analysis_with_cancel, run_core_job_with_cancel, analysis_status, recover_interrupted_analysis, resume_analysis, AnalysisCancellation, ProgressSink};

/// 给界面的错误：只带代码和原始细节，文案由界面按当前语言写
#[derive(serde::Serialize, Debug)]
pub struct Fail {
    pub code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
impl Fail {
    pub fn new(code: &'static str) -> Self {
        Fail { code, detail: None }
    }
    pub fn with(code: &'static str, detail: impl ToString) -> Self {
        Fail { code, detail: Some(detail.to_string()) }
    }
}
/// 作品库（creditia-store）报的错
impl From<String> for Fail {
    fn from(e: String) -> Self {
        Fail::with("store", e)
    }
}
pub type Res<T> = Result<T, Fail>;

pub fn project(lib: &Library, project_id: &str) -> Res<creditia_store::Project> {
    lib.projects()?.into_iter().find(|p| p.id == project_id).ok_or(Fail::new("not_in_library"))
}

fn path_kinds(lib: &Library, p: &creditia_store::Project) -> std::collections::BTreeMap<String, String> {
    let mut kinds: std::collections::BTreeMap<String, String> = lib.paths(&p.id).unwrap_or_default().into_iter().collect();
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(&p.path).args(["-c", "core.quotepath=false", "worktree", "list", "--porcelain"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    if let Ok(out) = cmd.output() {
        if out.status.success() {
            for path in String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.strip_prefix("worktree ")) {
                kinds.insert(path.to_string(), "worktree".into());
            }
        }
    }
    let cfg: Value = std::fs::read_to_string(PathBuf::from(&p.path).join(".creditia/config.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    for path in cfg["paths"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        kinds.insert(path.to_string(), "config".into());
    }
    kinds
}

pub fn projects() -> Res<Value> {
    let lib = Library::open()?;
    let list: Vec<Value> = lib
        .projects()?
        .into_iter()
        .map(|p| {
            let credits = PathBuf::from(&p.path).join(".creditia/credits.json");
            let path_kinds = path_kinds(&lib, &p);
            let paths: Vec<&String> = path_kinds.keys().collect();
            json!({
                "id": p.id, "name": p.name, "path": p.path, "remotes": p.remotes, "paths": paths, "pathKinds": path_kinds,
                "lastHead": p.last_head,
                "lastAnalyzed": p.last_analyzed,
                "evidence": lib.evidence_count(&p.id),
                "exists": PathBuf::from(&p.path).is_dir(),
                "hasCredits": credits.exists(),
            })
        })
        .collect();
    Ok(json!(list))
}

/// 优先读仓库里的档案（最新），没有就用作品库快照
pub fn credits(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    if let Ok(p) = project(&lib, &project_id) {
        let live = PathBuf::from(&p.path).join(".creditia/credits.json");
        if let Ok(t) = std::fs::read_to_string(&live) {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                return Ok(v);
            }
        }
        if let Some(t) = lib.snapshot_json(&project_id)? {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                return Ok(v);
            }
        }
    }
    Err(Fail::new("no_credits"))
}

/// 档案的「版本」：仓库里 credits.json 的修改时间 + 作品库里最近一次分析的时间。
/// 界面回到前台、或者每隔一会儿比一比，变了就原地换成新的
pub fn stamp(project_id: String) -> Res<String> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let live = PathBuf::from(&p.path).join(".creditia/credits.json");
    let modified = std::fs::metadata(live)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(format!("{modified}:{}", p.last_analyzed.unwrap_or(0)))
}

/// 仓库现在的提交（档案里记的 head 和它不一样，说明有了新提交）。分析的分支在 .creditia/config.json 的 branch 里，没写就是 HEAD
pub fn head(project_id: String) -> Res<String> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let cfg: Value = std::fs::read_to_string(PathBuf::from(&p.path).join(".creditia/config.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let rev = |r: &str| -> String {
        let mut c = Command::new("git");
        c.arg("-C").arg(&p.path).args(["rev-parse", "--verify", "-q", r]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        c.output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
    };
    let h = cfg["branch"].as_str().map(rev).filter(|h| !h.is_empty()).unwrap_or_else(|| rev("HEAD"));
    Ok(h)
}

pub fn config_of(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Ok(json!({})) };
    let path = PathBuf::from(&p.path).join(".creditia/config.json");
    Ok(std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({})))
}

pub fn set_display(project_id: String, hide_names: bool, sign: String) -> Res<()> {
    set_project_config(project_id, json!({"display":{"hideNames":hide_names,"sign":sign}}))?;
    Ok(())
}

/// 核心二进制：和 App 一起打包（sidecar 同目录），开发时用 PATH 里的
pub fn core_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CREDITIA_CORE") {
        return PathBuf::from(p);
    }
    let exe = if cfg!(windows) { "creditia-core.exe" } else { "creditia-core" };
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        let cand = dir.join(exe);
        if cand.exists() {
            return cand;
        }
    }
    PathBuf::from(exe)
}

/// 跑核心的命令：Windows 上不弹控制台窗口；Unix 上自成一个进程组，取消时连它起的 git 一起停
pub fn core_cmd(args: &[&str]) -> Command {
    let mut c = Command::new(core_bin());
    c.args(args).arg("--json");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c
}

pub fn run_core(args: &[&str]) -> Res<Value> {
    let out = core_cmd(args).output().map_err(|e| Fail::with("core_missing", e))?;
    core_result(out.status, &out.stdout, &out.stderr)
}

/// 核心的退出码：0 成功，2 成功但有警告，1 出错（JSON 里带 error）
pub fn core_result(status: std::process::ExitStatus, stdout: &[u8], stderr: &[u8]) -> Res<Value> {
    let v: Value = serde_json::from_slice(stdout).map_err(|_| Fail::with("core_failed", String::from_utf8_lossy(stderr).trim()))?;
    if let Some(e) = v["error"].as_str() {
        return Err(Fail::with("core_failed", e));
    }
    if !matches!(status.code(), Some(0) | Some(2)) {
        return Err(Fail::with("core_failed", String::from_utf8_lossy(stderr).trim()));
    }
    Ok(v)
}

/// 两项接入（提交盖章 hook、跟着发版的 workflow）的现状和用户的决定
pub fn integrations(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    if !PathBuf::from(&p.path).is_dir() {
        return Err(Fail::with("dir_missing", p.path));
    }
    run_core(&["integrations", "--repo", p.path.as_str()])
}

/// 开或关一项：交给核心的 hook / release install|uninstall，它会把决定记进项目的 .creditia/config.json
pub fn set_integration(project_id: String, which: String, on: bool, lang: Option<String>) -> Res<Value> {
    if which != "hook" && which != "release" {
        return Err(Fail::with("switch_failed", which));
    }
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let lang = lang.unwrap_or_else(|| "zh".into());
    run_core(&[which.as_str(), if on { "install" } else { "uninstall" }, "--repo", p.path.as_str(), "--lang", lang.as_str()]).map_err(|e| Fail { code: "switch_failed", detail: e.detail })
}

/// 从作品库里移除一个项目。只删项目这一行：证据、会话、路径、快照和缓存都留着，以后加回同一个仓库就直接接上
pub fn remove_project(project_id: String) -> Res<()> {
    Library::open()?.remove_project(&project_id)?;
    Ok(())
}

/// 关联或取消关联一个路径（搬过家的旧目录、别处的克隆）：交给核心的 link / unlink，它会确认是同一个仓库
pub fn link_path(project_id: String, path: String, unlink: Option<bool>, lang: Option<String>) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let lang = lang.unwrap_or_else(|| "zh".into());
    let cmd = if unlink.unwrap_or(false) { "unlink" } else { "link" };
    run_core(&[cmd, path.as_str(), "--repo", p.path.as_str(), "--lang", lang.as_str()])
}

pub fn embed_snippet(project_id: String, piece: String, theme: String) -> Res<String> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let rel = PathBuf::from(&p.path).join(".creditia/credits.json");
    let src = if rel.exists() { "/.creditia/credits.json" } else { "credits.json" };
    Ok(format!("<script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js\"></script>\n<creditia-card src=\"{src}\" piece=\"{piece}\" theme=\"{theme}\"></creditia-card>\n"))
}

pub fn library_path() -> String {
    creditia_store::data_dir().display().to_string()
}

pub fn home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    { std::env::var_os("USERPROFILE").map(PathBuf::from) }
    #[cfg(not(target_os = "windows"))]
    { std::env::var_os("HOME").map(PathBuf::from) }
}

// ───────────────────────── App 自己的设置（作品库目录里的 app.json） ─────────────────────────
// 导出目录只能经系统的选文件夹对话框改，界面传不进任意路径

pub fn default_export_dir() -> Res<PathBuf> {
    Ok(home().ok_or(Fail::new("no_home"))?.join("Downloads"))
}
/// 用户选过、而且还在的导出目录；否则「下载」
pub fn export_dir() -> Res<PathBuf> {
    match app_settings()["exportDir"].as_str().map(PathBuf::from) {
        Some(d) if d.is_dir() => Ok(d),
        _ => default_export_dir(),
    }
}


impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{}: {}", self.code, detail),
            None => f.write_str(self.code),
        }
    }
}
impl std::error::Error for Fail {}

/// Discover local projects through the same worker used by the CLI.
pub fn discover() -> Res<Value> {
    run_core(&["discover"])
}

pub fn analyze(project_id: String, lang: Option<String>, progress: Option<ProgressSink>) -> Res<Value> {
    analyze_with_cancel(project_id, lang, progress, AnalysisCancellation::default())
}

pub fn analyze_with_cancel(project_id: String, lang: Option<String>, progress: Option<ProgressSink>, cancellation: AnalysisCancellation) -> Res<Value> {
    let p = project(&Library::open()?, &project_id)?;
    if !PathBuf::from(&p.path).is_dir() {
        return Err(Fail::with("dir_missing", p.path));
    }
    run_analysis_with_cancel(&p.path, lang.as_deref(), progress, cancellation)
}

pub fn add_project(path: String, lang: Option<String>, progress: Option<ProgressSink>) -> Res<Value> {
    add_project_with_cancel(path, lang, progress, AnalysisCancellation::default())
}

pub fn add_project_with_cancel(path: String, lang: Option<String>, progress: Option<ProgressSink>, cancellation: AnalysisCancellation) -> Res<Value> {
    let result = run_analysis_with_cancel(&path, lang.as_deref(), progress, cancellation)?;
    let lib = Library::open()?;
    let path = PathBuf::from(path);
    let repo = path.canonicalize().unwrap_or(path);
    let id = lib.projects()?.into_iter().find(|p| std::path::Path::new(&p.path) == repo).map(|p| p.id).unwrap_or_default();
    Ok(json!({ "id": id, "result": result }))
}

pub fn reset_export_dir() -> Res<String> {
    edit_app_settings(|v| {
        if let Some(o) = v.as_object_mut() { o.remove("exportDir"); }
    })?;
    Ok(default_export_dir()?.display().to_string())
}
