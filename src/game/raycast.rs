//! Resident-only voxel traversal, including deterministic simultaneous face crossings.

use glam::Vec3;

use super::bounds::checked_floor;
use crate::world::{Block, ResidentWorld, WorldView};

/// Survival interaction reach in world/block units.
pub const INTERACTION_REACH: f32 = 4.5;

/// A solid voxel intersected by the view ray. Bedrock can be highlighted, not mined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockTarget {
    pub position: [i32; 3],
    /// Outward entry-face normal; zero only when starting inside a solid voxel.
    pub normal: [i32; 3],
    pub block: Block,
    pub distance: f32,
}

/// Finds the first resident solid within 4.5 blocks, stopping at unknown residency.
///
/// Liquids are transparent to targeting. At an edge/corner crossing all tied axes
/// advance together, avoiding zero-length side-voxel hits; the normal uses X, Y,
/// then Z priority. A ray starting on a boundary uses its forward-side voxel.
pub fn raycast(world: &ResidentWorld, origin: Vec3, direction: Vec3) -> Option<BlockTarget> {
    match trace(
        world,
        origin,
        direction,
        f64::from(INTERACTION_REACH),
        false,
    ) {
        RayEnd::Hit(target) => Some(target),
        RayEnd::Clear | RayEnd::Unknown => None,
    }
}

enum RayEnd {
    Clear,
    Hit(BlockTarget),
    Unknown,
}

pub(super) fn segment_clear(world: &ResidentWorld, start: Vec3, end: Vec3) -> bool {
    if !start.is_finite() || !end.is_finite() {
        return false;
    }
    let direction = end - start;
    let distance = direction
        .to_array()
        .map(f64::from)
        .map(|v| v * v)
        .iter()
        .sum::<f64>()
        .sqrt();
    if distance == 0.0 {
        let [x, y, z] = start.to_array().map(f64::from).map(checked_floor);
        return match (x, y, z) {
            (Some(x), Some(y), Some(z)) => world
                .block_at(x, y, z)
                .is_some_and(|block| !block.is_solid()),
            _ => false,
        };
    }
    matches!(
        trace(world, start, direction, distance, true),
        RayEnd::Clear
    )
}

fn trace(
    world: &ResidentWorld,
    origin: Vec3,
    direction: Vec3,
    limit: f64,
    conservative: bool,
) -> RayEnd {
    if !origin.is_finite() || !direction.is_finite() || !limit.is_finite() || limit < 0.0 {
        return RayEnd::Unknown;
    }
    let origin = origin.to_array().map(f64::from);
    let mut direction = direction.to_array().map(f64::from);
    let length = direction
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if length == 0.0 {
        return RayEnd::Unknown;
    }
    for value in &mut direction {
        *value /= length;
    }

    let mut cell = [0; 3];
    let mut step = [0; 3];
    let mut next = [f64::INFINITY; 3];
    let mut stride = [f64::INFINITY; 3];
    let mut normal = [0; 3];
    for axis in 0..3 {
        step[axis] = if direction[axis] > 0.0 {
            1
        } else if direction[axis] < 0.0 {
            -1
        } else {
            0
        };
        let on_boundary = origin[axis] == origin[axis].floor();
        let coordinate = origin[axis].floor() - f64::from(u8::from(on_boundary && step[axis] < 0));
        let Some(coordinate) = checked_floor(coordinate) else {
            return RayEnd::Unknown;
        };
        cell[axis] = coordinate;
        if step[axis] != 0 {
            let boundary = i64::from(coordinate) + i64::from(step[axis] > 0);
            next[axis] = (boundary as f64 - origin[axis]) / direction[axis];
            stride[axis] = direction[axis].abs().recip();
            if on_boundary && normal == [0; 3] {
                normal[axis] = -step[axis];
            }
        }
    }

    let mut distance = 0.0;
    loop {
        let Some(block) = world.block_at(cell[0], cell[1], cell[2]) else {
            return RayEnd::Unknown;
        };
        if block.is_solid() {
            return RayEnd::Hit(BlockTarget {
                position: cell,
                normal,
                block,
                distance: distance as f32,
            });
        }
        let crossing = next.into_iter().fold(f64::INFINITY, f64::min);
        if crossing > limit {
            return RayEnd::Clear;
        }
        distance = crossing;
        normal = [0; 3];
        let tied = next.map(|next| (next - crossing).abs() <= 1.0e-12);
        // Visibility cannot slip between two walls through a zero-width diagonal
        // corner. Targeting intentionally skips these merely touched voxels.
        if conservative && !crossed_sides_clear(world, cell, step, tied) {
            return RayEnd::Unknown;
        }
        for axis in 0..3 {
            if tied[axis] {
                let Some(coordinate) = cell[axis].checked_add(step[axis]) else {
                    return RayEnd::Unknown;
                };
                cell[axis] = coordinate;
                next[axis] += stride[axis];
                if normal == [0; 3] {
                    normal[axis] = -step[axis];
                }
            }
        }
    }
}

fn crossed_sides_clear(
    world: &ResidentWorld,
    cell: [i32; 3],
    step: [i32; 3],
    tied: [bool; 3],
) -> bool {
    let mask = (0..3).fold(0_u8, |mask, axis| mask | (u8::from(tied[axis]) << axis));
    for subset in 1..mask {
        if subset & !mask != 0 {
            continue;
        }
        let mut side = cell;
        for axis in 0..3 {
            if subset & (1 << axis) != 0 {
                let Some(coordinate) = side[axis].checked_add(step[axis]) else {
                    return false;
                };
                side[axis] = coordinate;
            }
        }
        if !world
            .block_at(side[0], side[1], side[2])
            .is_some_and(|block| !block.is_solid())
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
