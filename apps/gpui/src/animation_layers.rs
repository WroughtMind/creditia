//! Cache stationary Satori shadows while streaming a changing scene. Each
//! shadow is rendered by resvg with the original transform, before subsequent
//! paint operations. Other SVGs use the ordinary full-tree path.
use resvg::{tiny_skia, usvg};
use std::collections::{HashMap, VecDeque};

const BUDGET: usize = 48 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct LayerCache {
    layers: VecDeque<(String, tiny_skia::Transform, tiny_skia::Pixmap)>,
    bytes: usize,
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
            if !prefix
                .strip_prefix('s')?
                .bytes()
                .all(|b| b.is_ascii_digit())
                || !mask.starts_with(prefix)
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
                        resvg::render(&tree, transform, &mut pix.as_mut());
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
                    while self.bytes > BUDGET {
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
    fn moving_content_and_changing_shadows_match_unsplit_pixels() {
        let mut cache = LayerCache::default();
        let options = usvg::Options::default();
        let transform = tiny_skia::Transform::from_row(2., 0., 0., 2., -8., -6.);
        for (namespace, x, blur, expected) in [(1, 3, 2, 1), (2, 10, 2, 1), (3, 10, 4, 2)] {
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
