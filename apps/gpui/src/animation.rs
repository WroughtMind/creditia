//! Incremental artwork playback. The shared engine owns time sampling; this
//! producer prepares only the current frame and a small bounded look-ahead.
use crate::{native_preview::{self, RasterImage}, renderer, ui_model::{Artwork, Timeline}};
use serde_json::{Value, json};
use std::sync::{Arc, Condvar, Mutex};

pub const MAX_AHEAD: usize = 6;
pub const MAX_RESIDENT_FRAMES: usize = 8;
pub const DECODED_FRAME_BUDGET: u64 = 8 * 1024 * 1024;

#[derive(Default)]
struct Position { playhead: usize, epoch: u64, paused: bool }
struct Shared { position: Mutex<Position>, changed: Condvar, cancellation: renderer::Cancellation }

#[derive(Clone)]
pub struct PlaybackControl(Arc<Shared>);
impl Default for PlaybackControl { fn default() -> Self { Self::new() } }
impl PlaybackControl {
    pub fn new() -> Self { Self(Arc::new(Shared { position: Mutex::new(Position::default()), changed: Condvar::new(), cancellation: renderer::Cancellation::default() })) }
    pub fn cancellation(&self) -> renderer::Cancellation { self.0.cancellation.clone() }
    pub fn cancelled(&self) -> bool { self.0.cancellation.cancelled() }
    pub fn epoch(&self) -> u64 { self.0.position.lock().unwrap_or_else(|e| e.into_inner()).epoch }
    pub fn pause(&self) { self.0.position.lock().unwrap_or_else(|e| e.into_inner()).paused = true; }
    pub fn resume(&self) {
        self.0.position.lock().unwrap_or_else(|e| e.into_inner()).paused = false;
        self.0.changed.notify_all();
    }
    /// Always cancel through this control so a paused producer wakes as well.
    pub fn cancel(&self) { self.0.cancellation.cancel(); self.0.changed.notify_all(); }
    pub fn set_playhead(&self, index: usize) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        if position.playhead == index { return; }
        if index < position.playhead { position.epoch = position.epoch.wrapping_add(1); }
        position.playhead = index;
        self.0.changed.notify_all();
    }
    pub fn restart(&self) {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        position.epoch = position.epoch.wrapping_add(1);
        position.playhead = 0; position.paused = false;
        self.0.changed.notify_all();
    }
    fn plan(&self, epoch: u64, next: usize, last: usize) -> Result<(u64, usize), String> {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if self.cancelled() { return Err("Animation cancelled".into()); }
            if !position.paused {
                let current = position.playhead.min(last);
                let candidate = if position.epoch != epoch { current } else { next.max(current) };
                if candidate <= current.saturating_add(MAX_AHEAD).min(last) { return Ok((position.epoch, candidate)); }
            }
            position = self.0.changed.wait(position).unwrap_or_else(|e| e.into_inner());
        }
    }
    fn current(&self, epoch: u64) -> Result<bool, String> {
        let mut position = self.0.position.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if self.cancelled() { return Err("Animation cancelled".into()); }
            if position.epoch != epoch { return Ok(false); }
            if !position.paused { return Ok(true); }
            position = self.0.changed.wait(position).unwrap_or_else(|e| e.into_inner());
        }
    }
}

pub enum Event {
    Started { epoch: u64, timeline: Timeline, times: Vec<f64> },
    Frame { epoch: u64, index: usize, art: Arc<Artwork>, raster: Arc<RasterImage> },
}

/// Start on a background executor. `send` should feed a bounded channel (2–3
/// events) and return an error when the view is gone. The UI keeps at most eight
/// decoded frames, updates the playhead, and drops images outside its window.
/// This routine stays asleep after the final frame, ready for a replay/loop.
pub fn run(mut request: Value, pixel_width: u32, control: PlaybackControl, mut send: impl FnMut(Event) -> Result<(), String>) -> Result<(), String> {
    let object = request.as_object_mut().ok_or("Animation request must be an object")?;
    object.remove("frameCount"); object.insert("op".into(), json!("render"));
    object.insert("flatten".into(), json!(true));
    let first_time = if request["reducedMotion"] == true { 1.0 } else { 0.0 };
    request["t"] = json!(first_time);
    // A replay while the first request is in flight still needs its metadata.
    // Frame zero is identical across playback epochs, so adopt the latest epoch
    // after preparation instead of abandoning the producer before Started.
    control.plan(control.epoch(), 0, 0)?;
    let result = renderer::request_with_priority(request.clone(), control.cancellation(), 0)?;
    let first: Artwork = serde_json::from_value(result["art"].clone()).map_err(|e| format!("Invalid first animation frame: {e}"))?;
    let timeline: Timeline = serde_json::from_value(result["timeline"].clone()).map_err(|e| format!("Invalid animation timeline: {e}"))?;
    let times = playback_times(&timeline);
    let first_epoch = control.plan(control.epoch(), 0, 0)?.0;
    send(Event::Started { epoch: first_epoch, timeline: timeline.clone(), times: times.clone() })?;
    let first = Arc::new(first);
    let raster = raster_frame(&first, pixel_width, &control)?;
    if control.current(first_epoch)? { send(Event::Frame { epoch: first_epoch, index: 0, art: first, raster })?; }
    if times.len() == 1 { return Ok(()); }
    let mut epoch = first_epoch;
    let mut next = 1;
    loop {
        let (wanted_epoch, index) = control.plan(epoch, next, times.len()-1)?;
        epoch = wanted_epoch;
        request["t"] = json!(times[index]);
        let result = renderer::request_with_priority(request.clone(), control.cancellation(), 0)?;
        let art: Artwork = serde_json::from_value(result["art"].clone()).map_err(|e| format!("Invalid animation frame: {e}"))?;
        let art = Arc::new(art);
        let raster = raster_frame(&art, pixel_width, &control)?;
        if control.current(epoch)? { send(Event::Frame { epoch, index, art, raster })?; }
        next = index + 1;
    }
}

pub fn playback_times(timeline: &Timeline) -> Vec<f64> {
    if timeline.duration_ms == 0 { return vec![1.0]; }
    // Native playback can sample more densely than GIF without changing the
    // action definition. Long clips remain bounded to 120 indexed samples.
    let steps = (timeline.duration_ms.saturating_mul(60).div_ceil(1000)).clamp(1, 119) as usize;
    (0..=steps).map(|index| index as f64 / steps as f64).collect()
}

pub fn frame_width(width: f32, height: f32, requested: u32) -> Result<u32, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 || requested == 0 { return Err("Invalid animation dimensions".into()); }
    let ratio = f64::from(height) / f64::from(width);
    let max_width = ((DECODED_FRAME_BUDGET as f64 / 4.0 / ratio).sqrt().floor() as u32).min(32768);
    let mut pixels = requested.min(max_width);
    while pixels > 0 {
        let h = (f64::from(pixels) * ratio).ceil();
        if h <= 32768.0 && f64::from(pixels) * h * 4.0 <= DECODED_FRAME_BUDGET as f64 { return Ok(pixels); }
        pixels -= 1;
    }
    Err("This animation frame is too tall for a bounded preview; use its scrolling layout".into())
}
fn raster_frame(art: &Artwork, requested: u32, control: &PlaybackControl) -> Result<Arc<RasterImage>, String> {
    let width = frame_width(art.w, art.h, requested)?;
    native_preview::rasterize(&art.svg, width, &control.cancellation())
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
        let background = control.clone(); let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || { tx.send(background.plan(0, 7, 119)).unwrap(); });
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        control.cancel();
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap().unwrap_err(), "Animation cancelled");
        worker.join().unwrap();
    }
    #[test]
    fn seek_and_loop_reset_the_epoch_and_skip_obsolete_frames() {
        let control = PlaybackControl::new();
        control.set_playhead(20);
        assert_eq!(control.plan(0, 3, 119).unwrap(), (0, 20));
        control.set_playhead(0);
        assert_eq!(control.plan(0, 21, 119).unwrap(), (1, 0));
        control.restart(); assert_eq!(control.epoch(), 2);
        assert!(!control.current(1).unwrap());
        assert_eq!(control.plan(0, 0, 0).unwrap(), (2, 0), "a replay during initial preparation adopts its epoch");
    }
    #[test]
    fn sampled_times_include_both_endpoints_and_remain_bounded() {
        let timeline = Timeline { duration_ms: 5000, hold_ms: 1000, fps: 20, repeat: true };
        let times = playback_times(&timeline);
        assert_eq!(times.len(), 120); assert_eq!(times.first(), Some(&0.)); assert_eq!(times.last(), Some(&1.));
        assert_eq!(playback_times(&Timeline::default()), vec![1.]);
    }
}
