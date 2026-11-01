//! Bounded open-sky spill separates short outdoor overhangs from enclosed caves.

use super::{camera_ground_clearance, daylight_through};
use crate::world::{Block, MIN_WORLD_Y, WORLD_HEIGHT};
use glam::Vec3;

/// Maximum horizontal daylight reach under the first coastal arch.
pub const COASTAL_SKY_REACH: i32 = 8;

/// Maximum visible sky along four cardinal air corridors, using preprojected columns.
/// Solid cells stop a ray; water/leaves attenuate it. Work is at most `4 * reach` cells.
pub fn lateral_exposure(
    cell: [i32; 3],
    reach: i32,
    mut block_at: impl FnMut([i32; 3]) -> Block,
    mut vertical_at: impl FnMut([i32; 3]) -> f32,
) -> f32 {
    let mut result = vertical_at(cell);
    for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
        let mut transmission = 1.0;
        for distance in 1..=reach.clamp(0, COASTAL_SKY_REACH) {
            let (Some(x), Some(z)) = (
                cell[0].checked_add(dx * distance),
                cell[2].checked_add(dz * distance),
            ) else {
                break;
            };
            let sample = [x, cell[1], z];
            transmission = daylight_through(transmission, block_at(sample));
            if transmission == 0.0 {
                break;
            }
            result = result.max(vertical_at(sample) * transmission);
        }
    }
    if result.is_finite() {
        result.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A low camera alone is not a cave mouth: darken the lower sky only for a real
/// roofed air interval along the view. Resident-only reads never generate terrain.
pub fn camera_cavity_clearance(
    eye: Vec3,
    direction: Vec3,
    block_at: impl Fn(i32, i32, i32) -> Option<Block>,
) -> f32 {
    if !eye.is_finite() || !direction.is_finite() || direction.length_squared() < 0.01 {
        return 1000.0;
    }
    let direction = direction.normalize_or_zero();
    for distance in [2.0, 4.0, 8.0, 12.0] {
        let cell = (eye + direction * distance).floor().as_ivec3().to_array();
        let [x, y, z] = cell;
        if block_at(x, y, z) != Some(Block::Air) {
            continue;
        }
        let vertical = |[x, y, z]: [i32; 3]| {
            let mut light = 1.0;
            for y in y.max(MIN_WORLD_Y)..WORLD_HEIGHT {
                light = daylight_through(light, block_at(x, y, z).unwrap_or(Block::Air));
                if light == 0.0 {
                    break;
                }
            }
            light
        };
        let solid_sides = [[-1, 0], [1, 0], [0, -1], [0, 1]]
            .into_iter()
            .filter(|&[dx, dz]| {
                let (Some(x), Some(z)) = (x.checked_add(dx), z.checked_add(dz)) else {
                    return false;
                };
                block_at(x, y, z).is_some_and(|block| {
                    block.is_solid() && !matches!(block, Block::Leaves | Block::Wood)
                })
            })
            .count();
        if vertical(cell) == 0.0
            && solid_sides >= 2
            && lateral_exposure(
                cell,
                COASTAL_SKY_REACH,
                |[x, y, z]| block_at(x, y, z).unwrap_or(Block::Stone),
                vertical,
            ) < 0.05
        {
            return camera_ground_clearance(eye, block_at);
        }
    }
    1000.0
}

#[cfg(test)]
mod tests;
