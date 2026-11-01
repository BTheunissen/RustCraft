//! Presentation-only cutaway cache borrowing CPU residency from the game.

use glam::Vec3;

use super::{PHASE_SIZE, PhaseVolume, UNKNOWN_VOXEL, volume_origin, world_cell};
use crate::world::{
    Block, CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, ResidentWorld, WORLD_HEIGHT, WorldView,
    split_block_coordinate,
};

#[derive(Debug, Default)]
pub(crate) struct PhaseCache {
    volume: PhaseVolume,
    initialized: bool,
    revisions: [Option<u64>; 16],
}

impl PhaseCache {
    pub(crate) fn phasing(world: &ResidentWorld, eye: Vec3, flying: bool) -> bool {
        if !flying {
            return false;
        }
        let Some(cell) = world_cell(eye) else {
            return false;
        };
        volume_origin(cell).is_some()
            && world
                .block_at(cell[0], cell[1], cell[2])
                .is_some_and(Block::is_solid)
    }

    pub(crate) fn update(&mut self, world: &ResidentWorld, eye: Vec3) -> bool {
        let Some(origin) = world_cell(eye).and_then(volume_origin) else {
            return false;
        };
        let revisions = overlapping_revisions(world, origin);
        if self.initialized && self.volume.origin == origin && self.revisions == revisions {
            return false;
        }
        for z in 0..PHASE_SIZE {
            let (chunk_z, local_z) = split_block_coordinate(origin[2] + z);
            for x in 0..PHASE_SIZE {
                let (chunk_x, local_x) = split_block_coordinate(origin[0] + x);
                let chunk = world.chunk(ChunkCoord::new(chunk_x, chunk_z));
                for y in 0..PHASE_SIZE {
                    let world_y = origin[1] + y;
                    let voxel = if (MIN_WORLD_Y..WORLD_HEIGHT).contains(&world_y) {
                        chunk.map_or(UNKNOWN_VOXEL, |chunk| {
                            chunk.block(local_x, world_y, local_z) as u8
                        })
                    } else {
                        Block::Air as u8
                    };
                    let index = ((z * PHASE_SIZE + y) * PHASE_SIZE + x) as usize;
                    self.volume.voxels[index] = voxel;
                }
            }
        }
        self.volume.origin = origin;
        self.volume.generation = self.volume.generation.wrapping_add(1);
        self.initialized = true;
        self.revisions = revisions;
        true
    }

    pub(crate) fn volume(&self) -> &PhaseVolume {
        &self.volume
    }
}

fn overlapping_revisions(world: &ResidentWorld, origin: [i32; 3]) -> [Option<u64>; 16] {
    let mut revisions = [None; 16];
    if origin[1] >= WORLD_HEIGHT
        || i64::from(origin[1]) + i64::from(PHASE_SIZE) <= i64::from(MIN_WORLD_Y)
    {
        return revisions;
    }
    let min_x = origin[0].div_euclid(CHUNK_SIZE);
    let max_x = (origin[0] + PHASE_SIZE - 1).div_euclid(CHUNK_SIZE);
    let min_z = origin[2].div_euclid(CHUNK_SIZE);
    let max_z = (origin[2] + PHASE_SIZE - 1).div_euclid(CHUNK_SIZE);
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            revisions[((z - min_z) * 4 + x - min_x) as usize] =
                world.chunk_revision(ChunkCoord::new(x, z));
        }
    }
    revisions
}

#[cfg(test)]
mod tests;
