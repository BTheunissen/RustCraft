//! Batched chunk generation and decoration application.

use super::Terrain;
use super::hydrology::{HydrologyColumn, HydrologyRegion};
use super::surface::hydrology_block;
use super::trees::{TREE_RADIUS, Tree, tree_cells};
use super::underground::UndergroundRegion;
use crate::world::chunk::block_index;
use crate::world::{Block, CHUNK_SIZE, Chunk, ChunkCoord, Column, MIN_WORLD_Y, WORLD_DEPTH};

pub(super) const COLUMN_COUNT: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;
pub(super) const BLOCK_COUNT: usize = COLUMN_COUNT * WORLD_DEPTH as usize;

impl Chunk {
    /// Generates columns once, then applies trees from a world-space halo.
    /// Chunk coordinates beyond the representable `i32` block world are empty.
    pub fn generate(terrain: &Terrain, coord: ChunkCoord) -> Self {
        let mut chunk = Self {
            coord,
            blocks: vec![Block::Air; BLOCK_COUNT].into_boxed_slice(),
            water_columns: Box::new([None; COLUMN_COUNT]),
            coastal_roofs: Box::new([None; COLUMN_COUNT]),
        };
        let origin_x = i64::from(coord.x) * i64::from(CHUNK_SIZE);
        let origin_z = i64::from(coord.z) * i64::from(CHUNK_SIZE);
        let (Ok(world_x), Ok(world_z), Ok(_), Ok(_)) = (
            i32::try_from(origin_x),
            i32::try_from(origin_z),
            i32::try_from(origin_x + i64::from(CHUNK_SIZE - 1)),
            i32::try_from(origin_z + i64::from(CHUNK_SIZE - 1)),
        ) else {
            return chunk;
        };

        let hydrology = HydrologyRegion::columns(
            terrain.seed(),
            world_x,
            world_x + (CHUNK_SIZE - 1),
            world_z,
            world_z + (CHUNK_SIZE - 1),
        );
        let profiles: [HydrologyColumn; COLUMN_COUNT] = std::array::from_fn(|index| {
            hydrology.column(
                terrain,
                world_x + (index % CHUNK_SIZE as usize) as i32,
                world_z + (index / CHUNK_SIZE as usize) as i32,
            )
        });
        let columns = profiles.map(|profile| profile.column);
        *chunk.water_columns = profiles.map(|profile| profile.water);
        *chunk.coastal_roofs = profiles.map(|profile| profile.coastal.and_then(|coast| coast.roof));
        let underground = UndergroundRegion::chunk(terrain, world_x, world_z);
        let (block_columns, _) = chunk.blocks.as_chunks_mut::<{ WORLD_DEPTH as usize }>();
        for (index, (profile, blocks)) in profiles.iter().zip(block_columns).enumerate() {
            let x = world_x + (index % CHUNK_SIZE as usize) as i32;
            let z = world_z + (index / CHUNK_SIZE as usize) as i32;
            let column = profile.column;
            let underground_column =
                underground.column_with_surface_seal(x, z, column, profile.seal_from);
            for (offset, block) in blocks.iter_mut().enumerate() {
                let y = MIN_WORLD_Y + offset as i32;
                let base = hydrology_block(*profile, y);
                *block = if y <= column.height {
                    underground_column.block(y, base)
                } else {
                    base
                };
            }
        }

        let end_x = origin_x + i64::from(CHUNK_SIZE - 1);
        let end_z = origin_z + i64::from(CHUNK_SIZE - 1);
        for cell_z in tree_cells(origin_z, end_z) {
            for cell_x in tree_cells(origin_x, end_x) {
                let Some(candidate) = terrain.tree_candidate(cell_x, cell_z) else {
                    continue;
                };
                let local_x = i64::from(candidate.x) - origin_x;
                let local_z = i64::from(candidate.z) - origin_z;
                if local_x < -TREE_RADIUS
                    || local_x >= i64::from(CHUNK_SIZE) + TREE_RADIUS
                    || local_z < -TREE_RADIUS
                    || local_z >= i64::from(CHUNK_SIZE) + TREE_RADIUS
                {
                    continue;
                }
                let root_profile = if (0..i64::from(CHUNK_SIZE)).contains(&local_x)
                    && (0..i64::from(CHUNK_SIZE)).contains(&local_z)
                {
                    profiles[local_z as usize * CHUNK_SIZE as usize + local_x as usize]
                } else {
                    hydrology.column(terrain, candidate.x, candidate.z)
                };
                if root_profile.water.is_some() || root_profile.coastal.is_some() {
                    continue;
                }
                let column = root_profile.column;
                let supported = if (0..i64::from(CHUNK_SIZE)).contains(&local_x)
                    && (0..i64::from(CHUNK_SIZE)).contains(&local_z)
                {
                    chunk
                        .block(local_x as i32, column.height, local_z as i32)
                        .is_solid()
                } else {
                    terrain
                        .block_at(candidate.x, column.height, candidate.z)
                        .is_solid()
                };
                if let Some(tree) = Tree::from_candidate(candidate, column)
                    && supported
                {
                    chunk.apply_tree(tree, world_x, world_z, &columns);
                }
            }
        }
        // Neighboring canopies must not intrude on the same coastal air mask
        // used by direct collision and full-column sampling.
        for (profile, blocks) in profiles
            .iter()
            .zip(chunk.blocks.as_chunks_mut::<{ WORLD_DEPTH as usize }>().0)
        {
            if profile.coastal.is_some() {
                for (offset, block) in blocks.iter_mut().enumerate() {
                    let y = MIN_WORLD_Y + offset as i32;
                    if y > profile.column.height {
                        *block = hydrology_block(*profile, y);
                    }
                }
            }
        }
        chunk
    }

    pub(super) fn apply_tree(
        &mut self,
        tree: Tree,
        origin_x: i32,
        origin_z: i32,
        columns: &[Column; COLUMN_COUNT],
    ) {
        let local_x = i64::from(tree.x) - i64::from(origin_x);
        let local_z = i64::from(tree.z) - i64::from(origin_z);
        let min_x = (local_x - TREE_RADIUS).max(0) as i32;
        let max_x = (local_x + TREE_RADIUS).min(i64::from(CHUNK_SIZE - 1)) as i32;
        let min_z = (local_z - TREE_RADIUS).max(0) as i32;
        let max_z = (local_z + TREE_RADIUS).min(i64::from(CHUNK_SIZE - 1)) as i32;

        for z in min_z..=max_z {
            for x in min_x..=max_x {
                for y in tree.base_y..=tree.base_y + tree.trunk_height + 1 {
                    if y <= columns[(z * CHUNK_SIZE + x) as usize].height {
                        continue;
                    }
                    let decoration = tree.block_at(origin_x + x, y, origin_z + z);
                    let block = &mut self.blocks[block_index(x, y, z)];
                    // Wood wins overlapping canopies; neither decoration replaces
                    // terrain or water. The sampler uses exactly the same priority.
                    if decoration != Block::Air
                        && (*block == Block::Air
                            || (*block == Block::Leaves && decoration == Block::Wood))
                    {
                        *block = decoration;
                    }
                }
            }
        }
    }
}
