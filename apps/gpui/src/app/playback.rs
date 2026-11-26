use super::{Creditia, Playback, PlaybackFrame};
use crate::renderer;
use crate::ui_model::{Rect, focused_rect, frame_at, texture_width};
use creditia_desktop_service as service;
use gpui_kit::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl PlaybackFrame {
    pub(super) fn release(self, cx: &mut App) {
        for (_, image) in self.tiles {
            crate::image_assets::release_frame(image, cx);
        }
    }
}

impl Playback {
    // Follow elapsed time. Missing a raster skips samples rather than slowing
    // the whole animation. Completion still waits for the last frame to paint.
    pub(super) fn tick(&mut self, control: &crate::animation::PlaybackControl) -> bool {
        let last = self.times.len() - 1;
        let now = Instant::now();
        let duration = self.timeline.duration_ms;
        let end = duration.saturating_add(self.timeline.hold_ms);
        self.elapsed = now
            .duration_since(self.started)
            .min(Duration::from_millis(end));
        let time = self.elapsed.as_millis() as u64;
        self.requested = if time >= duration {
            last
        } else {
            frame_at(&self.times, &self.timeline, time)
        };
        control.set_playhead(self.requested);
        if let Some((&index, _)) = self
            .frames
            .range(..=self.requested)
            .rev()
            .find(|(_, frame)| frame.ready)
        {
            self.index = index;
        }
        let last_painted = self.frames.get(&last).and_then(|frame| frame.painted.get());
        if time >= end
            && last_painted.is_some_and(|painted| {
                now.duration_since(*painted) >= Duration::from_millis(self.timeline.hold_ms)
            })
        {
            self.playing = false;
            self.ended = true;
            control.pause();
            return false;
        }
        time < duration || self.index == last
    }

    pub(super) fn displayed(&self) -> Option<&PlaybackFrame> {
        self.frames
            .get(&self.index)
            .filter(|frame| frame.ready)
            .or(self.previous.as_ref())
    }
}

impl Creditia {
    pub(super) fn stop_playback(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn drop_focus_placeholder(&mut self, cx: &mut App) {
        if let Some(tile) = self.focus_placeholder.take()
            && let Some(image) = tile.image
            && !self.tiles.iter().any(|tile| {
                tile.image
                    .as_ref()
                    .is_some_and(|kept| Arc::ptr_eq(kept, &image))
            })
        {
            crate::image_assets::release(image, cx);
        }
    }

    pub(super) fn retain_focus_preview(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn prepare_focus_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn clear_preview(&mut self, cx: &mut Context<Self>) {
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
            if let Some(image) = tile.image.take()
                && !self
                    .focus_placeholder
                    .as_ref()
                    .and_then(|tile| tile.image.as_ref())
                    .is_some_and(|kept| Arc::ptr_eq(kept, &image))
            {
                crate::image_assets::release(image, cx);
            }
        }
        self.tiles.clear();
        self.roam_layout = None;
        self.roam_dimensions.clear();
        self.rects.clear();
        self.hover = None;
    }

    pub(super) fn play(&mut self, restart: bool, cx: &mut Context<Self>) {
        if self.animation_busy && self.playback.is_none() {
            return;
        }
        if let Some(play) = &mut self.playback {
            if restart || play.ended {
                play.elapsed = Duration::ZERO;
                play.started_once = false;
                play.playing = true;
                play.ended = false;
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
        let Some(raw) = self.raw.clone() else {
            return;
        };
        if self.export_animation() {
            request["size"] = json!(self.export_size);
            request["transparent"] = json!(false);
        } else {
            if !self.focused {
                self.saved_viewport = Some(self.viewport);
                self.viewport.reset();
            }
            self.focused = true;
        }
        self.animation_busy = true;
        let control = crate::animation::PlaybackControl::new();
        self.configure_playback_view(&control);
        self.animation_cancellation = Some(control.cancellation());
        self.animation_control = Some(control.clone());
        let width = texture_width(self.viewport_size.0, self.display_scale);
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
            if let Err(error) = result
                && !control.user_cancelled()
            {
                let _ = tx.send(Err(error)).await;
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
                                mut timeline,
                                times,
                            }) => {
                                // Preview playback is one-shot. The renderer's
                                // authored timeline remains intact for exports.
                                timeline.repeat = false;
                                let playing = timeline.duration_ms > 0;
                                view.playback = Some(Playback {
                                    frames: BTreeMap::new(),
                                    previous: None,
                                    times,
                                    timeline,
                                    started: Instant::now(),
                                    elapsed: Duration::ZERO,
                                    playing,
                                    ended: false,
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
                                ..
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
                                        index,
                                        ready: true,
                                        painted: Arc::new(std::sync::OnceLock::new()),
                                        art,
                                        pixels,
                                        decoded_bytes,
                                        regions: tiles.iter().map(|tile| json!({
                                            "width":tile.width,"height":tile.height,
                                            "region":tile.region.map(|r| json!({"x":r.x,"y":r.y,"w":r.w,"h":r.h}))
                                        })).collect(),
                                        tiles: tiles.into_iter().map(|tile| (tile.region, tile.image)).collect(),
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
                                        && let Some(previous) = play.previous.take() {
                                            previous.release(cx);
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
                                view.stop_playback(cx);
                                view.error = Some(error);
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

    fn prepare_playback_images(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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
    #[cfg(feature = "visual-test")]
    pub fn fixture_artwork_image(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        let image = self.current_tile()?.image.as_ref()?;
        crate::image_assets::track(image, window, cx)
    }

    pub fn fixture_images_ready(&self, window: &mut Window, cx: &mut App) -> bool {
        if self.fixture && !self.benchmark && (self.raw.is_none() || self.loading) {
            return true;
        }
        if self.export_workspace_open() {
            if self.export_animation() {
                return self
                    .playback
                    .as_ref()
                    .and_then(Playback::displayed)
                    .is_some_and(|frame| frame.painted.get().is_some());
            }
            return self.export_preview.ready();
        }
        if self.backdrop_task.is_some() || self.backdrop_pending.is_some() {
            return false;
        }
        if self
            .focus_transition
            .as_ref()
            .is_some_and(|transition| transition.started.elapsed() < Duration::from_millis(180))
        {
            return false;
        }
        if self.focused
            && let Some(play) = &self.playback
        {
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
            return !frame.tiles.is_empty() && frame.painted.get().is_some();
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
                .and_then(|art| self.crop_for_art(art, r, pixels))
                && tile.image_region.is_none_or(|have| {
                    have.x > required.x + 1.
                        || have.y > required.y + 1.
                        || have.x + have.w < required.x + required.w - 1.
                        || have.y + have.h < required.y + required.h - 1.
                })
            {
                return false;
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

    pub(super) fn start_background(&mut self, cx: &mut Context<Self>) {
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
                                            view.show_success(
                                                view.text("已取消分析", "Analysis cancelled")
                                                    .into(),
                                                cx,
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

    pub(super) fn update_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let playback_context = self.focused || self.export_animation();
        let animation_visible = (!self.export_workspace_open() || self.export_animation())
            && self
                .selected_tile
                .is_some_and(|index| self.screen_rect(index).intersects(self.artwork_viewport()));
        if animation_visible
            && playback_context
            && self.panel.is_none()
            && window.is_window_active()
            && self.window_visible
        {
            self.prepare_playback_images(window, cx);
        }
        if let Some(control) = &self.animation_control {
            control.set_visible(
                animation_visible
                    && playback_context
                    && self.panel.is_none()
                    && window.is_window_active()
                    && self.window_visible,
            );
            self.configure_playback_view(control);
        }
        let can_animate = animation_visible
            && playback_context
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
                && playback_context
                && self.panel.is_none()
                && window.is_window_active()
                && self.window_visible
                && !self.reduced_motion
            {
                if let Some(control) = &self.animation_control
                    && control.epoch() == play.epoch
                    && play.tick(control)
                {
                    window.request_animation_frame();
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
        can_animate
    }
}
