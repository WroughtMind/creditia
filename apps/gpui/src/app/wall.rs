use super::{Creditia, FocusStyles, FocusTransition, Panel, Playback, Tile, ViewBookmark};
use crate::ui_model::{
    Artwork, Rect, TileSpec, Viewport, default_variant, focused_rect, masonry, swap_variant,
    texture_width, wall_tiles,
};
use creditia_desktop_service as service;
use gpui_kit::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl Creditia {
    pub(super) fn group(&self) -> String {
        self.project_preferences().group
    }

    pub(super) fn style(&self) -> Option<&Value> {
        self.catalog
            .as_ref()?
            .get("styles")?
            .as_array()?
            .get(self.style_index)
    }

    pub(super) fn piece(&self, id: &str) -> Option<&Value> {
        self.style()?
            .get("pieces")?
            .as_array()?
            .iter()
            .find(|p| p["id"] == id)
    }

    pub(super) fn current_tile(&self) -> Option<&Tile> {
        self.selected_tile.and_then(|i| self.tiles.get(i))
    }

    pub(super) fn resolve_style(&mut self) {
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

    pub(super) fn selected_piece(&self) -> Option<&Value> {
        self.current_tile().and_then(|t| self.piece(&t.spec.piece))
    }

    pub(super) fn rebuild_tiles(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn bookmark(&self) -> ViewBookmark {
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
        if selected.is_none()
            && bookmark.focused
            && let Some(mut spec) = bookmark.selected_spec
        {
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
        self.viewport = bookmark.viewport;
        self.focused = bookmark.focused && selected.is_some();
        if selected.is_some() {
            self.selected_tile = selected;
        }
        self.saved_viewport = bookmark.saved_viewport;
        self.restore_fixture_artworks();
    }

    pub(super) fn rebuild_preserving_view(&mut self, cx: &mut Context<Self>) {
        self.retain_focus_preview(cx);
        let bookmark = self.bookmark();
        self.rebuild_tiles(cx);
        self.restore_bookmark(bookmark);
    }

    pub(super) fn restore_pending_view(&mut self) {
        if let Some(bookmark) = self.pending_view.take() {
            self.restore_bookmark(bookmark);
        }
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
            if tile.loading
                && let Some(cancel) = &self.preview_cancellation
            {
                cancel.cancel();
            }
            if let Some(image) = tile.image
                && !retained.contains(&(Arc::as_ptr(&image) as usize))
            {
                crate::image_assets::release(image, cx);
            }
        }
        self.selected_tile = selected
            .and_then(|identity| self.tiles.iter().position(|tile| tile.identity == identity))
            .or_else(|| (!self.tiles.is_empty()).then_some(0));
        self.restore_fixture_artworks();
    }

    pub(super) fn compute_layout(&mut self, width: f32, height: f32, cx: &mut Context<Self>) {
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

    pub(super) fn screen_rect(&self, index: usize) -> Rect {
        let target = if self.focused && self.selected_tile == Some(index) {
            let art = self
                .focus_placeholder
                .as_ref()
                .and_then(|tile| tile.art.as_deref())
                .or_else(|| {
                    if self.export_workspace_open() {
                        self.tiles.get(index).and_then(|tile| tile.art.as_deref())
                    } else {
                        self.playback
                            .as_ref()
                            .and_then(Playback::displayed)
                            .map(|frame| frame.art.as_ref())
                            .or_else(|| self.tiles.get(index).and_then(|tile| tile.art.as_deref()))
                    }
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
        if self.selected_tile == Some(index)
            && let Some(transition) = &self.focus_transition
        {
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

    pub(super) fn artwork_viewport(&self) -> Rect {
        crate::ui_model::artwork_viewport(self.viewport_size, self.focused)
    }

    pub(super) fn crop_for_art(&self, art: &Artwork, r: Rect, pixels: u32) -> Option<Rect> {
        let height = pixels as f32 * art.h / art.w;
        if pixels as f32 * height < 16_000_000. && height < 16384. && pixels < 16384 {
            return None;
        }
        let scale = r.w / art.w;
        let clip = self.artwork_viewport();
        let x = ((clip.x - r.x) / scale).clamp(0., art.w);
        let y = ((clip.y - r.y) / scale).clamp(0., art.h);
        let right = ((clip.x + clip.w - r.x) / scale).clamp(x, art.w);
        let bottom = ((clip.y + clip.h - r.y) / scale).clamp(y, art.h);
        Some(Rect {
            x,
            y,
            w: (right - x).max(1.),
            h: (bottom - y).max(1.),
        })
    }

    pub(super) fn choose_style(&mut self, index: usize, cx: &mut Context<Self>) {
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

    pub(super) fn apply_focus_style(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn step_style(&mut self, delta: isize, cx: &mut Context<Self>) {
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

    pub(super) fn set_group(&mut self, group: &str, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.preferences.projects.entry(id).or_default().group = group.into();
            self.persist();
            self.rebuild_preserving_view(cx);
        }
    }

    pub(super) fn set_names(&mut self, cx: &mut Context<Self>) {
        self.preferences.named = !self.preferences.named;
        self.persist();
        self.rebuild_preserving_view(cx);
    }

    pub(super) fn set_view(&mut self, mode: &str, cx: &mut Context<Self>) {
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

    pub(super) fn select_tile(&mut self, index: usize, focus: bool, cx: &mut Context<Self>) {
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
        if focus && self.focused && self.selected_tile != Some(index) {
            self.retain_focus_preview(cx);
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

    pub(super) fn close_focus(&mut self, cx: &mut Context<Self>) {
        self.export_open = false;
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

    pub(super) fn select_piece(&mut self, piece: String, cx: &mut Context<Self>) {
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
        self.cycle_variant(delta, cx);
    }

    pub(super) fn cycle_variant(&mut self, delta: isize, cx: &mut Context<Self>) {
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

    pub(super) fn choose_variant(&mut self, variant: String, cx: &mut Context<Self>) {
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
            self.clear_preview(cx);
            self.viewport = bookmark.viewport;
            if bookmark.focused {
                self.rebuild_tiles(cx);
            } else {
                self.sync_roam(cx);
            }
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

    pub(super) fn reset_viewport(&mut self, cx: &mut Context<Self>) {
        let target = if self.preferences.view == "roam" && !self.focused {
            crate::roam::home(self.viewport_size)
        } else {
            Viewport::default()
        };
        let smooth = self.preferences.view == "roam" && !self.focused && !self.reduced_motion;
        if self.roam_motion.glide_to(
            self.motion_clock.elapsed(),
            (self.viewport.x, self.viewport.y),
            (target.x, target.y),
            smooth,
        ) {
            self.viewport.zoom = target.zoom;
        } else {
            self.viewport = target;
        }
        cx.notify();
    }

    pub(super) fn keyboard_navigation(
        &mut self,
        horizontal: bool,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
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

    pub(super) fn zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        if !self.focused && self.preferences.view != "roam" {
            self.set_view("roam", cx);
        }
        self.zoom_at(factor, self.viewport_size.0 / 2., self.viewport_size.1 / 2.);
        cx.notify();
    }

    pub(super) fn wheel(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        self.roam_motion.stop();
        let delta = event.delta.pixel_delta(px(24.));
        let dx: f32 = delta.x.into();
        let dy: f32 = delta.y.into();
        if event.modifiers.control || event.modifiers.platform {
            if !self.focused && self.preferences.view != "roam" {
                self.set_view("roam", cx);
            }
            self.zoom_at(
                (-dy * 0.006).exp(),
                event.position.x.into(),
                f32::from(event.position.y) - 60.,
            );
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

    pub(super) fn pinch(&mut self, event: &PinchEvent, cx: &mut Context<Self>) {
        if self.panel.is_some() || !event.delta.is_finite() || event.delta <= -1. {
            return;
        }
        self.roam_motion.stop();
        if !self.focused && self.preferences.view != "roam" {
            self.set_view("roam", cx);
        }
        self.zoom_at(
            1. + event.delta,
            event.position.x.into(),
            f32::from(event.position.y) - 60.,
        );
        self.hover = None;
        cx.notify();
        cx.stop_propagation();
    }

    pub(super) fn pointer_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.panel.is_some() || self.export_workspace_open() {
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

    pub(super) fn pointer_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.panel.is_some() {
            self.drag = None;
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        if drag.moved && drag.tile.is_some() {
            self.export_open = false;
        }
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
        } else {
            self.toggle_export(from, cx);
        }
    }

    pub(super) fn animate_focus_transition(&mut self, window: &mut Window) {
        if self
            .focus_transition
            .as_ref()
            .is_some_and(|transition| transition.started.elapsed() < Duration::from_millis(180))
        {
            window.request_animation_frame();
        } else {
            self.focus_transition = None;
        }
    }

    pub(super) fn animate_roam(&mut self, window: &mut Window) {
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
    }
}
