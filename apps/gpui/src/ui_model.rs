//! Renderer-independent desktop state: wall identity, geometry and playback.
//! The production view and native input tests use these same transformations.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }
    pub fn transformed(self, v: Viewport) -> Self {
        Self {
            x: self.x * v.zoom + v.x,
            y: self.y * v.zoom + v.y,
            w: self.w * v.zoom,
            h: self.h * v.zoom,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            zoom: 1.,
        }
    }
}
impl Viewport {
    pub fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        let next = (self.zoom * factor).clamp(0.15, 6.);
        let ratio = next / self.zoom;
        self.x = x - (x - self.x) * ratio;
        self.y = y - (y - self.y) * ratio;
        self.zoom = next;
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Hit {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub w: f32,
    #[serde(default)]
    pub h: f32,
    #[serde(default)]
    pub k: Option<String>,
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub d: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artwork {
    pub w: f32,
    pub h: f32,
    pub svg: String,
    #[serde(default)]
    pub hits: Vec<Hit>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub bg: String,
    #[serde(default)]
    pub motion: Option<Value>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileSpec {
    pub key: String,
    pub piece: String,
    pub variant: Option<String>,
    pub span: usize,
}

pub fn default_variant(piece: &Value) -> Option<String> {
    piece["defaultVariant"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            piece["variants"].as_array()?.first()?["id"]
                .as_str()
                .map(str::to_owned)
        })
}
/// Match desktop wallTiles: original board first, then every valid layout, once.
pub fn wall_tiles(
    style: &Value,
    selected: &BTreeMap<String, String>,
    order: &[String],
) -> Vec<TileSpec> {
    let pieces = style["pieces"].as_array().cloned().unwrap_or_default();
    let board = style["board"].as_array().cloned().unwrap_or_else(|| {
        pieces
            .iter()
            .filter(|p| p["requires"].as_array().is_none_or(Vec::is_empty))
            .map(|p| serde_json::json!({"id":p["id"]}))
            .collect()
    });
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |entry: &Value, variant: Option<String>| {
        let Some(id) = entry["id"].as_str() else {
            return;
        };
        let key = format!("{}#{}", id, variant.as_deref().unwrap_or(""));
        if seen.insert(key.clone()) {
            out.push(TileSpec {
                variant: selected.get(&key).cloned().or(variant),
                key,
                piece: id.into(),
                span: entry["span"].as_u64().unwrap_or(0).min(2) as usize,
            });
        }
    };
    for entry in &board {
        let Some(p) = pieces.iter().find(|p| p["id"] == entry["id"]) else {
            continue;
        };
        add(
            entry,
            entry["variant"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| default_variant(p)),
        );
    }
    for entry in &board {
        if let Some(p) = pieces.iter().find(|p| p["id"] == entry["id"]) {
            for v in p["variants"].as_array().into_iter().flatten() {
                add(entry, v["id"].as_str().map(str::to_owned));
            }
        }
    }
    if !order.is_empty() {
        out.sort_by_key(|t| {
            order
                .iter()
                .position(|key| key == &t.key)
                .unwrap_or(usize::MAX)
        });
    }
    out
}
/// Selecting a layout swaps with its existing peer, preserving the full wall.
pub fn swap_variant(
    tiles: &[TileSpec],
    selected: &mut BTreeMap<String, String>,
    key: &str,
    variant: &str,
) -> bool {
    let Some(tile) = tiles.iter().find(|t| t.key == key) else {
        return false;
    };
    if tile.variant.as_deref() == Some(variant) {
        return false;
    }
    if let Some(peer) = tiles
        .iter()
        .find(|t| t.piece == tile.piece && t.variant.as_deref() == Some(variant))
    {
        selected.insert(peer.key.clone(), tile.variant.clone().unwrap_or_default());
    }
    selected.insert(key.into(), variant.into());
    true
}
pub fn masonry(
    tiles: &[TileSpec],
    dimensions: &BTreeMap<String, (f32, f32)>,
    width: f32,
) -> (Vec<Rect>, f32) {
    // Same sizes, column limits, span preference and hole cost as main.ts layout.
    let available = (width - 120.).clamp(120., 1680.);
    let gap = 40.;
    let cell = available
        .min(360.)
        .min(((available - gap * 2.) / 3.).floor().max(300.));
    let columns = (((available + gap) / (cell + gap)).floor() as usize).clamp(1, 4);
    let board_width = cell * columns as f32 + gap * (columns - 1) as f32;
    let left = (width - board_width) / 2.;
    let mut heights = vec![0.; columns];
    let mut out = vec![Rect::default(); tiles.len()];
    let mut queue = (0..tiles.len()).collect::<Vec<_>>();
    while !queue.is_empty() {
        let mut chosen = (0, 0, 1, 0., 0., 0., 0.);
        let mut best_score = f32::INFINITY;
        for (position, index) in queue.iter().copied().enumerate() {
            let tile = &tiles[index];
            let (aw, ah) = dimensions.get(&tile.key).copied().unwrap_or((640., 480.));
            let aw = aw.max(1.);
            let span = if columns >= 3 && aw >= 500. {
                if tile.span > 0 {
                    tile.span
                } else if aw / ah.max(1.) > 1.7 {
                    2
                } else {
                    1
                }
            } else {
                1
            }
            .min(columns);
            let full = cell * span as f32 + gap * (span - 1) as f32;
            let w = full.min(aw * if aw < 500. { 1.15 } else { 1. });
            let h = w / aw * ah;
            let mut placement = (0, f32::INFINITY, f32::INFINITY);
            for col in 0..=columns - span {
                let y = heights[col..col + span].iter().copied().fold(0., f32::max);
                let hole = heights[col..col + span]
                    .iter()
                    .map(|height| y - *height)
                    .fold(0., f32::max);
                if y + hole < placement.1 + placement.2 - 1. {
                    placement = (col, y, hole);
                }
            }
            let score = placement.1
                + placement.2
                + position as f32 * 60.
                + if span > 1 && placement.2 > 40. {
                    2000.
                } else {
                    0.
                };
            if score < best_score - 1. {
                best_score = score;
                chosen = (position, placement.0, span, placement.1, full, w, h);
            }
        }
        let (position, col, span, y, full, w, h) = chosen;
        let index = queue.remove(position);
        out[index] = Rect {
            x: left + col as f32 * (cell + gap) + (full - w) / 2.,
            y: 24. + y,
            w,
            h,
        };
        for height in &mut heights[col..col + span] {
            *height = y + h + gap + 22.;
        }
    }
    (out, heights.into_iter().fold(0., f32::max) + 100.)
}

pub fn texture_width(logical: f32, dpi: f32) -> u32 {
    let target = (logical * dpi).ceil().max(128.) as u32;
    [256, 512, 768, 1024, 1536, 2048, 3072, 4096]
        .into_iter()
        .find(|w| *w >= target)
        .unwrap_or(4096)
}
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub hold_ms: u64,
    #[serde(default)]
    pub fps: u32,
    #[serde(default, rename = "loop")]
    pub repeat: bool,
}
pub fn frame_at(times: &[f64], timeline: &Timeline, elapsed_ms: u64) -> usize {
    if times.is_empty() {
        return 0;
    }
    if timeline.duration_ms == 0 {
        return times.len() - 1;
    }
    let cycle = timeline.duration_ms.saturating_add(timeline.hold_ms).max(1);
    let time = if timeline.repeat {
        elapsed_ms % cycle
    } else {
        elapsed_ms
    };
    let t = (time as f64 / timeline.duration_ms as f64).min(1.);
    times
        .partition_point(|v| *v <= t)
        .saturating_sub(1)
        .min(times.len() - 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn wall_preserves_default_and_all_layouts() {
        let s = json!({"board":[{"id":"tier","span":2},{"id":"badge","variant":"stack"}],"pieces":[{"id":"tier","defaultVariant":"center","variants":[{"id":"rows"},{"id":"center"}]},{"id":"badge","defaultVariant":"flat","variants":[{"id":"flat"},{"id":"stack"}]}]});
        let t = wall_tiles(&s, &BTreeMap::new(), &[]);
        assert_eq!(
            t.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(),
            vec!["tier#center", "badge#stack", "tier#rows", "badge#flat"]
        );
        let mut selected = BTreeMap::new();
        assert!(swap_variant(&t, &mut selected, "tier#center", "rows"));
        let swapped = wall_tiles(&s, &selected, &[]);
        assert_eq!(swapped[0].variant.as_deref(), Some("rows"));
        assert_eq!(swapped[2].variant.as_deref(), Some("center"));
    }
    #[test]
    fn zoom_keeps_pointer_world_coordinate() {
        let mut v = Viewport {
            x: 100.,
            y: -30.,
            zoom: 0.7,
        };
        let point = ((240. - v.x) / v.zoom, (400. - v.y) / v.zoom);
        v.zoom_at(1.8, 240., 400.);
        assert!(((240. - v.x) / v.zoom - point.0).abs() < 0.001);
        assert!(((400. - v.y) / v.zoom - point.1).abs() < 0.001);
    }
    #[test]
    fn layout_has_no_overlaps_for_mixed_spans() {
        let t = (0..35)
            .map(|i| TileSpec {
                key: i.to_string(),
                piece: "tier".into(),
                variant: None,
                span: if i % 5 == 0 { 2 } else { 1 },
            })
            .collect::<Vec<_>>();
        let (r, _) = masonry(&t, &BTreeMap::new(), 1180.);
        for (i, a) in r.iter().enumerate() {
            for b in &r[i + 1..] {
                assert!(!a.intersects(*b));
            }
        }
    }
    #[test]
    fn timeline_uses_duration_and_holds_last_frame() {
        let t = Timeline {
            duration_ms: 2000,
            hold_ms: 1000,
            fps: 20,
            repeat: true,
        };
        assert_eq!(frame_at(&[0., 0.5, 1.], &t, 1500), 1);
        assert_eq!(frame_at(&[0., 0.5, 1.], &t, 2500), 2);
        assert_eq!(frame_at(&[0., 0.5, 1.], &t, 3100), 0);
    }
}
