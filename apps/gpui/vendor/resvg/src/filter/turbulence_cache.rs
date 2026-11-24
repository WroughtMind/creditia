// Creditia patch; upstream turbulence sampling is unchanged.
use super::{ImageRefMut, apply_uncached};
use rgb::RGBA8;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

// Blocks bound a cache miss and survive changes to animated filter bounds.
const EDGE: usize = 64;
const BYTES: usize = EDGE * EDGE * 4;
static BUDGET: AtomicUsize = AtomicUsize::new(0);
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Key {
    values: [u64; 6], // scale, frequency and subpixel origin
    octaves: u32,
    seed: i32,
    fractal: bool,
    x: i64,
    y: i64,
}
struct Entry {
    pixels: Arc<[RGBA8]>,
    used: u64,
}
#[derive(Default)]
struct Cache {
    entries: HashMap<Key, Entry>,
    clock: u64,
}
impl Cache {
    fn get(&mut self, key: &Key) -> Option<Arc<[RGBA8]>> {
        let entry = self.entries.get_mut(key)?;
        self.clock = self.clock.wrapping_add(1);
        entry.used = self.clock;
        Some(entry.pixels.clone())
    }
    fn trim(&mut self, budget: usize) {
        while self.entries.len().saturating_mul(BYTES) > budget {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.entries.remove(&key);
        }
    }
    fn insert(&mut self, key: Key, pixels: Arc<[RGBA8]>, budget: usize) -> Arc<[RGBA8]> {
        if let Some(old) = self.get(&key) {
            return old;
        }
        if budget < BYTES {
            return pixels;
        }
        self.trim(budget - BYTES);
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(
            key,
            Entry {
                pixels: pixels.clone(),
                used: self.clock,
            },
        );
        pixels
    }
}

pub(crate) fn set_budget(bytes: usize) {
    if BUDGET.swap(bytes, Ordering::Relaxed) != bytes {
        CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .trim(bytes);
    }
}
pub(crate) fn clear_cache() {
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entries
        .clear();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    offset_x: f64,
    offset_y: f64,
    sx: f64,
    sy: f64,
    fx: f64,
    fy: f64,
    octaves: u32,
    seed: i32,
    fractal: bool,
    dest: &mut ImageRefMut,
) -> bool {
    let budget = BUDGET.load(Ordering::Relaxed);
    if budget < BYTES
        || ![offset_x, offset_y, sx, sy, fx, fy]
            .iter()
            .all(|v| v.is_finite())
        || offset_x.abs() > i32::MAX as f64
        || offset_y.abs() > i32::MAX as f64
    {
        return false;
    }
    let origin_x = offset_x.floor() as i64;
    let origin_y = offset_y.floor() as i64;
    let phase_x = offset_x - origin_x as f64;
    let phase_y = offset_y - origin_y as f64;
    let values = [
        sx.to_bits(),
        sy.to_bits(),
        fx.to_bits(),
        fy.to_bits(),
        phase_x.to_bits(),
        phase_y.to_bits(),
    ];
    let cache = CACHE.get_or_init(Default::default);
    let mut row = 0;
    while row < dest.height as usize {
        let world_y = origin_y + row as i64;
        let tile_y = world_y.div_euclid(EDGE as i64) * EDGE as i64;
        let top = (world_y - tile_y) as usize;
        let rows = (EDGE - top).min(dest.height as usize - row);
        let mut column = 0;
        while column < dest.width as usize {
            let world_x = origin_x + column as i64;
            let tile_x = world_x.div_euclid(EDGE as i64) * EDGE as i64;
            let key = Key {
                values,
                octaves,
                seed,
                fractal,
                x: tile_x,
                y: tile_y,
            };
            let cached = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key);
            let pixels = if let Some(pixels) = cached {
                pixels
            } else {
                // Compute outside the lock so disjoint native strips can run
                // concurrently. The original noise function owns every sample.
                let mut pixels = vec![RGBA8::new(0, 0, 0, 0); EDGE * EDGE];
                apply_uncached(
                    tile_x as f64 + phase_x,
                    tile_y as f64 + phase_y,
                    sx,
                    sy,
                    fx,
                    fy,
                    octaves,
                    seed,
                    false,
                    fractal,
                    ImageRefMut {
                        width: EDGE as u32,
                        height: EDGE as u32,
                        data: &mut pixels,
                    },
                );
                let pixels: Arc<[RGBA8]> = pixels.into();
                cache.lock().unwrap_or_else(|e| e.into_inner()).insert(
                    key,
                    pixels,
                    BUDGET.load(Ordering::Relaxed),
                )
            };
            let left = (world_x - tile_x) as usize;
            let count = (EDGE - left).min(dest.width as usize - column);
            for dy in 0..rows {
                let at = (row + dy) * dest.width as usize + column;
                let from = (top + dy) * EDGE + left;
                dest.data[at..at + count].copy_from_slice(&pixels[from..from + count]);
            }
            column += count;
        }
        row += rows;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pixel_budget_evicts_oldest_blocks_and_equal_keys_share_pixels() {
        let mut cache = Cache::default();
        let key = Key {
            values: [0; 6],
            octaves: 3,
            seed: 7,
            fractal: true,
            x: 0,
            y: 0,
        };
        let pixels: Arc<[RGBA8]> = vec![RGBA8::new(1, 2, 3, 4); EDGE * EDGE].into();
        cache.insert(key, pixels.clone(), BYTES * 2);
        cache.insert(Key { x: 64, ..key }, pixels.clone(), BYTES * 2);
        assert!(Arc::ptr_eq(&cache.get(&key).unwrap(), &pixels));
        cache.insert(Key { x: 128, ..key }, pixels, BYTES * 2);
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get(&Key { x: 64, ..key }).is_none());
    }
    #[test]
    fn changing_bounds_and_fractional_scales_preserve_every_noise_sample() {
        set_budget(8 * 1024 * 1024);
        for (x, y, w, h, scale, seed) in [
            (0., 0., 131, 97, 2., 7),
            (-17.25, 31.5, 155, 105, 1.25, 19),
            (46.75, -80.5, 89, 141, 1.25, 19),
            (0., 0., 67, 99, 2., 41),
        ] {
            let mut expected = vec![RGBA8::new(0, 0, 0, 0); w * h];
            let mut actual = expected.clone();
            apply_uncached(
                x,
                y,
                scale,
                scale,
                0.012,
                0.065,
                3,
                seed,
                false,
                true,
                ImageRefMut {
                    width: w as u32,
                    height: h as u32,
                    data: &mut expected,
                },
            );
            assert!(apply(
                x,
                y,
                scale,
                scale,
                0.012,
                0.065,
                3,
                seed,
                true,
                &mut ImageRefMut {
                    width: w as u32,
                    height: h as u32,
                    data: &mut actual
                }
            ));
            assert_eq!(actual, expected);
        }
    }
}
