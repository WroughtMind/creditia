//! Production desktop view. Fixture construction uses the same render tree and
//! event handlers while keeping project data and filesystem services isolated.

mod chrome;
mod export;
mod panels;
mod playback;
mod project_panels;
mod projects;
mod surfaces;
mod wall;
mod wall_images;
mod wall_stage;

pub use chrome::{bind_keys, init_fonts, run};

use crate::material::Palette;
use crate::ui_model::{Artwork, Hit, Rect, TileSpec, Timeline, Viewport};
use crate::{Appearance, Language, Preferences, Project, renderer};
use chrome::{apply_appearance, bottom_fade, install_menus, logo_image, telemetry_enabled};
use creditia_desktop_service as service;
use export::ExportPreview;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
        ExportHistory,
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
    Layouts,
    Pieces,
    Settings,
    Project,
    Details,
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
    index: usize,
    ready: bool,
    art: Arc<Artwork>,
    tiles: Vec<(Option<Rect>, Arc<RenderImage>)>,
    painted: Arc<std::sync::OnceLock<Instant>>,
    pixels: u32,
    decoded_bytes: usize,
    regions: Vec<Value>,
}

struct Playback {
    frames: BTreeMap<usize, PlaybackFrame>,
    previous: Option<PlaybackFrame>,
    times: Vec<f64>,
    timeline: Timeline,
    started: Instant,
    elapsed: Duration,
    playing: bool,
    ended: bool,
    suspended: bool,
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
    settings_tab: &'static str,
    backdrop_pending: Option<crate::material::RasterBackdrop>,
    backdrop_image: Option<Arc<Image>>,
    backdrop_background: Option<Arc<Image>>,
    backdrop_top: Option<Arc<Image>>,
    backdrop_key: Option<u64>,
    backdrop_task: Option<Task<()>>,
    backdrop_error: Option<String>,
    surface_anchors: crate::material::Anchors,
    focus_transition: Option<FocusTransition>,
    focus_styles: Option<FocusStyles>,
    focus_placeholder: Option<Tile>,
    saved_viewport: Option<Viewport>,
    pending_view: Option<ViewBookmark>,
    mode_viewports: BTreeMap<String, Viewport>,
    preferences: Preferences,
    preference_writer: Option<Arc<crate::preference_writer::PreferenceWriter>>,
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
    success_notice: Option<String>,
    success_notice_task: Option<Task<()>>,
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
    failed_export: Option<u64>,
    export_stage: Option<ExportStage>,
    export_cancel_requested: bool,
    export_task: Option<Task<()>>,
    export_cancellation: Option<renderer::Cancellation>,
    export_open: bool,
    export_formats_open: bool,
    export_preview: ExportPreview,
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
        let sign_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(preferences.language.text("署名", "Signature"))
        });
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(preferences.language.text("搜索项目", "Search projects"))
        });
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
            settings_tab: "general",
            backdrop_pending: None,
            backdrop_image: None,
            backdrop_background: None,
            backdrop_top: None,
            backdrop_key: None,
            backdrop_task: None,
            backdrop_error: None,
            surface_anchors: Default::default(),
            focus_transition: None,
            focus_styles: None,
            focus_placeholder: None,
            saved_viewport: None,
            pending_view: None,
            mode_viewports: BTreeMap::new(),
            preference_writer,
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
            success_notice: None,
            success_notice_task: None,
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
            failed_export: None,
            export_stage: None,
            export_cancel_requested: false,
            export_task: None,
            export_cancellation: None,
            export_open: false,
            export_formats_open: false,
            export_preview: ExportPreview::default(),
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
            let jobs = cx
                .background_spawn(async { service::load_export_jobs() })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.load_export_history(jobs, cx);
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
            }
            "exporting-analyzing" => {
                view.export_stage = Some(ExportStage::Rendering);
                view.job_busy = true;
                view.progress = Some(json!({"stage":"logs","done":2,"total":5}));
                view.activity_since = Some(Instant::now() - Duration::from_secs(3));
                if !fixture.message.is_empty() {
                    view.error = Some(fixture.message);
                }
            }
            _ => {}
        }
        view
    }

    pub fn benchmark(
        raw: Value,
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::base(preferences, true, window, cx);
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

    fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.preferences.language.text(zh, en)
    }

    fn cancel_current(&mut self, _: &CancelCurrent, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
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
        } else if self.export_open {
            if self.export_formats_open {
                self.export_formats_open = false;
                cx.notify();
                return;
            }
            self.close_export_workspace(cx);
        } else if self.focused {
            self.close_focus(cx);
        } else if self.export_stage.is_some() {
            self.cancel_export(&CancelExport, window, cx);
        } else {
            self.cancel(&CancelAnalysis, window, cx);
        }
    }

    pub fn diagnostic_state(&self) -> Value {
        let selected = self.current_tile();
        let focused_art = self
            .focused
            .then(|| selected.and_then(|tile| tile.art.as_deref()))
            .flatten();
        let mut state = json!({
            "selected_id": selected.map(|tile| &tile.identity),
            "art_width": focused_art.map(|art| art.w),
            "art_height": focused_art.map(|art| art.h),
            "hit_count": focused_art.map(|art| art.hits.len()),
            "view": self.preferences.view,
            "focused": self.focused,
            "group": self.group(),
            "named": self.preferences.named,
            "selected_tile": self.selected_tile,
            "selected_piece": selected.map(|t| &t.spec.piece),
            "selected_variant": selected.and_then(|t| t.spec.variant.as_deref()),
            "zoom": self.viewport.zoom,
            "pan_x": self.viewport.x,
            "pan_y": self.viewport.y,
            "tile_count": self.tiles.len(),
            "style": self.style().map(|style| style["id"].clone()),
            "global_style": self.preferences.style,
            "language": self.preferences.language.code(),
            "appearance": self.preferences.appearance,
            "export_open": self.export_open,
            "preview_generation": self.preview_generation,
            "preview_busy": self.preview_busy,
            "error": self.error,
            "reduced_motion": self.reduced_motion,
            "preview_style": self.focus_styles.as_ref().is_some_and(|styles| styles.current != styles.original),
            "playing": self.playback.as_ref().is_some_and(|p| p.playing && !p.suspended),
            "loading": self.loading,
            "panel": self.panel.map(|p| match p {
                Panel::Projects => "projects",
                Panel::Styles => "styles",
                Panel::Layouts => "layouts",
                Panel::Pieces => "pieces",
                Panel::Blame => "blame",
                Panel::Exports => "exports",
                Panel::Settings => "settings",
                Panel::Project => "project",
                Panel::Details => "details"
            }),
            "project": self.selected,
            "project_count": self.projects.len(),
            "project_name": self.raw.as_ref().map(|raw| raw["project"]["name"].clone()),
            "operation_busy": self.operation_busy,
            "remove_confirm": self.remove_confirm,
            "search_query": self.search_query,
            "display_scale": self.display_scale,
            "picker_selected": self.picker_selected,
            "settings_project": self.settings_id(),
            "project_settings_ready": self.settings_data().is_some() && self.sign_loaded_for.as_ref() == self.settings_id(),
            "project_row_menu": self.project_row_menu.as_ref().map(|(id, _)| id),
            "hover": self.hover.as_ref().map(|hit| &hit.t),
            "batch": self.batch_results
        });
        state["animation_busy"] = json!(self.animation_busy);
        state["export_format"] = json!(self.export_format);
        state["panel_closing"] = json!(self.panel_closing);
        state["drag_active"] = json!(self.drag.is_some());
        state["export_size"] = json!(self.export_size);
        state["export_prompt_busy"] = json!(self.export_prompt_busy);
        state["export_workspace"] = json!(self.export_workspace_open());
        state["export_formats_open"] = json!(self.export_formats_open);
        state["failed_export"] = json!(self.failed_export);
        state["success_notice"] = json!(self.success_notice);
        state["notice"] = json!(self.notice);
        state["export_preview"] = self.export_preview.diagnostic();
        state["can_export"] = json!(self.can_export());
        state["export_unavailable"] = json!(self.export_unavailable_reason(&self.export_format));
        if let Some(hit) = focused_art.and_then(|art| art.hits.first()) {
            state["first_hit"] = json!({"x":hit.x,"y":hit.y,"w":hit.w,"h":hit.h,"t":hit.t});
        }
        if let Some(index) = self.selected_tile {
            let rect = self.screen_rect(index);
            state["selected_rect"] = json!({"x":rect.x,"y":rect.y,"w":rect.w,"h":rect.h});
        }
        if self.focused
            && let Some(index) = self.selected_tile
        {
            let r = self.screen_rect(index);
            state["focused_rect"] = json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h});
            if let Some(tile) = selected {
                state["focused_preview"] = json!({"pixels":tile.pixels,"region":tile.image_region.map(|r| json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h}))});
            }
        }
        if let Some(play) = &self.playback {
            let displayed = play.displayed();
            state["animation"] = json!({
                "epoch": play.epoch,
                "displayed_epoch": displayed.map(|f| f.epoch),
                "index": play.index,
                "requested": play.requested,
                "playing": play.playing && !play.suspended,
                "intent_playing": play.playing,
                "ended": play.ended,
                "paused": !play.playing || play.suspended,
                "suspended": play.suspended,
                "frame_pixels": displayed.map(|f| f.pixels),
                "chunks": displayed.map(|f| &f.regions),
                "resident_frames": play.frames.len() + usize::from(play.previous.is_some()),
                "resident_bytes": play.frames.values().map(|f| f.decoded_bytes).sum::<usize>()
                    + play.previous.as_ref().map(|f| f.decoded_bytes).unwrap_or(0),
                "decode_ready": displayed.is_some_and(|f| f.ready),
                "painted": displayed.is_some_and(|f| f.painted.get().is_some()),
                "elapsed_ms": play.elapsed.as_millis(),
                "timeline": {
                    "duration_ms": play.timeline.duration_ms,
                    "hold_ms": play.timeline.hold_ms,
                    "fps": play.timeline.fps,
                    "repeat": play.timeline.repeat,
                    "frame_count": play.times.len()
                },
                "sample_time": play.times.get(play.index)
            });
        }
        state
    }

    fn report_render_state(
        &mut self,
        width: f32,
        height: f32,
        can_animate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if telemetry_enabled() && !self.ready_emitted && self.fixture_images_ready(window, cx) {
            self.ready_emitted = true;
            write_performance_event(json!({
                "event": "ready",
                "width": width,
                "height": height,
                "tiles": self.tiles.len(),
                "loaded": self.tiles.iter().filter(|t| t.image.is_some()).count(),
                "state": self.diagnostic_state()
            }));
        }

        if telemetry_enabled() {
            let mut state = self.diagnostic_state();
            state["active"] = json!(window.is_window_active() && self.window_visible);
            state["visible"] = json!(self.window_visible);
            state["ready"] = json!(self.ready_emitted);
            let viewport_ready = self.fixture_images_ready(window, cx);
            state["viewport_ready"] = json!(viewport_ready);
            let assets = crate::image_assets::stats(cx);
            state["image_assets"] = json!({
                "tracked_sources": assets.tracked_sources,
                "decoded_images": assets.decoded_images,
                "decoded_bytes": assets.decoded_bytes,
                "atlas_releases": assets.atlas_releases
            });
            state["focus_ready"] = json!(self.focused && viewport_ready);
            state["display_scale"] = json!(self.display_scale);
            if self.focused
                && let Some(index) = self.selected_tile
            {
                let r = self.screen_rect(index);
                state["focused_rect"] = json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h});
            }
            state["viewport"] = json!({"width":self.viewport_size.0,"height":self.viewport_size.1});
            let visible = self.artwork_viewport();
            state["visible_tiles"] = json!(self
                .tiles
                .iter()
                .enumerate()
                .filter(|(index, _)| (!self.focused || self.selected_tile == Some(*index))
                    && self.screen_rect(*index).intersects(visible))
                .map(|(index, tile)| {
                    let r = self.screen_rect(index);
                    json!({
                        "piece": tile.spec.piece,
                        "variant": tile.spec.variant,
                        "loading": tile.loading,
                        "error": tile.error,
                        "pixels": tile.pixels,
                        "region": tile.image_region.map(|r| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h})),
                        "image_ready": tile.image.as_ref().is_some_and(|image| crate::image_assets::track(image, window, cx).is_some()),
                        "rect": {"x": r.x, "y": r.y + 60., "w": r.w, "h": r.h}
                    })
                })
                .collect::<Vec<_>>());
            if let Some(play) = &self.playback {
                let displayed = play.displayed();
                state["animation"] = json!({
                    "epoch": play.epoch,
                    "displayed_epoch": displayed.map(|f| f.epoch),
                    "index": play.index,
                    "requested": play.requested,
                    "playing": play.playing && !play.suspended,
                    "intent_playing": play.playing,
                    "ended": play.ended,
                    "paused": !play.playing || play.suspended,
                    "suspended": play.suspended,
                    "visible": can_animate,
                    "frame_pixels": displayed.map(|f| f.pixels),
                    "chunks": displayed.map(|f| &f.regions),
                    "resident_frames": play.frames.len() + usize::from(play.previous.is_some()),
                    "resident_bytes": play.frames.values().map(|f| f.decoded_bytes).sum::<usize>()
                        + play.previous.as_ref().map(|f| f.decoded_bytes).unwrap_or(0),
                    "decode_ready": displayed.is_some_and(|f| f.ready),
                    "painted": displayed.is_some_and(|f| f.painted.get().is_some()),
                    "elapsed_ms": play.elapsed.as_millis(),
                    "timeline": {
                        "duration_ms": play.timeline.duration_ms,
                        "hold_ms": play.timeline.hold_ms,
                        "fps": play.timeline.fps,
                        "repeat": play.timeline.repeat,
                        "frame_count": play.times.len()
                    },
                    "sample_time": play.times.get(play.index)
                });
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

impl Render for Creditia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_reduced_motion(cx);
        self.sync_panel_focus(window, cx);
        let (width, height) = self.sync_raster_geometry(window);
        self.animate_panel(window, cx);
        self.animate_focus_transition(window);
        self.sync_activity(cx);
        let delayed_job = (self.job_busy || self.automatic_job.is_some())
            && self.activity_age() >= Duration::from_secs(2);
        self.animate_roam(window);
        self.compute_layout(width.max(300.), (height - 60.).max(1.), cx);
        self.update_platform_chrome(window);
        if let Some(background) = &self.background {
            background.set_active(window.is_window_active() && self.window_visible);
        }
        self.update_project_mark(cx);
        self.track_project_marks(window, cx);
        self.prepare_static_images(window, cx);
        self.prepare_focus_preview(window, cx);
        self.refresh_export_preview(window, cx);
        let can_animate = self.update_playback(window, cx);

        self.schedule_visible(window.scale_factor(), cx);
        self.report_render_state(width, height, can_animate, window, cx);

        let palette = Palette::of(cx);
        let bg = palette.bg;
        let fg = palette.ink;
        let (toolbar, controls) = self.render_chrome(palette, delayed_job, window, cx);
        self.refresh_backdrop(window, cx);
        let in_export = self.export_workspace_open();
        let stage = self.render_stage(window, cx);
        let export = self.render_export_dock(cx);
        let general_feedback = self.general_feedback(cx);
        let export_feedback = if in_export {
            None
        } else {
            self.export_feedback(cx)
        };

        let modal = self.render_panel(cx);
        let ghost = self.render_drag_ghost();
        div()
            .relative()
            .size_full()
            .flex()
            .bg(bg)
            .text_color(fg)
            .text_size(px(13.))
            .line_height(relative(1.45))
            .track_focus(&self.focus)
            .key_context(if in_export {
                "CreditiaExport"
            } else {
                "Creditia"
            })
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
                if let Some(index) = view.selected_tile {
                    view.toggle_export(index, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|view, _: &ExportHistory, window, cx| {
                view.open_panel(Panel::Exports, window, cx);
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
            .child(div().absolute().inset_0().child(stage))
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
            .when(
                !self.focused && (self.viewport.y < 0. || self.preferences.view == "roam"),
                |root| {
                    root.children(self.backdrop_top.clone().map(|image| {
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(88.))
                            .child(img(image).size_full().object_fit(ObjectFit::Fill))
                    }))
                },
            )
            .when(true, |root| {
                root.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|view, _, _, cx| {
                                view.drag = None;
                                cx.stop_propagation();
                            }),
                        )
                        .child(toolbar),
                )
            })
            .when(!in_export, |root| root.child(controls))
            .child(export)
            .child(
                div()
                    .absolute()
                    .top(px(70.))
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .children(general_feedback)
                    .children(export_feedback),
            )
            .children(ghost)
            .children(modal)
    }
}

pub(crate) fn write_performance_event(mut event: Value) {
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

#[cfg(test)]
mod input_tests {
    use super::chrome::key_bindings;
    use super::{Artwork, Playback, PlaybackFrame, Timeline};
    use std::{
        collections::BTreeMap,
        sync::{Arc, OnceLock},
        time::{Duration, Instant},
    };
    #[test]
    fn preview_completes_once_after_the_final_frame_has_painted() {
        let control = crate::animation::PlaybackControl::new();
        let mut play = Playback {
            frames: BTreeMap::new(),
            previous: None,
            times: (0..=20).map(|n| n as f64 / 20.).collect(),
            timeline: Timeline {
                duration_ms: 1000,
                hold_ms: 200,
                fps: 20,
                repeat: true,
            },
            started: Instant::now() - Duration::from_secs(12),
            elapsed: Duration::ZERO,
            playing: true,
            ended: false,
            suspended: false,
            index: 0,
            requested: 0,
            epoch: control.epoch(),
            started_once: true,
        };
        let epoch = control.epoch();
        assert!(!play.tick(&control));
        assert_eq!(
            play.requested, 20,
            "late rendering requests the current time instead of slowing the clock"
        );
        assert_eq!(control.epoch(), epoch);
        play.frames.insert(
            20,
            PlaybackFrame {
                epoch,
                index: 20,
                ready: true,
                art: Arc::new(Artwork {
                    w: 1.,
                    h: 1.,
                    svg: String::new(),
                    hits: vec![],
                    title: String::new(),
                    bg: String::new(),
                    motion: None,
                    font_faces: vec![],
                }),
                tiles: vec![],
                pixels: 1,
                decoded_bytes: 4,
                regions: vec![],
                painted: Arc::new(OnceLock::new()),
            },
        );
        play.started = Instant::now() - Duration::from_millis(1201);
        assert!(
            play.tick(&control),
            "last frame must paint before completing"
        );
        assert_eq!(control.epoch(), epoch);
        play.frames[&20]
            .painted
            .set(Instant::now() - Duration::from_millis(201))
            .unwrap();
        assert!(!play.tick(&control));
        assert!(!play.playing);
        assert!(play.ended);
        assert_eq!(play.index, 20);
        assert_eq!(
            control.epoch(),
            epoch,
            "completion never restarts the producer"
        );
    }
    #[test]
    fn all_shortcuts_parse_on_both_platform_families() {
        for modifier in ["cmd", "ctrl"] {
            assert_eq!(key_bindings(modifier).len(), 34);
        }
    }
}
