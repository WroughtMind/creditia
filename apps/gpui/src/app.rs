//! Production desktop view. Fixture construction uses the same render tree and
//! event handlers while keeping project data and filesystem services isolated.
use crate::{
    Appearance, Language, Preferences, Project, export_filename, export_with_preferences,
    progress_text,
};
use crate::{native_preview, renderer, ui_model::*};
use creditia_desktop_service as service;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, FocusTrapElement, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

actions!(
    creditia,
    [
        Quit,
        OpenProject,
        Refresh,
        Analyze,
        CancelAnalysis,
        CancelCurrent,
        CopyArchive,
        RevealProject,
        ToggleLanguage,
        CycleAppearance,
        ToggleMotion,
        ExportArtwork,
        CancelExport,
        SwitchProject,
        Settings,
        ProjectSettings,
        Wall,
        Roam,
        PreviousStyle,
        NextStyle,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        Play,
        Replay,
        PreviousPiece,
        NextPiece,
        MinimizeWindow,
        FullScreen
    ]
);
#[derive(Clone, Copy)]
enum ExportStage {
    ChoosePath,
    Rendering,
    Writing,
}
enum JobEvent {
    Progress(Value),
    Finished(Result<Value, service::Fail>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Projects,
    Styles,
    Pieces,
    Layouts,
    Settings,
    Project,
    Details,
    Diagnostics,
    Blame,
    Exports,
}
#[derive(Clone)]
struct Tile {
    spec: TileSpec,
    art: Option<Arc<Artwork>>,
    image: Option<Arc<Image>>,
    image_region: Option<Rect>,
    pixels: u32,
    texture_bytes: usize,
    error: Option<String>,
    loading: bool,
}
struct Playback {
    frames: BTreeMap<usize, (Arc<Artwork>, Arc<Image>)>,
    times: Vec<f64>,
    timeline: Timeline,
    started: Instant,
    elapsed: Duration,
    playing: bool,
    index: usize,
    requested: usize,
    epoch: u64,
    started_once: bool,
}
#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    started: Instant,
}
impl Tween {
    fn value(self) -> f32 {
        let t = (self.started.elapsed().as_secs_f32() / 0.16).min(1.);
        self.from + (self.to - self.from) * (1. - (1. - t).powi(3))
    }
    fn done(self) -> bool {
        self.started.elapsed() >= Duration::from_millis(160)
    }
}
struct FocusTransition {
    from: Rect,
    started: Instant,
}
struct DragState {
    tile: Option<usize>,
    start: (f32, f32),
    last: (f32, f32),
    moved: bool,
}
#[derive(Clone, Deserialize)]
pub struct Fixture {
    pub raw: Value,
    pub catalog: Value,
    #[serde(default)]
    pub artworks: Vec<Value>,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub reduced_motion: bool,
}

pub struct Creditia {
    platform_ui: crate::platform_ui::PlatformUi,
    platform_error: Option<String>,
    focus: FocusHandle,
    panel_focus: FocusHandle,
    panel_close_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    panel_tween: Option<Tween>,
    panel_closing: bool,
    dock_height: f32,
    dock_tween: Option<Tween>,
    dock_target: f32,
    focus_transition: Option<FocusTransition>,
    saved_viewport: Option<Viewport>,
    preferences: Preferences,
    preference_writer: Option<crate::preference_writer::PreferenceWriter>,
    preference_task: Option<Task<()>>,
    subscriptions: Vec<Subscription>,
    logo_image: Arc<Image>,
    projects: Vec<Project>,
    discovered: Option<Vec<Value>>,
    discovery_task: Option<Task<()>>,
    discover_busy: bool,
    discovery_selected: Vec<String>,
    add_queue: VecDeque<String>,
    selected: Option<String>,
    raw: Option<Arc<Value>>,
    render_archive: Option<renderer::RenderArchive>,
    loading: bool,
    error: Option<String>,
    warnings: Vec<String>,
    notice: Option<String>,
    progress: Option<Value>,
    job_busy: bool,
    cancelling: bool,
    pending_analysis: Value,
    automatic_job: Option<(String, u64)>,
    analysis_cancellation: Option<service::AnalysisCancellation>,
    list_generation: u64,
    archive_generation: u64,
    list_task: Option<Task<()>>,
    archive_task: Option<Task<()>>,
    dialog_task: Option<Task<()>>,
    job_task: Option<Task<()>>,
    job_worker: Option<Task<()>>,
    catalog: Option<Value>,
    style_index: usize,
    tiles: Vec<Tile>,
    dimensions: BTreeMap<String, (f32, f32)>,
    rects: Vec<Rect>,
    wall_height: f32,
    viewport: Viewport,
    viewport_size: (f32, f32),
    selected_tile: Option<usize>,
    focused: bool,
    hover: Option<Hit>,
    pointer: (f32, f32),
    drag: Option<DragState>,
    preview_generation: u64,
    preview_task: Option<Task<()>>,
    preview_cancellation: Option<renderer::Cancellation>,
    preview_busy: bool,
    playback: Option<Playback>,
    animation_control: Option<crate::animation::PlaybackControl>,
    animation_worker: Option<Task<()>>,
    animation_task: Option<Task<()>>,
    animation_cancellation: Option<renderer::Cancellation>,
    animation_busy: bool,
    panel: Option<Panel>,
    details: Option<Value>,
    details_task: Option<Task<()>>,
    report_task: Option<Task<()>>,
    collect: Value,
    report: Option<Value>,
    remove_confirm: bool,
    operation_task: Option<Task<()>>,
    operation_busy: bool,
    sign_input: Entity<InputState>,
    sign_loaded_for: Option<String>,
    search_input: Entity<InputState>,
    blame_file: Entity<InputState>,
    blame_line: Entity<InputState>,
    blame: Option<Value>,
    all_archives: Arc<Vec<Value>>,
    export_queue: crate::export_jobs::Queue,
    export_journal: Option<crate::export_jobs::Journal>,
    export_journal_task: Option<Task<()>>,
    export_dialog_task: Option<Task<()>>,
    export_prompt_busy: bool,
    export_active: Option<(u64, u64)>,
    export_format: String,
    export_size: String,
    transparent: bool,
    font_mode: String,
    export_label: String,
    export_stage: Option<ExportStage>,
    export_cancel_requested: bool,
    export_task: Option<Task<()>>,
    export_cancellation: Option<renderer::Cancellation>,
    export_open: bool,
    fixture: bool,
    fixture_artworks: BTreeMap<(String, String), Option<Arc<Artwork>>>,
    benchmark: bool,
    ready_emitted: bool,
    telemetry_state: Option<Value>,
    telemetry_sequence: u64,
    background_task: Option<Task<()>>,
    background: Option<service::BackgroundService>,
}
impl Creditia {
    fn base(
        preferences: Preferences,
        fixture: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        bind_keys(cx);
        apply_appearance(&preferences.appearance, window, cx);
        install_menus(&preferences.language, cx);
        cx.set_reduce_motion(preferences.reduced_motion);
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window, cx);
        let sign_input = cx.new(|cx| InputState::new(window, cx).placeholder("署名 / Signature"));
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索项目 / Search projects"));
        let blame_file = cx.new(|cx| InputState::new(window, cx).placeholder("src/main.rs"));
        let blame_line = cx.new(|cx| InputState::new(window, cx).default_value("1"));
        let subscriptions = vec![cx.observe(&search_input, |_, _, cx| cx.notify())];
        let (preference_writer, preference_task) = if fixture {
            (None, None)
        } else {
            let (tx, rx) = async_channel::bounded(1);
            let writer =
                crate::preference_writer::PreferenceWriter::with_notify(Arc::new(move || {
                    let _ = tx.try_send(());
                }));
            let task = cx.spawn(async move |this, cx| {
                while rx.recv().await.is_ok() {
                    if this
                        .update(cx, |view, cx| {
                            if let Some(error) =
                                view.preference_writer.as_ref().and_then(|w| w.take_error())
                            {
                                view.error = Some(error);
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            (Some(writer), Some(task))
        };
        let logo_image = logo_image(&preferences.logo);
        Self {
            platform_ui: crate::platform_ui::PlatformUi::new(),
            platform_error: None,
            focus,
            panel_focus: cx.focus_handle(),
            panel_close_focus: cx.focus_handle().tab_stop(true),
            return_focus: None,
            panel_tween: None,
            panel_closing: false,
            dock_height: 0.,
            dock_tween: None,
            dock_target: 0.,
            focus_transition: None,
            saved_viewport: None,
            preference_writer,
            preference_task,
            subscriptions,
            logo_image,
            selected: preferences.last_project.clone(),
            export_format: preferences.export_format.clone(),
            export_size: preferences.export_size.clone(),
            preferences,
            projects: vec![],
            discovered: None,
            discovery_task: None,
            discover_busy: false,
            discovery_selected: vec![],
            add_queue: VecDeque::new(),
            raw: None,
            render_archive: None,
            loading: false,
            error: None,
            warnings: vec![],
            notice: None,
            progress: None,
            job_busy: false,
            cancelling: false,
            pending_analysis: Value::Null,
            automatic_job: None,
            analysis_cancellation: None,
            list_generation: 0,
            archive_generation: 0,
            list_task: None,
            archive_task: None,
            dialog_task: None,
            job_task: None,
            job_worker: None,
            catalog: None,
            style_index: 0,
            tiles: vec![],
            dimensions: BTreeMap::new(),
            rects: vec![],
            wall_height: 0.,
            viewport: Viewport::default(),
            viewport_size: (1000., 700.),
            selected_tile: None,
            focused: false,
            hover: None,
            pointer: (0., 0.),
            drag: None,
            preview_generation: 0,
            preview_task: None,
            preview_cancellation: None,
            preview_busy: false,
            playback: None,
            animation_control: None,
            animation_worker: None,
            animation_task: None,
            animation_cancellation: None,
            animation_busy: false,
            panel: None,
            details: None,
            details_task: None,
            report_task: None,
            collect: Value::Null,
            report: None,
            remove_confirm: false,
            operation_task: None,
            operation_busy: false,
            sign_input,
            sign_loaded_for: None,
            search_input,
            blame_file,
            blame_line,
            blame: None,
            all_archives: Arc::new(vec![]),
            export_queue: crate::export_jobs::Queue::default(),
            export_journal: None,
            export_journal_task: None,
            export_dialog_task: None,
            export_prompt_busy: false,
            export_active: None,
            transparent: false,
            font_mode: "outline".into(),
            export_label: String::new(),
            export_stage: None,
            export_cancel_requested: false,
            export_task: None,
            export_cancellation: None,
            export_open: false,
            fixture,
            fixture_artworks: BTreeMap::new(),
            benchmark: false,
            ready_emitted: false,
            telemetry_state: None,
            telemetry_sequence: 0,
            background_task: None,
            background: None,
        }
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::base(
            Preferences::read(&service::ui_preferences()),
            false,
            window,
            cx,
        );
        view.loading = true;
        view.list_task = Some(cx.spawn(async move |this, cx| {
            let (result, jobs) = cx
                .background_spawn(async {
                    (
                        service::migrate_legacy_with_helper(),
                        service::load_export_jobs(),
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(settings) => view.preferences = Preferences::read(&settings),
                    Err(error) => {
                        view.notice = Some(format!(
                            "{}: {}",
                            view.text("旧设置尚未导入", "Previous settings have not been imported"),
                            error
                        ))
                    }
                }
                cx.set_reduce_motion(view.preferences.reduced_motion);
                match view.preferences.appearance {
                    Appearance::System => Theme::sync_system_appearance(None, cx),
                    Appearance::Light => Theme::change(ThemeMode::Light, None, cx),
                    Appearance::Dark => Theme::change(ThemeMode::Dark, None, cx),
                }
                view.load_export_history(jobs, cx);
                view.logo_image = logo_image(&view.preferences.logo);
                view.selected = view.preferences.last_project.clone();
                view.export_format = view.preferences.export_format.clone();
                view.export_size = view.preferences.export_size.clone();
                view.refresh(cx);
                view.start_background(cx);
                cx.notify();
            });
        }));
        view
    }
    pub fn fixture(fixture: Fixture, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut preferences = Preferences {
            language: fixture.language,
            appearance: fixture.appearance,
            reduced_motion: fixture.reduced_motion,
            ..Default::default()
        };
        if let Some(style) = fixture.artworks.first().and_then(|a| a["style"].as_str()) {
            preferences.style = style.into();
        }
        let mut view = Self::base(preferences, true, window, cx);
        view.selected = Some("fixture".into());
        view.preferences.last_project = view.selected.clone();
        view.projects.push(Project {
            id: "fixture".into(),
            name: fixture.raw["project"]["name"]
                .as_str()
                .unwrap_or("Creditia")
                .into(),
            path: "".into(),
            exists: true,
            has_credits: true,
            evidence: 0,
        });
        view.raw = Some(Arc::new(fixture.raw));
        view.catalog = Some(fixture.catalog);
        view.resolve_style();
        view.rebuild_tiles(cx);
        for value in fixture.artworks {
            let piece = value["piece"].as_str().unwrap_or("").to_string();
            let variant = value["variant"].as_str().unwrap_or("").to_string();
            let art = if value["art"].is_null() {
                None
            } else {
                serde_json::from_value::<Artwork>(value["art"].clone())
                    .ok()
                    .map(Arc::new)
            };
            view.fixture_artworks.insert((piece, variant), art);
        }
        view.restore_fixture_artworks();
        view
    }
    fn restore_fixture_artworks(&mut self) {
        if !self.fixture {
            return;
        }
        for tile in &mut self.tiles {
            if let Some(art) = self.fixture_artworks.get(&(
                tile.spec.piece.clone(),
                tile.spec.variant.clone().unwrap_or_default(),
            )) {
                if let Some(art) = art {
                    tile.image = Some(Arc::new(Image::from_bytes(
                        ImageFormat::Svg,
                        art.svg.as_bytes().to_vec(),
                    )));
                    tile.pixels = 4096;
                    tile.art = Some(art.clone());
                    self.dimensions
                        .insert(tile.spec.key.clone(), (art.w, art.h));
                } else {
                    tile.error = Some("__empty__".into());
                }
            }
        }
    }

    pub fn benchmark(raw: Value, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::base(
            Preferences {
                language: Language::En,
                appearance: Appearance::Light,
                named: false,
                ..Default::default()
            },
            true,
            window,
            cx,
        );
        view.benchmark = true;
        view.selected = Some("fixture".into());
        view.preferences.last_project = view.selected.clone();
        view.projects.push(Project {
            id: "fixture".into(),
            name: raw["project"]["name"].as_str().unwrap_or("Creditia").into(),
            path: String::new(),
            exists: true,
            has_credits: true,
            evidence: 0,
        });
        view.raw = Some(Arc::new(raw));
        view.load_catalog(cx);
        view
    }
    fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.preferences.language.text(zh, en)
    }
    fn persist(&mut self) {
        if let Some(writer) = &self.preference_writer {
            writer.submit(self.preferences.shared_patch());
        }
    }
    fn project_preferences(&self) -> crate::ProjectPreferences {
        self.preferences.project(self.selected.as_deref())
    }
    fn group(&self) -> String {
        self.project_preferences().group
    }
    fn current(&self) -> Option<&Project> {
        self.projects
            .iter()
            .find(|p| Some(&p.id) == self.selected.as_ref())
    }
    fn style(&self) -> Option<&Value> {
        self.catalog
            .as_ref()?
            .get("styles")?
            .as_array()?
            .get(self.style_index)
    }
    fn piece(&self, id: &str) -> Option<&Value> {
        self.style()?
            .get("pieces")?
            .as_array()?
            .iter()
            .find(|p| p["id"] == id)
    }
    fn current_tile(&self) -> Option<&Tile> {
        self.selected_tile.and_then(|i| self.tiles.get(i))
    }
    fn resolve_style(&mut self) {
        self.style_index = self
            .catalog
            .as_ref()
            .and_then(|c| c["styles"].as_array())
            .and_then(|s| s.iter().position(|s| s["id"] == self.preferences.style))
            .unwrap_or(0);
    }
    fn stop_playback(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = self.animation_control.take() {
            control.cancel();
        }
        self.animation_worker = None;
        if let Some(c) = self.animation_cancellation.take() {
            c.cancel();
        }
        self.animation_task = None;
        self.animation_busy = false;
        if let Some(play) = self.playback.take() {
            for (_, (_, image)) in play.frames {
                ImageSource::Image(image).remove_asset(cx);
            }
        }
    }
    fn clear_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = self.preview_cancellation.take() {
            c.cancel();
        }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview_task = None;
        self.preview_busy = false;
        self.stop_playback(cx);
        for tile in &mut self.tiles {
            if let Some(image) = tile.image.take() {
                ImageSource::Image(image).remove_asset(cx);
            }
        }
        self.tiles.clear();
        self.rects.clear();
        self.hover = None;
    }
    fn rebuild_tiles(&mut self, cx: &mut Context<Self>) {
        self.clear_preview(cx);
        let prefs = self.project_preferences();
        if let Some(style) = self.style() {
            self.tiles = wall_tiles(style, &prefs.variants, &prefs.order)
                .into_iter()
                .map(|spec| Tile {
                    spec,
                    art: None,
                    image: None,
                    image_region: None,
                    pixels: 0,
                    texture_bytes: 0,
                    error: None,
                    loading: false,
                })
                .collect();
        }
        self.selected_tile = (!self.tiles.is_empty()).then_some(0);
        self.viewport.reset();
        self.focused = false;
        self.restore_fixture_artworks();
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation = self.list_generation.wrapping_add(1);
        let generation = self.list_generation;
        self.loading = true;
        self.error = None;
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::projects() }).await;
            let _ = this.update(cx, |view, cx| {
                if view.list_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<Vec<Project>>(value) {
                        Ok(projects) => {
                            view.projects = projects;
                            if view.selected.as_deref() == Some("__all__") {
                                view.show_all_projects(cx);
                                return;
                            }
                            let id = view
                                .selected
                                .clone()
                                .filter(|id| view.projects.iter().any(|p| &p.id == id))
                                .or_else(|| view.projects.first().map(|p| p.id.clone()));
                            if let Some(id) = id {
                                view.select(id, cx);
                            } else {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                            }
                        }
                        Err(error) => view.error = Some(error.to_string()),
                    },
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.all_archives = Arc::new(vec![]);
        self.blame = None;
        self.focused = false;
        self.details = None;
        self.automatic_job = None;
        if !self.job_busy {
            self.progress = None;
        }
        if let Some(background) = &self.background {
            background.watch_project(Some(id.clone()));
        }
        self.preferences.last_project = Some(id.clone());
        self.error = None;
        self.persist();
        self.raw = None;
        self.render_archive = None;
        self.loading = true;
        self.archive_task = Some(cx.spawn(async move |this, cx| {
            let (result, details) = cx
                .background_spawn(async move {
                    (service::credits(id.clone()), service::project_details(id))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                // A slower previous project must never replace the current archive.
                if view.archive_generation != generation {
                    return;
                }
                view.loading = false;
                match details {
                    Ok(details) => view.details = Some(details),
                    Err(error) => view.notice = Some(error.to_string()),
                }
                match result {
                    Ok(raw) => {
                        view.raw = Some(Arc::new(raw));
                        view.load_catalog(cx);
                    }
                    Err(error) if error.code == "no_credits" => {}
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn start_next_project(&mut self, cx: &mut Context<Self>) {
        if self.job_busy || self.automatic_job.is_some() {
            return;
        }
        if let Some(path) = self.add_queue.pop_front() {
            self.start_job(Some(path), cx);
        }
    }
    fn discover_projects(&mut self, cx: &mut Context<Self>) {
        if self.fixture || self.discovered.is_some() || self.discover_busy {
            return;
        }
        self.discover_busy = true;
        self.discovery_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::discover() }).await;
            let _ = this.update(cx, |view, cx| {
                view.discover_busy = false;
                match result {
                    Ok(value) => {
                        let projects = value["projects"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|p| {
                                p["exists"] == true
                                    && !view.projects.iter().any(|known| {
                                        Some(known.path.as_str()) == p["path"].as_str()
                                    })
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        view.discovery_selected = projects
                            .iter()
                            .take(3)
                            .filter_map(|p| p["path"].as_str().map(str::to_owned))
                            .collect();
                        view.discovered = Some(projects);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some(self.text("选择 Git 项目", "Choose a Git project").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                match choice {
                    Ok(Ok(Some(paths))) => {
                        view.add_queue.extend(
                            paths
                                .into_iter()
                                .map(|path| path.to_string_lossy().into_owned()),
                        );
                        view.start_next_project(cx);
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }

    fn start_job(&mut self, add_path: Option<String>, cx: &mut Context<Self>) {
        self.start_job_mode(add_path, false, cx);
    }
    fn start_job_mode(&mut self, add_path: Option<String>, resume: bool, cx: &mut Context<Self>) {
        if self.job_busy
            || self.automatic_job.is_some()
            || (!resume && add_path.is_none() && self.selected.is_none())
        {
            return;
        }
        self.job_busy = true;
        self.cancelling = false;
        self.progress = None;
        self.error = None;
        self.notice = None;
        self.warnings.clear();
        let selected = self.selected.clone();
        let selection_at_start = selected.clone();
        let lang = self.preferences.language.code().to_string();
        let adding = add_path.is_some();
        let cancellation = service::AnalysisCancellation::default();
        self.analysis_cancellation = Some(cancellation.clone());
        // Progress is bounded and disposable; completion is always delivered.
        let (sender, receiver) = async_channel::bounded(32);
        let progress_sender = sender.clone();
        let sink: service::ProgressSink = Arc::new(move |value| {
            let _ = progress_sender.try_send(JobEvent::Progress(value));
        });
        self.job_worker = Some(cx.background_spawn(async move {
            let result = if resume {
                service::resume_analysis(Some(sink), cancellation)
            } else {
                match add_path {
                    Some(path) => {
                        service::add_project_with_cancel(path, Some(lang), Some(sink), cancellation)
                    }
                    None => service::analyze_with_cancel(
                        selected.expect("selected project"),
                        Some(lang),
                        Some(sink),
                        cancellation,
                    ),
                }
            };
            let _ = sender.send(JobEvent::Finished(result)).await;
        }));
        self.job_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(&event, JobEvent::Finished(_));
                if this
                    .update(cx, |view, cx| {
                        match event {
                            JobEvent::Progress(value) => view.progress = Some(value),
                            JobEvent::Finished(result) => {
                                view.job_busy = false;
                                view.analysis_cancellation = None;
                                view.progress = None;
                                match result {
                                    Ok(value) => {
                                        view.warnings =
                                            crate::ui_reports::analysis_warnings(&value);
                                        if adding && view.selected == selection_at_start {
                                            view.selected =
                                                value["id"].as_str().map(str::to_string);
                                        }
                                        view.notice =
                                            Some(view.text("分析完成", "Analysis complete").into());
                                        view.refresh(cx);
                                    }
                                    Err(error) if view.cancelling || error.code == "cancelled" => {
                                        view.notice = Some(
                                            view.text("已取消分析", "Analysis cancelled").into(),
                                        );
                                    }
                                    Err(error) => view.error = Some(error.to_string()),
                                }
                                view.cancelling = false;
                                view.pending_analysis = service::analysis_status();
                                view.start_next_project(cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if finished {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn analyze(&mut self, _: &Analyze, _: &mut Window, cx: &mut Context<Self>) {
        self.start_job(None, cx);
    }

    fn cancel(&mut self, _: &CancelAnalysis, _: &mut Window, cx: &mut Context<Self>) {
        if self.automatic_job.is_some() && !self.job_busy {
            service::cancel_analyze();
            self.cancelling = true;
            cx.notify();
        }
        if self.job_busy {
            self.add_queue.clear();
            if let Some(cancellation) = &self.analysis_cancellation {
                cancellation.cancel();
            }
            self.cancelling = true;
            cx.notify();
        }
    }

    fn copy_archive(&mut self, _: &CopyArchive, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(raw) = &self.raw {
            match serde_json::to_string_pretty(raw.as_ref()) {
                Ok(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.notice = Some(self.text("档案已复制", "Archive copied").into());
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            cx.notify();
        }
    }

    fn reveal_project(&mut self, _: &RevealProject, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.current() {
            cx.reveal_path(Path::new(&project.path));
        }
    }

    fn load_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog.is_some() {
            self.resolve_style();
            self.rebuild_tiles(cx);
            return;
        }
        let cancellation = renderer::Cancellation::default();
        self.preview_cancellation = Some(cancellation.clone());
        let generation = self.preview_generation;
        self.preview_busy = true;
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    renderer::request(
                        json!({"version":1,"id":"catalog","op":"catalog"}),
                        cancellation,
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.preview_generation != generation {
                    return;
                }
                view.preview_busy = false;
                match result {
                    Ok(catalog) => {
                        view.catalog = Some(catalog);
                        view.resolve_style();
                        view.rebuild_tiles(cx);
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn artwork_request(&self, spec: &TileSpec, op: &str) -> Option<Value> {
        let mut request = json!({"version":1,"id":format!("{}:{}",self.preview_generation,spec.key),"op":op,"style":self.style()?["id"],"piece":spec.piece,"lang":self.preferences.language.code(),"group":self.group(),"named":self.preferences.named,"flatten":true});
        if let Some(variant) = &spec.variant {
            request["variant"] = json!(variant);
        }
        if let Some(sign) = self
            .details
            .as_ref()
            .and_then(|d| d["config"]["display"]["sign"].as_str())
        {
            request["sign"] = json!(sign);
        }
        if let Some(blame) = &self.blame {
            request["blame"] = blame.clone();
        }
        Some(request)
    }
    fn compute_layout(&mut self, width: f32, height: f32) {
        self.viewport_size = (width, height);
        let world_width = if self.preferences.view == "roam" {
            1440.
        } else {
            width
        };
        let specs = self
            .tiles
            .iter()
            .map(|t| t.spec.clone())
            .collect::<Vec<_>>();
        let (rects, total) = masonry(&specs, &self.dimensions, world_width);
        self.rects = rects;
        self.wall_height = total;
        if self.preferences.view == "wall" && !self.focused {
            self.viewport.x = 0.;
            self.viewport.zoom = 1.;
            self.viewport.y = self.viewport.y.clamp(-(total - height).max(0.), 0.);
        }
    }
    fn screen_rect(&self, index: usize) -> Rect {
        let target = if self.focused && self.selected_tile == Some(index) {
            let art = self
                .playback
                .as_ref()
                .and_then(|p| p.frames.get(&p.index))
                .map(|f| f.0.as_ref())
                .or_else(|| self.tiles.get(index).and_then(|t| t.art.as_deref()));
            let (w, h) = art.map(|a| (a.w, a.h)).unwrap_or((640., 480.));
            let scale = ((self.viewport_size.0 - 100.) / w)
                .min((self.viewport_size.1 - 100.) / h)
                .max(0.01)
                * self.viewport.zoom;
            Rect {
                x: (self.viewport_size.0 - w * scale) / 2. + self.viewport.x,
                y: (self.viewport_size.1 - h * scale) / 2. + self.viewport.y,
                w: w * scale,
                h: h * scale,
            }
        } else {
            self.rects
                .get(index)
                .copied()
                .unwrap_or_default()
                .transformed(self.viewport)
        };
        if self.selected_tile == Some(index) {
            if let Some(transition) = &self.focus_transition {
                let t = (transition.started.elapsed().as_secs_f32() / 0.18).min(1.);
                let t = 1. - (1. - t).powi(3);
                let from = transition.from;
                return Rect {
                    x: from.x + (target.x - from.x) * t,
                    y: from.y + (target.y - from.y) * t,
                    w: from.w + (target.w - from.w) * t,
                    h: from.h + (target.h - from.h) * t,
                };
            }
        }
        target
    }
    fn crop_for_art(&self, art: &Artwork, r: Rect, pixels: u32, margin: f32) -> Option<Rect> {
        let height = pixels as f32 * art.h / art.w;
        if pixels as f32 * height < 16_000_000. && height < 16384. {
            return None;
        }
        let scale = r.w / art.w;
        let x = ((-r.x - margin) / scale).clamp(0., art.w);
        let y = ((-r.y - margin) / scale).clamp(0., art.h);
        let right = ((self.viewport_size.0 - r.x + margin) / scale).clamp(x, art.w);
        let bottom = ((self.viewport_size.1 - r.y + margin) / scale).clamp(y, art.h);
        Some(Rect {
            x,
            y,
            w: (right - x).max(1.),
            h: (bottom - y).max(1.),
        })
    }
    fn schedule_visible(&mut self, dpi: f32, cx: &mut Context<Self>) {
        if (self.fixture && !self.benchmark)
            || self.preview_busy
            || self.raw.is_none()
            || self.catalog.is_none()
            || (self.focused
                && self
                    .playback
                    .as_ref()
                    .is_some_and(|p| p.frames.contains_key(&p.index)))
        {
            return;
        }
        let visible = Rect {
            x: -150.,
            y: -220.,
            w: self.viewport_size.0 + 300.,
            h: self.viewport_size.1 + 440.,
        };
        let next = self
            .tiles
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                (!self.focused || self.selected_tile == Some(*i)) && !t.loading && t.error.is_none()
            })
            .find_map(|(index, tile)| {
                let r = self.screen_rect(index);
                if !r.intersects(visible) {
                    return None;
                }
                let pixels = texture_width(r.w, dpi);
                let needs_crop = tile
                    .art
                    .as_deref()
                    .and_then(|art| self.crop_for_art(art, r, pixels, 0.))
                    .is_some_and(|required| {
                        tile.image_region.is_none_or(|have| {
                            have.x > required.x + 1.
                                || have.y > required.y + 1.
                                || have.x + have.w < required.x + required.w - 1.
                                || have.y + have.h < required.y + required.h - 1.
                        })
                    });
                (tile.image.is_none() || tile.pixels < pixels || needs_crop)
                    .then_some((index, pixels, r))
            });
        let Some((index, pixels, screen)) = next else {
            return;
        };
        let Some(request) = self.artwork_request(&self.tiles[index].spec, "render") else {
            return;
        };
        let Some(raw) = self.raw.clone() else {
            return;
        };
        let archive = self.render_archive.clone();
        let projects = self.all_archives.clone();
        let existing = self.tiles[index].art.clone();
        let key = self.tiles[index].spec.key.clone();
        let generation = self.preview_generation;
        let viewport = self.viewport_size;
        self.tiles[index].loading = true;
        self.preview_busy = true;
        let cancellation = renderer::Cancellation::default();
        self.preview_cancellation = Some(cancellation.clone());
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<
                (
                    Arc<Artwork>,
                    Arc<native_preview::RasterImage>,
                    Option<Rect>,
                    renderer::RenderArchive,
                ),
                String,
            > = cx
                .background_spawn(async move {
                    let archive = match archive {
                        Some(archive) => archive,
                        None => renderer::RenderArchive::new(raw)?,
                    };
                    let mut request = request;
                    if !projects.is_empty() {
                        request["projects"] = json!(projects.as_ref());
                    }
                    let art = match existing {
                        Some(art) => art,
                        None => {
                            let result = renderer::request_with_archive(
                                request,
                                archive.clone(),
                                cancellation.clone(),
                                1,
                            )?;
                            if result["art"].is_null() {
                                return Err("__empty__".into());
                            }
                            Arc::new(
                                serde_json::from_value::<Artwork>(result["art"].clone())
                                    .map_err(|e| e.to_string())?,
                            )
                        }
                    };
                    let height = pixels as f32 * art.h / art.w;
                    let region = if pixels as f32 * height >= 16_000_000. || height >= 16384. {
                        let scale = screen.w / art.w;
                        let x = ((-screen.x - 180.) / scale).clamp(0., art.w);
                        let y = ((-screen.y - 240.) / scale).clamp(0., art.h);
                        let right = ((viewport.0 - screen.x + 180.) / scale).clamp(x, art.w);
                        let bottom = ((viewport.1 - screen.y + 240.) / scale).clamp(y, art.h);
                        Some(Rect {
                            x,
                            y,
                            w: (right - x).max(1.),
                            h: (bottom - y).max(1.),
                        })
                    } else {
                        None
                    };
                    let raster = if let Some(r) = region {
                        native_preview::rasterize_region(
                            &art.svg,
                            (r.x, r.y, r.w, r.h),
                            (pixels as f32 * r.w / art.w).ceil().max(1.) as u32,
                            &cancellation,
                        )?
                    } else {
                        native_preview::rasterize(&art.svg, pixels, &cancellation)?
                    };
                    Ok((art, raster, region, archive))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if generation != view.preview_generation {
                    return;
                }
                view.preview_busy = false;
                view.preview_cancellation = None;
                if let Some(tile) = view.tiles.iter_mut().find(|t| t.spec.key == key) {
                    tile.loading = false;
                    match result {
                        Ok((art, raster, region, archive)) => {
                            view.render_archive = Some(archive);
                            if let Some(image) = tile.image.take() {
                                ImageSource::Image(image).remove_asset(cx);
                            }
                            tile.image = Some(Arc::new(Image::from_bytes(
                                ImageFormat::Png,
                                raster.png.clone(),
                            )));
                            tile.image_region = region;
                            tile.pixels = pixels;
                            tile.texture_bytes = raster.width as usize * raster.height as usize * 4;
                            view.dimensions.insert(key, (art.w, art.h));
                            tile.art = Some(art);
                        }
                        Err(error) => tile.error = Some(error),
                    }
                }
                view.evict_distant(cx);
                cx.notify();
            });
        }));
    }
    fn evict_distant(&mut self, cx: &mut Context<Self>) {
        let mut bytes = self.tiles.iter().map(|t| t.texture_bytes).sum::<usize>();
        if bytes < 128 * 1024 * 1024 {
            return;
        }
        let near = Rect {
            x: -self.viewport_size.0,
            y: -self.viewport_size.1,
            w: self.viewport_size.0 * 3.,
            h: self.viewport_size.1 * 3.,
        };
        for index in (0..self.tiles.len()).rev() {
            if bytes < 96 * 1024 * 1024 {
                break;
            }
            if self.selected_tile == Some(index) || self.screen_rect(index).intersects(near) {
                continue;
            }
            let tile = &mut self.tiles[index];
            if let Some(image) = tile.image.take() {
                ImageSource::Image(image).remove_asset(cx);
                bytes = bytes.saturating_sub(tile.texture_bytes);
                tile.texture_bytes = 0;
                tile.pixels = 0;
                tile.image_region = None;
                tile.art = None;
            }
        }
    }
    fn choose_style(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(style) = self
            .catalog
            .as_ref()
            .and_then(|c| c["styles"].as_array())
            .and_then(|s| s.get(index))
        {
            self.preferences.style = style["id"].as_str().unwrap_or("").into();
            self.style_index = index;
            self.persist();
            self.panel = None;
            self.rebuild_tiles(cx);
        }
    }
    fn step_style(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self
            .catalog
            .as_ref()
            .and_then(|c| c["styles"].as_array())
            .map(Vec::len)
            .unwrap_or(0);
        if count > 0 {
            let index = (self.style_index as isize + delta).rem_euclid(count as isize) as usize;
            self.choose_style(index, cx);
        }
    }
    fn set_group(&mut self, group: &str, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.preferences.projects.entry(id).or_default().group = group.into();
            self.persist();
            self.rebuild_tiles(cx);
        }
    }
    fn set_names(&mut self, cx: &mut Context<Self>) {
        self.preferences.named = !self.preferences.named;
        self.persist();
        self.rebuild_tiles(cx);
    }
    fn set_view(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.preferences.view = mode.into();
        self.focused = false;
        self.viewport.reset();
        if mode == "roam" {
            self.viewport.zoom = 0.62;
        }
        self.stop_playback(cx);
        self.persist();
        cx.notify();
    }
    fn select_tile(&mut self, index: usize, focus: bool, cx: &mut Context<Self>) {
        if index >= self.tiles.len() {
            return;
        }
        let source = self.screen_rect(index);
        let entering = focus && !self.focused;
        if entering {
            self.saved_viewport = Some(self.viewport);
        }
        self.stop_playback(cx);
        self.selected_tile = Some(index);
        self.focused = focus;
        if focus {
            self.viewport.reset();
        }
        self.hover = None;
        self.panel = None;
        self.focus_transition = if entering && !self.preferences.reduced_motion {
            Some(FocusTransition {
                from: source,
                started: Instant::now(),
            })
        } else {
            None
        };
        cx.notify();
    }
    fn close_focus(&mut self, cx: &mut Context<Self>) {
        let source = self.selected_tile.map(|i| self.screen_rect(i));
        self.focused = false;
        self.viewport = self.saved_viewport.take().unwrap_or_default();
        self.stop_playback(cx);
        self.focus_transition = if self.preferences.reduced_motion {
            None
        } else {
            source.map(|from| FocusTransition {
                from,
                started: Instant::now(),
            })
        };
        self.hover = None;
        cx.notify();
    }
    fn select_piece(&mut self, piece: String, cx: &mut Context<Self>) {
        let requires = self
            .piece(&piece)
            .and_then(|p| p["requires"].as_array())
            .cloned()
            .unwrap_or_default();
        if requires.iter().any(|r| r == "blame") && self.blame.is_none() {
            self.panel = Some(Panel::Blame);
            cx.notify();
            return;
        }
        if requires.iter().any(|r| r == "projects") && self.all_archives.is_empty() && !self.fixture
        {
            if self.operation_busy {
                return;
            }
            self.operation_busy = true;
            self.operation_task = Some(cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async { service::project_archives() })
                    .await;
                let _ = this.update(cx, |view, cx| {
                    view.operation_busy = false;
                    match result {
                        Ok(value) => {
                            view.all_archives = Arc::new(
                                value
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|p| p.get("raw").cloned())
                                    .collect(),
                            );
                            if view.all_archives.is_empty() {
                                view.error = Some("No project archives are available".into());
                            } else {
                                view.select_piece(piece, cx);
                            }
                        }
                        Err(error) => view.error = Some(error.to_string()),
                    }
                    cx.notify();
                });
            }));
            return;
        }
        if let Some(index) = self.tiles.iter().position(|t| t.spec.piece == piece) {
            self.select_tile(index, true, cx);
        } else if let Some(def) = self.piece(&piece) {
            let variant = default_variant(def);
            self.tiles.push(Tile {
                spec: TileSpec {
                    key: format!("{}#{}", piece, variant.as_deref().unwrap_or("")),
                    piece,
                    variant,
                    span: 1,
                },
                art: None,
                image: None,
                image_region: None,
                pixels: 0,
                texture_bytes: 0,
                error: None,
                loading: false,
            });
            self.select_tile(self.tiles.len() - 1, true, cx);
        }
    }
    fn choose_variant(&mut self, variant: String, cx: &mut Context<Self>) {
        let Some(tile) = self.current_tile() else {
            return;
        };
        let key = tile.spec.key.clone();
        let specs = self
            .tiles
            .iter()
            .map(|t| t.spec.clone())
            .collect::<Vec<_>>();
        let Some(id) = self.selected.clone() else {
            return;
        };
        let prefs = self.preferences.projects.entry(id).or_default();
        if swap_variant(&specs, &mut prefs.variants, &key, &variant) {
            self.persist();
            let selected = self.selected_tile;
            let focus = self.focused;
            self.rebuild_tiles(cx);
            self.selected_tile = selected;
            self.focused = focus;
        }
    }
    fn move_piece(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.tiles.is_empty() {
            return;
        }
        let next = (self.selected_tile.unwrap_or(0) as isize + delta)
            .rem_euclid(self.tiles.len() as isize) as usize;
        self.select_tile(next, self.focused, cx);
    }
    fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        let (x, y) = if self.focused {
            (x - self.viewport_size.0 / 2., y - self.viewport_size.1 / 2.)
        } else {
            (x, y)
        };
        self.viewport.zoom_at(factor, x, y);
    }
    fn zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        if !self.focused && self.preferences.view != "roam" {
            self.set_view("roam", cx);
        }
        self.zoom_at(factor, self.viewport_size.0 / 2., self.viewport_size.1 / 2.);
        cx.notify();
    }
    fn toggle_language(&mut self, _: &ToggleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.preferences.language = match self.preferences.language {
            Language::Zh => Language::En,
            Language::En => Language::Zh,
        };
        self.persist();
        install_menus(&self.preferences.language, cx);
        self.rebuild_tiles(cx);
    }
    fn cycle_appearance(
        &mut self,
        _: &CycleAppearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.appearance = match self.preferences.appearance {
            Appearance::System => Appearance::Light,
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::System,
        };
        apply_appearance(&self.preferences.appearance, window, cx);
        self.persist();
        cx.notify();
    }
    fn toggle_motion(&mut self, _: &ToggleMotion, _: &mut Window, cx: &mut Context<Self>) {
        self.preferences.reduced_motion = !self.preferences.reduced_motion;
        cx.set_reduce_motion(self.preferences.reduced_motion);
        self.stop_playback(cx);
        self.persist();
        cx.notify();
    }
    fn play(&mut self, restart: bool, cx: &mut Context<Self>) {
        if self.animation_busy && self.playback.is_none() {
            return;
        }
        if let Some(play) = &mut self.playback {
            if restart {
                play.elapsed = Duration::ZERO;
                play.started = Instant::now();
                play.playing = true;
                play.requested = 0;
                if let Some(control) = &self.animation_control {
                    control.restart();
                }
            } else if play.playing {
                play.elapsed = play.started.elapsed();
                play.playing = false;
                if let Some(control) = &self.animation_control {
                    control.pause();
                }
            } else {
                play.started = Instant::now() - play.elapsed;
                play.playing = true;
                if let Some(control) = &self.animation_control {
                    control.resume();
                }
            }
            cx.notify();
            return;
        }
        let Some(tile) = self.current_tile() else {
            return;
        };
        if self
            .piece(&tile.spec.piece)
            .is_none_or(|p| p["animated"] != true)
        {
            return;
        }
        let Some(mut request) = self.artwork_request(&tile.spec, "render") else {
            return;
        };
        let selected = self.selected_tile;
        let generation = self.preview_generation;
        request["reducedMotion"] = json!(self.preferences.reduced_motion);
        self.focused = true;
        self.viewport.reset();
        self.animation_busy = true;
        let control = crate::animation::PlaybackControl::new();
        self.animation_cancellation = Some(control.cancellation());
        self.animation_control = Some(control.clone());
        let width = texture_width(self.viewport_size.0.min(1200.), 1.);
        let Some(raw) = self.raw.clone() else {
            return;
        };
        let archive = self.render_archive.clone();
        let projects = self.all_archives.clone();
        let (tx, rx) = async_channel::bounded::<Result<crate::animation::Event, String>>(3);
        self.animation_worker = Some(cx.background_spawn(async move {
            let result = (|| {
                let archive = match archive {
                    Some(archive) => archive,
                    None => renderer::RenderArchive::new(raw)?,
                };
                if !projects.is_empty() {
                    request["projects"] = json!(projects.as_ref());
                }
                crate::animation::run_with_archive(
                    request,
                    archive,
                    width,
                    control.clone(),
                    |event| {
                        tx.send_blocking(Ok(event))
                            .map_err(|_| "Animation view closed".to_string())
                    },
                )
            })();
            if let Err(error) = result {
                if !control.cancelled() {
                    let _ = tx.send(Err(error)).await;
                }
            }
        }));
        self.animation_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                if this
                    .update(cx, |view, cx| {
                        if view.preview_generation != generation || view.selected_tile != selected {
                            return;
                        }
                        match event {
                            Ok(crate::animation::Event::Started {
                                epoch,
                                timeline,
                                times,
                            }) => {
                                view.playback = Some(Playback {
                                    frames: BTreeMap::new(),
                                    times,
                                    timeline,
                                    started: Instant::now(),
                                    elapsed: Duration::ZERO,
                                    playing: false,
                                    index: 0,
                                    requested: 0,
                                    epoch,
                                    started_once: false,
                                })
                            }
                            Ok(crate::animation::Event::Frame {
                                epoch,
                                index,
                                art,
                                raster,
                            }) => {
                                if view
                                    .animation_control
                                    .as_ref()
                                    .is_none_or(|c| c.epoch() != epoch)
                                {
                                    return;
                                }
                                if let Some(play) = &mut view.playback {
                                    if play.epoch != epoch {
                                        for (_, (_, image)) in std::mem::take(&mut play.frames) {
                                            ImageSource::Image(image).remove_asset(cx);
                                        }
                                        play.epoch = epoch;
                                    }
                                    if let Some((_, old)) = play.frames.insert(
                                        index,
                                        (
                                            art,
                                            Arc::new(Image::from_bytes(
                                                ImageFormat::Png,
                                                raster.png.clone(),
                                            )),
                                        ),
                                    ) {
                                        ImageSource::Image(old).remove_asset(cx);
                                    }
                                    if !play.started_once {
                                        play.started_once = true;
                                        play.started = Instant::now();
                                        play.playing = play.timeline.duration_ms > 0;
                                        play.index = index;
                                    } else if index <= play.requested
                                        || !play.frames.contains_key(&play.index)
                                    {
                                        play.index = index;
                                    }
                                    let keep = play.requested;
                                    let shown = play.index;
                                    let obsolete = play
                                        .frames
                                        .keys()
                                        .copied()
                                        .filter(|i| {
                                            *i != shown
                                                && (*i < keep.saturating_sub(1)
                                                    || *i > keep + crate::animation::MAX_AHEAD)
                                        })
                                        .collect::<Vec<_>>();
                                    for index in obsolete {
                                        if let Some((_, image)) = play.frames.remove(&index) {
                                            ImageSource::Image(image).remove_asset(cx);
                                        }
                                    }
                                    while play.frames.len() > crate::animation::MAX_RESIDENT_FRAMES
                                    {
                                        let candidate = play
                                            .frames
                                            .keys()
                                            .copied()
                                            .find(|i| *i != shown)
                                            .unwrap_or(shown);
                                        if let Some((_, image)) = play.frames.remove(&candidate) {
                                            ImageSource::Image(image).remove_asset(cx);
                                        }
                                    }
                                    view.animation_busy = false;
                                }
                            }
                            Err(error) => {
                                view.animation_busy = false;
                                view.error = Some(error);
                                if let Some(play) = &mut view.playback {
                                    play.playing = false;
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
    }
    pub fn fixture_images_ready(&self, window: &mut Window, cx: &mut App) -> bool {
        let visible = Rect {
            x: 0.,
            y: 0.,
            w: self.viewport_size.0,
            h: self.viewport_size.1,
        };
        let tiles = self
            .tiles
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                (!self.focused || self.selected_tile == Some(*i))
                    && self.screen_rect(*i).intersects(visible)
            })
            .map(|(_, tile)| tile)
            .collect::<Vec<_>>();
        !tiles.is_empty()
            && tiles.into_iter().all(|tile| {
                tile.error.as_deref() == Some("__empty__")
                    || tile
                        .image
                        .as_ref()
                        .is_some_and(|image| image.clone().get_render_image(window, cx).is_some())
            })
    }
    fn start_background(&mut self, cx: &mut Context<Self>) {
        self.pending_analysis = service::analysis_status();
        let (tx, rx) = async_channel::bounded(64);
        let sink: service::ProgressSink = Arc::new(move |event| {
            let _ = tx.try_send(event);
        });
        self.background = Some(service::BackgroundService::start(sink));
        self.background_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                if this
                    .update(cx, |view, cx| {
                        match event["kind"].as_str().unwrap_or("") {
                            "archive-changed" => {
                                if event["projectId"].as_str() == view.selected.as_deref() {
                                    if view.selected.as_deref() == Some("__all__") {
                                        view.show_all_projects(cx);
                                    } else if let Some(id) = view.selected.clone() {
                                        view.select(id, cx);
                                    }
                                }
                            }
                            "analysis-started" => {
                                if event["projectId"].as_str() == view.selected.as_deref() {
                                    view.automatic_job = Some((
                                        event["projectId"].as_str().unwrap_or("").into(),
                                        event["generation"].as_u64().unwrap_or(0),
                                    ));
                                    if !view.job_busy {
                                        view.progress = Some(json!({"stage":"history"}));
                                    }
                                }
                            }
                            "analysis-progress" => {
                                if !view.job_busy
                                    && view.automatic_job.as_ref().is_some_and(
                                        |(id, generation)| {
                                            event["projectId"].as_str() == Some(id)
                                                && event["generation"].as_u64() == Some(*generation)
                                        },
                                    )
                                {
                                    view.progress = Some(event["progress"].clone());
                                }
                            }
                            "analysis-finished" => {
                                if view.automatic_job.as_ref().is_some_and(|(id, generation)| {
                                    event["projectId"].as_str() == Some(id)
                                        && event["generation"].as_u64() == Some(*generation)
                                }) {
                                    view.automatic_job = None;
                                    view.cancelling = false;
                                    if !view.job_busy {
                                        view.progress = None;
                                        view.refresh(cx);
                                    }
                                }
                            }
                            "collect-done" => view.collect = service::collect_info(),
                            "analysis-interrupted" => {
                                view.pending_analysis = event["task"].clone();
                                view.notice = Some(
                                    view.text(
                                        "上次分析被中断，可以继续。",
                                        "The previous analysis was interrupted and can be resumed.",
                                    )
                                    .into(),
                                );
                            }
                            "error" => view.error = Some(event["error"].to_string()),
                            _ => {}
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel == Some(panel) && !self.panel_closing {
            self.close_panel(window, cx);
            return;
        }
        let from = self
            .panel_tween
            .map(Tween::value)
            .unwrap_or(if self.panel.is_some() { 1. } else { 0. });
        if self.panel.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.panel = Some(panel);
        self.panel_closing = false;
        self.panel_tween = (!self.preferences.reduced_motion).then_some(Tween {
            from,
            to: 1.,
            started: Instant::now(),
        });
        self.panel_close_focus.focus(window, cx);
        self.remove_confirm = false;
        self.hover = None;
        if panel == Panel::Project && self.panel.is_some() {
            let sign = self
                .details
                .as_ref()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.details.as_ref().and(self.selected.clone());
        }
        if panel == Panel::Projects {
            self.discover_projects(cx);
        }
        if panel == Panel::Settings && !self.fixture {
            self.collect = service::collect_info();
        }
        cx.notify();
    }
    fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.preferences.reduced_motion {
            self.panel = None;
            self.panel_tween = None;
            self.panel_closing = false;
            self.return_focus
                .take()
                .unwrap_or_else(|| self.focus.clone())
                .focus(window, cx);
        } else {
            let from = self.panel_tween.map(Tween::value).unwrap_or(1.);
            self.panel_tween = Some(Tween {
                from,
                to: 0.,
                started: Instant::now(),
            });
            self.panel_closing = true;
        }
        cx.notify();
    }
    fn fetch_details(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        let Some(id) = self.selected.clone().filter(|id| id != "__all__") else {
            return;
        };
        let selection = id.clone();
        self.details_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::project_details(id) })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.selected.as_ref() != Some(&selection) {
                    return;
                }
                match result {
                    Ok(details) => view.details = Some(details),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    fn load_blame(&mut self, cx: &mut Context<Self>) {
        if self.operation_busy {
            return;
        }
        let Some(id) = self.selected.clone().filter(|id| id != "__all__") else {
            return;
        };
        let file = self.blame_file.read(cx).value().to_string();
        let line = match self.blame_line.read(cx).value().parse::<u32>() {
            Ok(line) if line > 0 => line,
            _ => {
                self.error = Some(
                    self.text("请输入正整数行号", "Enter a positive line number")
                        .into(),
                );
                cx.notify();
                return;
            }
        };
        let selection = id.clone();
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::blame(id, file, u64::from(line)) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                if view.selected.as_ref() != Some(&selection) {
                    return;
                }
                match result {
                    Ok(blame) => {
                        view.blame = Some(blame);
                        view.select_piece("blame".into(), cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    fn export_evidence(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone().filter(|id| id != "__all__") else {
            return;
        };
        let directory = match service::export_dir() {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let prompt = cx.prompt_for_new_path(&directory, Some("creditia-evidence.json"));
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(path))) => path,
                _ => return,
            };
            let result = cx
                .background_spawn(async move {
                    let evidence = service::evidence_archive(id).map_err(|e| e.to_string())?;
                    let bytes = serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?;
                    renderer::write_export(&path, &bytes, renderer::Cancellation::default())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(()) => {
                        view.notice =
                            Some(view.text("证据档案已保存", "Evidence archive saved").into())
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn import_evidence(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone().filter(|id| id != "__all__") else {
            return;
        };
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.text("导入证据档案", "Import evidence archive").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => path,
                    None => return,
                },
                _ => return,
            };
            let cancellation = service::AnalysisCancellation::default();
            let worker = cancellation.clone();
            let _ = this.update(cx, |view, cx| {
                view.job_busy = true;
                view.analysis_cancellation = Some(cancellation);
                cx.notify();
            });
            let result = cx
                .background_spawn(async move { service::import_evidence(id, path, worker) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.job_busy = false;
                view.cancelling = false;
                view.analysis_cancellation = None;
                match result {
                    Ok(_) => {
                        view.notice = Some(
                            view.text(
                                "证据已导入，重新分析后更新作品",
                                "Evidence imported; analyze to update artwork",
                            )
                            .into(),
                        );
                        view.fetch_details(cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    fn load_diagnostics(&mut self, cx: &mut Context<Self>) {
        self.panel = Some(Panel::Diagnostics);
        self.report = None;
        if self.fixture {
            return;
        }
        self.report_task = Some(cx.spawn(async move |this, cx| {
            let report = cx.background_spawn(async { service::doctor() }).await;
            let _ = this.update(cx, |view, cx| {
                view.report = Some(report);
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn operation(
        &mut self,
        work: impl FnOnce() -> Result<Value, service::Fail> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.fixture || self.operation_busy {
            return;
        }
        self.operation_busy = true;
        self.error = None;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { work() }).await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(_) => {
                        view.collect = service::collect_info();
                        view.notice = Some(view.text("已更新", "Updated").into());
                        view.fetch_details(cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn save_project_display(&mut self, cx: &mut Context<Self>) {
        if self.operation_busy || self.fixture {
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        let selection = id.clone();
        let sign = self.sign_input.read(cx).value().to_string();
        let hidden = !self.preferences.named;
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    service::set_display(id.clone(), hidden, sign)?;
                    service::project_details(id)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(details) => {
                        if view.selected.as_ref() == Some(&selection) {
                            view.details = Some(details);
                            view.rebuild_tiles(cx);
                        }
                        view.notice =
                            Some(view.text("显示设置已保存", "Display settings saved").into());
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn set_integration(&mut self, which: &str, on: bool, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let which = which.to_string();
        let lang = self.preferences.language.code().to_string();
        self.operation(
            move || service::set_integration(id, which, on, Some(lang)),
            cx,
        );
    }
    fn remove_project(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        if !self.remove_confirm {
            self.remove_confirm = true;
            cx.notify();
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.panel = None;
        let removed = id.clone();
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::remove_project(id) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(()) => {
                        if view.selected.as_ref() == Some(&removed) {
                            view.selected = None;
                            view.raw = None;
                        }
                        view.refresh(cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    fn link_folder(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let lang = self.preferences.language.code().to_string();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.text("关联项目路径", "Link project path").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                if let Ok(Ok(Some(paths))) = choice {
                    if let Some(path) = paths.first() {
                        let path = path.to_string_lossy().into_owned();
                        view.operation(
                            move || service::link_path(id, path, Some(false), Some(lang)),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        }));
    }
    fn unlink_path(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.operation(move || service::link_path(id, path, Some(true), None), cx);
    }
    fn show_all_projects(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some("__all__".into());
        self.preferences.last_project = self.selected.clone();
        self.persist();
        if let Some(background) = &self.background {
            background.watch_project(Some("__all__".into()));
        }
        self.details = None;
        self.loading = true;
        self.panel = None;
        self.raw = None;
        self.render_archive = None;
        let lang = self.preferences.language.code().to_string();
        self.archive_task=Some(cx.spawn(async move|this,cx|{let result:Result<(Value,Vec<Value>),String>=cx.background_spawn(async move{let collection=service::project_archives().map_err(|e|e.to_string())?;let projects=collection.as_array().into_iter().flatten().filter_map(|p|p.get("raw").filter(|v|v.is_object()).cloned()).collect::<Vec<_>>();if projects.is_empty(){return Err("No project archives are available".into());}let result=renderer::request(json!({"version":1,"id":"all-projects","op":"merge","projects":projects,"lang":lang}),renderer::Cancellation::default())?;Ok((result["raw"].clone(),projects))}).await;let _=this.update(cx,|view,cx|{if view.archive_generation!=generation{return;}view.loading=false;match result{Ok((raw,projects))=>{view.raw=Some(Arc::new(raw));view.all_archives=Arc::new(projects);view.load_catalog(cx);},Err(error)=>view.error=Some(error)}cx.notify();});}));
    }
    fn selected_piece(&self) -> Option<&Value> {
        self.current_tile().and_then(|t| self.piece(&t.spec.piece))
    }
    fn can_export(&self) -> bool {
        self.raw.is_some()
            && self.current_tile().is_some()
            && !self.export_prompt_busy
            && (self.export_format != "gif"
                || self.selected_piece().is_some_and(|p| p["animated"] == true))
    }
    fn export_request(&self) -> Result<Value, String> {
        let raw = self.raw.as_ref().ok_or("No archive")?;
        let catalog = self.catalog.as_ref().ok_or("No artwork catalog")?;
        let tile = self.current_tile().ok_or("No artwork selected")?;
        let pieces = self
            .style()
            .and_then(|s| s["pieces"].as_array())
            .ok_or("No pieces")?;
        let piece_index = pieces
            .iter()
            .position(|p| p["id"] == tile.spec.piece)
            .ok_or("No piece")?;
        let variant_index = pieces[piece_index]["variants"]
            .as_array()
            .and_then(|vs| {
                vs.iter()
                    .position(|v| v["id"].as_str() == tile.spec.variant.as_deref())
            })
            .unwrap_or(0);
        let mut snapshot = export_with_preferences(
            raw,
            catalog,
            (self.style_index, piece_index, variant_index),
            &self.preferences,
            &self.export_format,
            &self.export_size,
            self.transparent,
            &self.font_mode,
        )?;
        if !self.all_archives.is_empty() {
            snapshot["projects"] = json!(self.all_archives.as_ref());
        }
        if let Some(blame) = &self.blame {
            snapshot["blame"] = blame.clone();
        }
        if let Some(sign) = self
            .details
            .as_ref()
            .and_then(|d| d["config"]["display"]["sign"].as_str())
        {
            snapshot["sign"] = json!(sign);
        }
        Ok(snapshot)
    }
    fn load_export_history(
        &mut self,
        result: Result<Value, service::Fail>,
        cx: &mut Context<Self>,
    ) {
        if self.fixture {
            return;
        }
        let snapshot = match result {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        match crate::export_jobs::Queue::from_snapshot(snapshot["queue"].clone()) {
            Ok(queue) => self.export_queue = queue,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        }
        let (tx, rx) = async_channel::bounded(1);
        self.export_journal = Some(crate::export_jobs::Journal::new(
            snapshot["revision"].as_u64().unwrap_or(0),
            Arc::new(move || {
                let _ = tx.try_send(());
            }),
        ));
        self.export_journal_task = Some(cx.spawn(async move |this, cx| {
            while rx.recv().await.is_ok() {
                if this
                    .update(cx, |view, cx| {
                        if let Some(error) = view
                            .export_journal
                            .as_ref()
                            .and_then(|journal| journal.take_error())
                        {
                            view.error = Some(error);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    fn persist_exports(&self) {
        if let Some(journal) = &self.export_journal {
            journal.submit(self.export_queue.clone());
        }
    }
    fn export_artwork(&mut self, _: &ExportArtwork, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() {
            return;
        }
        let snapshot = match self.export_request() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let directory = match service::export_dir() {
            Ok(directory) => directory,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let filename = export_filename(&snapshot);
        let prompt = cx.prompt_for_new_path(&directory, Some(&filename));
        self.export_prompt_busy = true;
        self.export_dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                view.export_prompt_busy = false;
                match choice {
                    Ok(Ok(Some(path))) => match view.export_queue.enqueue(snapshot, path, filename)
                    {
                        Ok(queued) => {
                            view.notice = Some(
                                view.text(
                                    if queued.deduplicated {
                                        "该任务已在导出队列中"
                                    } else {
                                        "已加入导出队列"
                                    },
                                    if queued.deduplicated {
                                        "This export is already queued"
                                    } else {
                                        "Export queued"
                                    },
                                )
                                .into(),
                            );
                            view.persist_exports();
                            view.pump_exports(cx);
                        }
                        Err(error) => view.error = Some(error),
                    },
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn pump_exports(&mut self, cx: &mut Context<Self>) {
        if self.export_active.is_some() {
            return;
        }
        let Some(job) = self.export_queue.start_next() else {
            return;
        };
        let id = job.id;
        let attempt = job.attempt;
        self.export_active = Some((id, attempt));
        self.export_stage = Some(ExportStage::Rendering);
        self.export_label = job.label.clone();
        self.export_cancel_requested = false;
        let cancellation = renderer::Cancellation::default();
        self.export_cancellation = Some(cancellation.clone());
        self.persist_exports();
        self.export_task = Some(cx.spawn(async move |this, cx| {
            let render_cancel = cancellation.clone();
            let path = job.path;
            let output_path = path.clone();
            let result = cx
                .background_spawn(async move {
                    let output = renderer::request(job.request.materialize(), render_cancel)?;
                    renderer::decode_export(&output, &job.format)
                })
                .await;
            let result = match result {
                Ok(bytes) => {
                    if this
                        .update(cx, |view, cx| {
                            if view.export_active != Some((id, attempt)) {
                                return;
                            }
                            view.export_queue.mark_writing(id, attempt);
                            view.export_stage = Some(ExportStage::Writing);
                            view.persist_exports();
                            cx.notify();
                        })
                        .is_err()
                    {
                        cancellation.cancel();
                        return;
                    }
                    cx.background_spawn(async move {
                        renderer::write_export(&output_path, &bytes, cancellation)
                    })
                    .await
                }
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |view, cx| {
                if view.export_active != Some((id, attempt)) {
                    return;
                }
                let cancelled = view.export_cancel_requested && result.is_err();
                if cancelled {
                    view.export_queue.finish_cancelled(id, attempt);
                    view.notice = Some(view.text("已取消导出", "Export cancelled").into());
                } else {
                    match &result {
                        Ok(()) => {
                            view.notice = Some(format!(
                                "{} {}",
                                view.text("已保存", "Saved"),
                                path.display()
                            ))
                        }
                        Err(error) => view.error = Some(error.clone()),
                    }
                    view.export_queue.finish(id, attempt, result);
                }
                view.export_active = None;
                view.export_stage = None;
                view.export_cancellation = None;
                view.export_cancel_requested = false;
                view.persist_exports();
                view.pump_exports(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn retry_export_job(&mut self, id: u64, cx: &mut Context<Self>) {
        match self.export_queue.retry(id) {
            Ok(()) => {
                self.persist_exports();
                self.pump_exports(cx);
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }
    fn cancel_export_job(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.export_active.is_some_and(|active| active.0 == id) {
            if let Some(cancellation) = &self.export_cancellation {
                cancellation.cancel();
                self.export_cancel_requested = true;
            }
        } else {
            self.export_queue.cancel_queued(id);
            self.persist_exports();
        }
        cx.notify();
    }
    fn cancel_export(&mut self, _: &CancelExport, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.export_cancellation {
            cancellation.cancel();
            self.export_cancel_requested = true;
            cx.notify();
        }
    }

    fn cancel_current(&mut self, _: &CancelCurrent, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_stage.is_some() {
            self.cancel_export(&CancelExport, window, cx);
        } else if self.panel.is_some() {
            self.close_panel(window, cx);
        } else if self.focused {
            self.close_focus(cx);
        } else if self.export_open {
            self.export_open = false;
            cx.notify();
        } else {
            self.cancel(&CancelAnalysis, window, cx);
        }
    }
    fn copy_export(&mut self, cx: &mut Context<Self>) {
        if self.operation_busy {
            return;
        }
        let mut request = match self.export_request() {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let format = if ["svg", "html", "md", "txt", "json", "embed"]
            .contains(&self.export_format.as_str())
        {
            self.export_format.clone()
        } else {
            "svg".into()
        };
        request["format"] = json!(format);
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let output = renderer::request(request, renderer::Cancellation::default())?;
                    let bytes = renderer::decode_export(&output, &format)?;
                    String::from_utf8(bytes).map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(text) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        view.notice = Some(view.text("已复制", "Copied").into());
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(24.));
        let dx: f32 = delta.x.into();
        let dy: f32 = delta.y.into();
        if event.modifiers.control || event.modifiers.platform {
            if !self.focused && self.preferences.view != "roam" {
                self.set_view("roam", cx);
            }
            self.zoom_at((-dy * 0.006).exp(), self.pointer.0, self.pointer.1);
        } else {
            self.viewport.y += dy;
            if self.preferences.view == "roam" || self.focused {
                self.viewport.x += dx;
            }
        }
        self.hover = None;
        cx.notify();
        cx.stop_propagation();
    }
    fn pointer_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.panel.is_some() {
            self.hover = None;
            return;
        }
        let x: f32 = event.position.x.into();
        let y: f32 = event.position.y.into();
        let local = (x, y - 60.);
        self.pointer = local;
        if let Some(drag) = &mut self.drag {
            if event.pressed_button == Some(MouseButton::Left) {
                let dx = local.0 - drag.last.0;
                let dy = local.1 - drag.last.1;
                if (local.0 - drag.start.0).abs() + (local.1 - drag.start.1).abs() > 6. {
                    drag.moved = true;
                }
                if drag.tile.is_none() && (self.preferences.view == "roam" || self.focused) {
                    self.viewport.x += dx;
                    self.viewport.y += dy;
                }
                drag.last = local;
                self.hover = None;
                cx.notify();
                return;
            } else {
                self.drag = None;
            }
        }
        let mut found = None;
        for (index, tile) in self.tiles.iter().enumerate() {
            if self.focused && self.selected_tile != Some(index) {
                continue;
            }
            let rect = self.screen_rect(index);
            if !rect.contains(local.0, local.1) {
                continue;
            }
            let art = if self.focused {
                self.playback
                    .as_ref()
                    .and_then(|p| p.frames.get(&p.index))
                    .map(|f| f.0.as_ref())
                    .or(tile.art.as_deref())
            } else {
                tile.art.as_deref()
            };
            if let Some(art) = art {
                let ax = (local.0 - rect.x) * art.w / rect.w;
                let ay = (local.1 - rect.y) * art.h / rect.h;
                found = art
                    .hits
                    .iter()
                    .find(|h| {
                        Rect {
                            x: h.x,
                            y: h.y,
                            w: h.w,
                            h: h.h,
                        }
                        .contains(ax, ay)
                    })
                    .cloned();
            }
            break;
        }
        if found.is_some()
            || found.as_ref().map(|h| (&h.t, &h.d)) != self.hover.as_ref().map(|h| (&h.t, &h.d))
        {
            self.hover = found;
            cx.notify();
        }
    }
    fn pointer_up(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.moved {
            return;
        }
        let Some(from) = drag.tile else {
            return;
        };
        let x: f32 = event.position.x.into();
        let y: f32 = event.position.y.into();
        let local = (x, y - 60.);
        if local.1 >= self.viewport_size.1 - 12. {
            self.selected_tile = Some(from);
            self.export_open = true;
            cx.notify();
            return;
        }
        let to = self
            .rects
            .iter()
            .enumerate()
            .find(|(i, _)| self.screen_rect(*i).contains(local.0, local.1))
            .map(|(i, _)| i);
        if let Some(to) = to.filter(|to| *to != from) {
            let item = self.tiles.remove(from);
            self.tiles.insert(to, item);
            self.selected_tile = Some(to);
            if let Some(id) = self.selected.clone() {
                self.preferences.projects.entry(id).or_default().order =
                    self.tiles.iter().map(|t| t.spec.key.clone()).collect();
                self.persist();
            }
            cx.notify();
        }
    }
}
impl Drop for Creditia {
    fn drop(&mut self) {
        if let Some(control) = &self.animation_control {
            control.cancel();
        }
        if let Some(c) = &self.analysis_cancellation {
            c.cancel();
        }
        if let Some(c) = &self.preview_cancellation {
            c.cancel();
        }
        if let Some(c) = &self.animation_cancellation {
            c.cancel();
        }
        if let Some(c) = &self.export_cancellation {
            c.cancel();
        }
    }
}
impl Creditia {
    fn artwork_element(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let tile = &self.tiles[index];
        let r = self.screen_rect(index);
        let selected = self.selected_tile == Some(index);
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let title = self
            .piece(&tile.spec.piece)
            .and_then(|p| {
                p[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(&tile.spec.piece)
            .to_string();
        let animated_image = self.focused
            && self
                .playback
                .as_ref()
                .is_some_and(|p| p.frames.contains_key(&p.index));
        let image = if self.focused {
            self.playback
                .as_ref()
                .and_then(|p| p.frames.get(&p.index))
                .map(|f| f.1.clone())
                .or(tile.image.clone())
        } else {
            tile.image.clone()
        };
        let mut picture = div()
            .id(("artwork", index))
            .test_support()
            .relative()
            .w(px(r.w))
            .h(px(r.h))
            .overflow_hidden()
            .rounded_lg()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                    view.selected_tile = Some(index);
                    view.drag = Some(DragState {
                        tile: if view.focused { None } else { Some(index) },
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    if event.click_count == 2 {
                        view.select_tile(index, true, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        if let Some(image) = image {
            if let (Some(region), Some(art)) = (
                if !animated_image {
                    tile.image_region
                } else {
                    None
                },
                tile.art.as_deref(),
            ) {
                picture = picture.child(
                    img(image)
                        .id(("artwork-image", index))
                        .absolute()
                        .left(px(region.x / art.w * r.w))
                        .top(px(region.y / art.h * r.h))
                        .w(px(region.w / art.w * r.w))
                        .h(px(region.h / art.h * r.h))
                        .object_fit(ObjectFit::Fill),
                );
            } else {
                picture = picture.child(
                    img(image)
                        .id(("artwork-image", index))
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .with_fallback(|| {
                            div().child("Unable to decode artwork").into_any_element()
                        }),
                );
            }
        } else {
            picture = picture
                .bg(cx.theme().muted)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_sm().text_color(muted).child(
                    if tile.error.as_deref() == Some("__empty__") {
                        self.text("暂无相应数据", "No data for this piece")
                            .to_string()
                    } else if let Some(error) = &tile.error {
                        error.clone()
                    } else {
                        self.text("正在排版…", "Preparing artwork…").to_string()
                    },
                ))
                .when(
                    tile.error
                        .as_deref()
                        .is_some_and(|error| error != "__empty__"),
                    |picture| {
                        picture.child(
                            Button::new(("retry-artwork", index))
                                .ghost()
                                .label(self.text("重试", "Retry"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(tile) = view.tiles.get_mut(index) {
                                        tile.error = None;
                                        tile.loading = false;
                                    }
                                    cx.notify();
                                })),
                        )
                    },
                );
        }
        let mut card = div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .child(picture);
        if !self.focused {
            let variant = tile.spec.variant.as_deref().unwrap_or("").to_string();
            card = card.child(
                div()
                    .absolute()
                    .top(px(-26.))
                    .left_0()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .h(px(22.))
                    .text_xs()
                    .text_color(muted)
                    .child(div().flex().gap_2().child(title).child(variant))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                Button::new(("tile-focus", index))
                                    .ghost()
                                    .label(self.text("放大", "Open"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.select_tile(index, true, cx)
                                    })),
                            )
                            .child(
                                Button::new(("tile-export", index))
                                    .ghost()
                                    .label(self.text("导出", "Export"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.selected_tile = Some(index);
                                        view.export_open = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if selected {
            card = card.border_1().border_color(border);
        }
        card.into_any_element()
    }
    fn render_stage(&self, cx: &mut Context<Self>) -> AnyElement {
        let visible = Rect {
            x: -50.,
            y: -80.,
            w: self.viewport_size.0 + 100.,
            h: self.viewport_size.1 + 160.,
        };
        let mut stage = div()
            .id("artwork-stage")
            .test_support()
            .relative()
            .size_full()
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(|view, event, _, cx| view.wheel(event, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.drag = Some(DragState {
                        tile: None,
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    view.hover = None;
                    cx.notify();
                }),
            );
        if self.loading {
            stage = stage.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.text("正在读取作品库…", "Loading project library…")),
            );
        } else if self.raw.is_none() {
            let empty = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .size_full()
                .child(
                    div()
                        .text_3xl()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(img(self.logo_image.clone()).w(px(56.)).h(px(56.)))
                        .child("/creditia"),
                )
                .child(div().text_color(cx.theme().muted_foreground).child(
                    if self.selected.is_some() {
                        self.text(
                            "分析项目，生成它的作品。",
                            "Analyze this project to create its artwork.",
                        )
                    } else {
                        self.text(
                            "代码背后的工具与模型，成为一面作品墙。",
                            "The tools and models behind your code, on a wall of artwork.",
                        )
                    },
                ))
                .child(
                    Button::new("discover-projects")
                        .ghost()
                        .label(self.text("查找本机项目…", "Find local projects…"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Projects, window, cx)
                        })),
                )
                .child(
                    Button::new("first-project")
                        .label(if self.selected.is_some() {
                            self.text("分析项目", "Analyze project")
                        } else {
                            self.text("添加项目…", "Add project…")
                        })
                        .on_click(cx.listener(|view, _, window, cx| {
                            if view.selected.is_some() {
                                view.start_job(None, cx)
                            } else {
                                view.open_project(&OpenProject, window, cx)
                            }
                        })),
                );
            stage = stage.child(empty);
        } else {
            for index in 0..self.tiles.len() {
                if (!self.focused || self.selected_tile == Some(index))
                    && self.screen_rect(index).intersects(visible)
                {
                    stage = stage.child(self.artwork_element(index, cx));
                }
            }
        }
        if let Some(hit) = &self.hover {
            let w = 300.;
            let h = 54. + hit.d.len().min(8) as f32 * 22.;
            let x = if self.pointer.0 + 20. + w > self.viewport_size.0 {
                self.pointer.0 - w - 14.
            } else {
                self.pointer.0 + 20.
            }
            .max(8.);
            let y = if self.pointer.1 + 20. + h > self.viewport_size.1 {
                self.pointer.1 - h - 12.
            } else {
                self.pointer.1 + 20.
            }
            .max(8.);
            stage = stage.child(
                div()
                    .id("evidence-tooltip")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .p_3()
                    .rounded_lg()
                    .bg(cx.theme().foreground)
                    .text_color(cx.theme().background)
                    .text_sm()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(hit.t.clone()))
                    .children(
                        hit.d
                            .iter()
                            .take(8)
                            .map(|line| div().mt_1().child(line.clone())),
                    ),
            );
        }
        stage.into_any_element()
    }
    fn render_export_dock(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let mut dock = div()
            .id("export-dock")
            .h(px(self.dock_height))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap_2()
            .px_5()
            .py_3()
            .border_t_1()
            .border_color(border);
        if !self.export_open {
            return dock
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(self.text("拖动作品到这里导出", "Drag an artwork here to export")),
                )
                .child(
                    Button::new("open-export-dock")
                        .ghost()
                        .label(self.text("导出坞", "Exports"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.export_open = true;
                            cx.notify();
                        })),
                )
                .into_any_element();
        }
        let formats = self
            .catalog
            .as_ref()
            .and_then(|c| c["formats"].as_array())
            .cloned()
            .unwrap_or_default();
        let mut buttons = div().flex().flex_wrap().gap_1();
        for (index, format) in formats.iter().enumerate() {
            let Some(id) = format["id"].as_str() else {
                continue;
            };
            let id = id.to_string();
            let chosen = self.export_format == id;
            let disabled =
                id == "gif" && self.selected_piece().is_none_or(|p| p["animated"] != true);
            buttons = buttons.child(
                Button::new(("format", index))
                    .ghost()
                    .label(id.to_uppercase())
                    .when(chosen, |button| button.bg(cx.theme().muted))
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.export_format = id.clone();
                        view.preferences.export_format = id.clone();
                        view.persist();
                        cx.notify();
                    })),
            );
        }
        let mut sizes = div().flex().flex_wrap().gap_1();
        for (index, size) in self
            .catalog
            .as_ref()
            .and_then(|c| c["sizes"].as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(id) = size["id"].as_str() else {
                continue;
            };
            let id = id.to_string();
            let label = if id == "native" {
                self.text("原尺寸", "Original").to_string()
            } else {
                format!("{} {}×{}", id, size["w"], size["h"])
            };
            let chosen = self.export_size == id;
            sizes = sizes.child(
                Button::new(("size", index))
                    .ghost()
                    .label(label)
                    .when(chosen, |b| b.bg(cx.theme().muted))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.export_size = id.clone();
                        view.preferences.export_size = id.clone();
                        view.persist();
                        cx.notify();
                    })),
            );
        }
        dock = dock
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(buttons)
                    .child(
                        Button::new("close-export-dock")
                            .ghost()
                            .label("×")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.export_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(sizes)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("transparent")
                            .ghost()
                            .label(if self.transparent {
                                self.text("透明背景 ✓", "Transparent ✓")
                            } else {
                                self.text("透明背景", "Transparent")
                            })
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.transparent = !view.transparent;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("font-mode")
                            .ghost()
                            .disabled(!["svg", "html"].contains(&self.export_format.as_str()))
                            .label(if self.font_mode == "embed" {
                                self.text("字体嵌入", "Embed fonts")
                            } else {
                                self.text("文字转路径", "Outline fonts")
                            })
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.font_mode = if view.font_mode == "embed" {
                                    "outline"
                                } else {
                                    "embed"
                                }
                                .into();
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("copy-export")
                            .ghost()
                            .label(self.text("复制", "Copy"))
                            .disabled(!self.can_export() || self.operation_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.copy_export(cx))),
                    )
                    .child(
                        Button::new("save-export")
                            .primary()
                            .label(self.text("保存…", "Save…"))
                            .disabled(!self.can_export())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.export_artwork(&ExportArtwork, window, cx)
                            })),
                    ),
            );
        dock.into_any_element()
    }
    fn render_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.panel?;
        let title = match panel {
            Panel::Projects => self.text("项目作品库", "Project library"),
            Panel::Styles => self.text("风格", "Styles"),
            Panel::Pieces => self.text("全部作品", "All artworks"),
            Panel::Layouts => self.text("版式", "Layouts"),
            Panel::Diagnostics => self.text("环境检查", "Diagnostics"),
            Panel::Blame => self.text("逐行归因", "Line attribution"),
            Panel::Exports => self.text("导出任务", "Export tasks"),
            Panel::Settings => self.text("设置", "Settings"),
            Panel::Project => self.text("项目设置", "Project settings"),
            Panel::Details => self.text("归因与证据", "Attribution & evidence"),
        };
        let muted = cx.theme().muted_foreground;
        let mut body = div().flex().flex_col().gap_3();
        match panel {
            Panel::Projects => {
                let search = self.search_input.read(cx).value().to_lowercase();
                body = body.child(Input::new(&self.search_input));
                body = body.child(
                    Button::new("all-projects")
                        .ghost()
                        .label(self.text("全部项目汇总", "All projects"))
                        .on_click(cx.listener(|view, _, _, cx| view.show_all_projects(cx))),
                );
                for (index, project) in self.projects.iter().enumerate().filter(|(_, p)| {
                    search.is_empty()
                        || p.name.to_lowercase().contains(&search)
                        || p.path.to_lowercase().contains(&search)
                }) {
                    let id = project.id.clone();
                    body = body.child(
                        Button::new(("choose-project", index))
                            .ghost()
                            .label(format!(
                                "{}  {}",
                                project.name,
                                if project.exists { "" } else { "◌" }
                            ))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.panel = None;
                                view.all_archives = Arc::new(vec![]);
                                view.select(id.clone(), cx);
                            })),
                    );
                }
                if self.discover_busy {
                    body = body.child(self.text("正在查找本机项目…", "Finding local projects…"));
                }
                if let Some(found) = &self.discovered {
                    if !found.is_empty() {
                        body =
                            body.child(div().mt_3().text_color(muted).child(
                                self.text("本机发现的项目", "Projects found on this computer"),
                            ));
                    }
                    for (index, project) in found.iter().enumerate() {
                        let path = project["path"].as_str().unwrap_or("").to_string();
                        let picked = self.discovery_selected.contains(&path);
                        let name = project["name"].as_str().unwrap_or(&path).to_string();
                        body = body.child(
                            Button::new(("discovered", index))
                                .ghost()
                                .label(format!("{} {}", if picked { "✓" } else { "○" }, name))
                                .disabled(self.job_busy)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(index) = view
                                        .discovery_selected
                                        .iter()
                                        .position(|value| value == &path)
                                    {
                                        view.discovery_selected.remove(index);
                                    } else {
                                        view.discovery_selected.push(path.clone());
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    if !found.is_empty() {
                        body = body.child(
                            Button::new("add-selected")
                                .label(format!(
                                    "{} ({})",
                                    self.text("添加所选项目", "Add selected projects"),
                                    self.discovery_selected.len()
                                ))
                                .disabled(self.job_busy || self.discovery_selected.is_empty())
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.add_queue.extend(view.discovery_selected.drain(..));
                                    view.panel = None;
                                    view.start_next_project(cx);
                                    cx.notify();
                                })),
                        );
                    }
                }
                body = body.child(
                    Button::new("add-project")
                        .label(self.text("添加文件夹…", "Add folder…"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_project(&OpenProject, window, cx)
                        })),
                );
            }
            Panel::Styles => {
                let mut choices = div().flex().flex_wrap().gap_2();
                for (index, style) in self
                    .catalog
                    .as_ref()
                    .and_then(|c| c["styles"].as_array())
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let name = style[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                    choices = choices.child(
                        Button::new(("style", index))
                            .ghost()
                            .label(name)
                            .when(index == self.style_index, |b| b.bg(cx.theme().muted))
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.choose_style(index, cx)),
                            ),
                    );
                }
                body = body.child(choices);
            }
            Panel::Pieces => {
                let mut choices = div().flex().flex_wrap().gap_2();
                for (index, piece) in self
                    .style()
                    .and_then(|s| s["pieces"].as_array())
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let id = piece["id"].as_str().unwrap_or("").to_string();
                    let name = piece[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                    .unwrap_or(&id)
                    .to_string();
                    choices =
                        choices.child(Button::new(("piece", index)).ghost().label(name).on_click(
                            cx.listener(move |view, _, _, cx| view.select_piece(id.clone(), cx)),
                        ));
                }
                body = body.child(choices);
            }
            Panel::Layouts => {
                for (index, variant) in self
                    .selected_piece()
                    .and_then(|p| p["variants"].as_array())
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let id = variant["id"].as_str().unwrap_or("").to_string();
                    let label = variant[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                    .unwrap_or(&id)
                    .to_string();
                    let active =
                        self.current_tile().and_then(|t| t.spec.variant.as_ref()) == Some(&id);
                    body = body.child(
                        Button::new(("variant", index))
                            .ghost()
                            .label(label)
                            .when(active, |b| b.bg(cx.theme().muted))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.choose_variant(id.clone(), cx);
                                view.panel = None;
                                cx.notify();
                            })),
                    );
                }
            }
            Panel::Exports => {
                if self.export_queue.jobs().is_empty() {
                    body = body.child(self.text("尚无导出任务", "No export jobs yet"));
                }
                for (index, job) in self.export_queue.jobs().iter().enumerate().rev() {
                    let id = job.id;
                    let state = match job.state {
                        crate::export_jobs::State::Queued => self.text("等待", "Queued"),
                        crate::export_jobs::State::Rendering => self.text("正在导出", "Rendering"),
                        crate::export_jobs::State::Writing => self.text("正在保存", "Saving"),
                        crate::export_jobs::State::Saved => self.text("已保存", "Saved"),
                        crate::export_jobs::State::Failed => self.text("失败", "Failed"),
                        crate::export_jobs::State::Cancelled => self.text("已取消", "Cancelled"),
                        crate::export_jobs::State::Interrupted => {
                            self.text("已中断", "Interrupted")
                        }
                    };
                    let mut row = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child(format!("{} · {}", job.label, state)));
                    if let Some(error) = &job.error {
                        row = row.child(div().text_xs().text_color(muted).child(error.clone()));
                    }
                    let mut actions = div().flex().gap_1();
                    if job.state == crate::export_jobs::State::Saved {
                        let path = job.path.clone();
                        actions = actions.child(
                            Button::new(("export-reveal", index))
                                .ghost()
                                .label(self.text("打开所在位置", "Reveal file"))
                                .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                        );
                    }
                    if matches!(
                        job.state,
                        crate::export_jobs::State::Failed
                            | crate::export_jobs::State::Cancelled
                            | crate::export_jobs::State::Interrupted
                    ) {
                        actions =
                            actions.child(
                                Button::new(("export-retry", index))
                                    .ghost()
                                    .label(self.text("重试", "Retry"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.retry_export_job(id, cx)
                                    })),
                            );
                    }
                    if job.state.pending() {
                        actions = actions.child(
                            Button::new(("export-cancel", index))
                                .ghost()
                                .label(self.text("取消", "Cancel"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.cancel_export_job(id, cx)
                                })),
                        );
                    } else {
                        actions = actions.child(
                            Button::new(("export-remove", index))
                                .ghost()
                                .label(self.text("移除记录", "Remove record"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.export_queue.remove(id);
                                    view.persist_exports();
                                    cx.notify();
                                })),
                        );
                    }
                    body = body.child(row.child(actions));
                }
            }
            Panel::Blame => {
                body = body
                    .child(div().text_sm().text_color(muted).child(self.text(
                        "输入仓库中的相对路径与行号。",
                        "Enter a repository-relative file path and line number.",
                    )))
                    .child(Input::new(&self.blame_file))
                    .child(Input::new(&self.blame_line))
                    .child(
                        Button::new("load-blame")
                            .label(self.text("追溯这一行", "Trace this line"))
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.load_blame(cx))),
                    );
            }
            Panel::Diagnostics => {
                if let Some(report) = &self.report {
                    for (label, value) in
                        crate::ui_reports::report_rows(report, &self.preferences.language)
                    {
                        body = body.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(label),
                                )
                                .child(div().text_sm().text_color(muted).child(value)),
                        );
                    }
                } else {
                    body = body.child(self.text("正在检查…", "Checking environment…"));
                }
            }
            Panel::Settings => {
                body = body
                    .child(
                        Button::new("language")
                            .ghost()
                            .label(self.text("语言：中文 → English", "Language: English → 中文"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.toggle_language(&ToggleLanguage, window, cx)
                            })),
                    )
                    .child(
                        Button::new("appearance")
                            .ghost()
                            .label(format!(
                                "{}：{:?}",
                                self.text("外观", "Appearance"),
                                self.preferences.appearance
                            ))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cycle_appearance(&CycleAppearance, window, cx)
                            })),
                    )
                    .child(
                        Button::new("reduced-motion")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text("减少动态效果", "Reduced motion"),
                                if self.preferences.reduced_motion {
                                    "✓"
                                } else {
                                    "○"
                                }
                            ))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.toggle_motion(&ToggleMotion, window, cx)
                            })),
                    )
                    .child(
                        Button::new("names-settings")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text("显示贡献者姓名", "Contributor names"),
                                if self.preferences.named { "✓" } else { "○" }
                            ))
                            .on_click(cx.listener(|view, _, _, cx| view.set_names(cx))),
                    )
                    .child(
                        Button::new("auto-analyze")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text(
                                    "有新提交时自动分析",
                                    "Analyze new commits automatically"
                                ),
                                if self.preferences.auto_analyze {
                                    "✓"
                                } else {
                                    "○"
                                }
                            ))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.preferences.auto_analyze = !view.preferences.auto_analyze;
                                view.persist();
                                if let Some(bg) = &view.background {
                                    bg.refresh();
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_color(muted)
                            .child(self.text("后台采集", "Background collection")),
                    );
                let mut logos = div().flex().flex_wrap().gap_1();
                for (id, label) in [
                    ("star", "✳"),
                    ("star-mono", "✳ Mono"),
                    ("orb", "Orb"),
                    ("ring", "Ring"),
                    ("color", "Color"),
                    ("mono", "Mono"),
                ] {
                    logos = logos.child(
                        Button::new(id)
                            .ghost()
                            .label(label)
                            .when(self.preferences.logo == id, |b| b.bg(cx.theme().muted))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.preferences.logo = id.into();
                                ImageSource::Image(view.logo_image.clone()).remove_asset(cx);
                                view.logo_image = logo_image(id);
                                view.persist();
                                cx.notify();
                            })),
                    );
                }
                body = body
                    .child(div().text_color(muted).child(self.text("标志", "Logo")))
                    .child(logos);
                let on = self.collect["on"].as_bool().unwrap_or(false);
                let agent = self.collect["agent"]["on"].as_bool().unwrap_or(false);
                let supported = self.collect["agent"]["supported"]
                    .as_bool()
                    .unwrap_or(false);
                body = body
                    .child(
                        Button::new("collect-toggle")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text("自动采集日志", "Collect logs automatically"),
                                if on { "✓" } else { "○" }
                            ))
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.operation(move || service::set_collect(!on), cx)
                            })),
                    )
                    .child(
                        Button::new("collect-now")
                            .label(self.text("立即采集", "Collect now"))
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.operation(service::collect_now, cx)
                            })),
                    )
                    .child(
                        Button::new("collect-agent")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text("退出后继续采集", "Continue after quitting"),
                                if agent { "✓" } else { "○" }
                            ))
                            .disabled(!supported || self.operation_busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.operation(move || service::set_collect_agent(!agent), cx)
                            })),
                    )
                    .child(
                        Button::new("export-directory")
                            .ghost()
                            .label(self.text("更改导出目录…", "Change export directory…"))
                            .on_click(
                                cx.listener(|view, _, _, cx| view.choose_export_directory(cx)),
                            ),
                    )
                    .child(
                        Button::new("open-export-directory")
                            .ghost()
                            .label(self.text("打开导出目录", "Open export directory"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                match service::export_dir() {
                                    Ok(path) => cx.reveal_path(&path),
                                    Err(error) => view.error = Some(error.to_string()),
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("reset-export-directory")
                            .ghost()
                            .label(self.text("恢复默认导出目录", "Reset export directory"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.operation(
                                    || service::reset_export_dir().map(Value::String),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("open-library")
                            .ghost()
                            .label(self.text("打开本地档案库", "Open local archive library"))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.reveal_path(Path::new(&service::library_path()))
                            })),
                    )
                    .child(
                        Button::new("diagnostics")
                            .ghost()
                            .label(self.text("环境检查", "Diagnostics"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.load_diagnostics(cx);
                            })),
                    );
                for (label, value) in
                    crate::ui_reports::report_rows(&self.collect, &self.preferences.language)
                {
                    body = body.child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("{label}: {value}")),
                    );
                }
                body = body.child(div().mt_3().text_xs().text_color(muted).child(format!(
                    "Creditia {} · GPUI Kit\n{}",
                    env!("CARGO_PKG_VERSION"),
                    service::library_path()
                )));
            }
            Panel::Project => {
                if let Some(project) = self.current() {
                    body = body
                        .child(div().text_lg().child(project.name.clone()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(project.path.clone()),
                        );
                }
                body = body.child(Input::new(&self.sign_input)).child(
                    Button::new("save-display")
                        .label(self.text("保存署名与姓名设置", "Save signature and name settings"))
                        .disabled(
                            self.operation_busy
                                || self.details.is_none()
                                || self.sign_loaded_for != self.selected,
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.save_project_display(cx))),
                );
                let current_auto = self.project_preferences().auto_analyze;
                let mut auto = div().flex().gap_1();
                for (index, (mode, label)) in [
                    (None, self.text("跟随全局", "Global default")),
                    (Some(true), self.text("自动分析", "Analyze automatically")),
                    (Some(false), self.text("手动分析", "Analyze manually")),
                ]
                .into_iter()
                .enumerate()
                {
                    auto = auto.child(
                        Button::new(("project-auto", index))
                            .ghost()
                            .label(label)
                            .when(current_auto == mode, |b| b.bg(cx.theme().muted))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(id) = view.selected.clone() {
                                    view.preferences
                                        .projects
                                        .entry(id)
                                        .or_default()
                                        .auto_analyze = mode;
                                    view.persist();
                                    if let Some(background) = &view.background {
                                        background.refresh();
                                    }
                                }
                                cx.notify();
                            })),
                    );
                }
                body = body.child(auto);
                for (which, zh, en) in [
                    ("hook", "提交时记录证据", "Record evidence on commit"),
                    ("release", "发布时生成作品", "Generate artwork on release"),
                ] {
                    let on = self
                        .details
                        .as_ref()
                        .and_then(|d| {
                            d["integrations"][which]["enabled"]
                                .as_bool()
                                .or_else(|| d["integrations"][which]["installed"].as_bool())
                        })
                        .unwrap_or(false);
                    body = body.child(
                        Button::new(which)
                            .ghost()
                            .label(format!(
                                "{} {}",
                                self.text(zh, en),
                                if on { "✓" } else { "○" }
                            ))
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.set_integration(which, !on, cx)
                            })),
                    );
                }
                if let Some(details) = &self.details {
                    body = body.child(div().text_sm().text_color(muted).child(format!(
                        "{} {} · {} {}",
                        self.text("证据", "Evidence"),
                        details["evidence"],
                        self.text("会话", "Sessions"),
                        details["sessions"].as_array().map(Vec::len).unwrap_or(0)
                    )));
                    for (index, path) in details["paths"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .enumerate()
                    {
                        let manual = path["kind"].as_str() == Some("manual");
                        let path = path["path"].as_str().unwrap_or("").to_string();
                        let unlink = path.clone();
                        body = body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().flex_1().text_sm().child(path))
                                .when(manual, |row| {
                                    row.child(
                                        Button::new(("unlink", index))
                                            .ghost()
                                            .label(self.text("解除关联", "Unlink"))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.unlink_path(unlink.clone(), cx)
                                            })),
                                    )
                                }),
                        );
                    }
                }
                body = body
                    .child(
                        Button::new("evidence-export")
                            .ghost()
                            .label(self.text("导出证据档案…", "Export evidence archive…"))
                            .disabled(self.operation_busy || self.job_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.export_evidence(cx))),
                    )
                    .child(
                        Button::new("evidence-import")
                            .ghost()
                            .label(self.text("导入证据档案…", "Import evidence archive…"))
                            .disabled(self.operation_busy || self.job_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.import_evidence(cx))),
                    )
                    .child(
                        Button::new("blame-query")
                            .ghost()
                            .label(self.text("查询逐行归因…", "Trace a line…"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_panel(Panel::Blame, window, cx)
                            })),
                    );
                body = body
                    .child(
                        Button::new("link-path")
                            .ghost()
                            .label(self.text("关联目录…", "Link directory…"))
                            .on_click(cx.listener(|view, _, _, cx| view.link_folder(cx))),
                    )
                    .child(
                        Button::new("reveal")
                            .ghost()
                            .label(self.text("打开所在位置", "Reveal folder"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.reveal_project(&RevealProject, window, cx)
                            })),
                    )
                    .child(
                        Button::new("remove-project")
                            .ghost()
                            .label(if self.remove_confirm {
                                self.text("确认移除，证据仍保留", "Confirm removal; evidence stays")
                            } else {
                                self.text(
                                    "从作品库移除（保留证据）",
                                    "Remove from library (keep evidence)",
                                )
                            })
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.remove_project(cx))),
                    );
            }
            Panel::Details => {
                for warning in &self.warnings {
                    body = body.child(div().text_sm().child(warning.clone()));
                }
                if let Some(raw) = &self.raw {
                    body = body.child(div().text_sm().text_color(muted).child(format!(
                        "{} {}",
                        self.text("档案生成于", "Archive generated"),
                        raw["generatedAt"].as_str().unwrap_or("—")
                    )));
                    for row in crate::contributions(raw) {
                        let name = if self.preferences.named {
                            row.name
                        } else {
                            self.text("贡献者", "Contributor").to_string()
                        };
                        body = body.child(
                            div()
                                .flex()
                                .justify_between()
                                .child(name)
                                .child(format!("{} / {}", row.lines_added, row.verified_lines)),
                        );
                    }
                    body = body.child(
                        Button::new("copy-details")
                            .label(self.text("复制完整档案", "Copy complete archive"))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.copy_archive(&CopyArchive, window, cx)
                            })),
                    );
                }
            }
        }
        let openness = self.panel_tween.map(Tween::value).unwrap_or(1.);
        let modal = div()
            .id("panel")
            .key_context("CreditiaModal")
            .opacity(openness)
            .mt(px((1. - openness) * 16.))
            .w(px(580.))
            .max_h(px(680.))
            .flex()
            .flex_col()
            .p_5()
            .gap_4()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_lg().child(title))
                    .child(
                        Button::new("panel-close")
                            .track_focus(&self.panel_close_focus)
                            .ghost()
                            .label(self.text("关闭", "Close"))
                            .on_click(
                                cx.listener(|view, _, window, cx| view.close_panel(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .id("panel-body")
                    .overflow_y_scroll()
                    .min_h_0()
                    .child(body),
            )
            .focus_trap("creditia-panel", &self.panel_focus);
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(rgba(0x00000066))
                .flex()
                .items_center()
                .justify_center()
                .child(modal)
                .into_any_element(),
        )
    }
    fn choose_export_directory(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.text("导出目录", "Export directory").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |view, cx| {
                if let Ok(Ok(Some(paths))) = result {
                    if let Some(path) = paths.first() {
                        let path = path.to_string_lossy().into_owned();
                        if let Err(error) = service::edit_app_settings(|settings| {
                            settings["exportDir"] = json!(path)
                        }) {
                            view.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        }));
    }
}
impl Creditia {
    pub fn diagnostic_state(&self) -> Value {
        json!({"view":self.preferences.view,"focused":self.focused,"group":self.group(),"named":self.preferences.named,"selected_tile":self.selected_tile,"selected_piece":self.current_tile().map(|t|t.spec.piece.clone()),"selected_variant":self.current_tile().and_then(|t|t.spec.variant.clone()),"zoom":self.viewport.zoom,"pan_x":self.viewport.x,"pan_y":self.viewport.y,"tile_count":self.tiles.len(),"style":self.preferences.style,"playing":self.playback.as_ref().is_some_and(|p|p.playing),"loading":self.loading,"panel":self.panel.map(|p|match p{Panel::Projects=>"projects",Panel::Styles=>"styles",Panel::Pieces=>"pieces",Panel::Layouts=>"layouts",Panel::Diagnostics=>"diagnostics",Panel::Blame=>"blame",Panel::Exports=>"exports",Panel::Settings=>"settings",Panel::Project=>"project",Panel::Details=>"details"})})
    }
}
impl Render for Creditia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.panel == Some(Panel::Project)
            && self.details.is_some()
            && self.sign_loaded_for != self.selected
        {
            let sign = self
                .details
                .as_ref()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.selected.clone();
        }
        let width: f32 = window.viewport_size().width.into();
        let height: f32 = window.viewport_size().height.into();
        let target = if self.export_open { 148. } else { 0. };
        if target != self.dock_target {
            self.dock_target = target;
            self.dock_tween = (!self.preferences.reduced_motion).then_some(Tween {
                from: self.dock_height,
                to: target,
                started: Instant::now(),
            });
        }
        self.dock_height = if let Some(tween) = self.dock_tween {
            if tween.done() {
                self.dock_tween = None;
                target
            } else {
                window.request_animation_frame();
                tween.value()
            }
        } else {
            target
        };
        let dock_height = self.dock_height;
        if let Some(tween) = self.panel_tween {
            if tween.done() {
                self.panel_tween = None;
                if self.panel_closing {
                    self.panel = None;
                    self.panel_closing = false;
                    self.return_focus
                        .take()
                        .unwrap_or_else(|| self.focus.clone())
                        .focus(window, cx);
                }
            } else {
                window.request_animation_frame();
            }
        }
        if self
            .focus_transition
            .as_ref()
            .is_some_and(|transition| transition.started.elapsed() < Duration::from_millis(180))
        {
            window.request_animation_frame();
        } else {
            self.focus_transition = None;
        }
        self.compute_layout(
            width.max(300.),
            (height - 60. - 56. - 24. - dock_height).max(200.),
        );
        if !self.fixture {
            if crate::platform_ui::dynamic_logo_supported() {
                if let Err(error) = self.platform_ui.apply_logo(window, &self.preferences.logo) {
                    if self.platform_error.as_ref() != Some(&error) {
                        self.notice = Some(error.clone());
                        self.platform_error = Some(error);
                    }
                }
            }
            if crate::platform_ui::progress_supported() {
                let progress = if self.error.is_some() {
                    crate::platform_ui::Progress::Error
                } else if self.job_busy || self.automatic_job.is_some() {
                    self.progress
                        .as_ref()
                        .and_then(analysis_fraction)
                        .map(crate::platform_ui::Progress::Fraction)
                        .unwrap_or(crate::platform_ui::Progress::Indeterminate)
                } else if self.export_stage.is_some() {
                    crate::platform_ui::Progress::Indeterminate
                } else {
                    crate::platform_ui::Progress::None
                };
                if let Err(error) = self.platform_ui.set_progress(window, progress) {
                    if self.platform_error.as_ref() != Some(&error) {
                        self.notice = Some(error.clone());
                        self.platform_error = Some(error);
                    }
                }
            }
        }
        if let Some(background) = &self.background {
            background.set_active(window.is_window_active());
        }
        let animation_visible = self.selected_tile.is_some_and(|index| {
            self.screen_rect(index).intersects(Rect {
                x: 0.,
                y: 0.,
                w: self.viewport_size.0,
                h: self.viewport_size.1,
            })
        });
        if let Some(play) = &mut self.playback {
            if play.playing
                && animation_visible
                && self.focused
                && self.panel.is_none()
                && window.is_window_active()
                && !self.preferences.reduced_motion
            {
                play.elapsed = play.started.elapsed();
                let desired =
                    frame_at(&play.times, &play.timeline, play.elapsed.as_millis() as u64);
                play.requested = desired;
                if let Some(control) = &self.animation_control {
                    control.set_playhead(desired);
                }
                if let Some((&index, _)) = play.frames.range(..=desired).next_back() {
                    play.index = index;
                }
                if !play.timeline.repeat
                    && play.elapsed.as_millis() as u64
                        >= play.timeline.duration_ms + play.timeline.hold_ms
                    && play.index == play.times.len().saturating_sub(1)
                {
                    play.playing = false;
                    if let Some(control) = &self.animation_control {
                        control.pause();
                    }
                } else {
                    window.request_animation_frame();
                }
            } else if play.playing
                && (!animation_visible
                    || !window.is_window_active()
                    || self.panel.is_some()
                    || self.preferences.reduced_motion)
            {
                play.elapsed = play.started.elapsed();
                play.playing = false;
                if let Some(control) = &self.animation_control {
                    control.pause();
                }
            }
        }

        self.schedule_visible(window.scale_factor(), cx);
        if telemetry_enabled() && !self.ready_emitted && self.fixture_images_ready(window, cx) {
            self.ready_emitted = true;
            write_performance_event(
                json!({"event":"ready","width":width,"height":height,"tiles":self.tiles.len(),"loaded":self.tiles.iter().filter(|t|t.image.is_some()).count(),"state":self.diagnostic_state()}),
            );
        }

        if telemetry_enabled() {
            let mut state = self.diagnostic_state();
            state["active"] = json!(window.is_window_active());
            state["ready"] = json!(self.ready_emitted);
            state["focus_ready"] = json!(self.focused && self.fixture_images_ready(window, cx));
            let visible = Rect {
                x: 0.,
                y: 0.,
                w: self.viewport_size.0,
                h: self.viewport_size.1,
            };
            if let Some((index, _)) = self
                .tiles
                .iter()
                .enumerate()
                .find(|(i, t)| t.image.is_some() && self.screen_rect(*i).intersects(visible))
            {
                let r = self.screen_rect(index);
                state["first_tile"] = json!({"x":(r.x+r.w/2.).clamp(20.,self.viewport_size.0-20.),"y":60.+(r.y+r.h/2.).clamp(20.,self.viewport_size.1-20.)});
            }
            if self.telemetry_state.as_ref() != Some(&state) {
                self.telemetry_sequence += 1;
                write_performance_event(
                    json!({"event":"state","sequence":self.telemetry_sequence,"state":state}),
                );
                self.telemetry_state = Some(state);
            }
        }
        let bg = rgb(if cx.theme().is_dark() {
            0x0A0A0A
        } else {
            0xEFEFED
        });
        let glass = rgba(if cx.theme().is_dark() {
            0x1E1E1ECC
        } else {
            0xFFFFFFCC
        });
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let title = if self.selected.as_deref() == Some("__all__") {
            self.text("全部项目", "All projects").to_string()
        } else {
            self.current()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| self.text("项目作品库", "Project library").into())
        };
        let style_name = self
            .style()
            .and_then(|s| {
                s[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(self.text("风格", "Style"))
            .to_string();
        let mut tools = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded_full()
            .bg(glass)
            .border_1()
            .border_color(border);
        let group = self.group();
        for (id, label) in [
            ("auto", self.text("自动", "Auto")),
            ("tool", self.text("工具", "Tools")),
            ("model", self.text("模型", "Models")),
        ] {
            let element = match id {
                "auto" => "group-auto",
                "tool" => "group-tool",
                _ => "group-model",
            };
            tools = tools.child(
                Button::new(element)
                    .ghost()
                    .label(label)
                    .when(group == id, |b| b.bg(cx.theme().muted))
                    .on_click(cx.listener(move |view, _, _, cx| view.set_group(id, cx))),
            );
        }
        tools = tools
            .child(
                Button::new("wall")
                    .ghost()
                    .label(self.text("展墙", "Wall"))
                    .when(self.preferences.view == "wall" && !self.focused, |b| {
                        b.bg(cx.theme().muted)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("wall", cx))),
            )
            .child(
                Button::new("roam")
                    .ghost()
                    .label(self.text("漫游", "Roam"))
                    .when(self.preferences.view == "roam" && !self.focused, |b| {
                        b.bg(cx.theme().muted)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("roam", cx))),
            )
            .child(
                Button::new("open-export-dock")
                    .ghost()
                    .label(self.text("导出", "Export"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.export_open = !view.export_open;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .label(self.text("设置", "Settings"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Settings, window, cx)
                    })),
            );
        if !self.export_queue.jobs().is_empty() {
            tools = tools.child(
                Button::new("export-history")
                    .ghost()
                    .label(format!(
                        "{} {}",
                        self.text("任务", "Jobs"),
                        self.export_queue.jobs().len()
                    ))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Exports, window, cx)
                    })),
            );
        }
        let toolbar = div()
            .h(px(60.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_5()
            .child(img(self.logo_image.clone()).w(px(24.)).h(px(24.)))
            .child(Button::new("switch-project").ghost().label(title).on_click(
                cx.listener(|view, _, window, cx| view.open_panel(Panel::Projects, window, cx)),
            ))
            .child(
                Button::new("project-settings")
                    .ghost()
                    .label("⋯")
                    .disabled(
                        self.selected.is_none() || self.selected.as_deref() == Some("__all__"),
                    )
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Project, window, cx)
                    })),
            )
            .child(div().flex_1())
            .child(tools);
        let mut controls =
            div()
                .id("view-controls")
                .h(px(56.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1()
                .px_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .p_1()
                        .rounded_full()
                        .bg(glass)
                        .border_1()
                        .border_color(border)
                        .child(
                            Button::new("style-previous")
                                .ghost()
                                .label("‹")
                                .on_click(cx.listener(|view, _, _, cx| view.step_style(-1, cx))),
                        )
                        .child(Button::new("styles").ghost().label(style_name).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.open_panel(Panel::Styles, window, cx)
                            }),
                        ))
                        .child(
                            Button::new("style-next")
                                .ghost()
                                .label("›")
                                .on_click(cx.listener(|view, _, _, cx| view.step_style(1, cx))),
                        ),
                )
                .child(
                    Button::new("pieces")
                        .ghost()
                        .label(self.text("作品", "Artworks"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Pieces, window, cx)
                        })),
                )
                .child(div().flex_1());
        if !self.focused {
            controls = controls
                .child(
                    Button::new("analyze")
                        .ghost()
                        .label(self.text("重新分析", "Analyze"))
                        .disabled(
                            self.job_busy
                                || self.automatic_job.is_some()
                                || self.current().is_none_or(|p| !p.exists),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.start_job(None, cx))),
                )
                .child(
                    Button::new("copy")
                        .ghost()
                        .label(self.text("复制档案", "Copy archive"))
                        .disabled(self.raw.is_none())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.copy_archive(&CopyArchive, window, cx)
                        })),
                )
                .child(
                    Button::new("details")
                        .ghost()
                        .label(self.text("证据", "Evidence"))
                        .disabled(self.raw.is_none())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Details, window, cx)
                        })),
                );
        }
        if self.focused {
            controls = controls
                .child(
                    Button::new("layouts")
                        .ghost()
                        .label(self.text("版式", "Layout"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Layouts, window, cx)
                        })),
                )
                .child(
                    Button::new("previous-piece")
                        .ghost()
                        .label("←")
                        .on_click(cx.listener(|view, _, _, cx| view.move_piece(-1, cx))),
                )
                .child(
                    Button::new("next-piece")
                        .ghost()
                        .label("→")
                        .on_click(cx.listener(|view, _, _, cx| view.move_piece(1, cx))),
                )
                .child(
                    Button::new("play")
                        .ghost()
                        .label(if self.playback.as_ref().is_some_and(|p| p.playing) {
                            self.text("暂停", "Pause")
                        } else {
                            self.text("播放", "Play")
                        })
                        .disabled(
                            self.animation_busy
                                || self.selected_piece().is_none_or(|p| p["animated"] != true),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.play(false, cx))),
                )
                .child(
                    Button::new("replay")
                        .ghost()
                        .label("↻")
                        .disabled(self.animation_busy)
                        .on_click(cx.listener(|view, _, _, cx| view.play(true, cx))),
                )
                .child(
                    Button::new("focus-close")
                        .ghost()
                        .label("×")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.close_focus(cx);
                        })),
                );
        }
        if self.focused || self.preferences.view == "roam" {
            controls = controls
                .child(
                    Button::new("zoom-out")
                        .ghost()
                        .label("−")
                        .on_click(cx.listener(|view, _, _, cx| view.zoom(1. / 1.2, cx))),
                )
                .child(
                    Button::new("zoom-reset")
                        .ghost()
                        .label(format!("{}%", (self.viewport.zoom * 100.).round() as i32))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.viewport.reset();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("zoom-in")
                        .ghost()
                        .label("+")
                        .on_click(cx.listener(|view, _, _, cx| view.zoom(1.2, cx))),
                );
        }
        let mut status = div()
            .h(px(24.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_5()
            .text_xs()
            .text_color(muted);
        if let Some(stage) = self.export_stage {
            let label = match stage {
                ExportStage::ChoosePath => self.text("选择保存位置…", "Choose a save location…"),
                ExportStage::Rendering => self.text("正在导出…", "Exporting…"),
                ExportStage::Writing => self.text("正在保存…", "Saving…"),
            };
            status = status
                .child(format!("{} · {}", label, self.export_label))
                .child(
                    Button::new("cancel-export")
                        .ghost()
                        .label(self.text("取消", "Cancel"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.cancel_export(&CancelExport, window, cx)
                        })),
                );
        } else if self.job_busy || self.automatic_job.is_some() {
            status = status
                .child(
                    self.progress
                        .as_ref()
                        .map(|p| progress_text(p, &self.preferences.language))
                        .unwrap_or_else(|| self.text("正在分析…", "Analyzing…").into()),
                )
                .child(
                    Button::new("cancel-analysis")
                        .ghost()
                        .label(self.text("取消", "Cancel"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.cancel(&CancelAnalysis, window, cx)
                        })),
                );
        } else if matches!(
            self.pending_analysis["state"].as_str(),
            Some("interrupted" | "failed" | "cancelled")
        ) {
            status = status
                .child(self.text("上次任务尚未完成", "The previous task did not finish"))
                .child(
                    Button::new("resume-analysis")
                        .ghost()
                        .label(self.text("继续分析", "Resume analysis"))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.start_job_mode(None, true, cx)),
                        ),
                );
        } else if let Some(error) = &self.error {
            status = status.child(error.clone()).child(
                Button::new("retry")
                    .ghost()
                    .label(self.text("重试", "Retry"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.error = None;
                        view.refresh(cx);
                    })),
            );
        } else if self.animation_busy {
            status = status.child(self.text("正在准备动画…", "Preparing animation…"));
        } else if let Some(notice) = &self.notice {
            status = status.child(notice.clone());
        } else {
            let loaded = self.tiles.iter().filter(|t| t.art.is_some()).count();
            status = status
                .child(format!(
                    "{} / {} {}",
                    loaded,
                    self.tiles.len(),
                    self.text("件作品", "artworks")
                ))
                .child(div().flex_1())
                .child(self.text(
                    "双击放大 · 拖动调整顺序",
                    "Double-click to open · Drag to reorder",
                ));
        }
        if !self.warnings.is_empty() {
            status = status.child(
                Button::new("analysis-warnings")
                    .ghost()
                    .label(format!(
                        "{} ({})",
                        self.text("分析提示", "Analysis notes"),
                        self.warnings.len()
                    ))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Details, window, cx)
                    })),
            );
        }
        let stage = self.render_stage(cx);
        let export = self.render_export_dock(cx);
        let modal = self.render_panel(cx);
        div()
            .relative()
            .size_full()
            .flex()
            .bg(bg)
            .text_color(fg)
            .track_focus(&self.focus)
            .key_context("Creditia")
            .on_action(cx.listener(|_, _: &MinimizeWindow, window, _| window.minimize_window()))
            .on_action(cx.listener(|_, _: &FullScreen, window, _| window.toggle_fullscreen()))
            .on_action(cx.listener(Self::open_project))
            .on_action(cx.listener(|view, _: &Refresh, _, cx| view.refresh(cx)))
            .on_action(cx.listener(Self::analyze))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::copy_archive))
            .on_action(cx.listener(Self::reveal_project))
            .on_action(cx.listener(Self::toggle_language))
            .on_action(cx.listener(Self::cycle_appearance))
            .on_action(cx.listener(Self::toggle_motion))
            .on_action(cx.listener(|view, _: &ExportArtwork, _, cx| {
                view.export_open = true;
                cx.notify();
            }))
            .on_action(cx.listener(Self::cancel_export))
            .on_action(cx.listener(Self::cancel_current))
            .on_action(cx.listener(|view, _: &SwitchProject, window, cx| {
                view.open_panel(Panel::Projects, window, cx)
            }))
            .on_action(cx.listener(|view, _: &Settings, window, cx| {
                view.open_panel(Panel::Settings, window, cx)
            }))
            .on_action(cx.listener(|view, _: &ProjectSettings, window, cx| {
                view.open_panel(Panel::Project, window, cx)
            }))
            .on_action(cx.listener(|view, _: &Wall, _, cx| view.set_view("wall", cx)))
            .on_action(cx.listener(|view, _: &Roam, _, cx| view.set_view("roam", cx)))
            .on_action(cx.listener(|view, _: &PreviousStyle, _, cx| view.step_style(-1, cx)))
            .on_action(cx.listener(|view, _: &NextStyle, _, cx| view.step_style(1, cx)))
            .on_action(cx.listener(|view, _: &ZoomIn, _, cx| view.zoom(1.2, cx)))
            .on_action(cx.listener(|view, _: &ZoomOut, _, cx| view.zoom(1. / 1.2, cx)))
            .on_action(cx.listener(|view, _: &ResetZoom, _, cx| {
                view.viewport.reset();
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &Play, _, cx| view.play(false, cx)))
            .on_action(cx.listener(|view, _: &Replay, _, cx| view.play(true, cx)))
            .on_action(cx.listener(|view, _: &PreviousPiece, _, cx| view.move_piece(-1, cx)))
            .on_action(cx.listener(|view, _: &NextPiece, _, cx| view.move_piece(1, cx)))
            .on_mouse_move(cx.listener(|view, event, _, cx| view.pointer_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, event, _, cx| view.pointer_up(event, cx)),
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(toolbar)
                    .child(div().flex_1().min_h_0().child(stage))
                    .child(export)
                    .child(controls)
                    .child(status),
            )
            .children(modal)
    }
}
fn apply_appearance(appearance: &Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}
fn install_menus(language: &Language, cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Creditia".into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("设置…", "Settings…"), Settings),
                MenuItem::action(language.text("切换语言", "Switch language"), ToggleLanguage),
                MenuItem::action(
                    language.text("切换明暗主题", "Cycle appearance"),
                    CycleAppearance,
                ),
                MenuItem::action(
                    language.text("减少动态效果", "Reduced motion"),
                    ToggleMotion,
                ),
                MenuItem::separator(),
                MenuItem::action(language.text("退出 Creditia", "Quit Creditia"), Quit),
            ],
        },
        Menu {
            name: language.text("项目", "Project").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("切换项目…", "Switch project…"), SwitchProject),
                MenuItem::action(language.text("添加文件夹…", "Add folder…"), OpenProject),
                MenuItem::action(language.text("重新分析", "Re-analyze"), Analyze),
                MenuItem::action(language.text("取消分析", "Cancel analysis"), CancelAnalysis),
                MenuItem::action(
                    language.text("项目设置…", "Project settings…"),
                    ProjectSettings,
                ),
                MenuItem::separator(),
                MenuItem::action(language.text("复制档案", "Copy archive"), CopyArchive),
                MenuItem::action(
                    language.text("打开所在位置", "Reveal folder"),
                    RevealProject,
                ),
            ],
        },
        Menu {
            name: language.text("编辑", "Edit").into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    language.text("撤销", "Undo"),
                    gpui_kit::component::input::Undo,
                ),
                MenuItem::action(
                    language.text("重做", "Redo"),
                    gpui_kit::component::input::Redo,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    language.text("剪切", "Cut"),
                    gpui_kit::component::input::Cut,
                ),
                MenuItem::action(
                    language.text("复制", "Copy"),
                    gpui_kit::component::input::Copy,
                ),
                MenuItem::action(
                    language.text("粘贴", "Paste"),
                    gpui_kit::component::input::Paste,
                ),
                MenuItem::action(
                    language.text("全选", "Select all"),
                    gpui_kit::component::input::SelectAll,
                ),
            ],
        },
        Menu {
            name: language.text("视图", "View").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("展墙", "Wall"), Wall),
                MenuItem::action(language.text("漫游", "Roam"), Roam),
                MenuItem::action(language.text("上一种风格", "Previous style"), PreviousStyle),
                MenuItem::action(language.text("下一种风格", "Next style"), NextStyle),
                MenuItem::separator(),
                MenuItem::action(language.text("放大", "Zoom in"), ZoomIn),
                MenuItem::action(language.text("缩小", "Zoom out"), ZoomOut),
                MenuItem::action(language.text("重置缩放", "Reset zoom"), ResetZoom),
                MenuItem::separator(),
                MenuItem::action(language.text("播放／暂停", "Play / pause"), Play),
                MenuItem::action(language.text("重播", "Replay"), Replay),
            ],
        },
        Menu {
            name: language.text("窗口", "Window").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("最小化", "Minimize"), MinimizeWindow),
                MenuItem::action(language.text("切换全屏", "Toggle Full Screen"), FullScreen),
            ],
        },
        Menu {
            name: language.text("导出", "Export").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("导出作品…", "Export artwork…"), ExportArtwork),
                MenuItem::action(language.text("取消导出", "Cancel export"), CancelExport),
            ],
        },
    ]);
}
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(key_bindings(if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }));
}
fn key_bindings(modifier: &str) -> Vec<KeyBinding> {
    const SHORTCUTS: &[(&str, &str, &str)] = &[
        ("q", "cmd-q", "ctrl-q"),
        ("k", "cmd-k", "ctrl-k"),
        ("o", "cmd-o", "ctrl-o"),
        ("r", "cmd-r", "ctrl-r"),
        ("i", "cmd-i", "ctrl-i"),
        (",", "cmd-,", "ctrl-,"),
        ("1", "cmd-1", "ctrl-1"),
        ("2", "cmd-2", "ctrl-2"),
        ("[", "cmd-[", "ctrl-["),
        ("]", "cmd-]", "ctrl-]"),
        ("e", "cmd-e", "ctrl-e"),
        ("shift-r", "cmd-shift-r", "ctrl-shift-r"),
        ("shift-c", "cmd-shift-c", "ctrl-shift-c"),
        ("=", "cmd-=", "ctrl-="),
        ("-", "cmd--", "ctrl--"),
        ("0", "cmd-0", "ctrl-0"),
    ];
    let shortcut = |key: &str| -> &'static str {
        let row = SHORTCUTS
            .iter()
            .find(|row| row.0 == key)
            .expect("registered shortcut");
        if modifier == "cmd" { row.1 } else { row.2 }
    };
    vec![
        KeyBinding::new(shortcut("q"), Quit, None),
        KeyBinding::new(shortcut("k"), SwitchProject, Some("Creditia")),
        KeyBinding::new(shortcut("o"), OpenProject, Some("Creditia")),
        KeyBinding::new(shortcut("r"), Analyze, Some("Creditia")),
        KeyBinding::new(shortcut("i"), ProjectSettings, Some("Creditia")),
        KeyBinding::new(shortcut(","), Settings, Some("Creditia")),
        KeyBinding::new(shortcut("1"), Wall, Some("Creditia")),
        KeyBinding::new(shortcut("2"), Roam, Some("Creditia")),
        KeyBinding::new(shortcut("["), PreviousStyle, Some("Creditia")),
        KeyBinding::new(shortcut("]"), NextStyle, Some("Creditia")),
        KeyBinding::new(shortcut("e"), ExportArtwork, Some("Creditia")),
        KeyBinding::new(shortcut("shift-r"), Refresh, Some("Creditia")),
        KeyBinding::new(shortcut("shift-c"), CopyArchive, Some("Creditia")),
        KeyBinding::new("escape", CancelCurrent, Some("Creditia")),
        KeyBinding::new(shortcut("="), ZoomIn, Some("Creditia")),
        KeyBinding::new(shortcut("-"), ZoomOut, Some("Creditia")),
        KeyBinding::new(shortcut("0"), ResetZoom, Some("Creditia")),
        KeyBinding::new("space", Play, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("left", PreviousPiece, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("right", NextPiece, Some("Creditia && !CreditiaModal")),
    ]
}
pub fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|s| s == "--version") {
        println!("Creditia {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args
        .iter()
        .any(|s| s == "--check-resources" || s == "--self-test")
    {
        let result = (|| -> Result<Value, String> {
            let catalog = renderer::request(
                json!({"version":1,"id":"resources","op":"catalog"}),
                renderer::Cancellation::default(),
            )?;
            let core = service::core_bin();
            let status = std::process::Command::new(&core)
                .arg("--help")
                .output()
                .map_err(|e| e.to_string())?;
            if !status.status.success() {
                return Err("Analysis executable failed to launch".into());
            }
            Ok(json!({"ok":true,"styles":catalog["styles"].as_array().map(Vec::len),"core":core}))
        })();
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let fixture = args
        .iter()
        .position(|s| s == "--fixture" || s == "--acceptance-fixture")
        .and_then(|i| args.get(i + 1))
        .map(|path| {
            std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Fixture>(&bytes).map_err(|e| e.to_string())
                })
        });
    let fixture = match fixture {
        Some(Ok(fixture)) => Some(fixture),
        Some(Err(error)) => {
            eprintln!("Invalid fixture: {error}");
            std::process::exit(2);
        }
        None => None,
    };
    let benchmark = args
        .iter()
        .position(|s| s == "--benchmark-archive")
        .and_then(|i| args.get(i + 1))
        .map(|path| {
            std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())
                })
        });
    let benchmark = match benchmark {
        Some(Ok(raw)) => Some(raw),
        Some(Err(error)) => {
            eprintln!("Invalid benchmark archive: {error}");
            std::process::exit(2);
        }
        None => None,
    };
    let acceptance = benchmark.is_some() || args.iter().any(|s| s == "--acceptance-fixture");
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_action(|_: &Quit, cx| {
            service::cancel_analyze();
            cx.quit();
        });
        open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("creditia".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(
                        px(if acceptance { 1360. } else { 1380. }),
                        px(if acceptance { 880. } else { 900. }),
                    ),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(620.))),
                ..Default::default()
            },
            cx,
            move |window, cx| {
                cx.new(|cx| {
                    if let Some(raw) = benchmark {
                        Creditia::benchmark(raw, window, cx)
                    } else {
                        match fixture {
                            Some(fixture) => {
                                let mut view = Creditia::fixture(fixture, window, cx);
                                view.benchmark = acceptance;
                                view
                            }
                            None => Creditia::new(window, cx),
                        }
                    }
                })
            },
        )
        .expect("Unable to open Creditia");
        cx.activate(true);
    });
}

fn write_performance_event(mut event: Value) {
    use std::io::Write;
    let Ok(path) =
        std::env::var("CREDITIA_APP_EVENTS").or_else(|_| std::env::var("CREDITIA_PERF_EVENTS"))
    else {
        return;
    };
    event["timeMs"] = json!(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", event);
    }
}

fn logo_image(id: &str) -> Arc<Image> {
    let svg = match id {
        "star-mono" => include_str!("../../../packages/render/assets/creditia/logo-star-mono.svg"),
        "orb" => include_str!("../../../packages/render/assets/creditia/logo-orb.svg"),
        "ring" => include_str!("../../../packages/render/assets/creditia/logo-ring.svg"),
        "color" => include_str!("../../../packages/render/assets/creditia/logo-color.svg"),
        "mono" => include_str!("../../../packages/render/assets/creditia/logo.svg"),
        _ => include_str!("../../../packages/render/assets/creditia/logo-star.svg"),
    };
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()))
}

fn telemetry_enabled() -> bool {
    std::env::var_os("CREDITIA_APP_EVENTS").is_some()
        || std::env::var_os("CREDITIA_PERF_EVENTS").is_some()
}

fn analysis_fraction(progress: &Value) -> Option<f64> {
    let (start, end) = match progress["stage"].as_str()? {
        "history" => (0., 0.1),
        "logs" => (0.1, 0.3),
        "attribute" => (0.3, 0.85),
        "blame" => (0.85, 1.),
        _ => return None,
    };
    let total = progress["total"].as_f64().unwrap_or(0.);
    let fraction = if total > 0. {
        progress["done"].as_f64().unwrap_or(0.) / total
    } else {
        0.
    };
    Some(start + (end - start) * fraction.clamp(0., 1.))
}
#[cfg(test)]
mod input_tests {
    use super::*;
    #[test]
    fn all_shortcuts_parse_on_both_platform_families() {
        for modifier in ["cmd", "ctrl"] {
            assert_eq!(key_bindings(modifier).len(), 20);
        }
    }
}
