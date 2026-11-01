//! Deterministic generators; only the terrain sampler is public.

mod chunks;
mod coastal;
mod hydrology;
mod landmarks;
mod noise;
mod spawn;
mod surface;
mod terrain;
mod trees;
mod underground;

pub use terrain::Terrain;

#[cfg(test)]
use super::{
    Biome, Block, CHUNK_SIZE, Chunk, ChunkCoord, MAX_TERRAIN_HEIGHT, MIN_BASE_TERRAIN_HEIGHT,
    MIN_TERRAIN_HEIGHT, MIN_WORLD_Y, SEA_LEVEL, WORLD_DEPTH, WORLD_HEIGHT, split_block_coordinate,
};
#[cfg(test)]
use surface::terrain_block;
#[cfg(test)]
use trees::{TREE_RADIUS, Tree, TreeCandidate, tree_cells};
#[cfg(test)]
mod tests;
