use super::*;
use crate::game::test_support::world;
use crate::world::{ChunkCoord, MIN_WORLD_Y, WORLD_HEIGHT};

#[test]
fn cardinal_ray_returns_entry_face_distance_and_first_solid() {
    let world = world(
        &[ChunkCoord::new(0, 0)],
        &[([2, 3, 4], Block::Stone), ([3, 3, 4], Block::Dirt)],
    );
    let target = raycast(&world, Vec3::new(0.5, 3.5, 4.5), Vec3::X * 20.0).unwrap();
    assert_eq!(target.position, [2, 3, 4]);
    assert_eq!(target.normal, [-1, 0, 0]);
    assert_eq!(target.block, Block::Stone);
    assert_eq!(target.distance, 1.5);
    let inside = raycast(&world, Vec3::new(2.5, 3.5, 4.5), Vec3::X).unwrap();
    assert_eq!(inside.distance, 0.0);
    assert_eq!(inside.normal, [0; 3]);
}

#[test]
fn reach_includes_the_boundary_but_not_the_next_voxel() {
    let world = world(&[ChunkCoord::new(0, 0)], &[([5, 2, 2], Block::Stone)]);
    assert_eq!(
        raycast(&world, Vec3::new(0.5, 2.5, 2.5), Vec3::X)
            .unwrap()
            .distance,
        4.5
    );
    assert!(raycast(&world, Vec3::new(0.49, 2.5, 2.5), Vec3::X).is_none());
}

#[test]
fn negative_coordinates_and_boundary_origins_use_the_forward_side() {
    let world = world(
        &[ChunkCoord::new(-1, -1)],
        &[([-3, -5, -1], Block::Dirt), ([-1, -5, -2], Block::Stone)],
    );
    let target = raycast(&world, Vec3::new(-0.5, -4.5, -0.5), -Vec3::X).unwrap();
    assert_eq!(target.position, [-3, -5, -1]);
    assert_eq!(target.normal, [1, 0, 0]);
    assert_eq!(target.distance, 1.5);
    let forward = raycast(&world, Vec3::new(0.0, -4.5, -1.5), -Vec3::X).unwrap();
    assert_eq!(forward.position, [-1, -5, -2]);
    assert_eq!(forward.distance, 0.0);
    assert_eq!(forward.normal, [1, 0, 0]);
}

#[test]
fn exact_edge_and_corner_ties_do_not_hit_zero_length_side_voxels() {
    let mut world = world(
        &[ChunkCoord::new(0, 0)],
        &[
            ([1, 0, 0], Block::Stone),
            ([0, 1, 0], Block::Stone),
            ([0, 0, 1], Block::Stone),
            ([1, 1, 0], Block::Wood),
            ([1, 1, 1], Block::Dirt),
        ],
    );
    let origin = Vec3::splat(0.5);
    let edge = raycast(&world, origin, Vec3::new(1.0, 1.0, 0.0)).unwrap();
    assert_eq!(edge.position, [1, 1, 0]);
    assert_eq!(edge.normal, [-1, 0, 0]);
    let corner = raycast(&world, origin, Vec3::ONE).unwrap();
    assert_eq!(corner.position, [1, 1, 1]);
    assert_eq!(corner.normal, [-1, 0, 0]);
    assert!((corner.distance - 3.0_f32.sqrt() * 0.5).abs() < 0.000_001);
    world.edit_block([1, 1, 1], Block::Air);
    assert!(raycast(&world, origin, Vec3::ONE).is_none());
}

#[test]
fn missing_residency_blocks_targeting_even_when_farther_voxels_are_loaded() {
    let world = world(&[ChunkCoord::new(1, 0)], &[([17, 2, 2], Block::Stone)]);
    assert!(raycast(&world, Vec3::new(15.5, 2.5, 2.5), Vec3::X).is_none());
    assert!(raycast(&world, Vec3::new(16.5, 2.5, 2.5), -Vec3::X).is_none());
}

#[test]
fn liquids_are_not_targets_but_bedrock_is_highlightable() {
    let world = world(
        &[ChunkCoord::new(0, 0)],
        &[
            ([1, 1, 1], Block::Water),
            ([2, 1, 1], Block::Lava),
            ([3, 1, 1], Block::Bedrock),
        ],
    );
    assert_eq!(
        raycast(&world, Vec3::new(0.5, 1.5, 1.5), Vec3::X)
            .unwrap()
            .block,
        Block::Bedrock
    );
}

#[test]
fn invalid_and_extreme_finite_inputs_cannot_saturate_into_phantom_targets() {
    let world = world(&[ChunkCoord::new(0, 0)], &[([1, 1, 1], Block::Stone)]);
    let origin = Vec3::new(0.5, 1.5, 1.5);
    for direction in [
        Vec3::ZERO,
        Vec3::splat(f32::NAN),
        Vec3::splat(f32::INFINITY),
    ] {
        assert!(raycast(&world, origin, direction).is_none());
    }
    for origin in [
        Vec3::splat(f32::NAN),
        Vec3::splat(f32::MAX),
        Vec3::splat(f32::INFINITY),
    ] {
        assert!(raycast(&world, origin, Vec3::X).is_none());
    }
    assert_eq!(
        raycast(&world, origin, Vec3::X * f32::MAX)
            .unwrap()
            .position,
        [1, 1, 1]
    );
    assert_eq!(
        raycast(&world, origin, Vec3::X * f32::MIN_POSITIVE)
            .unwrap()
            .position,
        [1, 1, 1]
    );
}

#[test]
fn vertical_bounds_can_be_entered_without_addressing_out_of_bounds_voxels() {
    let world = world(
        &[ChunkCoord::new(0, 0)],
        &[
            ([2, WORLD_HEIGHT - 1, 2], Block::Stone),
            ([2, MIN_WORLD_Y, 2], Block::Bedrock),
        ],
    );
    let above = Vec3::new(2.5, WORLD_HEIGHT as f32 + 0.5, 2.5);
    assert_eq!(
        raycast(&world, above, -Vec3::Y).unwrap().position[1],
        WORLD_HEIGHT - 1
    );
    let below = Vec3::new(2.5, MIN_WORLD_Y as f32 - 0.5, 2.5);
    assert_eq!(
        raycast(&world, below, Vec3::Y).unwrap().position[1],
        MIN_WORLD_Y
    );
}

#[test]
fn collection_visibility_rejects_walls_unknown_and_nonfinite_segments() {
    let world = world(&[ChunkCoord::new(0, 0)], &[([2, 1, 1], Block::Stone)]);
    let start = Vec3::new(1.5, 1.5, 1.5);
    assert!(segment_clear(&world, start, start));
    assert!(segment_clear(&world, start, Vec3::new(1.9, 1.5, 1.5)));
    assert!(!segment_clear(&world, start, Vec3::new(3.5, 1.5, 1.5)));
    assert!(!segment_clear(
        &world,
        Vec3::new(15.5, 1.5, 1.5),
        Vec3::new(16.5, 1.5, 1.5)
    ));
    assert!(!segment_clear(&world, start, Vec3::splat(f32::NAN)));
}

#[test]
fn collection_checks_touched_side_cells_without_changing_targeting_ties() {
    let mut world = world(
        &[ChunkCoord::new(0, 0)],
        &[
            ([9, 2, 8], Block::Stone),
            ([8, 2, 9], Block::Stone),
            ([10, 2, 10], Block::Wood),
        ],
    );
    let start = Vec3::new(8.5, 2.5, 8.5);
    let end = Vec3::new(9.5, 2.5, 9.5);
    assert!(!segment_clear(&world, start, end));
    assert_eq!(
        raycast(&world, start, end - start).unwrap().position,
        [10, 2, 10]
    );
    world.edit_block([9, 2, 8], Block::Air);
    world.edit_block([8, 2, 9], Block::Air);
    assert!(segment_clear(&world, start, end));
    world.edit_block([8, 3, 8], Block::Stone);
    assert!(!segment_clear(&world, start, start + Vec3::ONE));
}

#[test]
fn collection_cannot_skip_unknown_diagonal_seam_neighbors() {
    let world = world(&[ChunkCoord::new(-1, -1), ChunkCoord::new(0, 0)], &[]);
    assert!(!segment_clear(
        &world,
        Vec3::new(-0.5, 3.5, -0.5),
        Vec3::new(0.5, 3.5, 0.5)
    ));
}
