//! Infinite, deterministic artwork columns matching the original desktop roam.
//! A finite style repeats after a known slot period. At most five prefix tables
//! locate any row directly, so panning a million rows does not retain a million
//! cards or require drawing the intervening artwork.
use crate::ui_model::{Rect, TileSpec, Viewport, default_variant};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const CAPTION: f64 = 30.;
pub const GAP: f64 = 56.;
pub const MAX_VISIBLE_TILES: usize = 2048;
const STEP: i128 = 5;
const MAX_COLUMNS: i64 = 96;
const MAX_PERIOD: usize = 65_536;

#[derive(Clone, Debug)]
pub struct RoamTile {
    pub slot_key: String,
    pub column: i64,
    pub row: i64,
    /// Canonical piece/layout identity; multiple visible slots may share it.
    pub spec: TileSpec,
    pub rect: Rect,
}

struct BoardPiece {
    id: String,
    variants: Vec<String>,
    base: usize,
    fallback: Option<String>,
}
struct Prefix {
    offsets: Vec<f64>,
    total: f64,
}
#[derive(Clone, Copy)]
struct Anchor {
    column: i64,
    row: i64,
    column_fraction: f64,
    fraction: f64,
    gap: f64,
}

pub struct RoamLayout {
    board: Vec<BoardPiece>,
    period: usize,
    classes: usize,
    inverse: i128,
    prefixes: Vec<Prefix>,
    dimensions: BTreeMap<String, (f32, f32)>,
    empty: BTreeSet<String>,
    column_width: f64,
    previous_size: (f32, f32),
    dirty: bool,
}

impl RoamLayout {
    pub fn new(style: &Value, selected: &BTreeMap<String, String>) -> Result<Self, String> {
        let pieces = style["pieces"]
            .as_array()
            .ok_or("Roam style has no artwork catalog")?;
        let entries = style["board"].as_array().ok_or("Roam style has no board")?;
        let mut board = Vec::new();
        let mut rotations = 1usize;
        for entry in entries {
            let Some(id) = entry["id"].as_str() else {
                continue;
            };
            let Some(piece) = pieces.iter().find(|piece| piece["id"] == id) else {
                continue;
            };
            let variants = piece["variants"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|variant| variant["id"].as_str().map(str::to_owned))
                .collect::<Vec<_>>();
            let default = entry["variant"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| default_variant(piece));
            let key = canonical_key(id, default.as_deref());
            let current = selected.get(&key).cloned().or(default);
            let base = current
                .as_ref()
                .and_then(|current| variants.iter().position(|variant| variant == current))
                .unwrap_or(0);
            let count = variants.len().max(1);
            rotations = (rotations / gcd(rotations, count))
                .checked_mul(count)
                .filter(|value| *value <= MAX_PERIOD * 5)
                .ok_or("Roam layout period is too large")?;
            board.push(BoardPiece {
                id: id.into(),
                variants,
                base,
                fallback: current,
            });
        }
        if board.is_empty() {
            return Err("Roam style has no available board pieces".into());
        }
        let slot_period = board
            .len()
            .checked_mul(rotations)
            .ok_or("Roam layout period overflow")?;
        let classes = gcd(slot_period, STEP as usize);
        let period = slot_period / classes;
        if period > MAX_PERIOD {
            return Err("Roam layout period exceeds its geometry budget".into());
        }
        let inverse = if period == 1 {
            0
        } else {
            modular_inverse(STEP / classes as i128, period as i128)
        };
        Ok(Self {
            board,
            period,
            classes,
            inverse,
            prefixes: Vec::new(),
            dimensions: BTreeMap::new(),
            empty: BTreeSet::new(),
            column_width: 0.,
            previous_size: (0., 0.),
            dirty: true,
        })
    }

    /// Mark an artwork/layout with no data. Its occurrences disappear without
    /// changing any other slot identity, and an empty catalog does not spin.
    pub fn mark_empty(&mut self, canonical_key: &str) {
        if self.empty.insert(canonical_key.into()) {
            self.dirty = true;
        }
    }

    /// Grow only the visible neighborhood. World rectangles are transformed by
    /// the normal pan/zoom viewport. New measured artwork dimensions and window
    /// resizing preserve the artwork point at the previous viewport center.
    pub fn layout(
        &mut self,
        viewport: &mut Viewport,
        size: (f32, f32),
        dimensions: &BTreeMap<String, (f32, f32)>,
    ) -> Vec<RoamTile> {
        if ![viewport.x, viewport.y, viewport.zoom, size.0, size.1]
            .iter()
            .all(|n| n.is_finite())
            || viewport.zoom <= 0.
            || size.0 <= 0.
            || size.1 <= 0.
        {
            return Vec::new();
        }
        let width = column_width(size.0);
        let dimensions_changed = &self.dimensions != dimensions;
        let reflow = self.dirty || self.column_width != width || dimensions_changed;
        let resized = self.previous_size != size;
        let anchor = if !self.prefixes.is_empty() && (reflow || resized) {
            self.anchor(*viewport)
        } else {
            None
        };
        if reflow {
            self.column_width = width;
            if dimensions_changed {
                self.dimensions = dimensions
                    .iter()
                    .filter(|(_, (w, h))| w.is_finite() && h.is_finite() && *w > 0. && *h > 0.)
                    .map(|(key, size)| (key.clone(), *size))
                    .collect();
            }
            self.prefixes = (0..self.classes)
                .map(|class| self.make_prefix(class as i64))
                .collect();
            self.dirty = false;
        }
        if let Some(anchor) = anchor {
            let rect = self.rect(anchor.column, anchor.row);
            let center_x = (anchor.column as f64 + anchor.column_fraction) * self.pitch();
            let center_y = rect.1 + anchor.fraction * rect.3 + anchor.gap;
            viewport.x = (f64::from(size.0) / 2. - center_x * f64::from(viewport.zoom)) as f32;
            viewport.y = (f64::from(size.1) / 2. - center_y * f64::from(viewport.zoom)) as f32;
        }
        self.previous_size = size;
        let zoom = f64::from(viewport.zoom);
        let left = (-f64::from(viewport.x) - 700.) / zoom;
        let right = (-f64::from(viewport.x) + f64::from(size.0) + 700.) / zoom;
        let top = (-f64::from(viewport.y) - 700.) / zoom;
        let bottom = (-f64::from(viewport.y) + f64::from(size.1) + 700.) / zoom;
        let center_x = (-f64::from(viewport.x) + f64::from(size.0) / 2.) / zoom;
        let center_y = (-f64::from(viewport.y) + f64::from(size.1) / 2.) / zoom;
        let first = (left / self.pitch()).floor() as i64;
        let last = (right / self.pitch()).floor() as i64;
        let middle = (center_x / self.pitch()).floor() as i64;
        // Near columns come first when an unusually large viewport hits a cap.
        let mut columns = (first.max(middle.saturating_sub(MAX_COLUMNS / 2))
            ..=last.min(middle.saturating_add((MAX_COLUMNS - 1) / 2)))
            .collect::<Vec<_>>();
        columns.sort_by_key(|column| column.abs_diff(middle));
        let mut out = Vec::new();
        for column in columns {
            let Some(mut row) = self.locate(column, top) else {
                continue;
            };
            let mut scanned = 0usize;
            while out.len() < MAX_VISIBLE_TILES && scanned < MAX_VISIBLE_TILES + self.period {
                scanned += 1;
                let (x, y, w, h) = self.rect(column, row);
                if y - CAPTION > bottom {
                    break;
                }
                if h > 0. && y + h >= top && y - CAPTION <= bottom {
                    let spec = self.slot(column, row);
                    out.push(RoamTile {
                        slot_key: format!("{column}:{row}"),
                        column,
                        row,
                        spec,
                        rect: Rect {
                            x: x as f32,
                            y: y as f32,
                            w: w as f32,
                            h: h as f32,
                        },
                    });
                }
                let Some(next) = row.checked_add(1) else {
                    break;
                };
                row = next;
            }
            if out.len() >= MAX_VISIBLE_TILES {
                break;
            }
        }
        out.sort_by(|a, b| {
            let distance = |tile: &RoamTile| {
                (f64::from(tile.rect.x + tile.rect.w / 2.) - center_x).abs()
                    + (f64::from(tile.rect.y + tile.rect.h / 2.) - center_y).abs()
            };
            distance(a).total_cmp(&distance(b))
        });
        out
    }

    pub fn slot(&self, column: i64, row: i64) -> TileSpec {
        let (piece, variant) = self.selection(column, row);
        TileSpec {
            key: canonical_key(&piece.id, variant),
            piece: piece.id.clone(),
            variant: variant.map(str::to_owned),
            span: 1,
        }
    }
    pub fn geometry_entries(&self) -> usize {
        self.prefixes
            .iter()
            .map(|prefix| prefix.offsets.len())
            .sum()
    }
    fn pitch(&self) -> f64 {
        self.column_width + GAP
    }
    fn selection(&self, column: i64, row: i64) -> (&BoardPiece, Option<&str>) {
        let value = i128::from(column) + i128::from(row) * STEP;
        let count = self.board.len() as i128;
        let piece = &self.board[value.rem_euclid(count) as usize];
        let variant = if piece.variants.len() > 1 {
            let index = (piece.base as i128 + value.div_euclid(count))
                .rem_euclid(piece.variants.len() as i128) as usize;
            Some(piece.variants[index].as_str())
        } else {
            piece
                .fallback
                .as_deref()
                .or_else(|| piece.variants.first().map(String::as_str))
        };
        (piece, variant)
    }
    fn measure(&self, column: i64, row: i64) -> (f64, f64) {
        let (piece, variant) = self.selection(column, row);
        let key = canonical_key(&piece.id, variant);
        if self.empty.contains(&key) {
            return (0., 0.);
        }
        let (art_w, art_h) = self.dimensions.get(&key).copied().unwrap_or((640., 480.));
        let w = self.column_width.min(if art_w < 500. {
            f64::from(art_w) * 1.15
        } else {
            self.column_width
        });
        (w, w / f64::from(art_w) * f64::from(art_h))
    }
    fn make_prefix(&self, class: i64) -> Prefix {
        let mut offsets = Vec::with_capacity(self.period + 1);
        offsets.push(0.);
        for row in 0..self.period {
            let (_, h) = self.measure(class, row as i64);
            offsets.push(
                offsets.last().copied().unwrap_or(0.) + if h > 0. { h + GAP + CAPTION } else { 0. },
            );
        }
        Prefix {
            total: *offsets.last().unwrap(),
            offsets,
        }
    }
    fn shifted(&self, column: i64) -> (&Prefix, i64) {
        let class = column.rem_euclid(self.classes as i64);
        let shift = ((i128::from(column) - i128::from(class)) / self.classes as i128 * self.inverse)
            .rem_euclid(self.period as i128) as i64;
        (&self.prefixes[class as usize], shift)
    }
    fn prefix_sum(&self, prefix: &Prefix, index: i64) -> f64 {
        index.div_euclid(self.period as i64) as f64 * prefix.total
            + prefix.offsets[index.rem_euclid(self.period as i64) as usize]
    }
    fn rect(&self, column: i64, row: i64) -> (f64, f64, f64, f64) {
        let (prefix, shift) = self.shifted(column);
        let index = (i128::from(row) + i128::from(shift)).rem_euclid(self.period as i128) as usize;
        let h = (prefix.offsets[index + 1] - prefix.offsets[index] - GAP - CAPTION).max(0.);
        let (piece, variant) = self.selection(column, row);
        let art_w = self
            .dimensions
            .get(&canonical_key(&piece.id, variant))
            .map(|d| d.0)
            .unwrap_or(640.);
        let w = self.column_width.min(if art_w < 500. {
            f64::from(art_w) * 1.15
        } else {
            self.column_width
        });
        let y = column_offset(column) + self.prefix_sum(prefix, row.saturating_add(shift))
            - self.prefix_sum(prefix, shift);
        (
            column as f64 * self.pitch() + (self.column_width - w) / 2.,
            y,
            w,
            h,
        )
    }
    fn locate(&self, column: i64, y: f64) -> Option<i64> {
        let (prefix, shift) = self.shifted(column);
        if prefix.total <= 0. || !prefix.total.is_finite() {
            return None;
        }
        let target = y - column_offset(column) + self.prefix_sum(prefix, shift);
        let cycle = (target / prefix.total).floor();
        let offset = (target - cycle * prefix.total).clamp(0., prefix.total);
        let row = prefix
            .offsets
            .partition_point(|position| *position <= offset)
            .saturating_sub(1)
            .min(self.period - 1);
        Some(
            (cycle as i64)
                .saturating_mul(self.period as i64)
                .saturating_add(row as i64)
                .saturating_sub(shift),
        )
    }
    fn anchor(&self, viewport: Viewport) -> Option<Anchor> {
        let x = (f64::from(self.previous_size.0) / 2. - f64::from(viewport.x))
            / f64::from(viewport.zoom);
        let y = (f64::from(self.previous_size.1) / 2. - f64::from(viewport.y))
            / f64::from(viewport.zoom);
        let column = (x / self.pitch()).floor() as i64;
        let row = self.locate(column, y)?;
        let rect = self.rect(column, row);
        Some(Anchor {
            column,
            row,
            column_fraction: x / self.pitch() - column as f64,
            fraction: if rect.3 > 0. {
                ((y - rect.1) / rect.3).clamp(0., 1.)
            } else {
                0.
            },
            gap: (y - rect.1 - rect.3).max(0.),
        })
    }
}

pub fn canonical_key(piece: &str, variant: Option<&str>) -> String {
    format!("{piece}#{}", variant.unwrap_or(""))
}
pub fn column_width(viewport_width: f32) -> f64 {
    (f64::from(viewport_width) / 3.1).clamp(380., 520.).round()
}
pub fn column_offset(column: i64) -> f64 {
    // Match JavaScript Math.imul(... ) >>> 0, including negative columns.
    let hash = (column as u32 ^ 0x5bd1e995).wrapping_mul(0x27d4eb2d);
    f64::from(hash % 997) / 997. * 420.
}
pub fn home(size: (f32, f32)) -> Viewport {
    let cw = column_width(size.0);
    let pitch = cw + GAP;
    let count = ((f64::from(size.0) + GAP) / pitch).floor().max(1.);
    Viewport {
        x: ((f64::from(size.0) - (count * pitch - GAP)) / 2.).round() as f32,
        y: 96.,
        zoom: 1.,
    }
}
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn modular_inverse(a: i128, n: i128) -> i128 {
    let (mut old_r, mut r, mut old_s, mut s) = (a, n, 1, 0);
    while r != 0 {
        let q = old_r / r;
        (old_r, r) = (r, old_r - q * r);
        (old_s, s) = (s, old_s - q * s);
    }
    old_s.rem_euclid(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn style() -> Value {
        json!({"board":[{"id":"a"},{"id":"b"},{"id":"c"}],"pieces":[
        {"id":"a","defaultVariant":"wide","variants":[{"id":"wide"},{"id":"square"}]},
        {"id":"b","defaultVariant":"one","variants":[{"id":"one"},{"id":"two"},{"id":"three"}]},
        {"id":"c","variants":[{"id":"solo"}]}]})
    }
    #[test]
    fn slots_match_original_negative_modulo_laps_and_selected_variants() {
        let layout = RoamLayout::new(
            &style(),
            &BTreeMap::from([("a#wide".into(), "square".into())]),
        )
        .unwrap();
        for x in -20..20 {
            for row in -100..100 {
                let value: i64 = x + row * 5;
                let index = value.rem_euclid(3);
                let lap = value.div_euclid(3);
                let slot = layout.slot(x, row);
                let expected = match index {
                    0 => ["wide", "square"][(1 + lap).rem_euclid(2) as usize],
                    1 => ["one", "two", "three"][lap.rem_euclid(3) as usize],
                    _ => "solo",
                };
                assert_eq!(slot.variant.as_deref(), Some(expected));
                assert_eq!(slot.piece, ["a", "b", "c"][index as usize]);
            }
        }
    }
    #[test]
    fn prefix_geometry_matches_incremental_original_columns_in_both_directions() {
        let mut layout = RoamLayout::new(&style(), &BTreeMap::new()).unwrap();
        let dimensions = BTreeMap::from([
            ("a#wide".into(), (400., 100.)),
            ("b#one".into(), (800., 1200.)),
        ]);
        layout.layout(&mut home((1300., 800.)), (1300., 800.), &dimensions);
        for x in -13..14 {
            let mut y = column_offset(x);
            for row in 0..100 {
                let rect = layout.rect(x, row);
                assert!((rect.1 - y).abs() < 0.00001);
                y += rect.3 + GAP + CAPTION;
            }
            let mut y = column_offset(x);
            for row in (-100..0).rev() {
                let rect = layout.rect(x, row);
                y -= rect.3 + GAP + CAPTION;
                assert!((rect.1 - y).abs() < 0.00001);
            }
        }
    }
    #[test]
    fn huge_pan_is_bounded_revisitable_and_empty_art_is_skipped() {
        let mut layout = RoamLayout::new(&style(), &BTreeMap::new()).unwrap();
        let mut view = home((1280., 800.));
        let dimensions = BTreeMap::new();
        let origin = layout.layout(&mut view, (1280., 800.), &dimensions);
        let entries = layout.geometry_entries();
        let original_view = view;
        view.x -= 10_000_000.;
        view.y += 30_000_000.;
        let far = layout.layout(&mut view, (1280., 800.), &dimensions);
        assert!(!far.is_empty());
        assert!(far.len() <= MAX_VISIBLE_TILES);
        assert_eq!(layout.geometry_entries(), entries);
        view = original_view;
        let returned = layout.layout(&mut view, (1280., 800.), &dimensions);
        assert_eq!(
            origin
                .iter()
                .map(|tile| (&tile.slot_key, tile.rect))
                .collect::<Vec<_>>(),
            returned
                .iter()
                .map(|tile| (&tile.slot_key, tile.rect))
                .collect::<Vec<_>>()
        );
        for key in ["a#wide", "a#square", "b#one", "b#two", "b#three", "c#solo"] {
            layout.mark_empty(key);
        }
        assert!(
            layout
                .layout(&mut view, (1280., 800.), &dimensions)
                .is_empty()
        );
    }
    #[test]
    fn resize_and_measured_height_preserve_center_artwork_point() {
        let mut layout = RoamLayout::new(&style(), &BTreeMap::new()).unwrap();
        let mut view = home((1280., 800.));
        layout.layout(&mut view, (1280., 800.), &BTreeMap::new());
        let before = layout.anchor(view).unwrap();
        let spec = layout.slot(before.column, before.row);
        let dimensions = BTreeMap::from([(spec.key, (800., 1600.))]);
        layout.layout(&mut view, (1600., 1000.), &dimensions);
        let after = layout.anchor(view).unwrap();
        assert_eq!((before.column, before.row), (after.column, after.row));
        assert!((before.fraction - after.fraction).abs() < 0.0001);
        assert!((before.column_fraction - after.column_fraction).abs() < 0.0001);
    }
}
