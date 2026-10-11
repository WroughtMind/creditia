//! 桌面与开发预览只调用这里；分析口径与 CLI 相同。
use creditia_adapters as ad;
use creditia_core::config::Person;
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use crate::analyze;

pub fn projects() -> Result<Value, String> {
    let lib = Library::open()?;
    let mut seen = std::collections::HashSet::new();
    let mut rows = vec![];
    for p in lib.projects()? {
        if !seen.insert(p.path.clone()) { continue; }
        rows.push(json!({ "id": p.id, "name": p.name, "path": p.path, "paths": lib.paths(&p.id)?, "lastAnalyzed": p.last_analyzed, "exists": Path::new(&p.path).is_dir(), "hasCredits": Path::new(&p.path).join(".creditia/credits.json").exists() }));
    }
    Ok(json!(rows))
}
fn project_path(id: &str) -> Result<PathBuf, String> {
    Library::open()?.projects()?.into_iter().find(|p| p.id == id).map(|p| PathBuf::from(p.path)).ok_or("项目不在作品库中".into())
}
fn editable_project(id: &str) -> Result<PathBuf, String> {
    let repo = project_path(id)?;
    if !repo.is_dir() || !ad::git::git_ok(&repo, &["rev-parse", "--git-dir"]) { return Err("项目目录已不存在，请先打开当前的项目副本".into()); }
    Ok(repo)
}
pub fn credits(id: &str) -> Result<Value, String> {
    let path = project_path(id)?.join(".creditia/credits.json");
    let historical = !path.exists();
    let text = if path.exists() { std::fs::read_to_string(path).map_err(|e| e.to_string())? }
        else { Library::open()?.snapshot_json(id)?.ok_or("还没有分析记录")? };
    let mut record: Value = serde_json::from_str(&text).map_err(|e| format!("署名档案无法解析：{e}"))?;
    if !record["project"].is_object() { return Err("署名档案缺少项目信息".into()); }
    record["project"]["id"] = json!(id);
    record["archive"] = json!({ "historical": historical });
    Ok(record)
}
pub fn library_credits() -> Result<Value, String> {
    let mut records = vec![];
    let mut warnings = vec![];
    let mut paths = std::collections::HashSet::new();
    for project in Library::open()?.projects()? {
        if !paths.insert(project.path.clone()) { continue; }
        match credits(&project.id) {
            Ok(record) => records.push(record),
            Err(error) => warnings.push(format!("{}：{error}", project.name)),
        }
    }
    Ok(json!({ "projects": records, "warnings": warnings }))
}
pub fn config_of(id: &str) -> Result<Value, String> {
    let path = project_path(id)?.join(".creditia/config.json");
    if !path.exists() { return Ok(json!({})); }
    serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
pub fn analyze_project(path: &str, branch: Option<&str>) -> Result<Value, String> {
    let selected = Path::new(path).canonicalize().map_err(|_| "项目路径不存在")?;
    if !ad::git::git_ok(&selected, &["rev-parse", "--git-dir"]) { return Err("请选择 Git 项目文件夹".into()); }
    let repo = PathBuf::from(ad::git::git(&selected, &["rev-parse", "--show-toplevel"]).trim());
    let config = analyze::load_config(&repo, None)?;
    let id = analyze::project_of(&repo, &config).id;
    let report = analyze::run(&repo, &analyze::Opts { branch: branch.map(String::from), fresh: false, github: false, out: None, blame: None, config: None })?;
    Ok(json!({ "id": id, "warnings": report.warnings, "notes": report.notes }))
}
pub fn set_people(id: &str, people: Vec<Person>, show: bool) -> Result<Value, String> {
    let repo = editable_project(id)?;
    let mut config = analyze::load_config(&repo, None)?;
    config.people = people; config.display.people = show;
    config.save(&repo.join(".creditia/config.json"))?;
    Ok(json!({ "saved": true }))
}
pub fn link_path(id: &str, path: &str) -> Result<Value, String> {
    let repo = editable_project(id)?;
    let mut config = analyze::load_config(&repo, None)?;
    let linked = Path::new(path);
    // 允许显式登记另一台电脑的绝对路径；不要求它在当前系统存在。
    let windows_absolute = path.as_bytes().get(1) == Some(&b':') || path.starts_with("\\\\");
    if !linked.is_absolute() && !windows_absolute { return Err("请填写完整的项目路径".into()); }
    if linked.exists() {
        if !ad::git::git_ok(linked, &["rev-parse", "--git-dir"]) { return Err("这个目录不是 Git 项目".into()); }
        let (root, _) = ad::git::identity(linked);
        if root != ad::git::identity(&repo).0 { return Err("两个目录的仓库历史不同，请选择同一项目的工作副本".into()); }
    }
    if !config.paths.iter().any(|p| p == path) { config.paths.push(path.to_string()); }
    config.save(&repo.join(".creditia/config.json"))?;
    Library::open()?.register_path(id, path)?;
    Ok(json!({ "linked": path }))
}
pub fn doctor() -> Value { json!([ad::codex::doctor(), ad::claude::doctor(), ad::cursor::doctor(), ad::grok::doctor(), ad::kimi::doctor(), ad::opencode::doctor(), ad::gemini::doctor()]) }
