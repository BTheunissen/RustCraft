//! Bounded integration for walking, flight, and water transitions.

use super::{
    GRAVITY, JUMP_SPEED, MAX_FRAME_DT, MAX_STEP_DT, MovementInput, MovementReport, MovementStep,
    Player, SWIMMING_SUBMERSION, TERMINAL_SPEED, Vec3, WALK_SPEED, WaterSample, input_axis,
    supported, water_contact,
};

impl Player {
    /// Advance movement using a read-only world solidity callback.
    ///
    /// Invalid/nonpositive dt is ignored. At most 0.25 seconds is simulated per
    /// call, in steps no longer than 1/120 second; excess stall time is dropped.
    /// Walking jumps are edge-triggered internally, so callers may supply held
    /// Space or a one-update jump pulse. An airborne jump press is not buffered.
    pub fn update(
        &mut self,
        input: MovementInput,
        dt: f32,
        is_solid: impl Fn(i32, i32, i32) -> bool,
    ) {
        self.update_with_water(input, dt, is_solid, |_, _, _| None);
    }

    /// Advance walking/swimming using separate solidity and actual-water samples.
    ///
    /// `water_at` must return metadata only for water voxels, never air or lava.
    /// Partial mean surfaces and negative world Y are supported. Water adds drag,
    /// buoyancy and bounded currents without changing the upright collider;
    /// WASD follows yaw, held Space ascends and held descent dives. Releasing
    /// vertical input gently returns the feet to about 1.2 blocks below the
    /// surface. Holding Space against a bank at most one block above the water
    /// permits a collision-checked exit jump. A landing on a submerged step
    /// rearms it; dry land requires Space release rather than repeated jumps.
    ///
    /// Timing and dry jump semantics match `update`. Flight queries neither
    /// callback and ignores water entirely. Invalid water metadata is ignored.
    pub fn update_with_water(
        &mut self,
        input: MovementInput,
        dt: f32,
        is_solid: impl Fn(i32, i32, i32) -> bool,
        water_at: impl Fn(i32, i32, i32) -> Option<WaterSample>,
    ) {
        self.update_observed(input, dt, is_solid, water_at);
    }

    pub(crate) fn update_observed(
        &mut self,
        input: MovementInput,
        dt: f32,
        is_solid: impl Fn(i32, i32, i32) -> bool,
        water_at: impl Fn(i32, i32, i32) -> Option<WaterSample>,
    ) -> MovementReport {
        let mut report = MovementReport::default();
        if !dt.is_finite() || dt <= 0.0 {
            return report;
        }

        let jump_pressed = input.jump && !self.jump_held;
        self.jump_held = input.jump;
        if !input.jump {
            self.water_exit_used = false;
        }
        let dt = dt.min(MAX_FRAME_DT);
        let speed = WALK_SPEED * input.speed.factor();
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let forward = Vec3::new(yaw_sin, 0.0, -yaw_cos);
        let right = Vec3::new(yaw_cos, 0.0, yaw_sin);
        let horizontal = forward * input_axis(input.forward, input.backward)
            + right * input_axis(input.right, input.left);

        if self.flying {
            let direction =
                (horizontal + Vec3::Y * input_axis(input.jump, input.descend)).normalize_or_zero();
            self.position += direction * speed * dt;
            self.vertical_velocity = 0.0;
            self.grounded = false;
            self.reset_water();
            return report;
        }

        let mut water = water_contact(self.position, &water_at);
        self.submersion = water.fraction;
        self.grounded = self.vertical_velocity <= 0.0 && supported(self.position, &is_solid);
        if jump_pressed && self.grounded && !self.is_swimming() {
            self.vertical_velocity = JUMP_SPEED;
            self.grounded = false;
            self.water_exit_used = water.fraction > 0.0;
        }

        let steps = (dt / MAX_STEP_DT).ceil() as u32;
        let step_dt = dt / steps as f32;
        let displacement = horizontal.normalize_or_zero() * speed * step_dt;
        for observation in report.steps.iter_mut().take(steps as usize) {
            let before = self.position;
            let was_supported = self.grounded;
            let previous_submersion = water.fraction;
            if water.fraction > 0.0 {
                self.swim_step(input, horizontal, speed, water, step_dt, &is_solid);
            } else {
                self.water_velocity = Vec3::ZERO;
                self.move_axis(0, displacement.x, &is_solid);
                self.move_axis(2, displacement.z, &is_solid);
                self.vertical_velocity =
                    (self.vertical_velocity - GRAVITY * step_dt).max(-TERMINAL_SPEED);
            }
            if self.move_axis(1, self.vertical_velocity * step_dt, &is_solid) {
                self.vertical_velocity = 0.0;
            }

            self.grounded = self.vertical_velocity <= 0.0 && supported(self.position, &is_solid);
            if self.grounded {
                self.vertical_velocity = 0.0;
                if water.fraction >= SWIMMING_SUBMERSION && self.position.y < water.surface {
                    self.water_exit_used = false;
                }
            }
            water = water_contact(self.position, &water_at);
            *observation = Some(MovementStep {
                before,
                position: self.position,
                dt: step_dt,
                supported: was_supported && self.grounded,
                previous_submersion,
                submersion: water.fraction,
            });
        }
        self.submersion = water.fraction;
        report
    }
}
