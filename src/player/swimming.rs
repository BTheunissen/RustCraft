//! Actual body immersion, buoyancy, current response, and supported bank exits.

use super::{
    BANK_EXIT_CLEARANCE, COLLISION_EPSILON, FLOAT_DEPTH, FLOAT_RESPONSE, GRAVITY, JUMP_SPEED,
    MAX_BANK_EXIT_RISE, MAX_BANK_SURFACE_RISE, MAX_FLOAT_SPEED, MAX_HORIZONTAL_CURRENT,
    MAX_VERTICAL_CURRENT, MAX_WATER_VERTICAL_SPEED, MIN_VERTICAL_SWIM_SPEED, MovementInput,
    PLAYER_HALF_WIDTH, PLAYER_HEIGHT, Player, SWIM_SPEED, SWIM_VERTICAL_SPEED, SWIMMING_SUBMERSION,
    TERMINAL_SPEED, Vec3, WATER_DRAG, WaterSample, body_overlaps, input_axis, supported,
};

impl Player {
    pub(super) fn swim_step(
        &mut self,
        input: MovementInput,
        horizontal: Vec3,
        walk_speed: f32,
        water: WaterContact,
        dt: f32,
        is_solid: &impl Fn(i32, i32, i32) -> bool,
    ) {
        let strength = (water.fraction * PLAYER_HEIGHT / FLOAT_DEPTH).min(1.0);
        let vertical = input_axis(input.jump, input.descend);
        let direction = (horizontal + Vec3::Y * vertical).normalize_or_zero();
        let swim_speed = SWIM_SPEED * input.speed.factor();
        let stroke_vertical = direction.y * SWIM_VERTICAL_SPEED * input.speed.factor();
        let response = 1.0 - (-WATER_DRAG * dt).exp();
        let target = direction.with_y(0.0) * swim_speed + water.flow.with_y(0.0);
        self.water_velocity += (target - self.water_velocity) * response;
        let velocity = horizontal.normalize_or_zero() * walk_speed * (1.0 - strength)
            + self.water_velocity * strength;
        let blocked_x = self.move_axis(0, velocity.x * dt, is_solid);
        let blocked_z = self.move_axis(2, velocity.z * dt, is_solid);
        if blocked_x {
            self.water_velocity.x = 0.0;
        }
        if blocked_z {
            self.water_velocity.z = 0.0;
        }

        if (blocked_x || blocked_z)
            && input.jump
            && !input.descend
            && !self.water_exit_used
            && water.fraction >= SWIMMING_SUBMERSION
            && let Some(speed) = self.bank_exit_speed(horizontal, water.surface, is_solid)
        {
            self.vertical_velocity = speed;
            self.water_exit_used = true;
        }

        // Preserve an exit impulse long enough to clear the bank. The held-input
        // latch prevents repeated dry-land jumps; a submerged supported step
        // rearms the bank exit so shallow approaches cannot trap the swimmer.
        if self.water_exit_used && input.jump && !input.descend && self.vertical_velocity > 0.0 {
            self.vertical_velocity = (self.vertical_velocity - GRAVITY * dt).max(-TERMINAL_SPEED);
            return;
        }

        let float_velocity = ((water.surface - FLOAT_DEPTH - self.position.y) * FLOAT_RESPONSE)
            .clamp(-MAX_FLOAT_SPEED, MAX_FLOAT_SPEED);
        let mut target_vertical = if vertical == 0.0 {
            float_velocity
        } else {
            stroke_vertical
        } + water.flow.y * strength;
        // A diagonal stroke must still be able to escape a downward current.
        // Currents remain felt, but held vertical controls never reverse intent.
        if vertical > 0.0 {
            target_vertical = target_vertical.max(MIN_VERTICAL_SWIM_SPEED);
        } else if vertical < 0.0 {
            target_vertical = target_vertical.min(-MIN_VERTICAL_SWIM_SPEED);
        }
        if strength >= SWIMMING_SUBMERSION {
            let limit = MAX_WATER_VERTICAL_SPEED.max(stroke_vertical.abs() + MAX_VERTICAL_CURRENT);
            self.vertical_velocity = self.vertical_velocity.clamp(-limit, limit);
        }
        self.vertical_velocity =
            (self.vertical_velocity - GRAVITY * (1.0 - strength) * dt).max(-TERMINAL_SPEED);
        self.vertical_velocity += (target_vertical - self.vertical_velocity)
            * (1.0 - (-WATER_DRAG * strength * dt).exp());
    }

    fn bank_exit_speed(
        &self,
        horizontal: Vec3,
        surface: f32,
        is_solid: &impl Fn(i32, i32, i32) -> bool,
    ) -> Option<f32> {
        if horizontal == Vec3::ZERO || self.position.y < surface - FLOAT_DEPTH {
            return None;
        }
        let ahead =
            self.position + horizontal.normalize_or_zero() * (PLAYER_HALF_WIDTH * 2.0 + 0.05);
        for lift in 1..=2 {
            let candidate = ahead.with_y(self.position.y.floor() + lift as f32);
            let rise = candidate.y - self.position.y;
            if rise > MAX_BANK_EXIT_RISE || candidate.y > surface + MAX_BANK_SURFACE_RISE {
                break;
            }
            if !body_overlaps(candidate, is_solid) && supported(candidate, is_solid) {
                // A submerged body starts below a dry jumper's feet. Add only
                // the lift needed for this verified low shore, not a wall climb.
                return Some(
                    (2.0 * GRAVITY * (rise + BANK_EXIT_CLEARANCE))
                        .sqrt()
                        .max(JUMP_SPEED),
                );
            }
        }
        None
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct WaterContact {
    pub(super) fraction: f32,
    pub(super) surface: f32,
    pub(super) flow: Vec3,
}

pub(super) fn water_contact(
    position: Vec3,
    water_at: &impl Fn(i32, i32, i32) -> Option<WaterSample>,
) -> WaterContact {
    if !position.is_finite() {
        return WaterContact::default();
    }
    let x = position.x.floor() as i32;
    let z = position.z.floor() as i32;
    let feet = f64::from(position.y);
    let top = feet + f64::from(PLAYER_HEIGHT);
    let first = (feet + f64::from(COLLISION_EPSILON)).floor() as i32;
    let last = (top - f64::from(COLLISION_EPSILON)).floor() as i32;
    let mut submerged_height = 0.0;
    let mut surface = 0.0;
    let mut flow = Vec3::ZERO;
    // A 1.8-block upright body intersects at most three voxel heights. Integrate
    // only actual water intervals, so a partial top, air gap or shallow bed does
    // not turn an entire body column into water based on distant surface data.
    for y in first..=last {
        let Some(sample) = water_at(x, y, z) else {
            continue;
        };
        if !sample.surface.is_finite()
            || !sample.salinity.is_finite()
            || !sample.flow.iter().all(|value| value.is_finite())
            || f64::from(sample.surface) <= f64::from(sample.bottom) + 1.0
            || sample.bottom >= y
        {
            continue;
        }
        let lower = feet.max(f64::from(y)).max(f64::from(sample.bottom) + 1.0);
        let upper = top.min(f64::from(y) + 1.0).min(f64::from(sample.surface));
        let height = (upper - lower).max(0.0);
        if height == 0.0 {
            continue;
        }
        let current = Vec3::from_array(sample.flow).clamp(
            Vec3::splat(-MAX_HORIZONTAL_CURRENT),
            Vec3::splat(MAX_HORIZONTAL_CURRENT),
        );
        let current = current.with_y(0.0).clamp_length_max(MAX_HORIZONTAL_CURRENT)
            + Vec3::Y * current.y.clamp(-MAX_VERTICAL_CURRENT, MAX_VERTICAL_CURRENT);
        submerged_height += height;
        surface += f64::from(sample.surface) * height;
        flow += current * height as f32;
    }
    if submerged_height == 0.0 {
        return WaterContact::default();
    }
    WaterContact {
        fraction: (submerged_height / f64::from(PLAYER_HEIGHT)).clamp(0.0, 1.0) as f32,
        surface: (surface / submerged_height) as f32,
        flow: flow / submerged_height as f32,
    }
}
