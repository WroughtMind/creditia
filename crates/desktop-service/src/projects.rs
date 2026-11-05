//! Project operations shared by both desktop frontends and their panels.
use crate::{Fail, Res};
use creditia_store::Library;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub(crate) fn config_lock(root: &Path) -> Res<std::fs::File> {
    if !root.is_dir() { return Err(Fail::with("dir_missing", root.display())); }
    let directory = root.join(".creditia");
    std::fs::create_dir_all(&directory).map_err(|e| Fail::with("io", e))?;
    let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(directory.join("config.lock")).map_err(|e| Fail::with("io", e))?;
    lock.lock().map_err(|e| Fail::with("io", e))?;
    Ok(lock)
}

pub fn project_archives() -> Res<Value> {
    let library = Library::open()?;
    let mut archives = vec![];
    for project in library.projects()? {
        if let Ok(raw) = crate::credits(project.id.clone()) {
            archives.push(json!({"id":project.id,"name":project.name,"raw":raw}));
        }
    }
    Ok(json!(archives))
}

pub fn project_details(project_id: String) -> Res<Value> {
    let library = Library::open()?;
    let project = crate::project(&library, &project_id)?;
    let archive = crate::credits(project_id.clone()).unwrap_or(Value::Null);
    let integrations = crate::integrations(project_id.clone()).unwrap_or(Value::Null);
    Ok(json!({
        "project": {"id":project.id,"name":project.name,"path":project.path,"exists":Path::new(&project.path).is_dir(),"lastHead":project.last_head,"lastAnalyzed":project.last_analyzed},
        "config":crate::config_of(project_id.clone())?,
        "paths":library.paths(&project_id)?.into_iter().map(|(path,kind)|json!({"path":path,"kind":kind})).collect::<Vec<_>>(),
        "archive":archive,"evidence":library.evidence_count(&project_id),
        "sessions":archive["sessions"],"integrations":integrations,
        "snapshots":library.snapshots(&project_id)?.into_iter().map(|(head,generated)|json!({"head":head,"generatedAt":generated})).collect::<Vec<_>>()
    }))
}

/// Patch only configurable analysis/display fields. Repository identity and
/// integrations must go through their existing core operations.
pub fn set_project_config(project_id: String, patch: Value) -> Res<Value> {
    let fields = ["name", "branch", "level", "markers", "excludeMarkers", "directors", "agentEmails", "botEmails", "branchPrefixes", "segments", "ignoreTags", "privacy", "display"];
    let object = patch.as_object().ok_or(Fail::new("config_invalid"))?;
    for key in object.keys() { if !fields.contains(&key.as_str()) { return Err(Fail::with("config_invalid", key)); } }
    if let Some(level) = object.get("level") {
        if !level.as_str().is_some_and(|v| ["strict", "standard", "loose"].contains(&v)) { return Err(Fail::with("config_invalid", "level")); }
    }
    for key in ["name", "branch"] {
        if let Some(value) = object.get(key) {
            if !value.as_str().is_some_and(|v| !v.trim().is_empty() && v.len() <= 1024 && !v.contains('\0')) { return Err(Fail::with("config_invalid", key)); }
        }
    }
    for key in ["markers", "excludeMarkers", "botEmails", "ignoreTags"] {
        if let Some(value) = object.get(key) {
            if !value.as_array().is_some_and(|values| values.iter().all(|v| v.is_string())) { return Err(Fail::with("config_invalid", key)); }
        }
    }
    for key in ["privacy", "display", "agentEmails", "branchPrefixes"] {
        if let Some(value) = object.get(key) { if !value.is_object() { return Err(Fail::with("config_invalid", key)); } }
    }
    for key in ["agentEmails", "branchPrefixes"] {
        if let Some(value) = object.get(key) {
            if !value.as_object().is_some_and(|entries| entries.values().all(Value::is_string)) { return Err(Fail::with("config_invalid", key)); }
        }
    }
    if let Some(privacy) = object.get("privacy") {
        for key in ["hideTitles", "hidePaths"] {
            if privacy.get(key).is_some_and(|value| !value.is_boolean()) { return Err(Fail::with("config_invalid", key)); }
        }
        if let Some(aliases) = privacy.get("aliases") {
            if !aliases.as_object().is_some_and(|entries| entries.values().all(Value::is_string)) { return Err(Fail::with("config_invalid", "aliases")); }
        }
    }
    if let Some(display) = object.get("display") {
        if display.get("hideNames").is_some_and(|value| !value.is_boolean()) { return Err(Fail::with("config_invalid", "hideNames")); }
        if display.get("sign").is_some_and(|value| !value.as_str().is_some_and(|s| s.chars().count() <= 256)) { return Err(Fail::with("config_invalid", "sign")); }
    }
    if let Some(directors) = object.get("directors") {
        if !directors.as_array().is_some_and(|items| items.iter().all(|person| person["id"].is_string() && person["name"].is_string()
            && person.get("emails").is_none_or(|emails| emails.as_array().is_some_and(|items| items.iter().all(Value::is_string))))) {
            return Err(Fail::with("config_invalid", "directors"));
        }
    }
    if let Some(segments) = object.get("segments") {
        if !segments.as_array().is_some_and(|items| items.iter().all(|segment| segment.as_array().is_some_and(|parts| parts.len() == 3 && parts.iter().all(Value::is_string)))) {
            return Err(Fail::with("config_invalid", "segments"));
        }
    }
    if serde_json::to_vec(&patch).map_err(|e| Fail::with("config_invalid", e))?.len() > 1024 * 1024 { return Err(Fail::new("config_invalid")); }
    let project = crate::project(&Library::open()?, &project_id)?;
    let root = PathBuf::from(project.path);
    let _lock = config_lock(&root)?;
    let directory = root.join(".creditia");
    let path = directory.join("config.json");
    let mut config = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(|e| Fail::with("config_invalid", e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(Fail::with("io", e)),
    };
    if !config.is_object() { return Err(Fail::new("config_invalid")); }
    fn merge(target: &mut Value, patch: &Value) {
        if let Some(object) = patch.as_object() {
            if !target.is_object() { *target = json!({}); }
            for (key, value) in object { merge(&mut target[key], value); }
        } else { *target = patch.clone(); }
    }
    merge(&mut config, &patch);
    crate::settings::atomic_json(&path, &config)?;
    Ok(config)
}

pub fn blame(project_id: String, file: String, line: u64) -> Res<Value> {
    if file.is_empty() || line == 0 || file.starts_with('-') || Path::new(&file).components().any(|part| matches!(part, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(Fail::new("invalid_location"));
    }
    let project = crate::project(&Library::open()?, &project_id)?;
    crate::run_core_job_with_cancel(&["blame", &format!("{file}:{line}"), "--repo", &project.path], None, None, crate::AnalysisCancellation::default())
}

/// Same evidence archive schema as `creditia-core evidence export`. It contains
/// line hashes and metadata, never the code or chat text itself.
pub fn evidence_archive(project_id: String) -> Res<Value> {
    let library = Library::open()?;
    let project = crate::project(&library, &project_id)?;
    let (evidence, sessions) = library.load_evidence(&project_id)?;
    Ok(json!({"creditia":"evidence","version":1,"project":project.id,"name":project.name,"rootCommit":project.root_commit,"evidence":evidence,"sessions":sessions}))
}

pub fn import_evidence(project_id: String, path: PathBuf, cancellation: crate::AnalysisCancellation) -> Res<Value> {
    let project = crate::project(&Library::open()?, &project_id)?;
    if !path.is_file() { return Err(Fail::with("io", "Evidence file not found")); }
    crate::run_core_job_with_cancel(&["evidence", "import", &path.to_string_lossy(), "--repo", &project.path], None, None, cancellation)
}

pub fn doctor() -> Value {
    match crate::run_core(&["doctor"]) {
        Ok(value) => json!({"sources":value["sources"],"git":value["git"],"gh":value["gh"],"library":value["library"],"summary":value["summary"]}),
        Err(error) => json!({"sources":[],"error":error}),
    }
}
