use super::*;

#[test]
fn terrain_is_repeatable_and_seed_sensitive() {
    let terrain = Terrain::default();
    let copy = Terrain::new(terrain.seed());
    let other = Terrain::new(terrain.seed().wrapping_add(1));
    let mut changed_columns = 0;
    let mut changed_underground = 0;
    for z in (-240..=240).step_by(13) {
        for x in (-240..=240).step_by(11) {
            let column = terrain.column(x, z);
            assert_eq!(column, copy.column(x, z));
            for y in [
                MIN_WORLD_Y,
                -45,
                -15,
                0,
                column.height,
                column.height + 1,
                column.height + 5,
            ] {
                assert_eq!(terrain.block_at(x, y, z), copy.block_at(x, y, z));
            }
            changed_columns += usize::from(column != other.column(x, z));
            for y in [-50, -40, -20, -10] {
                changed_underground +=
                    usize::from(terrain.block_at(x, y, z) != other.block_at(x, y, z));
            }
        }
    }
    assert!(changed_columns > 500);
    assert!(changed_underground > 500);

    let coord = ChunkCoord::new(-2, 3);
    let first = Chunk::generate(&terrain, coord);
    let _unrelated = Chunk::generate(&terrain, ChunkCoord::new(11, -9));
    assert_eq!(first.blocks, Chunk::generate(&copy, coord).blocks);
    assert_ne!(first.blocks, Chunk::generate(&other, coord).blocks);
}

#[test]
fn base_terrain_heights_are_bounded_and_smooth_across_biome_edges() {
    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        let mut edges = 0;
        for z in (-384..=384).step_by(7) {
            for x in (-384..=384).step_by(7) {
                let column = terrain.base_column(x, z);
                assert!((MIN_BASE_TERRAIN_HEIGHT..=MAX_TERRAIN_HEIGHT).contains(&column.height));
                for (dx, dz) in [(1, 0), (0, 1)] {
                    let neighbor = terrain.base_column(x + dx, z + dz);
                    assert!(
                        (column.height - neighbor.height).abs() <= 2,
                        "seed {seed}, ({x}, {z}): {column:?} -> {neighbor:?}"
                    );
                    edges += usize::from(column.biome != neighbor.biome);
                }
            }
        }
        assert!(edges > 0);
    }
}

#[test]
fn biome_surfaces_have_expected_layers_and_water_uses_local_profiles() {
    let terrain = Terrain::default();
    let mut surfaces = [false; 4];
    let mut found_water = false;
    let mut found_rocky_alpine = false;
    for z in (-448..=448).step_by(4) {
        for x in (-448..=448).step_by(4) {
            let column = terrain.column(x, z);
            assert_eq!(terrain.block_at(x, MIN_WORLD_Y, z), Block::Bedrock);
            if let Some(water) = terrain.water_column(x, z) {
                found_water = true;
                assert_eq!(column.height, water.bottom);
                assert!(terrain.block_at(x, column.height, z).is_solid());
                assert_eq!(terrain.block_at(x, column.height + 1, z), Block::Water);
                assert_eq!(
                    terrain.block_at(x, water.surface.ceil() as i32 - 1, z),
                    Block::Water
                );
                assert_ne!(
                    terrain.block_at(x, water.surface.ceil() as i32, z),
                    Block::Water
                );
            }
            if column.height < SEA_LEVEL + 1 {
                continue;
            }
            let (surface, subsoil) = match column.biome {
                Biome::Plains | Biome::Forest => (Block::Grass, Block::Dirt),
                Biome::Desert => (Block::Sand, Block::Sand),
                Biome::Alpine if column.height >= 40 => (Block::Snow, Block::Stone),
                Biome::Alpine => {
                    found_rocky_alpine = true;
                    (Block::Stone, Block::Stone)
                }
            };
            for depth in 0..=4 {
                let expected = match depth {
                    0 => surface,
                    4 => Block::Stone,
                    _ => subsoil,
                };
                assert_eq!(terrain_block(column, column.height - depth), expected);
            }
            if column.biome != Biome::Alpine || surface == Block::Snow {
                surfaces[biome_index(column.biome)] = true;
            }
        }
    }
    assert!(surfaces.into_iter().all(|seen| seen));
    assert!(found_water);
    assert!(found_rocky_alpine);
}

#[test]
fn surface_relief_has_alpine_massifs_desert_crests_and_walkable_lowlands() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let mut minimum = [WORLD_HEIGHT; 4];
        let mut maximum = [MIN_TERRAIN_HEIGHT; 4];
        let mut alpine_high = 0;
        let mut alpine_land = 0;
        let mut lowland_steps = 0;
        let mut lowland_columns = 0;
        for z in (-384..=384).step_by(4) {
            for x in (-384..=384).step_by(4) {
                let column = terrain.column(x, z);
                let index = biome_index(column.biome);
                minimum[index] = minimum[index].min(column.height);
                maximum[index] = maximum[index].max(column.height);
                alpine_high += usize::from(column.biome == Biome::Alpine && column.height >= 55);
                alpine_land += usize::from(
                    column.biome == Biome::Alpine && terrain.water_column(x, z).is_none(),
                );
                if matches!(column.biome, Biome::Plains | Biome::Forest) {
                    lowland_columns += 1;
                    lowland_steps +=
                        usize::from((column.height - terrain.column(x + 1, z).height).abs() <= 1);
                }
            }
        }
        assert!(
            maximum[3] - minimum[3] >= 26,
            "seed {seed}: alpine relief {minimum:?}..{maximum:?}"
        );
        assert!(
            alpine_high >= 128 && alpine_high * 100 >= alpine_land * 3,
            "seed {seed}: only {alpine_high} high massif columns among {alpine_land} dry alpine columns"
        );
        assert!(
            maximum[2] - minimum[2] >= 13,
            "seed {seed}: desert relief {minimum:?}..{maximum:?}"
        );
        assert!(lowland_steps * 100 >= lowland_columns * 98);
    }
}
