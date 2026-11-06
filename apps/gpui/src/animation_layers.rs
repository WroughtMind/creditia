//! Cache stationary Satori shadows while streaming a changing scene. Each
//! shadow is rendered by resvg with the original transform, before subsequent
//! paint operations. Other SVGs use the ordinary full-tree path.
use resvg::{tiny_skia, usvg};
use std::collections::{HashMap, VecDeque};

const BUDGET: usize = 48 * 1024 * 1024;
const FULL_LAYER_PIXEL_LIMIT: u64 = 16 * 1024 * 1024;
const FULL_LAYER_CACHE_BUDGET: usize = 8 * 1024 * 1024;

struct Span {
    x: u32,
    end: u32,
    rgba: [u8; 4],
}

struct ShadowRows {
    rows: Vec<Vec<Span>>,
    bytes: usize,
}

impl ShadowRows {
    // A masked shadow is transparent across most of its artwork, and usually
    // has long runs of identical premultiplied pixels along the straight edges.
    fn encode(pixels: &tiny_skia::Pixmap) -> Option<Self> {
        let mut rows = Vec::with_capacity(pixels.height() as usize);
        let mut bytes = rows.capacity() * size_of::<Vec<Span>>();
        for line in pixels.data().chunks_exact(pixels.width() as usize * 4) {
            let mut row: Vec<Span> = Vec::new();
            for (index, color) in line.chunks_exact(4).enumerate() {
                if color[3] == 0 {
                    continue;
                }
                let x = index as u32;
                if let Some(last) = row.last_mut()
                    && last.end == x
                    && last.rgba == color
                {
                    last.end += 1;
                } else {
                    row.push(Span {
                        x,
                        end: x + 1,
                        rgba: color.try_into().unwrap(),
                    });
                }
            }
            bytes += row.capacity() * size_of::<Span>();
            if bytes > FULL_LAYER_CACHE_BUDGET {
                return None;
            }
            rows.push(row);
        }
        Some(Self { rows, bytes })
    }
    fn crop(&self, x: u32, y: u32, output: &mut tiny_skia::Pixmap) {
        let width = output.width();
        for (row, line) in output
            .data_mut()
            .chunks_exact_mut(width as usize * 4)
            .enumerate()
        {
            for run in &self.rows[y as usize + row] {
                let start = run.x.max(x);
                let end = run.end.min(x + width);
                if start >= end {
                    continue;
                }
                for pixel in
                    line[((start - x) * 4) as usize..((end - x) * 4) as usize].chunks_exact_mut(4)
                {
                    pixel.copy_from_slice(&run.rgba);
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct LayerCache {
    layers: VecDeque<(String, tiny_skia::Transform, tiny_skia::Pixmap)>,
    bytes: usize,
    full_layers: VecDeque<(String, tiny_skia::Transform, ShadowRows)>,
    full_bytes: usize,
    oversized: VecDeque<(u64, tiny_skia::Transform)>,
}

fn shadow_hash(xml: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hash = DefaultHasher::new();
    xml.hash(&mut hash);
    hash.finish()
}

fn definition(tag: &str) -> bool {
    matches!(
        tag,
        "defs" | "mask" | "clipPath" | "linearGradient" | "radialGradient" | "pattern" | "filter"
    )
}

fn url_id(value: &str) -> Option<&str> {
    value.strip_prefix("url(#")?.strip_suffix(')')
}

enum Layer {
    Paint(String),
    Shadow(String),
}

fn layers(svg: &str) -> Option<Vec<Layer>> {
    if svg.len() > 2 * 1024 * 1024 || !svg.contains("filter=\"url(#s") {
        return None;
    }
    let mut opts = roxmltree::ParsingOptions::default();
    opts.nodes_limit = 100_000;
    let doc = roxmltree::Document::parse_with_options(svg, opts).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg"
        || root.attributes().any(|a| {
            !matches!(
                a.name(),
                "width" | "height" | "viewBox" | "preserveAspectRatio"
            )
        })
        || root
            .descendants()
            .any(|n| n.has_tag_name("style") || n.has_tag_name("script"))
    {
        return None;
    }
    let open = &svg[root.range().start..=root.range().start + svg[root.range()].find('>')?];
    let mut defs = String::new();
    let mut ids = HashMap::new();
    for child in root
        .children()
        .filter(|c| c.is_element() && definition(c.tag_name().name()))
    {
        defs.push_str(&svg[child.range()]);
        for node in child.descendants().filter(|c| c.is_element()) {
            if let Some(id) = node.attribute("id") {
                if ids.insert(id, node).is_some() {
                    return None;
                }
            }
        }
    }
    // A reference to a painted element in another section would disappear
    // when splitting the SVG. Keep that artwork on the full-tree renderer.
    for node in root.descendants().filter(|n| n.is_element()) {
        for attribute in node.attributes() {
            let value = attribute.value();
            if attribute.name() == "href"
                && value.starts_with('#')
                && !ids.contains_key(&value[1..])
            {
                return None;
            }
            for reference in value.split("url(#").skip(1) {
                if !ids.contains_key(reference.split_once(')')?.0) {
                    return None;
                }
            }
        }
    }
    if root
        .children()
        .filter(|n| n.attribute("filter").is_some())
        .count()
        > 4
    {
        return None;
    }
    let mut sections = Vec::new();
    let mut paint = String::new();
    let flush = |paint: &mut String, out: &mut Vec<Layer>| {
        if !paint.is_empty() {
            out.push(Layer::Paint(format!("{open}{defs}{paint}</svg>")));
            paint.clear();
        }
    };
    for child in root
        .children()
        .filter(|n| n.is_element() && !definition(n.tag_name().name()))
    {
        let source = &svg[child.range()];
        let shadow = (|| {
            if !child.has_tag_name("g")
                || child
                    .attributes()
                    .any(|a| !matches!(a.name(), "mask" | "filter"))
                || child
                    .children()
                    .filter(|n| n.is_element())
                    .any(|n| !n.has_tag_name("path"))
            {
                return None;
            }
            let filter = url_id(child.attribute("filter")?)?;
            let mask = url_id(child.attribute("mask")?)?;
            let prefix = filter.split_once('_')?.0;
            // Satori increments this namespace in base 36 (s9, sa, ...).
            // A decimal-only match silently rerenders shadows after nine SVGs.
            let serial = prefix.strip_prefix('s')?;
            if serial.is_empty()
                || !serial
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
                || mask.split_once('_')?.0 != prefix
            {
                return None;
            }
            let filter_node = *ids.get(filter)?;
            let mask_node = *ids.get(mask)?;
            if !filter_node.has_tag_name("filter") || !mask_node.has_tag_name("mask") {
                return None;
            }
            let filter_xml = &svg[filter_node.range()];
            let mask_xml = &svg[mask_node.range()];
            // Only self-contained masks and filters may be reused. Other
            // references, blending, or inherited definitions need the full tree.
            if [filter_xml, mask_xml]
                .iter()
                .any(|s| s.contains("url(") || s.contains("href="))
                || source.matches("url(").count() != 2
                || source.contains("href=")
            {
                return None;
            }
            let xml = format!("{open}<defs>{mask_xml}{filter_xml}</defs>{source}</svg>");
            Some(
                xml.replace(&format!("id=\"{prefix}_"), "id=\"layer_")
                    .replace(&format!("url(#{prefix}_"), "url(#layer_"),
            )
        })();
        if let Some(shadow) = shadow {
            flush(&mut paint, &mut sections);
            sections.push(Layer::Shadow(shadow));
        } else {
            paint.push_str(source);
        }
    }
    if sections.is_empty() {
        return None;
    }
    flush(&mut paint, &mut sections);
    Some(sections)
}

impl LayerCache {
    // Preserve the artwork's physical pixel grid across cropped tiles and
    // panning. resvg otherwise runs the full-art blur once for *each* tile.
    fn crop_shadow(
        &mut self,
        xml: &str,
        tree: &usvg::Tree,
        transform: tiny_skia::Transform,
        tile: &mut tiny_skia::Pixmap,
    ) -> bool {
        if transform.kx != 0.
            || transform.ky != 0.
            || transform.sx <= 0.
            || transform.sx != transform.sy
        {
            return false;
        }
        // FramePlan derives these logical coordinates from integral physical
        // pixel bounds. Undo only f32 round-off; genuinely fractional transforms
        // continue to use a tile-local render.
        let integer = |value: f32| {
            let rounded = value.round();
            ((value - rounded).abs() <= (value.abs() * f32::EPSILON * 2.).max(0.00001))
                .then_some(rounded)
        };
        let (Some(full_width), Some(x), Some(y)) = (
            integer(tree.size().width() * transform.sx),
            integer(-transform.tx),
            integer(-transform.ty),
        ) else {
            return false;
        };
        let scale = full_width / tree.size().width();
        let width = full_width as u32;
        let height = (tree.size().height() * scale).ceil() as u32;
        if x < 0.
            || y < 0.
            || u64::from(width) * u64::from(height) > FULL_LAYER_PIXEL_LIMIT
            || x as u64 + u64::from(tile.width()) > u64::from(width)
            || y as u64 + u64::from(tile.height()) > u64::from(height)
            || (tile.width() == width && tile.height() == height)
        {
            return false;
        }
        let (x, y) = (x as u32, y as u32);
        let world = tiny_skia::Transform::from_scale(scale, scale);
        if !self.oversized.is_empty() && self.oversized.contains(&(shadow_hash(xml), world)) {
            return false;
        }
        let cached = self
            .full_layers
            .iter()
            .position(|(s, t, _)| s == xml && *t == world);
        let entry = if let Some(index) = cached {
            self.full_layers.remove(index).unwrap()
        } else {
            let Some(mut full) = tiny_skia::Pixmap::new(width, height) else {
                return false;
            };
            resvg::render(tree, world, &mut full.as_mut());
            let Some(rows) = ShadowRows::encode(&full) else {
                // At most four filtered groups are eligible per artwork.
                self.oversized.push_back((shadow_hash(xml), world));
                if self.oversized.len() > 4 {
                    self.oversized.pop_front();
                }
                let stride = tile.width() as usize * 4;
                for (row, line) in tile.data_mut().chunks_exact_mut(stride).enumerate() {
                    let start = ((y as usize + row) * width as usize + x as usize) * 4;
                    line.copy_from_slice(&full.data()[start..start + stride]);
                }
                return true;
            };
            self.full_bytes += rows.bytes + xml.len();
            (xml.to_string(), world, rows)
        };
        entry.2.crop(x, y, tile);
        self.full_layers.push_back(entry);
        while self.full_bytes > FULL_LAYER_CACHE_BUDGET {
            let (xml, _, rows) = self.full_layers.pop_front().unwrap();
            self.full_bytes -= xml.len() + rows.bytes;
        }
        true
    }
    pub(crate) fn render(
        &mut self,
        svg: &str,
        options: &usvg::Options,
        transform: tiny_skia::Transform,
        output: &mut tiny_skia::Pixmap,
    ) -> bool {
        let Some(layers) = layers(svg) else {
            return false;
        };
        for layer in layers {
            match layer {
                Layer::Paint(xml) => {
                    let Ok(tree) = usvg::Tree::from_str(&xml, options) else {
                        return false;
                    };
                    resvg::render(&tree, transform, &mut output.as_mut());
                }
                Layer::Shadow(xml) => {
                    let same = self.layers.iter().position(|(s, t, p)| {
                        s == &xml
                            && *t == transform
                            && p.width() == output.width()
                            && p.height() == output.height()
                    });
                    let entry = if let Some(index) = same {
                        self.layers.remove(index).unwrap()
                    } else {
                        let Ok(tree) = usvg::Tree::from_str(&xml, options) else {
                            return false;
                        };
                        let Some(mut pix) = tiny_skia::Pixmap::new(output.width(), output.height())
                        else {
                            return false;
                        };
                        if !self.crop_shadow(&xml, &tree, transform, &mut pix) {
                            resvg::render(&tree, transform, &mut pix.as_mut());
                        }
                        self.bytes += pix.data().len() + xml.len();
                        (xml, transform, pix)
                    };
                    output.draw_pixmap(
                        0,
                        0,
                        entry.2.as_ref(),
                        &tiny_skia::PixmapPaint::default(),
                        tiny_skia::Transform::identity(),
                        None,
                    );
                    self.layers.push_back(entry);
                    while self.bytes + self.full_bytes > BUDGET {
                        let (xml, _, pix) = self.layers.pop_front().unwrap();
                        self.bytes -= pix.data().len() + xml.len();
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panning_reads_exact_pixels_of_the_full_size_shadow_without_refiltering() {
        let xml = r##"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="60"><defs><filter id="blur" filterUnits="userSpaceOnUse" width="96" height="60" x="0" y="0"><feGaussianBlur stdDeviation="3"/></filter></defs><path filter="url(#blur)" fill="#369" d="M21 11h46v31H21z"/></svg>"##;
        let tree = usvg::Tree::from_str(xml, &usvg::Options::default()).unwrap();
        let mut full = tiny_skia::Pixmap::new(240, 150).unwrap();
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(2.5, 2.5),
            &mut full.as_mut(),
        );
        let mut cache = LayerCache::default();
        for (x, y) in [(13, 8), (37, 21), (48, 30)] {
            let mut tile = tiny_skia::Pixmap::new(80, 36).unwrap();
            let tf = tiny_skia::Transform::from_row(2.5, 0., 0., 2.5, -(x as f32), -(y as f32));
            assert!(cache.crop_shadow(xml, &tree, tf, &mut tile));
            for (row, actual) in tile.data().chunks_exact(80 * 4).enumerate() {
                let start = ((y + row) * 240 + x) * 4;
                assert_eq!(actual, &full.data()[start..start + 80 * 4]);
            }
            assert_eq!(cache.full_layers.len(), 1);
            assert!(cache.full_bytes <= FULL_LAYER_CACHE_BUDGET);
        }
        let fractional = tiny_skia::Transform::from_row(2.5, 0., 0., 2.5, -4.25, -3.);
        assert!(!cache.crop_shadow(
            xml,
            &tree,
            fractional,
            &mut tiny_skia::Pixmap::new(10, 10).unwrap()
        ));
    }

    #[test]
    fn moving_content_and_changing_shadows_match_unsplit_pixels() {
        let mut cache = LayerCache::default();
        let options = usvg::Options::default();
        let transform = tiny_skia::Transform::from_row(2., 0., 0., 2., -8., -6.);
        for (namespace, x, blur, expected) in [("9", 3, 2, 1), ("a", 10, 2, 1), ("1z", 10, 4, 2)] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="50" viewBox="0 0 60 50"><rect width="60" height="50" fill="#c3957a"/><rect x="{x}" y="4" width="22" height="30" fill="#b294c3"/><defs><filter id="s{namespace}_f"><feGaussianBlur stdDeviation="{blur}"/></filter><mask id="s{namespace}_m"><rect width="60" height="50" fill="white"/></mask></defs><g mask="url(#s{namespace}_m)" filter="url(#s{namespace}_f)"><path fill="#0f3751" d="M12 15h17v16H12z"/></g><circle cx="{x}" cy="28" r="7" fill="coral"/></svg>"##
            );
            let tree = usvg::Tree::from_str(&svg, &options).unwrap();
            let mut expected_pixels = tiny_skia::Pixmap::new(100, 90).unwrap();
            let mut actual = expected_pixels.clone();
            resvg::render(&tree, transform, &mut expected_pixels.as_mut());
            assert!(cache.render(&svg, &options, transform, &mut actual));
            assert_eq!(cache.layers.len(), expected);
            assert!(actual.data() == expected_pixels.data());
        }
    }
}
