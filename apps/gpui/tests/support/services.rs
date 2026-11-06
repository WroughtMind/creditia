//! Production project/settings callbacks against a real isolated SQLite library.
//! The environment is set once, before GPUI starts any threads, and stays fixed
//! until process exit. Archive input is declared synthetic; no migration or
//! native file dialog is represented as exercised by this suite.
use creditia_desktop_service as service;
use creditia_gpui::app::Creditia;
use creditia_store::{Library, Project};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, ElementId, Entity, HeadlessAppContext, ScrollDelta,
    Window, WindowBounds, WindowOptions, assets::Assets, point, px, size,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

const ALPHA: &str = "native-services-alpha";
const BETA: &str = "native-services-beta";

struct Workspace {
    root: PathBuf,
    alpha: Project,
    beta: Project,
    archive_bytes: Vec<u8>,
}
impl Workspace {
    fn new(raw: &Value) -> Self {
        let root =
            std::env::temp_dir().join(format!("creditia-native-services-{}", std::process::id()));
        assert!(
            !root.exists(),
            "refuse to reuse another service test workspace"
        );
        fs::create_dir_all(&root).unwrap();
        // SAFETY: this function runs before any GPUI context/executor/service
        // worker exists. This process never changes its environment again.
        unsafe { std::env::set_var("CREDITIA_HOME", root.join("library")) };
        service::import_legacy_preferences(json!({})).unwrap();
        service::patch_ui_preferences(json!({"global":{
            "autoAnalyze":false,"lang":"en","named":false,"appearance":"light",
            "reducedMotion":true,"lastProject":ALPHA,"style":"minimal-light","view":"wall"
        }}))
        .unwrap();
        service::edit_app_settings(|settings| settings["collect"] = json!({"on":false})).unwrap();
        let create = |id: &str, name: &str, directory: &str| {
            let path = root.join(directory);
            fs::create_dir_all(&path).unwrap();
            git(&path, &["init", "--initial-branch=main"]);
            fs::write(
                path.join("README.md"),
                format!("# {name}\nIsolated native UI fixture.\n"),
            )
            .unwrap();
            git(&path, &["add", "README.md"]);
            git(
                &path,
                &[
                    "-c",
                    "user.name=Native UI Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "-m",
                    "Fixture initial commit",
                ],
            );
            let head = git(&path, &["rev-parse", "HEAD"]);
            fs::create_dir_all(path.join(".creditia")).unwrap();
            let mut archive = raw.clone();
            archive["project"]["name"] = json!(name);
            archive["project"]["id"] = json!(id);
            archive["head"] = json!(head);
            let bytes = serde_json::to_vec_pretty(&archive).unwrap();
            fs::write(path.join(".creditia/credits.json"), &bytes).unwrap();
            fs::write(
                path.join(".creditia/config.json"),
                "{\"custom\":{\"retained\":true}}\n",
            )
            .unwrap();
            let project = Project {
                id: id.into(),
                name: name.into(),
                path: path.display().to_string(),
                root_commit: head.clone(),
                remotes: vec![],
                last_head: Some(head.clone()),
                last_analyzed: Some(1_770_000_000),
            };
            let library = Library::open().unwrap();
            library.upsert_project(&project).unwrap();
            library
                .save_snapshot(
                    id,
                    &head,
                    "2026-02-02T00:00:00Z",
                    &String::from_utf8(bytes).unwrap(),
                )
                .unwrap();
            let cache = creditia_store::cache_dir(id);
            fs::create_dir_all(&cache).unwrap();
            fs::write(cache.join("retained.txt"), "preserve analysis cache\n").unwrap();
            project
        };
        let alpha = create(ALPHA, "Alpha Studio", "path-only-first");
        let beta = create(BETA, "Beta Studio", "path-only-second");
        // Seed a real retained evidence row before application workers exist.
        // This is fixture setup, not a substitute for a UI service callback.
        let seed = Command::new("python")
            .args(["-c", "import sqlite3,sys\nc=sqlite3.connect(sys.argv[1])\nc.execute('insert into evidence(project,rel,key,src,model,session,ts,tab) values(?,?,?,?,?,?,?,?)',(sys.argv[2],'README.md',17,'claude-code','fixture-model','native-fixture-session',1770000000,0))\nc.commit()\nc.close()"])
            .arg(Library::open().unwrap().path()).arg(BETA).output().unwrap();
        assert!(
            seed.status.success(),
            "evidence fixture: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
        assert_eq!(
            Library::open()
                .unwrap()
                .load_evidence(BETA)
                .unwrap()
                .0
                .len(),
            1
        );
        let archive_bytes = fs::read(Path::new(&beta.path).join(".creditia/credits.json")).unwrap();
        Self {
            root,
            alpha,
            beta,
            archive_bytes,
        }
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("installed Git");
    assert!(
        result.status.success(),
        "Git fixture setup: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}

struct Harness {
    // Drop entity/window references before their application context.
    view: Option<Entity<Creditia>>,
    handle: Option<AnyWindowHandle>,
    output: PathBuf,
    key: String,
    artifacts: Vec<Value>,
    features: Vec<Value>,
    cx: HeadlessAppContext,
}
impl Harness {
    fn new(output: &Path) -> Self {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(Assets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(gpui_kit::init);
        cx.update(|cx| creditia_gpui::app::init_fonts(cx).expect("bundled UI fonts"));
        let mut harness = Self {
            view: None,
            handle: None,
            output: output.into(),
            key: String::new(),
            artifacts: vec![],
            features: vec![],
            cx,
        };
        harness.open();
        harness
    }
    fn open(&mut self) {
        assert!(self.handle.is_none());
        let (handle, view) = self
            .cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: Default::default(),
                            size: size(px(1440.), px(1000.)),
                        })),
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| Creditia::new(window, cx)),
                )
            })
            .expect("actual production service window");
        self.handle = Some(handle);
        self.view = Some(view);
    }
    fn close(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.cx
                .update_window(handle, |_, window, _| window.remove_window())
                .unwrap();
            drop(self.view.take());
            self.cx.run_until_parked();
        }
    }
    fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<Creditia>) -> R) -> R {
        let view = self.view.as_ref().unwrap().clone();
        self.cx
            .update_window(self.handle.unwrap(), |_, window, cx| f(window, cx, &view))
            .unwrap()
    }
    fn state(&mut self) -> Value {
        self.update(|_, cx, view| view.read(cx).diagnostic_state())
    }
    fn pump(&mut self) {
        self.cx.run_until_parked();
        self.update(|window, cx, _| window.render_frame(cx));
    }
    fn wait(&mut self, description: &str, predicate: impl Fn(&Value) -> bool) {
        let started = Instant::now();
        loop {
            self.pump();
            let state = self.state();
            assert!(
                state["error"].is_null(),
                "{description}: production error: {state}"
            );
            if predicate(&state) {
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(90),
                "{description}: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn ready(&mut self, id: &str) {
        self.wait("real project archive loads", |state| {
            state["project"] == id
                && state["loading"] == false
                && state["tile_count"].as_u64().unwrap_or(0) > 0
        });
        let started = Instant::now();
        loop {
            self.pump();
            if self.update(|window, cx, view| {
                view.update(cx, |view, cx| view.fixture_images_ready(window, cx))
            }) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(90),
                "real project artwork assets: {}",
                self.state()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn click(&mut self, id: impl Into<ElementId>) {
        let id = id.into();
        self.update(|window, cx, _| window.click(id, cx));
    }
    fn press(&mut self, key: &str) {
        self.update(|window, cx, _| window.press(key, cx));
    }
    fn shortcut(&mut self, key: &str) {
        self.press(&format!(
            "{}-{key}",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        ));
    }
    fn input(&mut self, id: &str, text: &str) {
        self.click(id);
        self.shortcut("a");
        self.update(|window, cx, _| window.input(text, cx));
    }
    fn project_index(&self, id: &str) -> usize {
        service::projects()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["id"] == id)
            .unwrap()
    }
    fn switch(&mut self, id: &str) {
        self.click("switch-project");
        assert_eq!(self.state()["panel"], "projects");
        self.click(("choose-project", self.project_index(id)));
        self.ready(id);
    }
    fn scroll_to(&mut self, id: &str) {
        for _ in 0..20 {
            let visible = self.update(|window, _, _| {
                let body = window.find("panel-body").bounds();
                window.try_find(id).is_some_and(|item| {
                    let bounds = item.bounds();
                    item.visible()
                        && bounds.origin.y >= body.origin.y
                        && bounds.origin.y + bounds.size.height <= body.origin.y + body.size.height
                })
            });
            if visible {
                return;
            }
            self.update(|window, cx, _| {
                window.scroll(
                    "panel-body",
                    ScrollDelta::Pixels(point(px(0.), px(-160.))),
                    cx,
                )
            });
        }
        panic!("settings control did not enter visible area: {id}");
    }
    fn snapshot(&mut self, label: &str) {
        self.pump();
        let screenshot = self
            .cx
            .capture_screenshot(self.handle.unwrap())
            .expect("actual native screenshot");
        let colors = screenshot
            .as_raw()
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect::<HashSet<_>>();
        assert!(colors.len() > 64, "native service window is blank");
        let name = format!("{}-{label}.png", self.key);
        screenshot.save(self.output.join(&name)).unwrap();
        self.artifacts.push(json!({"path":name}));
    }
    fn feature(&mut self, id: &str, check: &str, behavior: &str) {
        self.features.push(json!({"feature_id":id,"check":check,"passed":true,"behavior":behavior,"test_id":self.key}));
    }
    fn record(&mut self, label: &str, data: Value) {
        let name = format!("{}-{label}.json", self.key);
        fs::write(
            self.output.join(&name),
            serde_json::to_vec_pretty(&data).unwrap(),
        )
        .unwrap();
        self.artifacts.push(json!({"path":name}));
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.close();
    }
}

fn search_and_switch(h: &mut Harness, workspace: &Workspace) {
    h.ready(ALPHA);
    assert_eq!(h.state()["project_name"], "Alpha Studio");
    h.record(
        "input-archives",
        json!({"kind":"synthetic-service-library","projects":[
        {"project":workspace.alpha,"archive":service::credits(ALPHA.into()).unwrap()},
        {"project":workspace.beta,"archive":service::credits(BETA.into()).unwrap()}],
        "evidence_rows":Library::open().unwrap().evidence_count(BETA)}),
    );
    h.click("roam");
    h.ready(ALPHA);
    h.click("switch-project");
    h.update(|window, _, _| assert!(window.find("all-projects").visible()));
    h.feature(
        "aggregate",
        "多个项目时显示全部项目入口",
        "真实 SQLite 项目库包含两个项目时，原生项目选择器显示全部项目入口",
    );
    h.input("project-search", "Beta Studio");
    h.update(|window, _, _| {
        assert!(window.find(("choose-project", h_index(BETA))).visible());
        assert!(
            window
                .try_find(("choose-project", h_index(ALPHA)))
                .is_none()
        );
    });
    h.snapshot("name-search");
    h.input("project-search", "path-only-second");
    h.update(|window, _, _| {
        assert!(window.find(("choose-project", h_index(BETA))).visible());
        assert!(
            window
                .try_find(("choose-project", h_index(ALPHA)))
                .is_none()
        );
    });
    h.feature(
        "project-switcher",
        "按名称和路径搜索",
        "向真实项目搜索框分别输入项目名与仅路径包含的字符串，验证只有目标项目按钮保留",
    );
    let before = h.state();
    h.press("left");
    assert_eq!(h.state()["pan_x"], before["pan_x"]);
    assert_eq!(h.state()["selected_tile"], before["selected_tile"]);
    h.feature(
        "keyboard",
        "输入框不被漫游方向键抢占",
        "项目搜索输入框获得焦点时 Left 只移动输入光标，画布坐标和所选作品不变",
    );
    h.press("down");
    assert_eq!(h.state()["picker_selected"], BETA);
    h.press("enter");
    h.ready(BETA);
    assert!(h.state()["panel"].is_null());
    assert_eq!(h.state()["project_name"], "Beta Studio");
    h.snapshot("opened-beta");
    h.click("switch-project");
    h.update(|window, cx, _| window.click_at("artwork-stage", point(px(4.), px(300.)), cx));
    assert!(
        h.state()["panel"].is_null(),
        "click outside the project picker closes it"
    );
    h.feature("project-switcher","方向键选择、回车打开、点击外部关闭","搜索框 Down 选择目标、Enter 读取对应真实档案，重新打开选择器后在模态外派发原生鼠标点击使其关闭");
    let state = h.state();
    h.record("loaded-state",json!({"state":state,"stamp":service::stamp(BETA.into()).unwrap(),"archive_name":service::credits(BETA.into()).unwrap()["project"]["name"]}));
}
fn h_index(id: &str) -> usize {
    service::projects()
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == id)
        .unwrap()
}

fn settings_and_persistence(h: &mut Harness, _: &Workspace) {
    h.click("group-model");
    h.ready(BETA);
    h.wait("group preference stored", |_| {
        service::ui_preferences()["projects"][BETA]["group"] == "model"
    });
    h.switch(ALPHA);
    assert_eq!(h.state()["group"], "auto");
    h.click("group-tool");
    h.ready(ALPHA);
    h.wait("second group stored", |_| {
        service::ui_preferences()["projects"][ALPHA]["group"] == "tool"
    });
    h.switch(BETA);
    assert_eq!(h.state()["group"], "model");
    h.feature(
        "grouping",
        "逐项目记住偏好",
        "两个真实项目分别设置模型和工具分组，切换回原项目后恢复独立偏好，并读回共享偏好文件核对",
    );
    h.click("switch-project");
    h.click(("project-more", h.project_index(BETA)));
    assert_eq!(h.state()["project_row_menu"], BETA);
    h.update(|window, _, _| {
        let bounds = window.find("project-row-menu").bounds();
        assert!(
            bounds.origin.x >= px(0.)
                && bounds.origin.y >= px(0.)
                && bounds.origin.x + bounds.size.width <= px(1440.)
                && bounds.origin.y + bounds.size.height <= px(1000.)
        );
    });
    h.snapshot("project-more");
    h.click("row-settings");
    assert_eq!(h.state()["panel"], "project");
    assert_eq!(h.state()["settings_project"], BETA);
    h.feature(
        "project-row-menu",
        "项目设置",
        "从真实项目行更多菜单打开对应项目设置，目标项目 ID 与菜单所属项目相同",
    );
    h.wait("project details ready", |state| {
        state["project_settings_ready"] == true && state["operation_busy"] == false
    });
    h.input("project-signature", "Native signature 测试");
    h.click("save-display");
    h.wait("signature saved", |state| {
        state["operation_busy"] == false
            && service::config_of(BETA.into()).unwrap()["display"]["sign"]
                == "Native signature 测试"
    });
    assert_eq!(
        service::config_of(BETA.into()).unwrap()["custom"]["retained"],
        true
    );
    for (index, expected) in [(1, Some(true)), (2, Some(false)), (0, None)] {
        h.click(("project-auto", index as usize));
        h.wait("project auto-analysis override stored", |_| {
            service::ui_preferences()["projects"][BETA]["autoAnalyze"].as_bool() == expected
        });
    }
    h.feature(
        "project-settings",
        "跟随默认、自动跟上、关闭",
        "点击项目设置中的自动、手动、跟随全局三个真实按钮，并逐次读回持久配置的 true、false、null",
    );
    h.snapshot("saved-project-settings");
    h.click("panel-close");
    h.click("settings");
    h.click("language");
    h.click("appearance");
    h.click("names-settings");
    h.click("ring");
    h.click("auto-analyze");
    h.wait("global default enabled", |_| {
        service::ui_preferences()["global"]["autoAnalyze"] == true
    });
    h.click("auto-analyze");
    h.wait("preferences stored", |_| {
        let p = service::ui_preferences();
        p["global"]["lang"] == "zh"
            && p["global"]["appearance"] == "dark"
            && p["global"]["named"] == true
            && p["global"]["logo"] == "ring"
            && p["global"]["autoAnalyze"] == false
    });
    h.feature(
        "settings-general",
        "自动跟上新提交默认值",
        "真实全局设置开关先启用再关闭自动分析，持久偏好文件同步为 true 和 false",
    );
    h.snapshot("global-settings");
    h.click("panel-close");
    h.close();
    h.open();
    h.ready(BETA);
    let state = h.state();
    assert_eq!(state["language"], "zh");
    assert_eq!(state["appearance"], "dark");
    assert_eq!(state["named"], true);
    assert_eq!(state["group"], "model");
    h.record("reopened-preferences",json!({"state":state,"preferences":service::ui_preferences(),"project_config":service::config_of(BETA.into()).unwrap()}));
    h.snapshot("reopened");
    h.feature("settings-general","偏好持久化","关闭真实生产窗口并释放视图，再以 Creditia::new 创建新窗口，语言、主题、姓名、项目分组和所选项目从持久文件恢复");
}

fn remove_projects(h: &mut Harness, workspace: &Workspace) {
    h.shortcut("i");
    assert_eq!(h.state()["panel"], "project");
    h.scroll_to("remove-project");
    h.click("remove-project");
    assert_eq!(h.state()["remove_confirm"], true);
    assert_eq!(service::projects().unwrap().as_array().unwrap().len(), 2);
    h.snapshot("removal-confirmation");
    h.feature(
        "project-settings",
        "移除前确认",
        "首次点击移除后显示确认状态，SQLite 项目列表仍保留两个项目，只有第二次确认才执行移除",
    );
    h.click("remove-project");
    h.ready(ALPHA);
    assert_eq!(service::projects().unwrap().as_array().unwrap().len(), 1);
    assert_eq!(
        fs::read(Path::new(&workspace.beta.path).join(".creditia/credits.json")).unwrap(),
        workspace.archive_bytes
    );
    assert!(
        Library::open()
            .unwrap()
            .snapshot_json(BETA)
            .unwrap()
            .is_some()
    );
    let evidence = Library::open().unwrap().load_evidence(BETA).unwrap().0;
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].rel, "README.md");
    assert_eq!(evidence[0].key, 17);
    assert_eq!(evidence[0].session, "native-fixture-session");
    assert_eq!(
        fs::read_to_string(creditia_store::cache_dir(BETA).join("retained.txt")).unwrap(),
        "preserve analysis cache\n"
    );
    assert!(Path::new(&workspace.beta.path).join(".git/HEAD").is_file());
    h.record("removed-project-storage",json!({"projects":service::projects().unwrap(),"archive_unchanged":true,"snapshot_retained":true,"cache_retained":true,"git_head":git(Path::new(&workspace.beta.path),&["rev-parse","HEAD"])}));
    h.feature(
        "remove-readd",
        "只移除列表记录",
        "通过原生项目设置确认移除后，真实 SQLite 可见项目列表减少一项，另一项目自动加载",
    );
    h.feature(
        "remove-readd",
        "保留本地证据、缓存和仓库文件",
        "移除后逐字节核对原档案不变，并验证数据库快照、缓存标记与 Git 仓库文件均保留",
    );
    h.shortcut("i");
    h.scroll_to("remove-project");
    h.click("remove-project");
    h.click("remove-project");
    h.wait("last project removed", |state| {
        state["project_count"] == 0 && state["loading"] == false
    });
    h.update(|window, _, _| assert!(window.find("first-project").visible()));
    assert_eq!(service::projects().unwrap().as_array().unwrap().len(), 0);
    assert!(
        Path::new(&workspace.alpha.path)
            .join(".creditia/credits.json")
            .is_file()
    );
    h.snapshot("empty-library");
    h.feature(
        "remove-readd",
        "最后一个项目移除后回首次打开",
        "通过第二次真实移除使项目库清空，生产界面显示首次打开按钮且数据库列表为空",
    );
}

pub fn run() {
    let input = PathBuf::from(
        std::env::var("CREDITIA_VISUAL_FIXTURES").expect("canonical fixture directory"),
    );
    let output = PathBuf::from(
        std::env::var("CREDITIA_SERVICE_OUTPUT").expect("service evidence directory"),
    );
    fs::create_dir_all(&output).unwrap();
    let fixture: Value =
        serde_json::from_slice(&fs::read(input.join("minimal-light-en.json")).unwrap()).unwrap();
    let source: Value =
        serde_json::from_slice(&fs::read(input.join("fixture-manifest.json")).unwrap()).unwrap();
    let workspace = Workspace::new(&fixture["raw"]);
    let mut cases = vec![];
    let mut h = Harness::new(&output);
    let scenarios: [(&str, fn(&mut Harness, &Workspace)); 3] = [
        ("services-search", search_and_switch),
        ("services-persistence", settings_and_persistence),
        ("services-removal", remove_projects),
    ];
    let mut failed = false;
    for (key, scenario) in scenarios {
        if failed {
            cases.push(json!({"key":key,"status":"not_run","checks":[],"artifacts":[]}));
            continue;
        }
        h.key = key.into();
        h.artifacts.clear();
        h.features.clear();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scenario(&mut h, &workspace)
        }));
        let error = result.err().map(|e| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "native service case panicked".into())
        });
        if error.is_some() {
            failed = true;
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.snapshot("failed")));
        }
        cases.push(json!({"key":key,"status":if error.is_none(){"passed"}else{"failed"},"error":error,"state":h.state(),"checks":[{"id":"real-library-native-input-callbacks","passed":!failed}],"feature_checks":if failed{vec![]}else{h.features.clone()},"artifacts":h.artifacts}));
    }
    h.close();
    service::cancel_analyze();
    let manifest = json!({"schema_version":1,"kind":"native-ui","provenance":{"candidate_sha":std::env::var("GITHUB_SHA").ok(),"environment":{"platform":std::env::consts::OS,"arch":std::env::consts::ARCH},"fixture":source["fixture"],"profile":"real-services-isolated-library"},"method":"Real Creditia::new, native pointer/keyboard input, SQLite store, Git repositories, preference writer and shared artwork worker. Synthetic archives; existing-user migration and OS dialogs are not exercised. CREDITIA_HOME stays isolated for this entire process.","workspace":workspace.root,"cases":cases,"complete":!failed});
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        !failed,
        "production service-backed native UI acceptance failed"
    );
}
