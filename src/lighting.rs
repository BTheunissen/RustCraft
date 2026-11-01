//! Testable daylight and headlamp parameters, independent of windowing and GPU state.

use glam::Vec3;

use crate::world::{Block, CHUNK_SIZE, MIN_WORLD_Y, Terrain, WORLD_DEPTH, WORLD_HEIGHT};

mod exposure;
pub use exposure::{COASTAL_SKY_REACH, camera_cavity_clearance, lateral_exposure};

pub(crate) mod atmosphere;
mod celestial;
pub use celestial::{CelestialLight, DEFAULT_TIME_HOUR};
pub(crate) use celestial::{refresh_probes, solar_probe_budget};
pub(crate) mod hdr;
pub(crate) mod lantern;
pub(crate) mod materials;
pub(crate) mod shadows;
pub(crate) mod transport;
pub(crate) mod volume;

/// Personal light reaches this many blocks before smoothly fading to zero.
pub const HEADLAMP_RANGE: f32 = 28.0;
/// Cosine of the outer edge of the headlamp's broad viewing cone.
pub const HEADLAMP_OUTER_COS: f32 = 0.42;
/// Cosine of the fully illuminated center of the viewing cone.
pub const HEADLAMP_INNER_COS: f32 = 0.88;

/// Cave fog reaches darkness before even the smallest supported residency boundary.
pub fn cave_fog_range(render_distance: i32) -> (f32, f32) {
    let end = (render_distance.clamp(2, 12) as f32 * CHUNK_SIZE as f32 - 12.0).min(52.0);
    (14.0_f32.min(end * 0.55), end)
}

/// Below-horizon background must meet outdoor fog, not expose a black residency ring.
pub fn lower_atmosphere(exposure: f32, horizon: Vec3) -> Vec3 {
    Vec3::new(0.005, 0.007, 0.011).lerp(horizon, exposure.clamp(0.0, 1.0))
}

/// Distance above known ground in the resident camera column, ignoring tree crowns.
/// Unknown residency keeps the outdoor background rather than inventing a cave.
pub fn camera_ground_clearance(
    eye: Vec3,
    mut block_at: impl FnMut(i32, i32, i32) -> Option<Block>,
) -> f32 {
    if !eye.is_finite() {
        return 1000.0;
    }
    let x = eye.x.floor() as i32;
    let z = eye.z.floor() as i32;
    for y in (MIN_WORLD_Y..WORLD_HEIGHT).rev() {
        let Some(block) = block_at(x, y, z) else {
            return 1000.0;
        };
        if block.is_solid() && !matches!(block, Block::Leaves | Block::Wood) {
            return eye.y - (y + 1) as f32;
        }
    }
    1000.0
}

/// A nearby downward cavity should fade dark; high outdoor flight should meet sky fog.
pub fn cavity_background(ground_clearance: f32, direction_y: f32) -> f32 {
    (1.0 - smoothstep(3.0, 6.0, ground_clearance)) * smoothstep(0.02, 0.15, -direction_y)
}

/// Outdoor foliage attenuates local light without replacing the sky with cave atmosphere.
pub fn atmosphere_exposure(underground: bool, exposure: f32) -> f32 {
    if underground {
        exposure.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// Measures unobstructed vertical daylight at the camera's block position.
///
/// Absolute height is not a darkness test: a low open shaft can receive daylight,
/// while a roofed cave above zero cannot. Water and leaves attenuate the ray.
pub fn sky_exposure(eye: Vec3, mut block_at: impl FnMut(i32, i32, i32) -> Block) -> f32 {
    if !eye.is_finite() {
        return 0.0;
    }
    let x = eye.x.floor() as i32;
    let z = eye.z.floor() as i32;
    let first_y = (eye.y.floor() as i32).max(MIN_WORLD_Y);
    let mut exposure = 1.0;
    for y in first_y..WORLD_HEIGHT {
        exposure = daylight_through(exposure, block_at(x, y, z));
        if exposure == 0.0 {
            break;
        }
    }
    exposure
}

pub(crate) fn daylight_through(daylight: f32, block: Block) -> f32 {
    match block {
        Block::Air => daylight,
        Block::Water => daylight * 0.9,
        Block::Leaves => daylight * 0.65,
        _ => 0.0,
    }
}

/// Caches a vertical camera ray until its cell or world occlusion changes.
#[derive(Default)]
pub struct ExposureCache {
    previous: Option<([i32; 3], f32)>,
}

impl ExposureCache {
    /// Invalidates a stationary camera ray after nearby world edits.
    pub fn clear(&mut self) {
        self.previous = None;
    }

    /// Uses one projected terrain column instead of rebuilding cave features for every ray cell.
    pub fn sample_terrain(&mut self, eye: Vec3, terrain: Terrain) -> f32 {
        let mut blocks: Option<[Block; WORLD_DEPTH as usize]> = None;
        self.sample(eye, |x, y, z| {
            let column = blocks.get_or_insert_with(|| terrain.column_blocks(x, z));
            column[(y - MIN_WORLD_Y) as usize]
        })
    }
    /// Updates only at voxel boundaries; mouse look alone cannot change sky occlusion.
    pub fn sample(&mut self, eye: Vec3, block_at: impl FnMut(i32, i32, i32) -> Block) -> f32 {
        if !eye.is_finite() {
            return 0.0;
        }
        let cell = eye.floor().as_ivec3().to_array();
        if let Some((previous, exposure)) = self.previous
            && previous == cell
        {
            return exposure;
        }
        let exposure = sky_exposure(eye, block_at);
        self.previous = Some((cell, exposure));
        exposure
    }
}

/// CPU reference for the shader's headlamp cone and distance falloff.
pub fn headlamp_strength(eye: Vec3, direction: Vec3, point: Vec3, enabled: bool) -> f32 {
    if !enabled || !eye.is_finite() || !direction.is_finite() || !point.is_finite() {
        return 0.0;
    }
    let offset = point - eye;
    let distance = offset.length();
    let alignment = direction
        .normalize_or_zero()
        .dot(offset.normalize_or_zero());
    let cone = smoothstep(HEADLAMP_OUTER_COS, HEADLAMP_INNER_COS, alignment);
    let reach = 1.0 - smoothstep(HEADLAMP_RANGE * 0.65, HEADLAMP_RANGE, distance);
    cone * reach * 1.5 / (1.0 + distance * distance * 0.025)
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests;
