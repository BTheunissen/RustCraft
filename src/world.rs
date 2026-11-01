//! CPU-only world data and deterministic generation.

mod biome;
mod chunk;
mod coastal;
mod coordinates;
mod edits;
mod fluid;
mod generation;
mod resident;

pub use biome::{Biome, CaveBiome, CaveLandform, CaveLandmark, Column};
pub use chunk::Chunk;
pub use coastal::{CoastalArch, CoastalLandmark, CoastalStack};
pub use coordinates::{
    CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, SEA_LEVEL, WORLD_DEPTH, WORLD_HEIGHT,
    split_block_coordinate,
};
pub use edits::{BlockEdit, WorldEdits};
pub use fluid::{WaterBiome, WaterColumn, WaterLandmark, WaterSample};
pub use generation::Terrain;
pub use material::Block;
pub use resident::{ResidentWorld, SampledWorld, WorldView};

mod material;
use coordinates::{MAX_TERRAIN_HEIGHT, MIN_BASE_TERRAIN_HEIGHT, MIN_TERRAIN_HEIGHT};
