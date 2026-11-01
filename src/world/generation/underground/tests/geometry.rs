use super::*;

#[test]
fn masks_clip_bounds_and_cross_word_boundaries() {
    let mut mask = Mask::default();
    mask.range(i32::MIN, i32::MAX, true);
    for y in MIN_WORLD_Y..WORLD_HEIGHT {
        assert_eq!(mask.contains(y), y >= FIRST_CAVE_Y, "{y}");
    }
    mask.range(-2, 3, false);
    for y in -3..=4 {
        assert_eq!(mask.contains(y), !(-2..=3).contains(&y));
    }
    assert!(!mask.contains(MIN_WORLD_Y - 1));
    assert!(!mask.contains(WORLD_HEIGHT));
}

#[test]
fn tunnel_spans_are_roomy_connected_and_include_end_caps() {
    let feature = Feature::tunnel([0.5, -10.0, 0.5], [80.5, -40.0, 40.5], 3.5, 3.8);
    for step in 0..=80 {
        let x = f64::from(step) + 0.5;
        let z = f64::from(step) * 0.5 + 0.5;
        let (first, last) = feature.span(x, z).unwrap();
        assert!(last - first >= 6);
        assert!((first..=last).contains(&((-10.0 - f64::from(step) * 0.375).floor() as i32)));
    }
    assert!(feature.span(-2.5, 0.5).is_some());
    assert!(feature.span(84.5, 40.5).is_none());
}

#[test]
fn tight_feature_windows_include_maximum_ore_reach_and_basin_support() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let query = Bounds {
            min_x: -144.0,
            max_x: 144.0,
            min_z: -144.0,
            max_z: 144.0,
            min_y: f64::from(MIN_WORLD_Y),
            max_y: 14.0,
        };
        let region = UndergroundRegion::new(&terrain, query);
        let mut samples = 0;
        for vein in &region.veins {
            for (x, z) in [
                (
                    vein.feature.bounds.min_x.ceil() as i32,
                    vein.feature.bounds.min_z.ceil() as i32,
                ),
                (
                    vein.feature.bounds.max_x.floor() as i32,
                    vein.feature.bounds.max_z.floor() as i32,
                ),
                (
                    ((vein.feature.bounds.min_x + vein.feature.bounds.max_x) * 0.5).floor() as i32,
                    ((vein.feature.bounds.min_z + vein.feature.bounds.max_z) * 0.5).floor() as i32,
                ),
            ] {
                let Some((first, last)) = vein.feature.span(f64::from(x) + 0.5, f64::from(z) + 0.5)
                else {
                    continue;
                };
                let surface = terrain.column(x, z);
                let cached = region.column(x, z, surface);
                for y in first.max(FIRST_CAVE_Y)..=last.min(surface.height - 4) {
                    assert_eq!(
                        cached.block(y, Block::Stone),
                        terrain.block_at(x, y, z),
                        "seed {seed}, vein fringe ({x}, {y}, {z})"
                    );
                    samples += 1;
                }
            }
        }
        for pool in region.pools {
            let x = pool.x.floor() as i32;
            let z = pool.z.floor() as i32;
            assert!(terrain.block_at(x, pool.level - 3, z).is_solid());
            for y in pool.level - 2..pool.level {
                assert_eq!(terrain.block_at(x, y, z), pool.liquid);
            }
        }
        assert!(samples > 500);
    }
}

#[test]
fn ordinary_network_has_wide_chambers_and_gentle_layer_links() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let region = UndergroundRegion::chunk(&terrain, 0, 0);
        assert!(region.caves.len() + region.routes.len() < 160);
        for cell_z in -2..=2 {
            for cell_x in -2..=2 {
                let upper = node(seed, cell_x, cell_z, false);
                let lower = node(seed, cell_x + 1, cell_z + 1, true);
                let horizontal = ((upper.center[0] - lower.center[0]).powi(2)
                    + (upper.center[2] - lower.center[2]).powi(2))
                .sqrt();
                assert!((upper.center[1] - lower.center[1]).abs() / horizontal < 0.75);
                assert!(upper.radius[0] * 2.0 >= 36.0);
                assert!(lower.center[1] - lower.radius[1] >= f64::from(FIRST_CAVE_Y));
            }
        }
    }
}

#[test]
fn room_families_have_distinct_profiles_and_bounded_warp() {
    let mut seen = [false; 4];
    let mut flat_columns = [0; 4];
    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        for cell_z in -2..=2 {
            for cell_x in -2..=2 {
                let node = node(seed, cell_x, cell_z, false);
                let room = node.room();
                let family = match node.landform {
                    CaveLandform::Gallery => 0,
                    CaveLandform::Rift => 1,
                    CaveLandform::Cathedral => 2,
                    CaveLandform::Labyrinth => 3,
                };
                seen[family] = true;
                let mut center_heights = Vec::new();
                for along in -10..=10 {
                    let point = node.point(f64::from(along), 0.0, node.floor);
                    let (first, last) = room.span(point[0], point[2]).unwrap();
                    flat_columns[family] += usize::from((first - node.floor as i32).abs() <= 1);
                    center_heights.push(last - first + 1);
                }
                match node.landform {
                    CaveLandform::Gallery | CaveLandform::Labyrinth => {
                        assert!(
                            center_heights
                                .iter()
                                .all(|&height| (5..=17).contains(&height))
                        );
                    }
                    CaveLandform::Rift => {
                        assert!(center_heights.iter().all(|&height| height >= 23));
                        assert!(node.radius[2] * 2.0 <= 18.0);
                    }
                    CaveLandform::Cathedral => {
                        assert!(center_heights.iter().all(|&height| height >= 19));
                        assert!(node.radius[2] * 2.0 >= 44.0);
                    }
                }
                let bounds = room.bounds();
                for z in
                    (bounds.min_z.floor() as i32 - 4..=bounds.max_z.ceil() as i32 + 4).step_by(3)
                {
                    for x in (bounds.min_x.floor() as i32 - 4..=bounds.max_x.ceil() as i32 + 4)
                        .step_by(3)
                    {
                        if let Some((first, last)) =
                            room.span(f64::from(x) + 0.5, f64::from(z) + 0.5)
                        {
                            assert!(bounds.contains_column(f64::from(x) + 0.5, f64::from(z) + 0.5));
                            assert!(f64::from(first) >= bounds.min_y - 1.0);
                            assert!(f64::from(last) <= bounds.max_y + 1.0);
                        }
                    }
                }
            }
        }
    }
    assert!(seen.into_iter().all(|seen| seen));
    assert!(flat_columns.into_iter().all(|count| count > 100));
}

#[test]
fn network_retains_pillars_bridges_and_separate_bedding_levels() {
    let terrain = Terrain::default();
    let query = Bounds {
        min_x: -240.0,
        max_x: 240.0,
        min_z: -240.0,
        max_z: 240.0,
        min_y: f64::from(MIN_WORLD_Y),
        max_y: f64::from(WORLD_HEIGHT),
    };
    let region = UndergroundRegion::new(&terrain, query);
    let mut retained_pillars = 0;
    for rock in &region.retained {
        if let RetainedRock::Pillar { room, x, z, .. } = *rock {
            let x = x.floor() as i64;
            let z = z.floor() as i64;
            let Some((first, last)) = room.span(x as f64 + 0.5, z as f64 + 0.5) else {
                continue;
            };
            let mask = region.void_mask(x, z);
            if last - first > 16 && (first..=last).all(|y| !mask.contains(y)) {
                retained_pillars += 1;
            }
        }
    }
    let mut multiple_levels = 0;
    let mut internal_rock = 0;
    for z in (-220..=220).step_by(4) {
        for x in (-220..=220).step_by(4) {
            let mask = region.void_mask(x, z);
            let upper_intervals = (-30..=20)
                .filter(|&y| mask.contains(y) && !mask.contains(y - 1))
                .count();
            multiple_levels += usize::from(upper_intervals >= 2);
            let bridge = (-27..=12).any(|y| {
                mask.contains(y - 2)
                    && !mask.contains(y)
                    && !mask.contains(y + 1)
                    && mask.contains(y + 3)
            });
            internal_rock += usize::from(bridge);
        }
    }
    assert!(
        retained_pillars >= 20,
        "only {retained_pillars} full-height pillars"
    );
    assert!(
        multiple_levels >= 150,
        "only {multiple_levels} layered cave columns"
    );
    assert!(
        internal_rock >= 80,
        "only {internal_rock} rock bridges/shelves"
    );
}

#[test]
fn connectors_wind_vary_cross_section_and_include_loop_branches() {
    let terrain = Terrain::default();
    let query = Bounds {
        min_x: -200.0,
        max_x: 300.0,
        min_z: -200.0,
        max_z: 300.0,
        min_y: f64::from(MIN_WORLD_Y),
        max_y: f64::from(WORLD_HEIGHT),
    };
    for cell in -2..=2 {
        let start = node(terrain.seed(), cell, 0, false);
        let end = node(terrain.seed(), cell + 1, 0, false);
        let mut region = UndergroundRegion {
            seed: terrain.seed(),
            caves: Vec::new(),
            routes: Vec::new(),
            retained: Vec::new(),
            pools: Vec::new(),
            veins: Vec::new(),
            growths: Vec::new(),
        };
        region.connect(start, end, 3.6, 7.0, query);
        assert!(region.routes.len() >= 6);
        let direct = ((end.center[0] - start.center[0]).powi(2)
            + (end.center[2] - start.center[2]).powi(2))
        .sqrt();
        let mut length = 0.0;
        let mut lateral: f64 = 0.0;
        let mut widths = Vec::new();
        for route in region.routes.iter().take(6) {
            let Shape::Passage {
                start: a,
                end: b,
                width,
                height,
                ..
            } = route.shape
            else {
                unreachable!()
            };
            length += ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            lateral = lateral.max(
                segment_projection(start.anchor(), end.anchor(), b[0], b[2])
                    .1
                    .sqrt(),
            );
            widths.push(width);
            assert!((6.0..=11.0).contains(&height));
        }
        assert!(
            length > direct * 1.015,
            "straight connector: {length} / {direct}"
        );
        assert!(lateral > 4.0, "only {lateral} blocks of lateral bend");
        assert!(
            widths.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - widths.iter().copied().fold(f64::INFINITY, f64::min)
                > 0.5
        );
        if start.hash & 1 == 0 {
            assert_eq!(region.routes.len(), 8);
        }
    }
}

#[test]
fn main_route_centers_keep_supported_standing_clearance() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let query = Bounds {
            min_x: -80.0,
            max_x: 160.0,
            min_z: -80.0,
            max_z: 160.0,
            min_y: f64::from(MIN_WORLD_Y),
            max_y: f64::from(WORLD_HEIGHT),
        };
        let region = UndergroundRegion::new(&terrain, query);
        let mut chunks = std::collections::BTreeMap::new();
        for route in &region.routes {
            let Shape::Passage { start, end, .. } = route.shape else {
                unreachable!()
            };
            for step in 0..=20 {
                let t = f64::from(step) / 20.0;
                let x = (start[0] + (end[0] - start[0]) * t).floor() as i32;
                let z = (start[2] + (end[2] - start[2]) * t).floor() as i32;
                if !query.contains_column(f64::from(x), f64::from(z)) {
                    continue;
                }
                let y = (start[1] + (end[1] - start[1]) * t).round() as i32;
                let near_floor = (y - 4..=y + 5).any(|y| {
                    cached_block(&terrain, &mut chunks, x, y - 1, z).is_solid()
                        && (0..=1).all(|dy| {
                            cached_block(&terrain, &mut chunks, x, y + dy, z) == Block::Air
                        })
                });
                assert!(
                    near_floor,
                    "seed {seed}: route {start:?} -> {end:?}, sample ({x},{y},{z}); blocks={:?}",
                    (y - 5..=y + 3)
                        .map(|y| (y, cached_block(&terrain, &mut chunks, x, y, z)))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
