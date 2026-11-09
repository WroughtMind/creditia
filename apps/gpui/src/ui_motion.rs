//! Original roam release velocity and 520 ms cubic glide, independent of paints.
//! Positions use the caller's pan convention; no screen/scroll sign conversion.
use std::{collections::VecDeque, time::Duration};

const GLIDE: Duration = Duration::from_millis(520);
const STALE: Duration = Duration::from_millis(80);
const DISTANCE: f32 = 16. * 0.94 / (1. - 0.94);

#[derive(Clone, Copy)]
struct Sample {
    at: Duration,
    position: (f32, f32),
}
struct Glide {
    at: Duration,
    from: (f32, f32),
    to: (f32, f32),
}

#[derive(Default)]
pub struct RoamMotion {
    samples: VecDeque<Sample>,
    glide: Option<Glide>,
}
impl RoamMotion {
    /// New pointer, wheel, reset, hidden window and reduced motion cancel both
    /// current movement and velocity history, so no stale release can restart it.
    pub fn stop(&mut self) {
        self.samples.clear();
        self.glide = None;
    }

    /// Record actual moved viewport positions, not every pointer hover or down.
    pub fn record(&mut self, now: Duration, position: (f32, f32)) {
        self.glide = None;
        if !position.0.is_finite() || !position.1.is_finite() {
            self.samples.clear();
            return;
        }
        if self.samples.back().is_some_and(|last| now < last.at) {
            self.samples.clear();
        }
        self.samples.push_back(Sample { at: now, position });
        while self.samples.len() > 6 {
            self.samples.pop_front();
        }
    }

    /// Start only on a genuine pointer-up after a recent drag. Pointer cancel,
    /// hidden/reduced-motion states pass false and discard the velocity history.
    pub fn release(&mut self, now: Duration, position: (f32, f32), allow_motion: bool) -> bool {
        self.glide = None;
        let ends = self
            .samples
            .front()
            .copied()
            .zip(self.samples.back().copied());
        let enough = self.samples.len() >= 2;
        self.samples.clear();
        let Some((first, last)) = ends else {
            return false;
        };
        if !allow_motion
            || !enough
            || now < last.at
            || now - last.at > STALE
            || !position.0.is_finite()
            || !position.1.is_finite()
        {
            return false;
        }
        let ms = (last.at - first.at)
            .as_secs_f32()
            .mul_add(1000., 0.)
            .max(16.);
        let to = (
            position.0 + (last.position.0 - first.position.0) / ms * DISTANCE,
            position.1 + (last.position.1 - first.position.1) / ms * DISTANCE,
        );
        self.glide_to(now, position, to, true)
    }

    /// Keyboard and Home use the same easing as a released drag. The caller
    /// places the destination immediately when motion is disabled.
    pub fn glide_to(
        &mut self,
        now: Duration,
        from: (f32, f32),
        to: (f32, f32),
        allow_motion: bool,
    ) -> bool {
        self.stop();
        if !allow_motion
            || ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite())
            || from == to
        {
            return false;
        }
        self.glide = Some(Glide { at: now, from, to });
        true
    }

    pub fn is_active(&self) -> bool {
        self.glide.is_some()
    }

    /// Return the final position once, then stop requesting animation frames.
    pub fn sample(&mut self, now: Duration) -> Option<(f32, f32)> {
        let glide = self.glide.as_ref()?;
        let elapsed = now.saturating_sub(glide.at);
        let t = (elapsed.as_secs_f32() / GLIDE.as_secs_f32()).min(1.);
        let ease = 1. - (1. - t).powi(3);
        let position = (
            glide.from.0 + (glide.to.0 - glide.from.0) * ease,
            glide.from.1 + (glide.to.1 - glide.from.1) * ease,
        );
        if elapsed >= GLIDE {
            self.glide = None;
        }
        Some(position)
    }
}

#[cfg(test)]
mod tests {
    use super::RoamMotion;
    use std::time::Duration;
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn original_velocity_and_cubic_glide_have_exact_end_and_no_idle_frames() {
        let mut motion = RoamMotion::default();
        motion.record(ms(0), (10., 30.));
        motion.record(ms(32), (42., 14.));
        assert!(motion.release(ms(40), (42., 14.), true));
        assert_eq!(motion.sample(ms(40)), Some((42., 14.)));
        let half = motion.sample(ms(300)).unwrap();
        let end = motion.sample(ms(560)).unwrap();
        assert!((end.0 - 292.66666).abs() < 0.001);
        assert!((end.1 + 111.33333).abs() < 0.001);
        assert!((half.0 - (42. + (end.0 - 42.) * 0.875)).abs() < 0.001);
        assert!(!motion.is_active());
        assert!(motion.sample(ms(600)).is_none());
    }

    #[test]
    fn only_latest_six_samples_determine_release_speed() {
        let mut motion = RoamMotion::default();
        motion.record(ms(0), (-10000., 0.));
        for i in 1..=6 {
            motion.record(ms(i * 16), (i as f32 * 8., 0.));
        }
        assert!(motion.release(ms(96), (48., 0.), true));
        assert!((motion.sample(ms(616)).unwrap().0 - 173.33333).abs() < 0.001);
    }

    #[test]
    fn stale_cancelled_and_reduced_motion_releases_cannot_fling() {
        for (now, allowed) in [(113, true), (32, false)] {
            let mut motion = RoamMotion::default();
            motion.record(ms(0), (0., 0.));
            motion.record(ms(32), (20., 10.));
            assert!(!motion.release(ms(now), (20., 10.), allowed));
            assert!(!motion.is_active());
        }
        let mut motion = RoamMotion::default();
        motion.record(ms(0), (0., 0.));
        motion.record(ms(32), (20., 10.));
        assert!(motion.release(ms(32), (20., 10.), true));
        motion.stop();
        assert!(motion.sample(ms(100)).is_none());
        assert!(!motion.release(ms(100), (20., 10.), true));
    }

    #[test]
    fn short_interval_and_reversed_time_do_not_create_extreme_velocity() {
        let mut motion = RoamMotion::default();
        motion.record(ms(100), (0., 0.));
        motion.record(ms(100), (16., 0.));
        assert!(motion.release(ms(100), (16., 0.), true));
        assert!((motion.sample(ms(620)).unwrap().0 - 266.66666).abs() < 0.001);
        motion.record(ms(100), (0., 0.));
        motion.record(ms(90), (20., 0.));
        assert!(!motion.release(ms(90), (20., 0.), true));
    }

    #[test]
    fn keyboard_retarget_uses_current_position_and_reduced_motion_cancels() {
        let mut motion = RoamMotion::default();
        assert!(motion.glide_to(ms(0), (0., 0.), (800., -400.), true));
        let current = motion.sample(ms(260)).unwrap();
        assert_eq!(current, (700., -350.));
        assert!(motion.glide_to(ms(260), current, (100., 80.), true));
        assert_eq!(motion.sample(ms(260)), Some(current));
        assert_eq!(motion.sample(ms(780)), Some((100., 80.)));
        assert!(!motion.glide_to(ms(800), (100., 80.), (0., 0.), false));
        assert!(!motion.is_active());
    }
}
