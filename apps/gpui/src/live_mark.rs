//! Native status star. Its original five vector petals are uploaded once;
//! animation changes only GPUI layer geometry and opacity.
use gpui_kit::*;
use serde_json::Value;
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
    time::Instant,
};

/// Shared-renderer output, prepared once per archive and reused by the header,
/// center status and project picker. Attribution and petal geometry stay in the
/// same TypeScript model as existing Creditia artwork.
#[derive(Clone)]
pub struct ProjectMark {
    pub image: Arc<Image>,
    layers: Arc<[Arc<Image>]>,
    identity: u64,
}
impl ProjectMark {
    pub fn from_response(value: &Value) -> Result<Self, String> {
        let svg = value["svg"].as_str().ok_or("Missing project mark SVG")?;
        // Picker marks are 24 logical pixels. The shared renderer provides a
        // 4x density image so a large library does not rasterize every row at
        // the 384px size reserved for the animated center status.
        let row_svg = value["rowSvg"].as_str().unwrap_or(svg);
        let petals = value["petals"]
            .as_array()
            .ok_or("Missing project mark petals")?;
        if petals.is_empty() || petals.len() > 256 {
            return Err("Invalid project mark petal count".into());
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        svg.hash(&mut hash);
        let mut layers = Vec::with_capacity(petals.len());
        let mut bytes = svg.len().saturating_add(row_svg.len());
        for petal in petals {
            let source = petal["svg"].as_str().ok_or("Invalid project mark petal")?;
            bytes = bytes.saturating_add(source.len());
            if bytes > 2 * 1024 * 1024 {
                return Err("Project mark SVG exceeds its limit".into());
            }
            source.hash(&mut hash);
            layers.push(Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                source.as_bytes().to_vec(),
            )));
        }
        Ok(Self {
            image: Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                row_svg.as_bytes().to_vec(),
            )),
            layers: layers.into(),
            identity: hash.finish(),
        })
    }
}

struct ProjectTransition {
    mark: ProjectMark,
    opacity: f32,
}
impl ProjectTransition {
    fn new(mut mark: ProjectMark) -> Self {
        // The header and center can fade out at different times. Their source
        // handles must retire independently. GPUI keys its decoder by image
        // contents, so equal SVGs still share the same actual decoded texture.
        mark.layers = mark
            .layers
            .iter()
            .map(|image| Arc::new(image.as_ref().clone()))
            .collect();
        Self { mark, opacity: 0. }
    }
    fn release(&self, cx: &mut App) {
        for image in self.mark.layers.iter() {
            crate::image_assets::release(image.clone(), cx);
        }
        // mark.image belongs to the project picker's cache, not this animation.
    }
}

const LOGO: &str = include_str!("../../../packages/render/assets/creditia/logo-star.svg");
const PETALS: usize = 5;

fn petal_sources() -> [String; PETALS] {
    let paths = LOGO
        .split("<path")
        .skip(1)
        .map(|part| {
            let end = part.find("/>").expect("brand petal is self-closing");
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"384\" height=\"384\" viewBox=\"0 0 24 24\"><path{}/></svg>",
                &part[..end]
            )
        })
        .collect::<Vec<_>>();
    paths.try_into().expect("the brand star has five petals")
}

fn images() -> [Arc<Image>; PETALS] {
    static IMAGES: OnceLock<[Image; PETALS]> = OnceLock::new();
    IMAGES
        .get_or_init(|| {
            petal_sources().map(|svg| Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
        })
        .each_ref()
        .map(|image| Arc::new(image.clone()))
}

#[derive(Clone, Copy, Debug)]
struct Petal {
    scale: f32,
    opacity: f32,
}
impl Petal {
    const FULL: Self = Self {
        scale: 1.,
        opacity: 1.,
    };
}

#[derive(Default)]
struct Motion {
    phase: f32,
    current: Option<[Petal; PETALS]>,
}
impl Motion {
    fn step(
        &mut self,
        progress: Option<f64>,
        busy: bool,
        error: bool,
        reduced: bool,
        active: bool,
        dt: f32,
    ) -> ([Petal; PETALS], bool) {
        let progress = progress
            .filter(|value| value.is_finite())
            .map(|p| p.clamp(0., 1.));
        let breathing = busy && progress.is_none() && !error && !reduced;
        if active && breathing {
            self.phase = (self.phase + dt * std::f32::consts::TAU / 1.3) % std::f32::consts::TAU;
        }
        let target = std::array::from_fn::<_, PETALS, _>(|index| {
            if error {
                Petal {
                    scale: 0.5,
                    opacity: 0.55,
                }
            } else if !busy {
                Petal::FULL
            } else if let Some(progress) = progress {
                let filled = (progress as f32 * PETALS as f32 - index as f32).clamp(0., 1.);
                Petal {
                    scale: 0.36 + 0.64 * filled,
                    opacity: 0.32 + 0.68 * filled,
                }
            } else {
                let wave = if breathing {
                    (self.phase - index as f32 * std::f32::consts::TAU / PETALS as f32).sin()
                } else {
                    0.
                };
                Petal {
                    scale: 0.68 * (1. + 0.16 * wave),
                    opacity: 0.64 + 0.14 * wave,
                }
            }
        });
        let current = self.current.get_or_insert(target);
        let mut moving = false;
        if reduced {
            *current = target;
        } else if active {
            // Exponential damping starts from the currently visible value after
            // every retarget, including cancellation, replay and interruption.
            let weight = 1. - (-14. * dt.clamp(0., 0.05)).exp();
            for (petal, target) in current.iter_mut().zip(target) {
                petal.scale += (target.scale - petal.scale) * weight;
                petal.opacity += (target.opacity - petal.opacity) * weight;
                if (petal.scale - target.scale).abs() < 0.001
                    && (petal.opacity - target.opacity).abs() < 0.001
                {
                    *petal = target;
                } else {
                    moving = true;
                }
            }
        }
        (*current, active && !reduced && (moving || breathing))
    }
}

pub struct LiveMark {
    petals: [Arc<Image>; PETALS],
    motion: Motion,
    last_frame: Option<Instant>,
    was_active: bool,
    projects: Vec<ProjectTransition>,
    project_identity: Option<u64>,
    brand_opacity: f32,
}
impl Default for LiveMark {
    fn default() -> Self {
        Self::new()
    }
}
impl LiveMark {
    pub fn new() -> Self {
        Self {
            petals: images(),
            motion: Motion::default(),
            last_frame: None,
            was_active: false,
            projects: Vec::new(),
            project_identity: None,
            brand_opacity: 1.,
        }
    }

    pub fn set_project(&mut self, mark: Option<ProjectMark>, cx: &mut App) {
        let identity = mark.as_ref().map(|mark| mark.identity);
        if identity == self.project_identity {
            return;
        }
        self.project_identity = identity;
        if let Some(mark) = mark {
            if !self
                .projects
                .iter()
                .any(|old| old.mark.identity == mark.identity)
            {
                self.projects.push(ProjectTransition::new(mark));
            }
        }
        // Consecutive selections can interrupt a fade. Keep the visible layers,
        // but bound the number of retiring sets during very fast navigation.
        while self.projects.len() > 3 {
            let old = self.projects.remove(0);
            old.release(cx);
        }
    }

    pub fn release(&mut self, cx: &mut App) {
        for project in self.projects.drain(..) {
            project.release(cx);
        }
        for image in &self.petals {
            crate::image_assets::release(image.clone(), cx);
        }
        self.project_identity = None;
        self.brand_opacity = 1.;
    }

    pub fn render(
        &mut self,
        progress: Option<f64>,
        busy: bool,
        error: bool,
        reduced_motion: bool,
        size: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let now = Instant::now();
        let active = window.is_window_active();
        let dt = self.last_frame.map_or(0., |previous| {
            let elapsed = now.duration_since(previous).as_secs_f32();
            if self.was_active && active && elapsed < 0.2 {
                elapsed
            } else {
                0.
            }
        });
        self.last_frame = Some(now);
        self.was_active = active;
        let (petals, mut moving) =
            self.motion
                .step(progress, busy, error, reduced_motion, active, dt);
        let project_visible = self.project_identity.is_some()
            && !error
            && (!busy || progress.is_none_or(|value| !value.is_finite()));
        let mut approach = |value: &mut f32, target: f32| {
            if reduced_motion {
                *value = target;
            } else if active {
                *value += (target - *value) * (1. - (-14. * dt.clamp(0., 0.05)).exp());
                if (*value - target).abs() < 0.001 {
                    *value = target;
                } else {
                    moving = true;
                }
            }
        };
        approach(
            &mut self.brand_opacity,
            if project_visible { 0. } else { 1. },
        );
        for project in &mut self.projects {
            let target = if project_visible && Some(project.mark.identity) == self.project_identity
            {
                1.
            } else {
                0.
            };
            approach(&mut project.opacity, target);
        }
        self.projects.retain(|project| {
            if project.opacity > 0. || Some(project.mark.identity) == self.project_identity {
                return true;
            }
            project.release(cx);
            false
        });
        let size = if size.is_finite() { size.max(1.) } else { 24. };
        let mut star = div().relative().w(px(size)).h(px(size)).flex_shrink_0();
        for (image, petal) in self.petals.iter().zip(petals) {
            if self.brand_opacity == 0. {
                continue;
            }
            crate::image_assets::track(image, window, cx);
            let extent = size * petal.scale;
            let inset = (size - extent) * 0.5;
            star = star.child(
                img(image.clone())
                    .absolute()
                    .left(px(inset))
                    .top(px(inset))
                    .w(px(extent))
                    .h(px(extent))
                    .opacity(petal.opacity * self.brand_opacity),
            );
        }
        for project in &self.projects {
            if project.opacity == 0. {
                continue;
            }
            let count = project.mark.layers.len() as f32;
            for (index, image) in project.mark.layers.iter().enumerate() {
                crate::image_assets::track(image, window, cx);
                let breathe = if busy && project_visible && !reduced_motion {
                    1. + 0.06
                        * (self.motion.phase - index as f32 * std::f32::consts::TAU / count).sin()
                } else {
                    1.
                };
                let extent = size * breathe;
                let inset = (size - extent) * 0.5;
                star = star.child(
                    img(image.clone())
                        .absolute()
                        .left(px(inset))
                        .top(px(inset))
                        .w(px(extent))
                        .h(px(extent))
                        .opacity(project.opacity),
                );
            }
        }
        if moving {
            window.request_animation_frame();
        }
        if error {
            star = star.child(
                div()
                    .absolute()
                    .right(px(0.))
                    .bottom(px(0.))
                    .w(px((size * 0.14).max(3.)))
                    .h(px((size * 0.14).max(3.)))
                    .rounded_full()
                    .bg(rgb(0xb6504e)),
            );
        }
        star.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Motion, ProjectMark, petal_sources};
    use std::sync::Arc;
    #[test]
    fn project_layers_preserve_more_than_five_tools_and_share_images() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"></svg>";
        let value = serde_json::json!({"svg":svg,"petals":(0..8).map(|_|serde_json::json!({"svg":svg})).collect::<Vec<_>>()});
        let mark = ProjectMark::from_response(&value).unwrap();
        assert_eq!(mark.layers.len(), 8);
        let other = mark.clone();
        assert!(Arc::ptr_eq(&mark.layers, &other.layers));
        assert!(ProjectMark::from_response(&serde_json::json!({"svg":svg,"petals":[{}]})).is_err());
    }
    #[test]
    fn mark_surfaces_retire_independently_without_copying_picker_ownership() {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'/>";
        let mark = ProjectMark::from_response(
            &serde_json::json!({"svg":svg,"rowSvg":svg,"petals":[{"svg":svg}]}),
        )
        .unwrap();
        let header = super::ProjectTransition::new(mark.clone());
        let center = super::ProjectTransition::new(mark.clone());
        assert!(Arc::ptr_eq(&mark.image, &header.mark.image));
        assert!(!Arc::ptr_eq(&header.mark.layers[0], &center.mark.layers[0]));
        assert_eq!(header.mark.layers[0].id, center.mark.layers[0].id);
        let a = super::images();
        let b = super::images();
        assert!(!Arc::ptr_eq(&a[0], &b[0]));
        assert_eq!(a[0].id, b[0].id);
    }
    #[test]
    fn petals_keep_the_original_geometry_and_palette() {
        let sources = petal_sources();
        for (source, color) in sources
            .iter()
            .zip(["#D97757", "#10A37F", "#4C8DF6", "#8C7CFF", "#E8B730"])
        {
            assert_eq!(source.matches("<path").count(), 1);
            assert!(source.contains(color));
            assert!(source.contains("viewBox=\"0 0 24 24\""));
        }
    }
    #[test]
    fn sequential_progress_retargets_continuously_and_idle_stops() {
        let mut motion = Motion::default();
        let (first, _) = motion.step(Some(0.3), true, false, true, true, 0.);
        assert_eq!(first[0].scale, 1.);
        assert!(first[1].scale > first[2].scale);
        let (interrupted, moving) = motion.step(Some(0.8), true, false, false, true, 0.);
        assert_eq!(interrupted[1].scale, first[1].scale);
        assert!(moving);
        for _ in 0..60 {
            motion.step(None, false, false, false, true, 1. / 60.);
        }
        let (done, moving) = motion.step(None, false, false, false, true, 1. / 60.);
        assert!(!moving);
        assert!(
            done.iter()
                .all(|petal| petal.scale == 1. && petal.opacity == 1.)
        );
    }
    #[test]
    fn inactive_and_reduced_motion_never_schedule_frames() {
        let mut motion = Motion::default();
        let (first, _) = motion.step(None, true, false, false, true, 0.02);
        let phase = motion.phase;
        let (paused, moving) = motion.step(None, true, false, false, false, 30.);
        assert!(!moving);
        assert_eq!(motion.phase, phase);
        assert_eq!(paused[0].scale, first[0].scale);
        let (reduced, moving) = motion.step(Some(1.), true, false, true, true, 0.);
        assert!(!moving);
        assert!(reduced.iter().all(|petal| petal.scale == 1.));
    }
}
