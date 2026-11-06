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
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, FocusTrapElement, Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
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
        PreviousLayout,
        NextLayout,
        FocusArtwork,
        MinimizeWindow,
        FullScreen,
        CloseWindow,
        HideApp,
        HideOthers,
        ShowAll
    ]
);
#[derive(Clone, Copy)]
enum ExportStage {
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
struct PendingImage {
    image: Arc<Image>,
    region: Option<Rect>,
    pixels: u32,
    bytes: usize,
    size: (u32, u32),
}
#[derive(Clone)]
struct Tile {
    identity: String,
    spec: TileSpec,
    art: Option<Arc<Artwork>>,
    image: Option<Arc<Image>>,
    pending_image: Option<PendingImage>,
    image_region: Option<Rect>,
    pixels: u32,
    texture_bytes: usize,
    image_size: (u32, u32),
    error: Option<String>,
    loading: bool,
}
struct PlaybackFrame {
    epoch: u64,
    ready: bool,
    art: Arc<Artwork>,
    tiles: Vec<(Option<Rect>, Arc<Image>)>,
    pixels: u32,
    decoded_bytes: usize,
    regions: Vec<Value>,
}
impl PlaybackFrame {
    fn release(self, cx: &mut App) {
        for (_, image) in self.tiles {
            crate::image_assets::release(image, cx);
        }
    }
}
struct Playback {
    frames: BTreeMap<usize, PlaybackFrame>,
    previous: Option<PlaybackFrame>,
    times: Vec<f64>,
    timeline: Timeline,
    started: Instant,
    elapsed: Duration,
    playing: bool,
    suspended: bool,
    index: usize,
    requested: usize,
    epoch: u64,
    started_once: bool,
}
impl Playback {
    // A slow raster must finish before a repeat invalidates its epoch. Let
    // normal playback follow wall time, but bound outstanding work by the
    // producer's look-ahead. Its next decoded frame wakes a stalled clock.
    fn tick(&mut self, control: &crate::animation::PlaybackControl) -> bool {
        let last = self.times.len() - 1;
        let now = Instant::now();
        let duration = self.timeline.duration_ms;
        let end = duration.saturating_add(self.timeline.hold_ms);
        let limit = if self.index == last {
            end
        } else {
            let ahead = (self.index + crate::animation::MAX_AHEAD).min(last);
            (self.times[ahead] * duration as f64).ceil() as u64
        };
        let real = now.duration_since(self.started);
        self.elapsed = real.min(Duration::from_millis(limit));
        self.started = now - self.elapsed;
        if self.index == last && self.elapsed.as_millis() >= u128::from(end) {
            if self.timeline.repeat {
                self.elapsed = Duration::ZERO;
                self.started_once = false;
                self.requested = 0;
                control.restart();
            } else {
                self.playing = false;
                control.pause();
            }
            return false;
        }
        let time = self.elapsed.as_millis() as u64;
        self.requested = if time >= duration { last } else { frame_at(&self.times, &self.timeline, time) };
        control.set_playhead(self.requested);
        if let Some((&index, _)) = self.frames.range(..=self.requested).rev().find(|(_, frame)| frame.ready) {
            self.index = index;
        }
        time < limit || self.index == self.requested
    }
    fn displayed(&self) -> Option<&PlaybackFrame> {
        self.frames
            .get(&self.index)
            .filter(|frame| frame.ready)
            .or(self.previous.as_ref())
    }
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
#[derive(Clone)]
struct ViewBookmark {
    viewport: Viewport,
    selected_key: Option<String>,
    selected_spec: Option<TileSpec>,
    focused: bool,
    saved_viewport: Option<Viewport>,
}
struct FocusStyles {
    original: String,
    current: String,
    bookmark: ViewBookmark,
    variants: BTreeMap<String, BTreeMap<String, String>>,
}
struct FocusTransition {
    from: Rect,
    started: Instant,
}
struct DragState {
    tile: Option<usize>,
    clicked_tile: Option<usize>,
    source: Option<Rect>,
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
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub progress: Value,
    #[serde(default)]
    pub projects: Vec<Value>,
    #[serde(default)]
    pub person: String,
    #[serde(default)]
    pub blame: Option<Value>,
}

pub struct Creditia {
    live_mark: crate::live_mark::LiveMark,
    center_mark: crate::live_mark::LiveMark,
    project_marks: BTreeMap<String, crate::live_mark::ProjectMark>,
    mark_cache_keys: BTreeMap<String, String>,
    row_mark_task: Option<Task<()>>,
    row_mark_worker: Option<Task<()>>,
    row_mark_cancellation: Option<renderer::Cancellation>,
    row_mark_generation: u64,
    mark_tooltip: String,
    search_query: String,
    mark_source: Option<(usize, String, bool, String)>,
    mark_task: Option<Task<()>>,
    mark_cancellation: Option<renderer::Cancellation>,
    mark_generation: u64,
    has_models: Option<bool>,
    resolved_group: Option<String>,
    platform_ui: crate::platform_ui::PlatformUi,
    _system_motion_task: Option<Task<()>>,
    reduced_motion: bool,
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
    focus_styles: Option<FocusStyles>,
    focus_placeholder: Option<Tile>,
    saved_viewport: Option<Viewport>,
    pending_view: Option<ViewBookmark>,
    mode_viewports: BTreeMap<String, Viewport>,
    preferences: Preferences,
    preference_writer: Option<Arc<crate::preference_writer::PreferenceWriter>>,
    migration_error: Option<String>,
    migration_busy: bool,
    migration_task: Option<Task<()>>,
    // These handles retain background notifications and input subscriptions.
    _preference_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
    logo_image: Arc<Image>,
    projects: Vec<Project>,
    picker_selected: Option<String>,
    project_scroll: ScrollHandle,
    project_row_menu: Option<(String, (f32, f32))>,
    settings_project: Option<String>,
    settings_details: Option<Value>,
    discovered: Option<Vec<Value>>,
    discovery_task: Option<Task<()>>,
    discover_busy: bool,
    discovery_error: Option<String>,
    batch_results: BTreeMap<String, Value>,
    batch_order: Vec<String>,
    active_add: Option<String>,
    batch_opened: bool,
    discovery_selected: Vec<String>,
    add_queue: VecDeque<String>,
    selected: Option<String>,
    raw: Option<Arc<Value>>,
    render_archive: Option<renderer::RenderArchive>,
    loading: bool,
    activity_since: Option<Instant>,
    activity_task: Option<Task<()>>,
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
    roam_layout: Option<crate::roam::RoamLayout>,
    roam_dimensions: BTreeMap<String, (f32, f32)>,
    viewport: Viewport,
    viewport_size: (f32, f32),
    display_scale: f32,
    raster_geometry: (f32, f32, f32, f32),
    raster_changed_at: Instant,
    roam_motion: crate::ui_motion::RoamMotion,
    motion_clock: Instant,
    window_visible: bool,
    selected_tile: Option<usize>,
    focused: bool,
    hover: Option<Hit>,
    hovered_tile: Option<usize>,
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
    profile_person: Option<String>,
    all_archives: Arc<Vec<Value>>,
    export_queue: crate::export_jobs::Queue,
    export_history_error: Option<String>,
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
    format_bounds: ScrollHandle,
    size_bounds: ScrollHandle,
    export_hovered: Option<usize>,
    fixture: bool,
    fixture_artworks: BTreeMap<(String, String, String), Option<Arc<Artwork>>>,
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
        let system_motion_task = if fixture {
            None
        } else {
            crate::platform_ui::watch_system_motion(cx)
        };
        let reduced_motion = preferences.reduced_motion
            || (!fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
        cx.set_reduce_motion(reduced_motion);
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window, cx);
        let sign_input = cx.new(|cx| InputState::new(window, cx).placeholder("署名 / Signature"));
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索项目 / Search projects"));
        let blame_file = cx.new(|cx| InputState::new(window, cx).placeholder("src/main.rs"));
        let blame_line = cx.new(|cx| InputState::new(window, cx).default_value("1"));
        let subscriptions = vec![
            cx.on_release(|view, cx| view.release_images(cx)),
            cx.subscribe(&search_input, |view, input, event, cx| match event {
                InputEvent::Change => {
                    view.picker_selected = None;
                    view.project_row_menu = None;
                    view.search_query = input.read(cx).value().to_string();
                    view.fill_project_marks(cx);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } if view.panel == Some(Panel::Projects) => {
                    view.activate_picker(cx)
                }
                _ => {}
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                view.window_activity(window, cx)
            }),
            cx.observe_window_visibility(window, |view, visibility, window, cx| {
                view.window_visible = visibility.is_visible();
                view.window_activity(window, cx);
            }),
        ];
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
            (Some(Arc::new(writer)), Some(task))
        };
        let logo_image = logo_image(&preferences.logo);
        Self {
            live_mark: crate::live_mark::LiveMark::new(),
            center_mark: crate::live_mark::LiveMark::new(),
            project_marks: BTreeMap::new(),
            mark_cache_keys: BTreeMap::new(),
            row_mark_task: None,
            row_mark_worker: None,
            row_mark_cancellation: None,
            row_mark_generation: 0,
            mark_tooltip: String::new(),
            search_query: String::new(),
            mark_source: None,
            mark_task: None,
            mark_cancellation: None,
            mark_generation: 0,
            has_models: None,
            resolved_group: None,
            platform_ui: crate::platform_ui::PlatformUi::new(),
            _system_motion_task: system_motion_task,
            reduced_motion,
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
            focus_styles: None,
            focus_placeholder: None,
            saved_viewport: None,
            pending_view: None,
            mode_viewports: BTreeMap::new(),
            preference_writer,
            migration_error: None,
            migration_busy: false,
            migration_task: None,
            _preference_task: preference_task,
            _subscriptions: subscriptions,
            logo_image,
            selected: preferences.last_project.clone(),
            export_format: preferences.export_format.clone(),
            export_size: preferences.export_size.clone(),
            preferences,
            projects: vec![],
            picker_selected: None,
            project_scroll: ScrollHandle::new(),
            project_row_menu: None,
            settings_project: None,
            settings_details: None,
            discovered: None,
            discovery_task: None,
            discover_busy: false,
            discovery_error: None,
            batch_results: BTreeMap::new(),
            batch_order: vec![],
            active_add: None,
            batch_opened: false,
            discovery_selected: vec![],
            add_queue: VecDeque::new(),
            raw: None,
            render_archive: None,
            loading: false,
            activity_since: None,
            activity_task: None,
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
            roam_layout: None,
            roam_dimensions: BTreeMap::new(),
            viewport: Viewport::default(),
            viewport_size: (1000., 700.),
            display_scale: 1.,
            raster_geometry: (0., 0., 0., 0.),
            raster_changed_at: Instant::now(),
            roam_motion: crate::ui_motion::RoamMotion::default(),
            motion_clock: Instant::now(),
            window_visible: true,
            selected_tile: None,
            focused: false,
            hover: None,
            hovered_tile: None,
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
            profile_person: None,
            all_archives: Arc::new(vec![]),
            export_queue: crate::export_jobs::Queue::default(),
            export_history_error: None,
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
            format_bounds: ScrollHandle::new(),
            size_bounds: ScrollHandle::new(),
            export_hovered: None,
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
                        view.migration_error = Some(error.to_string());
                        view.notice = Some(format!(
                            "{}: {}",
                            view.text("旧设置尚未导入", "Previous settings have not been imported"),
                            error
                        ))
                    }
                }
                view.reduced_motion = view.preferences.reduced_motion
                    || (!view.fixture
                        && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
                cx.set_reduce_motion(view.reduced_motion);
                match view.preferences.appearance {
                    Appearance::System => Theme::sync_system_appearance(None, cx),
                    Appearance::Light => Theme::change(ThemeMode::Light, None, cx),
                    Appearance::Dark => Theme::change(ThemeMode::Dark, None, cx),
                }
                Theme::update(cx, |theme| theme.font_family = "Inter".into());
                view.load_export_history(jobs, cx);
                crate::image_assets::release(view.logo_image.clone(), cx);
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
            last_analyzed: None,
        });
        view.all_archives = Arc::new(fixture.projects);
        view.profile_person = (!fixture.person.is_empty()).then_some(fixture.person);
        view.blame = fixture.blame;
        view.raw = Some(Arc::new(fixture.raw));
        view.catalog = Some(fixture.catalog);
        view.resolve_style();
        view.rebuild_tiles(cx);
        for value in fixture.artworks {
            let style = value["style"]
                .as_str()
                .unwrap_or(&view.preferences.style)
                .to_string();
            let piece = value["piece"].as_str().unwrap_or("").to_string();
            let variant = value["variant"].as_str().unwrap_or("").to_string();
            let art = if value["art"].is_null() {
                None
            } else {
                serde_json::from_value::<Artwork>(value["art"].clone())
                    .ok()
                    .map(Arc::new)
            };
            view.fixture_artworks.insert((style, piece, variant), art);
        }
        view.restore_fixture_artworks();
        match fixture.state.as_str() {
            "empty" => {
                view.raw = None;
                view.projects.clear();
                view.selected = None;
                view.clear_preview(cx);
            }
            "loading" => view.loading = true,
            "error" => {
                view.error = Some(if fixture.message.is_empty() {
                    view.text("无法读取项目档案", "Unable to read project archive")
                        .into()
                } else {
                    fixture.message
                })
            }
            "analyzing" => {
                view.job_busy = true;
                view.progress = Some(if fixture.progress.is_null() {
                    json!({"stage":"logs","done":2,"total":5})
                } else {
                    fixture.progress
                });
            }
            "exporting" => {
                view.export_stage = Some(ExportStage::Rendering);
                view.export_label = "creditia-minimal-light-tier-native.png".into();
            }
            _ => {}
        }
        view
    }
    fn restore_fixture_artworks(&mut self) {
        if !self.fixture {
            return;
        }
        let style = self
            .style()
            .and_then(|style| style["id"].as_str())
            .unwrap_or(&self.preferences.style)
            .to_string();
        for tile in &mut self.tiles {
            if tile.image.is_some() || tile.error.is_some() {
                continue;
            }
            if let Some(art) = self.fixture_artworks.get(&(
                style.clone(),
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
                    self.roam_dimensions.insert(
                        crate::roam::canonical_key(&tile.spec.piece, tile.spec.variant.as_deref()),
                        (art.w, art.h),
                    );
                } else {
                    tile.error = Some("__empty__".into());
                    if let Some(roam) = &mut self.roam_layout {
                        roam.mark_empty(&crate::roam::canonical_key(
                            &tile.spec.piece,
                            tile.spec.variant.as_deref(),
                        ));
                    }
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
            last_analyzed: None,
        });
        view.raw = Some(Arc::new(raw));
        view.load_catalog(cx);
        view
    }
    fn release_images(&mut self, cx: &mut App) {
        self.live_mark.release(cx);
        self.center_mark.release(cx);
        for mark in self.project_marks.values() {
            crate::image_assets::release(mark.image.clone(), cx);
        }
        if let Some(cancel) = &self.mark_cancellation {
            cancel.cancel();
        }
        if let Some(cancel) = &self.row_mark_cancellation {
            cancel.cancel();
        }
        crate::image_assets::release(self.logo_image.clone(), cx);
        if let Some(control) = &self.animation_control {
            control.cancel();
        }
        if let Some(cancel) = &self.preview_cancellation {
            cancel.cancel();
        }
        if let Some(cancel) = &self.analysis_cancellation {
            cancel.cancel();
        }
        if let Some(cancel) = &self.export_cancellation {
            cancel.cancel();
        }
        if let Some(background) = &self.background {
            background.stop();
        }
        if let Some(tile) = self.focus_placeholder.take() {
            if let Some(image) = tile.image {
                crate::image_assets::release(image, cx);
            }
        }
        for tile in &mut self.tiles {
            if let Some(pending) = tile.pending_image.take() {
                crate::image_assets::release(pending.image, cx);
            }
            if let Some(image) = tile.image.take() {
                crate::image_assets::release(image, cx);
            }
        }
        if let Some(play) = self.playback.take() {
            if let Some(frame) = play.previous {
                frame.release(cx);
            }
            for (_, frame) in play.frames {
                frame.release(cx);
            }
        }
    }
    fn update_project_mark(&mut self, cx: &mut Context<Self>) {
        if self.fixture && !self.benchmark {
            return;
        }
        let Some(raw) = self.raw.clone() else {
            if self.mark_source.take().is_some() {
                self.mark_generation = self.mark_generation.wrapping_add(1);
                if let Some(cancel) = self.mark_cancellation.take() {
                    cancel.cancel();
                }
                let mark = self
                    .selected
                    .as_ref()
                    .and_then(|id| self.project_marks.get(id))
                    .cloned();
                self.live_mark.set_project(mark.clone(), cx);
                self.center_mark.set_project(mark, cx);
            }
            return;
        };
        let group = self.group().to_string();
        let lang = self.preferences.language.code().to_string();
        let named = self.preferences.named;
        let source = (
            Arc::as_ptr(&raw) as usize,
            group.clone(),
            named,
            lang.clone(),
        );
        if self.mark_source.as_ref() == Some(&source) {
            return;
        }
        self.mark_source = Some(source);
        self.mark_generation = self.mark_generation.wrapping_add(1);
        let generation = self.mark_generation;
        if let Some(cancel) = self.mark_cancellation.take() {
            cancel.cancel();
        }
        let cancellation = renderer::Cancellation::default();
        self.mark_cancellation = Some(cancellation.clone());
        let id = self.selected.clone();
        self.mark_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let request = json!({
                        "version":1, "id":"project-mark", "op":"mark", "raw":raw.as_ref(),
                        "group":group, "lang":lang, "named":named
                    });
                    renderer::request_with_priority(request, cancellation, 0)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.mark_generation != generation {
                    return;
                }
                view.mark_cancellation = None;
                if let Ok(value) = &result {
                    view.has_models = value["hasModels"].as_bool();
                    view.resolved_group = value["resolvedGroup"].as_str().map(str::to_owned);
                    let mut lines = vec![value["project"].as_str().unwrap_or("").to_string()];
                    lines.extend(value["rows"].as_array().into_iter().flatten().map(|row| {
                        format!(
                            "{}  {:.1}%",
                            row["name"].as_str().unwrap_or(""),
                            row["pct"].as_f64().unwrap_or(0.)
                        )
                    }));
                    view.mark_tooltip = lines.join("\n");
                }
                match result.and_then(|value| crate::live_mark::ProjectMark::from_response(&value))
                {
                    Ok(mark) => {
                        if let Some(id) = id {
                            if let Some(old) = view.project_marks.insert(id, mark.clone()) {
                                crate::image_assets::release(old.image, cx);
                            }
                            let protected = view.nearby_project_marks(cx);
                            view.trim_project_marks(&protected, cx);
                        }
                        view.live_mark.set_project(Some(mark.clone()), cx);
                        view.center_mark.set_project(Some(mark), cx);
                    }
                    Err(error) => view.notice = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn nearby_project_marks(&self, cx: &App) -> Vec<String> {
        let top = self.project_scroll.top_item().saturating_sub(12);
        let mut ids = self
            .picker_items(cx)
            .into_iter()
            .skip(top)
            .take(64)
            .filter(|id| self.projects.iter().any(|project| &project.id == id))
            .collect::<Vec<_>>();
        if let Some(id) = &self.selected {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        ids
    }
    fn trim_project_marks(&mut self, protected: &[String], cx: &mut App) {
        while self.project_marks.len() > 128 {
            let id = self
                .project_marks
                .keys()
                .find(|id| self.selected.as_ref() != Some(id) && !protected.contains(id))
                .cloned();
            let Some(id) = id else { break };
            if let Some(mark) = self.project_marks.remove(&id) {
                crate::image_assets::release(mark.image, cx);
            }
            self.mark_cache_keys.remove(&id);
        }
        // Failed/unreadable rows have no image. Retain their attempted key only
        // for this neighborhood so scrolling does not create an idle retry loop.
        self.mark_cache_keys
            .retain(|id, _| self.project_marks.contains_key(id) || protected.contains(id));
    }
    fn fill_project_marks(&mut self, cx: &mut Context<Self>) {
        if self.fixture
            || self.panel != Some(Panel::Projects)
            || self.row_mark_cancellation.is_some()
        {
            return;
        }
        let wanted = self.nearby_project_marks(cx);
        self.trim_project_marks(&wanted, cx);
        let mut keys = BTreeMap::new();
        let requests = self
            .projects
            .iter()
            .filter(|project| wanted.contains(&project.id))
            .filter_map(|project| {
                let group = self
                    .preferences
                    .projects
                    .get(&project.id)
                    .map(|prefs| prefs.group.clone())
                    .unwrap_or_else(|| "auto".into());
                let lang = self.preferences.language.code().to_string();
                let named = self.preferences.named;
                let key = format!("{:?}/{group}/{lang}/{named}", project.last_analyzed);
                if self.mark_cache_keys.get(&project.id) == Some(&key) {
                    return None;
                }
                keys.insert(project.id.clone(), key);
                Some(crate::project_marks::RowRequest {
                    id: project.id.clone(),
                    group,
                    named,
                    lang,
                })
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return;
        }
        self.row_mark_generation = self.row_mark_generation.wrapping_add(1);
        let generation = self.row_mark_generation;
        let cancellation = renderer::Cancellation::default();
        self.row_mark_cancellation = Some(cancellation.clone());
        let (tx, rx) = async_channel::bounded(4);
        self.row_mark_worker = Some(cx.background_spawn(async move {
            crate::project_marks::fill(requests, cancellation, |id, result| {
                tx.send_blocking((id, result)).is_ok()
            });
        }));
        self.row_mark_task = Some(cx.spawn(async move |this, cx| {
            while let Ok((id, result)) = rx.recv().await {
                let key = keys.get(&id).cloned();
                if this
                    .update(cx, |view, cx| {
                        if generation != view.row_mark_generation
                            || !view.projects.iter().any(|p| p.id == id)
                        {
                            return;
                        }
                        if let Some(key) = key {
                            view.mark_cache_keys.insert(id.clone(), key);
                        }
                        if let Ok(mark) = result
                            .and_then(|value| crate::live_mark::ProjectMark::from_response(&value))
                        {
                            if let Some(old) = view.project_marks.insert(id, mark) {
                                crate::image_assets::release(old.image, cx);
                            }
                            let protected = view.nearby_project_marks(cx);
                            view.trim_project_marks(&protected, cx);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
            let _ = this.update(cx, |view, cx| {
                if generation == view.row_mark_generation {
                    view.row_mark_cancellation = None;
                    // A search/scroll during the batch may have changed the
                    // visible neighborhood; continue only for those new rows.
                    view.fill_project_marks(cx);
                }
            });
        }));
    }
    fn window_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.fixture {
            crate::platform_ui::refresh_system_motion(cx);
        }
        let active = window.is_window_active() && self.window_visible;
        if let Some(background) = &self.background {
            background.set_active(active);
        }
        if !active {
            self.roam_motion.stop();
            self.hover = None;
            self.hovered_tile = None;
            self.drag = None;
            if let Some(control) = &self.animation_control {
                control.set_visible(false);
                control.pause();
            }
            if let Some(play) = &mut self.playback {
                if play.playing && !play.suspended {
                    play.suspended = true;
                }
            }
        }
        if telemetry_enabled() {
            let mut state = self.diagnostic_state();
            state["active"] = json!(active);
            state["visible"] = json!(self.window_visible);
            state["viewport_ready"] = json!(false);
            if state["animation"].is_object() {
                state["animation"]["visible"] = json!(active && self.panel.is_none());
            }
            write_performance_event(
                json!({"event":"visibility","visible":self.window_visible,"active":active,"state":state}),
            );
            self.telemetry_state = None;
        }
        cx.notify();
    }
    fn activity_age(&self) -> Duration {
        if self.fixture && !self.benchmark {
            Duration::from_secs(3)
        } else {
            self.activity_since
                .map(|since| since.elapsed())
                .unwrap_or_default()
        }
    }
    fn sync_activity(&mut self, cx: &mut Context<Self>) {
        let busy = self.loading || self.job_busy || self.automatic_job.is_some();
        if busy && self.activity_since.is_none() {
            self.activity_since = Some(Instant::now());
            let executor = cx.background_executor().clone();
            self.activity_task = Some(cx.spawn(async move |this, cx| {
                executor.timer(Duration::from_millis(300)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
                executor.timer(Duration::from_millis(1700)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
        } else if !busy {
            self.activity_since = None;
            self.activity_task = None;
        }
    }
    fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.preferences.language.text(zh, en)
    }
    fn retry_migration(&mut self, cx: &mut Context<Self>) {
        if self.fixture || self.migration_busy {
            return;
        }
        self.migration_busy = true;
        let writer = self.preference_writer.clone();
        self.migration_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if let Some(writer) = writer {
                        writer.flush().map_err(|error| error.to_string())?;
                    }
                    service::migrate_legacy_with_helper().map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.migration_busy = false;
                match result {
                    Ok(value) => {
                        view.preferences = Preferences::read(&value);
                        view.migration_error = None;
                        view.notice = Some(
                            view.text("已导入原有桌面设置", "Previous desktop settings imported")
                                .into(),
                        );
                        view.reduced_motion = view.preferences.reduced_motion
                            || (!view.fixture
                                && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
                        cx.set_reduce_motion(view.reduced_motion);
                        match view.preferences.appearance {
                            Appearance::System => Theme::sync_system_appearance(None, cx),
                            Appearance::Light => Theme::change(ThemeMode::Light, None, cx),
                            Appearance::Dark => Theme::change(ThemeMode::Dark, None, cx),
                        }
                        Theme::update(cx, |theme| theme.font_family = "Inter".into());
                        install_menus(&view.preferences.language, cx);
                        crate::image_assets::release(view.logo_image.clone(), cx);
                        view.logo_image = logo_image(&view.preferences.logo);
                        view.export_format = view.preferences.export_format.clone();
                        view.export_size = view.preferences.export_size.clone();
                        view.selected = view.preferences.last_project.clone();
                        view.refresh(cx);
                    }
                    Err(error) => {
                        view.migration_error = Some(error.clone());
                        view.error = Some(error);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn persist(&mut self) {
        if self.migration_busy {
            return;
        }
        if let Some(writer) = &self.preference_writer {
            writer.submit(self.preferences.shared_patch());
        }
    }
    fn project_preferences(&self) -> crate::ProjectPreferences {
        let mut preferences = self.preferences.project(self.selected.as_deref());
        if let Some(styles) = &self.focus_styles {
            if styles.current != styles.original {
                preferences.variants = styles
                    .variants
                    .get(&styles.current)
                    .cloned()
                    .unwrap_or_default();
                preferences.order.clear();
            }
        }
        preferences
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
        let style = self
            .focus_styles
            .as_ref()
            .map(|styles| styles.current.as_str())
            .unwrap_or(&self.preferences.style);
        self.style_index = self
            .catalog
            .as_ref()
            .and_then(|c| c["styles"].as_array())
            .and_then(|s| s.iter().position(|entry| entry["id"] == style))
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
            if let Some(frame) = play.previous {
                frame.release(cx);
            }
            for (_, frame) in play.frames {
                frame.release(cx);
            }
        }
    }
    fn drop_focus_placeholder(&mut self, cx: &mut App) {
        if let Some(tile) = self.focus_placeholder.take() {
            if let Some(image) = tile.image {
                if !self.tiles.iter().any(|tile| {
                    tile.image
                        .as_ref()
                        .is_some_and(|kept| Arc::ptr_eq(kept, &image))
                }) {
                    crate::image_assets::release(image, cx);
                }
            }
        }
    }
    fn retain_focus_preview(&mut self, cx: &mut Context<Self>) {
        if !self.focused {
            return;
        }
        if let Some(tile) = self
            .current_tile()
            .filter(|tile| tile.art.is_some() && tile.image.is_some())
            .cloned()
        {
            self.drop_focus_placeholder(cx);
            let mut tile = tile;
            tile.pending_image = None;
            self.focus_placeholder = Some(tile);
        }
    }
    fn prepare_static_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Keep the previous bitmap until GPUI has decoded its replacement.
        if !self.window_visible {
            return;
        }
        let mut retired = Vec::new();
        let mut decoding = false;
        for tile in &mut self.tiles {
            if let Some(pending) = &tile.pending_image {
                if crate::image_assets::track(&pending.image, window, cx).is_some() {
                    let pending = tile.pending_image.take().unwrap();
                    if let Some(old) = tile.image.replace(pending.image) {
                        retired.push(old);
                    }
                    tile.image_region = pending.region;
                    tile.pixels = pending.pixels;
                    tile.texture_bytes = pending.bytes;
                    tile.image_size = pending.size;
                    tile.loading = false;
                } else {
                    decoding = true;
                }
            }
        }
        for image in retired {
            let retained = self.tiles.iter().any(|tile| {
                tile.image
                    .as_ref()
                    .is_some_and(|kept| Arc::ptr_eq(kept, &image))
            }) || self
                .focus_placeholder
                .as_ref()
                .and_then(|tile| tile.image.as_ref())
                .is_some_and(|kept| Arc::ptr_eq(kept, &image));
            if !retained {
                crate::image_assets::release(image, cx);
            }
        }
        if decoding && window.is_window_active() {
            window.request_animation_frame();
        }
        // Track precomputed fixture images and shared roam occurrences too.
        let visible = self.artwork_viewport();
        for (index, tile) in self.tiles.iter().enumerate() {
            if (!self.focused || self.selected_tile == Some(index))
                && self.screen_rect(index).intersects(visible)
            {
                if let Some(image) = &tile.image {
                    crate::image_assets::track(image, window, cx);
                }
            }
        }
        if let Some(image) = self
            .focus_placeholder
            .as_ref()
            .and_then(|tile| tile.image.as_ref())
        {
            crate::image_assets::track(image, window, cx);
        }
        self.evict_distant(cx);
    }
    fn prepare_focus_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_placeholder.is_none() || !self.focused {
            return;
        }
        if self.current_tile().is_none_or(|tile| tile.error.is_some()) {
            self.drop_focus_placeholder(cx);
            return;
        }
        if let Some(image) = self.current_tile().and_then(|tile| tile.image.clone()) {
            if crate::image_assets::track(&image, window, cx).is_some() {
                self.drop_focus_placeholder(cx);
            } else if self.window_visible && window.is_window_active() {
                window.request_animation_frame();
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
            if let Some(pending) = tile.pending_image.take() {
                crate::image_assets::release(pending.image, cx);
            }
            if let Some(image) = tile.image.take() {
                if !self
                    .focus_placeholder
                    .as_ref()
                    .and_then(|tile| tile.image.as_ref())
                    .is_some_and(|kept| Arc::ptr_eq(kept, &image))
                {
                    crate::image_assets::release(image, cx);
                }
            }
        }
        self.tiles.clear();
        self.roam_layout = None;
        self.roam_dimensions.clear();
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
                    identity: spec.key.clone(),
                    spec,
                    art: None,
                    image: None,
                    pending_image: None,
                    image_region: None,
                    pixels: 0,
                    texture_bytes: 0,
                    image_size: (0, 0),
                    error: None,
                    loading: false,
                })
                .collect();
        }
        self.selected_tile = (!self.tiles.is_empty()).then_some(0);
        self.viewport = if self.preferences.view == "roam" {
            crate::roam::home(self.viewport_size)
        } else {
            Viewport::default()
        };
        self.focused = false;
        self.restore_fixture_artworks();
        cx.notify();
    }
    fn bookmark(&self) -> ViewBookmark {
        ViewBookmark {
            viewport: self.viewport,
            selected_key: self.current_tile().map(|tile| tile.identity.clone()),
            selected_spec: self.current_tile().map(|tile| tile.spec.clone()),
            focused: self.focused,
            saved_viewport: self.saved_viewport,
        }
    }
    fn restore_bookmark(&mut self, bookmark: ViewBookmark) {
        let mut selected = bookmark
            .selected_key
            .as_ref()
            .and_then(|key| self.tiles.iter().position(|tile| &tile.identity == key));
        // Focused roam slots and context-dependent artworks are absent from the
        // rebuilt wall. Retain their real selection while replacing its data.
        if selected.is_none() && bookmark.focused {
            if let Some(mut spec) = bookmark.selected_spec {
                if let Some(variant) = self.project_preferences().variants.get(&spec.key) {
                    spec.variant = Some(variant.clone());
                }
                self.tiles.push(Tile {
                    identity: bookmark.selected_key.unwrap_or_else(|| spec.key.clone()),
                    spec,
                    art: None,
                    image: None,
                    pending_image: None,
                    image_region: None,
                    pixels: 0,
                    texture_bytes: 0,
                    image_size: (0, 0),
                    error: None,
                    loading: false,
                });
                selected = Some(self.tiles.len() - 1);
            }
        }
        self.viewport = bookmark.viewport;
        self.focused = bookmark.focused && selected.is_some();
        if selected.is_some() {
            self.selected_tile = selected;
        }
        self.saved_viewport = bookmark.saved_viewport;
        self.restore_fixture_artworks();
    }
    fn rebuild_preserving_view(&mut self, cx: &mut Context<Self>) {
        self.retain_focus_preview(cx);
        let bookmark = self.bookmark();
        self.rebuild_tiles(cx);
        self.restore_bookmark(bookmark);
    }
    fn restore_pending_view(&mut self) {
        if let Some(bookmark) = self.pending_view.take() {
            self.restore_bookmark(bookmark);
        }
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
                            if view.projects.is_empty() {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                                view.preferences.last_project = None;
                                view.persist();
                                if let Some(background) = &view.background {
                                    background.watch_project(None);
                                }
                                view.discovered = None;
                                view.discover_projects(cx);
                                cx.notify();
                                return;
                            }
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
                                view.discover_projects(cx);
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
        let same_project = self.selected.as_ref() == Some(&id);
        if !same_project {
            self.focus_styles = None;
            self.drop_focus_placeholder(cx);
        }
        let refresh_projects = same_project && !self.all_archives.is_empty();
        let refresh_blame = same_project
            .then(|| {
                self.blame.as_ref().and_then(|blame| {
                    Some((blame["file"].as_str()?.to_string(), blame["line"].as_u64()?))
                })
            })
            .flatten();
        self.pending_view = if self.selected.as_ref() == Some(&id) && self.raw.is_some() {
            Some(self.bookmark())
        } else {
            None
        };
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.all_archives = Arc::new(vec![]);
        self.blame = None;
        self.focused = false;
        self.details = None;
        // Changing the watch cancels its worker asynchronously. Keep its identity
        // until completion so a queued project does not race the analysis lock.
        if !self.job_busy && !same_project {
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
            let (result, details, projects, blame) = cx
                .background_spawn(async move {
                    let projects = refresh_projects.then(service::project_archives);
                    let blame =
                        refresh_blame.map(|(file, line)| service::blame(id.clone(), file, line));
                    (
                        service::credits(id.clone()),
                        service::project_details(id),
                        projects,
                        blame,
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                // A slower previous project must never replace the current archive.
                if view.archive_generation != generation {
                    return;
                }
                view.loading = false;
                if let Some(projects) = projects {
                    match projects {
                        Ok(projects) => {
                            view.all_archives = Arc::new(
                                projects
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|project| project.get("raw").cloned())
                                    .collect(),
                            )
                        }
                        Err(error) => {
                            view.notice = Some(crate::ui_reports::failure(
                                &error,
                                &view.preferences.language,
                            ))
                        }
                    }
                }
                if let Some(blame) = blame {
                    match blame {
                        Ok(blame) => view.blame = Some(blame),
                        Err(error) => {
                            view.notice = Some(crate::ui_reports::failure(
                                &error,
                                &view.preferences.language,
                            ))
                        }
                    }
                }
                match details {
                    Ok(details) => view.details = Some(details),
                    Err(error) => {
                        view.notice = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                match result {
                    Ok(raw) => {
                        view.raw = Some(Arc::new(raw));
                        view.load_catalog(cx);
                    }
                    Err(error) if error.code == "no_credits" => {}
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn queue_projects(&mut self, paths: impl IntoIterator<Item = String>, cx: &mut Context<Self>) {
        if self.active_add.is_none() && self.add_queue.is_empty() {
            self.batch_opened = false;
        }
        for path in paths {
            if self.active_add.as_ref() == Some(&path) || self.add_queue.contains(&path) {
                continue;
            }
            if !self.batch_order.contains(&path) {
                self.batch_order.push(path.clone());
            }
            self.batch_results
                .insert(path.clone(), json!({"state":"queued"}));
            self.add_queue.push_back(path);
        }
        self.start_next_project(cx);
        cx.notify();
    }
    fn retry_discovery(&mut self, cx: &mut Context<Self>) {
        self.discovered = None;
        self.discovery_error = None;
        self.discover_projects(cx);
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
        self.discovery_error = None;
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
                    Err(error) => {
                        view.discovery_error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
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
                        view.queue_projects(
                            paths
                                .into_iter()
                                .map(|path| path.to_string_lossy().into_owned()),
                            cx,
                        );
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
        self.start_job_target(add_path, resume, None, cx);
    }
    fn start_job_target(
        &mut self,
        add_path: Option<String>,
        resume: bool,
        target: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let selected = target.or_else(|| self.selected.clone());
        if self.job_busy
            || self.automatic_job.is_some()
            || (!resume
                && add_path.is_none()
                && selected
                    .as_ref()
                    .and_then(|id| self.projects.iter().find(|project| &project.id == id))
                    .is_none_or(|project| !project.exists))
        {
            return;
        }
        self.job_busy = true;
        self.cancelling = false;
        self.progress = None;
        self.error = None;
        self.notice = None;
        if add_path.is_none() {
            self.warnings.clear();
        }
        self.active_add = add_path.clone();
        if let Some(path) = &add_path {
            self.batch_results
                .insert(path.clone(), json!({"state":"running"}));
        }
        let selection_at_start = self.selected.clone();
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
                                        let warnings = crate::ui_reports::analysis_warnings(&value);
                                        if adding {
                                            view.warnings.extend(warnings.clone());
                                            if let Some(path) = view.active_add.take() {
                                                view.batch_results.insert(
                                                    path.clone(),
                                                    json!({"state":"done","warnings":warnings}),
                                                );
                                                if let Some(found) = &mut view.discovered {
                                                    found.retain(|project| {
                                                        project["path"].as_str() != Some(&path)
                                                    });
                                                }
                                            }
                                        } else {
                                            view.warnings = warnings;
                                        }
                                        if adding && !view.batch_opened {
                                            if view.selected==selection_at_start {view.selected=value["id"].as_str().map(str::to_string);}
                                            view.batch_opened=true;
                                        }
                                        view.notice =
                                            Some(view.text("分析完成", "Analysis complete").into());
                                        view.refresh(cx);
                                    }
                                    Err(error) if view.cancelling || error.code == "cancelled" => {
                                        if let Some(path) = view.active_add.take() {
                                            view.batch_results
                                                .insert(path, json!({"state":"cancelled"}));
                                        }
                                        view.notice = Some(
                                            view.text("已取消分析", "Analysis cancelled").into(),
                                        );
                                    }
                                    Err(error) => {
                                        if let Some(path) = view.active_add.take() {
                                            view.batch_results.insert(
                                                path,
                                                json!({"state":"failed","error":crate::ui_reports::failure(&error,&view.preferences.language)}),
                                            );
                                        }
                                        view.error = Some(crate::ui_reports::failure(&error,&view.preferences.language));
                                    }
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
        if !self.job_busy {
            if let (Some((id, generation)), Some(background)) =
                (&self.automatic_job, &self.background)
            {
                self.cancelling = background.cancel_analysis(id, *generation);
                cx.notify();
            }
        }
        if self.job_busy {
            for path in self.add_queue.drain(..) {
                self.batch_results
                    .insert(path, json!({"state":"cancelled"}));
            }
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
        let project = if self.panel == Some(Panel::Project) {
            self.settings_current()
        } else {
            self.current()
        };
        if let Some(project) = project.filter(|project| project.exists) {
            cx.reveal_path(Path::new(&project.path));
        }
    }

    fn load_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog.is_some() {
            self.resolve_style();
            self.rebuild_tiles(cx);
            self.restore_pending_view();
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
                        view.restore_pending_view();
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
        if let Some(person) = &self.profile_person {
            request["person"] = json!(person);
        }
        Some(request)
    }
    fn sync_roam(&mut self, cx: &mut Context<Self>) {
        if self.roam_layout.is_none() {
            let prefs = self.project_preferences();
            if let Some(style) = self.style() {
                match crate::roam::RoamLayout::new(style, &prefs.variants) {
                    Ok(layout) => self.roam_layout = Some(layout),
                    Err(error) => {
                        self.error = Some(error);
                        return;
                    }
                }
            } else {
                return;
            }
        }
        let specs = self.roam_layout.as_mut().unwrap().layout(
            &mut self.viewport,
            self.viewport_size,
            &self.roam_dimensions,
        );
        let selected = self.current_tile().map(|tile| tile.identity.clone());
        let mut old = std::mem::take(&mut self.tiles)
            .into_iter()
            .map(|tile| (tile.identity.clone(), tile))
            .collect::<BTreeMap<_, _>>();
        let canonical = old
            .values()
            .filter(|tile| tile.art.is_some())
            .map(|tile| {
                (
                    crate::roam::canonical_key(&tile.spec.piece, tile.spec.variant.as_deref()),
                    tile.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        self.rects.clear();
        for slot in specs {
            self.rects.push(slot.rect);
            let mut tile = old.remove(&slot.slot_key).unwrap_or_else(|| {
                let mut tile = canonical.get(&slot.spec.key).cloned().unwrap_or(Tile {
                    identity: slot.slot_key.clone(),
                    spec: slot.spec.clone(),
                    art: None,
                    image: None,
                    pending_image: None,
                    image_region: None,
                    pixels: 0,
                    texture_bytes: 0,
                    image_size: (0, 0),
                    error: None,
                    loading: false,
                });
                tile.loading = false;
                tile.pending_image = None;
                tile
            });
            tile.identity = slot.slot_key;
            tile.spec = slot.spec;
            self.tiles.push(tile);
        }
        let mut retained = self
            .tiles
            .iter()
            .filter_map(|tile| tile.image.as_ref().map(|image| Arc::as_ptr(image) as usize))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(image) = self
            .focus_placeholder
            .as_ref()
            .and_then(|tile| tile.image.as_ref())
        {
            retained.insert(Arc::as_ptr(image) as usize);
        }
        for (_, tile) in old {
            if let Some(pending) = tile.pending_image {
                crate::image_assets::release(pending.image, cx);
            }
            if tile.loading {
                if let Some(cancel) = &self.preview_cancellation {
                    cancel.cancel();
                }
            }
            if let Some(image) = tile.image {
                if !retained.contains(&(Arc::as_ptr(&image) as usize)) {
                    crate::image_assets::release(image, cx);
                }
            }
        }
        self.selected_tile = selected
            .and_then(|identity| self.tiles.iter().position(|tile| tile.identity == identity))
            .or_else(|| (!self.tiles.is_empty()).then_some(0));
        self.restore_fixture_artworks();
    }
    fn compute_layout(&mut self, width: f32, height: f32, cx: &mut Context<Self>) {
        self.viewport_size = (width, height);
        if self.preferences.view == "roam" && !self.focused {
            self.sync_roam(cx);
            return;
        }
        if self.preferences.view == "roam" {
            return;
        }
        let specs = self
            .tiles
            .iter()
            .map(|tile| tile.spec.clone())
            .collect::<Vec<_>>();
        let (rects, total) = masonry(&specs, &self.dimensions, width);
        self.rects = rects;
        self.wall_height = total;
        if !self.focused {
            self.viewport.x = 0.;
            self.viewport.zoom = 1.;
            self.viewport.y = self.viewport.y.clamp(-(total - height).max(0.), 0.);
        }
    }
    fn screen_rect(&self, index: usize) -> Rect {
        let target = if self.focused && self.selected_tile == Some(index) {
            let art = self
                .focus_placeholder
                .as_ref()
                .and_then(|tile| tile.art.as_deref())
                .or_else(|| {
                    self.playback
                        .as_ref()
                        .and_then(Playback::displayed)
                        .map(|f| f.art.as_ref())
                        .or_else(|| self.tiles.get(index).and_then(|t| t.art.as_deref()))
                });
            let (w, h) = art.map(|a| (a.w, a.h)).unwrap_or((640., 480.));
            focused_rect(w, h, self.viewport, self.viewport_size)
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
        let dpi = self.display_scale.max(0.1);
        let physical_width = texture_width(target.w, dpi);
        Rect {
            x: (target.x * dpi).round() / dpi,
            y: (target.y * dpi).round() / dpi,
            w: physical_width as f32 / dpi,
            h: texture_width(target.h / target.w.max(0.001) * physical_width as f32, 1.) as f32
                / dpi,
        }
    }
    fn artwork_viewport(&self) -> Rect {
        Rect {
            x: 0.,
            y: 0.,
            w: self.viewport_size.0,
            h: if self.focused {
                (self.viewport_size.1 - 154.).max(120.)
            } else {
                self.viewport_size.1
            },
        }
    }
    fn crop_for_art(&self, art: &Artwork, r: Rect, pixels: u32, margin: f32) -> Option<Rect> {
        let height = pixels as f32 * art.h / art.w;
        if pixels as f32 * height < 16_000_000. && height < 16384. && pixels < 16384 {
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
            || !self.window_visible
            || self.preview_busy
            || self.raw.is_none()
            || self.catalog.is_none()
            || (self.focused
                && self
                    .playback
                    .as_ref()
                    .is_some_and(|play| play.displayed().is_some()))
        {
            return;
        }
        // Overlap the single raster worker with one small image decode. A
        // large pending image or a second pending image applies backpressure.
        let mut pending = self
            .tiles
            .iter()
            .filter_map(|tile| tile.pending_image.as_ref());
        if let Some(first) = pending.next() {
            if first.bytes > 8 * 1024 * 1024 || pending.next().is_some() {
                return;
            }
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
                let settled = self.raster_changed_at.elapsed() >= Duration::from_millis(120)
                    && self.focus_transition.is_none();
                (tile.image.is_none() || needs_crop || (settled && tile.pixels != pixels))
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
        let key = self.tiles[index].identity.clone();
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
                        None => renderer::RenderArchive::with_projects(raw, projects)?,
                    };
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
                    let region = if pixels as f32 * height >= 16_000_000.
                        || height >= 16384.
                        || pixels >= 16384
                    {
                        let scale = screen.w / art.w;
                        let physical = pixels as f32 / art.w;
                        let x = ((((-screen.x - 180.) / scale).clamp(0., art.w) * physical)
                            .floor()
                            / physical)
                            .clamp(0., art.w);
                        let y = ((((-screen.y - 240.) / scale).clamp(0., art.h) * physical)
                            .floor()
                            / physical)
                            .clamp(0., art.h);
                        let right = ((((viewport.0 - screen.x + 180.) / scale).clamp(x, art.w)
                            * physical)
                            .ceil()
                            / physical)
                            .clamp(x, art.w);
                        let bottom = ((((viewport.1 - screen.y + 240.) / scale).clamp(y, art.h)
                            * physical)
                            .ceil()
                            / physical)
                            .clamp(y, art.h);
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
                            texture_width(pixels as f32 * r.w / art.w, 1.),
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
                if let Some(tile) = view.tiles.iter_mut().find(|t| t.identity == key) {
                    tile.loading = false;
                    match result {
                        Ok((art, raster, region, archive)) => {
                            view.render_archive = Some(archive);
                            tile.pending_image = Some(PendingImage {
                                image: Arc::new(Image::from_bytes(
                                    ImageFormat::Png,
                                    raster.png.clone(),
                                )),
                                region,
                                pixels,
                                bytes: raster.width as usize * raster.height as usize * 4,
                                size: (raster.width, raster.height),
                            });
                            tile.loading = true;
                            view.dimensions
                                .insert(tile.spec.key.clone(), (art.w, art.h));
                            view.roam_dimensions.insert(
                                crate::roam::canonical_key(
                                    &tile.spec.piece,
                                    tile.spec.variant.as_deref(),
                                ),
                                (art.w, art.h),
                            );
                            tile.art = Some(art);
                        }
                        Err(error) => {
                            if error == "__empty__" {
                                if let Some(roam) = &mut view.roam_layout {
                                    roam.mark_empty(&crate::roam::canonical_key(
                                        &tile.spec.piece,
                                        tile.spec.variant.as_deref(),
                                    ));
                                }
                            }
                            tile.error = Some(error);
                        }
                    }
                }
                view.evict_distant(cx);
                cx.notify();
            });
        }));
    }
    fn evict_distant(&mut self, cx: &mut Context<Self>) {
        const HIGH_WATER: usize = 128 * 1024 * 1024;
        // Shared roam textures may be counted twice here, making this a safe
        // upper bound. Build the reference map only under memory pressure.
        if self
            .tiles
            .iter()
            .map(|tile| tile.texture_bytes)
            .sum::<usize>()
            < HIGH_WATER
        {
            return;
        }
        let mut textures = BTreeMap::<usize, (usize, usize)>::new();
        for tile in &self.tiles {
            if let Some(image) = &tile.image {
                let entry = textures
                    .entry(Arc::as_ptr(image) as usize)
                    .or_insert((tile.texture_bytes, 0));
                entry.1 += 1;
            }
        }
        let mut bytes = textures.values().map(|entry| entry.0).sum::<usize>();
        if bytes < HIGH_WATER {
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
                if let Some((size, references)) = textures.get_mut(&(Arc::as_ptr(&image) as usize))
                {
                    *references -= 1;
                    if *references == 0 {
                        bytes = bytes.saturating_sub(*size);
                        if !self
                            .focus_placeholder
                            .as_ref()
                            .and_then(|tile| tile.image.as_ref())
                            .is_some_and(|kept| Arc::ptr_eq(kept, &image))
                        {
                            crate::image_assets::release(image, cx);
                        }
                    }
                }
                tile.texture_bytes = 0;
                tile.pixels = 0;
                tile.image_region = None;
                tile.art = None;
            }
        }
    }
    fn choose_style(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(style) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog["styles"].as_array())
            .and_then(|styles| styles.get(index))
        else {
            return;
        };
        let id = style["id"].as_str().unwrap_or("").to_string();
        if self.focused {
            let Some(piece) = self.current_tile().map(|tile| tile.spec.piece.clone()) else {
                return;
            };
            if !style["pieces"]
                .as_array()
                .is_some_and(|pieces| pieces.iter().any(|entry| entry["id"] == piece))
            {
                return;
            }
            if self.focus_styles.is_none() {
                let mut bookmark = self.bookmark();
                bookmark.focused = false;
                bookmark.viewport = self.saved_viewport.unwrap_or_default();
                bookmark.saved_viewport = None;
                let original = self
                    .style()
                    .and_then(|style| style["id"].as_str())
                    .unwrap_or(&self.preferences.style)
                    .to_string();
                self.focus_styles = Some(FocusStyles {
                    original: original.clone(),
                    current: original,
                    bookmark,
                    variants: BTreeMap::new(),
                });
            }
            self.focus_styles.as_mut().unwrap().current = id;
            let saved = self.focus_styles.as_ref().unwrap().bookmark.viewport;
            self.retain_focus_preview(cx);
            self.style_index = index;
            self.panel = None;
            self.rebuild_tiles(cx);
            self.select_piece(piece, cx);
            self.saved_viewport = Some(saved);
            self.focus_transition = None;
        } else {
            self.focus_styles = None;
            self.preferences.style = id;
            self.style_index = index;
            self.persist();
            self.panel = None;
            self.rebuild_tiles(cx);
        }
    }
    fn restore_global_style(&mut self, cx: &mut Context<Self>) -> bool {
        self.drop_focus_placeholder(cx);
        let Some(styles) = self.focus_styles.take() else {
            return false;
        };
        self.resolve_style();
        self.rebuild_tiles(cx);
        self.restore_bookmark(styles.bookmark);
        self.focused = false;
        self.focus_transition = None;
        true
    }
    fn apply_focus_style(&mut self, cx: &mut Context<Self>) {
        self.drop_focus_placeholder(cx);
        let Some(styles) = self.focus_styles.take() else {
            return;
        };
        let mut bookmark = self.bookmark();
        bookmark.focused = false;
        bookmark.viewport = styles.bookmark.viewport;
        bookmark.saved_viewport = None;
        self.preferences.style = styles.current.clone();
        if let Some(id) = self.selected.clone() {
            let preferences = self.preferences.projects.entry(id).or_default();
            preferences.variants = styles
                .variants
                .get(&styles.current)
                .cloned()
                .unwrap_or_default();
            preferences.order.clear();
        }
        self.persist();
        self.rebuild_tiles(cx);
        self.restore_bookmark(bookmark);
        self.focused = false;
        self.focus_transition = None;
        cx.notify();
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
            self.rebuild_preserving_view(cx);
        }
    }
    fn set_names(&mut self, cx: &mut Context<Self>) {
        self.preferences.named = !self.preferences.named;
        self.persist();
        self.rebuild_preserving_view(cx);
    }
    fn set_view(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.roam_motion.stop();
        self.restore_global_style(cx);
        let previous = if self.focused {
            self.saved_viewport.unwrap_or_default()
        } else {
            self.viewport
        };
        self.mode_viewports
            .insert(self.preferences.view.clone(), previous);
        let changed = self.preferences.view != mode;
        self.preferences.view = mode.into();
        if changed && mode == "wall" {
            self.rebuild_tiles(cx);
        } else if changed {
            self.roam_layout = None;
        }
        self.focused = false;
        self.viewport = self.mode_viewports.get(mode).copied().unwrap_or_else(|| {
            if mode == "roam" {
                crate::roam::home(self.viewport_size)
            } else {
                Viewport::default()
            }
        });
        self.saved_viewport = None;
        self.focus_transition = None;
        self.stop_playback(cx);
        self.persist();
        cx.notify();
    }
    fn select_tile(&mut self, index: usize, focus: bool, cx: &mut Context<Self>) {
        self.roam_motion.stop();
        self.drag = None;
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
        self.focus_transition = if entering && !self.reduced_motion {
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
        if self.restore_global_style(cx) {
            cx.notify();
            return;
        }
        let source = self.selected_tile.map(|i| self.screen_rect(i));
        self.focused = false;
        self.viewport = self.saved_viewport.take().unwrap_or_default();
        self.stop_playback(cx);
        self.focus_transition = if self.reduced_motion {
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
                                if let Some(cancel) = view.preview_cancellation.take() {
                                    cancel.cancel();
                                }
                                view.preview_task = None;
                                view.preview_busy = false;
                                view.preview_generation = view.preview_generation.wrapping_add(1);
                                for tile in &mut view.tiles {
                                    tile.loading = false;
                                }
                                view.render_archive = None;
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
                identity: format!("{}#{}", piece, variant.as_deref().unwrap_or("")),
                spec: TileSpec {
                    key: format!("{}#{}", piece, variant.as_deref().unwrap_or("")),
                    piece,
                    variant,
                    span: 1,
                },
                art: None,
                image: None,
                pending_image: None,
                image_region: None,
                pixels: 0,
                texture_bytes: 0,
                image_size: (0, 0),
                error: None,
                loading: false,
            });
            self.restore_fixture_artworks();
            self.select_tile(self.tiles.len() - 1, true, cx);
        }
    }
    fn step_variant(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.focused {
            self.move_piece(delta, cx);
            return;
        }
        let Some(tile) = self.current_tile() else {
            return;
        };
        let Some(variants) = self
            .piece(&tile.spec.piece)
            .and_then(|piece| piece["variants"].as_array())
        else {
            return;
        };
        if variants.is_empty() {
            return;
        }
        let current = variants
            .iter()
            .position(|variant| variant["id"].as_str() == tile.spec.variant.as_deref())
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(variants.len() as isize) as usize;
        if let Some(id) = variants[next]["id"].as_str() {
            self.choose_variant(id.to_string(), cx);
        }
    }
    fn choose_variant(&mut self, variant: String, cx: &mut Context<Self>) {
        let Some(tile) = self.current_tile() else {
            return;
        };
        let key = tile.spec.key.clone();
        let piece = tile.spec.piece.clone();
        if self
            .focus_styles
            .as_ref()
            .is_some_and(|styles| styles.current != styles.original)
        {
            let specs = self
                .tiles
                .iter()
                .map(|tile| tile.spec.clone())
                .collect::<Vec<_>>();
            let styles = self.focus_styles.as_mut().unwrap();
            let variants = styles.variants.entry(styles.current.clone()).or_default();
            if swap_variant(&specs, variants, &key, &variant) {
                self.rebuild_preserving_view(cx);
            }
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        if self.preferences.view == "roam" {
            let base = self
                .style()
                .and_then(|style| style["board"].as_array())
                .and_then(|board| board.iter().find(|entry| entry["id"] == piece))
                .and_then(|entry| entry["variant"].as_str().map(str::to_owned))
                .or_else(|| self.piece(&piece).and_then(default_variant));
            let base_key = crate::roam::canonical_key(&piece, base.as_deref());
            self.preferences
                .projects
                .entry(id)
                .or_default()
                .variants
                .insert(base_key, variant.clone());
            let mut bookmark = self.bookmark();
            if let Some(spec) = &mut bookmark.selected_spec {
                spec.variant = Some(variant.clone());
                spec.key = crate::roam::canonical_key(&piece, Some(&variant));
            }
            self.persist();
            self.rebuild_tiles(cx);
            self.restore_bookmark(bookmark);
        } else {
            let specs = self
                .tiles
                .iter()
                .map(|tile| tile.spec.clone())
                .collect::<Vec<_>>();
            if swap_variant(
                &specs,
                &mut self.preferences.projects.entry(id).or_default().variants,
                &key,
                &variant,
            ) {
                self.persist();
                self.rebuild_preserving_view(cx);
            }
        }
    }
    fn move_piece(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.tiles.is_empty() {
            return;
        }
        let focused = self.focused;
        if focused && self.focus_styles.is_some() {
            self.restore_global_style(cx);
        }
        let current = self.selected_tile.unwrap_or(0) as isize;
        let next = current + delta;
        if !(0..self.tiles.len() as isize).contains(&next) {
            if focused {
                self.select_tile(current as usize, true, cx);
            }
            return;
        }
        self.select_tile(next as usize, focused, cx);
    }
    fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        if self.focused {
            let art = self
                .playback
                .as_ref()
                .and_then(Playback::displayed)
                .map(|frame| frame.art.as_ref())
                .or_else(|| self.current_tile().and_then(|tile| tile.art.as_deref()));
            if let Some(art) = art {
                let (w, h) = (art.w, art.h);
                let old = focused_rect(w, h, self.viewport, self.viewport_size);
                let zoom = (self.viewport.zoom * factor).clamp(0.15, 6.);
                let ratio = zoom / self.viewport.zoom;
                let base = focused_rect(w, h, Viewport { x: 0., y: 0., zoom }, self.viewport_size);
                self.viewport = Viewport {
                    x: x - (x - old.x) * ratio - base.x,
                    y: y - (y - old.y) * ratio - base.y,
                    zoom,
                };
                return;
            }
        }
        self.viewport.zoom_at(factor, x, y);
    }
    fn reset_viewport(&mut self, cx: &mut Context<Self>) {
        let target = if self.preferences.view == "roam" && !self.focused {
            crate::roam::home(self.viewport_size)
        } else {
            Viewport::default()
        };
        let smooth = self.preferences.view == "roam" && !self.focused && !self.reduced_motion;
        if !self.roam_motion.glide_to(
            self.motion_clock.elapsed(),
            (self.viewport.x, self.viewport.y),
            (target.x, target.y),
            smooth,
        ) {
            self.viewport = target;
        }
        cx.notify();
    }
    fn keyboard_navigation(&mut self, horizontal: bool, delta: isize, cx: &mut Context<Self>) {
        if self.preferences.view == "roam" && !self.focused {
            let mut target = (self.viewport.x, self.viewport.y);
            if horizontal {
                target.0 -= delta as f32
                    * ((self.viewport_size.0 / 3.1).clamp(380., 520.) + 56.)
                    * self.viewport.zoom;
            } else {
                target.1 -= delta as f32 * self.viewport_size.1 * 0.6;
            }
            if !self.roam_motion.glide_to(
                self.motion_clock.elapsed(),
                (self.viewport.x, self.viewport.y),
                target,
                !self.reduced_motion,
            ) {
                self.viewport.x = target.0;
                self.viewport.y = target.1;
            }
            self.hover = None;
            cx.notify();
        } else if horizontal {
            self.step_variant(delta, cx);
        } else {
            self.move_piece(delta, cx);
        }
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
        if self.selected.as_deref() == Some("__all__") && !self.fixture {
            self.show_all_projects(cx);
        } else {
            self.rebuild_preserving_view(cx);
        }
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
        self.reduced_motion = self.preferences.reduced_motion
            || (!self.fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
        cx.set_reduce_motion(self.reduced_motion);
        self.stop_playback(cx);
        if self.reduced_motion {
            self.focus_transition = None;
            self.panel_tween = None;
            self.dock_tween = None;
            if self.panel_closing {
                self.panel_closing = false;
                self.panel = None;
            }
        }
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
                play.started_once = false;
                play.playing = true;
                play.suspended = false;
                play.requested = 0;
                if let Some(control) = &self.animation_control {
                    control.restart();
                }
            } else if play.playing {
                // The decoder can trail the clock. Pause the frame on screen so
                // zooming, restoring or resuming cannot jump to an unseen frame.
                if frame_at(&play.times, &play.timeline, play.elapsed.as_millis() as u64)
                    != play.index
                {
                    play.elapsed = Duration::from_secs_f64(
                        play.times[play.index] * play.timeline.duration_ms as f64 / 1000.,
                    );
                }
                play.requested = play.index;
                play.playing = false;
                play.suspended = false;
                if let Some(control) = &self.animation_control {
                    control.pause();
                    control.set_playhead(play.index);
                }
            } else {
                play.started = Instant::now() - play.elapsed;
                play.playing = true;
                play.suspended = false;
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
        request["reducedMotion"] = json!(self.reduced_motion);
        if !self.focused {
            self.saved_viewport = Some(self.viewport);
        }
        self.focused = true;
        self.viewport.reset();
        self.animation_busy = true;
        let control = crate::animation::PlaybackControl::new();
        control.set_viewport(self.viewport, self.viewport_size, self.display_scale);
        self.animation_cancellation = Some(control.cancellation());
        self.animation_control = Some(control.clone());
        let width = texture_width(self.viewport_size.0, self.display_scale);
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
                    None => renderer::RenderArchive::with_projects(raw, projects)?,
                };
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
                                let playing = timeline.duration_ms > 0;
                                view.playback = Some(Playback {
                                    frames: BTreeMap::new(),
                                    previous: None,
                                    times,
                                    timeline,
                                    started: Instant::now(),
                                    elapsed: Duration::ZERO,
                                    playing,
                                    suspended: false,
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
                                tiles,
                                pixels,
                                decoded_bytes,
                                max_resident,
                            }) => {
                                if view
                                    .animation_control
                                    .as_ref()
                                    .is_none_or(|control| control.epoch() != epoch)
                                {
                                    return;
                                }
                                if let Some(play) = &mut view.playback {
                                    // Keep the previous geometry visible until this replacement is ready.
                                    if play.epoch != epoch {
                                        let current = play.frames.remove(&play.index);
                                        let previous = match current {
                                            Some(frame) if frame.ready => Some(frame),
                                            Some(frame) => {
                                                frame.release(cx);
                                                play.previous.take()
                                            }
                                            None => play.previous.take(),
                                        };
                                        if let Some(old) = play.previous.take() {
                                            old.release(cx);
                                        }
                                        play.previous = previous;
                                        for (_, frame) in std::mem::take(&mut play.frames) {
                                            frame.release(cx);
                                        }
                                        play.epoch = epoch;
                                    }
                                    let frame = PlaybackFrame {
                                        epoch,
                                        ready: false,
                                        art,
                                        pixels,
                                        decoded_bytes,
                                        regions: tiles.iter().map(|tile| json!({
                                            "width":tile.raster.width,"height":tile.raster.height,
                                            "region":tile.region.map(|r| json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h}))
                                        })).collect(),
                                        tiles: tiles
                                            .into_iter()
                                            .map(|tile| {
                                                (
                                                    tile.region,
                                                    Arc::new(Image::from_bytes(
                                                        ImageFormat::Png,
                                                        tile.raster.png.clone(),
                                                    )),
                                                )
                                            })
                                            .collect(),
                                    };
                                    if let Some(old) = play.frames.insert(index, frame) {
                                        old.release(cx);
                                    }
                                    let keep = play.requested;
                                    let shown = play.index;
                                    // The producer can finish behind the clock on high-DPI
                                    // displays. Keep its newest eligible frame for decoding.
                                    let latest = play.frames.range(..=keep).next_back().map(|(&i, _)| i);
                                    let obsolete = play
                                        .frames
                                        .keys()
                                        .copied()
                                        .filter(|i| {
                                            *i != shown
                                                && Some(*i) != latest
                                                && (*i < keep
                                                    || *i > keep + crate::animation::MAX_AHEAD)
                                        })
                                        .collect::<Vec<_>>();
                                    for index in obsolete {
                                        if let Some(frame) = play.frames.remove(&index) {
                                            frame.release(cx);
                                        }
                                    }
                                    let pending_bytes = play
                                        .frames
                                        .values()
                                        .map(|frame| frame.decoded_bytes as u64)
                                        .sum::<u64>();
                                    if pending_bytes
                                        + play
                                            .previous
                                            .as_ref()
                                            .map(|frame| frame.decoded_bytes as u64)
                                            .unwrap_or(0)
                                        > crate::animation::DECODED_RESIDENT_BUDGET
                                    {
                                        if let Some(previous) = play.previous.take() {
                                            previous.release(cx);
                                        }
                                    }
                                    while play.frames.len() > max_resident.max(1)
                                        || play.frames.len() + usize::from(play.previous.is_some()) > crate::animation::MAX_RESIDENT_FRAMES
                                        || play
                                            .frames
                                            .values()
                                            .map(|frame| frame.decoded_bytes as u64)
                                            .sum::<u64>()
                                            > crate::animation::DECODED_RESIDENT_BUDGET
                                    {
                                        let Some(candidate) = play
                                            .frames
                                            .keys()
                                            .copied()
                                            .filter(|i| *i != index)
                                            .max_by_key(|i| i.abs_diff(keep))
                                        else {
                                            break;
                                        };
                                        if let Some(frame) = play.frames.remove(&candidate) {
                                            frame.release(cx);
                                        }
                                    }
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
    fn prepare_playback_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(play) = &mut self.playback else {
            return;
        };
        if self
            .animation_control
            .as_ref()
            .is_some_and(|control| control.epoch() != play.epoch)
        {
            return;
        }
        let mut decoding = false;
        for frame in play.frames.values_mut() {
            if !frame.ready {
                let mut ready = true;
                for (_, image) in &frame.tiles {
                    ready &= crate::image_assets::track(image, window, cx).is_some();
                }
                frame.ready = ready;
                decoding |= !frame.ready;
            }
        }
        if decoding {
            window.request_animation_frame();
        }
        let desired = if play.started_once { play.requested } else { 0 };
        let next = play
            .frames
            .range(..=desired)
            .rev()
            .find(|(_, frame)| frame.ready)
            .map(|(&index, _)| index)
            .or_else(|| {
                (!play.started_once)
                    .then(|| {
                        play.frames
                            .iter()
                            .find(|(_, frame)| frame.ready)
                            .map(|(&index, _)| index)
                    })
                    .flatten()
            });
        if let Some(index) = next {
            play.index = index;
            if let Some(previous) = play.previous.take() {
                previous.release(cx);
            }
            if !play.started_once {
                play.started_once = true;
                play.started = Instant::now();
            }
            self.animation_busy = false;
        }
    }
    pub fn fixture_images_ready(&self, window: &mut Window, cx: &mut App) -> bool {
        if self.fixture && !self.benchmark && (self.raw.is_none() || self.loading) {
            return true;
        }
        if self
            .focus_transition
            .as_ref()
            .is_some_and(|transition| transition.started.elapsed() < Duration::from_millis(180))
        {
            return false;
        }
        if self.focused {
            if let Some(play) = &self.playback {
                if self
                    .animation_control
                    .as_ref()
                    .is_none_or(|control| control.epoch() != play.epoch)
                {
                    return false;
                }
                let Some(frame) = play.displayed().filter(|frame| frame.epoch == play.epoch) else {
                    return false;
                };
                let r = focused_rect(frame.art.w, frame.art.h, self.viewport, self.viewport_size);
                if frame.pixels < texture_width(r.w, window.scale_factor()) {
                    return false;
                }
                return !frame.tiles.is_empty()
                    && frame
                        .tiles
                        .iter()
                        .all(|(_, image)| crate::image_assets::track(image, window, cx).is_some());
            }
        }
        let visible = Rect {
            x: 0.,
            y: 0.,
            w: self.viewport_size.0,
            h: self.viewport_size.1,
        };
        let mut count = 0;
        for (index, tile) in self.tiles.iter().enumerate() {
            if (self.focused && self.selected_tile != Some(index))
                || !self.screen_rect(index).intersects(visible)
            {
                continue;
            }
            count += 1;
            if tile.error.as_deref() == Some("__empty__") {
                continue;
            }
            let r = self.screen_rect(index);
            let pixels = texture_width(r.w, window.scale_factor());
            if tile.loading
                || tile.pixels < pixels
                || ((!self.fixture || self.benchmark) && tile.pixels != pixels)
            {
                return false;
            }
            if let Some(required) = tile
                .art
                .as_deref()
                .and_then(|art| self.crop_for_art(art, r, pixels, 0.))
            {
                if tile.image_region.is_none_or(|have| {
                    have.x > required.x + 1.
                        || have.y > required.y + 1.
                        || have.x + have.w < required.x + required.w - 1.
                        || have.y + have.h < required.y + required.h - 1.
                }) {
                    return false;
                }
            }
            if tile
                .image
                .as_ref()
                .is_none_or(|image| crate::image_assets::track(image, window, cx).is_none())
            {
                return false;
            }
        }
        count > 0
    }
    fn start_background(&mut self, cx: &mut Context<Self>) {
        self.pending_analysis = service::analysis_status();
        let (tx, rx) = async_channel::bounded(64);
        let sink: service::ProgressSink = Arc::new(move |event| {
            if event["kind"] == "analysis-progress" {
                let _ = tx.try_send(event);
            } else {
                let _ = tx.send_blocking(event);
            }
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
                                if let Some(id) = event["projectId"].as_str() {
                                    view.automatic_job = Some((
                                        id.into(),
                                        event["generation"].as_u64().unwrap_or(0),
                                    ));
                                    if !view.job_busy && view.selected.as_deref() == Some(id) {
                                        view.progress = Some(json!({"stage":"history"}));
                                    }
                                }
                            }
                            "analysis-progress" => {
                                if !view.job_busy
                                    && event["projectId"].as_str() == view.selected.as_deref()
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
                                    if event["projectId"].as_str() != view.selected.as_deref() {
                                        if !view.job_busy {
                                            view.progress = None;
                                        }
                                        view.start_next_project(cx);
                                        cx.notify();
                                        return;
                                    }
                                    view.warnings =
                                        crate::ui_reports::analysis_warnings(&event["result"]);
                                    view.start_next_project(cx);
                                    if !event["error"].is_null() {
                                        if event["error"]["code"] == "cancelled" {
                                            view.notice = Some(
                                                view.text("已取消分析", "Analysis cancelled")
                                                    .into(),
                                            );
                                        } else {
                                            view.notice = crate::ui_reports::error_text(
                                                &event["error"],
                                                &view.preferences.language,
                                            );
                                        }
                                    }
                                    if !view.job_busy {
                                        view.progress = None;
                                        view.refresh(cx);
                                    }
                                }
                            }
                            "collect-started" => view.collect["running"] = json!(true),
                            "collect-done" | "collect-finished" => {
                                view.collect = service::collect_info()
                            }
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
                            "error" => {
                                view.error = crate::ui_reports::error_text(
                                    &event["error"],
                                    &view.preferences.language,
                                )
                            }
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
        self.roam_motion.stop();
        self.drag = None;
        self.hovered_tile = None;
        self.project_row_menu = None;
        if panel == Panel::Project {
            self.settings_project = self.selected.clone();
            self.settings_details = None;
        }
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
        self.panel_tween = (!self.reduced_motion).then_some(Tween {
            from,
            to: 1.,
            started: Instant::now(),
        });
        self.panel_close_focus.focus(window, cx);
        self.remove_confirm = false;
        self.hover = None;
        if panel == Panel::Project && self.panel.is_some() {
            let sign = self
                .settings_data()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.details.as_ref().and(self.selected.clone());
        }
        if panel == Panel::Projects {
            self.picker_selected = None;
            self.search_query.clear();
            self.search_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.project_scroll.scroll_to_item(0);
            self.search_input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            self.discover_projects(cx);
            self.fill_project_marks(cx);
        }
        if panel == Panel::Settings && !self.fixture {
            self.collect = service::collect_info();
        }
        cx.notify();
    }
    fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.row_mark_generation = self.row_mark_generation.wrapping_add(1);
        if let Some(cancel) = self.row_mark_cancellation.take() {
            cancel.cancel();
        }
        if self.reduced_motion {
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
    fn settings_id(&self) -> Option<&String> {
        self.settings_project
            .as_ref()
            .filter(|_| self.panel == Some(Panel::Project))
            .or(self.selected.as_ref())
            .filter(|id| id.as_str() != "__all__")
    }
    fn settings_current(&self) -> Option<&Project> {
        let id = self.settings_id()?;
        self.projects.iter().find(|p| &p.id == id)
    }
    fn settings_data(&self) -> Option<&Value> {
        if self.settings_project.is_some() && self.settings_project != self.selected {
            self.settings_details.as_ref()
        } else {
            self.details.as_ref()
        }
    }
    fn open_project_settings(
        &mut self,
        id: String,
        remove: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_panel(Panel::Project, window, cx);
        self.settings_project = Some(id);
        self.settings_details = None;
        self.sign_loaded_for = None;
        self.remove_confirm = remove;
        self.fetch_details(cx);
        cx.notify();
    }
    fn picker_items(&self, cx: &App) -> Vec<String> {
        let q = self.search_input.read(cx).value().trim().to_lowercase();
        let mut items = Vec::new();
        if self.projects.len() > 1
            && (q.is_empty()
                || self
                    .text("全部项目", "All projects")
                    .to_lowercase()
                    .contains(&q)
                || "all".contains(&q))
        {
            items.push("__all__".into());
        }
        items.extend(
            self.projects
                .iter()
                .filter(|p| {
                    q.is_empty() || format!("{} {}", p.name, p.path).to_lowercase().contains(&q)
                })
                .map(|p| p.id.clone()),
        );
        items.extend(
            self.discovered
                .iter()
                .flatten()
                .filter(|p| {
                    q.is_empty()
                        || format!(
                            "{} {}",
                            p["name"].as_str().unwrap_or(""),
                            p["path"].as_str().unwrap_or("")
                        )
                        .to_lowercase()
                        .contains(&q)
                })
                .filter_map(|p| p["path"].as_str().map(|path| format!("path:{path}"))),
        );
        items
    }
    fn move_picker(&mut self, delta: isize, cx: &mut Context<Self>) {
        let items = self.picker_items(cx);
        if items.is_empty() {
            return;
        }
        let at = self
            .picker_selected
            .as_ref()
            .and_then(|key| items.iter().position(|item| item == key));
        let next = at
            .map(|at| (at as isize + delta).rem_euclid(items.len() as isize) as usize)
            .unwrap_or(if delta > 0 { 0 } else { items.len() - 1 });
        self.picker_selected = Some(items[next].clone());
        self.project_row_menu = None;
        self.project_scroll.scroll_to_item(next);
        cx.notify();
    }
    fn activate_picker(&mut self, cx: &mut Context<Self>) {
        let items = self.picker_items(cx);
        let Some(id) = self
            .picker_selected
            .clone()
            .filter(|id| items.contains(id))
            .or_else(|| items.first().cloned())
        else {
            return;
        };
        self.project_row_menu = None;
        self.panel = None;
        if id == "__all__" {
            self.show_all_projects(cx);
        } else if let Some(path) = id.strip_prefix("path:") {
            self.queue_projects([path.into()], cx);
        } else {
            self.select(id, cx);
        }
    }
    fn fetch_details(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let selection = id.clone();
        self.details_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::project_details(id) })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.settings_id() != Some(&selection) {
                    return;
                }
                match result {
                    Ok(details) => {
                        if view.selected.as_ref() == Some(&selection) {
                            view.details = Some(details);
                        } else {
                            view.settings_details = Some(details);
                        }
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
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
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }
    fn export_evidence(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
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
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
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
        if self.job_busy || self.automatic_job.is_some() {
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
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
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = this.update(cx, |view, cx| view.start_evidence_import(id, path, cx));
        }));
    }
    fn start_evidence_import(&mut self, id: String, path: PathBuf, cx: &mut Context<Self>) {
        if self.job_busy || self.automatic_job.is_some() {
            self.notice = Some(
                self.text(
                    "已有任务正在运行，请稍后重试",
                    "Another task is running; try again when it finishes",
                )
                .into(),
            );
            cx.notify();
            return;
        }
        let cancellation = service::AnalysisCancellation::default();
        self.analysis_cancellation = Some(cancellation.clone());
        self.job_busy = true;
        self.job_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::import_evidence(id, path, cancellation) })
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
                view.start_next_project(cx);
                cx.notify();
            });
        }));
        cx.notify();
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
                view.collect = service::collect_info();
                match result {
                    Ok(_) => {
                        view.notice = Some(view.text("已更新", "Updated").into());
                        view.fetch_details(cx);
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
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
        let Some(id) = self.settings_id().cloned() else {
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
                            view.rebuild_preserving_view(cx);
                        } else if view.settings_id() == Some(&selection) {
                            view.settings_details = Some(details);
                        }
                        view.notice =
                            Some(view.text("显示设置已保存", "Display settings saved").into());
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn set_integration(&mut self, which: &str, on: bool, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
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
        let Some(id) = self.settings_id().cloned() else {
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
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }
    fn link_folder(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
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
                match choice {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let path = path.to_string_lossy().into_owned();
                            view.operation(
                                move || service::link_path(id, path, Some(false), Some(lang)),
                                cx,
                            );
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    fn unlink_path(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        self.operation(move || service::link_path(id, path, Some(true), None), cx);
    }
    fn show_all_projects(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        if self.selected.as_deref() != Some("__all__") {
            self.focus_styles = None;
        }
        self.pending_view = if self.selected.as_deref() == Some("__all__") && self.raw.is_some() {
            Some(self.bookmark())
        } else {
            None
        };
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
            && self.current_tile().is_some_and(|tile| {
                ["txt", "md", "json", "embed"].contains(&self.export_format.as_str())
                    || (tile.art.is_some() && tile.error.is_none())
            })
            && !self.export_prompt_busy
            && (self.export_format != "gif"
                || self.selected_piece().is_some_and(|p| p["animated"] == true))
    }
    fn export_history_writable(&self) -> bool {
        self.fixture
            || (self.export_history_error.is_none()
                && self
                    .export_journal
                    .as_ref()
                    .is_some_and(|journal| !journal.blocked()))
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
        if let Some(person) = &self.profile_person {
            snapshot["person"] = json!(person);
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
                self.export_history_error = Some(error.to_string());
                self.error = Some(error.to_string());
                return;
            }
        };
        match crate::export_jobs::Queue::from_snapshot(snapshot["queue"].clone()) {
            Ok(queue) => self.export_queue = queue,
            Err(error) => {
                self.export_history_error = Some(error.clone());
                self.error = Some(error);
                return;
            }
        }
        self.export_history_error = None;
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
                            view.export_history_error = Some(error.clone());
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
    fn reload_export_history(&mut self, cx: &mut Context<Self>) {
        if self.fixture
            || self
                .export_queue
                .jobs()
                .iter()
                .any(|job| job.state.pending())
        {
            return;
        }
        self.report_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async { service::load_export_jobs() })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.load_export_history(result, cx);
                cx.notify();
            });
        }));
    }
    fn set_collection_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        let on = mode != "off";
        let always = mode == "always";
        let supported = self.collect["agent"]["supported"]
            .as_bool()
            .unwrap_or(false);
        self.operation(
            move || {
                service::set_collect(on)?;
                if supported {
                    service::set_collect_agent(always)?;
                }
                Ok(service::collect_info())
            },
            cx,
        );
    }
    fn persist_exports(&self) {
        if let Some(journal) = &self.export_journal {
            journal.submit(self.export_queue.clone());
        }
    }
    fn export_artwork(&mut self, _: &ExportArtwork, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() || !self.export_history_writable() {
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
                    Ok(Ok(Some(_))) if !view.export_history_writable() => {
                        view.panel = Some(Panel::Exports);
                    }
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
            let path = job.path.clone();
            let output_path = path.clone();
            let result = cx
                .background_spawn(async move {
                    let output = renderer::request(job.renderer_request(), render_cancel)?;
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
        if !self.export_history_writable() {
            return;
        }
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
        if self.drag.take().is_some() {
            self.export_hovered = None;
            self.roam_motion.stop();
            cx.notify();
            return;
        }
        if self.project_row_menu.take().is_some() {
            cx.notify();
            return;
        }

        if self.panel.is_some() {
            self.close_panel(window, cx);
        } else if self.focused {
            self.close_focus(cx);
        } else if self.export_open {
            self.export_open = false;
            cx.notify();
        } else if self.export_stage.is_some() {
            self.cancel_export(&CancelExport, window, cx);
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
    fn wheel(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        self.roam_motion.stop();
        let delta = event.delta.pixel_delta(px(24.));
        let dx: f32 = delta.x.into();
        let dy: f32 = delta.y.into();
        if event.modifiers.control || event.modifiers.platform {
            if !self.focused && self.preferences.view != "roam" {
                self.set_view("roam", cx);
            }
            self.zoom_at((-dy * 0.006).exp(), self.pointer.0, self.pointer.1);
        } else {
            self.viewport.y += if event.modifiers.shift && dx == 0. {
                0.
            } else {
                dy
            };
            if self.preferences.view == "roam" || self.focused {
                self.viewport.x += if event.modifiers.shift && dx == 0. {
                    dy
                } else {
                    dx
                };
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
                    if drag.tile.is_some() {
                        self.export_open = true;
                    }
                }
                if drag.tile.is_none() && (self.preferences.view == "roam" || self.focused) {
                    self.viewport.x += dx;
                    self.viewport.y += dy;
                    if self.preferences.view == "roam" && !self.focused && drag.moved {
                        self.roam_motion.record(
                            self.motion_clock.elapsed(),
                            (self.viewport.x, self.viewport.y),
                        );
                    }
                }
                drag.last = local;
                if drag.tile.is_some() && drag.moved {
                    let formats = self
                        .catalog
                        .as_ref()
                        .and_then(|c| c["formats"].as_array())
                        .map(Vec::len)
                        .unwrap_or(0);
                    self.export_hovered = nearest_child(&self.format_bounds, formats, x, y);
                    if self.export_hovered.is_none() {
                        let size_count = self
                            .catalog
                            .as_ref()
                            .and_then(|c| c["sizes"].as_array())
                            .map(Vec::len)
                            .unwrap_or(0);
                        if let Some(index) = nearest_child(&self.size_bounds, size_count, x, y) {
                            if let Some(size) = self
                                .catalog
                                .as_ref()
                                .and_then(|c| c["sizes"][index]["id"].as_str())
                            {
                                if self.export_size != size {
                                    self.export_size = size.into();
                                    self.preferences.export_size = size.into();
                                    self.persist();
                                }
                            }
                        }
                    }
                }
                self.hover = None;
                cx.notify();
                return;
            } else {
                self.drag = None;
            }
        }
        if (self.focused && self.focus_placeholder.is_some())
            || !self.artwork_viewport().contains(local.0, local.1)
        {
            if self.hover.take().is_some() || self.hovered_tile.take().is_some() {
                cx.notify();
            }
            return;
        }
        let mut found = None;
        let mut hovered = None;
        for (index, tile) in self.tiles.iter().enumerate() {
            if self.focused && self.selected_tile != Some(index) {
                continue;
            }
            let rect = self.screen_rect(index);
            let caption = Rect {
                y: rect.y - 30.,
                h: rect.h + 30.,
                ..rect
            };
            if caption.contains(local.0, local.1) {
                hovered = Some(index);
            }
            if !rect.contains(local.0, local.1) {
                continue;
            }
            let art = if self.focused {
                self.playback
                    .as_ref()
                    .and_then(Playback::displayed)
                    .map(|f| f.art.as_ref())
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
        if hovered != self.hovered_tile {
            self.hovered_tile = hovered;
            cx.notify();
        }
        if found.is_some()
            || found.as_ref().map(|h| (&h.t, &h.d)) != self.hover.as_ref().map(|h| (&h.t, &h.d))
        {
            self.hover = found;
            cx.notify();
        }
    }
    fn pointer_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.export_hovered = None;
        if self.panel.is_some() {
            self.drag = None;
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.moved {
            if let Some(index) = drag.clicked_tile {
                if !self.focused {
                    self.select_tile(index, true, cx);
                }
            } else if self.focused {
                self.close_focus(cx);
            }
            return;
        }
        let Some(from) = drag.tile else {
            if self.preferences.view == "roam" && !self.focused {
                self.roam_motion.release(
                    self.motion_clock.elapsed(),
                    (self.viewport.x, self.viewport.y),
                    !self.reduced_motion && self.window_visible,
                );
                cx.notify();
            }
            return;
        };
        let x: f32 = event.position.x.into();
        let y: f32 = event.position.y.into();
        let local = (x, y - 60.);
        let formats = self
            .catalog
            .as_ref()
            .and_then(|c| c["formats"].as_array())
            .cloned()
            .unwrap_or_default();
        if let Some(index) = nearest_child(&self.format_bounds, formats.len(), x, y) {
            if let Some(format) = formats[index]["id"].as_str() {
                if format != "gif"
                    || self
                        .selected_piece()
                        .is_some_and(|piece| piece["animated"] == true)
                {
                    self.selected_tile = Some(from);
                    self.export_format = format.into();
                    self.preferences.export_format = format.into();
                    self.persist();
                    self.export_artwork(&ExportArtwork, window, cx);
                    return;
                }
            }
        }
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
        if let Some(cancel) = &self.mark_cancellation {
            cancel.cancel();
        }
        if let Some(cancel) = &self.row_mark_cancellation {
            cancel.cancel();
        }
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
        let selection = &self.tiles[index];
        let tile = if self.focused {
            self.focus_placeholder.as_ref().unwrap_or(selection)
        } else {
            selection
        };
        let r = self.screen_rect(index);
        let selected = self.selected_tile == Some(index);
        let dragging = self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.moved && drag.tile == Some(index));
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
        let animation = self
            .focused
            .then(|| self.playback.as_ref().and_then(Playback::displayed))
            .flatten();
        let image = tile.image.clone();
        let mut picture = div()
            .id(("artwork", index))
            .test_support()
            .relative()
            .w(px(r.w))
            .h(px(r.h))
            .overflow_hidden()
            .rounded(px(10.))
            .cursor_pointer()
            .when(dragging, |picture| picture.opacity(0.3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseDownEvent, _, cx| {
                    if view.panel.is_some() {
                        cx.stop_propagation();
                        return;
                    }
                    view.selected_tile = Some(index);
                    view.roam_motion.stop();
                    view.drag = Some(DragState {
                        tile: if view.focused || view.preferences.view == "roam" {
                            None
                        } else {
                            Some(index)
                        },
                        clicked_tile: Some(index),
                        source: Some(view.screen_rect(index)),
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        if let Some(frame) = animation {
            for (part, (region, image)) in frame.tiles.iter().enumerate() {
                let region = region.unwrap_or(Rect {
                    x: 0.,
                    y: 0.,
                    w: frame.art.w,
                    h: frame.art.h,
                });
                let exact = frame.pixels == texture_width(r.w, self.display_scale)
                    && self
                        .animation_control
                        .as_ref()
                        .is_some_and(|control| control.epoch() == frame.epoch);
                let scale = frame.pixels as f32 / frame.art.w / self.display_scale.max(0.1);
                let actual = frame.regions.get(part);
                let (left, top, width, height) = if exact {
                    (
                        (region.x * scale * self.display_scale).round() / self.display_scale,
                        (region.y * scale * self.display_scale).round() / self.display_scale,
                        actual.and_then(|r| r["width"].as_u64()).unwrap_or(1) as f32
                            / self.display_scale,
                        actual.and_then(|r| r["height"].as_u64()).unwrap_or(1) as f32
                            / self.display_scale,
                    )
                } else {
                    (
                        region.x / frame.art.w * r.w,
                        region.y / frame.art.h * r.h,
                        region.w / frame.art.w * r.w,
                        region.h / frame.art.h * r.h,
                    )
                };
                picture = picture.child(
                    img(image.clone())
                        .id(("animation-region", part))
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .object_fit(ObjectFit::Fill),
                );
            }
        } else if let Some(image) = image {
            if let (Some(region), Some(art)) = (tile.image_region, tile.art.as_deref()) {
                let exact =
                    tile.pixels == texture_width(r.w, self.display_scale) && tile.image_size.0 > 0;
                let scale = tile.pixels as f32 / art.w / self.display_scale;
                let (left, top, width, height) = if exact {
                    (
                        (region.x * scale * self.display_scale).round() / self.display_scale,
                        (region.y * scale * self.display_scale).round() / self.display_scale,
                        tile.image_size.0 as f32 / self.display_scale,
                        tile.image_size.1 as f32 / self.display_scale,
                    )
                } else {
                    (
                        region.x / art.w * r.w,
                        region.y / art.h * r.h,
                        region.w / art.w * r.w,
                        region.h / art.h * r.h,
                    )
                };
                picture = picture.child(
                    img(image)
                        .id(("artwork-image", index))
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .object_fit(ObjectFit::Fill),
                );
            } else {
                picture = picture.child(
                    img(image)
                        .id(("artwork-image", index))
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(r.w))
                        .h(px(if tile.image_size.0 > 0 {
                            r.w * tile.image_size.1 as f32 / tile.image_size.0 as f32
                        } else {
                            r.h
                        }))
                        .object_fit(ObjectFit::Fill)
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
        picture = picture.child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(10.))
                .border_1()
                .border_color(if self.hovered_tile == Some(index) {
                    border
                } else {
                    rgba(if cx.theme().is_dark() {
                        0xFFFFFF0F
                    } else {
                        0x00000012
                    })
                    .into()
                }),
        );
        let mut card = div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .rounded(px(10.))
            .shadow(vec![
                BoxShadow::new(px(0.), px(12.), rgba(0x00000014).into()).blur_radius(px(32.)),
            ])
            .child(picture);
        if !self.focused {
            let variant_id = tile.spec.variant.as_deref().unwrap_or("");
            let variant = self
                .piece(&tile.spec.piece)
                .and_then(|piece| piece["variants"].as_array())
                .and_then(|variants| variants.iter().find(|variant| variant["id"] == variant_id))
                .and_then(|variant| {
                    variant[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                })
                .unwrap_or(variant_id)
                .to_string();
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
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .max_w(px(r.w * 0.58))
                            .overflow_hidden()
                            .child(title)
                            .child(variant),
                    )
                    .when(self.hovered_tile == Some(index) || selected, |caption| {
                        caption.child(
                            div()
                                .flex()
                                .gap_1()
                                .child(
                                    Button::new(("tile-focus", index))
                                        .ghost()
                                        .icon(crate::ui_icons::icon("search"))
                                        .accessibility_label(self.text("放大作品", "Open artwork"))
                                        .tooltip(self.text("放大作品", "Open artwork"))
                                        .h(px(22.))
                                        .w(px(24.))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.select_tile(index, true, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("tile-export", index))
                                        .ghost()
                                        .icon(crate::ui_icons::icon("export"))
                                        .accessibility_label(
                                            self.text("导出作品", "Export artwork"),
                                        )
                                        .tooltip(self.text("导出作品", "Export artwork"))
                                        .h(px(22.))
                                        .w(px(24.))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.selected_tile = Some(index);
                                            view.export_open = true;
                                            cx.notify();
                                        })),
                                ),
                        )
                    }),
            );
        }
        if self.hovered_tile == Some(index) {
            card = card.border_color(border);
        }
        card.into_any_element()
    }
    fn render_stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
            .on_scroll(cx.listener(|view, event, _, cx| view.wheel(event, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.panel.is_some() {
                        cx.stop_propagation();
                        return;
                    }
                    view.roam_motion.stop();
                    view.drag = Some(DragState {
                        tile: None,
                        clicked_tile: None,
                        source: None,
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    view.hover = None;
                    cx.notify();
                }),
            );
        if self.focused {
            let viewport = self.saved_viewport.unwrap_or_default();
            for (index, tile) in self.tiles.iter().enumerate() {
                let Some(image) = tile.image.clone().filter(|_| tile.image_region.is_none()) else {
                    continue;
                };
                let Some(rect) = self
                    .rects
                    .get(index)
                    .copied()
                    .map(|rect| rect.transformed(viewport))
                else {
                    continue;
                };
                if !rect.intersects(visible) {
                    continue;
                }
                stage = stage.child(
                    div()
                        .absolute()
                        .left(px(rect.x))
                        .top(px(rect.y))
                        .w(px(rect.w))
                        .h(px(rect.h))
                        .opacity(0.12)
                        .child(
                            img(image)
                                .size_full()
                                .rounded(px(10.))
                                .object_fit(ObjectFit::Contain),
                        ),
                );
            }
        }
        if self.loading {
            if self.activity_age() >= Duration::from_millis(300) {
                stage = stage.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_col()
                        .gap_4()
                        .child(self.center_mark.render(
                            self.progress.as_ref().and_then(analysis_fraction),
                            true,
                            false,
                            self.reduced_motion,
                            64.,
                            window,
                            cx,
                        ))
                        .when(self.activity_age() >= Duration::from_secs(2), |view| {
                            view.child(self.text("正在读取作品库…", "Loading project library…"))
                        }),
                );
            }
        } else if self.raw.is_none() {
            let mut empty = div()
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
                        .child(self.center_mark.render(
                            self.progress.as_ref().and_then(analysis_fraction),
                            self.job_busy || self.discover_busy,
                            self.error.is_some(),
                            self.reduced_motion,
                            56.,
                            window,
                            cx,
                        ))
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
            if self.selected.is_none() {
                let mut choices = div()
                    .id("welcome-candidates")
                    .w(px(540.))
                    .max_h(px((self.viewport_size.1 - 290.).max(100.)))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1();
                if self.discover_busy {
                    choices =
                        choices.child(self.text("正在查找本机项目…", "Finding local projects…"));
                }
                for (index, project) in self.discovered.as_ref().into_iter().flatten().enumerate() {
                    let path = project["path"].as_str().unwrap_or("").to_string();
                    let picked = self.discovery_selected.contains(&path);
                    let name = project["name"].as_str().unwrap_or(&path).to_string();
                    choices = choices.child(
                        Button::new(("welcome-project", index))
                            .ghost()
                            .label(format!("{} {}", if picked { "✓" } else { "○" }, name))
                            .disabled(self.job_busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(at) =
                                    view.discovery_selected.iter().position(|p| p == &path)
                                {
                                    view.discovery_selected.remove(at);
                                } else {
                                    view.discovery_selected.push(path.clone());
                                }
                                cx.notify();
                            })),
                    );
                }
                if let Some(error) = &self.discovery_error {
                    choices = choices.child(div().text_sm().child(error.clone())).child(
                        Button::new("welcome-retry")
                            .ghost()
                            .label(self.text("重新查找", "Try again"))
                            .on_click(cx.listener(|view, _, _, cx| view.retry_discovery(cx))),
                    );
                } else if self.discovered.as_ref().is_some_and(Vec::is_empty) {
                    choices = choices.child(self.text(
                        "没有找到含 AI 记录的项目，可以选择一个文件夹。",
                        "No projects with AI records were found. Choose a folder to begin.",
                    ));
                }
                if !self.discovery_selected.is_empty() {
                    choices = choices.child(
                        Button::new("welcome-analyze")
                            .label(format!(
                                "{} ({})",
                                self.text("分析所选项目", "Analyze selected"),
                                self.discovery_selected.len()
                            ))
                            .disabled(self.job_busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                let paths = std::mem::take(&mut view.discovery_selected);
                                view.queue_projects(paths, cx);
                                view.open_panel(Panel::Projects, window, cx);
                            })),
                    );
                }
                empty = empty.child(choices);
            }
            stage = stage.child(empty);
        } else {
            for index in 0..self.tiles.len() {
                if (!self.focused || self.selected_tile == Some(index))
                    && self.screen_rect(index).intersects(visible)
                {
                    let artwork = self.artwork_element(index, cx);
                    stage = if self.focused {
                        stage.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .w_full()
                                .h(px((self.viewport_size.1 - 154.).max(120.)))
                                .overflow_hidden()
                                .child(artwork),
                        )
                    } else {
                        stage.child(artwork)
                    };
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
        let mut dock = div()
            .id("export-dock")
            .occlude()
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
            return div()
                .h(px(self.dock_height))
                .flex_shrink_0()
                .overflow_hidden()
                .into_any_element();
        }
        let formats = self
            .catalog
            .as_ref()
            .and_then(|c| c["formats"].as_array())
            .cloned()
            .unwrap_or_default();
        let mut buttons = div()
            .id("export-formats")
            .track_scroll(&self.format_bounds)
            .flex()
            .flex_wrap()
            .gap_1();
        for (index, format) in formats.iter().enumerate() {
            let Some(id) = format["id"].as_str() else {
                continue;
            };
            let id = id.to_string();
            let chosen = self.export_format == id;
            let drop_format = id.clone();
            let disabled =
                id == "gif" && self.selected_piece().is_none_or(|p| p["animated"] != true);
            buttons = buttons.child(
                div()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _, window, cx| {
                            if !disabled
                                && view
                                    .drag
                                    .as_ref()
                                    .is_some_and(|drag| drag.moved && drag.tile.is_some())
                            {
                                if let Some(drag) = view.drag.take() {
                                    view.selected_tile = drag.tile;
                                }
                                view.export_hovered = None;
                                view.export_format = drop_format.clone();
                                view.preferences.export_format = drop_format.clone();
                                view.persist();
                                view.export_artwork(&ExportArtwork, window, cx);
                                cx.stop_propagation();
                            }
                        }),
                    )
                    .child(
                        Button::new(("format", index))
                            .ghost()
                            .label(id.to_uppercase())
                            .when(chosen || self.export_hovered == Some(index), |button| {
                                button.bg(cx.theme().muted)
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.export_format = id.clone();
                                view.preferences.export_format = id.clone();
                                view.persist();
                                cx.notify();
                            })),
                    ),
            );
        }
        let mut sizes = div()
            .id("export-sizes")
            .track_scroll(&self.size_bounds)
            .flex()
            .flex_wrap()
            .gap_1();
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
                            .disabled(!self.can_export() || !self.export_history_writable())
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
                body = body.child(
                    div()
                        .id("project-search")
                        .test_support()
                        .track_focus(&self.search_input.read(cx).focus_handle(cx))
                        .child(Input::new(&self.search_input)),
                );
                let items = self.picker_items(cx);
                let mut rows = div()
                    .id("project-list")
                    .track_scroll(&self.project_scroll)
                    .overflow_y_scroll()
                    .max_h(px(380.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .on_scroll(cx.listener(|view, _, _, cx| {
                        view.project_row_menu = None;
                        view.fill_project_marks(cx);
                        cx.notify();
                    }));
                if items.iter().any(|id| id == "__all__") {
                    rows = rows.child(
                        Button::new("all-projects")
                            .ghost()
                            .label(format!(
                                "{} · {}",
                                self.text("全部项目", "All projects"),
                                self.projects.len()
                            ))
                            .when(self.picker_selected.as_deref() == Some("__all__"), |b| {
                                b.bg(cx.theme().muted)
                            })
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.panel = None;
                                view.show_all_projects(cx)
                            })),
                    );
                }
                for (index, project) in self
                    .projects
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| items.contains(&p.id))
                {
                    let id = project.id.clone();
                    let more_id = id.clone();
                    rows = rows.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when_some(self.project_marks.get(&project.id), |row, mark| {
                                row.child(img(mark.image.clone()).w(px(24.)).h(px(24.)))
                            })
                            .rounded_md()
                            .when(self.picker_selected.as_ref() == Some(&id), |row| {
                                row.bg(cx.theme().muted)
                            })
                            .child(
                                Button::new(("choose-project", index))
                                    .ghost()
                                    .flex_1()
                                    .h(px(58.))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_col()
                                            .items_start()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .w_full()
                                                    .flex()
                                                    .justify_between()
                                                    .gap_3()
                                                    .child(project.name.clone())
                                                    .child(
                                                        div().text_xs().text_color(muted).child(
                                                            if !project.exists {
                                                                self.text(
                                                                    "目录不在了",
                                                                    "Folder missing",
                                                                )
                                                                .to_string()
                                                            } else {
                                                                project
                                                                    .last_analyzed
                                                                    .map(|stamp| {
                                                                        project_date(
                                                                            stamp,
                                                                            &self
                                                                                .preferences
                                                                                .language,
                                                                        )
                                                                    })
                                                                    .unwrap_or_default()
                                                            },
                                                        ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .truncate()
                                                    .child(project.path.clone()),
                                            ),
                                    )
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.panel = None;
                                        view.project_row_menu = None;
                                        view.select(id.clone(), cx)
                                    })),
                            )
                            .child(
                                Button::new(("project-more", index))
                                    .ghost()
                                    .icon(crate::ui_icons::icon("more"))
                                    .accessibility_label(self.text("更多", "More"))
                                    .on_click(cx.listener(
                                        move |view, event: &ClickEvent, window, cx| {
                                            if view
                                                .project_row_menu
                                                .as_ref()
                                                .is_some_and(|(id, _)| id == &more_id)
                                            {
                                                view.project_row_menu = None;
                                            } else {
                                                let point = event.position();
                                                let x: f32 = point.x.into();
                                                let y: f32 = point.y.into();
                                                let size = window.viewport_size();
                                                let width: f32 = size.width.into();
                                                let height: f32 = size.height.into();
                                                view.project_row_menu = Some((
                                                    more_id.clone(),
                                                    (
                                                        (x - 216.)
                                                            .clamp(8., (width - 240.).max(8.)),
                                                        if y + 190. > height - 8. {
                                                            (y - 190.).max(8.)
                                                        } else {
                                                            y + 4.
                                                        },
                                                    ),
                                                ));
                                            }
                                            cx.stop_propagation();
                                            cx.notify();
                                        },
                                    )),
                            ),
                    );
                }
                let discovered = self
                    .discovered
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .filter(|(_, p)| {
                        p["path"]
                            .as_str()
                            .is_some_and(|path| items.contains(&format!("path:{path}")))
                    })
                    .collect::<Vec<_>>();
                for (index, project) in discovered {
                    let path = project["path"].as_str().unwrap_or("").to_string();
                    let selected_path = path.clone();
                    let run_path = path.clone();
                    let key = format!("path:{path}");
                    let picked = self.discovery_selected.contains(&path);
                    let name = project["name"].as_str().unwrap_or(&path).to_string();
                    let tools = project["tools"]
                        .as_object()
                        .map(|tools| tools.keys().cloned().collect::<Vec<_>>().join(" · "))
                        .unwrap_or_default();
                    rows = rows.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded_md()
                            .when(self.picker_selected.as_ref() == Some(&key), |row| {
                                row.bg(cx.theme().muted)
                            })
                            .child(
                                Button::new(("discovered", index))
                                    .ghost()
                                    .label(if picked { "✓" } else { "○" })
                                    .disabled(self.job_busy)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if let Some(i) = view
                                            .discovery_selected
                                            .iter()
                                            .position(|p| p == &selected_path)
                                        {
                                            view.discovery_selected.remove(i);
                                        } else {
                                            view.discovery_selected.push(selected_path.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("discover-open", index))
                                    .ghost()
                                    .flex_1()
                                    .h(px(58.))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_col()
                                            .items_start()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .w_full()
                                                    .flex()
                                                    .justify_between()
                                                    .child(name)
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .child(tools),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .truncate()
                                                    .child(path),
                                            ),
                                    )
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.queue_projects([run_path.clone()], cx)
                                    })),
                            ),
                    );
                }
                body = body.child(rows);
                if self.discover_busy {
                    body = body.child(self.text("正在查找本机项目…", "Finding local projects…"));
                }
                if let Some(error) = &self.discovery_error {
                    body = body.child(div().text_sm().child(error.clone())).child(
                        Button::new("retry-discovery")
                            .ghost()
                            .label(self.text("重新查找", "Try again"))
                            .on_click(cx.listener(|view, _, _, cx| view.retry_discovery(cx))),
                    );
                } else if items.is_empty() && !self.discover_busy {
                    body = body.child(self.text("没有匹配的项目", "No matching projects"));
                }
                if !self.discovery_selected.is_empty() {
                    body = body.child(
                        Button::new("add-selected")
                            .label(format!(
                                "{} ({})",
                                self.text("分析所选项目", "Analyze selected"),
                                self.discovery_selected.len()
                            ))
                            .disabled(self.job_busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                let paths = std::mem::take(&mut view.discovery_selected);
                                view.queue_projects(paths, cx);
                            })),
                    );
                }
                for (index, path) in self.batch_order.iter().enumerate() {
                    let Some(result) = self.batch_results.get(path) else {
                        continue;
                    };
                    let state = result["state"].as_str().unwrap_or("queued");
                    let label = match state {
                        "running" => self.text("分析中", "Analyzing"),
                        "done" => self.text("已完成", "Done"),
                        "failed" => self.text("失败", "Failed"),
                        "cancelled" => self.text("已取消", "Cancelled"),
                        _ => self.text("等待中", "Queued"),
                    };
                    let retry = path.clone();
                    body = body.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().flex_1().text_sm().child(format!(
                                            "{} · {}",
                                            Path::new(path)
                                                .file_name()
                                                .and_then(|p| p.to_str())
                                                .unwrap_or(path),
                                            label
                                        )))
                                    .when(matches!(state, "failed" | "cancelled"), |row| {
                                        row.child(
                                            Button::new(("retry-batch", index))
                                                .ghost()
                                                .label(self.text("重试", "Retry"))
                                                .disabled(self.job_busy)
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.queue_projects([retry.clone()], cx)
                                                })),
                                        )
                                    }),
                            )
                            .when_some(result["error"].as_str(), |row, error| {
                                row.child(
                                    div().text_xs().text_color(muted).child(error.to_string()),
                                )
                            })
                            .children(
                                result["warnings"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_str)
                                    .map(|note| {
                                        div().text_xs().text_color(muted).child(note.to_string())
                                    }),
                            ),
                    );
                }
                body = body.child(
                    Button::new("add-project")
                        .label(self.text("添加文件夹…", "Add folder…"))
                        .disabled(self.job_busy)
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
                    if index == 15 {
                        choices =
                            choices.child(div().w_full().h(px(1.)).my_2().bg(cx.theme().border));
                    }
                    choices = choices.child(
                        Button::new(("style", index))
                            .ghost()
                            .when_some(style["dot"].as_str(), |button, dot| {
                                button.child(img(style_dot(dot)).w(px(20.)).h(px(20.)))
                            })
                            .label(name)
                            .disabled(
                                self.focused
                                    && self.current_tile().is_some_and(|tile| {
                                        !style["pieces"].as_array().is_some_and(|pieces| {
                                            pieces
                                                .iter()
                                                .any(|piece| piece["id"] == tile.spec.piece)
                                        })
                                    }),
                            )
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
                if let Some(error) = &self.export_history_error {
                    body = body.child(div().text_sm().child(error.clone())).child(
                        Button::new("reload-export-history")
                            .label(self.text("重新读取导出历史", "Reload export history"))
                            .disabled(
                                self.export_queue
                                    .jobs()
                                    .iter()
                                    .any(|job| job.state.pending()),
                            )
                            .on_click(cx.listener(|view, _, _, cx| view.reload_export_history(cx))),
                    );
                }
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
                                .disabled(!self.export_history_writable())
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
                if !self.fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false) {
                    body = body.child(div().text_sm().text_color(muted).child(self.text(
                        "系统已启用减少动态效果。",
                        "Reduced motion is enabled by the operating system.",
                    )));
                }

                if let Some(error) = &self.migration_error {
                    body = body.child(div().text_sm().child(error.clone())).child(
                        Button::new("retry-settings-migration")
                            .label(if self.migration_busy {
                                self.text("正在导入…", "Importing…")
                            } else {
                                self.text(
                                    "重新导入原有桌面设置",
                                    "Import previous desktop settings again",
                                )
                            })
                            .disabled(self.migration_busy)
                            .on_click(cx.listener(|view, _, _, cx| view.retry_migration(cx))),
                    );
                }

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
                                crate::image_assets::release(view.logo_image.clone(), cx);
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
                let mode = if agent {
                    "always"
                } else if on {
                    "open"
                } else {
                    "off"
                };
                let mut modes = div().flex().gap_1();
                for (id, label) in [
                    ("off", self.text("关闭", "Off")),
                    ("open", self.text("应用打开时", "While open")),
                    ("always", self.text("始终采集", "Always")),
                ] {
                    modes = modes.child(
                        Button::new(id)
                            .ghost()
                            .label(label)
                            .when(mode == id, |b| b.bg(cx.theme().muted))
                            .disabled(self.operation_busy || (id == "always" && !supported))
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.set_collection_mode(id, cx)),
                            ),
                    );
                }
                body = body
                    .child(modes)
                    .child(
                        Button::new("collect-now")
                            .label(self.text("立即采集", "Collect now"))
                            .disabled(self.operation_busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.operation(service::collect_now, cx)
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
                ))).child(div().text_sm().child(self.text("看清 AI 工具在仓库里写了多少，再把它做成 README、发版说明和作品集的署名。", "See what AI tools wrote in your repositories, then create credits for README files, releases and portfolios."))).child(div().text_xs().text_color(muted).child(self.text("只在本机运行，不上传代码或记录。", "Runs on this computer. No code or logs are uploaded.")));
            }
            Panel::Project => {
                body = body.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("analyze")
                                .ghost()
                                .icon(crate::ui_icons::icon("refresh"))
                                .label(self.text("重新分析", "Analyze"))
                                .disabled(
                                    self.job_busy
                                        || self.automatic_job.is_some()
                                        || self
                                            .settings_current()
                                            .is_none_or(|project| !project.exists),
                                )
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.start_job_target(
                                        None,
                                        false,
                                        view.settings_id().cloned(),
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            Button::new("copy")
                                .ghost()
                                .icon(crate::ui_icons::icon("copy"))
                                .label(self.text("复制档案", "Copy archive"))
                                .disabled(
                                    self.raw.is_none()
                                        || self.settings_id() != self.selected.as_ref(),
                                )
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.copy_archive(&CopyArchive, window, cx)
                                })),
                        )
                        .child(
                            Button::new("details")
                                .ghost()
                                .icon(crate::ui_icons::icon("info"))
                                .label(self.text("证据", "Evidence"))
                                .disabled(
                                    self.raw.is_none()
                                        || self.settings_id() != self.selected.as_ref(),
                                )
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.open_panel(Panel::Details, window, cx)
                                })),
                        ),
                );
                if let Some(project) = self.settings_current() {
                    body = body
                        .child(div().text_lg().child(project.name.clone()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(project.path.clone()),
                        );
                    if let Some(stamp) = project.last_analyzed {
                        body = body.child(div().text_sm().text_color(muted).child(format!(
                            "{} {}",
                            self.text("上次分析", "Last analyzed"),
                            project_date(stamp, &self.preferences.language)
                        )));
                    }
                }
                body = body
                    .child(
                        div()
                            .id("project-signature")
                            .test_support()
                            .track_focus(&self.sign_input.read(cx).focus_handle(cx))
                            .child(Input::new(&self.sign_input)),
                    )
                    .child(
                        Button::new("save-display")
                            .label(
                                self.text("保存署名与姓名设置", "Save signature and name settings"),
                            )
                            .disabled(
                                self.operation_busy
                                    || self.settings_data().is_none()
                                    || self.sign_loaded_for.as_ref() != self.settings_id(),
                            )
                            .on_click(cx.listener(|view, _, _, cx| view.save_project_display(cx))),
                    );
                let current_auto = self
                    .settings_id()
                    .and_then(|id| self.preferences.projects.get(id))
                    .and_then(|prefs| prefs.auto_analyze);
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
                                if let Some(id) = view.settings_id().cloned() {
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
                    let known = self
                        .settings_data()
                        .is_some_and(|d| d["integrations"][which].is_object());
                    let exists = self
                        .settings_current()
                        .is_some_and(|project| project.exists);
                    let on = self
                        .settings_data()
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
                            .disabled(self.operation_busy || !known || !exists)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.set_integration(which, !on, cx)
                            })),
                    );
                    if let Some(details) = self.settings_data() {
                        if let Some(path) = details["integrations"][which]["path"].as_str() {
                            body = body
                                .child(div().text_xs().text_color(muted).child(path.to_string()));
                        }
                        if which == "release" && known {
                            body = body.child(div().text_xs().text_color(muted).child(
                                if details["integrations"][which]["github"] == true {
                                    self.text("GitHub 发版工作流", "GitHub release workflow")
                                } else {
                                    self.text("需要 GitHub 远端仓库", "A GitHub remote is required")
                                },
                            ));
                        }
                    }
                }
                if let Some(details) = self.settings_data() {
                    if !details["integrationsError"].is_null() {
                        body = body
                            .child(
                                div().text_sm().child(
                                    crate::ui_reports::error_text(
                                        &details["integrationsError"],
                                        &self.preferences.language,
                                    )
                                    .unwrap_or_default(),
                                ),
                            )
                            .child(
                                Button::new("retry-integrations")
                                    .ghost()
                                    .label(self.text("重新检查接入", "Check integrations again"))
                                    .on_click(cx.listener(|view, _, _, cx| view.fetch_details(cx))),
                            );
                    }
                    if details["integrations"]["release"]["published"] == true {
                        body = body.child(div().text_sm().text_color(muted).child(self.text(
                            "仓库中已有发布档案",
                            "Published archive exists in this repository",
                        )));
                    }
                    if details["integrations"]["release"]["installed"] == true
                        && details["integrations"]["hook"]["installed"] != true
                    {
                        body = body.child(div().text_sm().text_color(muted).child(self.text(
                            "提交接入可为之后的发布补充证据",
                            "Enable commit integration to retain evidence for future releases",
                        )));
                    }
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
                        let kind = path["kind"].as_str().unwrap_or("primary");
                        let label = match kind {
                            "manual" => self.text("手动关联", "Linked"),
                            "auto" => self.text("自动发现", "Discovered"),
                            _ => self.text("主目录", "Main"),
                        };
                        let manual = kind == "manual";
                        let path = path["path"].as_str().unwrap_or("").to_string();
                        let unlink = path.clone();
                        body = body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_xs().text_color(muted).child(label))
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
                                if let Some(id) = view.settings_id().cloned() {
                                    if view.selected.as_ref() != Some(&id) {
                                        view.select(id, cx);
                                    }
                                }
                                view.open_panel(Panel::Blame, window, cx)
                            })),
                    );
                body = body
                    .child(
                        Button::new("link-path")
                            .ghost()
                            .disabled(
                                self.operation_busy
                                    || self.settings_current().is_none_or(|p| !p.exists),
                            )
                            .label(self.text("关联目录…", "Link directory…"))
                            .on_click(cx.listener(|view, _, _, cx| view.link_folder(cx))),
                    )
                    .child(
                        Button::new("reveal")
                            .ghost()
                            .disabled(self.settings_current().is_none_or(|p| !p.exists))
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
            .occlude()
            .key_context("CreditiaModal")
            .capture_action(
                cx.listener(|view, _: &gpui_kit::component::input::MoveUp, _, cx| {
                    if view.panel == Some(Panel::Projects) {
                        view.move_picker(-1, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .capture_action(
                cx.listener(|view, _: &gpui_kit::component::input::MoveDown, _, cx| {
                    if view.panel == Some(Panel::Projects) {
                        view.move_picker(1, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
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
                    .test_support()
                    .overflow_y_scroll()
                    .min_h_0()
                    .child(body),
            )
            .focus_trap("creditia-panel", &self.panel_focus);
        let row_menu = self
            .project_row_menu
            .as_ref()
            .filter(|_| panel == Panel::Projects)
            .and_then(|(id, position)| {
                let project = self.projects.iter().find(|p| &p.id == id)?;
                let analyze = id.clone();
                let settings = id.clone();
                let remove = id.clone();
                let path = project.path.clone();
                Some(
                    div()
                        .id("project-row-menu")
                        .test_support()
                        .occlude()
                        .absolute()
                        .left(px(position.0))
                        .top(px(position.1))
                        .w(px(232.))
                        .p_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .rounded_lg()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("row-analyze")
                                .ghost()
                                .label(self.text("重新分析", "Re-analyze"))
                                .disabled(
                                    !project.exists
                                        || self.job_busy
                                        || self.automatic_job.is_some(),
                                )
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.project_row_menu = None;
                                    view.start_job_target(None, false, Some(analyze.clone()), cx)
                                })),
                        )
                        .child(
                            Button::new("row-reveal")
                                .ghost()
                                .label(self.text("打开所在位置", "Reveal folder"))
                                .disabled(!project.exists)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.project_row_menu = None;
                                    cx.reveal_path(Path::new(&path));
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("row-settings")
                                .ghost()
                                .label(self.text("项目设置…", "Project settings…"))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.open_project_settings(settings.clone(), false, window, cx)
                                })),
                        )
                        .child(
                            Button::new("row-remove")
                                .ghost()
                                .label(self.text("从作品库移除…", "Remove from library…"))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.open_project_settings(remove.clone(), true, window, cx)
                                })),
                        ),
                )
            });
        Some(
            div()
                .id("panel-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| {
                        view.drag = None;
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|view, _, window, cx| {
                    if view.project_row_menu.take().is_some() {
                        cx.notify();
                    } else {
                        view.close_panel(window, cx);
                    }
                }))
                .bg(rgba(0x00000066))
                .flex()
                .items_center()
                .justify_center()
                .child(modal)
                .children(row_menu)
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
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let path = path.to_string_lossy().into_owned();
                            view.operation(
                                move || {
                                    service::edit_app_settings(|settings| {
                                        settings["exportDir"] = json!(path)
                                    })?;
                                    Ok(Value::Null)
                                },
                                cx,
                            );
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
}
impl Creditia {
    pub fn diagnostic_state(&self) -> Value {
        let selected = self.current_tile();
        let focused_art = self
            .focused
            .then(|| selected.and_then(|tile| tile.art.as_deref()))
            .flatten();
        let mut state = json!({
            "selected_id":selected.map(|tile|&tile.identity), "art_width":focused_art.map(|art|art.w), "art_height":focused_art.map(|art|art.h), "hit_count":focused_art.map(|art|art.hits.len()),
            "view":self.preferences.view,"focused":self.focused,"group":self.group(),"named":self.preferences.named,"selected_tile":self.selected_tile,"selected_piece":selected.map(|t|&t.spec.piece),"selected_variant":selected.and_then(|t|t.spec.variant.as_deref()),
            "zoom":self.viewport.zoom,"pan_x":self.viewport.x,"pan_y":self.viewport.y,"tile_count":self.tiles.len(),"style":self.style().map(|style|style["id"].clone()),"global_style":self.preferences.style,
            "language":self.preferences.language.code(),"appearance":self.preferences.appearance,"export_open":self.export_open,"preview_generation":self.preview_generation,"preview_busy":self.preview_busy,"error":self.error,
            "reduced_motion":self.reduced_motion,"preview_style":self.focus_styles.as_ref().is_some_and(|styles|styles.current!=styles.original),"playing":self.playback.as_ref().is_some_and(|p|p.playing&&!p.suspended),"loading":self.loading,
            "panel":self.panel.map(|p|match p{Panel::Projects=>"projects",Panel::Styles=>"styles",Panel::Pieces=>"pieces",Panel::Layouts=>"layouts",Panel::Diagnostics=>"diagnostics",Panel::Blame=>"blame",Panel::Exports=>"exports",Panel::Settings=>"settings",Panel::Project=>"project",Panel::Details=>"details"}),
            "project":self.selected,"project_count":self.projects.len(),"project_name":self.raw.as_ref().map(|raw|raw["project"]["name"].clone()),"operation_busy":self.operation_busy,"remove_confirm":self.remove_confirm,"search_query":self.search_query,
            "display_scale":self.display_scale,"picker_selected":self.picker_selected,"settings_project":self.settings_id(),"project_settings_ready":self.settings_data().is_some() && self.sign_loaded_for.as_ref()==self.settings_id(),"project_row_menu":self.project_row_menu.as_ref().map(|(id,_)|id),"hover":self.hover.as_ref().map(|hit|&hit.t),"batch":self.batch_results
        });
        if let Some(hit) = focused_art.and_then(|art| art.hits.first()) {
            state["first_hit"] = json!({"x":hit.x,"y":hit.y,"w":hit.w,"h":hit.h,"t":hit.t});
        }
        if self.focused {
            if let Some(index) = self.selected_tile {
                let r = self.screen_rect(index);
                state["focused_rect"] = json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h});
            }
        }
        if let Some(play) = &self.playback {
            let displayed = play.displayed();
            state["animation"] = json!({"epoch":play.epoch,"displayed_epoch":displayed.map(|f|f.epoch),"index":play.index,"requested":play.requested,"playing":play.playing&&!play.suspended,"intent_playing":play.playing,"paused":!play.playing||play.suspended,"suspended":play.suspended,"frame_pixels":displayed.map(|f|f.pixels),"chunks":displayed.map(|f|&f.regions),"resident_frames":play.frames.len()+usize::from(play.previous.is_some()),"resident_bytes":play.frames.values().map(|f|f.decoded_bytes).sum::<usize>()+play.previous.as_ref().map(|f|f.decoded_bytes).unwrap_or(0),"decode_ready":displayed.is_some_and(|f|f.ready),"elapsed_ms":play.elapsed.as_millis(),"timeline":{"duration_ms":play.timeline.duration_ms,"hold_ms":play.timeline.hold_ms,"fps":play.timeline.fps,"repeat":play.timeline.repeat,"frame_count":play.times.len()},"sample_time":play.times.get(play.index)});
        }
        state
    }
}
impl Render for Creditia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reduced = self.preferences.reduced_motion
            || (!self.fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
        // Kit also observes OS settings. Keep its effective value in sync even
        // when an OS change does not change our app-or-system preference.
        if cx.reduce_motion() != reduced {
            cx.set_reduce_motion(reduced);
        }
        if reduced != self.reduced_motion {
            self.reduced_motion = reduced;
            if reduced {
                self.stop_playback(cx);
                self.focus_transition = None;
                self.panel_tween = None;
                self.dock_tween = None;
                if self.panel_closing {
                    self.panel_closing = false;
                    self.panel = None;
                }
            }
        }

        if self.panel.is_none() && !self.panel_closing {
            if let Some(focus) = self.return_focus.take() {
                focus.focus(window, cx);
            }
        }
        if self.panel == Some(Panel::Project)
            && self.settings_data().is_some()
            && self.sign_loaded_for.as_ref() != self.settings_id()
        {
            let sign = self
                .settings_data()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.settings_id().cloned();
        }
        self.display_scale = window.scale_factor();
        let width: f32 = window.viewport_size().width.into();
        let height: f32 = window.viewport_size().height.into();
        let geometry = (width, height, self.viewport.zoom, self.display_scale);
        if geometry != self.raster_geometry {
            self.raster_geometry = geometry;
            self.raster_changed_at = Instant::now();
        }
        if self.raster_changed_at.elapsed() < Duration::from_millis(120)
            && self.window_visible
            && window.is_window_active()
        {
            window.request_animation_frame();
        }
        let target = if self.export_open { 148. } else { 0. };
        if target != self.dock_target {
            self.dock_target = target;
            self.dock_tween = (!self.reduced_motion).then_some(Tween {
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
        self.sync_activity(cx);
        let delayed_job = (self.job_busy || self.automatic_job.is_some())
            && self.activity_age() >= Duration::from_secs(2);
        let status_height = if self.error.is_some()
            || self.notice.is_some()
            || delayed_job
            || self.export_stage.is_some()
            || self.animation_busy
            || !self.warnings.is_empty()
            || matches!(
                self.pending_analysis["state"].as_str(),
                Some("interrupted" | "failed" | "cancelled")
            ) {
            24.
        } else {
            0.
        };
        if self.reduced_motion || !self.window_visible || !window.is_window_active() {
            self.roam_motion.stop();
        }
        if let Some((x, y)) = self.roam_motion.sample(self.motion_clock.elapsed()) {
            self.viewport.x = x;
            self.viewport.y = y;
            if self.roam_motion.is_active() {
                window.request_animation_frame();
            }
        }
        self.compute_layout(
            width.max(300.),
            (height - 60. - status_height - dock_height).max(200.),
            cx,
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
            background.set_active(window.is_window_active() && self.window_visible);
        }
        self.update_project_mark(cx);
        if self.panel == Some(Panel::Projects) {
            for id in self.nearby_project_marks(cx) {
                if let Some(mark) = self.project_marks.get(&id) {
                    crate::image_assets::track(&mark.image, window, cx);
                }
            }
        }
        self.prepare_static_images(window, cx);
        self.prepare_focus_preview(window, cx);
        let animation_visible = self
            .selected_tile
            .is_some_and(|index| self.screen_rect(index).intersects(self.artwork_viewport()));
        if animation_visible
            && self.focused
            && self.panel.is_none()
            && window.is_window_active()
            && self.window_visible
        {
            self.prepare_playback_images(window, cx);
        }
        if let Some(control) = &self.animation_control {
            control.set_visible(
                animation_visible
                    && self.focused
                    && self.panel.is_none()
                    && window.is_window_active()
                    && self.window_visible,
            );
            control.set_viewport(self.viewport, self.viewport_size, self.display_scale);
        }
        let can_animate = animation_visible
            && self.focused
            && self.panel.is_none()
            && window.is_window_active()
            && self.window_visible
            && !self.reduced_motion;
        if let Some(play) = &mut self.playback {
            if play.playing && play.suspended && can_animate {
                play.started = Instant::now() - play.elapsed;
                play.suspended = false;
                if let Some(control) = &self.animation_control {
                    control.resume();
                }
            }
            if play.playing
                && !play.suspended
                && play.started_once
                && animation_visible
                && self.focused
                && self.panel.is_none()
                && window.is_window_active()
                && self.window_visible
                && !self.reduced_motion
            {
                if let Some(control) = &self.animation_control {
                    if control.epoch() == play.epoch && play.tick(control) {
                        window.request_animation_frame();
                    }
                }
            } else if play.playing
                && (!animation_visible
                    || !window.is_window_active()
                    || !self.window_visible
                    || self.panel.is_some()
                    || self.reduced_motion)
            {
                play.suspended = true;
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
            state["active"] = json!(window.is_window_active() && self.window_visible);
            state["visible"] = json!(self.window_visible);
            state["ready"] = json!(self.ready_emitted);
            let viewport_ready = self.fixture_images_ready(window, cx);
            state["viewport_ready"] = json!(viewport_ready);
            let assets = crate::image_assets::stats(cx);
            state["image_assets"] = json!({"tracked_sources":assets.tracked_sources,"decoded_images":assets.decoded_images,"decoded_bytes":assets.decoded_bytes,"atlas_releases":assets.atlas_releases});
            state["focus_ready"] = json!(self.focused && viewport_ready);
            state["display_scale"] = json!(self.display_scale);
            if self.focused {
                if let Some(index) = self.selected_tile {
                    let r = self.screen_rect(index);
                    state["focused_rect"] = json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h});
                }
            }
            state["viewport"] = json!({"width":self.viewport_size.0,"height":self.viewport_size.1});
            let visible = self.artwork_viewport();
            state["visible_tiles"] = json!(self.tiles.iter().enumerate().filter(|(index,_)| (!self.focused || self.selected_tile==Some(*index)) && self.screen_rect(*index).intersects(visible)).map(|(index,tile)| {
                let r=self.screen_rect(index);
                json!({"piece":tile.spec.piece,"variant":tile.spec.variant,"loading":tile.loading,"error":tile.error,"pixels":tile.pixels,"region":tile.image_region.map(|r|json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h})),"image_ready":tile.image.as_ref().is_some_and(|image|crate::image_assets::track(image,window,cx).is_some()),"rect":{"x":r.x,"y":r.y+60.,"w":r.w,"h":r.h}})
            }).collect::<Vec<_>>());
            if let Some(play) = &self.playback {
                let displayed = play.displayed();
                state["animation"] = json!({"epoch":play.epoch,"displayed_epoch":displayed.map(|f|f.epoch),"index":play.index,"requested":play.requested,"playing":play.playing&&!play.suspended,"intent_playing":play.playing,"paused":!play.playing||play.suspended,"suspended":play.suspended,"visible":can_animate,"frame_pixels":displayed.map(|f|f.pixels),"chunks":displayed.map(|f|&f.regions),"resident_frames":play.frames.len()+usize::from(play.previous.is_some()),"resident_bytes":play.frames.values().map(|f|f.decoded_bytes).sum::<usize>()+play.previous.as_ref().map(|f|f.decoded_bytes).unwrap_or(0),"decode_ready":displayed.is_some_and(|f|f.ready),"elapsed_ms":play.elapsed.as_millis(),"timeline":{"duration_ms":play.timeline.duration_ms,"hold_ms":play.timeline.hold_ms,"fps":play.timeline.fps,"repeat":play.timeline.repeat,"frame_count":play.times.len()},"sample_time":play.times.get(play.index)});
            }
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
            0x1E1E1E8C
        } else {
            0xFFFFFF94
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
        let style_dot = self
            .style()
            .and_then(|style| style["dot"].as_str())
            .map(style_dot);
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
        let group = if group == "auto" {
            self.resolved_group.as_deref().unwrap_or("tool")
        } else {
            group.as_str()
        };
        for (id, label) in [
            ("tool", self.text("按工具", "By tool")),
            ("model", self.text("按模型", "By model")),
        ] {
            if self.has_models == Some(false) {
                continue;
            }
            let element = match id {
                "auto" => "group-auto",
                "tool" => "group-tool",
                _ => "group-model",
            };
            tools = tools.child(
                Button::new(element)
                    .ghost()
                    .label(label)
                    .h(px(30.))
                    .rounded_full()
                    .text_size(px(12.))
                    .when(group == id, |b| b.bg(cx.theme().muted))
                    .on_click(cx.listener(move |view, _, _, cx| view.set_group(id, cx))),
            );
        }
        tools = tools
            .child(
                Button::new("wall")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .rounded_full()
                    .icon(crate::ui_icons::icon("wall"))
                    .accessibility_label(self.text("展墙", "Wall"))
                    .tooltip(self.text("展墙", "Wall"))
                    .when(self.preferences.view == "wall" && !self.focused, |b| {
                        b.bg(cx.theme().muted)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("wall", cx))),
            )
            .child(
                Button::new("roam")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .rounded_full()
                    .icon(crate::ui_icons::icon("roam"))
                    .accessibility_label(self.text("漫游", "Roam"))
                    .tooltip(self.text("漫游", "Roam"))
                    .when(self.preferences.view == "roam" && !self.focused, |b| {
                        b.bg(cx.theme().muted)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("roam", cx))),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .rounded_full()
                    .icon(crate::ui_icons::icon("gear"))
                    .accessibility_label(self.text("设置", "Settings"))
                    .tooltip(self.text("设置", "Settings"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Settings, window, cx)
                    })),
            );
        if !self.export_queue.jobs().is_empty() || self.export_history_error.is_some() {
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
        let mark = if self.preferences.logo == "star" {
            self.live_mark.render(
                self.progress.as_ref().and_then(analysis_fraction),
                self.loading || self.job_busy || self.automatic_job.is_some(),
                self.error.is_some(),
                self.reduced_motion,
                20.,
                window,
                cx,
            )
        } else {
            crate::image_assets::track(&self.logo_image, window, cx);
            img(self.logo_image.clone())
                .w(px(20.))
                .h(px(20.))
                .into_any_element()
        };
        let toolbar = div()
            .h(px(60.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .pl(px(if cfg!(target_os = "macos") { 84. } else { 58. }))
            .pr_4()
            .child(
                Button::new("project-mark")
                    .ghost()
                    .w(px(24.))
                    .h(px(28.))
                    .p_0()
                    .child(mark)
                    .tooltip(
                        if self.loading || self.job_busy || self.automatic_job.is_some() {
                            self.text("分析状态", "Analysis status").to_string()
                        } else {
                            self.mark_tooltip.clone()
                        },
                    )
                    .accessibility_label(self.text("项目贡献份额", "Project contribution shares")),
            )
            .child(
                Button::new("switch-project")
                    .ghost()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(15.))
                    .label(title)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Projects, window, cx)
                    })),
            )
            .child(
                Button::new("project-settings")
                    .ghost()
                    .icon(crate::ui_icons::icon("more"))
                    .accessibility_label(self.text("项目设置", "Project settings"))
                    .tooltip(self.text("项目设置", "Project settings"))
                    .disabled(
                        self.selected.is_none() || self.selected.as_deref() == Some("__all__"),
                    )
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Project, window, cx)
                    })),
            )
            .child(div().flex_1())
            .child(tools);
        let toolbar = if self.focused {
            let title = self
                .current_tile()
                .and_then(|tile| self.piece(&tile.spec.piece))
                .and_then(|piece| {
                    piece[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                })
                .unwrap_or("");
            div()
                .h(px(60.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .pl(px(if cfg!(target_os = "macos") { 94. } else { 60. }))
                .pr_4()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(15.))
                        .child(title.to_string()),
                )
                .child(div().flex_1())
                .child(
                    Button::new("focus-close-top")
                        .ghost()
                        .rounded_full()
                        .bg(glass)
                        .icon(crate::ui_icons::icon("close"))
                        .accessibility_label(self.text("返回展览", "Return to exhibition"))
                        .tooltip(self.text("返回展览", "Return to exhibition"))
                        .on_click(cx.listener(|view, _, _, cx| view.close_focus(cx))),
                )
                .into_any_element()
        } else {
            toolbar.into_any_element()
        };
        let mut controls = div()
            .id("view-controls")
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.drag = None;
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(status_height + dock_height + 16.))
            .h(px(40.))
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
                            .icon(crate::ui_icons::icon("prev"))
                            .accessibility_label(self.text("上一种风格", "Previous style"))
                            .tooltip(self.text("上一种风格", "Previous style"))
                            .on_click(cx.listener(|view, _, _, cx| view.step_style(-1, cx))),
                    )
                    .child(
                        Button::new("styles")
                            .ghost()
                            .when_some(style_dot, |button, dot| {
                                button.child(img(dot).w(px(18.)).h(px(18.)))
                            })
                            .label(style_name)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_panel(Panel::Styles, window, cx)
                            })),
                    )
                    .child(
                        Button::new("style-next")
                            .ghost()
                            .icon(crate::ui_icons::icon("next"))
                            .accessibility_label(self.text("下一种风格", "Next style"))
                            .tooltip(self.text("下一种风格", "Next style"))
                            .on_click(cx.listener(|view, _, _, cx| view.step_style(1, cx))),
                    ),
            )
            .child(
                Button::new("pieces")
                    .ghost()
                    .icon(crate::ui_icons::icon("more"))
                    .accessibility_label(self.text("作品", "Artworks"))
                    .tooltip(self.text("作品", "Artworks"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Pieces, window, cx)
                    })),
            )
            .when(!self.focused, |controls| controls.child(div().flex_1()));
        if self.focused {
            controls = controls
                .justify_center()
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
                        .icon(crate::ui_icons::icon("prev"))
                        .accessibility_label(self.text("上一件作品", "Previous artwork"))
                        .tooltip(self.text("上一件作品", "Previous artwork"))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.keyboard_navigation(false, -1, cx)),
                        ),
                )
                .child(
                    Button::new("next-piece")
                        .ghost()
                        .icon(crate::ui_icons::icon("next"))
                        .accessibility_label(self.text("下一件作品", "Next artwork"))
                        .tooltip(self.text("下一件作品", "Next artwork"))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.keyboard_navigation(false, 1, cx)),
                        ),
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
                        .icon(crate::ui_icons::icon("refresh"))
                        .accessibility_label(self.text("重播", "Replay"))
                        .tooltip(self.text("重播", "Replay"))
                        .disabled(self.animation_busy)
                        .on_click(cx.listener(|view, _, _, cx| view.play(true, cx))),
                )
                .child(
                    Button::new("focus-close")
                        .ghost()
                        .icon(crate::ui_icons::icon("close"))
                        .accessibility_label(self.text("返回展览", "Return to exhibition"))
                        .tooltip(self.text("返回展览", "Return to exhibition"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.close_focus(cx);
                        })),
                );
        }
        if self.focused
            && self
                .focus_styles
                .as_ref()
                .is_some_and(|styles| styles.current != styles.original)
        {
            controls = controls.child(
                Button::new("apply-focus-style")
                    .ghost()
                    .label(self.text("设为全局风格", "Use for all cards"))
                    .on_click(cx.listener(|view, _, _, cx| view.apply_focus_style(cx))),
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
                            view.reset_viewport(cx);
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
            .h(px(status_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_5()
            .text_xs()
            .text_color(muted);
        if let Some(stage) = self.export_stage {
            let label = match stage {
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
        } else if delayed_job {
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
        let stage = self.render_stage(window, cx);
        let export = self.render_export_dock(cx);
        let modal = self.render_panel(cx);
        let ghost = self
            .drag
            .as_ref()
            .filter(|drag| drag.moved)
            .and_then(|drag| {
                let tile = self.tiles.get(drag.tile?)?;
                let image = tile.image.clone()?;
                let art = tile.art.as_ref()?;
                if tile.image_region.is_some() {
                    return None;
                }
                let source = drag.source?;
                let in_dock = self.pointer.1 >= self.viewport_size.1;
                let width = art.w.min(if in_dock { 120. } else { 240. });
                let height = width * art.h / art.w;
                let fx = ((drag.start.0 - source.x) / source.w).clamp(0., 1.);
                let fy = ((drag.start.1 - source.y) / source.h).clamp(0., 1.);
                Some(
                    div()
                        .absolute()
                        .left(px(self.pointer.0 - width * fx))
                        .top(px(60. + self.pointer.1 - height * fy))
                        .w(px(width))
                        .h(px(height))
                        .rounded_lg()
                        .overflow_hidden()
                        .opacity(0.92)
                        .child(img(image).size_full().object_fit(ObjectFit::Contain))
                        .into_any_element(),
                )
            });
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
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
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
                view.reset_viewport(cx);
            }))
            .on_action(cx.listener(|view, _: &Play, _, cx| view.play(false, cx)))
            .on_action(cx.listener(|view, _: &Replay, _, cx| view.play(true, cx)))
            .on_action(
                cx.listener(|view, _: &PreviousLayout, _, cx| {
                    view.keyboard_navigation(true, -1, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &NextLayout, _, cx| view.keyboard_navigation(true, 1, cx)),
            )
            .on_action(
                cx.listener(|view, _: &PreviousPiece, _, cx| {
                    view.keyboard_navigation(false, -1, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &NextPiece, _, cx| view.keyboard_navigation(false, 1, cx)),
            )
            .on_action(cx.listener(|view, _: &FocusArtwork, _, cx| {
                if let Some(index) = view.selected_tile {
                    view.select_tile(index, true, cx);
                }
            }))
            .on_mouse_move(cx.listener(|view, event, _, cx| view.pointer_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, event, window, cx| view.pointer_up(event, window, cx)),
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
                    .child(status),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(80.))
                    .child(
                        img(bottom_fade(cx.theme().is_dark()))
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
            )
            .child(controls)
            .children(ghost)
            .children(modal)
    }
}
fn apply_appearance(appearance: &Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
    Theme::update(cx, |theme| theme.font_family = "Inter".into());
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
                MenuItem::action(language.text("隐藏 Creditia", "Hide Creditia"), HideApp),
                MenuItem::action(language.text("隐藏其他应用", "Hide Others"), HideOthers),
                MenuItem::action(language.text("显示全部", "Show All"), ShowAll),
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
                MenuItem::action(language.text("关闭窗口", "Close Window"), CloseWindow),
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
        ("h", "cmd-h", "ctrl-h"),
        ("alt-h", "cmd-alt-h", "ctrl-alt-h"),
        ("w", "cmd-w", "ctrl-w"),
        ("m", "cmd-m", "ctrl-m"),
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
        ("shift-space", "cmd-shift-space", "ctrl-shift-space"),
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
        KeyBinding::new(shortcut("h"), HideApp, Some("Creditia && !Input")),
        KeyBinding::new(shortcut("alt-h"), HideOthers, Some("Creditia && !Input")),
        KeyBinding::new(shortcut("w"), CloseWindow, Some("Creditia")),
        KeyBinding::new(shortcut("m"), MinimizeWindow, Some("Creditia && !Input")),
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
        KeyBinding::new("home", ResetZoom, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("0", ResetZoom, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("up", PreviousPiece, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("down", NextPiece, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("enter", FocusArtwork, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("space", Play, Some("Creditia && !CreditiaModal")),
        KeyBinding::new(
            shortcut("shift-space"),
            Replay,
            Some("Creditia && !CreditiaModal"),
        ),
        KeyBinding::new("left", PreviousLayout, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("right", NextLayout, Some("Creditia && !CreditiaModal")),
    ]
}
pub fn init_fonts(cx: &mut App) -> Result<(), String> {
    let files = renderer::ui_font_files()?;
    let bytes = files
        .into_iter()
        .map(|path| {
            std::fs::read(&path)
                .map(std::borrow::Cow::Owned)
                .map_err(|error| format!("Cannot load {}: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    cx.text_system()
        .add_fonts(bytes)
        .map_err(|error| error.to_string())?;
    Theme::update(cx, |theme| theme.font_family = "Inter".into());
    Ok(())
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
    let application = application().with_assets(assets::Assets);
    let reopen_fixture = fixture.clone();
    let reopen_benchmark = benchmark.clone();
    application.on_reopen(move |cx| {
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        } else {
            open_creditia_window(
                cx,
                reopen_fixture.clone(),
                reopen_benchmark.clone(),
                acceptance,
                None,
            );
        }
        cx.activate(true);
    });
    application.run(move |cx| {
        init(cx);
        let font_error = init_fonts(cx).err();
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &Quit, cx| {
            service::cancel_analyze();
            cx.quit();
        });
        let closed = cx.on_window_closed(|cx, _| {
            if !cfg!(target_os = "macos") && cx.windows().is_empty() {
                cx.quit();
            }
        });
        cx.set_global(DesktopLifetime { _closed: closed });
        open_creditia_window(cx, fixture, benchmark, acceptance, font_error);
        cx.activate(true);
    });
}

struct DesktopLifetime {
    _closed: Subscription,
}
impl Global for DesktopLifetime {}
fn open_creditia_window(
    cx: &mut App,
    fixture: Option<Fixture>,
    benchmark: Option<Value>,
    acceptance: bool,
    font_error: Option<String>,
) {
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
                let mut view = if let Some(raw) = benchmark {
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
                };
                if let Some(error) = font_error {
                    view.error = Some(error);
                }
                view
            })
        },
    )
    .expect("Unable to open Creditia");
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

fn style_dot(svg: &str) -> Arc<Image> {
    static DOTS: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Image>>>> =
        std::sync::OnceLock::new();
    let mut dots = DOTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    dots.entry(svg.to_string()).or_insert_with(|| Arc::new(Image::from_bytes(ImageFormat::Svg, format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">{svg}</svg>"#).into_bytes()))).clone()
}

fn bottom_fade(dark: bool) -> Arc<Image> {
    static LIGHT: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    static DARK: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    (if dark { &DARK } else { &LIGHT }).get_or_init(|| {
        let color = if dark { "#0A0A0A" } else { "#EFEFED" };
        let svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="80"><defs><linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop stop-color="{color}" stop-opacity="0"/><stop offset="1" stop-color="{color}" stop-opacity=".88"/></linearGradient></defs><path fill="url(#fade)" d="M0 0h4v80H0z"/></svg>"##);
        Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
    }).clone()
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
    use super::{key_bindings, Playback, Timeline};
    use std::{collections::BTreeMap, time::{Duration, Instant}};
    #[test]
    fn slow_paint_cannot_skip_an_entire_animation_cycle() {
        let control = crate::animation::PlaybackControl::new();
        let mut play = Playback {
            frames: BTreeMap::new(), previous: None, times: (0..=20).map(|n| n as f64 / 20.).collect(),
            timeline: Timeline {duration_ms: 1000, hold_ms: 200, fps: 20, repeat: true},
            started: Instant::now() - Duration::from_secs(12), elapsed: Duration::ZERO,
            playing: true, suspended: false, index: 0, requested: 0, epoch: control.epoch(), started_once: true,
        };
        let epoch = control.epoch();
        assert!(!play.tick(&control));
        assert_eq!(play.requested, crate::animation::MAX_AHEAD);
        assert_eq!(control.epoch(), epoch);
        play.index = 20;
        play.started = Instant::now() - Duration::from_millis(1201);
        assert!(!play.tick(&control));
        assert!(!play.started_once);
        assert_ne!(control.epoch(), epoch);
    }
    #[test]
    fn all_shortcuts_parse_on_both_platform_families() {
        for modifier in ["cmd", "ctrl"] {
            assert_eq!(key_bindings(modifier).len(), 30);
        }
    }
}

fn project_date(stamp: i64, language: &Language) -> String {
    let days = stamp.div_euclid(86400) + 719468;
    let era = days.div_euclid(146097);
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if *language == Language::Zh {
        format!("{year}/{month}/{day}")
    } else {
        format!("{month}/{day}/{year}")
    }
}

fn nearest_child(handle: &ScrollHandle, count: usize, x: f32, y: f32) -> Option<usize> {
    let mut nearest = None;
    let mut distance = f32::INFINITY;
    for index in 0..count {
        let Some(bounds) = handle.bounds_for_item(index) else {
            continue;
        };
        let left: f32 = bounds.origin.x.into();
        let top: f32 = bounds.origin.y.into();
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        if x < left - 14. || x > left + width + 14. || y < top - 5. || y > top + height + 5. {
            continue;
        }
        let d = (x - left - width / 2.).abs();
        if d < distance {
            nearest = Some(index);
            distance = d;
        }
    }
    nearest
}
