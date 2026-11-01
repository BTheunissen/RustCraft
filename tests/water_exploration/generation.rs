use super::*;

#[test]
fn nearby_landmarks_name_real_water_with_distinct_physical_profiles_for_extreme_seeds() {
    for seed in seeds() {
        let terrain = Terrain::new(seed);
        let mut sites = BTreeSet::new();
        for kind in SURFACE_BIOMES {
            let site = landmark(&terrain, kind);
            assert!(
                sites.insert(site.water),
                "seed {seed} reuses a wet landmark voxel"
            );
            let [x, y, z] = site.water;
            let sample = terrain.water_at(x, y, z).unwrap();
            let profile = terrain.water_column(x, z).unwrap();
            assert_profile(sample);
            assert_water_eq(Some(sample), Some(profile));
            assert_eq!(terrain.column(x, z).height, sample.bottom);
            assert!(terrain.block_at(x, sample.bottom, z).is_solid());
            assert!(terrain.water_at(x, sample.bottom, z).is_none());
            assert!(
                terrain
                    .water_at(x, sample.surface.ceil() as i32, z)
                    .is_none()
            );
            match kind {
                WaterBiome::Lake => {
                    assert!(
                        sample.surface > SEA_LEVEL as f32 + 1.0,
                        "seed {seed}, sea-level lake: {sample:?}"
                    );
                    assert_eq!(sample.salinity, 0.0);
                    assert!(Vec3::from_array(sample.flow).length() < 0.01);
                }
                WaterBiome::River => {
                    assert_eq!(sample.salinity, 0.0);
                    assert!(Vec3::from_array(sample.flow).with_y(0.0).length() > 0.05);
                }
                WaterBiome::Estuary => assert!(
                    (0.0..1.0).contains(&sample.salinity) && sample.salinity > 0.0,
                    "seed {seed}, estuary lacks mixed salinity: {sample:?}"
                ),
                WaterBiome::Ocean => assert_eq!(sample.salinity, 1.0),
                WaterBiome::Waterfall => {
                    assert_eq!(sample.salinity, 0.0);
                    assert!(sample.flow[1] < -0.05);
                    let (_, core) = waterfall_core(&terrain, site);
                    assert!(
                        depth(core) >= 4.0,
                        "seed {seed}, waterfall lacks a four-block core: {core:?}"
                    );
                }
                WaterBiome::CavePool => unreachable!(),
            }
        }
    }
}

#[test]
fn seeded_hydrology_repeats_changes_with_seed_and_survives_negative_coordinates() {
    let terrain = Terrain::default();
    let copy = Terrain::new(terrain.seed());
    let other = Terrain::new(terrain.seed().wrapping_add(1));
    let mut changed = 0;
    let mut wet = 0;
    let mut negative_wet = 0;
    for z in (-640..=640).step_by(16) {
        for x in (-640..=640).step_by(16) {
            let a = terrain.water_column(x, z);
            assert_water_eq(a, copy.water_column(x, z));
            if let Some(sample) = a {
                assert_profile(sample);
                let y = sample.surface.ceil() as i32 - 1;
                assert_eq!(terrain.block_at(x, y, z), Block::Water);
                assert_water_eq(terrain.water_at(x, y, z), copy.water_at(x, y, z));
                wet += 1;
                negative_wet += usize::from(x < 0 || z < 0);
            }
            let b = other.water_column(x, z);
            changed += usize::from(match (a, b) {
                (None, None) => false,
                (Some(a), Some(b)) => {
                    a.bottom != b.bottom
                        || a.biome != b.biome
                        || (a.surface - b.surface).abs() > 0.001
                        || (Vec3::from_array(a.flow) - Vec3::from_array(b.flow)).length() > 0.001
                }
                _ => true,
            });
        }
    }
    assert!(
        wet > 100 && negative_wet > 20,
        "wet={wet}, negative={negative_wet}"
    );
    assert!(
        changed > 100,
        "too few seed-sensitive water columns: {changed}"
    );
    for [x, z] in [
        [-1, -1],
        [-16, -17],
        [i32::MIN, i32::MAX],
        [i32::MAX, i32::MIN],
    ] {
        assert_water_eq(terrain.water_column(x, z), copy.water_column(x, z));
        for y in [MIN_WORLD_Y - 1, WORLD_HEIGHT, i32::MIN, i32::MAX] {
            assert!(terrain.water_at(x, y, z).is_none());
        }
    }
}

#[test]
fn resident_water_agrees_with_batched_columns_direct_sampling_and_reverse_load_order_at_seams() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let mut coords = BTreeSet::from([
            ChunkCoord::new(-2, -2),
            ChunkCoord::new(-1, -2),
            ChunkCoord::new(-2, -1),
        ]);
        for kind in SURFACE_BIOMES {
            let wet = landmark(&terrain, kind).water;
            let center =
                ChunkCoord::new(wet[0].div_euclid(CHUNK_SIZE), wet[2].div_euclid(CHUNK_SIZE));
            coords.extend([
                center,
                ChunkCoord::new(center.x + 1, center.z),
                ChunkCoord::new(center.x, center.z + 1),
            ]);
        }
        let forward: BTreeMap<_, _> = coords
            .iter()
            .map(|&coord| (coord, Chunk::generate(&terrain, coord)))
            .collect();
        let reverse: BTreeMap<_, _> = coords
            .iter()
            .rev()
            .map(|&coord| (coord, Chunk::generate(&terrain, coord)))
            .collect();
        let mut actual_seam_water = 0;
        let mut partial_surfaces = 0;
        for (&coord, chunk) in &forward {
            let other = &reverse[&coord];
            let mut positions = Vec::new();
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let world = [coord.x * CHUNK_SIZE + x, coord.z * CHUNK_SIZE + z];
                    if x == 0 || x == CHUNK_SIZE - 1 || z == 0 || z == CHUNK_SIZE - 1 {
                        positions.push(world);
                    }
                    for y in MIN_WORLD_Y..WORLD_HEIGHT {
                        assert_eq!(
                            chunk.block(x, y, z),
                            other.block(x, y, z),
                            "seed {seed}, reverse load at {world:?}, y={y}"
                        );
                        assert_water_eq(chunk.water(x, y, z), other.water(x, y, z));
                        assert_eq!(
                            chunk.water(x, y, z).is_some(),
                            chunk.block(x, y, z) == Block::Water
                        );
                    }
                }
            }
            for (&[x, z], blocks) in positions.iter().zip(terrain.column_block_batch(&positions)) {
                let local = [x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE)];
                for y in MIN_WORLD_Y..WORLD_HEIGHT {
                    assert_eq!(
                        chunk.block(local[0], y, local[1]),
                        blocks[(y - MIN_WORLD_Y) as usize],
                        "seed {seed}, batched seam ({x}, {y}, {z})"
                    );
                    let sample = chunk.water(local[0], y, local[1]);
                    if let Some(sample) = sample {
                        actual_seam_water += 1;
                        partial_surfaces += usize::from(sample.surface.fract().abs() > 0.001);
                        if y == sample.surface.ceil() as i32 - 1 || y == sample.bottom + 1 {
                            assert_water_eq(Some(sample), terrain.water_at(x, y, z));
                            assert_eq!(terrain.block_at(x, y, z), Block::Water);
                        }
                    }
                }
                for y in [MIN_WORLD_Y, -36, -1, 0, WORLD_HEIGHT - 1] {
                    assert_water_eq(
                        chunk.water(local[0], y, local[1]),
                        terrain.water_at(x, y, z),
                    );
                    assert_eq!(
                        chunk.block(local[0], y, local[1]),
                        terrain.block_at(x, y, z)
                    );
                }
            }
            for [x, y, z] in [
                [-1, 18, 0],
                [CHUNK_SIZE, 18, 0],
                [0, MIN_WORLD_Y - 1, 0],
                [0, WORLD_HEIGHT, 0],
                [0, 18, -1],
                [0, 18, CHUNK_SIZE],
            ] {
                assert!(chunk.water(x, y, z).is_none());
            }
        }
        assert!(
            actual_seam_water > 100,
            "seed {seed}, no meaningful wet seam coverage"
        );
        assert!(
            partial_surfaces > 0,
            "seed {seed}, fractional surfaces absent at water seams"
        );
        let empty = Chunk::generate(&terrain, ChunkCoord::new(i32::MAX, i32::MIN));
        assert!(empty.water(0, SEA_LEVEL - 1, 0).is_none());
    }
}

#[test]
fn surface_beds_are_supported_sediment_caps_not_abandoned_global_sea_fill() {
    for seed in seeds() {
        let terrain = Terrain::new(seed);
        let mut materials = [0_usize; Block::COUNT];
        let mut supported = 0;
        for kind in SURFACE_BIOMES {
            let site = landmark(&terrain, kind);
            let center = ChunkCoord::new(
                site.water[0].div_euclid(CHUNK_SIZE),
                site.water[2].div_euclid(CHUNK_SIZE),
            );
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let coord = ChunkCoord::new(center.x + dx, center.z + dz);
                    let chunk = Chunk::generate(&terrain, coord);
                    for z in 0..CHUNK_SIZE {
                        for x in 0..CHUNK_SIZE {
                            let [world_x, world_z] =
                                [coord.x * CHUNK_SIZE + x, coord.z * CHUNK_SIZE + z];
                            let Some(profile) = terrain.water_column(world_x, world_z) else {
                                continue;
                            };
                            assert_eq!(terrain.column(world_x, world_z).height, profile.bottom);
                            materials[chunk.block(x, profile.bottom, z) as usize] += 1;
                            for y in profile.bottom - 3..=profile.bottom {
                                assert!(
                                    chunk.block(x, y, z).is_solid(),
                                    "seed {seed}, carved water bed ({world_x}, {y}, {world_z})"
                                );
                                assert!(chunk.water(x, y, z).is_none());
                            }
                            for y in profile.bottom + 1..profile.surface.ceil() as i32 {
                                assert_eq!(
                                    chunk.block(x, y, z),
                                    Block::Water,
                                    "seed {seed}, hovering/gapped water ({world_x}, {y}, {world_z})"
                                );
                                assert_water_eq(chunk.water(x, y, z), Some(profile));
                            }
                            let above = profile.surface.ceil() as i32;
                            assert_ne!(chunk.block(x, above, z), Block::Water);
                            assert!(chunk.water(x, above, z).is_none());
                            supported += 1;
                        }
                    }
                }
            }
        }
        assert!(
            supported > 1000,
            "seed {seed}, supported water columns={supported}"
        );
        for material in [Block::Sand, Block::Gravel, Block::Clay] {
            assert!(
                materials[material as usize] > 0,
                "seed {seed}, missing actual {material:?} bed"
            );
        }
        let mut dry_below_sea = 0;
        for z in (-640..=640).step_by(8) {
            for x in (-640..=640).step_by(8) {
                let column = terrain.column(x, z);
                if column.height + 1 >= SEA_LEVEL || terrain.water_column(x, z).is_some() {
                    continue;
                }
                let blocks = terrain.column_blocks(x, z);
                if blocks[(column.height - MIN_WORLD_Y) as usize].is_solid() {
                    assert_eq!(
                        blocks[(SEA_LEVEL - 1 - MIN_WORLD_Y) as usize],
                        Block::Air,
                        "seed {seed}, old blanket sea fill survives at ({x}, {z})"
                    );
                    dry_below_sea += 1;
                }
            }
        }
        assert!(
            dry_below_sea > 0,
            "seed {seed}, no supported dry sub-sea column exercising explicit footprint fill"
        );
    }
}
