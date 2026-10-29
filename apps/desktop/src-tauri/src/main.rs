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

#[tauri::command]
fn projects() -> Res<Value> {
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

/// 仓库现在的提交（档案里记的 head 和它不一样，说明有了新提交）。分析的分支在 .creditia/config.json 的 branch 里，没写就是 HEAD
#[tauri::command]
fn head(project_id: String) -> Res<String> {
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

#[tauri::command]
fn config_of(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Ok(json!({})) };
    let path = PathBuf::from(&p.path).join(".creditia/config.json");
    Ok(std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({})))
}

#[tauri::command]
fn set_display(project_id: String, hide_names: bool, sign: String) -> Res<()> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    Ok(creditia_store::Library::set_display(std::path::Path::new(&p.path), hide_names, &sign)?)
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

/// stderr 一行一行读：`@progress {…}` 转成给界面的事件，其余的攒起来（出错时当细节）
fn drain_err<R: std::io::Read + Send + 'static>(r: Option<R>, app: Option<tauri::AppHandle>) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    use std::io::BufRead;
    use tauri::Emitter;
    r.map(|r| {
        std::thread::spawn(move || {
            let mut rest = vec![];
            for line in std::io::BufReader::new(r).split(b'\n').map_while(Result::ok) {
                match (line.strip_prefix(b"@progress "), &app) {
                    (Some(p), Some(app)) => {
                        if let Ok(v) = serde_json::from_slice::<Value>(p) {
                            let _ = app.emit("analyze-progress", v);
                        }
                    }
                    (Some(_), None) => {}
                    _ => {
                        rest.extend_from_slice(&line);
                        rest.push(b'\n');
                    }
                }
            }
            rest
        })
    })
}

/// lang：界面当前的语言（zh / en），核心的提醒按它写；app 在时把进度转给界面
fn run_analysis(repo: &str, lang: Option<&str>, app: Option<tauri::AppHandle>) -> Res<Value> {
    let (out, err) = {
        let mut slot = RUNNING.lock().unwrap();
        if slot.is_some() {
            return Err(Fail::new("busy"));
        }
        let mut cmd = core_cmd(&["analyze", "--repo", repo, "--github"]);
        if let Some(l) = lang {
            cmd.env("CREDITIA_LANG", l);
        }
        cmd.env("CREDITIA_PROGRESS", "1");
        let mut c = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| Fail::with("core_missing", e))?;
        CANCELLED.store(false, std::sync::atomic::Ordering::SeqCst);
        let pipes = (drain(c.stdout.take()), drain_err(c.stderr.take(), app));
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
async fn analyze(app: tauri::AppHandle, project_id: String, lang: Option<String>) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    if !PathBuf::from(&p.path).is_dir() {
        return Err(Fail::with("dir_missing", p.path));
    }
    tauri::async_runtime::spawn_blocking(move || run_analysis(&p.path, lang.as_deref(), Some(app))).await.map_err(|e| Fail::with("core_failed", e))?
}

/// 本机用过 AI 工具的项目（核心的 discover）：第一次打开时挑项目用。要扫各工具的记录，放到后台线程
#[tauri::command]
async fn discover() -> Res<Value> {
    tauri::async_runtime::spawn_blocking(|| run_core(&["discover"])).await.map_err(|e| Fail::with("core_failed", e))?
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
async fn add_project(app: tauri::AppHandle, path: String, lang: Option<String>) -> Res<Value> {
    let p = path.clone();
    let v = tauri::async_runtime::spawn_blocking(move || run_analysis(&p, lang.as_deref(), Some(app))).await.map_err(|e| Fail::with("core_failed", e))??;
    let lib = Library::open()?;
    let path = PathBuf::from(path);
    let repo = path.canonicalize().unwrap_or(path);
    let id = lib.projects()?.into_iter().find(|x| std::path::Path::new(&x.path) == repo).map(|x| x.id).unwrap_or_default();
    Ok(json!({ "id": id, "result": v }))
}

/// 两项接入（提交盖章 hook、跟着发版的 workflow）的现状和用户的决定
#[tauri::command]
fn integrations(project_id: String) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    if !PathBuf::from(&p.path).is_dir() {
        return Err(Fail::with("dir_missing", p.path));
    }
    run_core(&["integrations", "--repo", p.path.as_str()])
}

/// 开或关一项：交给核心的 hook / release install|uninstall，它会把决定记进项目的 .creditia/config.json
#[tauri::command]
fn set_integration(project_id: String, which: String, on: bool, lang: Option<String>) -> Res<Value> {
    if which != "hook" && which != "release" {
        return Err(Fail::with("switch_failed", which));
    }
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let lang = lang.unwrap_or_else(|| "zh".into());
    run_core(&[which.as_str(), if on { "install" } else { "uninstall" }, "--repo", p.path.as_str(), "--lang", lang.as_str()]).map_err(|e| Fail { code: "switch_failed", detail: e.detail })
}

/// 从作品库里移除一个项目。只删项目这一行：证据、会话、路径、快照和缓存都留着，以后加回同一个仓库就直接接上
#[tauri::command]
fn remove_project(project_id: String) -> Res<()> {
    Library::open()?.remove_project(&project_id)?;
    Ok(())
}

/// 关联或取消关联一个路径（搬过家的旧目录、别处的克隆）：交给核心的 link / unlink，它会确认是同一个仓库
#[tauri::command]
fn link_path(project_id: String, path: String, unlink: Option<bool>, lang: Option<String>) -> Res<Value> {
    let lib = Library::open()?;
    let p = project(&lib, &project_id)?;
    let lang = lang.unwrap_or_else(|| "zh".into());
    let cmd = if unlink.unwrap_or(false) { "unlink" } else { "link" };
    run_core(&[cmd, path.as_str(), "--repo", p.path.as_str(), "--lang", lang.as_str()])
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
/// 读改写一次设置：后台采集也会写，两边不互相盖掉
fn edit_app_settings(f: impl FnOnce(&mut Value)) -> Res<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut v = app_settings();
    f(&mut v);
    let p = app_settings_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| Fail::with("io", e))?;
    }
    std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| Fail::with("io", e))
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
    edit_app_settings(|v| v["exportDir"] = json!(dir.display().to_string()))?;
    Ok(Some(dir.display().to_string()))
}

#[tauri::command]
fn reset_export_dir() -> Res<String> {
    edit_app_settings(|v| {
        if let Some(o) = v.as_object_mut() {
            o.remove("exportDir");
        }
    })?;
    Ok(default_export_dir()?.display().to_string())
}

// ───────────────────────── 定时采集证据 ─────────────────────────
// 各工具的日志会被清理（有的只留几十天）。App 开着时每小时把作品库里每个项目的新证据收进作品库：
// 只采集，不分析，不改仓库。设置在 app.json 的 collect 里：on（默认开）、last（上一轮的结果）

const COLLECT_EVERY: i64 = 3600;
/// 同一时间只跑一轮（定时的和「现在采集」不叠在一起）
static COLLECTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn collect_on() -> bool {
    app_settings()["collect"]["on"].as_bool().unwrap_or(true)
}

/// 跑一轮：一个项目一个核心进程，有分析在跑就等它跑完再接着收。定时的一轮在关着时不跑，中途被关掉就停下
fn collect_round(timed: bool) -> Res<Value> {
    use std::sync::atomic::Ordering;
    if timed && !collect_on() {
        return Err(Fail::new("off"));
    }
    if COLLECTING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(Fail::new("busy"));
    }
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            COLLECTING.store(false, Ordering::SeqCst);
        }
    }
    let _done = Done;
    let repos: Vec<String> = Library::open()?.projects()?.into_iter().map(|p| p.path).filter(|p| PathBuf::from(p).is_dir()).collect();
    let (mut ok, mut failed, mut new) = (0, 0, 0);
    for repo in repos {
        while RUNNING.lock().unwrap().is_some() {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        if timed && !collect_on() {
            break;
        }
        match run_core(&["collect", "--repo", repo.as_str()]) {
            Ok(v) => {
                ok += 1;
                new += v["collected"].as_array().map(|a| a.iter().filter_map(|c| c["newEvidence"].as_u64()).sum::<u64>()).unwrap_or(0);
            }
            Err(_) => failed += 1,
        }
    }
    let last = json!({ "at": now(), "projects": ok, "failed": failed, "newEvidence": new });
    edit_app_settings(|v| v["collect"]["last"] = last.clone())?;
    Ok(last)
}

/// 后台线程：每分钟看一眼，离上一轮满一小时就收一轮。按 app.json 里记的时间算，重开 App、电脑睡醒都接得上
fn collect_loop(app: tauri::AppHandle) {
    use tauri::Emitter;
    std::thread::sleep(std::time::Duration::from_secs(30)); // 先让开启动
    loop {
        let last = last_collect()["at"].as_i64().unwrap_or(0);
        if collect_on() && now() - last >= COLLECT_EVERY {
            if let Ok(v) = collect_round(true) {
                let _ = app.emit("collect-done", v);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

/// 最近一轮：App 自己收的和后台任务收的，取晚的那个。后台的带 by: agent
fn last_collect() -> Value {
    let app = app_settings()["collect"]["last"].clone();
    let agent = agent_last();
    match (app["at"].as_i64(), agent["at"].as_i64()) {
        (Some(a), Some(b)) if b <= a => app,
        (_, Some(_)) => agent,
        _ => app,
    }
}

#[tauri::command]
fn collect_info() -> Value {
    json!({
        "on": collect_on(), "last": last_collect(), "running": COLLECTING.load(std::sync::atomic::Ordering::SeqCst), "every": COLLECT_EVERY,
        "agent": { "supported": cfg!(target_os = "macos"), "on": agent_installed() },
    })
}

#[tauri::command]
fn set_collect(on: bool) -> Res<Value> {
    edit_app_settings(|v| v["collect"]["on"] = json!(on))?;
    Ok(collect_info())
}

#[tauri::command]
async fn collect_now() -> Res<Value> {
    tauri::async_runtime::spawn_blocking(|| collect_round(false)).await.map_err(|e| Fail::with("core_failed", e))?
}

// ───────────────────────── App 关着时也收：macOS 的 launchd 后台任务 ─────────────────────────
// 装在 ~/Library/LaunchAgents，登录后每小时跑一次 App 里带的核心 `collect --all`，结果写进作品库目录的 collect-agent.json。
// App 被删掉、核心不在了，任务就什么也不做

const AGENT_LABEL: &str = "dev.creditia.collect";
/// $0 是核心，$1 是作品库目录。先写临时文件再改名，App 读到的总是完整的一份
const AGENT_SCRIPT: &str = r#"[ -x "$0" ] || exit 0; mkdir -p "$1" || exit 0; "$0" collect --all --json > "$1/collect-agent.json.tmp" 2>/dev/null; mv -f "$1/collect-agent.json.tmp" "$1/collect-agent.json""#;

fn agent_out() -> PathBuf {
    creditia_store::data_dir().join("collect-agent.json")
}

/// 后台任务上一轮的结果：时间取文件的修改时间，数字从核心的输出里数
fn agent_last() -> Value {
    let p = agent_out();
    let Some(at) = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) else { return Value::Null };
    let v: Value = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let c = v["collected"].as_array().cloned().unwrap_or_default();
    let failed = c.iter().filter(|x| x["error"].is_string()).count();
    let new: u64 = c.iter().filter_map(|x| x["newEvidence"].as_u64()).sum();
    json!({ "at": at.as_secs() as i64, "projects": c.len() - failed, "failed": failed, "newEvidence": new, "by": "agent" })
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// launchd 任务的 plist。launchd 的 PATH 很短，补上 Homebrew 的位置，核心才找得到 git
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn agent_plist(core: &std::path::Path, lib: &std::path::Path, creditia_home: Option<&str>) -> String {
    let home = creditia_home.map(|h| format!("\n    <key>CREDITIA_HOME</key><string>{}</string>", xml(h))).unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- creditia App 装的：App 关着时也每小时把各项目的新记录收进作品库。在 App 的设置里关掉就会删掉 -->
<plist version="1.0">
<dict>
  <key>Label</key><string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>-c</string>
    <string>{}</string>
    <string>{}</string>
    <string>{}</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>{home}
  </dict>
  <key>StartInterval</key><integer>{COLLECT_EVERY}</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
</dict>
</plist>
"#,
        xml(AGENT_SCRIPT),
        xml(&core.display().to_string()),
        xml(&lib.display().to_string()),
    )
}

fn agent_plist_path() -> Option<PathBuf> {
    Some(home()?.join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist")))
}

fn agent_installed() -> bool {
    cfg!(target_os = "macos") && agent_plist_path().is_some_and(|p| p.exists())
}

/// 现在这份 App 该装的 plist：核心要是绝对路径（开发时从 PATH 找到的不算）
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn agent_wanted() -> Res<String> {
    let core = core_bin();
    if !core.is_absolute() || !core.exists() {
        return Err(Fail::with("core_missing", core.display()));
    }
    Ok(agent_plist(&core, &creditia_store::data_dir(), std::env::var("CREDITIA_HOME").ok().filter(|h| !h.is_empty()).as_deref()))
}

#[cfg(target_os = "macos")]
fn launchctl(args: &[&str]) -> Res<()> {
    let o = Command::new("/bin/launchctl").args(args).output().map_err(|e| Fail::with("agent_failed", e))?;
    if o.status.success() { Ok(()) } else { Err(Fail::with("agent_failed", String::from_utf8_lossy(&o.stderr).trim())) }
}

#[cfg(target_os = "macos")]
fn agent_install() -> Res<()> {
    let body = agent_wanted()?;
    let p = agent_plist_path().ok_or(Fail::new("no_home"))?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| Fail::with("io", e))?;
    // SAFETY: 只读当前用户的 uid
    let domain = format!("gui/{}", unsafe { libc::getuid() });
    let _ = launchctl(&["bootout", &domain, &p.display().to_string()]); // 装过旧的就先卸，没装过会报错，不管
    std::fs::write(&p, body).map_err(|e| Fail::with("io", e))?;
    launchctl(&["bootstrap", &domain, &p.display().to_string()])
}

#[cfg(target_os = "macos")]
fn agent_uninstall() -> Res<()> {
    let Some(p) = agent_plist_path() else { return Ok(()) };
    if p.exists() {
        // SAFETY: 只读当前用户的 uid
        let _ = launchctl(&["bootout", &format!("gui/{}", unsafe { libc::getuid() }), &p.display().to_string()]);
        std::fs::remove_file(&p).map_err(|e| Fail::with("io", e))?;
    }
    Ok(())
}

/// App 换了位置或升级后核心路径变了：装着的任务跟着改。开发时不动
#[cfg(target_os = "macos")]
fn agent_refresh() {
    if cfg!(debug_assertions) || !agent_installed() {
        return;
    }
    let cur = agent_plist_path().and_then(|p| std::fs::read_to_string(p).ok());
    if let Ok(want) = agent_wanted() {
        if cur.as_deref() != Some(want.as_str()) {
            let _ = agent_install();
        }
    }
}

#[tauri::command]
fn set_collect_agent(on: bool) -> Res<Value> {
    #[cfg(target_os = "macos")]
    {
        if on { agent_install()? } else { agent_uninstall()? }
        Ok(collect_info())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = on;
        Err(Fail::new("unsupported"))
    }
}

/// 打开导出目录、作品库目录，或者作品库里一个项目的仓库目录（只认这三种）
#[tauri::command]
fn open_folder(which: String, project_id: Option<String>) -> Res<()> {
    let dir = match which.as_str() {
        "exports" => export_dir()?,
        "library" => creditia_store::data_dir(),
        "project" => {
            let p = project(&Library::open()?, project_id.as_deref().unwrap_or_default())?;
            let dir = PathBuf::from(&p.path);
            if !dir.is_dir() {
                return Err(Fail::with("dir_missing", p.path));
            }
            return open_path(dir.display().to_string());
        }
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
            return json!(a.iter().map(|h| json!({ "id": h["source"], "path": h["path"], "ok": h["readable"], "records": h["records"], "scope": h["scope"], "unsupported": h["unsupported"].as_bool().unwrap_or(false) })).collect::<Vec<_>>());
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

// ───────────────────────── 标志：设置里选，程序坞图标跟着换 ─────────────────────────
// 默认星号使用安装包的分层图标；其他标志在 App 运行时替换为静态图。

const DEFAULT_LOGO: &str = "star";

fn icon_png(id: &str) -> Option<&'static [u8]> {
    match id {
        "star" => Some(include_bytes!("../icons/icon.png")),
        "star-mono" => Some(include_bytes!("../icons/dock/star-mono.png")),
        "orb" => Some(include_bytes!("../icons/dock/orb.png")),
        "ring" => Some(include_bytes!("../icons/dock/ring.png")),
        "color" => Some(include_bytes!("../icons/dock/color.png")),
        "mono" => Some(include_bytes!("../icons/dock/mono.png")),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn apply_icon(app: &tauri::AppHandle, id: &str) {
    let Some(png) = icon_png(id) else { return };
    let restore_bundle_icon = id == DEFAULT_LOGO;
    let _ = app.run_on_main_thread(move || {
        use objc2::AnyThread;
        use objc2_app_kit::{NSApplication, NSImage};
        let Some(mtm) = objc2::MainThreadMarker::new() else { return };
        if restore_bundle_icon {
            // nil 恢复系统从 Assets.car 读取的图标，不用 PNG 覆盖其分层与外观变体。
            unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(None) };
            return;
        }
        let data = objc2_foundation::NSData::with_bytes(png);
        if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
            // SAFETY: 在主线程上，传的是刚解码好的 NSImage
            unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&img)) };
        }
    });
}
#[cfg(not(target_os = "macos"))]
fn apply_icon(_app: &tauri::AppHandle, _id: &str) {}

#[tauri::command]
fn set_logo(app: tauri::AppHandle, id: String) -> Res<Value> {
    if icon_png(&id).is_none() {
        return Err(Fail::new("unsupported"));
    }
    edit_app_settings(|v| v["logo"] = json!(id))?;
    apply_icon(&app, &id);
    Ok(json!({ "logo": id }))
}

#[tauri::command]
fn logo() -> Value {
    json!(app_settings()["logo"].as_str().unwrap_or(DEFAULT_LOGO))
}

mod preview;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            agent_refresh();
            if let Some(id) = app_settings()["logo"].as_str() {
                if id != DEFAULT_LOGO {
                    apply_icon(app.handle(), id);
                }
            }
            let h = app.handle().clone();
            std::thread::spawn(move || collect_loop(h));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            projects, credits, stamp, head, config_of, set_display, analyze, cancel_analyze, discover, pick_folder, add_project, integrations, set_integration, remove_project, link_path, embed_snippet,
            library_path, app_info, choose_export_dir, reset_export_dir, open_folder, save_file, reveal, doctor, collect_info, set_collect, collect_now, set_collect_agent, logo, set_logo, preview::render_preview
        ])
        .run(tauri::generate_context!())
        .expect("启动 creditia App 失败");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// 测试都改 CREDITIA_CORE / CREDITIA_HOME 这类进程级的环境变量：一个跑完再跑下一个
    static ENV: Mutex<()> = Mutex::new(());

    /// 假的核心：打印一段 JSON 或报错，按参数决定睡多久
    fn fake_core(dir: &std::path::Path, body: &str) -> PathBuf {
        let p = dir.join("creditia-core");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn export_names_stay_in_the_folder() {
        assert_eq!(safe_name("catnap-share-ink.png"), "catnap-share-ink.png");
        assert_eq!(safe_name("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(safe_name(".."), "export");
        assert_eq!(safe_name("a:b*c?.svg"), "a-b-c-.svg");
        assert_eq!(safe_name(" .hidden. "), "hidden");
    }

    #[test]
    fn analysis_runs_reports_and_cancels() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("creditia-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // 成功（有警告时退出码 2 也算成功）
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"case " $* " in *" --github "*) ;; *) echo '{"error":"missing GitHub enrichment"}'; exit 1;; esac
echo '{"ok":true}'; exit 2"#));
        assert_eq!(run_analysis("/tmp", None, None).unwrap()["ok"], true);

        // 核心报错：带上它的原话
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"echo '{"error":"not a git repo"}'; exit 1"#));
        let e = run_analysis("/tmp", None, None).unwrap_err();
        assert_eq!((e.code, e.detail.as_deref()), ("core_failed", Some("not a git repo")));

        // 跑着的时候不能再开第二个；取消之后返回 cancelled，槽位也空出来
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, "sleep 30; echo '{}'"));
        let t = std::thread::spawn(|| run_analysis("/tmp", None, None));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(run_analysis("/tmp", None, None).unwrap_err().code, "busy");
        let t0 = std::time::Instant::now();
        cancel_analyze();
        assert_eq!(t.join().unwrap().unwrap_err().code, "cancelled");
        assert!(t0.elapsed() < std::time::Duration::from_secs(5));
        assert!(RUNNING.lock().unwrap().is_none());

        // 找不到核心
        std::env::set_var("CREDITIA_CORE", dir.join("nope"));
        assert_eq!(run_analysis("/tmp", None, None).unwrap_err().code, "core_missing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_rounds_cover_the_library() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("creditia-app-collect-{}", std::process::id()));
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::env::set_var("CREDITIA_HOME", dir.join("lib"));
        let lib = Library::open().unwrap();
        let proj = |id: &str, path: &std::path::Path| creditia_store::Project { id: id.into(), name: id.into(), path: path.display().to_string(), root_commit: id.into(), remotes: vec![], last_head: None, last_analyzed: None };
        lib.upsert_project(&proj("a", &repo)).unwrap();
        lib.upsert_project(&proj("gone", &dir.join("deleted"))).unwrap();
        drop(lib);

        // 默认开着；目录已经不在的项目跳过；有警告（退出码 2）也算收到了
        assert!(collect_info()["on"].as_bool().unwrap());
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"echo '{"collected":[{"newEvidence":3}]}'; exit 2"#));
        let r = collect_round(false).unwrap();
        assert_eq!((r["projects"].as_u64(), r["failed"].as_u64(), r["newEvidence"].as_u64()), (Some(1), Some(0), Some(3)));
        assert_eq!(collect_info()["last"]["newEvidence"], 3);

        // 核心出错记成失败，上一轮的结果被这一轮换掉
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"echo '{"error":"x"}'; exit 1"#));
        assert_eq!(collect_round(false).unwrap()["failed"], 1);
        assert_eq!(collect_info()["last"]["newEvidence"], 0);

        // 一轮没跑完不能再开一轮
        std::env::set_var("CREDITIA_CORE", fake_core(&dir, r#"sleep 1; echo '{"collected":[]}'"#));
        let t = std::thread::spawn(|| collect_round(false));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(collect_info()["running"].as_bool().unwrap());
        assert_eq!(collect_round(false).unwrap_err().code, "busy");
        t.join().unwrap().unwrap();
        assert!(!collect_info()["running"].as_bool().unwrap());

        // 关掉以后定时的一轮不收，手动的照收；设置里别的键不受影响
        edit_app_settings(|v| v["exportDir"] = json!("/x")).unwrap();
        assert!(!set_collect(false).unwrap()["on"].as_bool().unwrap());
        assert_eq!(collect_round(true).unwrap_err().code, "off");
        assert_eq!(collect_info()["last"]["projects"], 1, "没跑的那轮不覆盖上一轮");
        assert_eq!(collect_round(false).unwrap()["projects"], 1);
        assert_eq!(app_settings()["exportDir"], "/x");

        std::env::remove_var("CREDITIA_HOME");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn background_agent_writes_what_the_app_reads() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("creditia-app-agent-{}", std::process::id()));
        let lib = dir.join("lib dir");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CREDITIA_HOME", &lib);
        let core = fake_core(&dir, r#"[ "$1 $2 $3" = "collect --all --json" ] || exit 9; echo '{"collected":[{"newEvidence":5},{"newEvidence":2},{"error":"x"}]}'; exit 2"#);
        let sh = |core: &std::path::Path| Command::new("/bin/sh").arg("-c").arg(AGENT_SCRIPT).arg(core).arg(&lib).status().unwrap();

        // 后台没跑过：用 App 自己那轮
        edit_app_settings(|v| v["collect"]["last"] = json!({ "at": 1, "projects": 1, "failed": 0, "newEvidence": 0 })).unwrap();
        assert_eq!(last_collect()["at"], 1);

        // 跑一次：退出码 2 也写下结果，App 按文件时间并进「上次」
        assert!(sh(&core).success());
        let l = last_collect();
        assert_eq!((l["by"].as_str(), l["projects"].as_u64(), l["failed"].as_u64(), l["newEvidence"].as_u64()), (Some("agent"), Some(2), Some(1), Some(7)), "{l}");
        assert!(!lib.join("collect-agent.json.tmp").exists());

        // App 自己后来又收了一轮：用 App 的
        edit_app_settings(|v| v["collect"]["last"] = json!({ "at": now() + 60, "projects": 3, "failed": 0, "newEvidence": 1 })).unwrap();
        assert_eq!(last_collect()["projects"], 3);

        // App 删掉了、核心不在：什么也不做
        std::fs::remove_file(agent_out()).unwrap();
        assert!(sh(&dir.join("gone")).success());
        assert!(!agent_out().exists());

        std::env::remove_var("CREDITIA_HOME");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_plist_escapes_paths() {
        let p = agent_plist(std::path::Path::new("/Applications/A & \"B\".app/Contents/MacOS/creditia-core"), std::path::Path::new("/Users/me/lib"), Some("/Users/me/lib"));
        assert!(p.contains("<string>/Applications/A &amp; &quot;B&quot;.app/Contents/MacOS/creditia-core</string>"), "{p}");
        assert!(p.contains("&gt; &quot;$1/collect-agent.json.tmp&quot;"), "{p}");
        assert!(p.contains("<key>CREDITIA_HOME</key><string>/Users/me/lib</string>"), "{p}");
        assert!(!agent_plist(std::path::Path::new("/c"), std::path::Path::new("/l"), None).contains("CREDITIA_HOME"));
        // macOS 上让系统自己读一遍：格式对，读回来的脚本和写进去的一样
        if cfg!(target_os = "macos") {
            let f = std::env::temp_dir().join(format!("creditia-agent-{}.plist", std::process::id()));
            std::fs::write(&f, &p).unwrap();
            assert!(Command::new("plutil").arg("-lint").arg(&f).status().unwrap().success());
            let o = Command::new("plutil").args(["-extract", "ProgramArguments.2", "raw", "-o", "-"]).arg(&f).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&o.stdout).trim_end(), AGENT_SCRIPT);
            let _ = std::fs::remove_file(&f);
        }
    }
}
