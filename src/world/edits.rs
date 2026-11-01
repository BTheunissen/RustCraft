//! Sparse session overlays, independent of procedural generation and residency.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::chunk::{block_index, cave_water_sample};
use super::{
    Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain, WORLD_DEPTH, WORLD_HEIGHT,
    WaterSample, split_block_coordinate,
};

static NEXT_EDIT_REVISION: AtomicU64 = AtomicU64::new(1);

/// One persistent override, expressed in integer world coordinates, including Y.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockEdit {
    pub position: [i32; 3],
    pub block: Block,
}

/// Sparse world-coordinate overrides independent of residency and disk storage.
///
/// Clones are immutable worker snapshots: unchanged chunks share their sparse
/// data, and the next edit copies only the modified chunk's overrides.
#[derive(Clone, Debug, Default)]
pub struct WorldEdits {
    chunks: HashMap<ChunkCoord, Arc<ChunkEdits>>,
    revision: u64,
}

#[derive(Clone, Debug, Default)]
struct ChunkEdits {
    blocks: BTreeMap<usize, Block>,
    revision: u64,
}

impl WorldEdits {
    /// Restores an overlay before any chunks are generated. Invalid Y or duplicate
    /// coordinates are rejected instead of silently losing saved changes.
    pub fn from_entries(
        entries: impl IntoIterator<Item = BlockEdit>,
    ) -> Result<Self, &'static str> {
        let mut edits = Self::default();
        for entry in entries {
            let [x, y, z] = entry.position;
            if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
                return Err("block edit is outside the world's vertical bounds");
            }
            if edits.block_at(x, y, z).is_some() {
                return Err("duplicate block edit coordinate");
            }
            edits.record(entry.position, entry.block);
        }
        Ok(edits)
    }

    /// All overrides, including evicted chunks, in stable world-coordinate order.
    /// Residency and revision counters are deliberately not part of the save data.
    pub fn entries(&self) -> Vec<BlockEdit> {
        let mut entries = Vec::new();
        for (coord, chunk) in &self.chunks {
            for (&index, &block) in &chunk.blocks {
                let column = index / WORLD_DEPTH as usize;
                entries.push(BlockEdit {
                    position: [
                        coord.x * CHUNK_SIZE + (column % CHUNK_SIZE as usize) as i32,
                        MIN_WORLD_Y + (index % WORLD_DEPTH as usize) as i32,
                        coord.z * CHUNK_SIZE + (column / CHUNK_SIZE as usize) as i32,
                    ],
                    block,
                });
            }
        }
        entries.sort_unstable_by_key(|entry| entry.position);
        entries
    }

    /// Changes only when an override changes or a nonempty overlay is cleared.
    /// Revisions also distinguish snapshots from different scene instances.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Reads an override, not procedural terrain. Y is a world coordinate.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return None;
        }
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        self.chunks
            .get(&ChunkCoord::new(chunk_x, chunk_z))?
            .blocks
            .get(&block_index(local_x, y, local_z))
            .copied()
    }

    /// Defensively reapplies this chunk's overrides without changing fluid profiles.
    /// Water occupancy still comes from the resulting voxels, never metadata alone.
    pub fn apply_to_chunk(&self, chunk: &mut Chunk) {
        if let Some(edits) = self.chunks.get(&chunk.coord()) {
            for (&index, &block) in &edits.blocks {
                chunk.blocks[index] = block;
            }
        }
    }

    pub(crate) fn has_chunk_edits(&self, coord: ChunkCoord) -> bool {
        self.chunks.contains_key(&coord)
    }

    /// Worker-only source discovery in one chunk's fixed 3x3/eight-block light halo.
    /// Never enumerates every world edit or grants generated blocks emission.
    pub(crate) fn lantern_sources(&self, origin: [i32; 2]) -> Vec<[i32; 3]> {
        let mut sources = Vec::new();
        let center = ChunkCoord::new(
            origin[0].div_euclid(CHUNK_SIZE),
            origin[1].div_euclid(CHUNK_SIZE),
        );
        for coord in halo(center) {
            let Some(edits) = self.chunks.get(&coord) else {
                continue;
            };
            for (&index, &block) in &edits.blocks {
                if block != Block::Lantern {
                    continue;
                }
                let column = index / WORLD_DEPTH as usize;
                let x = i64::from(coord.x) * i64::from(CHUNK_SIZE)
                    + (column % CHUNK_SIZE as usize) as i64;
                let z = i64::from(coord.z) * i64::from(CHUNK_SIZE)
                    + (column / CHUNK_SIZE as usize) as i64;
                if !(i64::from(origin[0]) - 8..i64::from(origin[0]) + i64::from(CHUNK_SIZE) + 8)
                    .contains(&x)
                    || !(i64::from(origin[1]) - 8..i64::from(origin[1]) + i64::from(CHUNK_SIZE) + 8)
                        .contains(&z)
                {
                    continue;
                }
                let (Ok(x), Ok(z)) = (i32::try_from(x), i32::try_from(z)) else {
                    continue;
                };
                sources.push([x, MIN_WORLD_Y + (index % WORLD_DEPTH as usize) as i32, z]);
            }
        }
        sources.sort_unstable();
        sources
    }

    pub(crate) fn apply_to_column(
        &self,
        [x, z]: [i32; 2],
        blocks: &mut [Block; WORLD_DEPTH as usize],
    ) {
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        let Some(edits) = self.chunks.get(&ChunkCoord::new(chunk_x, chunk_z)) else {
            return;
        };
        let first = block_index(local_x, MIN_WORLD_Y, local_z);
        for (&index, &block) in edits.blocks.range(first..first + WORLD_DEPTH as usize) {
            blocks[index - first] = block;
        }
    }

    /// A conservative dependency revision includes diagonal water-corner columns.
    pub(crate) fn halo_revision(&self, coord: ChunkCoord) -> u64 {
        halo(coord)
            .filter_map(|coord| self.chunks.get(&coord).map(|edits| edits.revision))
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn affected_chunks(&self, previous: &Self) -> BTreeSet<ChunkCoord> {
        if self.revision == previous.revision {
            return BTreeSet::new();
        }
        self.chunks
            .keys()
            .chain(previous.chunks.keys())
            .filter(|coord| {
                self.chunks.get(coord).map(|edits| edits.revision)
                    != previous.chunks.get(coord).map(|edits| edits.revision)
            })
            .flat_map(|&coord| halo(coord))
            .collect()
    }

    pub(super) fn record(&mut self, [x, y, z]: [i32; 3], block: Block) -> bool {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) || self.block_at(x, y, z) == Some(block) {
            return false;
        }
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        self.revision = NEXT_EDIT_REVISION.fetch_add(1, Ordering::Relaxed);
        let edits = Arc::make_mut(
            self.chunks
                .entry(ChunkCoord::new(chunk_x, chunk_z))
                .or_default(),
        );
        edits.blocks.insert(block_index(local_x, y, local_z), block);
        edits.revision = self.revision;
        true
    }

    pub(super) fn clear(&mut self) {
        if !self.chunks.is_empty() {
            self.chunks.clear();
            self.revision = NEXT_EDIT_REVISION.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn water_at(
        &self,
        terrain: &Terrain,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<WaterSample> {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return None;
        }
        if let Some(block) = self.block_at(x, y, z)
            && block != Block::Water
        {
            return None;
        }
        let (chunk_x, local_x) = split_block_coordinate(x);
        let (chunk_z, local_z) = split_block_coordinate(z);
        let first = block_index(local_x, MIN_WORLD_Y, local_z);
        let edited_column = self
            .chunks
            .get(&ChunkCoord::new(chunk_x, chunk_z))
            .is_some_and(|edits| {
                edits
                    .blocks
                    .range(first..first + WORLD_DEPTH as usize)
                    .next()
                    .is_some()
            });
        if !edited_column {
            return terrain.water_at(x, y, z);
        }
        let mut blocks = terrain.column_blocks(x, z);
        self.apply_to_column([x, z], &mut blocks);
        if blocks[(y - MIN_WORLD_Y) as usize] != Block::Water {
            return None;
        }
        terrain
            .water_column(x, z)
            .filter(|water| water.contains_y(y))
            .or_else(|| cave_water_sample(&blocks, y))
    }
}

fn halo(coord: ChunkCoord) -> impl Iterator<Item = ChunkCoord> {
    (-1..=1).flat_map(move |z| {
        (-1..=1).filter_map(move |x| {
            Some(ChunkCoord::new(
                coord.x.checked_add(x)?,
                coord.z.checked_add(z)?,
            ))
        })
    })
}

#[cfg(test)]
mod tests;
