//! One CPU solar sample shared by sky, direct light, shadows, probes, water, and air.

use glam::Vec3;

/// Fresh worlds and legacy saves start in full, late-morning daylight.
pub const DEFAULT_TIME_HOUR: f64 = 10.0;

/// Linear-light colors and a world-space solar direction; sky occlusion stays separate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CelestialLight {
    /// Unit world-space direction toward the sun; below the horizon at night.
    pub sun_direction: Vec3,
    /// Direct linear HDR radiance, zero below the horizon.
    pub sun_radiance: Vec3,
    /// Diffuse outdoor sky irradiance, retaining a small blue night floor.
    pub sky_irradiance: Vec3,
    /// Background upper hemisphere in linear light.
    pub sky_top: Vec3,
    /// Outdoor horizon and residency-fog color.
    pub sky_horizon: Vec3,
    /// Cloud albedo illumination; the cloud motion clock is independent.
    pub clouds: Vec3,
    /// Available outdoor daylight, not camera enclosure or vertical voxel transmission.
    pub daylight: f32,
    /// Visibility of the fixed star field and the opposite, decorative moon.
    pub stars: f32,
}

impl Default for CelestialLight {
    fn default() -> Self {
        Self::sample(DEFAULT_TIME_HOUR)
    }
}

impl CelestialLight {
    /// Smooth, periodic stylized lighting. Invalid phases fall back to fresh-world daylight.
    pub fn sample(hour: f64) -> Self {
        let hour = canonical_hour(hour);
        let angle = ((hour - 6.0) * std::f64::consts::TAU / 24.0) as f32;
        // A tilted orbit avoids a degenerate shadow basis at noon and gives moving side light.
        let tilt = 0.35_f32.atan2(0.78);
        let sun_direction = Vec3::new(
            -angle.cos(),
            angle.sin() * tilt.cos(),
            -angle.sin() * tilt.sin(),
        )
        .normalize();
        let elevation = sun_direction.y;
        let daylight = super::smoothstep(-0.18, 0.20, elevation);
        let sunlight = super::smoothstep(0.0, 0.22, elevation);
        let warm = 1.0 - super::smoothstep(0.04, 0.50, elevation);
        let twilight = warm * super::smoothstep(-0.20, 0.02, elevation);
        Self {
            sun_direction,
            sun_radiance: Vec3::new(1.65, 1.40, 1.10).lerp(Vec3::new(1.85, 0.62, 0.22), warm)
                * sunlight,
            sky_irradiance: Vec3::new(0.010, 0.017, 0.033)
                .lerp(Vec3::new(0.22, 0.34, 0.55), daylight)
                .lerp(Vec3::new(0.24, 0.17, 0.16), twilight * 0.65),
            sky_top: Vec3::new(0.0025, 0.005, 0.014)
                .lerp(Vec3::new(0.12, 0.32, 0.67), daylight)
                .lerp(Vec3::new(0.095, 0.12, 0.31), twilight),
            sky_horizon: Vec3::new(0.011, 0.019, 0.043)
                .lerp(Vec3::new(0.61, 0.77, 0.90), daylight)
                .lerp(Vec3::new(0.75, 0.24, 0.095), twilight),
            clouds: Vec3::new(0.014, 0.020, 0.035)
                .lerp(Vec3::new(0.89, 0.93, 0.97), daylight)
                .lerp(Vec3::new(0.76, 0.37, 0.20), twilight * 0.80),
            daylight,
            stars: 1.0 - super::smoothstep(-0.18, 0.06, elevation),
        }
    }
}

fn canonical_hour(hour: f64) -> f64 {
    if hour.is_finite() {
        hour.rem_euclid(24.0)
    } else {
        DEFAULT_TIME_HOUR
    }
}

/// Refresh once on explicit jumps or solar horizon crossings, not on ordinary clock ticks.
/// This prevents an old daylight probe sweep from surviving a debug jump into night.
pub(crate) fn refresh_probes(previous: Option<f64>, hour: f64) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let previous = canonical_hour(previous);
    let hour = canonical_hour(hour);
    let distance = phase_distance(previous, hour);
    distance > 0.25
        || (CelestialLight::sample(previous).sun_radiance.max_element() > 0.0)
            != (CelestialLight::sample(hour).sun_radiance.max_element() > 0.0)
}

/// Accelerated solar motion must not leave several degrees of stale bounce in the grid.
/// Ordinary time retains 512 probes; fast ticks spend up to the same bounded full grid.
pub(crate) fn solar_probe_budget(previous: Option<f64>, hour: f64) -> u32 {
    use super::volume::{PROBE_COUNT, PROBES_PER_FRAME};
    let Some(previous) = previous else {
        return PROBE_COUNT;
    };
    let distance = phase_distance(canonical_hour(previous), canonical_hour(hour));
    // At most 0.06 solar hours (0.9 degrees) of sweep lag at the current step rate.
    let sweep_frames = (0.06 / distance)
        .floor()
        .clamp(1.0, (PROBE_COUNT / PROBES_PER_FRAME) as f64) as u32;
    PROBE_COUNT
        .div_ceil(sweep_frames)
        .div_ceil(PROBES_PER_FRAME)
        * PROBES_PER_FRAME
}

fn phase_distance(previous: f64, hour: f64) -> f64 {
    let distance = (hour - previous).abs();
    distance.min(24.0 - distance)
}

#[cfg(test)]
mod tests;
