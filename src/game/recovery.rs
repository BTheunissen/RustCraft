//! Deterministic, dry supported recovery against the current edited world.

use glam::Vec3;

use super::bounds::Bounds;
use crate::world::{Block, MIN_WORLD_Y, SampledWorld, WORLD_DEPTH, WORLD_HEIGHT, WorldView};

const SEARCH_RADIUS: i32 = 32;

pub(super) fn spawn_position(world: &SampledWorld<'_>, original: Vec3) -> Option<Vec3> {
    if clear_supported(world, original) {
        return Some(original);
    }
    let (origin, _) = Bounds::cube(original, 0.001).cells()?;
    let base_y = original.y.floor() as i32;
    let mut best: Option<(i32, Vec3)> = None;
    for radius in 0..=SEARCH_RADIUS {
        if best.is_some_and(|(score, _)| radius * radius > score) {
            break;
        }
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                let (Some(x), Some(z)) = (origin[0].checked_add(dx), origin[2].checked_add(dz))
                else {
                    continue;
                };
                let horizontal_score = dx * dx + dz * dz;
                for offset in 0..WORLD_DEPTH {
                    let score = horizontal_score + offset * offset;
                    if best.is_some_and(|(best_score, _)| score >= best_score) {
                        break;
                    }
                    for y in [base_y + offset, base_y - offset] {
                        if !(MIN_WORLD_Y + 1..WORLD_HEIGHT - 1).contains(&y) {
                            continue;
                        }
                        let position = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                        if clear_supported(world, position) {
                            best = Some((score, position));
                            break;
                        }
                    }
                    if best.is_some_and(|(best_score, _)| best_score == score) {
                        break;
                    }
                }
            }
        }
    }
    best.map(|(_, position)| position)
}

fn clear_supported(world: &SampledWorld<'_>, position: Vec3) -> bool {
    let Some((min, max)) = Bounds::player(position).cells() else {
        return false;
    };
    if min[1] <= MIN_WORLD_Y || max[1] >= WORLD_HEIGHT {
        return false;
    }
    for x in min[0]..=max[0] {
        for z in min[2]..=max[2] {
            if !world
                .block_at(x, min[1] - 1, z)
                .is_some_and(|block| block.is_solid())
            {
                return false;
            }
            for y in min[1]..=max[1] {
                if world.block_at(x, y, z) != Some(Block::Air) {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests;
