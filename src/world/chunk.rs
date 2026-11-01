//! Dense chunk storage; generation is implemented separately.

use super::{
    Block, CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, WORLD_DEPTH, WORLD_HEIGHT, WaterBiome, WaterColumn,
    WaterSample,
};
const COLUMN_COUNT: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// Dense, owned voxels for one bounded-height chunk column.
#[derive(Clone, Debug)]
pub struct Chunk {
    pub(super) coord: ChunkCoord,
    pub(super) blocks: Box<[Block]>,
    pub(super) water_columns: Box<[Option<WaterColumn>; COLUMN_COUNT]>,
    pub(super) coastal_roofs: Box<[Option<[i32; 2]>; COLUMN_COUNT]>,
}

impl Chunk {
    /// Returns this chunk's horizontal chunk coordinate.
    pub const fn coord(&self) -> ChunkCoord {
        self.coord
    }

    /// Generated overhang interval, separate from ground/water; edits remain voxel authority.
    pub fn coastal_roof(&self, x: i32, z: i32) -> Option<[i32; 2]> {
        if !(0..CHUNK_SIZE).contains(&x) || !(0..CHUNK_SIZE).contains(&z) {
            return None;
        }
        self.coastal_roofs[(z * CHUNK_SIZE + x) as usize]
    }

    /// Reads local X/Z and world Y, returning air for any out-of-bounds axis.
    pub fn block(&self, x: i32, y: i32, z: i32) -> Block {
        if !(0..CHUNK_SIZE).contains(&x)
            || !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y)
            || !(0..CHUNK_SIZE).contains(&z)
        {
            return Block::Air;
        }
        self.blocks[block_index(x, y, z)]
    }

    pub(super) fn set_block(&mut self, [x, y, z]: [i32; 3], block: Block) -> bool {
        if !(0..CHUNK_SIZE).contains(&x)
            || !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y)
            || !(0..CHUNK_SIZE).contains(&z)
        {
            return false;
        }
        let current = &mut self.blocks[block_index(x, y, z)];
        if *current == block {
            return false;
        }
        *current = block;
        true
    }

    /// Reads water metadata at local X/Z and world Y using resident voxels only.
    /// Cave pools infer their bounded still-water interval from the dense column.
    pub fn water(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        if self.block(x, y, z) != Block::Water {
            return None;
        }
        if let Some(water) =
            self.water_columns[(z * CHUNK_SIZE + x) as usize].filter(|water| water.contains_y(y))
        {
            return Some(water);
        }
        let first = block_index(x, MIN_WORLD_Y, z);
        cave_water_sample(&self.blocks[first..first + WORLD_DEPTH as usize], y)
    }

    /// Known surface fluid only when its actual top voxel remains wet after edits.
    pub(crate) fn surface_water(&self, x: i32, z: i32) -> Option<WaterSample> {
        if !(0..CHUNK_SIZE).contains(&x) || !(0..CHUNK_SIZE).contains(&z) {
            return None;
        }
        let water = self.water_columns[(z * CHUNK_SIZE + x) as usize]?;
        if !water.surface.is_finite()
            || !water.salinity.is_finite()
            || !water.flow.into_iter().all(f32::is_finite)
            || f64::from(water.surface) <= f64::from(water.bottom) + 1.0
        {
            return None;
        }
        let top = f64::from(water.surface).ceil() - 1.0;
        if !(f64::from(MIN_WORLD_Y)..f64::from(WORLD_HEIGHT)).contains(&top) {
            return None;
        }
        self.water(x, top as i32, z)
    }
}

pub(super) fn block_index(x: i32, y: i32, z: i32) -> usize {
    ((z * CHUNK_SIZE + x) * WORLD_DEPTH + y - MIN_WORLD_Y) as usize
}

pub(super) fn cave_water_sample(blocks: &[Block], y: i32) -> Option<WaterSample> {
    if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y)
        || blocks[(y - MIN_WORLD_Y) as usize] != Block::Water
    {
        return None;
    }
    let mut first = (y - MIN_WORLD_Y) as usize;
    let mut last = first;
    while first > 0 && blocks[first - 1] == Block::Water {
        first -= 1;
    }
    while last + 1 < blocks.len() && blocks[last + 1] == Block::Water {
        last += 1;
    }
    Some(WaterColumn {
        bottom: MIN_WORLD_Y + first as i32 - 1,
        surface: (MIN_WORLD_Y + last as i32 + 1) as f32,
        flow: [0.0; 3],
        biome: WaterBiome::CavePool,
        salinity: 0.0,
    })
}
