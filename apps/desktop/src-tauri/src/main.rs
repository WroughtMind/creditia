//! creditia App 的本机桥：项目列表、档案、分析、保存导出文件、数据源体检。
//! 分析交给同一个 Rust 核心二进制（creditia-core），App 不调用任何大模型。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use creditia_desktop_service as service;
use service::{Fail, Res, app_settings, default_export_dir, edit_app_settings, export_dir, home, project, run_core};

#[tauri::command]
fn projects() -> Res<Value> { service::projects() }
#[tauri::command]
fn credits(project_id: String) -> Res<Value> { service::credits(project_id) }
#[tauri::command]
fn stamp(project_id: String) -> Res<String> { service::stamp(project_id) }
#[tauri::command]
fn head(project_id: String) -> Res<String> { service::head(project_id) }
#[tauri::command]
fn config_of(project_id: String) -> Res<Value> { service::config_of(project_id) }
#[tauri::command]
fn set_display(project_id: String, hide_names: bool, sign: String) -> Res<()> { service::set_display(project_id, hide_names, sign) }

fn progress_sink(app: tauri::AppHandle) -> service::ProgressSink {
    use tauri::Emitter;
    std::sync::Arc::new(move |progress| { let _ = app.emit("analyze-progress", progress); })
}
#[cfg(all(test, unix))]
fn run_analysis(repo: &str, lang: Option<&str>, app: Option<tauri::AppHandle>) -> Res<Value> {
    service::run_analysis(repo, lang, app.map(progress_sink))
}
#[tauri::command]
fn cancel_analyze() { service::cancel_analyze(); }

/// 重新分析一个项目（在它的仓库目录里跑核心）
#[tauri::command]
async fn analyze(app: tauri::AppHandle, project_id: String, lang: Option<String>) -> Res<Value> {
    tauri::async_runtime::spawn_blocking(move || service::analyze(project_id, lang, Some(progress_sink(app)))).await.map_err(|e| Fail::with("core_failed", e))?
}

/// 本机用过 AI 工具的项目（核心的 discover）：第一次打开时挑项目用。要扫各工具的记录，放到后台线程
#[tauri::command]
async fn discover() -> Res<Value> {
    tauri::async_runtime::spawn_blocking(service::discover).await.map_err(|e| Fail::with("core_failed", e))?
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
    tauri::async_runtime::spawn_blocking(move || service::add_project(path, lang, Some(progress_sink(app)))).await.map_err(|e| Fail::with("core_failed", e))?
}

#[tauri::command]
fn integrations(project_id: String) -> Res<Value> { service::integrations(project_id) }
#[tauri::command]
fn set_integration(project_id: String, which: String, on: bool, lang: Option<String>) -> Res<Value> { service::set_integration(project_id, which, on, lang) }
#[tauri::command]
fn remove_project(project_id: String) -> Res<()> { service::remove_project(project_id) }
#[tauri::command]
fn link_path(project_id: String, path: String, unlink: Option<bool>, lang: Option<String>) -> Res<Value> { service::link_path(project_id, path, unlink, lang) }
#[tauri::command]
fn embed_snippet(project_id: String, piece: String, theme: String) -> Res<String> { service::embed_snippet(project_id, piece, theme) }
#[tauri::command]
fn library_path() -> String { service::library_path() }

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

#[tauri::command]
fn import_legacy_preferences(values: Value) -> Res<Value> { service::import_legacy_preferences(values) }
#[tauri::command]
fn set_legacy_preference(key: String, value: String) -> Res<()> { service::set_legacy_preference(key, value) }
#[tauri::command]
fn legacy_preferences() -> Value { service::legacy_preferences() }

struct MigrationMode(bool);
#[tauri::command]
fn migration_mode(mode: tauri::State<'_, MigrationMode>) -> bool { mode.inner().0 }
#[tauri::command]
fn migration_done(app: tauri::AppHandle, mode: tauri::State<'_, MigrationMode>, success: bool) -> Res<()> {
    if !mode.inner().0 { return Err(Fail::new("unsupported")); }
    app.exit(if success { 0 } else { 1 });
    Ok(())
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
fn reset_export_dir() -> Res<String> { service::reset_export_dir() }

// ───────────────────────── 定时采集证据 ─────────────────────────
// 各工具的日志会被清理（有的只留几十天）。App 开着时每小时把作品库里每个项目的新证据收进作品库：
// 只采集，不分析，不改仓库。设置在 app.json 的 collect 里：on（默认开）、last（上一轮的结果）

#[tauri::command]
fn collect_info() -> Value { service::collect_info() }
#[tauri::command]
fn set_collect(on: bool) -> Res<Value> { service::set_collect(on) }
#[tauri::command]
async fn collect_now() -> Res<Value> {
    tauri::async_runtime::spawn_blocking(service::collect_now).await.map_err(|e| Fail::with("core_failed", e))?
}
#[tauri::command]
fn set_collect_agent(on: bool) -> Res<Value> { service::set_collect_agent(on) }
#[cfg(target_os = "macos")]
fn agent_refresh() { service::collection::agent_refresh(); }
#[cfg(all(test, unix))]
use service::analysis_running;
#[cfg(all(test, unix))]
use service::collection::{AGENT_SCRIPT, agent_out, agent_plist, collect_round, last_collect, now};

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
    let migration = std::env::args().any(|arg| arg == "--migrate-ui-settings");
    let mut context = tauri::generate_context!();
    if migration {
        for window in &mut context.config_mut().app.windows { window.visible = false; window.focus = false; }
    }
    tauri::Builder::default()
        .manage(MigrationMode(migration))
        .on_page_load(move |webview, payload| {
            if migration && payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.eval(include_str!("migration.js"));
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            use tauri::{Emitter, Manager};
            if migration {
                if let Some(window) = app.get_webview_window("main") { window.hide()?; }
                let handle = app.handle().clone();
                std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_secs(45)); handle.exit(2); });
                return Ok(());
            }
            #[cfg(target_os = "macos")]
            agent_refresh();
            if let Some(id) = app_settings()["logo"].as_str() {
                if id != DEFAULT_LOGO {
                    apply_icon(app.handle(), id);
                }
            }
            let h = app.handle().clone();
            let background = service::BackgroundService::start(std::sync::Arc::new(move |event| {
                if event["kind"] == "collect-done" { let _ = h.emit("collect-done", &event["result"]); }
                let _ = h.emit("service-event", event);
            }));
            app.manage(background);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            projects, credits, stamp, head, config_of, set_display, analyze, cancel_analyze, discover, pick_folder, add_project, integrations, set_integration, remove_project, link_path, embed_snippet,
            library_path, app_info, migration_mode, migration_done, import_legacy_preferences, set_legacy_preference, legacy_preferences, choose_export_dir, reset_export_dir, open_folder, save_file, reveal, doctor, collect_info, set_collect, collect_now, set_collect_agent, logo, set_logo, preview::render_preview
        ])
        .run(context)
        .expect("启动 creditia App 失败");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// 测试都改 CREDITIA_CORE / CREDITIA_HOME 这类进程级的环境变量：一个跑完再跑下一个
    static ENV: Mutex<()> = Mutex::new(());
    struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl EnvGuard {
        fn new() -> Self { Self(["CREDITIA_HOME", "CREDITIA_CORE"].into_iter().map(|key| (key, std::env::var_os(key))).collect()) }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value { Some(value) => std::env::set_var(key, value), None => std::env::remove_var(key) }
            }
        }
    }


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
        let _restore = EnvGuard::new();
        let dir = std::env::temp_dir().join(format!("creditia-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CREDITIA_HOME", dir.join("library"));

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
        assert!(!analysis_running());

        // 找不到核心
        std::env::set_var("CREDITIA_CORE", dir.join("nope"));
        assert_eq!(run_analysis("/tmp", None, None).unwrap_err().code, "core_missing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_rounds_cover_the_library() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvGuard::new();
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
        let _restore = EnvGuard::new();
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
