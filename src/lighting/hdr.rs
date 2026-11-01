//! Bounded exposure and the CPU reference for the HDR display transform.

#[cfg(test)]
use glam::Vec3;

pub(crate) fn exposure_target(sky_exposure: f32) -> f32 {
    let sky = if sky_exposure.is_finite() {
        sky_exposure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0.8 + (1.0 - sky) * 0.16).clamp(0.8, 0.96)
}

#[derive(Clone, Copy)]
pub(crate) struct Exposure {
    value: f32,
    previous_time: Option<f32>,
}

impl Default for Exposure {
    fn default() -> Self {
        Self {
            value: 0.8,
            previous_time: None,
        }
    }
}

impl Exposure {
    pub(crate) fn update(&mut self, sky: f32, time: f32, deterministic: bool) -> f32 {
        let target = exposure_target(sky);
        let time = if time.is_finite() { time } else { 0.0 };
        let dt = self
            .previous_time
            .map_or(0.0, |previous| (time - previous).clamp(0.0, 0.1));
        if deterministic
            || self.previous_time.is_none()
            || self.previous_time.is_some_and(|previous| time < previous)
        {
            self.value = target;
        } else {
            self.value += (target - self.value) * (1.0 - (-dt * 2.5).exp());
        }
        self.previous_time = Some(time);
        self.value
    }
}

#[cfg(test)]
fn tone_map(color: Vec3, exposure: f32) -> Vec3 {
    let x = color.max(Vec3::ZERO) * exposure;
    ((x * (x * 2.51 + Vec3::splat(0.03)))
        / (x * (x * 2.43 + Vec3::splat(0.59)) + Vec3::splat(0.14)))
    .clamp(Vec3::ZERO, Vec3::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdr_highlights_roll_off_without_lifting_black_or_clipping_at_one() {
        assert_eq!(tone_map(Vec3::ZERO, 0.8), Vec3::ZERO);
        assert_eq!(tone_map(Vec3::splat(-1.0), 0.8), Vec3::ZERO);
        let mut previous = 0.0;
        for brightness in [0.01, 0.1, 0.5, 1.0, 2.0, 4.0, 8.0] {
            let mapped = tone_map(Vec3::splat(brightness), 0.8);
            assert!(mapped.is_finite() && mapped.x > previous && mapped.x <= 1.0);
            previous = mapped.x;
        }
        assert!(tone_map(Vec3::splat(2.0), 0.8).x > tone_map(Vec3::ONE, 0.8).x);
        let orange = tone_map(Vec3::new(5.5, 1.35, 0.16), 0.8);
        assert!(orange.x > orange.y && orange.y > orange.z);
    }

    #[test]
    fn exposure_adapts_smoothly_but_cannot_make_unlit_caves_bright() {
        let mut exposure = Exposure::default();
        assert_eq!(exposure.update(1.0, 1.0, false), 0.8);
        assert_eq!(exposure.update(0.0, 1.0, false), 0.8);
        let transition = exposure.update(0.0, 1.1, false);
        assert!(transition > 0.8 && transition < 0.96);
        for step in 0..100 {
            assert!(exposure.update(0.0, 1.2 + step as f32 * 0.1, false) <= 0.96);
        }
        assert_eq!(exposure.update(1.0, 0.0, false), 0.8);
        assert_eq!(exposure.update(0.0, 50.0, true), 0.96);
        assert!(exposure.update(f32::NAN, f32::NAN, false).is_finite());
        assert_eq!(exposure_target(-100.0), 0.96);
        assert_eq!(exposure_target(100.0), 0.8);
    }
}
