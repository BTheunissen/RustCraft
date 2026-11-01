//! Frame inputs and the single CPU/WGSL camera-layout contract.

use super::phase_material_table;
use crate::atlas::TILE_SIZE;
use crate::camera::view_projection;
use crate::game::{BlockTarget, Item, Pickup};
use crate::hud::HudVertex;
use crate::lighting::{
    CelestialLight, HEADLAMP_INNER_COS, HEADLAMP_OUTER_COS, HEADLAMP_RANGE, camera_cavity_clearance,
};
use crate::phase::{PHASE_LAYERS, PHASE_OPACITY, PHASE_RANGE};
use crate::player::Player;
use crate::water::underwater_optics;
use crate::world::{Block, CHUNK_SIZE, ResidentWorld, WaterSample};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct CameraUniform {
    pub(super) view_projection: [[f32; 4]; 4],
    pub(super) inverse_ray_projection: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) sky_top: [f32; 4],
    pub(super) sky_horizon: [f32; 4],
    pub(super) sun: [f32; 4],
    pub(super) settings: [f32; 4],
    pub(super) look: [f32; 4],
    pub(super) environment: [f32; 4],
    pub(super) lamp: [f32; 4],
    pub(super) phase_origin: [i32; 4],
    pub(super) phase_settings: [f32; 4],
    pub(super) phase_materials: [[u32; 4]; 8],
    pub(super) water_fog: [f32; 4],
    pub(super) water_optics: [f32; 4],
    pub(super) atmosphere: [f32; 4],
    pub(super) clouds: [f32; 4],
}

/// Borrowed survival presentation; gameplay remains the authority for drops and reach.
#[derive(Clone, Copy)]
pub struct SurvivalFrame<'a> {
    /// World-space dropped stacks, rendered without copying resident chunks.
    pub pickups: &'a [Pickup],
    /// Selected stack's block, or a bare hand when the selected slot is empty.
    pub held: Option<Item>,
    /// Gameplay's reachable, resident solid target; stale targets are also rejected here.
    pub target: Option<BlockTarget>,
    /// Fraction of the current block's mining duration, clamped to `0..=1`.
    pub mining_progress: f32,
    /// Only first-person presentation is hidden by pause/menu states; drops remain visible.
    pub show_hand: bool,
}

/// Everything needed to draw one frame, also used for offscreen verification.
pub struct FrameState<'a> {
    /// Read-only CPU residency; presentation never owns or generates chunks.
    pub world: &'a ResidentWorld,
    /// Camera pose from renderer-independent movement logic.
    pub player: &'a Player,
    /// Triangle-list HUD geometry.
    pub hud: &'a [HudVertex],
    /// Survival-only overlays; creative diagnostics leave this absent.
    pub survival: Option<SurvivalFrame<'a>>,
    /// Session animation seconds, paused during inventory/pause; menus may use wall time.
    pub elapsed: f32,
    /// Independent solar hour; time acceleration never speeds up clouds, water, or lava.
    pub time_of_day: f64,
    /// Optional water-only animation time; diagnostics hold sky/lava time fixed.
    pub water_time: Option<f32>,
    /// Resident radius in chunk columns; fog hides the streaming boundary.
    pub render_distance: i32,
    /// Enables underwater tint and short-distance fog.
    pub underwater: bool,
    /// Actual camera-fluid metadata, shared with currents and water-surface rendering.
    pub water: Option<WaterSample>,
    /// Camera-column daylight. Zero means roofed terrain, regardless of absolute Y.
    pub sky_exposure: f32,
    /// Toggles personal illumination without changing terrain or simulation.
    pub headlamp: bool,
    /// Enables a warm tint when the camera is inside non-damaging lava scenery.
    pub in_lava: bool,
}

/// Basic scene-only readback statistics, excluding HUD margins and controls.
#[derive(Clone, Copy, Debug)]
pub struct FrameStats {
    /// Mean sRGB luminance over the central scene rectangle.
    pub mean_luminance: f32,
    /// Fraction of pixels that are not near-black.
    pub visible_fraction: f32,
    /// Scene contrast; distinguishes textured terrain from a uniformly tinted void.
    pub luminance_deviation: f32,
}

/// Pixel differences in the compared region; diagnostics choose scene-only or full-frame.
#[derive(Clone, Copy, Debug)]
pub struct FrameDifference {
    /// Mean absolute sRGB channel difference in `0..=1`.
    pub mean_absolute_error: f32,
    /// Fraction of pixels with a change larger than two 8-bit channel levels.
    pub changed_fraction: f32,
}

impl CameraUniform {
    pub(super) fn from_frame(
        frame: &FrameState<'_>,
        aspect: f32,
        phase_origin: [i32; 3],
        phasing: bool,
        volumetrics: bool,
        world_block: impl Fn(i32, i32, i32) -> Option<Block>,
    ) -> Self {
        let eye = frame.player.eye_position();
        let [phase_x, phase_y, phase_z] = phase_origin;
        let water = underwater_optics(frame.water, eye.y);
        let celestial = CelestialLight::sample(frame.time_of_day);
        let radius = frame.render_distance as f32 * CHUNK_SIZE as f32;
        let matrix = view_projection(
            eye,
            frame.player.look_direction(),
            aspect,
            radius * 2.0 + 64.0,
        );
        Self {
            view_projection: matrix.to_cols_array_2d(),
            inverse_ray_projection: view_projection(
                glam::Vec3::ZERO,
                frame.player.look_direction(),
                aspect,
                radius * 2.0 + 64.0,
            )
            .inverse()
            .to_cols_array_2d(),
            eye: eye.extend(1.0).to_array(),
            sky_top: celestial.sky_top.extend(celestial.stars).to_array(),
            sky_horizon: celestial.sky_horizon.extend(celestial.daylight).to_array(),
            sun: celestial.sun_direction.extend(0.0).to_array(),
            settings: [
                frame.elapsed,
                radius * 0.50,
                radius - 12.0,
                if frame.underwater { 1.0 } else { 0.0 },
            ],
            look: frame
                .player
                .look_direction()
                .extend(if frame.sky_exposure > 0.05 {
                    camera_cavity_clearance(eye, frame.player.look_direction(), world_block)
                } else {
                    1000.0
                })
                .to_array(),
            environment: [
                frame.sky_exposure,
                if frame.headlamp { 1.0 } else { 0.0 },
                if frame.in_lava { 1.0 } else { 0.0 },
                1.0,
            ],
            lamp: [
                HEADLAMP_RANGE,
                HEADLAMP_OUTER_COS,
                HEADLAMP_INNER_COS,
                TILE_SIZE as f32,
            ],
            phase_origin: [phase_x, phase_y, phase_z, Block::COUNT as i32],
            phase_settings: [
                if phasing { 1.0 } else { 0.0 },
                PHASE_RANGE,
                PHASE_OPACITY,
                PHASE_LAYERS as f32,
            ],
            phase_materials: phase_material_table(),
            water_fog: [
                water.color[0],
                water.color[1],
                water.color[2],
                water.density,
            ],
            water_optics: [
                water.depth,
                water.salinity,
                if frame.water.is_some() { 1.0 } else { 0.0 },
                water_animation_time(frame.elapsed, frame.water_time),
            ],
            atmosphere: {
                let mut parameters = crate::lighting::atmosphere::parameters(
                    volumetrics,
                    frame.underwater,
                    frame.in_lava,
                    phasing,
                );
                parameters[1] *= celestial.daylight;
                parameters
            },
            clouds: celestial.clouds.extend(0.0).to_array(),
        }
    }
}

pub(super) fn water_animation_time(elapsed: f32, override_time: Option<f32>) -> f32 {
    override_time
        .filter(|time| time.is_finite())
        .unwrap_or(if elapsed.is_finite() { elapsed } else { 0.0 })
}
