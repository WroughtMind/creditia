//! 桌面 App 与 CLI 使用同一分析核心。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use base64::Engine as _;
use creditia_cli::desktop;
use creditia_core::config::Person;
use serde_json::Value;
use tauri::Manager;
use std::io::Write;

#[tauri::command]
fn projects() -> Result<Value, String> { desktop::projects() }
#[tauri::command]
fn credits(project_id: String) -> Result<Value, String> { desktop::credits(&project_id) }
#[tauri::command]
fn library_credits() -> Result<Value, String> { desktop::library_credits() }
#[tauri::command]
fn launch_context() -> Value {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = serde_json::Map::new();
    for pair in args.windows(2) {
        if ["--repo", "--theme", "--form"].contains(&pair[0].as_str()) { out.insert(pair[0][2..].to_string(), Value::String(pair[1].clone())); }
    }
    Value::Object(out)
}
#[tauri::command]
fn config_of(project_id: String) -> Result<Value, String> { desktop::config_of(&project_id) }
#[tauri::command]
fn set_people(project_id: String, people: Vec<Person>, show: bool) -> Result<Value, String> { desktop::set_people(&project_id, people, show) }
#[tauri::command]
fn link_path(project_id: String, path: String) -> Result<Value, String> { desktop::link_path(&project_id, &path) }
#[tauri::command]
async fn analyze_project(path: String, branch: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || desktop::analyze_project(&path, branch.as_deref())).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn doctor() -> Result<Value, String> { tauri::async_runtime::spawn_blocking(desktop::doctor).await.map_err(|e| e.to_string()) }

/// 原子创建，不覆盖同名文件；系统提供下载目录，无序号上限。
#[tauri::command]
fn save_file(app: tauri::AppHandle, name: String, bytes: String, mime: Option<String>) -> Result<String, String> {
    let _ = mime;
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-");
    let safe = safe.trim().trim_matches('.');
    let safe = if safe.is_empty() { "creditia-export" } else { safe };
    let data = base64::engine::general_purpose::STANDARD.decode(bytes).map_err(|e| e.to_string())?;
    let stem = std::path::Path::new(safe).file_stem().and_then(|s| s.to_str()).unwrap_or("export");
    let extension = std::path::Path::new(safe).extension().and_then(|s| s.to_str());
    let mut suffix = 1u64;
    loop {
        let filename = if suffix == 1 { safe.to_string() } else { format!("{stem}-{suffix}{}", extension.map(|e| format!(".{e}")).unwrap_or_default()) };
        let path = dir.join(filename);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&data) { drop(file); let _ = std::fs::remove_file(&path); return Err(error.to_string()); }
                return Ok(path.display().to_string());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => suffix += 1,
            Err(e) => return Err(e.to_string()),
        }
    }
}
fn main() {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![projects, credits, library_credits, launch_context, config_of, set_people, link_path, analyze_project, doctor, save_file])
        .run(tauri::generate_context!()).expect("启动 creditia App 失败");
}
