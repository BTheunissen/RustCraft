use super::*;
use crate::game::test_support::empty_chunk;
use crate::world::{ChunkCoord, ResidentWorld, Terrain};

#[test]
fn recovery_preserves_a_valid_original_spawn_without_residency() {
    let terrain = Terrain::new(42);
    let world = ResidentWorld::default();
    let original = Vec3::from_array(terrain.spawn_position());
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(spawn_position(&sampled, original), Some(original));
}

#[test]
fn recovery_avoids_edited_body_liquids_and_missing_support() {
    let terrain = Terrain::new(42);
    let mut world = ResidentWorld::default();
    world.insert(empty_chunk(ChunkCoord::new(0, 0)));
    let original = Vec3::new(8.5, 1.0, 8.5);
    world.edit_block([8, 0, 8], Block::Stone);
    world.edit_block([8, 1, 8], Block::Lava);
    world.edit_block([7, 0, 7], Block::Stone);
    let sampled = SampledWorld::new(&world, &terrain);
    let recovered = spawn_position(&sampled, original).unwrap();
    assert_eq!(recovered, Vec3::new(7.5, 1.0, 7.5));
    assert!(clear_supported(&sampled, recovered));
    assert!(!clear_supported(&sampled, original));
    world.edit_block([8, 1, 8], Block::Air);
    world.edit_block([8, 0, 8], Block::Air);
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(spawn_position(&sampled, original), Some(recovered));
}

#[test]
fn recovery_can_stand_above_new_support_and_never_overlaps_a_low_roof() {
    let terrain = Terrain::new(42);
    let mut world = ResidentWorld::default();
    world.insert(empty_chunk(ChunkCoord::new(0, 0)));
    world.edit_block([8, 1, 8], Block::Stone);
    let original = Vec3::new(8.5, 1.0, 8.5);
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(
        spawn_position(&sampled, original),
        Some(Vec3::new(8.5, 2.0, 8.5))
    );
    assert!(!clear_supported(&sampled, Vec3::new(8.5, 0.0, 8.5)));
    assert!(!clear_supported(
        &sampled,
        Vec3::new(8.5, WORLD_HEIGHT as f32 - 1.0, 8.5)
    ));
    assert!(!clear_supported(&sampled, Vec3::splat(f32::NAN)));
}
