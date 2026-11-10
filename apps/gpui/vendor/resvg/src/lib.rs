// Copyright 2017 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
[resvg](https://github.com/linebender/resvg) is an SVG rendering library.
*/

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::identity_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::wrong_self_convention)]

pub use tiny_skia;
pub use usvg;

mod clip;
mod filter;
mod geom;
mod image;
mod mask;
mod path;
mod render;

/// Renders a tree onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The produced content is in the sRGB color space.
pub fn render(
    tree: &usvg::Tree,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    let max_bbox = max_filter_bbox(pixmap.width(), pixmap.height());

    let ctx = render::Context { max_bbox };
    render::render_nodes(tree.root(), &ctx, transform, pixmap);
}

/// Renders a node onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The expected pixmap size can be retrieved from `usvg::Node::abs_layer_bounding_box()`.
///
/// Returns `None` when `node` has a zero size.
///
/// The produced content is in the sRGB color space.
pub fn render_node(
    node: &usvg::Node,
    mut transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let bbox = node.abs_layer_bounding_box()?;

    let max_bbox = max_filter_bbox(pixmap.width(), pixmap.height());

    transform = transform.pre_translate(-bbox.x(), -bbox.y());

    let ctx = render::Context { max_bbox };
    render::render_node(node, &ctx, transform, pixmap);

    Some(())
}

pub(crate) trait OptionLog {
    fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
    #[inline]
    fn log_none<F: FnOnce()>(self, f: F) -> Self {
        self.or_else(|| {
            f();
            None
        })
    }
}

fn max_filter_bbox(width: u32, height: u32) -> tiny_skia::IntRect {
    tiny_skia::IntRect::from_xywh(
        i32::try_from(width).unwrap_or(i32::MAX).saturating_mul(-2),
        i32::try_from(height).unwrap_or(i32::MAX).saturating_mul(-2),
        width.saturating_mul(5),
        height.saturating_mul(5),
    )
    .unwrap_or_else(|| {
        tiny_skia::IntRect::from_ltrb(i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2)
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn displacement_uses_authored_scale_once_on_each_physical_axis() {
        for scale in [-8, 0, 8] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80"><defs><filter id="d" filterUnits="userSpaceOnUse" x="0" y="0" width="100" height="80" color-interpolation-filters="sRGB"><feFlood flood-color="#ff8080" result="map"/><feDisplacementMap in="SourceGraphic" in2="map" scale="{scale}" xChannelSelector="R" yChannelSelector="G"/></filter></defs><rect x="30" y="20" width="40" height="40" fill="red" filter="url(#d)"/></svg>"##
            );
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
            for (sx, sy) in [(1, 1), (2, 3)] {
                let mut pixels = tiny_skia::Pixmap::new(100 * sx, 80 * sy).unwrap();
                super::render(
                    &tree,
                    tiny_skia::Transform::from_scale(sx as f32, sy as f32),
                    &mut pixels.as_mut(),
                );
                let left = ((30 - scale / 2) * sx as i32) as u32;
                assert_eq!(pixels.pixel(left, 25 * sy).unwrap().alpha(), 255);
                assert_eq!(pixels.pixel(left - 1, 25 * sy).unwrap().alpha(), 0);
                assert_eq!(
                    pixels.pixel(left + 40 * sx - 1, 25 * sy).unwrap().alpha(),
                    255
                );
                assert_eq!(pixels.pixel(left + 40 * sx, 25 * sy).unwrap().alpha(), 0);
            }
        }
    }

    #[test]
    fn max_filter_bbox_is_clamped() {
        let bbox = super::max_filter_bbox(u32::MAX, u32::MAX);
        assert_eq!(bbox.left(), i32::MIN / 2);
        assert_eq!(bbox.top(), i32::MIN / 2);
        assert_eq!(bbox.right(), i32::MAX / 2);
        assert_eq!(bbox.bottom(), i32::MAX / 2);
    }
}

/// Set the pixel budget of the optional non-stitching turbulence cache.
/// Zero (the default) retains the upstream uncached rendering path.
pub fn set_turbulence_cache_budget(bytes: usize) {
    filter::turbulence::set_budget(bytes);
}

/// Release cached noise pixels. In-flight renders retain only their current block.
pub fn clear_turbulence_cache() {
    filter::turbulence::clear_cache();
}
