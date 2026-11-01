//! Half-open world-space bounds shared by placement, recovery, and pickup physics.

use glam::Vec3;

use crate::player::{PLAYER_HALF_WIDTH, PLAYER_HEIGHT};

const FACE_EPSILON: f64 = 0.000_01;

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bounds {
    pub fn player(position: Vec3) -> Self {
        let [x, y, z] = position.to_array().map(f64::from);
        let half = f64::from(PLAYER_HALF_WIDTH);
        Self {
            min: [x - half, y, z - half],
            max: [x + half, y + f64::from(PLAYER_HEIGHT), z + half],
        }
    }

    pub fn cube(position: Vec3, half: f32) -> Self {
        let center = position.to_array().map(f64::from);
        let half = f64::from(half);
        Self {
            min: center.map(|value| value - half),
            max: center.map(|value| value + half),
        }
    }

    pub fn cells(self) -> Option<([i32; 3], [i32; 3])> {
        let mut min = [0; 3];
        let mut max = [0; 3];
        for axis in 0..3 {
            min[axis] = checked_floor(self.min[axis] + FACE_EPSILON)?;
            max[axis] = checked_floor(self.max[axis] - FACE_EPSILON)?;
            if min[axis] > max[axis] {
                return None;
            }
        }
        Some((min, max))
    }

    pub fn overlaps_block(self, position: [i32; 3]) -> bool {
        (0..3).all(|axis| {
            self.min[axis].is_finite()
                && self.max[axis].is_finite()
                && self.max[axis] > f64::from(position[axis]) + FACE_EPSILON
                && self.min[axis] < f64::from(position[axis]) + 1.0 - FACE_EPSILON
        })
    }
}

pub(super) fn checked_floor(value: f64) -> Option<i32> {
    let value = value.floor();
    (value.is_finite() && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

pub(super) fn block_distance_squared(point: Vec3, position: [i32; 3]) -> f64 {
    point
        .to_array()
        .into_iter()
        .zip(position)
        .map(|(point, minimum)| {
            let point = f64::from(point);
            let minimum = f64::from(minimum);
            (point - point.clamp(minimum, minimum + 1.0)).powi(2)
        })
        .sum()
}

#[cfg(test)]
mod tests;
