use super::*;

#[test]
fn block_materials_have_stable_ids_and_collision_rules() {
    for (id, block) in Block::ALL.into_iter().enumerate() {
        assert_eq!(block as usize, id);
        assert_eq!(block.is_solid(), id != 0 && id != 8 && id != 16);
        assert_eq!(block.is_opaque(), id != 0 && id != 8);
        assert_eq!(block.is_liquid(), id == 8 || id == 16);
    }
}

#[test]
fn cached_vertical_columns_match_chunks_and_direct_samples() {
    for seed in [42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        for coord in [
            ChunkCoord::new(-3, 2),
            ChunkCoord::new(-1, -1),
            ChunkCoord::new(0, 0),
            ChunkCoord::new(2, -4),
        ] {
            let chunk = Chunk::generate(&terrain, coord);
            for (x, z) in [(0, 0), (15, 0), (0, 15), (15, 15), (7, 8)] {
                let world_x = coord.x * CHUNK_SIZE + x;
                let world_z = coord.z * CHUNK_SIZE + z;
                let blocks = terrain.column_blocks(world_x, world_z);
                assert_eq!(blocks.len(), WORLD_DEPTH as usize);
                for (index, block) in blocks.into_iter().enumerate() {
                    let y = MIN_WORLD_Y + index as i32;
                    assert_eq!(
                        block,
                        chunk.block(x, y, z),
                        "seed {seed}, ({world_x}, {y}, {world_z})"
                    );
                    assert_eq!(block, terrain.block_at(world_x, y, world_z));
                }
            }
        }
    }
}

#[test]
fn warped_landforms_and_growth_halos_match_sampling_across_chunk_seams() {
    use super::CaveLandform;

    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        for form in [
            CaveLandform::Gallery,
            CaveLandform::Rift,
            CaveLandform::Cathedral,
            CaveLandform::Labyrinth,
        ] {
            let site = terrain.cave_landmark(form).unwrap();
            let center = ChunkCoord::from_world(site.position[0], site.position[2]);
            for coord in [center, ChunkCoord::new(center.x - 1, center.z)] {
                let chunk = Chunk::generate(&terrain, coord);
                for (local_x, local_z) in [(0, 0), (15, 0), (0, 15), (15, 15)] {
                    let x = coord.x * CHUNK_SIZE + local_x;
                    let z = coord.z * CHUNK_SIZE + local_z;
                    let column = terrain.column_blocks(x, z);
                    for y in MIN_WORLD_Y..WORLD_HEIGHT {
                        let expected = chunk.block(local_x, y, local_z);
                        assert_eq!(column[(y - MIN_WORLD_Y) as usize], expected);
                        assert_eq!(
                            terrain.block_at(x, y, z),
                            expected,
                            "seed {seed}, {form:?} seam ({x},{y},{z})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bedrock_seals_every_column_without_erasing_the_surface_height_contract() {
    for seed in [42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        let mut irregular = [false; 3];
        for coord in [
            ChunkCoord::new(-2, -1),
            ChunkCoord::new(0, 0),
            ChunkCoord::new(4, -3),
        ] {
            let chunk = Chunk::generate(&terrain, coord);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let world_x = coord.x * CHUNK_SIZE + x;
                    let world_z = coord.z * CHUNK_SIZE + z;
                    let top = super::super::underground::bedrock_top(seed, world_x, world_z);
                    irregular[(top - MIN_WORLD_Y) as usize] = true;
                    for y in MIN_WORLD_Y..=top {
                        assert_eq!(chunk.block(x, y, z), Block::Bedrock);
                    }
                    assert_ne!(chunk.block(x, top + 1, z), Block::Bedrock);
                    let column = terrain.column(world_x, world_z);
                    assert!((MIN_TERRAIN_HEIGHT..=MAX_TERRAIN_HEIGHT).contains(&column.height));
                }
            }
        }
        assert!(irregular.into_iter().all(|present| present));
    }
}

#[test]
fn out_of_bounds_blocks_are_air_and_extreme_horizontal_samples_do_not_overflow() {
    let terrain = Terrain::default();
    let chunk = Chunk::generate(&terrain, ChunkCoord::new(-1, -1));
    assert_eq!(chunk.coord(), ChunkCoord::new(-1, -1));
    for (x, y, z) in [
        (-1, 0, 0),
        (CHUNK_SIZE, 0, 0),
        (0, MIN_WORLD_Y - 1, 0),
        (0, WORLD_HEIGHT, 0),
        (0, 0, -1),
        (0, 0, CHUNK_SIZE),
        (i32::MIN, i32::MAX, i32::MAX),
    ] {
        assert_eq!(chunk.block(x, y, z), Block::Air);
    }
    for y in [i32::MIN, MIN_WORLD_Y - 1, WORLD_HEIGHT, i32::MAX] {
        assert_eq!(terrain.block_at(0, y, 0), Block::Air);
    }
    for (x, z) in [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
        let column = terrain.column(x, z);
        assert!(column.height >= MIN_TERRAIN_HEIGHT);
        assert_eq!(terrain.block_at(x, MIN_WORLD_Y, z), Block::Bedrock);
        assert_eq!(terrain.block_at(x, WORLD_HEIGHT - 1, z), Block::Air);
        let coord = ChunkCoord::new(split_block_coordinate(x).0, split_block_coordinate(z).0);
        let extreme = Chunk::generate(&terrain, coord);
        for y in MIN_WORLD_Y..WORLD_HEIGHT {
            assert_eq!(
                extreme.block(split_block_coordinate(x).1, y, split_block_coordinate(z).1),
                terrain.block_at(x, y, z)
            );
        }
    }
    for coord in [ChunkCoord::new(i32::MIN, 0), ChunkCoord::new(0, i32::MAX)] {
        let invalid = Chunk::generate(&terrain, coord);
        assert!(invalid.blocks.iter().all(|block| *block == Block::Air));
    }
}
