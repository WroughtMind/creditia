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
    previous_helper: Option<std::ffi::OsString>,
    previous_test_runner: Option<std::ffi::OsString>,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("creditia-service-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let previous_home = std::env::var_os("CREDITIA_HOME");
        let previous_core = std::env::var_os("CREDITIA_CORE");
        let previous_helper = std::env::var_os("CREDITIA_MIGRATION_HELPER");
        let previous_test_runner = std::env::var_os("CREDITIA_SERVICE_TEST_RUNNER");
        std::env::set_var("CREDITIA_HOME", root.join("library"));
        Self {
            root,
            previous_home,
            previous_core,
            previous_helper,
            previous_test_runner,
        }
    }
    fn project(&self) -> Project {
        let repo = self.root.join("project");
        std::fs::create_dir_all(repo.join(".creditia")).unwrap();
        Project {
            id: "existing-project".into(),
            name: "Existing project".into(),
            path: repo.display().to_string(),
            root_commit: "root".into(),
            remotes: vec!["https://example.org/project.git".into()],
            last_head: Some("previous-head".into()),
            last_analyzed: Some(42),
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
        for (key, previous) in [
            ("CREDITIA_HOME", &self.previous_home),
            ("CREDITIA_CORE", &self.previous_core),
            ("CREDITIA_MIGRATION_HELPER", &self.previous_helper),
            ("CREDITIA_SERVICE_TEST_RUNNER", &self.previous_test_runner),
        ] {
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
    lib.add_path(&project.id, "/previous/project", "linked")
        .unwrap();
    lib.save_snapshot(
        &project.id,
        "head",
        "2026-01-01",
        r#"{"project":{"name":"snapshot"}}"#,
    )
    .unwrap();
    let live = Path::new(&project.path).join(".creditia/credits.json");
    std::fs::write(&live, r#"{"project":{"name":"live"}}"#).unwrap();

    // Snapshot persistence advances project metadata in the existing store.
    let stored = lib
        .projects()
        .unwrap()
        .into_iter()
        .find(|p| p.id == project.id)
        .unwrap();
    let list = service::projects().unwrap();
    assert_eq!(list[0]["id"], project.id);
    assert_eq!(list[0]["lastHead"], json!(stored.last_head));
    assert_eq!(list[0]["lastAnalyzed"], json!(stored.last_analyzed));
    assert_eq!(list[0]["pathKinds"]["/previous/project"], "linked");
    assert_eq!(list[0]["exists"], true);
    assert_eq!(list[0]["hasCredits"], true);
    assert_eq!(
        service::credits(project.id.clone()).unwrap()["project"]["name"],
        "live"
    );

    // An unreadable live archive still falls back to the retained library copy.
    std::fs::write(&live, "interrupted write").unwrap();
    assert_eq!(
        service::credits(project.id.clone()).unwrap()["project"]["name"],
        "snapshot"
    );
    std::fs::remove_file(&live).unwrap();
    assert_eq!(
        service::stamp(project.id.clone()).unwrap(),
        format!("0:{}", stored.last_analyzed.unwrap_or(0))
    );
    assert_eq!(
        service::credits("missing".into()).unwrap_err().code,
        "no_credits"
    );
    assert_eq!(service::config_of("missing".into()).unwrap(), json!({}));

    service::remove_project(project.id.clone()).unwrap();
    assert!(service::projects().unwrap().as_array().unwrap().is_empty());
    assert!(
        lib.snapshot_json(&project.id).unwrap().is_some(),
        "removal keeps the archive for reattachment"
    );
}

#[test]
fn unavailable_integration_worker_is_distinct_from_disabled_integrations() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("integration-error");
    let project = fixture.project();
    Library::open().unwrap().upsert_project(&project).unwrap();
    std::env::set_var("CREDITIA_CORE", fixture.root.join("missing-core"));

    let details = service::project_details(project.id.clone()).unwrap();
    assert_eq!(details["project"]["id"], project.id);
    assert_eq!(details["project"]["exists"], true);
    assert!(details["integrations"].is_null());
    assert!(details["integrationsError"]["code"].is_string());
    assert!(details["integrationsError"]["detail"].is_string());
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
    service::set_project_config(
        "existing-project".into(),
        json!({"privacy":{"hidePaths":true,"aliases":{"person":"alias"}},"level":"strict"}),
    )
    .unwrap();
    let before_invalid = std::fs::read(&config).unwrap();
    assert!(service::set_project_config(
        "existing-project".into(),
        json!({"privacy":{"hidePaths":null}})
    )
    .is_err());
    assert!(
        service::set_project_config("existing-project".into(), json!({"repo":"/other"})).is_err()
    );
    assert_eq!(std::fs::read(&config).unwrap(), before_invalid);

    let exports = fixture.root.join("exports");
    std::fs::create_dir_all(&exports).unwrap();
    service::edit_app_settings(|settings| {
        settings["exportDir"] = json!(exports.display().to_string());
        settings["collect"] = json!({"on": false, "last": {"at": 42}});
        settings["logo"] = json!("star-mono");
    })
    .unwrap();
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
    fixture.core(
        r#"
[ "$CREDITIA_LANG" = "en" ] || exit 8
[ "$CREDITIA_PROGRESS" = "1" ] || exit 9
case " $* " in *" --github "*) ;; *) exit 10;; esac
echo '@progress {"phase":"git","current":2}' >&2
echo '@progress invalid-json' >&2
echo '{"ok":true}'
exit 2
"#,
    );
    let values = Arc::new(Mutex::new(vec![]));
    let sink_values = values.clone();
    let result = service::run_analysis(
        "/tmp",
        Some("en"),
        Some(Arc::new(move |value| {
            sink_values.lock().unwrap().push(value)
        })),
    )
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(
        *values.lock().unwrap(),
        vec![json!({"phase":"git","current":2})]
    );

    fixture.core("echo '{\"error\":\"not a git repo\"}'; exit 1");
    let error = service::run_analysis("/tmp", None, None).unwrap_err();
    assert_eq!(
        (error.code, error.detail.as_deref()),
        ("core_failed", Some("not a git repo"))
    );
    fixture.core("echo invalid-json; echo 'worker diagnostic' >&2; exit 1");
    assert_eq!(
        service::run_analysis("/tmp", None, None)
            .unwrap_err()
            .detail
            .as_deref(),
        Some("worker diagnostic")
    );

    // Cancellation from the UI can precede worker dispatch. It must not launch
    // even a missing worker or affect a different operation started afterwards.
    let cancelled_before_dispatch = service::AnalysisCancellation::default();
    cancelled_before_dispatch.cancel();
    std::env::set_var("CREDITIA_CORE", fixture.root.join("missing"));
    assert_eq!(
        service::run_analysis_with_cancel("/tmp", None, None, cancelled_before_dispatch.clone())
            .unwrap_err()
            .code,
        "cancelled"
    );

    fixture.core("sleep 30; echo '{}'");
    let running_cancel = service::AnalysisCancellation::default();
    let worker_cancel = running_cancel.clone();
    let worker = std::thread::spawn(move || {
        service::run_analysis_with_cancel("/tmp", None, None, worker_cancel)
    });
    let start = std::time::Instant::now();
    while !service::analysis_running() && start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(service::analysis_running());
    assert_eq!(
        service::run_analysis("/tmp", None, None).unwrap_err().code,
        "busy"
    );
    cancelled_before_dispatch.cancel();
    assert!(
        !running_cancel.is_cancelled(),
        "an old token cannot cancel the new job"
    );
    let cancel_at = std::time::Instant::now();
    running_cancel.cancel();
    assert_eq!(worker.join().unwrap().unwrap_err().code, "cancelled");
    assert!(cancel_at.elapsed() < std::time::Duration::from_secs(5));
    assert!(!service::analysis_running());

    // Even a progress callback that was already dispatched must finish before
    // cancellation reports completion and a replacement job can start.
    fixture.core("echo '@progress {\"phase\":\"git\"}' >&2; sleep 30; echo '{}'");
    let (progress_started, ready) = std::sync::mpsc::channel();
    let release_progress = Arc::new(std::sync::Barrier::new(2));
    let release = release_progress.clone();
    let token = service::AnalysisCancellation::default();
    let worker_token = token.clone();
    let (completed, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = service::run_analysis_with_cancel(
            "/tmp",
            None,
            Some(Arc::new(move |_| {
                progress_started.send(()).unwrap();
                release.wait();
            })),
            worker_token,
        );
        completed.send(()).unwrap();
        result
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    token.cancel();
    let premature_completion = completion
        .recv_timeout(std::time::Duration::from_millis(150))
        .is_ok();
    release_progress.wait();
    assert_eq!(worker.join().unwrap().unwrap_err().code, "cancelled");
    assert!(
        !premature_completion,
        "completion must wait for in-flight progress to drain"
    );

    fixture.core("echo '{\"restarted\":true}'");
    assert_eq!(
        service::run_analysis("/tmp", None, None).unwrap()["restarted"],
        true
    );
    std::env::set_var("CREDITIA_CORE", fixture.root.join("missing"));
    assert_eq!(service::discover().unwrap_err().code, "core_missing");
}

#[test]
fn legacy_preferences_round_trip_preserves_privacy_and_newer_edits() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let _fixture = Fixture::new("preference-migration");
    service::edit_app_settings(|settings| {
        settings["unknown"] = json!({"keep": 7});
        settings["gpui"] = json!({"language":"en","reducedMotion":true});
    })
    .unwrap();
    let projected = service::import_legacy_preferences(json!({
        "creditia.names":"off", "creditia.lang":"zh", "creditia.theme":"ink",
        "creditia.group.project.with.dot":"model", "creditia.autofresh.project.with.dot":"off",
        "creditia.custom-pref":"retained"
    }))
    .unwrap();
    assert_eq!(projected["creditia.names"], "off");
    let prefs = service::ui_preferences();
    assert_eq!(prefs["global"]["named"], false);
    assert_eq!(prefs["global"]["style"], "ink");
    assert_eq!(prefs["global"]["reducedMotion"], true);
    assert_eq!(prefs["projects"]["project.with.dot"]["group"], "model");
    assert_eq!(prefs["projects"]["project.with.dot"]["autoAnalyze"], false);
    assert_eq!(service::app_settings()["unknown"]["keep"], 7);

    service::patch_ui_preferences(json!({"global":{"style":"bauhaus","lang":"en"},"projects":{"project.with.dot":{"autoAnalyze":null}}})).unwrap();
    let again = service::import_legacy_preferences(
        json!({"creditia.theme":"ink","creditia.lang":"zh","creditia.names":"on"}),
    )
    .unwrap();
    assert_eq!(
        again["creditia.theme"], "bauhaus",
        "reopening legacy client cannot overwrite GPUI edits"
    );
    assert_eq!(again["creditia.lang"], "en");
    assert!(
        again.get("creditia.autofresh.project.with.dot").is_none(),
        "inherit clears stale override in legacy projection"
    );
    assert_eq!(again["creditia.custom-pref"], "retained");
    service::set_legacy_preference("creditia.names".into(), "on".into()).unwrap();
    assert_eq!(service::ui_preferences()["global"]["named"], true);
    assert!(service::patch_ui_preferences(json!({"global":{"named":"false"}})).is_err());
    assert_eq!(service::ui_preferences()["global"]["named"], true);
}

#[test]
fn corrupted_settings_are_not_overwritten_and_future_versions_are_preserved() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let _fixture = Fixture::new("settings-invalid");
    let directory = creditia_store::data_dir();
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("app.json");
    std::fs::write(&path, b"{unfinished").unwrap();
    assert_eq!(
        service::edit_app_settings(|s| s["logo"] = json!("star"))
            .unwrap_err()
            .code,
        "settings_invalid"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{unfinished");
    std::fs::write(&path, r#"{"ui":{"version":2,"unknown":"future"},"keep":9}"#).unwrap();
    let future = std::fs::read(&path).unwrap();
    assert_eq!(
        service::patch_ui_preferences(json!({"global":{"named":false}}))
            .unwrap_err()
            .code,
        "preferences_version"
    );
    assert_eq!(
        service::import_legacy_preferences(json!({"creditia.names":"on"}))
            .unwrap_err()
            .code,
        "preferences_version"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        future,
        "unsupported versions are not rewritten"
    );
    assert_eq!(service::app_settings()["ui"]["unknown"], "future");
    for malformed in [
        json!({"version":1,"legacy":[]}),
        json!({"version":"future"}),
        json!({"version":1,"projects":{"a":false}}),
    ] {
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({"ui":malformed,"keep":9})).unwrap(),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(service::import_legacy_preferences(json!({"creditia.names":"on"})).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

// Invoked as an isolated test subprocess by the following test. This exercises
// real OS file locks rather than an in-process mutex pretending to be one.
#[test]
fn settings_child_process_writer() {
    if std::env::var_os("CREDITIA_SERVICE_TEST_WRITER").is_none() {
        return;
    }
    for _ in 0..25 {
        service::edit_app_settings(|settings| {
            settings["counter"] = json!(settings["counter"].as_u64().unwrap_or(0) + 1)
        })
        .unwrap();
    }
}

#[test]
fn concurrent_desktop_processes_merge_settings_without_partial_json() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let _fixture = Fixture::new("settings-processes");
    service::edit_app_settings(|settings| {
        settings["counter"] = json!(0);
        settings["unknown"] = json!({"keep":true});
    })
    .unwrap();
    let mut children = (0..4)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "settings_child_process_writer"])
                .env("CREDITIA_SERVICE_TEST_WRITER", "1")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut finished = vec![false; children.len()];
    let started = std::time::Instant::now();
    while finished.iter().any(|done| !done) {
        let bytes = std::fs::read(creditia_store::data_dir().join("app.json")).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("every reader sees a complete file");
        assert_eq!(value["unknown"]["keep"], true);
        for (child, done) in children.iter_mut().zip(&mut finished) {
            if !*done {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success());
                    *done = true;
                }
            }
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(service::app_settings()["counter"], 100);
}

#[cfg(unix)]
#[test]
fn migration_child_process() {
    match std::env::var("CREDITIA_SERVICE_TEST_MIGRATOR").as_deref() {
        Ok("launch") => {
            service::migrate_legacy_with_helper().unwrap();
        }
        Ok("import") => {
            // Give the other desktop time to contend for the migration lease.
            std::thread::sleep(std::time::Duration::from_millis(100));
            service::import_legacy_preferences(
                json!({"creditia.names":"off","creditia.group.original.project":"model"}),
            )
            .unwrap();
            service::edit_app_settings(|settings| {
                settings["helperRuns"] = json!(settings["helperRuns"].as_u64().unwrap_or(0) + 1)
            })
            .unwrap();
        }
        _ => {}
    }
}

#[cfg(unix)]
#[test]
fn simultaneous_desktops_run_one_migration_helper_and_failed_import_keeps_privacy() {
    use std::os::unix::fs::PermissionsExt;
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("migration-processes");
    let helper = fixture.root.join("migration-helper");
    std::fs::write(&helper, "#!/bin/sh\n[ \"$1\" = --migrate-ui-settings ] || exit 9\nCREDITIA_SERVICE_TEST_MIGRATOR=import exec \"$CREDITIA_SERVICE_TEST_RUNNER\" --exact migration_child_process\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("CREDITIA_MIGRATION_HELPER", &helper);
    std::env::set_var(
        "CREDITIA_SERVICE_TEST_RUNNER",
        std::env::current_exe().unwrap(),
    );
    let mut desktops = (0..2)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "migration_child_process"])
                .env("CREDITIA_SERVICE_TEST_MIGRATOR", "launch")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    for desktop in &mut desktops {
        assert!(desktop.wait().unwrap().success());
    }
    assert_eq!(service::app_settings()["helperRuns"], 1);
    assert_eq!(service::ui_preferences()["global"]["named"], false);
    assert_eq!(
        service::ui_preferences()["projects"]["original.project"]["group"],
        "model"
    );

    std::fs::write(
        creditia_store::data_dir().join("app.json"),
        r#"{"keep":"original"}"#,
    )
    .unwrap();
    std::fs::write(&helper, "#!/bin/sh\nexit 1\n").unwrap();
    assert_eq!(
        service::migrate_legacy_with_helper().unwrap_err().code,
        "migration_failed"
    );
    assert_eq!(service::ui_preferences()["global"]["named"], false);
    assert_eq!(service::app_settings(), json!({"keep":"original"}));
}

#[cfg(unix)]
#[test]
fn durable_analysis_recovers_after_exit_and_retries_the_original_request() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("task-recovery");
    service::edit_app_settings(|settings| {
        settings["tasks"]["analysis"] =
            json!({"id":"exited-owner","state":"running","repo":"/tmp","lang":"en","startedAt":1})
    })
    .unwrap();
    assert_eq!(
        service::recover_interrupted_analysis().unwrap()["state"],
        "interrupted"
    );
    fixture.core("[ \"$CREDITIA_LANG\" = en ] || exit 9; echo '{\"resumed\":true}'");
    let result = service::resume_analysis(None, service::AnalysisCancellation::default()).unwrap();
    assert_eq!(result["resumed"], true);
    assert_eq!(service::analysis_status()["state"], "completed");
    assert_eq!(service::analysis_status()["repo"], "/tmp");
    assert_eq!(
        service::resume_analysis(None, service::AnalysisCancellation::default())
            .unwrap_err()
            .code,
        "no_interrupted_task"
    );

    fixture.core("sleep 30; echo '{}'");
    let token = service::AnalysisCancellation::default();
    let worker_token = token.clone();
    let job = std::thread::spawn(move || {
        service::run_analysis_with_cancel("/tmp", None, None, worker_token)
    });
    let start = std::time::Instant::now();
    while !service::analysis_running() && start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(service::analysis_running());
    assert_eq!(
        service::recover_interrupted_analysis().unwrap()["state"],
        "running",
        "a live process lease prevents false recovery"
    );
    token.cancel();
    assert_eq!(job.join().unwrap().unwrap_err().code, "cancelled");
    assert_eq!(service::analysis_status()["state"], "cancelled");
}

#[cfg(unix)]
#[test]
fn closing_background_service_cancels_its_analysis_before_later_writes() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("background-cancel");
    let project = fixture.project();
    let repo = Path::new(&project.path);
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(repo)
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "fixture"
        ])
        .status()
        .unwrap()
        .success());
    Library::open().unwrap().upsert_project(&project).unwrap();
    std::fs::write(
        repo.join(".creditia/credits.json"),
        r#"{"head":"previous","project":{"name":"fixture"}}"#,
    )
    .unwrap();
    let marker = fixture.root.join("should-not-exist");
    fixture.core(&format!(
        "echo '@progress {{\"stage\":\"history\"}}' >&2; sleep 30; touch '{}'; echo '{{}}'",
        marker.display()
    ));
    let (sender, receiver) = std::sync::mpsc::channel();
    let background = service::BackgroundService::start(std::sync::Arc::new(move |event| {
        let _ = sender.send(event);
    }));
    background.watch_project(Some(project.id));
    loop {
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("automatic analysis starts");
        if event["kind"] == "analysis-progress" {
            break;
        }
    }
    drop(background);
    let start = std::time::Instant::now();
    while service::analysis_running() && start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!service::analysis_running());
    assert!(!marker.exists());
    assert_eq!(service::analysis_status()["state"], "cancelled");
}

#[cfg(unix)]
#[test]
fn explicit_background_cancel_waits_for_a_new_head_despite_forced_refresh() {
    use std::time::{Duration, Instant};
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = Fixture::new("background-user-cancel");
    let project = fixture.project();
    let repo = Path::new(&project.path);
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(repo)
        .status()
        .unwrap()
        .success());
    let commit = |message: &str| {
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                message
            ])
            .status()
            .unwrap()
            .success());
    };
    commit("first");
    let library = Library::open().unwrap();
    library.upsert_project(&project).unwrap();
    std::fs::write(
        repo.join(".creditia/credits.json"),
        r#"{"head":"previous","project":{"name":"fixture"}}"#,
    )
    .unwrap();
    fixture.core("echo '@progress {\"stage\":\"history\"}' >&2; sleep 30; echo '{}'");
    let (sender, receiver) = std::sync::mpsc::channel();
    let background = service::BackgroundService::start(std::sync::Arc::new(move |event| {
        let _ = sender.send(event);
    }));
    background.watch_project(Some(project.id.clone()));
    let wait_for = |kind: &str| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "expected {kind}");
            match receiver.recv_timeout(Duration::from_millis(30)) {
                Ok(event) if event["kind"] == kind => return event,
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => background.refresh(),
                Err(error) => panic!("background channel closed: {error}"),
            }
        }
    };
    let generation = wait_for("analysis-progress")["generation"].as_u64().unwrap();
    assert!(!background.cancel_analysis(&project.id, generation.wrapping_sub(1)));
    assert!(!background.cancel_analysis("different-project", generation));
    assert!(background.cancel_analysis(&project.id, generation));
    assert_eq!(wait_for("analysis-finished")["cancelled"], true);
    // Change the archive stamp to prove the worker actually scans again; merely
    // sleeping after cancellation would not establish suppression under refresh.
    library
        .save_snapshot(&project.id, "previous", "2026-01-01", "{}")
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut refreshed = false;
    while Instant::now() < deadline {
        background.refresh();
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(20)) {
            assert_ne!(
                event["kind"], "analysis-started",
                "explicitly cancelled HEAD must stay stopped"
            );
            refreshed |= event["kind"] == "archive-changed";
        }
    }
    assert!(
        refreshed,
        "forced refresh observed the changed archive stamp"
    );
    assert!(!service::analysis_running());
    commit("new work");
    wait_for("analysis-progress");
    drop(background);
    let deadline = Instant::now() + Duration::from_secs(5);
    while service::analysis_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!service::analysis_running());
}
