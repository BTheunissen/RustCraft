use super::*;

#[test]
fn splits_non_negative_coordinates_at_chunk_boundaries() {
    for (coordinate, expected) in [
        (0, (0, 0)),
        (1, (0, 1)),
        (15, (0, 15)),
        (16, (1, 0)),
        (17, (1, 1)),
        (31, (1, 15)),
        (32, (2, 0)),
        (33, (2, 1)),
    ] {
        assert_eq!(
            split_block_coordinate(coordinate),
            expected,
            "world coordinate: {coordinate}"
        );
    }
}

#[test]
fn splits_negative_coordinates_at_chunk_boundaries() {
    for (coordinate, expected) in [
        (-1, (-1, 15)),
        (-15, (-1, 1)),
        (-16, (-1, 0)),
        (-17, (-2, 15)),
        (-31, (-2, 1)),
        (-32, (-2, 0)),
        (-33, (-3, 15)),
    ] {
        assert_eq!(
            split_block_coordinate(coordinate),
            expected,
            "world coordinate: {coordinate}"
        );
    }
}

#[test]
fn supports_extreme_world_coordinates() {
    assert_eq!(split_block_coordinate(i32::MIN), (-134_217_728, 0));
    assert_eq!(split_block_coordinate(i32::MAX), (134_217_727, 15));
}

#[test]
fn reconstructs_world_coordinates_with_bounded_local_offsets() {
    for coordinate in -4_096..=4_096 {
        let (chunk, local) = split_block_coordinate(coordinate);

        assert!(
            (0..CHUNK_SIZE).contains(&local),
            "world coordinate: {coordinate}, local offset: {local}"
        );
        assert_eq!(
            chunk * CHUNK_SIZE + local,
            coordinate,
            "world coordinate: {coordinate}"
        );
    }
}

#[test]
fn world_positions_use_floor_division_on_both_axes() {
    for (position, expected) in [
        ([0.0, 15.999], ChunkCoord::new(0, 0)),
        ([16.0, 32.0], ChunkCoord::new(1, 2)),
        ([-0.001, -15.999], ChunkCoord::new(-1, -1)),
        ([-16.0, -16.001], ChunkCoord::new(-1, -2)),
        ([31.999, -32.0], ChunkCoord::new(1, -2)),
    ] {
        assert_eq!(ChunkCoord::from_world(position[0], position[1]), expected);
    }
}

#[test]
fn chunk_matches_world_samples_for_negative_and_positive_coordinates() {
    for seed in [Terrain::default().seed(), 7] {
        let terrain = Terrain::new(seed);
        for coord in [
            ChunkCoord::new(-7, -5),
            ChunkCoord::new(-1, 0),
            ChunkCoord::new(0, 0),
            ChunkCoord::new(3, 4),
        ] {
            assert_chunk_matches(&terrain, coord);
        }
    }
}

#[test]
fn batched_boundary_rays_match_individual_columns_chunks_and_extreme_coordinates() {
    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        for coord in [ChunkCoord::new(-3, -2), ChunkCoord::new(0, 0)] {
            let origin_x = coord.x * CHUNK_SIZE;
            let origin_z = coord.z * CHUNK_SIZE;
            let positions: Vec<_> = (0..CHUNK_SIZE)
                .flat_map(|offset| {
                    [
                        [origin_x - 1, origin_z + offset],
                        [origin_x + CHUNK_SIZE, origin_z + offset],
                        [origin_x + offset, origin_z - 1],
                        [origin_x + offset, origin_z + CHUNK_SIZE],
                    ]
                })
                .collect();
            let batches = terrain.column_block_batch(&positions);
            assert_eq!(batches.len(), positions.len());
            for (&[x, z], blocks) in positions.iter().zip(batches) {
                assert_eq!(
                    blocks,
                    terrain.column_blocks(x, z),
                    "seed {seed}, halo ({x},{z})"
                );
                let chunk = Chunk::generate(
                    &terrain,
                    ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE)),
                );
                for y in MIN_WORLD_Y..WORLD_HEIGHT {
                    assert_eq!(
                        blocks[(y - MIN_WORLD_Y) as usize],
                        chunk.block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
                    );
                }
            }
        }
        assert!(terrain.column_block_batch(&[]).is_empty());
        for positions in [
            vec![[i32::MIN, i32::MAX], [i32::MIN + 1, i32::MAX - 1]],
            vec![[i32::MIN, i32::MAX], [i32::MAX, i32::MIN], [0, 0], [0, 0]],
        ] {
            for (&[x, z], blocks) in positions.iter().zip(terrain.column_block_batch(&positions)) {
                assert_eq!(blocks, terrain.column_blocks(x, z));
            }
        }
    }
}
