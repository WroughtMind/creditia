use super::{Creditia, PendingImage};
use crate::ui_model::{Artwork, Rect, TileSpec, texture_width};
use crate::{native_preview, renderer};
use gpui_kit::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl Creditia {
    pub(super) fn load_catalog(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn artwork_request(&self, spec: &TileSpec, op: &str) -> Option<Value> {
        let mut request = json!({
            "version": 1,
            "id": format!("{}:{}", self.preview_generation, spec.key),
            "op": op,
            "style": self.style()?["id"],
            "piece": spec.piece,
            "lang": self.preferences.language.code(),
            "group": self.group(),
            "named": self.preferences.named,
            "flatten": false
        });
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

    pub(super) fn restore_fixture_artworks(&mut self) {
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

    pub(super) fn prepare_static_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                && let Some(image) = &tile.image
            {
                crate::image_assets::track(image, window, cx);
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

    pub(super) fn schedule_visible(&mut self, dpi: f32, cx: &mut Context<Self>) {
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
        if let Some(first) = pending.next()
            && (first.bytes > 8 * 1024 * 1024 || pending.next().is_some())
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
                    .and_then(|art| self.crop_for_art(art, r, pixels))
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
            let result = cx
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
                            Arc::new(Artwork::from_result(&result)?)
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
                    let raster = native_preview::rasterize_artwork_region(
                        &art,
                        region
                            .map(|r| texture_width(pixels as f32 * r.w / art.w, 1.))
                            .unwrap_or(pixels),
                        region.map(|r| (r.x, r.y, r.w, r.h)),
                        &cancellation,
                    )?;
                    Ok::<_, String>((art, raster, region, archive))
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
                            if error == "__empty__"
                                && let Some(roam) = &mut view.roam_layout
                            {
                                roam.mark_empty(&crate::roam::canonical_key(
                                    &tile.spec.piece,
                                    tile.spec.variant.as_deref(),
                                ));
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

    pub(super) fn release_images(&mut self, cx: &mut App) {
        self.export_preview.clear(cx);
        if let Some(pending) = self.backdrop_pending.take() {
            for image in std::iter::once(pending.image)
                .chain(pending.background)
                .chain(pending.top)
            {
                crate::image_assets::release(image, cx);
            }
        }
        for image in [
            self.backdrop_image.take(),
            self.backdrop_background.take(),
            self.backdrop_top.take(),
        ]
        .into_iter()
        .flatten()
        {
            crate::image_assets::release(image, cx);
        }
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
        if let Some(tile) = self.focus_placeholder.take()
            && let Some(image) = tile.image
        {
            crate::image_assets::release(image, cx);
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

    pub(super) fn sync_raster_geometry(&mut self, window: &mut Window) -> (f32, f32) {
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
        (width, height)
    }
}
