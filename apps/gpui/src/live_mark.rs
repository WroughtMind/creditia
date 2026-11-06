//! Native status star. Its original five vector petals are uploaded once;
//! animation changes only GPUI layer geometry and opacity.
use gpui_kit::*;
use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};

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
    static IMAGES: OnceLock<[Arc<Image>; PETALS]> = OnceLock::new();
    IMAGES
        .get_or_init(|| {
            petal_sources()
                .map(|svg| Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
        })
        .clone()
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
        }
    }

    pub fn render(
        &mut self,
        progress: Option<f64>,
        busy: bool,
        error: bool,
        reduced_motion: bool,
        size: f32,
        window: &mut Window,
        _cx: &mut App,
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
        let (petals, moving) = self
            .motion
            .step(progress, busy, error, reduced_motion, active, dt);
        if moving {
            window.request_animation_frame();
        }
        let size = if size.is_finite() { size.max(1.) } else { 24. };
        let mut star = div().relative().w(px(size)).h(px(size)).flex_shrink_0();
        for (image, petal) in self.petals.iter().zip(petals) {
            let extent = size * petal.scale;
            let inset = (size - extent) * 0.5;
            star = star.child(
                img(image.clone())
                    .absolute()
                    .left(px(inset))
                    .top(px(inset))
                    .w(px(extent))
                    .h(px(extent))
                    .opacity(petal.opacity),
            );
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
    use super::{Motion, petal_sources};
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
