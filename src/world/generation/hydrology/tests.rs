use super::*;
use crate::world::{CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, WORLD_DEPTH};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const SEEDS: [u64; 5] = [0, 1, 42, 0x5255_5354_4352_4146, u64::MAX];

fn point(catchment: &Catchment, local: [f64; 2]) -> [i32; 2] {
    catchment.world(local).map(|value| value.floor() as i32)
}

#[test]
fn local_fractional_profiles_have_unambiguous_occupancy_and_depth() {
    for (bottom, surface, expected) in [
        (30, 34.25, vec![31, 32, 33, 34]),
        (-12, 18.0, (-11..18).collect()),
        (-34, -31.5, vec![-33, -32]),
    ] {
        let water = WaterColumn {
            bottom,
            surface,
            flow: [0.0; 3],
            biome: WaterBiome::Lake,
            salinity: 0.0,
        };
        let actual: Vec<_> = (MIN_WORLD_Y..WORLD_HEIGHT)
            .filter(|&y| water.contains_y(y))
            .collect();
        assert_eq!(actual, expected);
        assert!(water.depth() > 0.0);
        assert!(!water.contains_y(i32::MIN));
        assert!(!water.contains_y(i32::MAX));
    }
    assert_eq!(WaterBiome::River.name(), "Freshwater River");
    assert_eq!(WaterBiome::Estuary.name(), "Brackish Estuary");
    assert_eq!(WaterBiome::CavePool.name(), "Cave Pool");
}

#[test]
fn every_source_drains_downstream_through_a_real_confluence_to_the_sea() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        let main: Vec<_> = catchment
            .rivers
            .iter()
            .filter(|river| !river.tributary)
            .collect();
        let tributary: Vec<_> = catchment
            .rivers
            .iter()
            .filter(|river| river.tributary)
            .collect();
        assert_eq!(main[0].start.position, catchment.lakes[0].center);
        assert_eq!(tributary[0].start.position, catchment.lakes[1].center);
        assert_eq!(
            tributary.last().unwrap().end.position,
            main[3].start.position
        );
        assert_eq!(tributary.last().unwrap().end.surface, main[3].start.surface);
        for route in [&main, &tributary] {
            for pair in route.windows(2) {
                assert_eq!(pair[0].end.position, pair[1].start.position);
                assert_eq!(pair[0].end.surface, pair[1].start.surface);
            }
            let mut previous = f32::INFINITY;
            for segment in route {
                assert!(segment.end.surface <= segment.start.surface);
                let length = (segment.end.position[0] - segment.start.position[0])
                    .hypot(segment.end.position[1] - segment.start.position[1]);
                for step in 0..=length.ceil() as usize {
                    let t = step as f64 / length.ceil();
                    let local = [
                        mix(segment.start.position[0], segment.end.position[0], t),
                        mix(segment.start.position[1], segment.end.position[1], t),
                    ];
                    let [x, z] = point(&catchment, local);
                    let water = terrain.water_column(x, z).unwrap_or_else(|| {
                        panic!("seed {seed}: dry drainage path at {local:?}, ({x},{z})")
                    });
                    assert!(
                        water.surface <= previous + 0.8,
                        "seed {seed}: uphill river at {local:?}, {previous} -> {water:?}"
                    );
                    previous = water.surface;
                    assert!(water.flow.iter().all(|component| component.is_finite()));
                    assert!(water.flow[1] <= 0.0 && water.flow[1] >= -4.0);
                    assert_eq!(terrain.block_at(x, water.bottom + 1, z), Block::Water);
                }
            }
        }
        let mouth = main.last().unwrap().end;
        assert_eq!(mouth.surface, f64::from(SEA_LEVEL));
        assert!(catchment.ocean.radius_at(mouth.position) < 0.6);
        let [x, z] = point(&catchment, mouth.position);
        assert_eq!(terrain.water_column(x, z).unwrap().biome, WaterBiome::Ocean);
    }
}

#[test]
fn marine_basin_is_broad_deep_connected_and_has_shelves_and_an_island() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        let mut wet = BTreeSet::new();
        let mut deep = 0;
        let mut shelf = 0;
        let mut sediments = [0; Block::COUNT];
        for v in (-152..=230).step_by(4) {
            for u in (278..=628).step_by(4) {
                let [x, z] = point(&catchment, [f64::from(u), f64::from(v)]);
                if let Some(water) = terrain.water_column(x, z)
                    && matches!(water.biome, WaterBiome::Ocean | WaterBiome::Estuary)
                {
                    wet.insert((u, v));
                    deep += usize::from(water.depth() >= 15.0);
                    shelf += usize::from(water.depth() <= 6.0);
                    let region = HydrologyRegion::columns(seed, x, x, z, z);
                    sediments[region.column(&terrain, x, z).sediment.unwrap() as usize] += 1;
                }
            }
        }
        assert!(wet.len() >= 4_000, "seed {seed}: marine area {}", wet.len());
        assert!(deep >= 1_200, "seed {seed}: deep area {deep}");
        assert!(shelf >= 500, "seed {seed}: shelf area {shelf}");
        assert!(sediments[Block::Sand as usize] >= 128);
        assert!(sediments[Block::Gravel as usize] >= 8);
        let first = *wet
            .iter()
            .min_by_key(|&&(u, v)| (u - 483_i32).pow(2) + (v - 64_i32).pow(2))
            .unwrap();
        let mut visited = BTreeSet::from([first]);
        let mut queue = VecDeque::from([first]);
        while let Some((x, z)) = queue.pop_front() {
            for (dx, dz) in [(-4, 0), (4, 0), (0, -4), (0, 4)] {
                let next = (x + dx, z + dz);
                if wet.contains(&next) && visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        assert!(
            visited.len() * 100 >= wet.len() * 99,
            "seed {seed}: disjoint sea ({}/{})",
            visited.len(),
            wet.len()
        );
        let [x, z] = point(&catchment, catchment.island.center);
        assert!(terrain.water_column(x, z).is_none());
        assert!(terrain.column(x, z).height >= 30);
        let [x, z] = point(&catchment, catchment.ocean.center);
        assert!(terrain.water_column(x, z).unwrap().depth() >= 25.0);
    }
}

#[test]
fn lakes_are_fresh_at_distinct_elevations_and_mouth_mixing_increases_to_marine() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        let mut elevations = [0.0; 2];
        for (index, lake) in catchment.lakes.iter().enumerate() {
            let [x, z] = point(&catchment, lake.center);
            let water = terrain.water_column(x, z).unwrap();
            assert_eq!(water.biome, WaterBiome::Lake);
            assert_eq!(water.surface, lake.surface as f32);
            assert_eq!(water.salinity, 0.0);
            assert_eq!(water.flow, [0.0; 3]);
            assert!(water.depth() >= 6.0);
            assert_eq!(terrain.block_at(x, water.bottom, z), Block::Clay);
            elevations[index] = water.surface;
        }
        assert!(elevations[1] - elevations[0] >= 10.0);
        let mut previous = 0.0;
        let mut mixed = 0;
        for segment in catchment.rivers.iter().filter(|segment| !segment.tributary) {
            for step in 0..=8 {
                let t = f64::from(step) / 8.0;
                let local = [
                    mix(segment.start.position[0], segment.end.position[0], t),
                    mix(segment.start.position[1], segment.end.position[1], t),
                ];
                let [x, z] = point(&catchment, local);
                let water = terrain.water_column(x, z).unwrap();
                assert!((0.0..=1.0).contains(&water.salinity));
                assert!(water.salinity >= previous - 0.02);
                mixed += usize::from(water.salinity > 0.1 && water.salinity < 0.9);
                previous = water.salinity;
            }
        }
        assert!(mixed >= 8);
        assert!(previous > 0.99);
    }
}

#[test]
fn landmarks_are_real_clear_nearby_deep_enough_and_currents_are_not_cosmetic() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        for biome in WaterBiome::ALL
            .into_iter()
            .filter(|biome| *biome != WaterBiome::CavePool)
        {
            let landmark = terrain
                .water_landmark(biome)
                .unwrap_or_else(|| panic!("seed {seed}: {biome:?}"));
            assert_eq!(landmark.biome, biome);
            let [x, y, z] = landmark.water;
            let water = terrain.water_at(x, y, z).unwrap();
            assert_eq!(water.biome, biome);
            assert_eq!(terrain.block_at(x, y, z), Block::Water);
            assert!(
                (landmark.position[0] - spawn[0]).hypot(landmark.position[2] - spawn[2]) <= 640.0
            );
            let distance = (landmark.position[0] - landmark.target[0])
                .hypot(landmark.position[2] - landmark.target[2]);
            assert!((8.0..=30.0).contains(&distance), "{landmark:?}");
            let [camera_x, feet, camera_z] = landmark.position.map(|value| value.floor() as i32);
            for dy in 0..=3 {
                assert_eq!(terrain.block_at(camera_x, feet + dy, camera_z), Block::Air);
            }
            match biome {
                WaterBiome::Lake | WaterBiome::Ocean => {
                    assert!(water.depth() >= 4.0);
                    assert!(water.flow[0].hypot(water.flow[2]) <= 0.2);
                }
                WaterBiome::River => assert!(water.flow[0].hypot(water.flow[2]) >= 1.0),
                WaterBiome::Waterfall => assert_eq!(water.flow[1], -4.0),
                WaterBiome::Estuary => assert!(water.salinity > 0.0 && water.salinity < 1.0),
                WaterBiome::CavePool => unreachable!(),
            }
            if seed == 42 {
                println!("{biome:?}: {landmark:?}, {water:?}");
            }
        }
        assert!(terrain.water_landmark(WaterBiome::CavePool).is_none());
    }
}

#[test]
fn surface_water_has_four_solid_sealing_layers_and_no_tree_roots() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        let mut samples = 0;
        for v in (-268..=230).step_by(11) {
            for u in (168..=632).step_by(11) {
                let [x, z] = point(&catchment, [f64::from(u), f64::from(v)]);
                let Some(water) = terrain.water_column(x, z) else {
                    continue;
                };
                for y in water.bottom - 3..=water.bottom {
                    assert!(
                        terrain.block_at(x, y, z).is_solid(),
                        "seed {seed}: leaking ({x},{y},{z}) {water:?}"
                    );
                }
                for y in water.bottom + 1..water.surface.ceil() as i32 {
                    assert_eq!(terrain.block_at(x, y, z), Block::Water);
                }
                assert_eq!(terrain.water_at(x, water.bottom, z), None);
                assert_eq!(terrain.water_at(x, water.surface.ceil() as i32, z), None);
                samples += 1;
            }
        }
        assert!(
            samples >= 500,
            "seed {seed}: only {samples} sealed water sites"
        );
        let spawn = terrain.spawn_position().map(|value| value.floor() as i32);
        assert!(terrain.water_column(spawn[0], spawn[2]).is_none());
    }
}

#[test]
fn basin_halos_are_dry_and_wet_dry_neighbors_cannot_leave_hanging_water_walls() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        for lake in catchment.lakes {
            for angle in 0..32 {
                let angle = f64::from(angle) * std::f64::consts::TAU / 32.0;
                for radius in [1.15, 1.4, 1.65] {
                    let local = [
                        lake.center[0] + angle.cos() * lake.radius[0] * radius,
                        lake.center[1] + angle.sin() * lake.radius[1] * radius,
                    ];
                    let [x, z] = point(&catchment, local);
                    assert!(
                        terrain
                            .water_column(x, z)
                            .is_none_or(|water| water.biome != WaterBiome::Lake),
                        "seed {seed}: wet lake halo {local:?}"
                    );
                }
            }
        }
        let mut shorelines = 0;
        for v in (-315..=246).step_by(2) {
            for u in (146..=648).step_by(2) {
                let [x, z] = point(&catchment, [f64::from(u), f64::from(v)]);
                let Some(water) = terrain.water_column(x, z) else {
                    continue;
                };
                if water.biome == WaterBiome::Waterfall {
                    continue;
                }
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    if terrain.water_column(x + dx, z + dz).is_some() {
                        continue;
                    }
                    let bank = terrain.column(x + dx, z + dz);
                    assert!(
                        water.surface - (bank.height + 1) as f32 <= 1.0,
                        "seed {seed}: hanging water ({x},{z}) {water:?}, dry bank ({},{}) {bank:?}",
                        x + dx,
                        z + dz
                    );
                    shorelines += 1;
                }
            }
        }
        assert!(shorelines >= 100);
    }
}

#[test]
fn whole_water_chunks_columns_and_samples_agree_independent_of_order() {
    for seed in [42, u64::MAX] {
        let terrain = Terrain::new(seed);
        let mut coords = BTreeSet::new();
        for biome in WaterBiome::ALL
            .into_iter()
            .filter(|biome| *biome != WaterBiome::CavePool)
        {
            let site = terrain.water_landmark(biome).unwrap();
            let coord = ChunkCoord::from_world(site.water[0] as f32, site.water[2] as f32);
            coords.insert(coord);
        }
        coords.insert(ChunkCoord::new(-1, -1));
        let first: BTreeMap<_, _> = coords
            .iter()
            .map(|&coord| (coord, Chunk::generate(&terrain, coord)))
            .collect();
        for coord in coords.into_iter().rev() {
            let chunk = Chunk::generate(&terrain, coord);
            let old = &first[&coord];
            let positions: Vec<_> = (0..CHUNK_SIZE)
                .flat_map(|z| {
                    (0..CHUNK_SIZE)
                        .map(move |x| [coord.x * CHUNK_SIZE + x, coord.z * CHUNK_SIZE + z])
                })
                .collect();
            for (index, (position, blocks)) in positions
                .iter()
                .zip(terrain.column_block_batch(&positions))
                .enumerate()
            {
                let [x, z] = *position;
                let local_x = index as i32 % CHUNK_SIZE;
                let local_z = index as i32 / CHUNK_SIZE;
                for (offset, block) in blocks.into_iter().enumerate() {
                    let y = MIN_WORLD_Y + offset as i32;
                    assert_eq!(
                        block,
                        chunk.block(local_x, y, local_z),
                        "seed {seed}: ({x},{y},{z})"
                    );
                    assert_eq!(block, old.block(local_x, y, local_z));
                    assert_eq!(block, terrain.block_at(x, y, z));
                    let water = chunk.water(local_x, y, local_z);
                    assert_eq!(water.is_some(), block == Block::Water);
                    assert_eq!(water, old.water(local_x, y, local_z));
                    if block == Block::Water {
                        assert_eq!(
                            water,
                            terrain.water_at(x, y, z),
                            "seed {seed}: water ({x},{y},{z})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ordinary_shores_and_aprons_are_bounded_outside_designed_rock_lips_and_coasts() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        let mut ordinary = 0;
        let mut coastal = 0;
        let mut lip_steps = 0;
        let mut dry_terraces = 0;
        for v in (-315..=246).step_by(3) {
            for u in (146..=648).step_by(3) {
                let [x, z] = point(&catchment, [f64::from(u), f64::from(v)]);
                let region = HydrologyRegion::columns(seed, x, x + 1, z, z + 1);
                let profile = region.column(&terrain, x, z);
                for (dx, dz) in [(1, 0), (0, 1)] {
                    let next = region.column(&terrain, x + dx, z + dz);
                    let change = (profile.column.height - next.column.height).abs();
                    if profile.coastal.is_some() || next.coastal.is_some() {
                        coastal += 1;
                        for coastal in [profile, next] {
                            if let Some(water) = coastal.water {
                                assert_eq!(water.bottom, coastal.column.height);
                                assert!(water.depth() > 0.0);
                            }
                        }
                    } else if profile.rock_lip || next.rock_lip {
                        assert!(change <= 14);
                        lip_steps += usize::from(change > 2);
                    } else {
                        let dry = profile.water.is_none() && next.water.is_none();
                        dry_terraces += usize::from(dry && change > 2);
                        assert!(
                            change <= if dry { 3 } else { 2 },
                            "seed {seed}: ordinary bank {change} at ({x},{z}) local({u},{v}) height {} -> {}, water {:?} -> {:?}",
                            profile.column.height,
                            next.column.height,
                            profile.water,
                            next.water
                        );
                        ordinary += 1;
                    }
                }
            }
        }
        assert!(ordinary > 50_000 && ordinary + coastal > 60_000);
        assert!(
            dry_terraces * 1_000 <= ordinary,
            "seed {seed}: too many steep dry terraces ({dry_terraces}/{ordinary})"
        );
        assert!(lip_steps > 0);
    }
}

#[test]
fn region_projection_is_bounded_repeatable_and_matches_extreme_direct_samples() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        for (x, z) in [
            (i32::MIN, i32::MAX - 16),
            (i32::MAX - 16, i32::MIN),
            (-1_415, -1_415),
            (-16, 0),
        ] {
            let region = HydrologyRegion::columns(seed, x, x + 15, z, z + 15);
            assert!(region.catchments.len() <= 4);
            for dz in 0..16 {
                for dx in 0..16 {
                    let profile = region.column(&terrain, x + dx, z + dz);
                    assert_eq!(profile.column, terrain.column(x + dx, z + dz));
                    assert_eq!(profile.water, terrain.water_column(x + dx, z + dz));
                    if let Some(water) = profile.water {
                        assert!(water.surface.is_finite());
                        assert!(water.surface < WORLD_HEIGHT as f32);
                        assert!((0.0..=1.0).contains(&water.salinity));
                    }
                }
            }
        }
    }
    assert_eq!(WORLD_DEPTH, 144);
}

#[test]
fn coastal_roots_repeat_across_negative_catchments_without_changing_unmodified_profiles() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        for (cell_x, cell_z) in [(0, 0), (-1, 0), (0, -1), (-1, -1)] {
            let catchment = Catchment::new(seed, cell_x, cell_z);
            let site = catchment.coast.landmark(&terrain).unwrap_or_else(|| {
                panic!("seed {seed}: no coast view in catchment ({cell_x},{cell_z})")
            });
            for body in site.stacks {
                let [x, z] = body.center;
                let region = HydrologyRegion::columns(seed, x - 1, x + 1, z - 1, z + 1);
                assert!(region.column(&terrain, x, z).coastal.is_some());
                assert!(terrain.block_at(x, body.top, z).is_solid());
                assert_eq!(terrain.block_at(x, body.top + 1, z), Block::Air);
                assert_eq!(terrain.water_at(x, SEA_LEVEL - 1, z), None);
                let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
                let chunk = Chunk::generate(&terrain, coord);
                for y in MIN_WORLD_Y..WORLD_HEIGHT {
                    assert_eq!(
                        chunk.block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)),
                        terrain.block_at(x, y, z)
                    );
                }
            }
            for point in [[0.0, 0.0], [295.0, -146.0], [483.0, 64.0], [220.0, -180.0]] {
                let [x, z] = catchment.world(point).map(|value| value.floor() as i32);
                let region = HydrologyRegion::columns(seed, x, x, z, z);
                let unmodified = region.filtered_column(&terrain, x, z);
                let sampled = region.column(&terrain, x, z);
                assert!(sampled.coastal.is_none());
                assert_eq!(sampled.column, unmodified.column);
                assert_eq!(sampled.water, unmodified.water);
                assert_eq!(sampled.sediment, unmodified.sediment);
                assert_eq!(sampled.seal_from, unmodified.seal_from);
            }
        }
    }
}

#[test]
#[ignore = "CPU generation timing is diagnostic, not a timing-sensitive assertion"]
fn generation_profile() {
    let terrain = Terrain::new(42);
    for biome in WaterBiome::ALL
        .into_iter()
        .filter(|biome| *biome != WaterBiome::CavePool)
    {
        let site = terrain.water_landmark(biome).unwrap();
        let center = ChunkCoord::from_world(site.water[0] as f32, site.water[2] as f32);
        let started = std::time::Instant::now();
        for repeat in 0..8 {
            for dz in -2..=2 {
                for dx in -2..=2 {
                    std::hint::black_box(Chunk::generate(
                        &terrain,
                        ChunkCoord::new(center.x + dx, center.z + dz),
                    ));
                }
            }
            std::hint::black_box(repeat);
        }
        println!(
            "{biome:?}: {:.3} ms/chunk",
            started.elapsed().as_secs_f64() * 1_000.0 / 200.0
        );
    }
}
