use super::{
    PHASE_LAYERS, PHASE_RANGE, PHASE_SIZE, PhaseVolume, PhaseWorld, UNKNOWN_VOXEL, phase_layers,
    volume_origin, world_cell,
};
use crate::world::{Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain, WORLD_HEIGHT};
use glam::Vec3;

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0e-5,
        "{actual} differs from {expected}"
    );
}

mod rays;
mod volume;
