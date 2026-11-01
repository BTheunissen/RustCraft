use super::*;
use crate::world::{Chunk, Terrain};

#[test]
fn phase_cache_borrows_residency_and_tracks_only_overlapping_changes() {
    let terrain = Terrain::default();
    let mut world = ResidentWorld::default();
    let mut cache = PhaseCache::default();
    let eye = Vec3::new(0.5, -62.5, 0.5);
    assert!(!PhaseCache::phasing(&world, eye, true));
    assert!(cache.update(&world, eye));
    assert_eq!(cache.volume().voxel([0, -63, 0]), UNKNOWN_VOXEL);
    let initial = cache.volume().generation();
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(8, 8)));
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(9, 9)));
    assert!(!cache.update(&world, eye));
    assert_eq!(cache.volume().generation(), initial);
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    assert!(cache.update(&world, eye));
    assert_eq!(
        cache.volume().voxel([0, -63, 0]),
        terrain.block_at(0, -63, 0) as u8
    );
    assert!(!PhaseCache::phasing(&world, eye, false));
    assert_eq!(
        PhaseCache::phasing(&world, eye, true),
        terrain.block_at(0, -63, 0).is_solid()
    );
    assert!(!cache.update(&world, eye + Vec3::X * 0.1));
    world.remove(ChunkCoord::new(0, 0));
    assert!(cache.update(&world, eye));
    assert_eq!(cache.volume().voxel([0, -63, 0]), UNKNOWN_VOXEL);
    assert!(!cache.update(&world, Vec3::splat(f32::NAN)));
    assert!(!PhaseCache::phasing(&world, Vec3::splat(f32::NAN), true));
}

#[test]
fn phase_cache_reanchors_and_respects_negative_coordinates_and_vertical_limits() {
    let mut world = ResidentWorld::default();
    let terrain = Terrain::default();
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(-1, -1)));
    let mut cache = PhaseCache::default();
    let eye = Vec3::new(-0.5, -50.5, -0.5);
    assert!(cache.update(&world, eye));
    assert_eq!(cache.volume().origin(), [-28, -76, -28]);
    assert_eq!(cache.volume().voxel([-1, -64, -1]), Block::Bedrock as u8);
    assert_eq!(cache.volume().voxel([-1, -65, -1]), Block::Air as u8);
    assert!(cache.update(&world, eye + Vec3::X * 4.0));
    world.clear();
    assert!(cache.update(&world, eye + Vec3::X * 4.0));
    assert!(cache.update(&world, Vec3::Y * 200.0));
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    assert!(!cache.update(&world, Vec3::Y * 200.0));
}

#[test]
fn different_world_instances_at_the_same_pose_cannot_share_a_cached_volume() {
    let mut first = ResidentWorld::default();
    let mut second = ResidentWorld::default();
    let coord = ChunkCoord::new(0, 0);
    first.insert(Chunk::generate(&Terrain::default(), coord));
    second.insert(Chunk::generate(&Terrain::new(42), coord));
    let mut cache = PhaseCache::default();
    let eye = Vec3::new(0.5, -32.5, 0.5);
    assert!(cache.update(&first, eye));
    let bytes = cache.volume().bytes().to_vec();
    assert!(cache.update(&second, eye));
    assert_ne!(cache.volume().bytes(), bytes);
    assert!(!cache.update(&second, eye));
}
