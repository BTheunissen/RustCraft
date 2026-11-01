//! CPU chunk residency shared by simulation and presentation, with no GPU ownership.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    Block, Chunk, ChunkCoord, MIN_WORLD_Y, WORLD_HEIGHT, WaterSample, WorldEdits,
    split_block_coordinate,
};

// A renderer may borrow another world at the same anchor; local counters alone
// would allow its cutaway upload to silently reuse a different world's voxels.
static NEXT_CHUNK_REVISION: AtomicU64 = AtomicU64::new(1);

/// Read-only voxel/fluid access; consumers choose whether missing chunks permit fallback.
pub trait WorldView {
    /// Returns known voxels, or `None` for missing horizontal residency.
    fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block>;

    /// Returns actual wet occupancy and its mean fluid profile.
    fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample>;
}

/// Dense CPU chunks and an inexpensive revision for dependent presentation caches.
#[derive(Debug, Default)]
pub struct ResidentWorld {
    chunks: HashMap<ChunkCoord, ResidentChunk>,
    edits: WorldEdits,
}

#[derive(Debug)]
struct ResidentChunk {
    chunk: Chunk,
    revision: u64,
}

impl ResidentWorld {
    /// Starts with restored sparse changes and no dense residency. Generation and
    /// procedural fallback both see the overlay from the first loaded frame.
    pub fn with_edits(edits: WorldEdits) -> Self {
        Self {
            chunks: HashMap::new(),
            edits,
        }
    }

    /// Inserts or replaces a completed chunk without duplicating its voxels.
    pub fn insert(&mut self, mut chunk: Chunk) {
        self.edits.apply_to_chunk(&mut chunk);
        let coord = chunk.coord();
        self.chunks.insert(
            coord,
            ResidentChunk {
                chunk,
                revision: NEXT_CHUNK_REVISION.fetch_add(1, Ordering::Relaxed),
            },
        );
    }

    /// Evicts only the specified chunk; an absent chunk leaves the revision unchanged.
    pub fn remove(&mut self, coord: ChunkCoord) {
        self.chunks.remove(&coord);
    }

    /// Resets the entire scene, including edits retained outside residency.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.edits.clear();
    }

    /// Sparse persistent overrides, including edits to currently evicted chunks.
    pub fn edits(&self) -> &WorldEdits {
        &self.edits
    }

    /// Changes a loaded, in-bounds voxel only when its material differs.
    /// Gameplay owns survival material/placement eligibility; this is CPU storage.
    pub fn edit_block(&mut self, position: [i32; 3], block: Block) -> bool {
        let [x, y, z] = position;
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        let Some(resident) = self.chunks.get_mut(&ChunkCoord::new(chunk_x, chunk_z)) else {
            return false;
        };
        if !resident.chunk.set_block([local_x, y, local_z], block) {
            return false;
        }
        self.edits.record(position, block);
        resident.revision = NEXT_CHUNK_REVISION.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Number of resident chunks.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether residency contains no chunks.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub(crate) fn chunk_revision(&self, coord: ChunkCoord) -> Option<u64> {
        self.chunks.get(&coord).map(|resident| resident.revision)
    }

    pub(crate) fn chunk(&self, coord: ChunkCoord) -> Option<&Chunk> {
        self.chunks.get(&coord).map(|resident| &resident.chunk)
    }

    /// Constant-work nearby-water readout; missing residency is never procedural water.
    pub(crate) fn surface_water_at(&self, x: i32, z: i32) -> Option<WaterSample> {
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        self.chunk(ChunkCoord::new(chunk_x, chunk_z))?
            .surface_water(local_x, local_z)
    }
}

impl WorldView for ResidentWorld {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return Some(Block::Air);
        }
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        self.chunk(ChunkCoord::new(chunk_x, chunk_z))
            .map(|chunk| chunk.block(local_x, y, local_z))
    }

    fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return None;
        }
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        self.chunk(ChunkCoord::new(chunk_x, chunk_z))
            .and_then(|chunk| chunk.water(local_x, y, local_z))
    }
}

impl WorldView for super::Terrain {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        Some(super::Terrain::block_at(self, x, y, z))
    }

    fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        super::Terrain::water_at(self, x, y, z)
    }
}

/// Resident-first world view with explicit procedural fallback for simulation.
pub struct SampledWorld<'a> {
    resident: &'a ResidentWorld,
    terrain: &'a super::Terrain,
}

impl<'a> SampledWorld<'a> {
    /// Compose explicitly permitted terrain fallback with an existing resident cache.
    pub fn new(resident: &'a ResidentWorld, terrain: &'a super::Terrain) -> Self {
        Self { resident, terrain }
    }
}

impl WorldView for SampledWorld<'_> {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        self.resident
            .block_at(x, y, z)
            .or_else(|| self.resident.edits.block_at(x, y, z))
            .or_else(|| WorldView::block_at(self.terrain, x, y, z))
    }

    fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        match self.resident.block_at(x, y, z) {
            Some(Block::Water) => self.resident.water_at(x, y, z),
            Some(_) => None,
            None => self.resident.edits.water_at(self.terrain, x, y, z),
        }
    }
}

#[cfg(test)]
mod tests;
