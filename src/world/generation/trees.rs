//! Rooted, seam-consistent tree decorations.

use super::Terrain;
use super::noise::hash_2d;
use crate::world::{Biome, Block, Column, SEA_LEVEL};

pub(super) const TREE_CELL_SIZE: i64 = 8;
pub(super) const TREE_RADIUS: i64 = 2;

impl Terrain {
    pub(super) fn tree_candidate(&self, cell_x: i64, cell_z: i64) -> Option<TreeCandidate> {
        let hash = hash_2d(self.seed ^ 0x1A36D, cell_x, cell_z);
        if hash % 100 >= 85 {
            return None;
        }
        Some(TreeCandidate {
            x: i32::try_from(cell_x * TREE_CELL_SIZE + 1 + ((hash >> 8) % 6) as i64).ok()?,
            z: i32::try_from(cell_z * TREE_CELL_SIZE + 1 + ((hash >> 24) % 6) as i64).ok()?,
            hash,
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct TreeCandidate {
    pub(super) x: i32,
    pub(super) z: i32,
    pub(super) hash: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Tree {
    pub(super) x: i32,
    pub(super) z: i32,
    pub(super) base_y: i32,
    pub(super) trunk_height: i32,
}

impl Tree {
    pub(super) fn from_candidate(candidate: TreeCandidate, column: Column) -> Option<Self> {
        let density = match column.biome {
            Biome::Forest => 85,
            Biome::Plains => 14,
            Biome::Desert | Biome::Alpine => 0,
        };
        if candidate.hash % 100 >= density || column.height < SEA_LEVEL + 1 {
            return None;
        }
        Some(Self {
            x: candidate.x,
            z: candidate.z,
            base_y: column.height + 1,
            trunk_height: 4 + ((candidate.hash >> 48) % 3) as i32,
        })
    }

    pub(super) fn block_at(self, x: i32, y: i32, z: i32) -> Block {
        let dx = (i64::from(x) - i64::from(self.x)).abs();
        let dz = (i64::from(z) - i64::from(self.z)).abs();
        let dy = y - self.base_y;
        if dx == 0 && dz == 0 && (0..self.trunk_height).contains(&dy) {
            return Block::Wood;
        }
        let leaf_layer = dy - self.trunk_height;
        let has_leaves = match leaf_layer {
            -2 | -1 => dx <= TREE_RADIUS && dz <= TREE_RADIUS && dx + dz <= 3,
            0 => dx <= 1 && dz <= 1,
            1 => dx + dz <= 1,
            _ => false,
        };
        if has_leaves {
            Block::Leaves
        } else {
            Block::Air
        }
    }
}

pub(super) fn tree_cells(min: i64, max: i64) -> std::ops::RangeInclusive<i64> {
    (min - TREE_RADIUS).div_euclid(TREE_CELL_SIZE)..=(max + TREE_RADIUS).div_euclid(TREE_CELL_SIZE)
}
