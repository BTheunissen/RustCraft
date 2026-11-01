use super::*;
use crate::world::{Chunk, Terrain};

#[test]
fn negative_coordinates_and_extreme_cameras_keep_volume_bounds_valid() {
    assert_eq!(
        volume_origin(Vec3::new(-0.1, -53.0, -16.1)),
        Some([-32, -64, -48])
    );
    assert_eq!(
        volume_origin(Vec3::new(0.0, 500.0, 0.0)),
        Some([-32, 16, -32])
    );
    assert!(volume_origin(Vec3::NAN).is_none());
    assert!(volume_origin(Vec3::splat(f32::MAX)).is_none());
    assert!(volume_origin(Vec3::splat(i32::MAX as f32)).is_none());
}

#[test]
fn skylight_accounts_for_roofs_outside_the_local_vertical_volume() {
    let mut volume = LightVolume::default();
    volume.rebuild([-32, -64, -32], |x, z| {
        std::array::from_fn(|index| {
            let y = MIN_WORLD_Y + index as i32;
            Some(if x == -1 && z == -1 && y == 25 {
                Block::Stone
            } else {
                Block::Air
            })
        })
    });
    assert_eq!(volume.sample([-1, -50, -1]), Some((Block::Air, 0.0)));
    assert_eq!(volume.sample([-2, -50, -1]), Some((Block::Air, 1.0)));
    assert_eq!(volume.sample([-33, -50, -1]), None);
    assert_eq!(volume.sample([-1, 0, -1]), None);
    assert_eq!(volume.sample([i32::MAX, 0, 0]), None);
}

#[test]
fn texture_rows_preserve_materials_and_attenuate_foliage_and_water() {
    let mut volume = LightVolume::default();
    volume.rebuild([0, 0, 0], |x, z| {
        std::array::from_fn(|index| {
            if x == 5 && z == 6 {
                return None;
            }
            Some(match MIN_WORLD_Y + index as i32 {
                50 => Block::Leaves,
                49 => Block::Water,
                _ => Block::Air,
            })
        })
    });
    let (block, sky) = volume.sample([3, 48, 7]).unwrap();
    assert_eq!(block, Block::Air);
    assert!((sky - 0.65 * 0.9).abs() <= 1.0 / 255.0);
    assert_eq!(volume.sample([3, 49, 7]).unwrap().0, Block::Water);
    assert_eq!(volume.sample([5, 48, 6]), None);
    let index = ((7 * 64 + 49) * 64 + 3) * 2;
    assert_eq!(volume.bytes()[index], Block::Water as u8);
}

#[test]
fn light_cache_refreshes_for_edits_eviction_and_new_worlds_but_not_unrelated_chunks() {
    let terrain = Terrain::new(42);
    let mut world = ResidentWorld::default();
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    let mut volume = LightVolume::default();
    let eye = Vec3::new(0.5, 20.0, 0.5);
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Rebuild);
    assert_eq!(
        volume.update(&world, eye + Vec3::splat(0.1)),
        VolumeUpdate::Unchanged
    );
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(8, 8)));
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Unchanged);
    assert!(world.edit_block([1, 60, 1], Block::Crystal));
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Rebuild);
    world.remove(ChunkCoord::new(0, 0));
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Rebuild);
    assert_eq!(volume.sample([1, 20, 1]), None);
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Rebuild);
    let previous_origin = volume.origin();
    assert_eq!(
        volume.update(&world, eye + Vec3::X * 8.0),
        VolumeUpdate::Scroll { previous_origin }
    );
    assert_eq!(volume.update(&world, Vec3::NAN), VolumeUpdate::Unchanged);
    let mut other = ResidentWorld::default();
    other.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    assert_eq!(volume.update(&other, eye), VolumeUpdate::Rebuild);
}

#[test]
fn scrolling_retains_light_only_when_the_old_transport_residency_is_unchanged() {
    let terrain = Terrain::new(42);
    let eye = Vec3::new(-0.5, -20.0, -0.5);
    let mut world = ResidentWorld::default();
    let coord = ChunkCoord::new(-2, -1);
    world.insert(Chunk::generate(&terrain, coord));
    let mut volume = LightVolume::default();
    assert_eq!(volume.update(&world, eye), VolumeUpdate::Rebuild);
    let previous_origin = volume.origin();
    assert_eq!(
        volume.update(&world, eye + Vec3::new(16.0, 16.0, -16.0)),
        VolumeUpdate::Scroll { previous_origin }
    );
    assert_eq!(volume.sample([-1, -20, -1]), None);
    let previous_origin = volume.origin();
    assert_eq!(
        volume.update(&world, eye),
        VolumeUpdate::Scroll { previous_origin }
    );
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(-1, -1)));
    assert_eq!(
        volume.update(&world, eye + Vec3::X * 8.0),
        VolumeUpdate::Rebuild
    );
    assert_eq!(
        volume.update(&world, eye + Vec3::X * 100.0),
        VolumeUpdate::Rebuild
    );
}

#[test]
fn evicting_a_chunk_that_leaves_the_window_cannot_preserve_its_emitter_light() {
    let terrain = Terrain::new(42);
    let eye = Vec3::new(-0.5, -20.0, -0.5);
    let mut world = ResidentWorld::default();
    let coord = ChunkCoord::new(-2, -1);
    world.insert(Chunk::generate(&terrain, coord));
    let mut volume = LightVolume::default();
    volume.update(&world, eye);
    world.remove(coord);
    assert_eq!(
        volume.update(&world, eye + Vec3::X * 16.0),
        VolumeUpdate::Rebuild
    );
}

#[test]
fn an_edit_above_the_local_y_window_invalidates_cached_roof_lighting_on_a_scroll() {
    let terrain = Terrain::new(42);
    let mut world = ResidentWorld::default();
    world.insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    let mut volume = LightVolume::default();
    let eye = Vec3::new(0.5, -50.0, 0.5);
    volume.update(&world, eye);
    assert!(world.edit_block([1, 60, 1], Block::Stone));
    assert_eq!(
        volume.update(&world, eye + Vec3::X * 8.0),
        VolumeUpdate::Rebuild
    );
}

#[test]
fn anchor_hysteresis_prevents_jitter_and_immediate_reverse_scrolls_at_voxel_grid_edges() {
    let initial = Vec3::new(-0.1, -20.0, -0.1);
    let origin = volume_origin(initial).unwrap();
    for movement in [-0.02, 0.02, 0.1, 0.2, 2.0] {
        assert_eq!(
            tracked_origin(initial + Vec3::X * movement, Some(origin)),
            Some(origin)
        );
    }
    let forward = initial + Vec3::X * 8.2;
    let shifted = tracked_origin(forward, Some(origin)).unwrap();
    assert_eq!(shifted[0], origin[0] + 8);
    for back in [0.02, 0.2, 1.0, 4.0] {
        assert_eq!(
            tracked_origin(forward - Vec3::X * back, Some(shifted)),
            Some(shifted)
        );
    }
    assert_eq!(
        tracked_origin(initial - Vec3::X * 0.1, Some(shifted)),
        Some(origin)
    );
    assert_eq!(tracked_origin(Vec3::NAN, Some(origin)), None);
    assert_eq!(
        tracked_origin(Vec3::new(0.0, 500.0, 0.0), Some(origin)).unwrap()[1],
        16
    );
}
