//! creditia App：项目列表、档案浏览、统计、导出。分析在 CLI（Rust 核心）里做，App 只读作品库和仓库里的档案。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

#[tauri::command]
fn projects() -> Result<Value, String> {
    let lib = Library::open()?;
    let list: Vec<Value> = lib
        .projects()?
        .into_iter()
        .map(|p| {
            let credits = PathBuf::from(&p.path).join(".creditia/credits.json");
            json!({
                "id": p.id, "name": p.name, "path": p.path, "remotes": p.remotes,
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
fn credits(project_id: String) -> Result<Value, String> {
    let lib = Library::open()?;
    if let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) {
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
    Err("这个项目还没有档案；先在 CLI 里 creditia analyze".into())
}

#[tauri::command]
fn config_of(project_id: String) -> Result<Value, String> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Ok(json!({})) };
    let path = PathBuf::from(&p.path).join(".creditia/config.json");
    Ok(std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({})))
}

#[tauri::command]
fn set_display_director(project_id: String, on: bool) -> Result<(), String> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Err("项目不在作品库里".into()) };
    creditia_store::Library::set_display_director(std::path::Path::new(&p.path), on)
}

#[tauri::command]
fn embed_snippet(project_id: String, form: String, theme: String, director: bool) -> Result<String, String> {
    let lib = Library::open()?;
    let Some(p) = lib.projects()?.into_iter().find(|p| p.id == project_id) else { return Err("项目不在作品库里".into()) };
    let rel = PathBuf::from(&p.path).join(".creditia/credits.json");
    let src = if rel.exists() { "/.creditia/credits.json".to_string() } else { "credits.json".to_string() };
    Ok(format!(
        "<script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js\"></script>\n<creditia-card src=\"{src}\" form=\"{form}\" theme=\"{theme}\"{director}/>\n"
    ))
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

/// 导出写进「下载」目录；重名自动加序号
#[tauri::command]
fn save_file(name: String, bytes: String, mime: Option<String>) -> Result<String, String> {
    let _ = mime;
    let dir = home().ok_or("找不到用户目录")?.join("Downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe = name.replace(['/', '\\', ':'], "-");
    let mut path = dir.join(&safe);
    if path.exists() {
        let stem = std::path::Path::new(&safe).file_stem().and_then(|s| s.to_str()).unwrap_or("export").to_string();
        let ext = std::path::Path::new(&safe).extension().and_then(|s| s.to_str()).map(|s| s.to_string());
        for i in 2..1000 {
            let cand = match &ext { Some(e) => dir.join(format!("{stem}-{i}.{e}")), None => dir.join(format!("{stem}-{i}")) };
            if !cand.exists() { path = cand; break; }
        }
    }
    let data = base64::engine::general_purpose::STANDARD.decode(bytes).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
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

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    { let _ = Command::new("open").arg(path.as_str()).status(); }
    #[cfg(target_os = "windows")]
    { let _ = Command::new("cmd").args(["/c", "start", "", path.as_str()]).status(); }
    #[cfg(all(unix, not(target_os = "macos")))]
    { let _ = Command::new("xdg-open").arg(path.as_str()).status(); }
    Ok(())
}

/// 数据源体检：和 CLI 的 creditia doctor 同一份路径规则
#[tauri::command]
fn doctor() -> Value {
    let h = home().unwrap_or_default();
    let env_dir = |k: &str, d: &str| std::env::var_os(k).map(PathBuf::from).unwrap_or_else(|| h.join(d));
    let mut out: Vec<Value> = vec![];
    let mut add = |id: &str, name: &str, path: PathBuf, marker: &str| {
        let probe = path.join(marker);
        let ok = probe.exists();
        let detail = if !ok { "没有找到".to_string() } else if probe.is_dir() {
            format!("{} 个条目", std::fs::read_dir(&probe).map(|d| d.count()).unwrap_or(0))
        } else { "已就绪".to_string() };
        out.push(json!({ "id": id, "name": name, "path": path.display().to_string(), "ok": ok, "detail": detail }));
    };
    add("codex", "Codex", env_dir("CODEX_HOME", ".codex"), "state_5.sqlite");
    add("claude-code", "Claude Code", env_dir("CLAUDE_CONFIG_DIR", ".claude"), "projects");
    add("cursor", "Cursor", env_dir("CREDITIA_CURSOR_DIR", ".cursor"), "ai-tracking/ai-code-tracking.db");
    add("grok", "Grok", env_dir("CREDITIA_GROK_DIR", ".grok"), "sessions");
    let kimi = std::env::var_os("CREDITIA_KIMI_DIR").map(PathBuf::from).unwrap_or_else(|| {
        let a = h.join(".kimi-code");
        if a.exists() { a } else { h.join(".kimi") }
    });
    add("kimi", "Kimi", kimi, "sessions");
    json!(out)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            projects, credits, config_of, set_display_director, embed_snippet,
            library_path, save_file, reveal, open_path, doctor
        ])
        .run(tauri::generate_context!())
        .expect("启动 creditia App 失败");
}
