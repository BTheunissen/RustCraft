//! Revision-aware, resident-only material/skylight upload for bounded light transport.

use super::daylight_through;
use crate::world::{
    Block, CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, ResidentWorld, WORLD_HEIGHT, split_block_coordinate,
};
use glam::Vec3;

pub(crate) const LIGHT_VOLUME_SIZE: i32 = 64;
pub(crate) const PROBE_SPACING: i32 = 4;
pub(crate) const PROBE_GRID: u32 = (LIGHT_VOLUME_SIZE / PROBE_SPACING) as u32;
pub(crate) const PROBE_COUNT: u32 = PROBE_GRID * PROBE_GRID * PROBE_GRID;
pub(crate) const PROBES_PER_FRAME: u32 = 512;
const UNKNOWN: u8 = 255;
const VOXELS: usize = (LIGHT_VOLUME_SIZE * LIGHT_VOLUME_SIZE * LIGHT_VOLUME_SIZE) as usize;

pub(crate) struct LightVolume {
    origin: [i32; 3],
    bytes: Vec<u8>,
    revisions: [Option<u64>; 25],
    initialized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VolumeUpdate {
    Unchanged,
    Rebuild,
    Scroll { previous_origin: [i32; 3] },
}

impl Default for LightVolume {
    fn default() -> Self {
        Self {
            origin: [0; 3],
            bytes: [UNKNOWN, 0].repeat(VOXELS),
            revisions: [None; 25],
            initialized: false,
        }
    }
}

impl LightVolume {
    pub(crate) fn origin(&self) -> [i32; 3] {
        self.origin
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn update(&mut self, world: &ResidentWorld, eye: Vec3) -> VolumeUpdate {
        let Some(origin) = tracked_origin(eye, self.initialized.then_some(self.origin)) else {
            return VolumeUpdate::Unchanged;
        };
        let min_x = origin[0].div_euclid(CHUNK_SIZE);
        let min_z = origin[2].div_euclid(CHUNK_SIZE);
        let max_x = (origin[0] + LIGHT_VOLUME_SIZE - 1).div_euclid(CHUNK_SIZE);
        let max_z = (origin[2] + LIGHT_VOLUME_SIZE - 1).div_euclid(CHUNK_SIZE);
        let mut revisions = [None; 25];
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                revisions[((z - min_z) * 5 + x - min_x) as usize] =
                    world.chunk_revision(ChunkCoord::new(x, z));
            }
        }
        if self.initialized && origin == self.origin && revisions == self.revisions {
            return VolumeUpdate::Unchanged;
        }
        let update = if self.can_scroll(world, origin) {
            VolumeUpdate::Scroll {
                previous_origin: self.origin,
            }
        } else {
            VolumeUpdate::Rebuild
        };
        self.rebuild(origin, |x, z| {
            let (chunk_x, local_x) = split_block_coordinate(x);
            let (chunk_z, local_z) = split_block_coordinate(z);
            let chunk = world.chunk(ChunkCoord::new(chunk_x, chunk_z));
            std::array::from_fn(|index| {
                chunk.map(|chunk| chunk.block(local_x, MIN_WORLD_Y + index as i32, local_z))
            })
        });
        self.revisions = revisions;
        update
    }

    fn can_scroll(&self, world: &ResidentWorld, origin: [i32; 3]) -> bool {
        if !self.initialized
            || origin == self.origin
            || (0..3).any(|axis| {
                (i64::from(origin[axis]) - i64::from(self.origin[axis])).abs()
                    >= i64::from(LIGHT_VOLUME_SIZE)
            })
        {
            return false;
        }
        let [old_x, _, old_z] = self.origin;
        let min_x = old_x.div_euclid(CHUNK_SIZE);
        let min_z = old_z.div_euclid(CHUNK_SIZE);
        let max_x = (old_x + LIGHT_VOLUME_SIZE - 1).div_euclid(CHUNK_SIZE);
        let max_z = (old_z + LIGHT_VOLUME_SIZE - 1).div_euclid(CHUNK_SIZE);
        // Changes anywhere in the old transport field invalidate its cached light,
        // including a source in a chunk that the new window no longer covers.
        (min_z..=max_z).all(|z| {
            (min_x..=max_x).all(|x| {
                world.chunk_revision(ChunkCoord::new(x, z))
                    == self.revisions[((z - min_z) * 5 + x - min_x) as usize]
            })
        })
    }

    pub(crate) fn rebuild(
        &mut self,
        origin: [i32; 3],
        mut column: impl FnMut(i32, i32) -> [Option<Block>; (WORLD_HEIGHT - MIN_WORLD_Y) as usize],
    ) {
        self.origin = origin;
        self.initialized = true;
        for z in 0..LIGHT_VOLUME_SIZE {
            for x in 0..LIGHT_VOLUME_SIZE {
                let blocks = column(origin[0] + x, origin[2] + z);
                let mut sky = 1.0;
                for y in (MIN_WORLD_Y..WORLD_HEIGHT).rev() {
                    let block = blocks[(y - MIN_WORLD_Y) as usize];
                    sky = block.map_or(0.0, |block| daylight_through(sky, block));
                    let local_y = y - origin[1];
                    if !(0..LIGHT_VOLUME_SIZE).contains(&local_y) {
                        continue;
                    }
                    let index =
                        ((z * LIGHT_VOLUME_SIZE + local_y) * LIGHT_VOLUME_SIZE + x) as usize * 2;
                    self.bytes[index] = block.map_or(UNKNOWN, |block| block as u8);
                    self.bytes[index + 1] = (sky * 255.0).round() as u8;
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn sample(&self, cell: [i32; 3]) -> Option<(Block, f32)> {
        let local = std::array::from_fn::<_, 3, _>(|axis| {
            i64::from(cell[axis]) - i64::from(self.origin[axis])
        });
        if local
            .iter()
            .any(|&value| !(0..i64::from(LIGHT_VOLUME_SIZE)).contains(&value))
        {
            return None;
        }
        let index = ((local[2] * i64::from(LIGHT_VOLUME_SIZE) + local[1])
            * i64::from(LIGHT_VOLUME_SIZE)
            + local[0]) as usize
            * 2;
        Some((
            *Block::ALL.get(self.bytes[index] as usize)?,
            f32::from(self.bytes[index + 1]) / 255.0,
        ))
    }
}

pub(crate) fn volume_origin(eye: Vec3) -> Option<[i32; 3]> {
    if !eye.is_finite() {
        return None;
    }
    let mut origin = [0; 3];
    for (axis, coordinate) in eye.to_array().into_iter().enumerate() {
        let start = (f64::from(coordinate) / 8.0).round() * 8.0 - f64::from(LIGHT_VOLUME_SIZE / 2);
        if start < f64::from(i32::MIN) || start + f64::from(LIGHT_VOLUME_SIZE) > f64::from(i32::MAX)
        {
            return None;
        }
        origin[axis] = start as i32;
    }
    origin[1] = origin[1].clamp(MIN_WORLD_Y, WORLD_HEIGHT - LIGHT_VOLUME_SIZE);
    Some(origin)
}

fn tracked_origin(eye: Vec3, previous: Option<[i32; 3]>) -> Option<[i32; 3]> {
    let mut desired = volume_origin(eye)?;
    if let Some(previous) = previous {
        for axis in 0..3 {
            let local = f64::from(eye[axis]) - f64::from(previous[axis]);
            // The eight-block overlap band stops tiny reversals on a grid boundary
            // from repeatedly scrolling occupancy, sky classification, and probes.
            if (24.0..40.0).contains(&local) {
                desired[axis] = previous[axis];
            }
        }
    }
    desired[1] = desired[1].clamp(MIN_WORLD_Y, WORLD_HEIGHT - LIGHT_VOLUME_SIZE);
    Some(desired)
}

#[cfg(test)]
mod tests;
