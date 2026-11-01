//! Resident voxel data and a CPU reference for the geological flight cutaway.
//!
//! This cache never generates terrain or changes the world. Missing resident
//! chunks remain unknown rather than becoming invented air or cave openings.

mod cache;
pub(crate) use cache::PhaseCache;

use glam::Vec3;

use crate::world::{Block, Chunk, ChunkCoord, WaterSample};

/// Width, height, and depth of the one-block-per-texel phase volume.
pub const PHASE_SIZE: i32 = 48;
/// Maximum cutaway ray distance, in world blocks.
pub const PHASE_RANGE: f32 = 20.0;
/// An unloaded voxel, distinct from every material ID, including air.
pub const UNKNOWN_VOXEL: u8 = 255;
/// Maximum number of translucent grid-boundary faces along a phase ray.
pub const PHASE_LAYERS: usize = 4;
/// Base opacity of an interior cutaway face, before texture and grid tinting.
pub const PHASE_OPACITY: f32 = 0.14;

const ANCHOR_SIZE: i64 = 4;
const VOXEL_COUNT: usize = (PHASE_SIZE * PHASE_SIZE * PHASE_SIZE) as usize;
const TIE_EPSILON: f64 = 1.0e-5;
const SELF_FACE_DISTANCE: f64 = 0.02;

/// A bounded `R8Uint` upload, with X fastest, then Y rows, then Z slices.
#[derive(Debug)]
pub struct PhaseVolume {
    origin: [i32; 3],
    voxels: Vec<u8>,
    generation: u64,
}

impl Default for PhaseVolume {
    fn default() -> Self {
        Self {
            origin: [0; 3],
            voxels: vec![UNKNOWN_VOXEL; VOXEL_COUNT],
            generation: 0,
        }
    }
}

impl PhaseVolume {
    /// World coordinate of texture texel `(0, 0, 0)`.
    pub const fn origin(&self) -> [i32; 3] {
        self.origin
    }

    /// Unpadded texture bytes: `(z * PHASE_SIZE + y) * PHASE_SIZE + x`.
    /// GPU writes use `PHASE_SIZE` bytes per row and rows per image.
    pub fn bytes(&self) -> &[u8] {
        &self.voxels
    }

    /// Upload version, incremented only when the volume is rebuilt.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Reads a world block's raw material ID; out-of-volume reads are unknown.
    pub fn voxel(&self, position: [i32; 3]) -> u8 {
        let local = std::array::from_fn::<_, 3, _>(|axis| {
            i64::from(position[axis]) - i64::from(self.origin[axis])
        });
        if local
            .iter()
            .any(|&coordinate| !(0..i64::from(PHASE_SIZE)).contains(&coordinate))
        {
            return UNKNOWN_VOXEL;
        }
        let index =
            (local[2] * i64::from(PHASE_SIZE) + local[1]) * i64::from(PHASE_SIZE) + local[0];
        self.voxels[index as usize]
    }
}

/// Convenience CPU resident world with a geological cutaway cache.
/// Gameplay may instead own `ResidentWorld` and let presentation borrow it.
#[derive(Debug, Default)]
pub struct PhaseWorld {
    world: crate::world::ResidentWorld,
    cache: PhaseCache,
}

impl PhaseWorld {
    /// Inserts or replaces a generated resident chunk.
    pub fn insert(&mut self, chunk: Chunk) {
        self.world.insert(chunk);
    }

    /// Evicts CPU residency under the caller's streaming policy.
    pub fn remove(&mut self, coord: ChunkCoord) {
        self.world.remove(coord);
    }

    /// Releases all resident chunks.
    pub fn clear(&mut self) {
        self.world.clear();
    }

    /// Reads resident blocks without procedural fallback.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        crate::world::WorldView::block_at(&self.world, x, y, z)
    }

    /// Reads actual resident fluid occupancy and metadata.
    pub fn water_at(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        crate::world::WorldView::water_at(&self.world, x, y, z)
    }

    /// Activates the cutaway only for a flying eye inside known solid terrain.
    pub fn phasing(&self, eye: Vec3, flying: bool) -> bool {
        PhaseCache::phasing(&self.world, eye, flying)
    }

    /// Rebuilds upload bytes only when its anchor or overlapping residency changes.
    pub fn update_volume(&mut self, eye: Vec3) -> bool {
        self.cache.update(&self.world, eye)
    }

    /// Latest reusable bounded cutaway upload.
    pub fn volume(&self) -> &PhaseVolume {
        self.cache.volume()
    }
}

/// A translucent face at a crossed solid-block grid boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseLayer {
    /// Physical ray distance, independent of the input direction's length.
    pub distance: f32,
    /// Material of the block being left, including solid-to-solid boundaries.
    pub block: Block,
    /// World-space boundary point for atlas projection and block-grid tinting.
    pub position: Vec3,
    /// Axis-aligned normal facing the camera; ties prefer X, then Y, then Z.
    pub normal: Vec3,
}

/// CPU reference for the shader's bounded voxel DDA, without world generation.
///
/// The initial voxel must be known and solid. Each crossing emits its leaving
/// material before sampling the entered voxel; air, liquids, and unknown end
/// the ray, so the last known solid's exit face remains visible. Tied axes all
/// advance together, never visiting zero-length edge/corner neighbors. Faces
/// within 0.02 blocks of the eye are skipped to match shader self-plane removal.
/// Directions are normalized in `f64`; zero axes have infinite crossing times.
pub fn phase_layers(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut sample: impl FnMut(i32, i32, i32) -> Option<Block>,
) -> Vec<PhaseLayer> {
    let mut layers = Vec::new();
    if !direction.is_finite() || !max_distance.is_finite() || max_distance <= 0.0 {
        return layers;
    }
    let Some(mut cell) = world_cell(origin) else {
        return layers;
    };
    let mut ray = direction.to_array().map(f64::from);
    let length = ray.iter().map(|value| value * value).sum::<f64>().sqrt();
    if length == 0.0 {
        return layers;
    }
    for component in &mut ray {
        *component /= length;
    }
    let Some(mut block) = sample(cell[0], cell[1], cell[2]) else {
        return layers;
    };
    if !block.is_solid() {
        return layers;
    }

    let eye = origin.to_array().map(f64::from);
    let limit = f64::from(max_distance.min(PHASE_RANGE));
    let mut step = [0; 3];
    let mut next = [f64::INFINITY; 3];
    let mut delta = [f64::INFINITY; 3];
    for axis in 0..3 {
        if ray[axis] > 0.0 {
            step[axis] = 1;
            next[axis] = (f64::from(cell[axis]) + 1.0 - eye[axis]) / ray[axis];
        } else if ray[axis] < 0.0 {
            step[axis] = -1;
            next[axis] = (f64::from(cell[axis]) - eye[axis]) / ray[axis];
        }
        if step[axis] != 0 {
            delta[axis] = ray[axis].abs().recip();
        }
    }

    while layers.len() < PHASE_LAYERS {
        let distance = next.into_iter().fold(f64::INFINITY, f64::min);
        if !distance.is_finite() || distance > limit {
            break;
        }
        let tied = next.map(|crossing| crossing <= distance + TIE_EPSILON);
        let Some(normal_axis) = tied.iter().position(|&crossing| crossing) else {
            break;
        };
        let mut entered = cell;
        for axis in 0..3 {
            if tied[axis] {
                let Some(coordinate) = cell[axis].checked_add(step[axis]) else {
                    return layers;
                };
                entered[axis] = coordinate;
                next[axis] += delta[axis];
            }
        }

        if distance > SELF_FACE_DISTANCE {
            let mut normal = [0.0; 3];
            normal[normal_axis] = -step[normal_axis] as f32;
            layers.push(PhaseLayer {
                distance: distance as f32,
                block,
                position: Vec3::from_array(std::array::from_fn(|axis| {
                    (eye[axis] + ray[axis] * distance) as f32
                })),
                normal: Vec3::from_array(normal),
            });
        }

        let Some(entered_block) = sample(entered[0], entered[1], entered[2]) else {
            break;
        };
        if !entered_block.is_solid() {
            break;
        }
        cell = entered;
        block = entered_block;
    }
    layers
}

fn world_cell(position: Vec3) -> Option<[i32; 3]> {
    fn coordinate(value: f32) -> Option<i32> {
        let value = f64::from(value).floor();
        if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
            return None;
        }
        Some(value as i32)
    }
    Some([
        coordinate(position.x)?,
        coordinate(position.y)?,
        coordinate(position.z)?,
    ])
}

fn volume_origin(cell: [i32; 3]) -> Option<[i32; 3]> {
    let mut origin = [0; 3];
    for axis in 0..3 {
        let anchor = i64::from(cell[axis]).div_euclid(ANCHOR_SIZE) * ANCHOR_SIZE;
        let start = anchor - i64::from(PHASE_SIZE / 2);
        i32::try_from(start + i64::from(PHASE_SIZE - 1)).ok()?;
        origin[axis] = i32::try_from(start).ok()?;
    }
    Some(origin)
}

#[cfg(test)]
mod tests;
