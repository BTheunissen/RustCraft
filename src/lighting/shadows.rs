//! Stable camera-centered sunlight coverage, with WebGPU zero-to-one depth.

use crate::world::CHUNK_SIZE;
use glam::{Mat4, Vec3};

pub(crate) const SHADOW_CASCADES: usize = 2;
pub(crate) const SHADOW_RESOLUTION: u32 = 2048;

#[derive(Clone, Copy)]
pub(crate) struct ShadowCoverage {
    pub(crate) matrices: [Mat4; SHADOW_CASCADES],
    pub(crate) radii: [f32; SHADOW_CASCADES],
}

pub(crate) fn shadow_coverage(
    eye: Vec3,
    render_distance: i32,
    sun_direction: Vec3,
) -> ShadowCoverage {
    let eye = if eye.is_finite() { eye } else { Vec3::ZERO };
    let fog_end = render_distance.clamp(2, 12) as f32 * CHUNK_SIZE as f32 - 12.0;
    let radii = [(fog_end * 0.42).clamp(12.0, 40.0), fog_end + 16.0];
    let sun = sun_direction
        .try_normalize()
        .filter(|sun| sun.is_finite())
        .unwrap_or_else(|| super::CelestialLight::default().sun_direction);
    let up = if sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let rotation = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, -sun, up);
    let center = rotation.transform_point3(eye);
    let matrices = radii.map(|radius| {
        let texel = radius * 2.0 / SHADOW_RESOLUTION as f32;
        let snapped = Vec3::new(
            (center.x / texel).round() * texel,
            (center.y / texel).round() * texel,
            (center.z / 64.0).floor() * 64.0,
        );
        // Reconstructing look_to from two translated world points perturbs its
        // rotation with every snap. Translate this frame's light basis instead.
        let view = Mat4::from_translation(-snapped - Vec3::Z * 256.0) * rotation;
        let projection = glam::camera::rh::proj::directx::orthographic(
            -radius, radius, -radius, radius, 1.0, 512.0,
        );
        projection * view
    });
    ShadowCoverage { matrices, radii }
}

#[cfg(test)]
mod tests;
