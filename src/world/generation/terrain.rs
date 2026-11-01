//! Public sampling facade and ordering of generation stages.

use super::hydrology::{HydrologyColumn, HydrologyRegion};
use super::surface::hydrology_block;
use super::trees::{TREE_RADIUS, Tree, tree_cells};
use super::underground::{self, UndergroundRegion};
use crate::world::chunk::cave_water_sample;
use crate::world::{
    Block, CHUNK_SIZE, Column, MIN_WORLD_Y, WORLD_DEPTH, WORLD_HEIGHT, WaterColumn, WaterSample,
};

/// An inexpensive, reproducible world sampler with no mutable generation state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terrain {
    pub(super) seed: u64,
}

impl Default for Terrain {
    fn default() -> Self {
        Self::new(0x5255_5354_4352_4146)
    }
}

impl Terrain {
    /// Creates a world sampler with the supplied reproducible seed.
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Returns the seed used for all terrain and decoration sampling.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Samples one column. Height changes are blended even at biome boundaries.
    pub fn column(&self, x: i32, z: i32) -> Column {
        self.hydrology_column(x, z).column
    }

    /// Samples surface water only; cave pools have independent bounded basins.
    pub fn water_column(&self, x: i32, z: i32) -> Option<WaterColumn> {
        self.hydrology_column(x, z).water
    }

    /// Generated coastal overhang, not a ground cap or fluid interval.
    pub fn coastal_roof(&self, x: i32, z: i32) -> Option<[i32; 2]> {
        HydrologyRegion::columns(self.seed, x, x, z, z).coastal_roof(self, x, z)
    }

    pub(crate) fn coastal_roof_batch(&self, positions: &[[i32; 2]]) -> Vec<Option<[i32; 2]>> {
        let Some(&[first_x, first_z]) = positions.first() else {
            return Vec::new();
        };
        let [min_x, max_x, min_z, max_z] = positions.iter().fold(
            [first_x, first_x, first_z, first_z],
            |[min_x, max_x, min_z, max_z], &[x, z]| {
                [min_x.min(x), max_x.max(x), min_z.min(z), max_z.max(z)]
            },
        );
        if i64::from(max_x) - i64::from(min_x) > i64::from(CHUNK_SIZE * 4)
            || i64::from(max_z) - i64::from(min_z) > i64::from(CHUNK_SIZE * 4)
        {
            return positions
                .iter()
                .map(|&[x, z]| self.coastal_roof(x, z))
                .collect();
        }
        let region = HydrologyRegion::columns(self.seed, min_x, max_x, min_z, max_z);
        positions
            .iter()
            .map(|&[x, z]| region.coastal_roof(self, x, z))
            .collect()
    }

    /// Samples only actual water occupancy, including the original cave pools.
    /// Use resident `Chunk::water` when available to avoid procedural fallback.
    pub fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return None;
        }
        let profile = self.hydrology_column(x, z);
        if let Some(water) = profile.water.filter(|water| water.contains_y(y)) {
            return Some(water);
        }
        if y > profile.column.height {
            return None;
        }
        cave_water_sample(&self.column_blocks(x, z), y)
    }

    pub(in crate::world::generation) fn hydrology_column(&self, x: i32, z: i32) -> HydrologyColumn {
        HydrologyRegion::columns(self.seed, x, x, z, z).column(self, x, z)
    }

    /// Samples terrain, caves, liquids, geology, and trees in world coordinates.
    /// Coordinates outside `MIN_WORLD_Y..WORLD_HEIGHT` in Y are air.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return Block::Air;
        }
        let profile = self.hydrology_column(x, z);
        let column = profile.column;
        let terrain_block = hydrology_block(profile, y);
        if y <= column.height {
            return underground::sample(self, x, y, z, column, terrain_block, profile.seal_from);
        }
        if terrain_block != Block::Air {
            return terrain_block;
        }
        if profile.coastal.is_some() {
            return Block::Air;
        }

        let mut result = Block::Air;
        for cell_z in tree_cells(i64::from(z), i64::from(z)) {
            for cell_x in tree_cells(i64::from(x), i64::from(x)) {
                let Some(candidate) = self.tree_candidate(cell_x, cell_z) else {
                    continue;
                };
                if (i64::from(candidate.x) - i64::from(x)).abs() > TREE_RADIUS
                    || (i64::from(candidate.z) - i64::from(z)).abs() > TREE_RADIUS
                {
                    continue;
                }
                let tree_column = if candidate.x == x && candidate.z == z {
                    column
                } else {
                    self.column(candidate.x, candidate.z)
                };
                if let Some(tree) = Tree::from_candidate(candidate, tree_column)
                    && self.water_column(candidate.x, candidate.z).is_none()
                    && self
                        .hydrology_column(candidate.x, candidate.z)
                        .coastal
                        .is_none()
                    && self
                        .block_at(candidate.x, tree_column.height, candidate.z)
                        .is_solid()
                {
                    match tree.block_at(x, y, z) {
                        Block::Wood => return Block::Wood,
                        Block::Leaves => result = Block::Leaves,
                        _ => {}
                    }
                }
            }
        }
        result
    }

    /// Samples a complete world-Y column with shared cave feature projections.
    /// Index `0` is `MIN_WORLD_Y`. Prefer this for skylight or mesh boundary rays.
    pub fn column_blocks(&self, x: i32, z: i32) -> [Block; WORLD_DEPTH as usize] {
        let region = UndergroundRegion::vertical_column(self, x, z);
        let hydrology = HydrologyRegion::columns(self.seed, x, x, z, z);
        self.column_blocks_in_region(x, z, &region, &hydrology)
    }

    /// Samples nearby world `[x, z]` columns in input order, sharing one feature
    /// projection region. Index `0` of each result is `MIN_WORLD_Y`.
    ///
    /// Use this for a mesh's boundary rays rather than rebuilding the same cave
    /// and growth roots for every ray. Empty inputs are empty; repeated or
    /// negative coordinates are valid. Batches spanning more than four chunks
    /// on either axis fall back to individual bounded samplers.
    pub fn column_block_batch(&self, positions: &[[i32; 2]]) -> Vec<[Block; WORLD_DEPTH as usize]> {
        let Some(&[first_x, first_z]) = positions.first() else {
            return Vec::new();
        };
        let [min_x, max_x, min_z, max_z] = positions.iter().fold(
            [first_x, first_x, first_z, first_z],
            |[min_x, max_x, min_z, max_z], &[x, z]| {
                [min_x.min(x), max_x.max(x), min_z.min(z), max_z.max(z)]
            },
        );
        let bounded = i64::from(max_x) - i64::from(min_x) <= i64::from(CHUNK_SIZE * 4)
            && i64::from(max_z) - i64::from(min_z) <= i64::from(CHUNK_SIZE * 4);
        if !bounded {
            return positions
                .iter()
                .map(|&[x, z]| self.column_blocks(x, z))
                .collect();
        }
        let region = UndergroundRegion::columns(self, min_x, max_x, min_z, max_z);
        let hydrology = HydrologyRegion::columns(self.seed, min_x, max_x, min_z, max_z);
        positions
            .iter()
            .map(|&[x, z]| self.column_blocks_in_region(x, z, &region, &hydrology))
            .collect()
    }

    pub(super) fn column_blocks_in_region(
        &self,
        x: i32,
        z: i32,
        region: &UndergroundRegion,
        hydrology: &HydrologyRegion,
    ) -> [Block; WORLD_DEPTH as usize] {
        let profile = hydrology.column(self, x, z);
        let surface = profile.column;
        let underground = region.column_with_surface_seal(x, z, surface, profile.seal_from);
        let mut blocks = std::array::from_fn(|index| {
            let y = MIN_WORLD_Y + index as i32;
            let base = hydrology_block(profile, y);
            if y <= surface.height {
                underground.block(y, base)
            } else {
                base
            }
        });
        if profile.coastal.is_some() {
            return blocks;
        }
        for cell_z in tree_cells(i64::from(z), i64::from(z)) {
            for cell_x in tree_cells(i64::from(x), i64::from(x)) {
                let Some(candidate) = self.tree_candidate(cell_x, cell_z) else {
                    continue;
                };
                if (i64::from(candidate.x) - i64::from(x)).abs() > TREE_RADIUS
                    || (i64::from(candidate.z) - i64::from(z)).abs() > TREE_RADIUS
                {
                    continue;
                }
                let root_profile = if candidate.x == x && candidate.z == z {
                    profile
                } else {
                    hydrology.column(self, candidate.x, candidate.z)
                };
                if root_profile.water.is_some() || root_profile.coastal.is_some() {
                    continue;
                }
                let root = root_profile.column;
                let Some(tree) = Tree::from_candidate(candidate, root) else {
                    continue;
                };
                let support = if candidate.x == x && candidate.z == z {
                    blocks[(root.height - MIN_WORLD_Y) as usize]
                } else {
                    region
                        .column(candidate.x, candidate.z, root)
                        .block(root.height, hydrology_block(root_profile, root.height))
                };
                if !support.is_solid() {
                    continue;
                }
                for y in tree.base_y.max(surface.height + 1)..=tree.base_y + tree.trunk_height + 1 {
                    let decoration = tree.block_at(x, y, z);
                    let block = &mut blocks[(y - MIN_WORLD_Y) as usize];
                    if decoration != Block::Air
                        && (*block == Block::Air
                            || (*block == Block::Leaves && decoration == Block::Wood))
                    {
                        *block = decoration;
                    }
                }
            }
        }
        blocks
    }
}
