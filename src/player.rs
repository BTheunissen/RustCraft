//! First-person movement with feet-centered, axis-aligned voxel collision.
//!
//! +Y is up, yaw zero faces -Z, positive yaw turns right, and positive pitch
//! looks up. Application/window input belongs outside this module.

use glam::Vec3;
use std::f32::consts::PI;

use crate::world::WaterSample;

/// Height of the walking collision box, measured from the feet.
pub const PLAYER_HEIGHT: f32 = 1.8;
/// Half the width and depth of the walking collision box.
pub const PLAYER_HALF_WIDTH: f32 = 0.30;
/// Height of the camera above the feet.
pub const EYE_HEIGHT: f32 = 1.62;

const WALK_SPEED: f32 = 5.0;
const GRAVITY: f32 = 24.0;
const JUMP_SPEED: f32 = 7.589_466;
const TERMINAL_SPEED: f32 = 50.0;
const MOUSE_SENSITIVITY: f64 = 0.0025;
const MAX_PITCH: f32 = 89.0 * PI / 180.0;
const MAX_FRAME_DT: f32 = 0.25;
const MAX_STEP_DT: f32 = 1.0 / 120.0;
const COLLISION_EPSILON: f32 = 0.000_01;
const GROUND_PROBE: f32 = 0.001;
const MAX_FLIGHT_EXIT_LIFT: i32 = 64;
const SWIM_SPEED: f32 = 2.5;
const SWIM_VERTICAL_SPEED: f32 = 2.5;
const MIN_VERTICAL_SWIM_SPEED: f32 = 0.5;
const WATER_DRAG: f32 = 6.0;
const FLOAT_DEPTH: f32 = 1.2;
const FLOAT_RESPONSE: f32 = 2.0;
const MAX_FLOAT_SPEED: f32 = 1.5;
const MAX_WATER_VERTICAL_SPEED: f32 = 6.0;
const MAX_HORIZONTAL_CURRENT: f32 = 3.0;
const MAX_VERTICAL_CURRENT: f32 = 2.0;
const SWIMMING_SUBMERSION: f32 = 0.4;
const MAX_BANK_EXIT_RISE: f32 = 2.0;
const MAX_BANK_SURFACE_RISE: f32 = 1.0;
const BANK_EXIT_CLEARANCE: f32 = 0.15;
const BODY_MIN: Vec3 = Vec3::new(-PLAYER_HALF_WIDTH, 0.0, -PLAYER_HALF_WIDTH);
const BODY_MAX: Vec3 = Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);

mod collision;
mod controls;
mod movement;
mod observation;
mod swimming;

use collision::{body_overlaps, supported};
use controls::input_axis;
pub use controls::{MovementInput, SpeedMultiplier};
pub(crate) use observation::{MovementReport, MovementStep};
use swimming::water_contact;

/// A first-person player with a feet-centered walking collider.
#[derive(Clone, Debug)]
pub struct Player {
    /// Center of the feet, in world/block coordinates.
    pub position: Vec3,
    /// Horizontal heading in radians: zero is -Z, positive turns right.
    pub yaw: f32,
    /// Vertical view angle in radians: positive looks up.
    pub pitch: f32,
    /// Creative flight ignores gravity and collision.
    pub flying: bool,
    vertical_velocity: f32,
    grounded: bool,
    jump_held: bool,
    water_velocity: Vec3,
    submersion: f32,
    water_exit_used: bool,
}

impl Player {
    /// Start walking, provisionally grounded until the first world-aware update.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: 0.0,
            flying: false,
            vertical_velocity: 0.0,
            grounded: true,
            jump_held: false,
            water_velocity: Vec3::ZERO,
            submersion: 0.0,
            water_exit_used: false,
        }
    }

    /// Whether the walking collider is supported by solid voxels.
    pub fn is_grounded(&self) -> bool {
        self.grounded
    }

    /// Whether the last update immersed enough of the upright body to swim.
    /// Shallow wading and creative flight are not swimming.
    pub fn is_swimming(&self) -> bool {
        !self.flying && self.submersion >= SWIMMING_SUBMERSION
    }

    /// Fraction of the upright body in actual water during the last update.
    /// The estimate samples one central column; flight always reports zero.
    pub fn submersion(&self) -> f32 {
        if self.flying { 0.0 } else { self.submersion }
    }

    /// Camera position above the center of the feet.
    pub fn eye_position(&self) -> Vec3 {
        self.position + Vec3::Y * EYE_HEIGHT
    }

    /// Unit view direction, including pitch.
    pub fn look_direction(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(yaw_sin * pitch_cos, pitch_sin, -yaw_cos * pitch_cos)
    }

    /// Apply raw mouse deltas, with downward mouse motion looking down.
    ///
    /// Non-finite deltas are ignored as a pair. Yaw wraps to [-pi, pi), and
    /// pitch stops at +/-89 degrees. Calculating in f64 also bounds huge finite
    /// deltas before converting them to the stored f32 angles.
    pub fn look(&mut self, delta_x: f64, delta_y: f64) {
        if !delta_x.is_finite() || !delta_y.is_finite() {
            return;
        }

        let yaw = f64::from(self.yaw) + delta_x * MOUSE_SENSITIVITY;
        self.yaw = ((yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI) as f32;
        self.pitch = (f64::from(self.pitch) - delta_y * MOUSE_SENSITIVITY)
            .clamp(-f64::from(MAX_PITCH), f64::from(MAX_PITCH)) as f32;
    }

    /// Toggle creative flight, resetting vertical momentum.
    ///
    /// Disabling flight first finds a clear walking body at the current position
    /// or a voxel boundary up to 64 blocks above it. If no clearance exists,
    /// flight stays enabled and the position is unchanged.
    pub fn toggle_flight(&mut self, is_solid: impl Fn(i32, i32, i32) -> bool) {
        if !self.flying {
            self.flying = true;
            self.vertical_velocity = 0.0;
            self.grounded = false;
            self.reset_water();
            return;
        }

        let mut candidate = self.position;
        for lift in 0..=MAX_FLIGHT_EXIT_LIFT {
            if lift > 0 {
                candidate.y = self.position.y.floor() + lift as f32;
            }
            if !body_overlaps(candidate, &is_solid) {
                self.position = candidate;
                self.flying = false;
                self.vertical_velocity = 0.0;
                self.grounded = supported(candidate, &is_solid);
                self.reset_water();
                return;
            }
        }
    }

    /// Teleport to new feet coordinates and reset movement, preserving the view.
    ///
    /// Respawning starts walking and rearms jump input. Grounded state is
    /// provisional until the next update checks the world.
    pub fn respawn(&mut self, position: Vec3) {
        self.position = position;
        self.flying = false;
        self.vertical_velocity = 0.0;
        self.grounded = true;
        self.jump_held = false;
        self.reset_water();
    }

    /// Rearms jump edges when application input is cleared, without changing momentum.
    pub fn clear_input(&mut self) {
        self.jump_held = false;
        self.water_exit_used = false;
    }

    fn reset_water(&mut self) {
        self.water_velocity = Vec3::ZERO;
        self.submersion = 0.0;
        self.water_exit_used = false;
    }
}

#[cfg(test)]
mod speed_tests;
#[cfg(test)]
mod tests;
