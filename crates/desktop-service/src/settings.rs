//! Versioned preferences shared by the two desktop frontends. Files are never
//! truncated in place: readers see the previous or the next complete document.
use crate::{Fail, Res};
use serde_json::{json, Map, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static SETTINGS_LOCK: Mutex<()> = Mutex::new(());
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub fn app_settings_path() -> PathBuf { creditia_store::data_dir().join("app.json") }

fn read_settings(path: &Path) -> Res<Value> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| Fail::with("settings_invalid", e))?;
            if !value.is_object() { return Err(Fail::new("settings_invalid")); }
            Ok(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(Fail::with("io", e)),
    }
}

pub fn app_settings() -> Value {
    read_settings(&app_settings_path()).unwrap_or_else(|_| json!({}))
}

/// Stable advisory lock files survive atomic replacement of the data file. OS
/// locks release when either process closes or crashes; there is no stale-PID
/// deletion race. All clients must use this operation for app.json mutations.
pub fn edit_app_settings(f: impl FnOnce(&mut Value)) -> Res<()> {
    let _thread = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = app_settings_path();
    let parent = path.parent().ok_or(Fail::new("io"))?;
    std::fs::create_dir_all(parent).map_err(|e| Fail::with("io", e))?;
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(parent.join("app.json.lock")).map_err(|e| Fail::with("io", e))?;
    lock.lock().map_err(|e| Fail::with("io", e))?;
    let mut settings = read_settings(&path)?;
    f(&mut settings);
    if !settings.is_object() { return Err(Fail::new("settings_invalid")); }
    atomic_json(&path, &settings)
}

pub(crate) fn atomic_json(path: &Path, value: &Value) -> Res<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| Fail::with("io", e))?;
    atomic_bytes(path, &bytes)
}

pub(crate) fn atomic_bytes(path: &Path, bytes: &[u8]) -> Res<()> {
    let parent = path.parent().ok_or(Fail::new("io"))?;
    std::fs::create_dir_all(parent).map_err(|e| Fail::with("io", e))?;
    let temp = parent.join(format!(".creditia-write-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed)));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|e| Fail::with("io", e))?;
    let result = (|| {
        if let Ok(metadata) = std::fs::metadata(path) { file.set_permissions(metadata.permissions())?; }
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        #[cfg(unix)] File::open(parent)?.sync_all()?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result.map_err(|e| Fail::with("io", e))
}

fn merge(target: &mut Value, patch: &Value) {
    if let Some(object) = patch.as_object() {
        if !target.is_object() { *target = json!({}); }
        for (key, value) in object { merge(&mut target[key], value); }
    } else { *target = patch.clone(); }
}

fn defaults(settings: &Value) -> Value {
    let gpui = &settings["gpui"];
    json!({
        "version": 1,
        "global": {
            "lang": gpui["language"].as_str().or(gpui["lang"].as_str()).unwrap_or("zh"),
            "appearance": gpui["appearance"].as_str().unwrap_or("system"),
            "reducedMotion": gpui["reducedMotion"].as_bool().unwrap_or(false),
            "lastProject": gpui["lastProject"], "named": settings["ui"]["legacyImported"].as_bool().unwrap_or(false),
            "style": "minimal-light", "view": "wall", "exportFormat": "png", "exportSize": "native",
            "autoAnalyze": true, "logo": settings["logo"].as_str().unwrap_or("star")
        },
        "projects": {}, "legacy": {}
    })
}

pub fn ui_preferences() -> Value {
    let settings = app_settings();
    let mut value = defaults(&settings);
    if settings["ui"].is_object() { merge(&mut value, &settings["ui"]); }
    value
}

fn validate_preferences(value: &Value) -> Res<()> {
    if !value.is_object() { return Err(Fail::new("preferences_invalid")); }
    if value["version"].as_u64().unwrap_or(1) != 1 { return Err(Fail::new("preferences_version")); }
    for key in ["global", "projects", "legacy"] {
        if !value[key].is_null() && !value[key].is_object() { return Err(Fail::with("preferences_invalid", key)); }
    }
    let global = &value["global"];
    for (key, options) in [
        ("lang", &["zh", "en"][..]), ("appearance", &["system", "light", "dark"][..]),
        ("view", &["wall", "roam"][..]),
    ] {
        if !global[key].is_null() && !global[key].as_str().is_some_and(|v| options.contains(&v)) {
            return Err(Fail::with("preferences_invalid", key));
        }
    }
    for key in ["named", "reducedMotion", "autoAnalyze"] {
        if !global[key].is_null() && !global[key].is_boolean() { return Err(Fail::with("preferences_invalid", key)); }
    }
    for project in value["projects"].as_object().into_iter().flat_map(Map::values) {
        if !project.is_object() { return Err(Fail::new("preferences_invalid")); }
        if !project["group"].is_null() && !project["group"].as_str().is_some_and(|g| ["auto", "tool", "model"].contains(&g)) {
            return Err(Fail::with("preferences_invalid", "group"));
        }
        if !project["autoAnalyze"].is_null() && !project["autoAnalyze"].is_boolean() { return Err(Fail::with("preferences_invalid", "autoAnalyze")); }
    }
    if serde_json::to_vec(value).map_err(|e| Fail::with("preferences_invalid", e))?.len() > 4 * 1024 * 1024 {
        return Err(Fail::new("preferences_too_large"));
    }
    Ok(())
}

pub fn patch_ui_preferences(patch: Value) -> Res<Value> {
    validate_preferences(&patch)?;
    let mut result = Value::Null;
    let mut failure = None;
    edit_app_settings(|settings| {
        let mut value = defaults(settings);
        if settings["ui"].is_object() { merge(&mut value, &settings["ui"]); }
        if value["version"].as_u64() != Some(1) { failure = Some(Fail::new("preferences_version")); return; }
        merge(&mut value, &patch);
        match validate_preferences(&value) {
            Ok(()) => { settings["ui"] = value.clone(); result = value; }
            Err(e) => failure = Some(e),
        }
    })?;
    if let Some(error) = failure { return Err(error); }
    Ok(result)
}

fn legacy_target(key: &str, value: &str) -> Option<(Vec<String>, Value)> {
    let key = key.strip_prefix("creditia.")?;
    let global = |name: &str, value: Value| Some((vec!["global".into(), name.into()], value));
    match key {
        "lang" if ["zh", "en"].contains(&value) => global("lang", json!(value)),
        "theme" => global("style", json!(value)),
        "view" if ["wall", "roam"].contains(&value) => global("view", json!(value)),
        "names" => global("named", json!(value != "off")),
        "autofresh" => global("autoAnalyze", json!(value != "off")),
        "fmt" => global("exportFormat", json!(value)),
        "size" => global("exportSize", json!(value)),
        "project" => global("lastProject", json!(value)),
        "logo" => global("logo", json!(value)),
        _ => {
            if let Some(id) = key.strip_prefix("group.") {
                if !id.is_empty() && ["auto", "tool", "model"].contains(&value) {
                    return Some((vec!["projects".into(), id.into(), "group".into()], json!(value)));
                }
            }
            if let Some(id) = key.strip_prefix("autofresh.") {
                if !id.is_empty() {
                    let value = match value { "on" => json!(true), "off" => json!(false), _ => Value::Null };
                    return Some((vec!["projects".into(), id.into(), "autoAnalyze".into()], value));
                }
            }
            None
        }
    }
}
fn at_path<'a>(value: &'a mut Value, path: &[String]) -> &'a mut Value {
    let mut current = value;
    for key in path {
        if !current.is_object() { *current = json!({}); }
        current = &mut current[key.as_str()];
    }
    current
}

fn validate_legacy(values: &Value) -> Res<()> {
    let object = values.as_object().ok_or(Fail::new("preferences_invalid"))?;
    if object.len() > 4096 { return Err(Fail::new("preferences_too_large")); }
    for (key, value) in object {
        if !key.starts_with("creditia.") || key.len() > 512 || !value.as_str().is_some_and(|v| v.len() <= 256 * 1024) {
            return Err(Fail::new("preferences_invalid"));
        }
    }
    if serde_json::to_vec(values).map_err(|e| Fail::with("preferences_invalid", e))?.len() > 4 * 1024 * 1024 {
        return Err(Fail::new("preferences_too_large"));
    }
    Ok(())
}

/// One-time import. Existing canonical fields win, so reopening the old client
/// after a GPUI edit cannot silently restore stale WebView preferences.
pub fn import_legacy_preferences(values: Value) -> Res<Value> {
    validate_legacy(&values)?;
    let mut failure = None;
    edit_app_settings(|settings| {
        if settings["ui"]["version"].as_u64().is_some_and(|v| v != 1) { failure = Some(Fail::new("preferences_version")); return; }
        if settings["ui"]["legacyImported"].as_bool() == Some(true) { return; }
        if !settings["ui"].is_object() { settings["ui"] = json!({"version":1}); }
        for (key, value) in values.as_object().unwrap() {
            if let Some((path, mapped)) = legacy_target(key, value.as_str().unwrap()) {
                let target = at_path(&mut settings["ui"], &path);
                if target.is_null() { *target = mapped; }
            }
            settings["ui"]["legacy"][key] = value.clone();
        }
        settings["ui"]["legacyImported"] = json!(true);
    })?;
    if let Some(e) = failure { return Err(e); }
    Ok(legacy_preferences())
}

pub fn set_legacy_preference(key: String, value: String) -> Res<()> {
    let mut values = json!({}); values[&key] = json!(value);
    validate_legacy(&values)?;
    let mut patch = json!({"legacy": values});
    if let Some((path, mapped)) = legacy_target(&key, &value) { *at_path(&mut patch, &path) = mapped; }
    patch_ui_preferences(patch)?;
    Ok(())
}

/// Projection lets the legacy frontend retain its storage API and keeps a
/// downgrade usable. Unknown old keys remain available unchanged.
pub fn legacy_preferences() -> Value {
    let prefs = ui_preferences();
    let mut out = prefs["legacy"].as_object().cloned().unwrap_or_default();
    let global = &prefs["global"];
    for (old, new) in [("lang", "lang"), ("theme", "style"), ("view", "view"), ("fmt", "exportFormat"), ("size", "exportSize"), ("project", "lastProject"), ("logo", "logo")] {
        if let Some(value) = global[new].as_str() { out.insert(format!("creditia.{old}"), json!(value)); }
    }
    for (old, new) in [("names", "named"), ("autofresh", "autoAnalyze")] {
        if let Some(value) = global[new].as_bool() { out.insert(format!("creditia.{old}"), json!(if value { "on" } else { "off" })); }
    }
    for (id, project) in prefs["projects"].as_object().into_iter().flat_map(Map::iter) {
        if let Some(group) = project["group"].as_str() { out.insert(format!("creditia.group.{id}"), json!(group)); }
        if let Some(on) = project["autoAnalyze"].as_bool() { out.insert(format!("creditia.autofresh.{id}"), json!(if on { "on" } else { "off" })); }
        else { out.remove(&format!("creditia.autofresh.{id}")); }
    }
    Value::Object(out)
}

/// Run the original-storage WebView bridge once. The helper retains the original
/// Tauri application identifier and origin; reading its files as arbitrary JSON
/// would not work across WebKit/WebView2 versions. This is a blocking startup job.
pub fn migrate_legacy_with_helper() -> Res<Value> {
    if app_settings()["ui"]["legacyImported"].as_bool() == Some(true) { return Ok(ui_preferences()); }
    let executable = std::env::current_exe().map_err(|e| Fail::with("migration_failed", e))?;
    let directory = executable.parent().ok_or(Fail::new("migration_helper_missing"))?;
    let name = if cfg!(windows) { "creditia-migrate.exe" } else { "creditia-migrate" };
    let mut candidates = vec![];
    if let Some(path) = std::env::var_os("CREDITIA_MIGRATION_HELPER") { candidates.push(PathBuf::from(path)); }
    candidates.extend([
        directory.join("resources/legacy").join(name),
        directory.join("../Resources/legacy/Creditia Migration.app/Contents/MacOS/creditia"),
        directory.join("../Resources/legacy").join(name),
        directory.join("legacy").join(name),
    ]);
    let helper = candidates.into_iter().find(|path| path.is_file()).ok_or(Fail::new("migration_helper_missing"))?;
    let mut command = std::process::Command::new(helper);
    command.arg("--migrate-ui-settings").stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)] {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| Fail::with("migration_failed", e))?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() || app_settings()["ui"]["legacyImported"].as_bool() != Some(true) { return Err(Fail::with("migration_failed", format!("Helper exited with {status}"))); }
                return Ok(ui_preferences());
            }
            Ok(None) => {}
            Err(error) => { let _ = child.kill(); let _ = child.wait(); return Err(Fail::with("migration_failed", error)); }
        }
        if started.elapsed() > std::time::Duration::from_secs(50) {
            #[cfg(unix)] if let Ok(pid) = i32::try_from(child.id()) {
                // SAFETY: the helper was started in its own process group.
                if pid > 1 { unsafe { libc::kill(-pid, libc::SIGKILL); } }
            }
            #[cfg(windows)] {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).creation_flags(0x0800_0000).status();
            }
            let _ = child.kill(); let _ = child.wait();
            return Err(Fail::new("migration_timeout"));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
