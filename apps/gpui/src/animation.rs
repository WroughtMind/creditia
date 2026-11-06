//! Incremental artwork playback. The shared engine owns time sampling; this
//! producer prepares only the current frame and a small bounded look-ahead.
use crate::{
    native_preview::{self, RasterImage},
    renderer,
    ui_model::{Artwork, Rect, Timeline, Viewport, focused_rect, texture_width},
};
use serde_json::{Value, json};
use std::sync::{Arc, Condvar, Mutex};

pub const MAX_AHEAD: usize = 6;
pub const MAX_RESIDENT_FRAMES: usize = 8;
pub const DECODED_FRAME_BUDGET: u64 = 8 * 1024 * 1024;
pub const DECODED_RESIDENT_BUDGET: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameView {
    viewport: Viewport,
    size: (f32, f32),
    dpi: f32,
}
struct Position {
    playhead: usize,
    epoch: u64,
    paused: bool,
    visible: bool,
    view: Option<FrameView>,
    ahead: usize,
}
impl Default for Position {
    fn default() -> Self {
        Self {
            playhead: 0,
            epoch: 0,
            paused: false,
            visible: true,
            view: None,
            ahead: MAX_AHEAD,
        }
    }
}
struct Shared {
    position: Mutex<Position>,
    changed: Condvar,
    cancellation: renderer::Cancellation,
}

#[derive(Clone)]
pub struct PlaybackControl(Arc<Shared>);
impl Default for PlaybackControl {
    fn default() -> Self {
        Self::new()
    }
}
impl PlaybackControl {
    pub fn new() -> Self {
        Self(Arc::new(Shared {
            position: Mutex::new(Position::default()),
            changed: Condvar::new(),
            cancellation: renderer::Cancellation::default(),
        }))
    }
    pub fn cancellation(&self) -> renderer::Cancellation {
        self.0.cancellation.clone()
    }
    pub fn cancelled(&self) -> bool {
        self.0.cancellation.cancelled()
    }
    pub fn epoch(&self) -> u64 {
        self.0
            .position
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .epoch
    }
    /// Latest focus geometry in logical pixels. Configuration changes invalidate
    /// old frames, including while user-paused; unchanged draws do no work.
    pub fn set_viewport(&self, viewport: Viewport, size: (f32, f32), dpi: f32) {
        if ![viewport.x, viewport.y, viewport.zoom, size.0, size.1, dpi]
            .iter()
            .all(|value| value.is_finite())
            || viewport.zoom <= 0.
            || size.0 <= 0.
            || size.1 <= 0.
            || dpi <= 0.
        {
            return;
        }
        let view = FrameView {
            viewport,
            size,
            dpi,
        };
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        if position.view == Some(view) {
            return;
        }
        position.view = Some(view);
        position.epoch = position.epoch.wrapping_add(1);
        self.0.changed.notify_all();
    }
    /// Hiding differs from a user pause: hidden views do not even rerasterize a
    /// new zoom/DPI configuration. Returning wakes only the latest request.
    pub fn set_visible(&self, visible: bool) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        if position.visible != visible {
            position.visible = visible;
            // A frame may have been discarded while hidden before it ever
            // reached the view. Reappearance must refresh even when paused.
            if visible {
                position.epoch = position.epoch.wrapping_add(1);
            }
            self.0.changed.notify_all();
        }
    }
    pub fn pause(&self) {
        self.0
            .position
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paused = true;
    }
    pub fn resume(&self) {
        self.0
            .position
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paused = false;
        self.0.changed.notify_all();
    }
    /// Always cancel through this control so a paused producer wakes as well.
    pub fn cancel(&self) {
        self.0.cancellation.cancel();
        self.0.changed.notify_all();
    }
    pub fn set_playhead(&self, index: usize) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        if position.playhead == index {
            return;
        }
        if index < position.playhead || position.paused {
            position.epoch = position.epoch.wrapping_add(1);
        }
        position.playhead = index;
        self.0.changed.notify_all();
    }
    pub fn restart(&self) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        position.epoch = position.epoch.wrapping_add(1);
        position.playhead = 0;
        position.paused = false;
        self.0.changed.notify_all();
    }
    fn plan(&self, epoch: u64, next: usize, last: usize) -> Result<(u64, usize), String> {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if self.cancelled() {
                return Err("Animation cancelled".into());
            }
            if position.visible && (!position.paused || position.epoch != epoch) {
                let current = position.playhead.min(last);
                let candidate = if position.epoch != epoch {
                    current
                } else {
                    next.max(current)
                };
                let ahead = if position.paused { 0 } else { position.ahead };
                if candidate <= current.saturating_add(ahead).min(last) {
                    return Ok((position.epoch, candidate));
                }
            }
            position = self
                .0
                .changed
                .wait(position)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
    fn initial_plan(&self) -> Result<(u64, usize), String> {
        // A newly opened player still needs one visible frame if it was paused
        // before metadata arrived. Visibility and cancellation continue to gate
        // this work; only the first-frame pause restriction is bypassed.
        self.plan(self.epoch().wrapping_sub(1), 0, 0)
    }
    fn current(&self, epoch: u64) -> Result<bool, String> {
        if self.cancelled() {
            return Err("Animation cancelled".into());
        }
        let position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        Ok(position.epoch == epoch && position.visible)
    }
    fn view(&self) -> Option<FrameView> {
        self.0
            .position
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .view
    }
    fn set_resident_limit(&self, epoch: u64, resident: usize) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        if position.epoch == epoch {
            position.ahead = resident.saturating_sub(1).min(MAX_AHEAD);
        }
    }
}

pub struct FrameTile {
    pub region: Option<Rect>,
    pub raster: Arc<RasterImage>,
}

pub enum Event {
    Started {
        epoch: u64,
        timeline: Timeline,
        times: Vec<f64>,
    },
    Frame {
        epoch: u64,
        index: usize,
        art: Arc<Artwork>,
        tiles: Vec<FrameTile>,
        /// Physical pixel width of the equivalent full artwork, even when only
        /// its visible viewport was rasterized.
        pixels: u32,
        decoded_bytes: usize,
        max_resident: usize,
    },
}

/// Start on a background executor. `send` should feed a bounded channel (2–3
/// events) and return an error when the view is gone. The UI keeps at most eight
/// decoded frames, updates the playhead, and drops images outside its window.
/// This routine stays asleep after the final frame, ready for a replay/loop.
pub fn run(
    mut request: Value,
    pixel_width: u32,
    control: PlaybackControl,
    send: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    let raw = request
        .as_object_mut()
        .and_then(|object| object.remove("raw"));
    let archive = if let Some(raw) = raw {
        let projects = request
            .as_object_mut()
            .and_then(|object| object.remove("projects"))
            .map(serde_json::from_value::<Vec<Value>>)
            .transpose()
            .map_err(|error| format!("Invalid animation project context: {error}"))?
            .unwrap_or_default();
        Some(renderer::RenderArchive::with_projects(
            Arc::new(raw),
            Arc::new(projects),
        )?)
    } else {
        None
    };
    run_internal(request, archive, pixel_width, control, send)
}
pub fn run_with_archive(
    request: Value,
    archive: renderer::RenderArchive,
    pixel_width: u32,
    control: PlaybackControl,
    send: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    if request.get("raw").is_some() || request.get("archiveId").is_some() {
        return Err("Animation archive handles cannot be combined with raw/archiveId".into());
    }
    run_internal(request, Some(archive), pixel_width, control, send)
}
fn render_frame(
    request: &Value,
    archive: Option<&renderer::RenderArchive>,
    control: &PlaybackControl,
) -> Result<Value, String> {
    match archive {
        Some(archive) => renderer::request_with_archive(
            request.clone(),
            archive.clone(),
            control.cancellation(),
            0,
        ),
        None => renderer::request_with_priority(request.clone(), control.cancellation(), 0),
    }
}
fn run_internal(
    mut request: Value,
    archive: Option<renderer::RenderArchive>,
    pixel_width: u32,
    control: PlaybackControl,
    mut send: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    let object = request
        .as_object_mut()
        .ok_or("Animation request must be an object")?;
    object.remove("frameCount");
    object.insert("op".into(), json!("render"));
    object.insert("flatten".into(), json!(true));
    let first_time = if request["reducedMotion"] == true {
        1.0
    } else {
        0.0
    };
    request["t"] = json!(first_time);
    // A replay while the first request is in flight still needs its metadata.
    // Frame zero is identical across playback epochs, so adopt the latest epoch
    // after preparation instead of abandoning the producer before Started.
    control.initial_plan()?;
    let result = render_frame(&request, archive.as_ref(), &control)?;
    let first: Artwork = serde_json::from_value(result["art"].clone())
        .map_err(|e| format!("Invalid first animation frame: {e}"))?;
    let timeline: Timeline = serde_json::from_value(result["timeline"].clone())
        .map_err(|e| format!("Invalid animation timeline: {e}"))?;
    let times = playback_times(&timeline);
    let first_epoch = control.initial_plan()?.0;
    send(Event::Started {
        epoch: first_epoch,
        timeline: timeline.clone(),
        times: times.clone(),
    })?;
    let mut first = Some(Arc::new(first));
    // Prepare one initial frame even if the user pauses immediately after
    // Started. Subsequent same-epoch work obeys the paused look-ahead limit.
    let mut epoch = first_epoch.wrapping_sub(1);
    let mut next = 0;
    let mut layers = crate::animation_layers::LayerCache::default();
    loop {
        let (wanted_epoch, index) = control.plan(epoch, next, times.len() - 1)?;
        epoch = wanted_epoch;
        let view = control.view();
        let art = if index == 0 && first.is_some() {
            first.take().unwrap()
        } else {
            request["t"] = json!(times[index]);
            let result = render_frame(&request, archive.as_ref(), &control)?;
            Arc::new(
                serde_json::from_value::<Artwork>(result["art"].clone())
                    .map_err(|e| format!("Invalid animation frame: {e}"))?,
            )
        };
        if !control.current(epoch)? {
            continue;
        }
        let Some(frame) = raster_frame(&art, pixel_width, view, &control, epoch, &mut layers)? else {
            continue;
        };
        if control.current(epoch)? {
            control.set_resident_limit(epoch, frame.max_resident);
            send(Event::Frame {
                epoch,
                index,
                art,
                tiles: frame.tiles,
                pixels: frame.pixels,
                decoded_bytes: frame.decoded_bytes,
                max_resident: frame.max_resident,
            })?;
        }
        next = index + 1;
    }
}

pub fn playback_times(timeline: &Timeline) -> Vec<f64> {
    if timeline.duration_ms == 0 {
        return vec![1.0];
    }
    // Native playback can sample more densely than GIF without changing the
    // action definition. Long clips remain bounded to 120 indexed samples.
    let steps = (timeline.duration_ms.saturating_mul(60).div_ceil(1000)).clamp(1, 119) as usize;
    (0..=steps)
        .map(|index| index as f64 / steps as f64)
        .collect()
}

pub fn frame_width(width: f32, height: f32, requested: u32) -> Result<u32, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 || requested == 0
    {
        return Err("Invalid animation dimensions".into());
    }
    let ratio = f64::from(height) / f64::from(width);
    let max_width = ((DECODED_FRAME_BUDGET as f64 / 4.0 / ratio).sqrt().floor() as u32).min(32768);
    let mut pixels = requested.min(max_width);
    while pixels > 0 {
        let h = (f64::from(pixels) * ratio).ceil();
        if h <= 32768.0 && f64::from(pixels) * h * 4.0 <= DECODED_FRAME_BUDGET as f64 {
            return Ok(pixels);
        }
        pixels -= 1;
    }
    Err("This animation frame is too tall for a bounded preview; use its scrolling layout".into())
}
#[derive(Clone, Debug)]
struct TilePlan {
    region: Option<Rect>,
    width: u32,
    height: u32,
}
struct FramePlan {
    tiles: Vec<TilePlan>,
    pixels: u32,
    decoded_bytes: usize,
    max_resident: usize,
}
struct PreparedFrame {
    tiles: Vec<FrameTile>,
    pixels: u32,
    decoded_bytes: usize,
    max_resident: usize,
}

fn frame_plan(
    art: &Artwork,
    requested: u32,
    view: Option<FrameView>,
) -> Result<Option<FramePlan>, String> {
    if ![art.w, art.h].iter().all(|v| v.is_finite() && *v > 0.) {
        return Err("Invalid animation dimensions".into());
    }
    let Some(view) = view else {
        // Compatibility for non-window callers. Production always supplies its
        // viewport and never downsamples a large entire artwork to this width.
        let width = frame_width(art.w, art.h, requested)?;
        let height = (f64::from(art.h) * f64::from(width) / f64::from(art.w)).ceil() as u32;
        let bytes = width as usize * height as usize * 4;
        return Ok(Some(FramePlan {
            tiles: vec![TilePlan {
                region: None,
                width,
                height,
            }],
            pixels: width,
            decoded_bytes: bytes,
            max_resident: resident_limit(bytes),
        }));
    };
    let rect = focused_rect(art.w, art.h, view.viewport, view.size);
    if rect.w <= 0.
        || rect.h <= 0.
        || !rect.intersects(Rect {
            x: 0.,
            y: 0.,
            w: view.size.0,
            h: view.size.1,
        })
    {
        return Ok(None);
    }
    let physical_width = f64::from(rect.w) * f64::from(view.dpi);
    if !physical_width.is_finite() || physical_width < 1. || physical_width > f64::from(u32::MAX) {
        return Err("Animation zoom resolution is out of range".into());
    }
    // Share rounding with the image's final physical paint bounds, including
    // fractional desktop scales and logical -> physical -> logical round trips.
    let pixels = texture_width(rect.w, view.dpi);
    let full_width = f64::from(pixels);
    let scale = full_width / f64::from(art.w);
    let logical_scale = f64::from(rect.w) / f64::from(art.w);
    let full_height = (f64::from(art.h) * scale).ceil();
    let x0 = ((-f64::from(rect.x) / logical_scale).max(0.) * scale)
        .floor()
        .clamp(0., full_width) as u64;
    let y0 = ((-f64::from(rect.y) / logical_scale).max(0.) * scale)
        .floor()
        .clamp(0., full_height) as u64;
    let x1 = (((f64::from(view.size.0) - f64::from(rect.x)) / logical_scale).min(f64::from(art.w))
        * scale)
        .ceil()
        .clamp(0., full_width) as u64;
    let y1 = (((f64::from(view.size.1) - f64::from(rect.y)) / logical_scale).min(f64::from(art.h))
        * scale)
        .ceil()
        .clamp(0., full_height) as u64;
    let width = x1.saturating_sub(x0);
    let height = y1.saturating_sub(y0);
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Animation viewport dimensions overflow")?;
    if bytes > DECODED_RESIDENT_BUDGET {
        return Err("Animation viewport exceeds the 64 MiB native pixel budget; use a smaller window or display scale".into());
    }
    let mut tiles = Vec::new();
    let mut x = x0;
    while x < x1 {
        let tile_width = (x1 - x).min(8192) as u32;
        let max_height = (DECODED_FRAME_BUDGET / 4 / u64::from(tile_width))
            .min(32768)
            .max(1);
        let mut y = y0;
        while y < y1 {
            let tile_height = (y1 - y).min(max_height) as u32;
            let region = if x == 0
                && y == 0
                && u64::from(tile_width) == pixels as u64
                && f64::from(tile_height) == full_height
            {
                None
            } else {
                Some(Rect {
                    x: (x as f64 / scale) as f32,
                    y: (y as f64 / scale) as f32,
                    w: (f64::from(tile_width) / scale) as f32,
                    h: (f64::from(tile_height) / scale) as f32,
                })
            };
            tiles.push(TilePlan {
                region,
                width: tile_width,
                height: tile_height,
            });
            y += u64::from(tile_height);
        }
        x += u64::from(tile_width);
    }
    Ok(Some(FramePlan {
        tiles,
        pixels,
        decoded_bytes: bytes as usize,
        max_resident: resident_limit(bytes as usize),
    }))
}
fn resident_limit(decoded_bytes: usize) -> usize {
    (DECODED_RESIDENT_BUDGET / decoded_bytes.max(1) as u64).clamp(1, MAX_RESIDENT_FRAMES as u64)
        as usize
}
fn raster_frame(
    art: &Artwork,
    requested: u32,
    view: Option<FrameView>,
    control: &PlaybackControl,
    epoch: u64,
    layers: &mut crate::animation_layers::LayerCache,
) -> Result<Option<PreparedFrame>, String> {
    let Some(plan) = frame_plan(art, requested, view)? else {
        control.set_visible(false);
        return Ok(None);
    };
    let mut tiles = Vec::with_capacity(plan.tiles.len());
    for tile in plan.tiles {
        if !control.current(epoch)? {
            return Ok(None);
        }
        let raster = native_preview::rasterize_animation(&art.svg, tile.width, tile.height,
            tile.region.map(|r| (r.x,r.y,r.w,r.h)), &control.cancellation(), layers)?;
        if raster.width as u64 * raster.height as u64 * 4 > DECODED_FRAME_BUDGET {
            return Err("Animation tile exceeded its decoded pixel budget".into());
        }
        tiles.push(FrameTile {
            region: tile.region,
            raster,
        });
    }
    Ok(Some(PreparedFrame {
        tiles,
        pixels: plan.pixels,
        decoded_bytes: plan.decoded_bytes,
        max_resident: plan.max_resident,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    #[test]
    fn tall_frames_never_escape_the_decoded_budget_via_a_minimum_width() {
        for (width, height) in [(1080., 720.), (100., 30000.), (1080., 1000000.)] {
            let pixels = frame_width(width, height, 4096).unwrap();
            let h = (f64::from(pixels) * f64::from(height) / f64::from(width)).ceil();
            assert!(f64::from(pixels) * h * 4. <= DECODED_FRAME_BUDGET as f64);
            assert!(h <= 32768.);
        }
        assert!(frame_width(0., 200., 128).is_err());
        assert!(frame_width(1., 100000000., 128).is_err());
    }
    #[test]
    fn lookahead_waits_and_cancellation_wakes_a_paused_producer() {
        let control = PlaybackControl::new();
        assert_eq!(control.plan(0, 6, 119).unwrap(), (0, 6));
        control.pause();
        let background = control.clone();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            tx.send(background.plan(0, 7, 119)).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        control.cancel();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap_err(),
            "Animation cancelled"
        );
        worker.join().unwrap();
    }
    #[test]
    fn first_frame_preparation_obeys_visibility_but_survives_an_early_pause() {
        let control = PlaybackControl::new();
        control.pause();
        assert_eq!(control.initial_plan().unwrap(), (0, 0));
        control.set_visible(false);
        let background = control.clone();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            tx.send(background.initial_plan()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        control.set_visible(true);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap().unwrap(),
            (control.epoch(), 0)
        );
        worker.join().unwrap();
    }
    #[test]
    fn seek_and_loop_reset_the_epoch_and_skip_obsolete_frames() {
        let control = PlaybackControl::new();
        control.set_playhead(20);
        assert_eq!(control.plan(0, 3, 119).unwrap(), (0, 20));
        control.set_playhead(0);
        assert_eq!(control.plan(0, 21, 119).unwrap(), (1, 0));
        control.restart();
        assert_eq!(control.epoch(), 2);
        assert!(!control.current(1).unwrap());
        assert_eq!(
            control.plan(0, 0, 0).unwrap(),
            (2, 0),
            "a replay during initial preparation adopts its epoch"
        );
    }
    #[test]
    fn sampled_times_include_both_endpoints_and_remain_bounded() {
        let timeline = Timeline {
            duration_ms: 5000,
            hold_ms: 1000,
            fps: 20,
            repeat: true,
        };
        let times = playback_times(&timeline);
        assert_eq!(times.len(), 120);
        assert_eq!(times.first(), Some(&0.));
        assert_eq!(times.last(), Some(&1.));
        assert_eq!(playback_times(&Timeline::default()), vec![1.]);
    }

    fn artwork(w: f32, h: f32) -> Artwork {
        Artwork {
            w,
            h,
            svg: String::new(),
            hits: vec![],
            title: String::new(),
            bg: String::new(),
            motion: None,
        }
    }
    #[test]
    fn hidpi_and_deep_zoom_preserve_density_by_tiling_only_the_visible_viewport() {
        let art = artwork(2000., 1000.);
        let view = FrameView {
            viewport: Viewport {
                zoom: 6.,
                ..Viewport::default()
            },
            size: (1920., 1080.),
            dpi: 2.,
        };
        let plan = frame_plan(&art, 1200, Some(view)).unwrap().unwrap();
        // The 1100-point focus width at 6× zoom and 2× DPI lands on an integer.
        assert_eq!(plan.pixels, 13_200);
        assert!(plan.pixels > 4096, "deep zoom must retain physical density");
        assert!(plan.tiles.len() > 1);
        assert!(plan.max_resident <= 2);
        assert!(plan.decoded_bytes as u64 <= DECODED_RESIDENT_BUDGET);
        assert_eq!(
            plan.decoded_bytes,
            plan.tiles
                .iter()
                .map(|tile| tile.width as usize * tile.height as usize * 4)
                .sum::<usize>()
        );
        for tile in &plan.tiles {
            assert!(tile.width as u64 * tile.height as u64 * 4 <= DECODED_FRAME_BUDGET);
            let region = tile.region.unwrap();
            assert!(
                region.x >= 0.
                    && region.y >= 0.
                    && region.x + region.w <= art.w
                    && region.y + region.h <= art.h
            );
            let density = tile.width as f64 / f64::from(region.w);
            assert!((density - plan.pixels as f64 / f64::from(art.w)).abs() < 0.001);
        }
        let hidden = FrameView {
            viewport: Viewport {
                x: 100000.,
                ..view.viewport
            },
            ..view
        };
        assert!(frame_plan(&art, 1200, Some(hidden)).unwrap().is_none());
    }
    #[test]
    fn zoom_changes_refresh_one_paused_frame_and_hidden_views_do_no_work() {
        let control = PlaybackControl::new();
        control.set_playhead(4);
        control.pause();
        control.set_viewport(Viewport::default(), (1200., 800.), 2.);
        let epoch = control.epoch();
        assert_eq!(control.plan(0, 5, 119).unwrap(), (epoch, 4));
        control.set_viewport(Viewport::default(), (1200., 800.), 2.);
        assert_eq!(
            control.epoch(),
            epoch,
            "unchanged drawing does not invalidate frames"
        );
        let background = control.clone();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            tx.send(background.plan(epoch, 5, 119)).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        control.set_visible(false);
        control.set_viewport(
            Viewport {
                zoom: 3.,
                ..Viewport::default()
            },
            (1200., 800.),
            2.,
        );
        assert!(
            rx.recv_timeout(Duration::from_millis(20)).is_err(),
            "hidden zoom must not rasterize"
        );
        control.set_visible(true);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap().unwrap(),
            (control.epoch(), 4)
        );
        worker.join().unwrap();
    }
    #[test]
    fn fractional_display_scale_uses_the_same_pixel_grid_as_static_paint() {
        let art = artwork(1200., 630.);
        for dpi in [1.25, 1.5, 1.75, 2.] {
            for zoom in [1., 1.2, 2.48832, 6.] {
                let view = FrameView {
                    viewport: Viewport {
                        zoom,
                        ..Viewport::default()
                    },
                    size: (1360., 820.),
                    dpi,
                };
                let rect = focused_rect(art.w, art.h, view.viewport, view.size);
                let width = texture_width(rect.w, dpi);
                let plan = frame_plan(&art, 1, Some(view)).unwrap().unwrap();
                assert_eq!(plan.pixels, width);
                assert_eq!(texture_width(width as f32 / dpi, dpi), width);
                for tile in plan.tiles {
                    assert!(tile.width as u64 * tile.height as u64 * 4 <= DECODED_FRAME_BUDGET);
                }
            }
        }
    }
    #[test]
    fn resident_frame_count_shrinks_with_viewport_cost() {
        assert_eq!(resident_limit(8 * 1024 * 1024), 8);
        assert_eq!(resident_limit(32 * 1024 * 1024), 2);
        assert_eq!(resident_limit(40 * 1024 * 1024), 1);
        let plan = frame_plan(
            &artwork(2000., 1000.),
            1200,
            Some(FrameView {
                viewport: Viewport {
                    zoom: 10.,
                    ..Viewport::default()
                },
                size: (8000., 6000.),
                dpi: 2.,
            }),
        );
        assert!(
            plan.is_err(),
            "excessive viewports fail explicitly instead of allocating unbounded pixels"
        );
    }
}
