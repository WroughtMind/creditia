//! Contract tests independent of either UI toolkit.
use creditia_desktop_service as service;
use creditia_store::{Library, Project};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV: Mutex<()> = Mutex::new(());

struct Fixture {
    root: PathBuf,
    previous_home: Option<std::ffi::OsString>,
    previous_core: Option<std::ffi::OsString>,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("creditia-service-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let previous_home = std::env::var_os("CREDITIA_HOME");
        let previous_core = std::env::var_os("CREDITIA_CORE");
        std::env::set_var("CREDITIA_HOME", root.join("library"));
        Self { root, previous_home, previous_core }
    }
    fn project(&self) -> Project {
        let repo = self.root.join("project");
        std::fs::create_dir_all(repo.join(".creditia")).unwrap();
        Project {
            id: "existing-project".into(), name: "Existing project".into(), path: repo.display().to_string(),
            root_commit: "root".into(), remotes: vec!["https://example.org/project.git".into()],
            last_head: Some("previous-head".into()), last_analyzed: Some(42),
        }
    }
    #[cfg(unix)]
    fn core(&self, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.join("creditia-core");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("CREDITIA_CORE", path);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        service::cancel_analyze();
        for (key, previous) in [("CREDITIA_HOME", &self.previous_home), ("CREDITIA_CORE", &self.previous_core)] {
            match previous {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn existing_library_live_archive_and_snapshot_keep_their_contract() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("library");
    let project = fixture.project();
    let lib = Library::open().unwrap();
    lib.upsert_project(&project).unwrap();
    lib.add_path(&project.id, "/previous/project", "linked").unwrap();
    lib.save_snapshot(&project.id, "head", "2026-01-01", r#"{"project":{"name":"snapshot"}}"#).unwrap();
    let live = Path::new(&project.path).join(".creditia/credits.json");
    std::fs::write(&live, r#"{"project":{"name":"live"}}"#).unwrap();

    // Snapshot persistence advances project metadata in the existing store.
    let stored = lib.projects().unwrap().into_iter().find(|p| p.id == project.id).unwrap();
    let list = service::projects().unwrap();
    assert_eq!(list[0]["id"], project.id);
    assert_eq!(list[0]["lastHead"], json!(stored.last_head));
    assert_eq!(list[0]["lastAnalyzed"], json!(stored.last_analyzed));
    assert_eq!(list[0]["pathKinds"]["/previous/project"], "linked");
    assert_eq!(list[0]["exists"], true);
    assert_eq!(list[0]["hasCredits"], true);
    assert_eq!(service::credits(project.id.clone()).unwrap()["project"]["name"], "live");

    // An unreadable live archive still falls back to the retained library copy.
    std::fs::write(&live, "interrupted write").unwrap();
    assert_eq!(service::credits(project.id.clone()).unwrap()["project"]["name"], "snapshot");
    std::fs::remove_file(&live).unwrap();
    assert_eq!(service::stamp(project.id.clone()).unwrap(), format!("0:{}", stored.last_analyzed.unwrap_or(0)));
    assert_eq!(service::credits("missing".into()).unwrap_err().code, "no_credits");
    assert_eq!(service::config_of("missing".into()).unwrap(), json!({}));

    service::remove_project(project.id.clone()).unwrap();
    assert!(service::projects().unwrap().as_array().unwrap().is_empty());
    assert!(lib.snapshot_json(&project.id).unwrap().is_some(), "removal keeps the archive for reattachment");
}

#[test]
fn settings_and_project_display_preserve_existing_fields() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("settings");
    let project = fixture.project();
    Library::open().unwrap().upsert_project(&project).unwrap();
    let config = Path::new(&project.path).join(".creditia/config.json");
    std::fs::write(&config, r#"{"branch":"release","custom":{"keep":true}}"#).unwrap();
    service::set_display(project.id.clone(), true, "— test".into()).unwrap();
    let value = service::config_of(project.id).unwrap();
    assert_eq!(value["branch"], "release");
    assert_eq!(value["custom"]["keep"], true);

    let exports = fixture.root.join("exports");
    std::fs::create_dir_all(&exports).unwrap();
    service::edit_app_settings(|settings| {
        settings["exportDir"] = json!(exports.display().to_string());
        settings["collect"] = json!({"on": false, "last": {"at": 42}});
        settings["logo"] = json!("star-mono");
    }).unwrap();
    service::edit_app_settings(|settings| settings["gpui"] = json!({"lang":"en"})).unwrap();
    assert_eq!(service::export_dir().unwrap(), exports);
    let settings = service::app_settings();
    assert_eq!(settings["collect"]["last"]["at"], 42);
    assert_eq!(settings["logo"], "star-mono");
    assert_eq!(settings["gpui"]["lang"], "en");
}

#[cfg(unix)]
#[test]
fn analysis_progress_warnings_errors_and_restart_use_the_same_worker_protocol() {
    use std::sync::Arc;
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("worker");
    fixture.core(r#"
[ "$CREDITIA_LANG" = "en" ] || exit 8
[ "$CREDITIA_PROGRESS" = "1" ] || exit 9
case " $* " in *" --github "*) ;; *) exit 10;; esac
echo '@progress {"phase":"git","current":2}' >&2
echo '@progress invalid-json' >&2
echo '{"ok":true}'
exit 2
"#);
    let values = Arc::new(Mutex::new(vec![]));
    let sink_values = values.clone();
    let result = service::run_analysis("/tmp", Some("en"), Some(Arc::new(move |value| sink_values.lock().unwrap().push(value)))).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(*values.lock().unwrap(), vec![json!({"phase":"git","current":2})]);

    fixture.core("echo '{\"error\":\"not a git repo\"}'; exit 1");
    let error = service::run_analysis("/tmp", None, None).unwrap_err();
    assert_eq!((error.code, error.detail.as_deref()), ("core_failed", Some("not a git repo")));
    fixture.core("echo invalid-json; echo 'worker diagnostic' >&2; exit 1");
    assert_eq!(service::run_analysis("/tmp", None, None).unwrap_err().detail.as_deref(), Some("worker diagnostic"));

    // Cancellation from the UI can precede worker dispatch. It must not launch
    // even a missing worker or affect a different operation started afterwards.
    let cancelled_before_dispatch = service::AnalysisCancellation::default();
    cancelled_before_dispatch.cancel();
    std::env::set_var("CREDITIA_CORE", fixture.root.join("missing"));
    assert_eq!(service::run_analysis_with_cancel("/tmp", None, None, cancelled_before_dispatch.clone()).unwrap_err().code, "cancelled");

    fixture.core("sleep 30; echo '{}'");
    let running_cancel = service::AnalysisCancellation::default();
    let worker_cancel = running_cancel.clone();
    let worker = std::thread::spawn(move || service::run_analysis_with_cancel("/tmp", None, None, worker_cancel));
    let start = std::time::Instant::now();
    while !service::analysis_running() && start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(service::analysis_running());
    assert_eq!(service::run_analysis("/tmp", None, None).unwrap_err().code, "busy");
    cancelled_before_dispatch.cancel();
    assert!(!running_cancel.is_cancelled(), "an old token cannot cancel the new job");
    let cancel_at = std::time::Instant::now();
    running_cancel.cancel();
    assert_eq!(worker.join().unwrap().unwrap_err().code, "cancelled");
    assert!(cancel_at.elapsed() < std::time::Duration::from_secs(5));
    assert!(!service::analysis_running());

    fixture.core("echo '{\"restarted\":true}'");
    assert_eq!(service::run_analysis("/tmp", None, None).unwrap()["restarted"], true);
    std::env::set_var("CREDITIA_CORE", fixture.root.join("missing"));
    assert_eq!(service::discover().unwrap_err().code, "core_missing");
}
