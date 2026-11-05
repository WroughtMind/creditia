//! Shared evidence collection and native background-agent integration.
use crate::{Fail, Res, AnalysisCancellation, analysis_running, app_settings, core_bin, home, run_core_job_with_cancel};
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;
pub const COLLECT_EVERY: i64 = 3600;
/// 同一时间只跑一轮（定时的和「现在采集」不叠在一起）
static COLLECTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ACTIVE_COLLECTION: std::sync::Mutex<Option<(bool, AnalysisCancellation)>> = std::sync::Mutex::new(None);

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn collect_on() -> bool {
    app_settings()["collect"]["on"].as_bool().unwrap_or(true)
}

fn save_collection_field(key: &str, value: Value) -> Res<()> {
    crate::settings::edit_app_settings_checked(|settings| {
        if !settings["collect"].is_null() && !settings["collect"].is_object() { return Err(Fail::with("settings_invalid", "collect")); }
        settings["collect"][key] = value;
        Ok(())
    })
}

/// 跑一轮：一个项目一个核心进程，有分析在跑就等它跑完再接着收。定时的一轮在关着时不跑，中途被关掉就停下
pub fn collect_round(timed: bool) -> Res<Value> { collect_round_with_cancel(timed, AnalysisCancellation::default()) }

pub fn collect_round_with_cancel(timed: bool, cancellation: AnalysisCancellation) -> Res<Value> {
    use std::sync::atomic::Ordering;
    if cancellation.is_cancelled() { return Err(Fail::new("cancelled")); }
    if timed && !collect_on() {
        return Err(Fail::new("off"));
    }
    if COLLECTING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(Fail::new("busy"));
    }
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            *ACTIVE_COLLECTION.lock().unwrap_or_else(|e| e.into_inner()) = None;
            COLLECTING.store(false, Ordering::SeqCst);
        }
    }
    let _done = Done;
    let directory = creditia_store::data_dir();
    std::fs::create_dir_all(&directory).map_err(|e| Fail::with("io", e))?;
    let lease = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(directory.join("desktop-collection.lock")).map_err(|e| Fail::with("io", e))?;
    lease.try_lock().map_err(|_| Fail::new("busy"))?;
    *ACTIVE_COLLECTION.lock().unwrap_or_else(|e| e.into_inner()) = Some((timed, cancellation.clone()));
    let repos: Vec<String> = Library::open()?.projects()?.into_iter().map(|p| p.path).filter(|p| PathBuf::from(p).is_dir()).collect();
    let (mut ok, mut failed, mut new) = (0, 0, 0);
    for repo in repos {
        while analysis_running() {
            if cancellation.is_cancelled() { return Err(Fail::new("cancelled")); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if cancellation.is_cancelled() { return Err(Fail::new("cancelled")); }
        if timed && !collect_on() {
            break;
        }
        let result = loop {
            let result = run_core_job_with_cancel(&["collect", "--repo", repo.as_str()], None, None, cancellation.clone());
            if result.as_ref().err().is_none_or(|error| error.code != "busy") { break result; }
            if cancellation.is_cancelled() || timed && !collect_on() { return Err(Fail::new("cancelled")); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        match result {
            Ok(v) => {
                ok += 1;
                new += v["collected"].as_array().map(|a| a.iter().filter_map(|c| c["newEvidence"].as_u64()).sum::<u64>()).unwrap_or(0);
            }
            Err(error) if error.code == "cancelled" => return Err(error),
            Err(_) => failed += 1,
        }
    }
    let last = json!({ "at": now(), "projects": ok, "failed": failed, "newEvidence": new });
    save_collection_field("last", last.clone())?;
    Ok(last)
}

/// 最近一轮：App 自己收的和后台任务收的，取晚的那个。后台的带 by: agent
pub fn last_collect() -> Value {
    let app = app_settings()["collect"]["last"].clone();
    let agent = agent_last();
    match (app["at"].as_i64(), agent["at"].as_i64()) {
        (Some(a), Some(b)) if b <= a => app,
        (_, Some(_)) => agent,
        _ => app,
    }
}

pub fn collect_info() -> Value {
    let other_process = std::fs::OpenOptions::new().read(true).write(true)
        .open(creditia_store::data_dir().join("desktop-collection.lock"))
        .is_ok_and(|file| file.try_lock().is_err());
    json!({
        "on": collect_on(), "last": last_collect(), "running": COLLECTING.load(std::sync::atomic::Ordering::SeqCst) || other_process, "every": COLLECT_EVERY,
        "agent": { "supported": cfg!(target_os = "macos"), "on": agent_installed() },
    })
}

pub fn set_collect(on: bool) -> Res<Value> {
    save_collection_field("on", json!(on))?;
    if !on {
        if let Some((true, token)) = ACTIVE_COLLECTION.lock().unwrap_or_else(|e| e.into_inner()).as_ref() { token.cancel(); }
    }
    Ok(collect_info())
}

pub fn collect_now() -> Res<Value> { collect_round(false) }

// ───────────────────────── App 关着时也收：macOS 的 launchd 后台任务 ─────────────────────────
// 装在 ~/Library/LaunchAgents，登录后每小时跑一次 App 里带的核心 `collect --all`，结果写进作品库目录的 collect-agent.json。
// App 被删掉、核心不在了，任务就什么也不做

const AGENT_LABEL: &str = "dev.creditia.collect";
/// $0 是核心，$1 是作品库目录。先写临时文件再改名，App 读到的总是完整的一份
pub const AGENT_SCRIPT: &str = r#"[ -x "$0" ] || exit 0; mkdir -p "$1" || exit 0; "$0" collect --all --json > "$1/collect-agent.json.tmp" 2>/dev/null; mv -f "$1/collect-agent.json.tmp" "$1/collect-agent.json""#;

pub fn agent_out() -> PathBuf {
    creditia_store::data_dir().join("collect-agent.json")
}

/// 后台任务上一轮的结果：时间取文件的修改时间，数字从核心的输出里数
pub fn agent_last() -> Value {
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
pub fn agent_plist(core: &std::path::Path, lib: &std::path::Path, creditia_home: Option<&str>) -> String {
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
    crate::settings::atomic_bytes(&p, body.as_bytes())?;
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
pub fn agent_refresh() {
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

pub fn set_collect_agent(on: bool) -> Res<Value> {
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

