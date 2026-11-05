//! Scale-aware native artwork previews. SVG layout/fonts are supplied by the
//! shared artwork engine; resvg handles filters and textures off the UI thread.
use crate::renderer::Cancellation;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

const CACHE_BUDGET: usize = 64 * 1024 * 1024;
const PIXEL_LIMIT: u64 = 32_000_000;
const EDGE_LIMIT: u32 = 32768;
const SVG_LIMIT: usize = 64 * 1024 * 1024;

pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

struct Entry {
    svg: String,
    width: u32,
    region: Option<[u32; 4]>,
    image: Arc<RasterImage>,
    cost: usize,
}

#[derive(Default)]
struct Cache { entries: VecDeque<Entry>, bytes: usize }
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
// One bounded raster operation at a time prevents many visible tiles each
// allocating their maximum pixmap concurrently. Callers queue on a worker.
static RASTER: Mutex<()> = Mutex::new(());

pub fn clear_cache() {
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache.entries.clear();
    cache.bytes = 0;
}

pub fn cache_stats() -> (usize, usize) {
    let cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    (cache.entries.len(), cache.bytes)
}

fn cached(svg: &str, width: u32, region: Option<[u32; 4]>) -> Option<Arc<RasterImage>> {
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let index = cache.entries.iter().position(|entry| entry.width == width && entry.region == region && entry.svg == svg)?;
    let entry = cache.entries.remove(index)?;
    let result = entry.image.clone();
    cache.entries.push_back(entry);
    Some(result)
}

/// Render at a caller-selected physical pixel width. Cache ownership counts
/// source bytes, encoded PNG and estimated decoded GPU/image bytes. Eviction
/// releases the cache's reference; the view must also release invisible images.
pub fn rasterize(svg: &str, pixel_width: u32, cancellation: &Cancellation) -> Result<Arc<RasterImage>, String> {
    rasterize_view(svg, pixel_width, None, cancellation)
}

/// Render only a viewport of a tall/deeply zoomed artwork. Region coordinates
/// are original artwork pixels, not SVG viewBox user units. Source geometry is
/// still parsed once, but the pixmap/texture covers only this visible region.
pub fn rasterize_region(svg: &str, region: (f32, f32, f32, f32), pixel_width: u32, cancellation: &Cancellation) -> Result<Arc<RasterImage>, String> {
    let (x, y, width, height) = region;
    if ![x, y, width, height].iter().all(|value| value.is_finite()) || width <= 0.0 || height <= 0.0 {
        return Err("Invalid preview region".into());
    }
    rasterize_view(svg, pixel_width, Some(region), cancellation)
}

fn rasterize_view(svg: &str, pixel_width: u32, region: Option<(f32, f32, f32, f32)>, cancellation: &Cancellation) -> Result<Arc<RasterImage>, String> {
    if cancellation.cancelled() { return Err("Preview cancelled".into()); }
    if svg.len() > SVG_LIMIT { return Err("Preview SVG exceeds 64 MiB".into()); }
    if pixel_width == 0 || pixel_width > EDGE_LIMIT { return Err("Invalid preview width".into()); }
    let key = region.map(|(x, y, w, h)| [x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits()]);
    if let Some(image) = cached(svg, pixel_width, key) { return Ok(image); }
    // try_lock permits cancellation while waiting for another tile's filters.
    let _guard = loop {
        if cancellation.cancelled() { return Err("Preview cancelled".into()); }
        match RASTER.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(8)),
        }
    };
    if let Some(image) = cached(svg, pixel_width, key) { return Ok(image); }
    let mut options = resvg::usvg::Options::default();
    // Artwork is self-contained. Never resolve embedded local or network paths.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|error| format!("Invalid preview SVG: {error}"))?;
    let (x, y, width, height) = region.unwrap_or((0.0, 0.0, tree.size().width(), tree.size().height()));
    let output_height = (f64::from(height) * f64::from(pixel_width) / f64::from(width)).ceil();
    if !output_height.is_finite() || output_height < 1.0 || output_height > f64::from(EDGE_LIMIT) || f64::from(pixel_width) * output_height > PIXEL_LIMIT as f64 {
        return Err("Preview exceeds 32 million pixels; use a smaller scale or a visible crop".into());
    }
    let output_height = output_height as u32;
    if cancellation.cancelled() { return Err("Preview cancelled".into()); }
    let mut pixels = resvg::tiny_skia::Pixmap::new(pixel_width, output_height).ok_or("Cannot allocate preview pixels")?;
    let scale = pixel_width as f32 / width;
    if ![scale, -x * scale, -y * scale].iter().all(|value| value.is_finite()) { return Err("Preview region transform is out of range".into()); }
    let transform = resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, -x * scale, -y * scale);
    resvg::render(&tree, transform, &mut pixels.as_mut());
    if cancellation.cancelled() { return Err("Preview cancelled".into()); }
    let png = pixels.encode_png().map_err(|error| format!("Cannot encode preview: {error}"))?;
    let cost = svg.len().saturating_add(png.len()).saturating_add(pixels.data().len());
    let image = Arc::new(RasterImage { width: pixel_width, height: output_height, png });
    if cost <= CACHE_BUDGET {
        let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        while cache.bytes.saturating_add(cost) > CACHE_BUDGET {
            let Some(old) = cache.entries.pop_front() else { break; };
            cache.bytes = cache.bytes.saturating_sub(old.cost);
        }
        cache.bytes += cost;
        cache.entries.push_back(Entry { svg: svg.into(), width: pixel_width, region: key, image: image.clone(), cost });
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scales_preserves_transparency_and_reuses_exact_cache_entry() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect x="20" y="10" width="40" height="20" fill="#ff0000"/></svg>"##;
        let image = rasterize(svg, 160, &Cancellation::default()).unwrap();
        assert_eq!((image.width, image.height), (160, 80));
        let pixels = resvg::tiny_skia::Pixmap::decode_png(&image.png).unwrap();
        assert_eq!(pixels.pixel(0, 0).unwrap().alpha(), 0);
        assert_eq!(pixels.pixel(80, 40).unwrap().red(), 255);
        let again = rasterize(svg, 160, &Cancellation::default()).unwrap();
        assert!(Arc::ptr_eq(&image, &again));
        assert!(!Arc::ptr_eq(&image, &rasterize(svg, 80, &Cancellation::default()).unwrap()));
    }
    #[test]
    fn rejects_excessive_dimensions_and_cancelled_work_before_allocation() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="100000"/>"#;
        assert!(rasterize(svg, 1000, &Cancellation::default()).is_err());
        assert!(rasterize(svg, 0, &Cancellation::default()).is_err());
        let cancel = Cancellation::default(); cancel.cancel();
        assert_eq!(rasterize("invalid", 100, &cancel).err().unwrap(), "Preview cancelled");
    }
    #[test]
    fn ignores_external_image_paths() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><image width="10" height="10" xlink:href="/etc/passwd"/></svg>"#;
        let image = rasterize(svg, 10, &Cancellation::default()).unwrap();
        let pixels = resvg::tiny_skia::Pixmap::decode_png(&image.png).unwrap();
        assert_eq!(pixels.pixel(5, 5).unwrap().alpha(), 0);
    }
    #[test]
    fn crops_tall_artwork_in_original_coordinates_without_allocating_the_whole_image() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="100000"><rect y="50000" width="80" height="40" fill="#ff0000"/></svg>"##;
        let image = rasterize_region(svg, (0.0, 50000.0, 80.0, 40.0), 160, &Cancellation::default()).unwrap();
        assert_eq!((image.width, image.height), (160, 80));
        let pixels = resvg::tiny_skia::Pixmap::decode_png(&image.png).unwrap();
        assert_eq!(pixels.pixel(80, 40).unwrap().red(), 255);
        let above = rasterize_region(svg, (0.0, 49960.0, 80.0, 40.0), 160, &Cancellation::default()).unwrap();
        assert!(!Arc::ptr_eq(&image, &above));
        assert_eq!(resvg::tiny_skia::Pixmap::decode_png(&above.png).unwrap().pixel(80, 40).unwrap().alpha(), 0);
        assert!(rasterize_region(svg, (0.0, f32::NAN, 80.0, 40.0), 160, &Cancellation::default()).is_err());
    }
}
