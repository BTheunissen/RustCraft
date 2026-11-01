//! Dry, supported surface spawning without diagnostic shortcuts.

use super::Terrain;
use crate::world::{Biome, Block, SEA_LEVEL, WORLD_HEIGHT};

impl Terrain {
    /// Finds dry, clear ground near the central biome junction.
    /// The returned position is the player's feet, centered on a block.
    pub fn spawn_position(&self) -> [f32; 3] {
        let (offset_x, offset_z) = self.biome_offset();
        let center_x = -offset_x.round() as i32;
        let center_z = -offset_z.round() as i32;
        let target_x = center_x - 16;
        let target_z = center_z + 16;
        let mut best = None;
        let mut fallback = (center_x, center_z, self.column(center_x, center_z));

        for dz in (-64..=64).step_by(4) {
            for dx in (-64..=64).step_by(4) {
                let x = center_x + dx;
                let z = center_z + dz;
                let column = self.column(x, z);
                if column.height > fallback.2.height {
                    fallback = (x, z, column);
                }
                if column.height < SEA_LEVEL + 1 || self.water_column(x, z).is_some() {
                    continue;
                }
                let biome_penalty = match column.biome {
                    Biome::Plains => 0,
                    Biome::Forest => 8_192,
                    Biome::Desert => 12_288,
                    Biome::Alpine => 16_384,
                };
                let score = (x - target_x).pow(2)
                    + (z - target_z).pow(2)
                    + (column.height - 26).abs() * 8
                    + biome_penalty;
                if best.is_some_and(|(_, _, _, best_score)| best_score <= score) {
                    continue;
                }
                if self.spawn_is_clear(x, z, column.height) {
                    best = Some((x, z, column.height + 1, score));
                }
            }
        }

        let (x, z, feet_y) = if let Some((x, z, feet_y, _)) = best {
            (x, z, feet_y)
        } else {
            // The highest dry column remains a safe fallback even if unusually
            // dense trees prevent finding the preferred three-by-three clearing.
            let (x, z, _) = fallback;
            let top = (0..WORLD_HEIGHT)
                .rev()
                .find(|&y| self.block_at(x, y, z).is_solid())
                .unwrap_or(0);
            (x, z, top + 1)
        };
        [x as f32 + 0.5, feet_y as f32, z as f32 + 0.5]
    }

    pub(super) fn spawn_is_clear(&self, x: i32, z: i32, height: i32) -> bool {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let neighbor = self.column(x + dx, z + dz);
                if (neighbor.height - height).abs() > 1 {
                    return false;
                }
                if neighbor.height < SEA_LEVEL + 1
                    || self.water_column(x + dx, z + dz).is_some()
                    || !self.block_at(x + dx, neighbor.height, z + dz).is_solid()
                {
                    return false;
                }
                for y in height + 1..=height + 3 {
                    if self.block_at(x + dx, y, z + dz) != Block::Air {
                        return false;
                    }
                }
            }
        }
        true
    }
}
