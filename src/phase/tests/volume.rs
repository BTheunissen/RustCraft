use super::*;

#[test]
fn fluid_metadata_uses_resident_water_and_disappears_with_chunk_eviction() {
    let terrain = Terrain::default();
    let site = terrain
        .water_landmark(crate::world::WaterBiome::River)
        .unwrap();
    let [x, y, z] = site.water;
    let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
    let mut world = PhaseWorld::default();
    assert_eq!(world.water_at(x, y, z), None);
    let chunk = Chunk::generate(&terrain, coord);
    let expected = chunk
        .water(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
        .unwrap();
    world.insert(chunk);
    assert_eq!(world.water_at(x, y, z), Some(expected));
    assert_eq!(world.water_at(x, MIN_WORLD_Y, z), None);
    assert_eq!(world.water_at(x, WORLD_HEIGHT, z), None);
    world.remove(coord);
    assert_eq!(world.water_at(x, y, z), None);
    world.insert(Chunk::generate(&terrain, coord));
    assert_eq!(world.water_at(x, y, z), Some(expected));
    world.clear();
    assert_eq!(world.water_at(x, y, z), None);
}

#[test]
fn default_volume_and_out_of_volume_reads_are_unknown_not_air() {
    let volume = PhaseVolume::default();
    assert_eq!(volume.bytes().len(), 48 * 48 * 48);
    assert!(volume.bytes().iter().all(|&voxel| voxel == UNKNOWN_VOXEL));
    assert_eq!(volume.generation(), 0);
    for position in [
        [0; 3],
        [-1, 0, 0],
        [PHASE_SIZE; 3],
        [i32::MIN; 3],
        [i32::MAX; 3],
    ] {
        assert_eq!(volume.voxel(position), UNKNOWN_VOXEL);
    }
}

#[test]
fn missing_chunks_are_unknown_but_vertical_world_limits_are_known_air() {
    let world = PhaseWorld::default();
    for (x, z) in [(0, 0), (-1, -17), (i32::MIN, i32::MAX)] {
        assert_eq!(world.block_at(x, MIN_WORLD_Y, z), None);
        assert_eq!(world.block_at(x, WORLD_HEIGHT - 1, z), None);
        assert_eq!(world.block_at(x, MIN_WORLD_Y - 1, z), Some(Block::Air));
        assert_eq!(world.block_at(x, WORLD_HEIGHT, z), Some(Block::Air));
        assert_eq!(world.block_at(x, i32::MIN, z), Some(Block::Air));
        assert_eq!(world.block_at(x, i32::MAX, z), Some(Block::Air));
    }
}

#[test]
fn resident_reads_and_x_fast_volume_match_dense_data_across_negative_seams() {
    let terrain = Terrain::default();
    let mut world = PhaseWorld::default();
    for coord in [
        ChunkCoord::new(-1, -1),
        ChunkCoord::new(0, -1),
        ChunkCoord::new(-1, 0),
        ChunkCoord::new(0, 0),
    ] {
        world.insert(Chunk::generate(&terrain, coord));
    }
    for x in [-16, -1, 0, 15] {
        for z in [-16, -1, 0, 15] {
            for y in [MIN_WORLD_Y, -49, -29, 0, WORLD_HEIGHT - 1] {
                assert_eq!(world.block_at(x, y, z), Some(terrain.block_at(x, y, z)));
            }
        }
    }

    assert!(world.update_volume(Vec3::new(-0.5, -48.5, -0.5)));
    assert_eq!(world.volume().origin(), [-28, -76, -28]);
    assert_eq!(
        world.volume().voxel([-1, MIN_WORLD_Y, -1]),
        Block::Bedrock as u8
    );
    assert_eq!(world.volume().voxel([-17, MIN_WORLD_Y, -1]), UNKNOWN_VOXEL);
    assert_eq!(world.volume().voxel([-1, MIN_WORLD_Y, -17]), UNKNOWN_VOXEL);
    assert_eq!(
        world.volume().voxel([-17, MIN_WORLD_Y - 1, -17]),
        Block::Air as u8
    );

    let origin = world.volume().origin();
    for z in 0..PHASE_SIZE {
        for y in 0..PHASE_SIZE {
            for x in 0..PHASE_SIZE {
                let position = [origin[0] + x, origin[1] + y, origin[2] + z];
                let expected = world
                    .block_at(position[0], position[1], position[2])
                    .map_or(UNKNOWN_VOXEL, |block| block as u8);
                let index = ((z * PHASE_SIZE + y) * PHASE_SIZE + x) as usize;
                assert_eq!(world.volume().bytes()[index], expected, "{position:?}");
                assert_eq!(world.volume().voxel(position), expected, "{position:?}");
            }
        }
    }
    assert_eq!(world.world.len(), 4);
}

#[test]
fn phasing_requires_flight_and_the_exact_known_solid_eye_block() {
    let terrain = Terrain::default();
    let chunk = Chunk::generate(&terrain, ChunkCoord::new(-1, -1));
    let mut world = PhaseWorld::default();
    world.insert(chunk);
    let rock_eye = Vec3::new(-0.5, MIN_WORLD_Y as f32 + 0.5, -0.5);

    assert!(world.phasing(rock_eye, true));
    assert!(!world.phasing(rock_eye, false));
    assert!(!world.phasing(Vec3::new(0.0, rock_eye.y, -0.5), true));
    assert!(!world.phasing(Vec3::new(-0.5, MIN_WORLD_Y as f32 - 0.01, -0.5), true));
    assert!(!world.phasing(Vec3::new(-0.5, WORLD_HEIGHT as f32, -0.5), true));

    for y in MIN_WORLD_Y..WORLD_HEIGHT {
        let eye = Vec3::new(-0.5, y as f32 + 0.5, -0.5);
        assert_eq!(
            world.phasing(eye, true),
            terrain.block_at(-1, y, -1).is_solid()
        );
    }
}

#[test]
fn invalid_eyes_cannot_enable_phase_or_replace_a_valid_cached_volume() {
    let mut world = PhaseWorld::default();
    let eye = Vec3::new(0.5, -30.5, 0.5);
    assert!(world.update_volume(eye));
    let origin = world.volume().origin();
    let generation = world.volume().generation();
    for invalid in [
        Vec3::NAN,
        Vec3::splat(f32::INFINITY),
        Vec3::splat(f32::NEG_INFINITY),
        Vec3::new(i32::MIN as f32, 0.0, 0.0),
        Vec3::new(i32::MAX as f32, 0.0, 0.0),
        Vec3::new(0.0, i32::MIN as f32, 0.0),
        Vec3::new(0.0, 0.0, i32::MIN as f32),
        Vec3::splat(f32::MAX),
    ] {
        assert!(!world.phasing(invalid, true));
        assert!(!world.update_volume(invalid));
        assert_eq!(world.volume().origin(), origin);
        assert_eq!(world.volume().generation(), generation);
    }
    assert!(!world.update_volume(eye));
}

#[test]
fn four_block_anchors_cache_sub_block_motion_and_recenter_each_axis() {
    let mut world = PhaseWorld::default();
    assert!(world.update_volume(Vec3::new(0.25, -30.25, 0.25)));
    let allocation = world.volume().bytes().as_ptr();
    let generation = world.volume().generation();
    assert_eq!(world.volume().origin(), [-24, -56, -24]);
    for eye in [Vec3::new(3.99, -28.01, 3.99), Vec3::new(0.0, -32.0, 0.0)] {
        assert!(!world.update_volume(eye));
        assert_eq!(world.volume().generation(), generation);
    }
    for eye in [
        Vec3::new(4.0, -32.0, 0.0),
        Vec3::new(4.0, -28.0, 0.0),
        Vec3::new(4.0, -28.0, 4.0),
        Vec3::new(-0.01, -28.0, 4.0),
    ] {
        assert!(world.update_volume(eye));
        assert_eq!(world.volume().bytes().as_ptr(), allocation);
    }
    assert_eq!(world.volume().origin(), [-28, -52, -20]);
    assert_eq!(world.volume().generation(), generation + 4);
}

#[test]
fn distant_insert_remove_clear_and_nonresident_remove_do_not_invalidate() {
    let mut world = PhaseWorld::default();
    let eye = Vec3::new(8.5, -30.5, 8.5);
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().origin(), [-16, -56, -16]);
    let generation = world.volume().generation();
    let terrain = Terrain::default();
    for coord in [
        ChunkCoord::new(2, 0),
        ChunkCoord::new(-2, 0),
        ChunkCoord::new(0, 2),
        ChunkCoord::new(0, -2),
        ChunkCoord::new(7, 7),
    ] {
        world.insert(Chunk::generate(&terrain, coord));
        assert!(!world.update_volume(eye), "{coord:?}");
    }
    world.remove(ChunkCoord::new(7, 7));
    world.remove(ChunkCoord::new(0, 0));
    assert!(!world.update_volume(eye));
    world.clear();
    assert!(!world.update_volume(eye));
    assert_eq!(world.volume().generation(), generation);
    assert!(world.world.is_empty());
}

#[test]
fn overlapping_insert_replace_remove_and_clear_refresh_resident_voxels() {
    let terrain = Terrain::default();
    let coord = ChunkCoord::new(0, 0);
    let eye = Vec3::new(8.5, -40.5, 8.5);
    let point = [0, MIN_WORLD_Y, 0];
    let mut world = PhaseWorld::default();
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), UNKNOWN_VOXEL);

    world.insert(Chunk::generate(&terrain, coord));
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), Block::Bedrock as u8);
    let before = world.volume().bytes().to_vec();
    world.insert(Chunk::generate(&Terrain::new(terrain.seed() + 1), coord));
    assert_eq!(world.world.len(), 1);
    assert!(world.update_volume(eye));
    assert_ne!(world.volume().bytes(), before);
    assert!(!world.update_volume(eye));

    world.remove(coord);
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), UNKNOWN_VOXEL);
    assert!(!world.phasing(Vec3::from_array(point.map(|axis| axis as f32 + 0.5)), true));
    world.remove(coord);
    assert!(!world.update_volume(eye));

    world.insert(Chunk::generate(&terrain, coord));
    assert!(world.update_volume(eye));
    world.clear();
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), UNKNOWN_VOXEL);
    assert!(world.world.is_empty());
    world.clear();
    assert!(!world.update_volume(eye));
}

#[test]
fn a_partial_edge_chunk_is_relevant_and_removal_does_not_invent_air() {
    let mut world = PhaseWorld::default();
    let eye = Vec3::new(0.5, -40.5, 0.5);
    assert!(world.update_volume(eye));
    let coord = ChunkCoord::new(1, -2);
    let point = [23, MIN_WORLD_Y, -24];
    assert_eq!(world.volume().voxel(point), UNKNOWN_VOXEL);
    world.insert(Chunk::generate(&Terrain::default(), coord));
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), Block::Bedrock as u8);
    assert_eq!(world.volume().voxel([24, MIN_WORLD_Y, -24]), UNKNOWN_VOXEL);
    world.remove(coord);
    assert!(world.update_volume(eye));
    assert_eq!(world.volume().voxel(point), UNKNOWN_VOXEL);
}

#[test]
fn volumes_outside_the_vertical_world_are_air_and_ignore_residency_changes() {
    let mut world = PhaseWorld::default();
    let terrain = Terrain::default();
    let coord = ChunkCoord::new(0, 0);
    for eye in [Vec3::new(0.5, 200.5, 0.5), Vec3::new(0.5, -100.5, 0.5)] {
        assert!(world.update_volume(eye));
        assert!(
            world
                .volume()
                .bytes()
                .iter()
                .all(|&voxel| voxel == Block::Air as u8)
        );
        world.insert(Chunk::generate(&terrain, coord));
        assert!(!world.update_volume(eye));
        assert!(!world.phasing(eye, true));
        world.remove(coord);
        assert!(!world.update_volume(eye));
    }
}

#[test]
fn euclidean_anchors_keep_the_full_ray_range_inside_the_volume() {
    for coordinate in -8..=8 {
        for fraction in [0.0, 0.001, 0.5, 0.999] {
            let eye = Vec3::splat(coordinate as f32 + fraction);
            let cell = world_cell(eye).unwrap();
            let origin = volume_origin(cell).unwrap();
            for (axis, &start) in origin.iter().enumerate() {
                assert!(eye[axis] - PHASE_RANGE >= start as f32);
                assert!(eye[axis] + PHASE_RANGE < (start + PHASE_SIZE) as f32);
                assert_eq!((start + PHASE_SIZE / 2).rem_euclid(4), 0);
            }
        }
    }
    assert_eq!(volume_origin([-1; 3]), Some([-28; 3]));
    assert_eq!(volume_origin([-4; 3]), Some([-28; 3]));
    assert_eq!(volume_origin([-5; 3]), Some([-32; 3]));
}

#[test]
fn volume_bounds_are_checked_before_any_i32_coordinate_addition() {
    assert_eq!(
        volume_origin([i32::MIN + 24, 0, 0]),
        Some([i32::MIN, -24, -24])
    );
    assert_eq!(
        volume_origin([i32::MAX - 23, 0, 0]),
        Some([i32::MAX - 47, -24, -24])
    );
    for coordinate in [i32::MIN, i32::MIN + 23, i32::MAX - 19, i32::MAX] {
        for axis in 0..3 {
            let mut cell = [0; 3];
            cell[axis] = coordinate;
            assert_eq!(volume_origin(cell), None, "{cell:?}");
        }
    }
}
