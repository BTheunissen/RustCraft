use super::*;

#[test]
fn oceans_have_real_hundred_block_open_water_deep_floor_and_gradual_ordinary_shores() {
    for seed in seeds() {
        let terrain = Terrain::new(seed);
        let ocean = landmark(&terrain, WaterBiome::Ocean);
        let (point, deepest) = deepest_ocean(&terrain, ocean);
        assert!(
            depth(deepest) >= 15.0,
            "seed {seed}, deepest ocean={deepest:?}"
        );
        assert!(
            deepest.bottom < 0,
            "seed {seed}, ocean has no negative-Y seafloor"
        );
        assert_eq!(deepest.salinity, 1.0);
        let mut widest = 0;
        let mut floors = BTreeSet::new();
        let mut shore_edges = 0;
        let mut gradual_edges = 0;
        let mut max_shore_step = 0;
        let mut nonmarine_ocean = None;
        let mut pure_core_width = 0;
        for axis in 0..2 {
            let mut run = 0;
            let mut pure_run = 0;
            let mut previous: Option<WaterSample> = None;
            for offset in -320..=320 {
                let mut sample_point = point;
                sample_point[axis] += offset;
                let water = terrain.water_column(sample_point[0], sample_point[1]);
                if let Some(water) = water.filter(|water| water.biome == WaterBiome::Ocean) {
                    run += 1;
                    widest = widest.max(run);
                    floors.insert(water.bottom);
                    if water.salinity < 0.98 {
                        nonmarine_ocean.get_or_insert((sample_point, water));
                    }
                    pure_run = if water.salinity == 1.0 {
                        pure_run + 1
                    } else {
                        0
                    };
                    pure_core_width = pure_core_width.max(pure_run);
                    if let Some(previous) = previous {
                        assert!(
                            (water.bottom - previous.bottom).abs() <= 3,
                            "seed {seed}, one-block ocean floor wall at {sample_point:?}: {previous:?} -> {water:?}"
                        );
                    }
                    previous = Some(water);
                } else {
                    if let Some(previous) = previous {
                        let height = water.map_or_else(
                            || terrain.column(sample_point[0], sample_point[1]).height,
                            |water| water.bottom,
                        );
                        let step = (height - previous.bottom).abs();
                        if step > 3
                            && water.is_none()
                            && matches!(
                                terrain.block_at(sample_point[0], height, sample_point[1]),
                                Block::Limestone | Block::Calcite
                            )
                        {
                            let site = terrain.coastal_landmark().unwrap();
                            assert!((0..2).all(|axis| {
                                (site.bounds[0][axis]..=site.bounds[1][axis])
                                    .contains(&sample_point[axis])
                            }));
                            assert!(height >= SEA_LEVEL);
                            assert!(
                                (previous.bottom..=height).all(|y| terrain
                                    .block_at(sample_point[0], y, sample_point[1])
                                    .is_solid()),
                                "ungrounded coastal rock at {sample_point:?}"
                            );
                        } else {
                            max_shore_step = max_shore_step.max(step);
                            gradual_edges += 1;
                        }
                        shore_edges += 1;
                    }
                    run = 0;
                    pure_run = 0;
                    previous = None;
                }
            }
        }
        assert!(
            widest >= 100,
            "seed {seed}, largest ocean run={widest} blocks"
        );
        assert!(
            floors.len() >= 6,
            "seed {seed}, flat/fake seafloor levels={floors:?}"
        );
        assert!(
            shore_edges > 0 && gradual_edges > 0,
            "seed {seed}, no coast in bounded ocean transects"
        );
        assert!(
            max_shore_step <= 3,
            "seed {seed}, marine-to-shore bed wall={max_shore_step} blocks"
        );
        assert!(
            nonmarine_ocean.is_none(),
            "seed {seed}, Ocean-labelled water must be marine-dominated (salinity >= 0.98), including the outgoing plume: {nonmarine_ocean:?}"
        );
        assert!(
            pure_core_width >= 100,
            "seed {seed}, contiguous fully marine ocean core is only {pure_core_width} blocks wide"
        );
    }
}

#[test]
fn river_water_has_connected_mixed_salinity_mouth_and_local_downhill_current() {
    for seed in [Terrain::default().seed(), 42, 1, u64::MAX] {
        let terrain = Terrain::new(seed);
        let river = landmark(&terrain, WaterBiome::River);
        let root = [river.water[0], river.water[2]];
        let reach = connected_river(&terrain, root);
        assert!(
            reach.visited > 200 && reach.estuary && reach.ocean && reach.mixed_salinity > 20,
            "seed {seed}, disconnected/fresh-only river mouth: {reach:?}"
        );
        assert!(
            reach.max_surface - reach.min_surface > 1.0,
            "seed {seed}, river has no elevation fall: {reach:?}"
        );
        let mut downstream_checks = 0;
        for dz in -16..=16 {
            for dx in -16..=16 {
                let [x, z] = [root[0] + dx, root[1] + dz];
                let Some(water) = terrain
                    .water_column(x, z)
                    .filter(|water| water.biome == WaterBiome::River)
                else {
                    continue;
                };
                assert_eq!(water.salinity, 0.0);
                let flow = Vec3::from_array(water.flow).with_y(0.0);
                assert!(
                    flow.length() > 0.05,
                    "seed {seed}, stagnant river at ({x}, {z})"
                );
                if depth(water) < 2.0 {
                    continue;
                }
                let direction = flow.normalize();
                for distance in [0.25, 0.5, 0.75, 1.0, 1.25] {
                    let next_point = [
                        (x as f32 + 0.5 + direction.x * distance).floor() as i32,
                        (z as f32 + 0.5 + direction.z * distance).floor() as i32,
                    ];
                    let next = terrain.water_column(next_point[0], next_point[1]).unwrap_or_else(|| {
                        panic!("seed {seed}, river current points into dry land from ({x}, {z}) at distance {distance}, next={next_point:?}: {water:?}")
                    });
                    assert!(
                        next.surface <= water.surface + 0.8,
                        "seed {seed}, uphill local current ({x}, {z}) -> {next_point:?}: {water:?} -> {next:?}"
                    );
                    assert!(
                        next.surface.min(water.surface)
                            > (next.bottom.max(water.bottom) + 1) as f32,
                        "seed {seed}, current crosses a non-overlapping water column at {next_point:?}"
                    );
                }
                downstream_checks += 1;
            }
        }
        assert!(
            downstream_checks > 20,
            "seed {seed}, too little local river coverage"
        );
    }
}

#[test]
fn waterfall_is_a_supported_continuous_falling_column_connected_to_source_and_outlet() {
    for seed in seeds() {
        let terrain = Terrain::new(seed);
        let site = landmark(&terrain, WaterBiome::Waterfall);
        let ([x, z], profile) = waterfall_core(&terrain, site);
        let blocks = terrain.column_blocks(x, z);
        let first = profile.bottom + 1;
        let last = profile.surface.ceil() as i32 - 1;
        assert!(last - first + 1 >= 4);
        assert!(blocks[(profile.bottom - MIN_WORLD_Y) as usize].is_solid());
        for water_y in first..=last {
            assert_eq!(
                blocks[(water_y - MIN_WORLD_Y) as usize],
                Block::Water,
                "seed {seed}, waterfall gap at ({x}, {water_y}, {z})"
            );
            let actual = terrain.water_at(x, water_y, z).unwrap();
            assert_eq!(actual.biome, WaterBiome::Waterfall);
            assert!(actual.flow[1] < -0.05);
            assert_eq!(actual.salinity, 0.0);
        }
        let mut source = false;
        let mut outlet = false;
        for dz in -12..=12 {
            for dx in -12..=12 {
                let Some(neighbor) = terrain.water_column(x + dx, z + dz) else {
                    continue;
                };
                if neighbor.biome == WaterBiome::Waterfall {
                    continue;
                }
                if matches!(neighbor.biome, WaterBiome::Lake | WaterBiome::River) {
                    source |= neighbor.surface >= profile.surface - 0.8;
                    outlet |= neighbor.surface <= profile.surface - 4.0
                        && neighbor.surface > (profile.bottom + 1) as f32;
                }
            }
        }
        assert!(
            source && outlet,
            "seed {seed}, unsupported isolated curtain without nearby source/outlet: {profile:?}; source={source}, outlet={outlet}"
        );
        let reach = connected_river(&terrain, [x, z]);
        assert!(
            reach.visited > 200 && reach.estuary && reach.ocean,
            "seed {seed}, waterfall does not join the downstream water network: {reach:?}"
        );
    }
}

#[test]
fn retained_roofed_cave_pools_are_supported_calm_freshwater_and_unloaded_water_is_none() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let mut cave_pool_columns = 0;
        let mut resident = BTreeMap::new();
        for coord in cave_survey_coords(&terrain) {
            let chunk = Chunk::generate(&terrain, coord);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let mut recorded = false;
                    for y in MIN_WORLD_Y + 1..0 {
                        let Some(pool) = chunk
                            .water(x, y, z)
                            .filter(|water| water.biome == WaterBiome::CavePool)
                        else {
                            continue;
                        };
                        assert_eq!(chunk.block(x, y, z), Block::Water);
                        assert_eq!(pool.flow, [0.0; 3]);
                        assert_eq!(pool.salinity, 0.0);
                        assert_profile(pool);
                        assert!(
                            chunk.block(x, pool.bottom, z).is_solid(),
                            "seed {seed}, floating cave pool"
                        );
                        assert!(pool.surface <= 0.0);
                        let roof = (pool.surface.ceil() as i32..WORLD_HEIGHT)
                            .any(|above| chunk.block(x, above, z).is_opaque());
                        assert!(roof, "seed {seed}, claimed cave pool is not roofed");
                        if !recorded {
                            cave_pool_columns += 1;
                            let world = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                            assert_water_eq(
                                Some(pool),
                                terrain.water_at(world[0], world[1], world[2]),
                            );
                            recorded = true;
                        }
                    }
                }
            }
            resident.insert(coord, chunk);
        }
        assert!(
            cave_pool_columns > 8,
            "seed {seed}, retained pool columns={cave_pool_columns}"
        );
        let world = ResidentWorld {
            terrain,
            chunks: RefCell::new(resident),
        };
        let distant = landmark(&terrain, WaterBiome::Ocean).water;
        if !world.chunks.borrow().contains_key(&ChunkCoord::new(
            distant[0].div_euclid(CHUNK_SIZE),
            distant[2].div_euclid(CHUNK_SIZE),
        )) {
            assert!(world.water(distant[0], distant[1], distant[2]).is_none());
            assert!(
                terrain
                    .water_at(distant[0], distant[1], distant[2])
                    .is_some()
            );
        }
    }
}
