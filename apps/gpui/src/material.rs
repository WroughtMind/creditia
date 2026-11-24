//! Product surfaces: the original desktop tokens and a blurred artwork backdrop.
//! The backdrop samples the same decoded previews as the canvas, off the UI
//! thread. It never captures the desktop or includes the surface's own controls.
use crate::{image_assets, ui_model::Rect};
use gpui_kit::component::{ActiveTheme, button::Button};
use gpui_kit::*;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, Default)]
pub struct Anchors(Rc<RefCell<BTreeMap<&'static str, Bounds<Pixels>>>>);
impl Anchors {
    pub fn get(&self, id: &'static str) -> Option<Bounds<Pixels>> {
        self.0.borrow().get(id).copied()
    }
    pub fn contains(&self, id: &'static str, position: Point<Pixels>) -> bool {
        self.get(id)
            .is_some_and(|bounds| bounds.contains(&position))
    }
    pub fn tracker(&self, id: &'static str) -> impl IntoElement {
        let anchors = self.clone();
        canvas(
            move |bounds, _, _| {
                anchors.0.borrow_mut().insert(id, bounds);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    pub fn wrap(&self, id: &'static str, element: impl IntoElement) -> Div {
        let anchors = self.clone();
        div()
            .relative()
            .flex_shrink_0()
            .child(
                canvas(
                    move |bounds, _, _| {
                        anchors.0.borrow_mut().insert(id, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(element)
    }
}

pub fn short_path(path: &str) -> String {
    std::env::var(if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    })
    .ok()
    .filter(|home| {
        path == home
            || path
                .strip_prefix(home)
                .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('\\'))
    })
    .map(|home| format!("~{}", &path[home.len()..]))
    .unwrap_or_else(|| path.to_string())
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Hsla,
    pub ink: Hsla,
    pub soft: Hsla,
    pub faint: Hsla,
    pub edge: Hsla,
    pub selected: Hsla,
    pub glass: Hsla,
    pub solid: Hsla,
}
impl Palette {
    pub fn new(dark: bool) -> Self {
        let bg: Hsla = rgb(if dark { 0x0A0A0A } else { 0xEFEFED }).into();
        let ink: Hsla = rgb(if dark { 0xF4F4F2 } else { 0x0E0E0E }).into();
        Self {
            dark,
            bg,
            ink,
            soft: rgb(if dark { 0x9C9C98 } else { 0x6B6B68 }).into(),
            faint: rgb(if dark { 0x5E5E5B } else { 0xA3A3A0 }).into(),
            edge: rgba(if dark { 0xFFFFFF0F } else { 0x00000012 }).into(),
            selected: ink.opacity(0.09),
            glass: rgba(if dark { 0x1E1E1E8C } else { 0xFFFFFF94 }).into(),
            solid: bg.opacity(0.96),
        }
    }
    pub fn of(cx: &App) -> Self {
        Self::new(cx.theme().is_dark())
    }
    pub fn shadows(self) -> Vec<BoxShadow> {
        vec![
            BoxShadow::new(
                px(0.),
                px(1.),
                rgba(if self.dark { 0x0000004D } else { 0x0000000A }).into(),
            )
            .blur_radius(px(1.)),
            BoxShadow::new(
                px(0.),
                px(if self.dark { 10. } else { 8. }),
                rgba(if self.dark { 0x00000073 } else { 0x00000014 }).into(),
            )
            .blur_radius(px(if self.dark { 30. } else { 28. })),
        ]
    }
}

pub fn configure_theme(theme: &mut gpui_kit::component::Theme) {
    let p = Palette::new(theme.is_dark());
    theme.font_family = "Inter".into();
    theme.colors.background = p.bg;
    theme.colors.foreground = p.ink;
    theme.colors.muted_foreground = p.soft;
    theme.colors.muted = p.selected;
    theme.colors.border = p.edge;
    theme.colors.popover = p.bg;
    theme.colors.popover_foreground = p.ink;
    theme.colors.primary = p.ink;
    theme.colors.primary_foreground = p.bg;
    theme.colors.accent = p.selected;
    theme.colors.accent_foreground = p.ink;
}

/// Kit's label container supplies its own text size; style the actual label.
pub trait ShellLabel {
    fn shell_label(self, label: impl Into<SharedString>) -> Self;
}
impl ShellLabel for Button {
    fn shell_label(self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.accessibility_label(label.clone()).child(
            div()
                .min_w_0()
                .text_size(px(13.))
                .font_weight(FontWeight::NORMAL)
                .truncate()
                .child(label),
        )
    }
}

#[derive(Clone)]
pub struct Layer {
    pub image: Arc<RenderImage>,
    /// Rectangle occupied by this texture, including a viewport crop if present.
    pub rect: Rect,
    /// Complete artwork bounds, used to preserve its outer 10px corners.
    pub artwork: Rect,
}
pub struct RasterBackdrop {
    pub image: Arc<Image>,
    pub background: Option<Arc<Image>>,
    pub top: Option<Arc<Image>>,
}
const SAMPLE: f32 = 4.;
pub struct Scene {
    pub size: (f32, f32),
    pub dark: bool,
    pub focused: bool,
    pub header_fade: bool,
    pub layers: Vec<Layer>,
    pub foreground: Vec<Layer>,
}
impl Scene {
    pub fn key(&self) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (
            self.size.0.to_bits(),
            self.size.1.to_bits(),
            self.dark,
            self.focused,
            self.header_fade,
        )
            .hash(&mut hash);
        for layer in self.layers.iter().chain(&self.foreground) {
            layer.image.id.0.hash(&mut hash);
            for r in [layer.rect, layer.artwork] {
                [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()].hash(&mut hash);
            }
        }
        hash.finish()
    }
    pub fn render(&self) -> Result<RasterBackdrop, String> {
        // The smallest backdrop blur is 14 CSS pixels. Quarter resolution keeps
        // its low-frequency detail while avoiding a full Retina-sized readback.
        let width = (self.size.0 / SAMPLE).ceil().max(1.) as usize;
        let height = (self.size.1 / SAMPLE).ceil().max(1.) as usize;
        let base = if self.dark {
            [10, 10, 10, 255]
        } else {
            [239, 239, 237, 255]
        };
        let mut pixels = base.repeat(width * height);
        composite(&mut pixels, width, height, &self.layers);
        let top_pixels = if self.header_fade {
            Some(top_fade_pixels(&pixels, width, height, base))
        } else {
            None
        };
        let top = if self.focused {
            None
        } else {
            top_pixels
                .as_ref()
                .map(|pixels| encode(pixels, width, pixels.len() / width / 4))
                .transpose()?
        };
        for y in 0..height {
            let from_bottom = self.size.1 - (y as f32 + 0.5) * SAMPLE;
            let fade = (1. - from_bottom / 80.).clamp(0., 1.) * 0.88;
            for pixel in pixels[y * width * 4..(y + 1) * width * 4].chunks_exact_mut(4) {
                for c in 0..3 {
                    pixel[c] = (f32::from(pixel[c]) * (1. - fade) + f32::from(base[c]) * fade)
                        .round() as u8;
                }
            }
        }
        if let Some(top) = top_pixels {
            for (pixel, source) in pixels.chunks_exact_mut(4).zip(top.chunks_exact(4)) {
                let alpha = f32::from(source[3]) / 255.;
                for c in 0..3 {
                    pixel[c] = (f32::from(pixel[c]) * (1. - alpha) + f32::from(source[c]) * alpha)
                        .round() as u8;
                }
            }
        }
        let background = if self.focused {
            gaussian_blur(&mut pixels, width, height, 28. / SAMPLE);
            saturate(&mut pixels, 1.4);
            let background = encode(&pixels, width, height)?;
            for pixel in pixels.chunks_exact_mut(4) {
                for c in 0..3 {
                    pixel[c] =
                        (f32::from(pixel[c]) * 0.28 + f32::from(base[c]) * 0.72).round() as u8;
                }
            }
            composite(&mut pixels, width, height, &self.foreground);
            Some(background)
        } else {
            None
        };
        gaussian_blur(&mut pixels, width, height, 22. / SAMPLE);
        saturate(&mut pixels, 1.8);
        Ok(RasterBackdrop {
            image: encode(&pixels, width, height)?,
            background,
            top,
        })
    }
}
fn composite(pixels: &mut [u8], width: usize, height: usize, layers: &[Layer]) {
    for layer in layers {
        let Some(bytes) = layer.image.as_bytes(0) else {
            continue;
        };
        let size = layer.image.size(0);
        let (sw, sh) = (size.width.0 as usize, size.height.0 as usize);
        if sw == 0 || sh == 0 || layer.rect.w <= 0. || layer.rect.h <= 0. {
            continue;
        }
        let left = (layer.rect.x / SAMPLE).floor().max(0.) as usize;
        let top = (layer.rect.y / SAMPLE).floor().max(0.) as usize;
        let right = ((layer.rect.x + layer.rect.w) / SAMPLE).ceil().max(0.) as usize;
        let bottom = ((layer.rect.y + layer.rect.h) / SAMPLE).ceil().max(0.) as usize;
        for y in top..bottom.min(height) {
            for x in left..right.min(width) {
                let (dx, dy) = ((x as f32 + 0.5) * SAMPLE, (y as f32 + 0.5) * SAMPLE);
                if !inside_rounded(dx, dy, layer.artwork, 10.) {
                    continue;
                }
                let sx = ((dx - layer.rect.x) / layer.rect.w * sw as f32 - 0.5)
                    .clamp(0., (sw - 1) as f32);
                let sy = ((dy - layer.rect.y) / layer.rect.h * sh as f32 - 0.5)
                    .clamp(0., (sh - 1) as f32);
                let color = sample_bgra(bytes, sw, sh, sx, sy);
                let offset = (y * width + x) * 4;
                let alpha = color[3] / 255.;
                for channel in 0..3 {
                    pixels[offset + channel] = (color[channel] * alpha
                        + f32::from(pixels[offset + channel]) * (1. - alpha))
                        .round() as u8;
                }
            }
        }
    }
}
fn saturate(pixels: &mut [u8], saturation: f32) {
    for pixel in pixels.chunks_exact_mut(4) {
        let luma = 0.2126 * f32::from(pixel[0])
            + 0.7152 * f32::from(pixel[1])
            + 0.0722 * f32::from(pixel[2]);
        for channel in &mut pixel[..3] {
            *channel = (luma + (f32::from(*channel) - luma) * saturation)
                .round()
                .clamp(0., 255.) as u8;
        }
    }
}
fn top_fade_pixels(source: &[u8], width: usize, height: usize, bg: [u8; 4]) -> Vec<u8> {
    let rows = ((88. + 42.) / SAMPLE).ceil() as usize;
    let mut pixels = source[..width * height.min(rows) * 4].to_vec();
    gaussian_blur(&mut pixels, width, height.min(rows), 14. / SAMPLE);
    let out_height = height.min((88. / SAMPLE).ceil() as usize);
    pixels.truncate(width * out_height * 4);
    for y in 0..out_height {
        let position = (y as f32 + 0.5) * SAMPLE / 88.;
        let tint = if position <= 0.3 {
            0.82
        } else {
            0.82 * (1. - position) / 0.7
        };
        let opacity = if position <= 0.55 {
            1.
        } else {
            (1. - position) / 0.45
        };
        for pixel in pixels[y * width * 4..(y + 1) * width * 4].chunks_exact_mut(4) {
            for c in 0..3 {
                pixel[c] =
                    (f32::from(pixel[c]) * (1. - tint) + f32::from(bg[c]) * tint).round() as u8;
            }
            pixel[3] = (opacity * 255.).round().clamp(0., 255.) as u8;
        }
    }
    pixels
}
fn encode(pixels: &[u8], width: usize, height: usize) -> Result<Arc<Image>, String> {
    let mut encoded = Vec::new();
    let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(pixels).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(Arc::new(Image::from_bytes(ImageFormat::Png, encoded)))
}

fn inside_rounded(x: f32, y: f32, r: Rect, radius: f32) -> bool {
    if x < r.x || y < r.y || x > r.x + r.w || y > r.y + r.h {
        return false;
    }
    let (half_w, half_h) = (r.w / 2., r.h / 2.);
    let radius = radius.min(half_w).min(half_h);
    // At a capsule's short side the inset interval has zero width. Computing
    // its two endpoints separately can reverse them through f32 rounding.
    let dx = ((x - (r.x + half_w)).abs() - (half_w - radius)).max(0.);
    let dy = ((y - (r.y + half_h)).abs() - (half_h - radius)).max(0.);
    dx.powi(2) + dy.powi(2) <= radius.powi(2)
}
fn sample_bgra(bytes: &[u8], width: usize, height: usize, x: f32, y: f32) -> [f32; 4] {
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x.fract(), y.fract());
    let mut result = [0.; 4];
    for (channel, source) in [2, 1, 0, 3].into_iter().enumerate() {
        let at = |x, y| f32::from(bytes[(y * width + x) * 4 + source]);
        result[channel] = (at(x0, y0) * (1. - fx) + at(x1, y0) * fx) * (1. - fy)
            + (at(x0, y1) * (1. - fx) + at(x1, y1) * fx) * fy;
    }
    result
}
fn gaussian_blur(pixels: &mut Vec<u8>, width: usize, height: usize, sigma: f32) {
    let ideal = (4. * sigma * sigma + 1.).sqrt();
    let lower = (ideal.floor() as i32) | 1;
    let lower = if lower as f32 > ideal {
        lower - 2
    } else {
        lower
    }
    .max(1);
    let n = ((12. * sigma * sigma - 3. * (lower * lower + 4 * lower + 3) as f32)
        / (-4 * lower - 4) as f32)
        .round()
        .clamp(0., 3.) as usize;
    let mut scratch = vec![0; pixels.len()];
    for pass in 0..3 {
        let radius = ((if pass < n { lower } else { lower + 2 }) / 2) as usize;
        box_pass(pixels, &mut scratch, width, height, radius, true);
        box_pass(&scratch, pixels, width, height, radius, false);
    }
}
fn box_pass(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    horizontal: bool,
) {
    let (length, lines) = if horizontal {
        (width, height)
    } else {
        (height, width)
    };
    let offset = |line: usize, position: usize| {
        if horizontal {
            (line * width + position) * 4
        } else {
            (position * width + line) * 4
        }
    };
    let span = (radius * 2 + 1) as u32;
    for line in 0..lines {
        let mut sums = [0u32; 4];
        for p in 0..radius * 2 + 1 {
            let source = offset(line, p.saturating_sub(radius).min(length - 1));
            for c in 0..4 {
                sums[c] += u32::from(input[source + c]);
            }
        }
        for position in 0..length {
            let dest = offset(line, position);
            let remove = offset(line, position.saturating_sub(radius));
            let add = offset(line, (position + radius + 1).min(length - 1));
            for c in 0..4 {
                output[dest + c] = ((sums[c] + span / 2) / span) as u8;
                sums[c] = sums[c] - u32::from(input[remove + c]) + u32::from(input[add + c]);
            }
        }
    }
}

/// A rounded crop of the shared blurred scene, then tint, rim and highlight.
/// `paint_image` crops UVs to the surface bounds, so controls stay sharp.
pub fn surface(
    image: Option<Arc<Image>>,
    tint: Hsla,
    radius: f32,
    palette: Palette,
) -> impl IntoElement {
    canvas(
        move |_, window, cx| image.and_then(|image| image_assets::use_image(&image, window, cx)),
        move |bounds, decoded, window, _| {
            if let Some(decoded) = decoded {
                let image_bounds = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
                let _ = window.paint_image(
                    bounds,
                    image_bounds,
                    Corners::all(px(radius)),
                    decoded,
                    0,
                    false,
                );
            }
            window.paint_quad(quad(
                bounds,
                px(radius),
                tint,
                px(if radius > 0. { 1. } else { 0. }),
                palette.edge,
                BorderStyle::Solid,
            ));
            if radius == 0. {
                return;
            }
            window.paint_quad(quad(
                bounds,
                px(radius),
                linear_gradient(
                    180.,
                    linear_color_stop(rgba(if palette.dark { 0xFFFFFF09 } else { 0xFFFFFF1A }), 0.),
                    linear_color_stop(rgba(0xFFFFFF00), 0.42),
                ),
                px(0.),
                transparent_black(),
                BorderStyle::Solid,
            ));
        },
    )
    .absolute()
    .inset_0()
}

/// Clip the preview itself; rounded parent divs only clip rectangular bounds.
pub fn preview(image: Arc<Image>, rect: Rect, corners: Corners<Pixels>) -> impl IntoElement {
    canvas(
        move |_, window, cx| image_assets::use_image(&image, window, cx),
        move |bounds, decoded, window, _| {
            if let Some(decoded) = decoded {
                let image_bounds = Bounds::new(
                    bounds.origin + point(px(rect.x), px(rect.y)),
                    size(px(rect.w), px(rect.h)),
                );
                let _ = window.paint_image(bounds, image_bounds, corners, decoded, 0, false);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Fit an export canvas and clip the image's own corners, not its letterbox.
pub fn contained_preview(image: Arc<Image>, radius: f32, transparent: bool) -> impl IntoElement {
    canvas(
        move |_, window, cx| image_assets::use_image(&image, window, cx),
        move |bounds, decoded, window, _| {
            if let Some(decoded) = decoded {
                let source = decoded.size(0);
                let width = source.width.0 as f32;
                let height = source.height.0 as f32;
                if width <= 0. || height <= 0. {
                    return;
                }
                let scale = (f32::from(bounds.size.width) / width)
                    .min(f32::from(bounds.size.height) / height);
                let (width, height) = (width * scale, height * scale);
                let fitted = Bounds::new(
                    bounds.origin
                        + point(
                            px((f32::from(bounds.size.width) - width) / 2.),
                            px((f32::from(bounds.size.height) - height) / 2.),
                        ),
                    size(px(width), px(height)),
                );
                if transparent {
                    // The grid is a preview backing only. It never becomes
                    // part of the renderer's exported transparent pixels.
                    let cols = (width / 12.).floor().max(1.) as usize;
                    let rows = (height / 12.).floor().max(1.) as usize;
                    let (cw, ch) = (width / cols as f32, height / rows as f32);
                    let corner = px(radius.min(width / 2.).min(height / 2.));
                    for row in 0..rows {
                        for col in 0..cols {
                            let (x, y) = (col as f32 * cw, row as f32 * ch);
                            window.paint_quad(quad(
                                Bounds::new(
                                    fitted.origin + point(px(x), px(y)),
                                    size(px(cw), px(ch)),
                                ),
                                Corners {
                                    top_left: if row == 0 && col == 0 { corner } else { px(0.) },
                                    top_right: if row == 0 && col + 1 == cols {
                                        corner
                                    } else {
                                        px(0.)
                                    },
                                    bottom_left: if row + 1 == rows && col == 0 {
                                        corner
                                    } else {
                                        px(0.)
                                    },
                                    bottom_right: if row + 1 == rows && col + 1 == cols {
                                        corner
                                    } else {
                                        px(0.)
                                    },
                                },
                                rgb(if (row + col) % 2 == 0 {
                                    0xeeeeea
                                } else {
                                    0xdadad6
                                }),
                                px(0.),
                                transparent_black(),
                                BorderStyle::Solid,
                            ));
                        }
                    }
                }
                let _ = window.paint_image(
                    fitted,
                    fitted,
                    Corners::all(px(radius.min(width / 2.).min(height / 2.))),
                    decoded,
                    0,
                    false,
                );
            }
        },
    )
    .size_full()
}

/// A prepared playback frame goes straight to the atlas; no PNG conversion or
/// asynchronous decoder is involved. Record completed painting after upload.
pub fn frame_preview(
    image: Arc<RenderImage>,
    rect: Rect,
    corners: Corners<Pixels>,
    painted: Option<Arc<std::sync::OnceLock<std::time::Instant>>>,
    trace: Option<(u64, usize)>,
) -> impl IntoElement {
    canvas(|_, _, _| (), move |bounds, _, window, _| {
        let image_bounds = Bounds::new(bounds.origin + point(px(rect.x), px(rect.y)), size(px(rect.w), px(rect.h)));
        if window.paint_image(bounds, image_bounds, corners, image.clone(), 0, false).is_ok() {
            if let Some(painted) = &painted {
                if painted.set(std::time::Instant::now()).is_ok() {
                    if let Some((epoch, index)) = trace {
                        crate::app::write_performance_event(serde_json::json!({"event":"animation-painted","epoch":epoch,"index":index}));
                    }
                }
            }
        }
    }).absolute().inset_0()
}

pub fn image_corners(region: Rect, width: f32, height: f32) -> Corners<Pixels> {
    let left = region.x <= 0.;
    let top = region.y <= 0.;
    let right = region.x + region.w >= width - f32::EPSILON * width;
    let bottom = region.y + region.h >= height - f32::EPSILON * height;
    Corners {
        top_left: px(if left && top { 10. } else { 0. }),
        top_right: px(if right && top { 10. } else { 0. }),
        bottom_left: px(if left && bottom { 10. } else { 0. }),
        bottom_right: px(if right && bottom { 10. } else { 0. }),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fractional_capsule_bounds_keep_their_center_inside() {
        for (width, height) in [(341., 20.), (20., 341.), (1., 0.1)] {
            let rect = super::Rect {
                x: 298.3736,
                y: 298.3736,
                w: width,
                h: height,
            };
            assert!(super::inside_rounded(
                rect.x + width / 2.,
                rect.y + height / 2.,
                rect,
                10.
            ));
            assert!(!super::inside_rounded(rect.x - 1., rect.y, rect, 10.));
        }
    }
    use super::{gaussian_blur, sample_bgra};
    #[test]
    fn blur_preserves_uniform_surfaces_and_softens_real_edges() {
        let (w, h) = (80, 40);
        let mut flat = [239, 239, 237, 255].repeat(w * h);
        let expected = flat.clone();
        gaussian_blur(&mut flat, w, h, 5.5);
        assert_eq!(flat, expected);
        let mut edge = [0, 0, 0, 255].repeat(w * h);
        for y in 0..h {
            for x in w / 2..w {
                edge[(y * w + x) * 4..(y * w + x) * 4 + 3].fill(255);
            }
        }
        gaussian_blur(&mut edge, w, h, 5.5);
        let at = |x| edge[(h / 2 * w + x) * 4];
        assert!(at(35) > 0 && at(35) < at(40) && at(40) < at(45) && at(45) < 255);
        assert_eq!(at(0), 0);
        assert_eq!(at(w - 1), 255);
    }
    #[test]
    fn backdrop_sampling_preserves_color_channels_and_alpha() {
        assert_eq!(
            sample_bgra(&[10, 20, 230, 128], 1, 1, 0., 0.),
            [230., 20., 10., 128.]
        );
    }
}
