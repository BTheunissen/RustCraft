use super::*;

#[test]
fn all_biomes_are_near_spawn_for_multiple_seeds() {
    for seed in [0, 1, 2, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        let mut counts = [0; 4];
        for dz in (-128..=128).step_by(8) {
            for dx in (-128..=128).step_by(8) {
                let biome = terrain
                    .column(spawn[0] as i32 + dx, spawn[2] as i32 + dz)
                    .biome;
                counts[biome_index(biome)] += 1;
            }
        }
        assert!(
            counts.into_iter().all(|count| count >= 32),
            "seed {seed}, spawn {spawn:?}: {counts:?}"
        );
    }
    assert_eq!(Biome::Plains.name(), "Plains");
    assert_eq!(Biome::Forest.name(), "Forest");
    assert_eq!(Biome::Desert.name(), "Desert");
    assert_eq!(Biome::Alpine.name(), "Alpine");
}

#[test]
fn default_spawn_view_contains_recognizable_biome_surfaces() {
    let terrain = Terrain::default();
    let spawn = terrain.spawn_position();
    let mut surfaces = [0; 4];
    let mut forest_trees = 0;
    for dz in -64..=64 {
        for dx in -64..=64 {
            let x = spawn[0].floor() as i32 + dx;
            let z = spawn[2].floor() as i32 + dz;
            let column = terrain.column(x, z);
            let surface = terrain.block_at(x, column.height, z);
            let recognizable = match column.biome {
                Biome::Plains | Biome::Forest => surface == Block::Grass,
                Biome::Desert => surface == Block::Sand,
                Biome::Alpine => surface == Block::Snow,
            };
            surfaces[biome_index(column.biome)] += usize::from(recognizable);
            if column.biome == Biome::Forest
                && terrain.block_at(x, column.height + 1, z) == Block::Wood
            {
                forest_trees += 1;
            }
        }
    }
    assert!(surfaces.into_iter().all(|count| count >= 128));
    assert!(forest_trees >= 16);
}

#[test]
fn tree_density_is_greater_in_forests_than_plains() {
    let terrain = Terrain::default();
    let mut columns = [0; 4];
    let mut trunks = [0; 4];
    for z in -192..=192 {
        for x in -192..=192 {
            let column = terrain.column(x, z);
            let biome = biome_index(column.biome);
            columns[biome] += 1;
            if terrain.block_at(x, column.height + 1, z) == Block::Wood {
                trunks[biome] += 1;
            }
        }
    }
    assert!(trunks[0] > 0);
    assert!(trunks[1] * columns[0] > trunks[0] * columns[1] * 3);
    assert_eq!(trunks[2], 0);
    assert_eq!(trunks[3], 0);
}

#[test]
fn carved_surface_roots_cannot_place_trees_or_claim_spawn_support() {
    let terrain = Terrain::default();
    let mut carved_roots = 0;
    let mut chunks = std::collections::BTreeMap::new();
    for cell_z in -64..=64 {
        for cell_x in -64..=64 {
            let Some(candidate) = terrain.tree_candidate(cell_x, cell_z) else {
                continue;
            };
            let column = terrain.column(candidate.x, candidate.z);
            let Some(tree) = Tree::from_candidate(candidate, column) else {
                continue;
            };
            if terrain.block_at(tree.x, tree.base_y - 1, tree.z).is_solid() {
                continue;
            }
            carved_roots += 1;
            assert!(!terrain.spawn_is_clear(tree.x, tree.z, column.height));
            let (chunk_x, local_x) = split_block_coordinate(tree.x);
            let (chunk_z, local_z) = split_block_coordinate(tree.z);
            let coord = ChunkCoord::new(chunk_x, chunk_z);
            let chunk = chunks
                .entry(coord)
                .or_insert_with(|| Chunk::generate(&terrain, coord));
            for y in MIN_WORLD_Y..=column.height {
                assert!(!matches!(
                    chunk.block(local_x, y, local_z),
                    Block::Wood | Block::Leaves
                ));
            }
            assert_ne!(chunk.block(local_x, tree.base_y, local_z), Block::Wood);
            assert_ne!(terrain.block_at(tree.x, tree.base_y, tree.z), Block::Wood);
        }
    }
    assert!(carved_roots >= 8, "only {carved_roots} carved roots");
}

#[test]
fn trees_cross_positive_and_negative_chunk_seams_consistently() {
    let terrain = Terrain::default();
    for (negative, along_x) in [(false, true), (true, true), (false, false), (true, false)] {
        let tree = find_seam_tree(&terrain, negative, along_x);
        let canopy_y = tree.base_y + tree.trunk_height - 2;
        let mut chunks = std::collections::BTreeMap::new();
        let mut decorated_chunks = std::collections::BTreeSet::new();
        for z in tree.z - TREE_RADIUS as i32..=tree.z + TREE_RADIUS as i32 {
            for x in tree.x - TREE_RADIUS as i32..=tree.x + TREE_RADIUS as i32 {
                let coord =
                    ChunkCoord::new(split_block_coordinate(x).0, split_block_coordinate(z).0);
                let chunk = chunks
                    .entry(coord)
                    .or_insert_with(|| Chunk::generate(&terrain, coord));
                for y in tree.base_y..=tree.base_y + tree.trunk_height + 1 {
                    let block =
                        chunk.block(split_block_coordinate(x).1, y, split_block_coordinate(z).1);
                    assert_eq!(block, terrain.block_at(x, y, z));
                    if y == canopy_y && block == Block::Leaves {
                        decorated_chunks.insert(coord);
                    }
                }
            }
        }
        assert!(decorated_chunks.len() >= 2);
        for coord in chunks.keys() {
            assert_chunk_matches(&terrain, *coord);
        }
    }
}

#[test]
fn trees_have_blocky_canopies_and_deterministic_overlap_priority() {
    let terrain = Terrain::default();
    let tree = find_seam_tree(&terrain, false, true);
    for y in tree.base_y..tree.base_y + tree.trunk_height {
        assert_eq!(tree.block_at(tree.x, y, tree.z), Block::Wood);
        assert_eq!(terrain.block_at(tree.x, y, tree.z), Block::Wood);
    }
    assert_eq!(
        tree.block_at(tree.x + 2, tree.base_y + tree.trunk_height - 2, tree.z),
        Block::Leaves
    );
    assert_eq!(
        tree.block_at(tree.x + 2, tree.base_y + tree.trunk_height + 1, tree.z),
        Block::Air
    );
    assert_eq!(tree.block_at(tree.x, tree.base_y - 1, tree.z), Block::Air);

    let coord = ChunkCoord::new(0, 0);
    let first = Tree {
        x: 4,
        z: 4,
        base_y: 25,
        trunk_height: 4,
    };
    let second = Tree {
        x: 5,
        z: 4,
        base_y: 26,
        trunk_height: 5,
    };
    let empty = || Chunk {
        coord,
        blocks: vec![Block::Air; super::BLOCK_COUNT].into_boxed_slice(),
        water_columns: Box::new([None; super::COLUMN_COUNT]),
        coastal_roofs: Box::new([None; super::COLUMN_COUNT]),
    };
    let mut left = empty();
    let mut right = empty();
    let columns = [super::Column {
        height: 0,
        biome: Biome::Plains,
    }; super::COLUMN_COUNT];
    left.blocks[super::block_index(4, 25, 4)] = Block::Stone;
    right.blocks[super::block_index(4, 25, 4)] = Block::Stone;
    left.apply_tree(first, 0, 0, &columns);
    left.apply_tree(second, 0, 0, &columns);
    right.apply_tree(second, 0, 0, &columns);
    right.apply_tree(first, 0, 0, &columns);
    assert_eq!(left.blocks, right.blocks);
    assert_eq!(left.block(4, 25, 4), Block::Stone);
    assert_eq!(left.block(5, 28, 4), Block::Wood);
}

#[test]
fn spawn_is_reproducible_dry_and_clear_for_a_standing_player() {
    for seed in [
        0,
        1,
        2,
        3,
        42,
        Terrain::default().seed(),
        u64::MAX,
        0xDEAD_BEEF,
    ] {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        assert_eq!(spawn, Terrain::new(seed).spawn_position());
        let x = spawn[0].floor() as i32;
        let y = spawn[1] as i32;
        let z = spawn[2].floor() as i32;
        let column = terrain.column(x, z);
        assert_eq!(spawn[0] - x as f32, 0.5);
        assert_eq!(spawn[2] - z as f32, 0.5);
        assert_eq!(y, column.height + 1);
        assert!(y > SEA_LEVEL);
        assert!(terrain.block_at(x, y - 1, z).is_solid());
        for dz in -1..=1 {
            for dx in -1..=1 {
                for headroom in 0..3 {
                    assert_eq!(terrain.block_at(x + dx, y + headroom, z + dz), Block::Air);
                }
                let height = terrain.column(x + dx, z + dz).height;
                assert!(terrain.block_at(x + dx, height, z + dz).is_solid());
            }
        }
    }
}

#[test]
fn tree_candidates_reject_dry_biomes_and_underwater_roots() {
    let candidate = TreeCandidate {
        x: -1,
        z: 16,
        hash: 0,
    };
    for biome in [Biome::Desert, Biome::Alpine] {
        assert!(Tree::from_candidate(candidate, super::Column { height: 30, biome }).is_none());
    }
    assert!(
        Tree::from_candidate(
            candidate,
            super::Column {
                height: SEA_LEVEL - 1,
                biome: Biome::Forest,
            }
        )
        .is_none()
    );
    assert_eq!(tree_cells(-16, -1), -3..=0);
    assert_eq!(tree_cells(0, 15), -1..=2);
    assert_eq!(
        terrain_block(
            super::Column {
                height: 18,
                biome: Biome::Plains
            },
            18
        ),
        Block::Sand
    );
}
