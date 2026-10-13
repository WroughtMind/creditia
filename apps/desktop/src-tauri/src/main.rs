//! creditia App 的本机桥：项目列表、档案、分析、保存导出文件、数据源体检。
//! 分析交给同一个 Rust 核心二进制（creditia-core），App 不调用任何大模型。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// 给界面的错误：只带代码和原始细节，文案由界面按当前语言写
#[derive(serde::Serialize, Debug)]
struct Fail {
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}
impl Fail {
    fn new(code: &'static str) -> Self {
        Fail { code, detail: None }
    }
    fn with(code: &'static str, detail: impl ToString) -> Self {
        Fail { code, detail: Some(detail.to_string()) }
    }
}
/// 作品库（creditia-store）报的错
impl From<String> for Fail {
    fn from(e: String) -> Self {
        Fail::with("store", e)
    }
}
type Res<T> = Result<T, Fail>;

fn project(lib: &Library, project_id: &str) -> Res<creditia_store::Project> {
    lib.projects()?.into_iter().find(|p| p.id == project_id).ok_or(Fail::new("not_in_library"))
}

#[tauri::command]
fn projects() -> Res<Value> {
    let lib = Library::open()?;
    let list: Vec<Value> = lib
        .projects()?
        .into_iter()
        .map(|p| {
            let credits = PathBuf::from(&p.path).join(".creditia/credits.json");
            let paths: Vec<String> = lib.paths(&p.id).unwrap_or_default().into_iter().map(|(x, _)| x).collect();
            json!({
                "id": p.id, "name": p.name, "path": p.path, "remotes": p.remotes, "paths": paths,
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
#[tauri::command]
fn credits(project_id: String) -> Res<Value> {
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
#[tauri::command]
fn stamp(project_id: String) -> Res<String> {
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

#[tauri::command]
fn config_of(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Ok(json!({})) };
    let path = PathBuf::from(&p.path).join(".creditia/config.json");
    Ok(std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({})))
}

#[tauri::command]
fn set_display(project_id: String, named: bool, sign: String) -> Res<()> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    Ok(creditia_store::Library::set_display(std::path::Path::new(&p.path), named, &sign)?)
}

/// 核心二进制：和 App 一起打包（sidecar 同目录），开发时用 PATH 里的
fn core_bin() -> PathBuf {
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
fn core_cmd(args: &[&str]) -> Command {
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

fn run_core(args: &[&str]) -> Res<Value> {
    let out = core_cmd(args).output().map_err(|e| Fail::with("core_missing", e))?;
    core_result(out.status, &out.stdout, &out.stderr)
}

/// 核心的退出码：0 成功，2 成功但有警告，1 出错（JSON 里带 error）
fn core_result(status: std::process::ExitStatus, stdout: &[u8], stderr: &[u8]) -> Res<Value> {
    let v: Value = serde_json::from_slice(stdout).map_err(|_| Fail::with("core_failed", String::from_utf8_lossy(stderr).trim()))?;
    if let Some(e) = v["error"].as_str() {
        return Err(Fail::with("core_failed", e));
    }
    if !matches!(status.code(), Some(0) | Some(2)) {
        return Err(Fail::with("core_failed", String::from_utf8_lossy(stderr).trim()));
    }
    Ok(v)
}

/// 正在跑的分析：同一时间只跑一个，界面可以取消
static RUNNING: Mutex<Option<Child>> = Mutex::new(None);
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 管道各开一条线程读完，免得写满卡住核心
fn drain<R: std::io::Read + Send + 'static>(r: Option<R>) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    r.map(|mut r| {
        std::thread::spawn(move || {
            let mut b = vec![];
            let _ = r.read_to_end(&mut b);
            b
        })
    })
}

fn run_analysis(repo: &str) -> Res<Value> {
    let (out, err) = {
        let mut slot = RUNNING.lock().unwrap();
        if slot.is_some() {
            return Err(Fail::new("busy"));
        }
        let mut c = core_cmd(&["analyze", "--repo", repo]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| Fail::with("core_missing", e))?;
        CANCELLED.store(false, std::sync::atomic::Ordering::SeqCst);
        let pipes = (drain(c.stdout.take()), drain(c.stderr.take()));
        *slot = Some(c);
        pipes
    };
    // 等的是核心进程退出，不是管道关上：取消时核心起的子进程可能还拿着管道
    let status = loop {
        {
            let mut slot = RUNNING.lock().unwrap();
            match slot.as_mut().map(|c| c.try_wait()) {
                Some(Ok(None)) => {}
                Some(r) => {
                    *slot = None;
                    break r.map_err(|e| Fail::with("core_failed", e))?.unwrap();
                }
                None => return Err(Fail::new("cancelled")),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    if CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(Fail::new("cancelled"));
    }
    let stdout = out.and_then(|t| t.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|t| t.join().ok()).unwrap_or_default();
    core_result(status, &stdout, &stderr)
}

#[tauri::command]
fn cancel_analyze() {
    if let Some(c) = RUNNING.lock().unwrap().as_mut() {
        CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
        // 连同核心起的 git 一起停。直接调系统调用：有的系统上 `kill -KILL -<pgid>` 会被解析错、杀到自己
        #[cfg(unix)]
        if let Ok(pgid) = i32::try_from(c.id()) {
            if pgid > 1 {
                // SAFETY: 只是发信号；核心用 process_group(0) 启动，进程组号就是它的 pid
                unsafe { libc::kill(-pgid, libc::SIGKILL) };
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &c.id().to_string()]).creation_flags(0x0800_0000).status();
        }
        let _ = c.kill();
    }
}

/// 重新分析一个项目（在它的仓库目录里跑核心）
#[tauri::command]
async fn analyze(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    if !PathBuf::from(&p.path).is_dir() {
        return Err(Fail::with("dir_missing", p.path));
    }
    tauri::async_runtime::spawn_blocking(move || run_analysis(&p.path)).await.map_err(|e| Fail::with("core_failed", e))?
}

/// 选一个文件夹（只选，不分析；分析交给 add_project，界面这期间能显示进度）
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let Some(dir) = app.dialog().file().blocking_pick_folder() else { return Ok(None) };
    Ok(Some(dir.into_path().map_err(|e| Fail::with("io", e))?.display().to_string()))
}

/// 把一个文件夹加进作品库：先分析一遍
#[tauri::command]
async fn add_project(path: String) -> Res<Value> {
    let p = path.clone();
    let v = tauri::async_runtime::spawn_blocking(move || run_analysis(&p)).await.map_err(|e| Fail::with("core_failed", e))??;
    let lib = Library::open()?;
    let path = PathBuf::from(path);
    let repo = path.canonicalize().unwrap_or(path);
    let id = lib.projects()?.into_iter().find(|x| std::path::Path::new(&x.path) == repo).map(|x| x.id).unwrap_or_default();
    Ok(json!({ "id": id, "result": v }))
}

#[tauri::command]
fn embed_snippet(project_id: String, piece: String, theme: String) -> Res<String> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let rel = PathBuf::from(&p.path).join(".creditia/credits.json");
    let src = if rel.exists() { "/.creditia/credits.json" } else { "credits.json" };
    Ok(format!("<script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js\"></script>\n<creditia-card src=\"{src}\" piece=\"{piece}\" theme=\"{theme}\"></creditia-card>\n"))
}

#[tauri::command]
fn library_path() -> String {
    creditia_store::data_dir().display().to_string()
}

fn home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    { std::env::var_os("USERPROFILE").map(PathBuf::from) }
    #[cfg(not(target_os = "windows"))]
    { std::env::var_os("HOME").map(PathBuf::from) }
}

// ───────────────────────── App 自己的设置（作品库目录里的 app.json） ─────────────────────────
// 导出目录只能经系统的选文件夹对话框改，界面传不进任意路径

fn app_settings_path() -> PathBuf {
    creditia_store::data_dir().join("app.json")
}
fn app_settings() -> Value {
    std::fs::read_to_string(app_settings_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}))
}
fn write_app_settings(v: &Value) -> Res<()> {
    let p = app_settings_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| Fail::with("io", e))?;
    }
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap()).map_err(|e| Fail::with("io", e))
}
fn default_export_dir() -> Res<PathBuf> {
    Ok(home().ok_or(Fail::new("no_home"))?.join("Downloads"))
}
/// 用户选过、而且还在的导出目录；否则「下载」
fn export_dir() -> Res<PathBuf> {
    match app_settings()["exportDir"].as_str().map(PathBuf::from) {
        Some(d) if d.is_dir() => Ok(d),
        _ => default_export_dir(),
    }
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> Res<Value> {
    let dir = export_dir()?;
    Ok(json!({
        "version": app.package_info().version.to_string(),
        "library": creditia_store::data_dir().display().to_string(),
        "exportDir": dir.display().to_string(),
        "exportDefault": dir == default_export_dir()?,
    }))
}

/// 弹出选文件夹对话框改导出目录；取消就返回 null
#[tauri::command]
async fn choose_export_dir(app: tauri::AppHandle) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let Some(dir) = app.dialog().file().set_directory(export_dir()?).blocking_pick_folder() else { return Ok(None) };
    let dir = dir.into_path().map_err(|e| Fail::with("io", e))?;
    let mut v = app_settings();
    v["exportDir"] = json!(dir.display().to_string());
    write_app_settings(&v)?;
    Ok(Some(dir.display().to_string()))
}

#[tauri::command]
fn reset_export_dir() -> Res<String> {
    let mut v = app_settings();
    if let Some(o) = v.as_object_mut() {
        o.remove("exportDir");
    }
    write_app_settings(&v)?;
    Ok(default_export_dir()?.display().to_string())
}

/// 打开导出目录或作品库目录（只认这两个名字）
#[tauri::command]
fn open_folder(which: String) -> Res<()> {
    let dir = match which.as_str() {
        "exports" => export_dir()?,
        "library" => creditia_store::data_dir(),
        _ => return Err(Fail::new("io")),
    };
    std::fs::create_dir_all(&dir).map_err(|e| Fail::with("io", e))?;
    open_path(dir.display().to_string())
}

/// 导出写进导出目录（默认「下载」）；重名自动加序号
#[tauri::command]
fn save_file(name: String, bytes: String, mime: Option<String>) -> Res<String> {
    let _ = mime;
    let dir = export_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| Fail::with("io", e))?;
    let safe = safe_name(&name);
    let mut path = dir.join(&safe);
    if path.exists() {
        let stem = std::path::Path::new(&safe).file_stem().and_then(|s| s.to_str()).unwrap_or("export").to_string();
        let ext = std::path::Path::new(&safe).extension().and_then(|s| s.to_str()).map(|s| s.to_string());
        for i in 2..1000 {
            let cand = match &ext { Some(e) => dir.join(format!("{stem}-{i}.{e}")), None => dir.join(format!("{stem}-{i}")) };
            if !cand.exists() { path = cand; break; }
        }
    }
    let data = base64::engine::general_purpose::STANDARD.decode(bytes).map_err(|e| Fail::with("io", e))?;
    std::fs::write(&path, data).map_err(|e| Fail::with("io", e))?;
    SAVED.lock().unwrap().insert(path.clone());
    Ok(path.display().to_string())
}

/// 文件名只留一个干净的名字：不带目录、不带各系统不认的字符、不以点开头或结尾
fn safe_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let s = s.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if s.is_empty() { "export".into() } else { s.to_string() }
}

/// 这次打开 App 以来存过的文件：「在访达中显示」只认这些
static SAVED: Mutex<std::collections::BTreeSet<PathBuf>> = Mutex::new(std::collections::BTreeSet::new());

#[tauri::command]
fn reveal(path: String) -> Res<()> {
    let p = PathBuf::from(&path);
    if !SAVED.lock().unwrap().contains(&p) {
        return Err(Fail::new("not_allowed"));
    }
    #[cfg(target_os = "macos")]
    { let _ = Command::new("open").args(["-R", path.as_str()]).status(); }
    #[cfg(target_os = "windows")]
    { let _ = Command::new("explorer").arg(format!("/select,{}", path)).status(); }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = if p.is_dir() { p.clone() } else { p.parent().map(|x| x.to_path_buf()).unwrap_or(p.clone()) };
        let _ = Command::new("xdg-open").arg(dir).status();
    }
    Ok(())
}

/// 只给 open_folder 用，不直接开放给页面
fn open_path(path: String) -> Res<()> {
    #[cfg(target_os = "macos")]
    { let _ = Command::new("open").arg(path.as_str()).status(); }
    #[cfg(target_os = "windows")]
    { let _ = Command::new("cmd").args(["/c", "start", "", path.as_str()]).status(); }
    #[cfg(all(unix, not(target_os = "macos")))]
    { let _ = Command::new("xdg-open").arg(path.as_str()).status(); }
    Ok(())
}

/// 数据源体检：交给核心（和 CLI 的 creditia doctor 完全一致）；核心不在时退回本地探测。
/// 只给数字和状态，文字由界面按语言写
#[tauri::command]
fn doctor() -> Value {
    if let Ok(v) = run_core(&["doctor"]) {
        if let Some(a) = v["sources"].as_array() {
            return json!(a.iter().map(|h| json!({ "id": h["source"], "path": h["path"], "ok": h["readable"], "records": h["records"] })).collect::<Vec<_>>());
        }
    }
    doctor_local()
}

fn doctor_local() -> Value {
    let h = home().unwrap_or_default();
    let env_dir = |k: &str, d: &str| std::env::var_os(k).map(PathBuf::from).unwrap_or_else(|| h.join(d));
    let mut out: Vec<Value> = vec![];
    let mut add = |id: &str, path: PathBuf, marker: &str| {
        let probe = path.join(marker);
        let ok = probe.exists();
        let entries = if ok && probe.is_dir() { std::fs::read_dir(&probe).map(|d| d.count()).ok() } else { None };
        out.push(json!({ "id": id, "path": path.display().to_string(), "ok": ok, "entries": entries }));
    };
    add("codex", env_dir("CODEX_HOME", ".codex"), "state_5.sqlite");
    add("claude-code", env_dir("CLAUDE_CONFIG_DIR", ".claude"), "projects");
    add("cursor", env_dir("CREDITIA_CURSOR_DIR", ".cursor"), "ai-tracking/ai-code-tracking.db");
    add("grok", env_dir("CREDITIA_GROK_DIR", ".grok"), "sessions");
    let kimi = std::env::var_os("CREDITIA_KIMI_DIR").map(PathBuf::from).unwrap_or_else(|| {
        let a = h.join(".kimi-code");
        if a.exists() { a } else { h.join(".kimi") }
    });
    add("kimi", kimi, "sessions");
    json!(out)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            projects, credits, stamp, config_of, set_display, analyze, cancel_analyze, pick_folder, add_project, embed_snippet,
            library_path, app_info, choose_export_dir, reset_export_dir, open_folder, save_file, reveal, doctor
        ])
        .run(tauri::generate_context!())
        .expect("启动 creditia App 失败");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// 假的核心：打印一段 JSON 或报错，按参数决定睡多久
    fn fake_core(dir: &std::path::Path, body: &str) -> PathBuf {
        let p = dir.join("creditia-core");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn export_names_stay_in_the_folder() {
        assert_eq!(safe_name("lumen-share-ink.png"), "lumen-share-ink.png");
        assert_eq!(safe_name("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(safe_name(".."), "export");
        assert_eq!(safe_name("a:b*c?.svg"), "a-b-c-.svg");
        assert_eq!(safe_name(" .hidden. "), "hidden");
    }

    #[test]
    fn analysis_runs_reports_and_cancels() {
        let dir = std::env::temp_dir().join(format!("creditia-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // 成功（有警告时退出码 2 也算成功）
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"echo '{"ok":true}'; exit 2"#));
        assert_eq!(run_analysis("/tmp").unwrap()["ok"], true);

        // 核心报错：带上它的原话
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"echo '{"error":"not a git repo"}'; exit 1"#));
        let e = run_analysis("/tmp").unwrap_err();
        assert_eq!((e.code, e.detail.as_deref()), ("core_failed", Some("not a git repo")));

        // 跑着的时候不能再开第二个；取消之后返回 cancelled，槽位也空出来
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, "sleep 30; echo '{}'"));
        let t = std::thread::spawn(|| run_analysis("/tmp"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(run_analysis("/tmp").unwrap_err().code, "busy");
        let t0 = std::time::Instant::now();
        cancel_analyze();
        assert_eq!(t.join().unwrap().unwrap_err().code, "cancelled");
        assert!(t0.elapsed() < std::time::Duration::from_secs(5));
        assert!(RUNNING.lock().unwrap().is_none());

        // 找不到核心
        std::env::set_var("CREDITIA_CORE", dir.join("nope"));
        assert_eq!(run_analysis("/tmp").unwrap_err().code, "core_missing");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
