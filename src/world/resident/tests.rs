use super::*;
use crate::world::{Terrain, WaterBiome};

mod edits;

#[test]
fn residency_preserves_unknown_air_and_fluid_until_chunk_eviction() {
    let terrain = Terrain::default();
    let site = terrain.water_landmark(WaterBiome::River).unwrap();
    let [x, y, z] = site.water;
    let coord = ChunkCoord::from_world(x as f32, z as f32);
    let mut world = ResidentWorld::default();
    assert_eq!(world.block_at(x, y, z), None);
    assert_eq!(world.water_at(x, y, z), None);
    assert_eq!(world.block_at(x, MIN_WORLD_Y - 1, z), Some(Block::Air));
    world.insert(Chunk::generate(&terrain, coord));
    assert_eq!(world.len(), 1);
    assert_eq!(world.water_at(x, y, z), terrain.water_at(x, y, z));
    let revision = world.chunk_revision(coord);
    world.remove(ChunkCoord::new(100, 100));
    assert_eq!(world.chunk_revision(coord), revision);
    world.insert(Chunk::generate(&terrain, coord));
    assert_ne!(world.chunk_revision(coord), revision);
    world.remove(coord);
    assert_eq!(world.water_at(x, y, z), None);
    assert!(world.is_empty());
    world.clear();
    assert!(world.is_empty());
}

#[test]
fn sampled_world_only_falls_back_for_missing_residency_and_keeps_negative_seams() {
    let terrain = Terrain::default();
    let other = Terrain::new(42);
    let mut resident = ResidentWorld::default();
    resident.insert(Chunk::generate(&other, ChunkCoord::new(-1, -1)));
    let sampled = SampledWorld::new(&resident, &terrain);
    for [x, y, z] in [[-1, -64, -1], [-16, 10, -16], [-2, 20, -3]] {
        assert_eq!(sampled.block_at(x, y, z), Some(other.block_at(x, y, z)));
        assert_eq!(sampled.water_at(x, y, z), other.water_at(x, y, z));
    }
    for y in [MIN_WORLD_Y - 1, WORLD_HEIGHT, i32::MAX] {
        assert_eq!(sampled.block_at(-1, y, -1), Some(Block::Air));
        assert_eq!(sampled.water_at(-1, y, -1), None);
    }
    let site = terrain.water_landmark(WaterBiome::Ocean).unwrap().water;
    assert_eq!(
        sampled.block_at(site[0], site[1], site[2]),
        Some(Block::Water)
    );
    assert_eq!(
        sampled.water_at(site[0], site[1], site[2]),
        terrain.water_at(site[0], site[1], site[2])
    );
}
