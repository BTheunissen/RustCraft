use super::*;

#[test]
fn climate_is_three_dimensional_coherent_and_seeded() {
    let terrain = Terrain::default();
    let mut seen = [false; 3];
    let mut depth_changes = 0;
    let mut seed_changes = 0;
    for z in (-160..=160).step_by(8) {
        for x in (-160..=160).step_by(8) {
            let biome = cave_biome(terrain.seed(), x, -15, z);
            seen[match biome {
                CaveBiome::Rocky => 0,
                CaveBiome::Lush => 1,
                CaveBiome::Crystal => 2,
            }] = true;
            assert_eq!(biome, cave_biome(terrain.seed(), x, -15, z));
            depth_changes += usize::from(biome != cave_biome(terrain.seed(), x, -50, z));
            seed_changes +=
                usize::from(biome != cave_biome(terrain.seed().wrapping_add(1), x, -15, z));
        }
    }
    assert!(seen.into_iter().all(|seen| seen));
    assert!(depth_changes > 100);
    assert!(seed_changes > 100);
    assert_eq!(CaveBiome::Rocky.name(), "Rocky Caves");
    assert_eq!(CaveBiome::Lush.name(), "Lush Caves");
    assert_eq!(CaveBiome::Crystal.name(), "Crystal Caves");
}

#[test]
fn geological_beds_tilt_fold_and_change_the_deep_host_regionally() {
    let terrain = Terrain::default();
    let mut same = 0;
    let mut compared = 0;
    let mut limestone_edges = std::collections::BTreeSet::new();
    let mut boundaries = Vec::new();
    let mut seen = [false; Block::COUNT];
    let mut seed_changes = 0;
    for z in (-192..=192).step_by(8) {
        for x in (-192..=192).step_by(8) {
            let geology = Geology::new(terrain.seed(), x, z);
            boundaries.push(geology.deep_boundary);
            for y in (-58..=8).step_by(2) {
                let rock = geology.host(y);
                seen[rock as usize] = true;
                compared += 1;
                same += usize::from(rock == Geology::new(terrain.seed(), x + 1, z).host(y));
                seed_changes += usize::from(rock != Geology::new(42, x, z).host(y));
            }
            if let Some(y) = (-10..=20).find(|&y| {
                geology.host(y) == Block::Limestone && geology.host(y - 1) != Block::Limestone
            }) {
                limestone_edges.insert(y);
            }
        }
    }
    assert!(
        same * 100 > compared * 93,
        "adjacent bed coherence: {same}/{compared}"
    );
    assert!(seed_changes > compared / 8);
    assert!(limestone_edges.len() > 12);
    assert!(
        boundaries.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - boundaries.iter().copied().fold(f64::INFINITY, f64::min)
            > 10.0
    );
    for rock in [
        Block::Stone,
        Block::Limestone,
        Block::Slate,
        Block::DeepStone,
        Block::Basalt,
    ] {
        assert!(seen[rock as usize], "missing {rock:?}");
    }
}

#[test]
fn growths_are_rooted_tapered_and_keep_routes_clear() {
    let terrain = Terrain::default();
    let query = Bounds {
        min_x: -192.0,
        max_x: 192.0,
        min_z: -192.0,
        max_z: 192.0,
        min_y: f64::from(MIN_WORLD_Y),
        max_y: f64::from(WORLD_HEIGHT),
    };
    let region = UndergroundRegion::new(&terrain, query);
    let mut chunks = std::collections::BTreeMap::new();
    let mut crystal = 0;
    let mut calcite = 0;
    let mut columns = 0;
    let mut ceiling = 0;
    let mut actual_growth = 0;
    for growth in &region.growths {
        let [x, y, z] = growth.root.map(|value| value.floor() as i32);
        assert!(
            cached_block(&terrain, &mut chunks, x, y, z).is_solid(),
            "unrooted {:?} at {:?}",
            growth.block,
            growth.root
        );
        crystal += usize::from(growth.block == Block::Crystal);
        calcite += usize::from(growth.block == Block::Calcite);
        columns += usize::from(growth.column);
        ceiling += usize::from(growth.direction[1] < 0.0);
        if !growth.column {
            assert!(growth.contains(
                growth.root[0] + growth.radius * 0.7,
                growth.root[1],
                growth.root[2]
            ));
            assert!(growth.radius_at(0.9) < growth.radius_at(0.1) * 0.25);
        }
        let first = (growth.bounds().min_y.floor() as i32).max(FIRST_CAVE_Y);
        let last = (growth.bounds().max_y.ceil() as i32).min(WORLD_HEIGHT - 1);
        for y in first..=last {
            let block = cached_block(&terrain, &mut chunks, x, y, z);
            let projection = region.projection(i64::from(x), i64::from(z));
            if block == growth.block && projection.void.contains(y) {
                actual_growth += 1;
                assert!(!projection.protected.contains(y));
            }
        }
    }
    assert!(crystal >= 8, "only {crystal} crystal spears");
    assert!(calcite >= 8, "only {calcite} calcite formations");
    assert!(columns >= 1, "no supported calcite column");
    assert!(ceiling >= 8);
    assert!(
        actual_growth >= 40,
        "only {actual_growth} actual air-space growth blocks"
    );
}

#[test]
fn calcite_roots_are_present_near_the_landform_diagnostics() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let query = Bounds {
            min_x: -192.0,
            max_x: 192.0,
            min_z: -192.0,
            max_z: 192.0,
            min_y: f64::from(MIN_WORLD_Y),
            max_y: f64::from(WORLD_HEIGHT),
        };
        let region = UndergroundRegion::new(&terrain, query);
        let forms = [
            CaveLandform::Gallery,
            CaveLandform::Rift,
            CaveLandform::Cathedral,
            CaveLandform::Labyrinth,
        ];
        let positions = forms.map(|form| terrain.cave_landmark(form).unwrap().position);
        let near = region
            .growths
            .iter()
            .filter(|growth| {
                growth.block == Block::Calcite
                    && positions.iter().any(|position| {
                        (growth.root[0] - f64::from(position[0])).abs() <= 20.0
                            && (growth.root[2] - f64::from(position[2])).abs() <= 20.0
                    })
            })
            .collect::<Vec<_>>();
        assert!(
            near.len() >= 4,
            "seed {seed}: only {} nearby rooted calcite formations",
            near.len()
        );
    }
}

#[test]
fn slanted_voxel_spears_have_no_detached_tip_components() {
    use std::collections::{BTreeSet, VecDeque};

    for root in [[0.5, -20.5, 0.5], [-16.5, -34.5, 15.5]] {
        for direction in [[-0.44, 1.0, 0.42], [0.3, -1.0, -0.28], [0.0, 1.0, 0.0]] {
            for length in [2.0, 3.5, 7.5] {
                let growth = Growth {
                    root,
                    direction,
                    length,
                    radius: 1.16,
                    block: Block::Crystal,
                    column: false,
                };
                let bounds = growth.bounds();
                let mut voxels = BTreeSet::new();
                for z in bounds.min_z.floor() as i32..=bounds.max_z.ceil() as i32 {
                    for x in bounds.min_x.floor() as i32..=bounds.max_x.ceil() as i32 {
                        for y in bounds.min_y.floor() as i32..=bounds.max_y.ceil() as i32 {
                            if growth.contains(
                                f64::from(x) + 0.5,
                                f64::from(y) + 0.5,
                                f64::from(z) + 0.5,
                            ) {
                                voxels.insert([x, y, z]);
                            }
                        }
                    }
                }
                let root = growth.root.map(|value| value.floor() as i32);
                assert!(voxels.remove(&root));
                let mut queue = VecDeque::from([root]);
                while let Some(point) = queue.pop_front() {
                    for offset in [
                        [-1, 0, 0],
                        [1, 0, 0],
                        [0, -1, 0],
                        [0, 1, 0],
                        [0, 0, -1],
                        [0, 0, 1],
                    ] {
                        let next = std::array::from_fn(|axis| point[axis] + offset[axis]);
                        if voxels.remove(&next) {
                            queue.push_back(next);
                        }
                    }
                }
                assert!(
                    voxels.is_empty(),
                    "detached tip for {direction:?}, length {length}: {voxels:?}"
                );
            }
        }
    }
}
