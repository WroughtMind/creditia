//! creditia App：项目列表、档案浏览、展示选项。分析在 CLI（Rust 核心）里做，App 只读作品库和仓库里的档案。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use creditia_store::Library;
use serde_json::{json, Value};
use std::path::PathBuf;

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![projects, credits, config_of, set_display_director, embed_snippet])
        .run(tauri::generate_context!())
        .expect("启动 creditia App 失败");
}
