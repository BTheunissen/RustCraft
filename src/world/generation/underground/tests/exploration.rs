use super::*;

#[test]
fn landmarks_use_actual_families_jump_clear_ground_and_meaningful_views() {
    let mut positions = std::collections::BTreeSet::new();
    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        let mut chunks = std::collections::BTreeMap::new();
        for form in [
            CaveLandform::Gallery,
            CaveLandform::Rift,
            CaveLandform::Cathedral,
            CaveLandform::Labyrinth,
        ] {
            let landmark = terrain
                .cave_landmark(form)
                .unwrap_or_else(|| panic!("seed {seed}: no {form:?}"));
            assert_eq!(landmark, terrain.cave_landmark(form).unwrap());
            assert_eq!(landmark.landform, form);
            assert!(!form.name().is_empty());
            let point = landmark.position.map(|value| value.floor() as i32);
            positions.insert(point);
            assert_eq!(terrain.cave_landform(point[0], point[1], point[2]), form);
            assert!(diagnostic_clear(&terrain, &mut chunks, point));
            assert!(
                (landmark.position[0] - spawn[0]).hypot(landmark.position[2] - spawn[2]) <= 250.0
            );
            let eye = [
                landmark.position[0],
                landmark.position[1] + 1.62,
                landmark.position[2],
            ];
            let distance = (landmark.target[0] - eye[0]).hypot(landmark.target[2] - eye[2]);
            assert!((15.0..=28.0).contains(&distance));
            for step in 0..=40 {
                let t = step as f32 / 40.0;
                let [x, y, z] = std::array::from_fn(|axis| {
                    (eye[axis] + (landmark.target[axis] - eye[axis]) * t).floor() as i32
                });
                assert_eq!(
                    cached_block(&terrain, &mut chunks, x, y, z),
                    Block::Air,
                    "seed {seed} {form:?}: blocked view at ({x},{y},{z})"
                );
            }
        }
    }
    assert!(positions.len() >= 16);
}

#[test]
fn entrance_ramp_centers_keep_supported_standing_clearance() {
    let terrain = Terrain::new(42);
    let start = terrain.entrance_position().unwrap();
    let query = Bounds {
        min_x: -320.0,
        max_x: 320.0,
        min_z: -320.0,
        max_z: 320.0,
        min_y: f64::from(MIN_WORLD_Y),
        max_y: f64::from(WORLD_HEIGHT),
    };
    let mut selected = None;
    for z in -5..=5 {
        for x in -5..=5 {
            if let Some(entrance) = entrance(&terrain, x, z, query) {
                let distance = (entrance.mouth[0] - f64::from(start[0])).powi(2)
                    + (entrance.mouth[2] - f64::from(start[2])).powi(2);
                if selected.is_none_or(|(old, _)| distance < old) {
                    selected = Some((distance, entrance));
                }
            }
        }
    }
    let (_, entrance) = selected.unwrap();
    let mut chunks = std::collections::BTreeMap::new();
    for (a, b) in [
        (entrance.inner.anchor(), entrance.middle),
        (
            entrance.middle,
            [
                entrance.mouth[0],
                entrance.mouth[1] - 3.0,
                entrance.mouth[2],
            ],
        ),
    ] {
        for step in 0..=128 {
            let t = f64::from(step) / 128.0;
            let x = (a[0] + (b[0] - a[0]) * t).floor() as i32;
            let z = (a[2] + (b[2] - a[2]) * t).floor() as i32;
            let y = (a[1] + (b[1] - a[1]) * t).round() as i32;
            let levels = (y - 8..=y + 4)
                .filter(|&y| {
                    cached_block(&terrain, &mut chunks, x, y - 1, z).is_solid()
                        && (0..=1).all(|dy| {
                            cached_block(&terrain, &mut chunks, x, y + dy, z) == Block::Air
                        })
                })
                .collect::<Vec<_>>();
            assert!(
                !levels.is_empty(),
                "ramp {:?} -> {:?}: ({x},{y},{z}); blocks={:?}",
                a,
                b,
                (y - 10..=y + 4)
                    .map(|y| (y, cached_block(&terrain, &mut chunks, x, y, z)))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn surface_water_bias_rejects_entrance_routes_that_would_cut_basin_seals() {
    use crate::world::generation::hydrology::HydrologyRegion;
    let terrain = Terrain::new(42);
    let query = Bounds {
        min_x: -320.0,
        max_x: 320.0,
        min_z: -320.0,
        max_z: 320.0,
        min_y: f64::from(MIN_WORLD_Y),
        max_y: f64::from(WORLD_HEIGHT),
    };
    let mut entrances = 0;
    for cell_z in -3..=3 {
        for cell_x in -3..=3 {
            let Some(site) = entrance(&terrain, cell_x, cell_z, query) else {
                continue;
            };
            let points = [
                site.inner.anchor(),
                site.middle,
                [site.mouth[0], site.mouth[1] - 3.0, site.mouth[2]],
            ];
            let min_x = points
                .iter()
                .map(|point| point[0])
                .fold(f64::INFINITY, f64::min)
                .floor() as i32;
            let max_x = points
                .iter()
                .map(|point| point[0])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil() as i32;
            let min_z = points
                .iter()
                .map(|point| point[2])
                .fold(f64::INFINITY, f64::min)
                .floor() as i32;
            let max_z = points
                .iter()
                .map(|point| point[2])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil() as i32;
            let hydrology = HydrologyRegion::columns(terrain.seed(), min_x, max_x, min_z, max_z);
            for pair in points.windows(2) {
                for step in 0..=64 {
                    let t = f64::from(step) / 64.0;
                    let point: [f64; 3] = std::array::from_fn(|axis| {
                        pair[0][axis] + (pair[1][axis] - pair[0][axis]) * t
                    });
                    let profile = hydrology.column(
                        &terrain,
                        point[0].floor() as i32,
                        point[2].floor() as i32,
                    );
                    assert!(
                        profile
                            .seal_from
                            .is_none_or(|first| point[1] + 2.0 < f64::from(first)),
                        "entrance intersects surface seal at {point:?}"
                    );
                }
            }
            entrances += 1;
        }
    }
    assert!(entrances > 0, "nearby water removed every surface entrance");
}

#[test]
fn underground_materials_are_nearby_and_liquid_basins_are_supported() {
    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        let center = ChunkCoord::from_world(spawn[0], spawn[2]);
        let mut counts = [0; Block::COUNT];
        let mut exposed_ores = [0; 3];
        let mut adjacent_ores = [0; 3];
        let mut chunks = std::collections::BTreeMap::new();
        for chunk_z in center.z - 6..=center.z + 6 {
            for chunk_x in center.x - 6..=center.x + 6 {
                let chunk = Chunk::generate(&terrain, ChunkCoord::new(chunk_x, chunk_z));
                for local_z in 0..CHUNK_SIZE {
                    for local_x in 0..CHUNK_SIZE {
                        let x = chunk_x * CHUNK_SIZE + local_x;
                        let z = chunk_z * CHUNK_SIZE + local_z;
                        for y in MIN_WORLD_Y..0 {
                            let block = chunk.block(local_x, y, local_z);
                            counts[block as usize] += 1;
                            if block.is_liquid() {
                                let below = chunk.block(local_x, y - 1, local_z);
                                assert!(
                                    below.is_solid() || below == block,
                                    "unsupported {block:?} ({x}, {y}, {z})"
                                );
                                if block == Block::Lava {
                                    assert!(y < -40);
                                }
                                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                    let bank =
                                        cached_block(&terrain, &mut chunks, x + dx, y, z + dz);
                                    assert!(
                                        bank.is_solid() || bank == block,
                                        "leaking {block:?} ({x}, {y}, {z}) -> {bank:?}"
                                    );
                                }
                            }
                            let ore = match block {
                                Block::CoalOre => Some(0),
                                Block::IronOre => Some(1),
                                Block::GoldOre => Some(2),
                                _ => None,
                            };
                            if let Some(ore) = ore {
                                for [dx, dy, dz] in [
                                    [-1, 0, 0],
                                    [1, 0, 0],
                                    [0, -1, 0],
                                    [0, 1, 0],
                                    [0, 0, -1],
                                    [0, 0, 1],
                                ] {
                                    let neighbor = if (0..CHUNK_SIZE).contains(&(local_x + dx))
                                        && (0..CHUNK_SIZE).contains(&(local_z + dz))
                                    {
                                        chunk.block(local_x + dx, y + dy, local_z + dz)
                                    } else {
                                        cached_block(&terrain, &mut chunks, x + dx, y + dy, z + dz)
                                    };
                                    exposed_ores[ore] += usize::from(neighbor == Block::Air);
                                    adjacent_ores[ore] += usize::from(neighbor == block);
                                }
                            }
                        }
                    }
                }
            }
        }
        for material in [
            Block::Air,
            Block::Stone,
            Block::DeepStone,
            Block::Bedrock,
            Block::CoalOre,
            Block::IronOre,
            Block::GoldOre,
            Block::Moss,
            Block::Crystal,
            Block::Water,
            Block::Lava,
            Block::Limestone,
            Block::Slate,
            Block::Basalt,
            Block::Calcite,
        ] {
            assert!(
                counts[material as usize] > 16,
                "seed {seed}, missing nearby {material:?}: {counts:?}"
            );
        }
        assert!(
            exposed_ores.into_iter().all(|count| count > 16),
            "seed {seed}: {exposed_ores:?}"
        );
        assert!(
            adjacent_ores.into_iter().all(|count| count > 128),
            "seed {seed}: {adjacent_ores:?}"
        );
        assert!(
            counts[Block::Water as usize] + counts[Block::Lava as usize]
                < counts[Block::Air as usize] / 20
        );
    }
}

#[test]
fn nearby_diagnostic_sites_have_jump_clearance_and_long_chamber_views() {
    for seed in [0, 1, 42, Terrain::default().seed(), u64::MAX] {
        let terrain = Terrain::new(seed);
        let spawn = terrain.spawn_position();
        let mut chunks = std::collections::BTreeMap::new();
        for biome in [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal] {
            let position = terrain
                .cave_position(biome)
                .unwrap_or_else(|| panic!("seed {seed}: no {biome:?} site"));
            assert!(position[1] < 0.0);
            assert!((position[0] - spawn[0]).hypot(position[2] - spawn[2]) <= 250.0);
            let [x, y, z] = position.map(|coordinate| coordinate.floor() as i32);
            assert_eq!(terrain.cave_biome(x, y, z), biome);
            assert_eq!(terrain.cave_biome(x, y + 2, z), biome);
            assert!(cached_block(&terrain, &mut chunks, x, y - 1, z).is_solid());
            for dz in -1..=1 {
                for dx in -1..=1 {
                    for dy in 0..=3 {
                        assert_eq!(
                            cached_block(&terrain, &mut chunks, x + dx, y + dy, z + dz),
                            Block::Air,
                            "seed {seed}, {biome:?} at {position:?}"
                        );
                    }
                }
            }
            let long_view = (0..16).any(|angle| {
                let angle = f64::from(angle) * std::f64::consts::TAU / 16.0;
                (1..=18).all(|step| {
                    let ray_x = (f64::from(x) + 0.5 + angle.cos() * f64::from(step)).floor() as i32;
                    let ray_z = (f64::from(z) + 0.5 + angle.sin() * f64::from(step)).floor() as i32;
                    cached_block(&terrain, &mut chunks, ray_x, y + 1, ray_z) == Block::Air
                })
            });
            assert!(long_view, "seed {seed}, {biome:?} at {position:?}");
            assert!(diagnostic_climate_core(seed, x, y + 1, z, biome));
            assert!(diagnostic_walls_match(
                &terrain,
                &mut chunks,
                x,
                y,
                z,
                biome
            ));
        }
    }
}

#[test]
fn surface_entrances_connect_walkable_chunk_seams_and_both_cave_layers() {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    for seed in [42, Terrain::default().seed()] {
        let terrain = Terrain::new(seed);
        let start = terrain
            .entrance_position()
            .expect("surface entrance")
            .map(|coordinate| coordinate.floor() as i32);
        let spawn = terrain
            .spawn_position()
            .map(|coordinate| coordinate.floor() as i32);
        let mut goals = [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal]
            .map(|biome| {
                terrain
                    .cave_position(biome)
                    .expect("nearby chamber")
                    .map(|coordinate| coordinate.floor() as i32)
            })
            .to_vec();
        let mut chunks = BTreeMap::new();
        let mut queue = VecDeque::from([start]);
        let mut visited = BTreeSet::from([start]);
        let mut underground_chunks = BTreeSet::new();
        let mut deepest = start[1];
        while let Some([x, y, z]) = queue.pop_front() {
            goals.retain(|goal| *goal != [x, y, z]);
            deepest = deepest.min(y);
            if y < 0 {
                underground_chunks
                    .insert((split_block_coordinate(x).0, split_block_coordinate(z).0));
            }
            if goals.is_empty() && deepest < -40 {
                break;
            }
            assert!(
                visited.len() < 600_000,
                "seed {seed}: BFS failed to find {goals:?}"
            );
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let next_x = x + dx;
                let next_z = z + dz;
                // A nearby mouth can lead away from spawn before joining
                // its cavern; the ramp's world-space run is 128 blocks.
                if (next_x - spawn[0]).abs() > 240 || (next_z - spawn[2]).abs() > 240 {
                    continue;
                }
                for dy in [-1, 0, 1] {
                    let next_y = y + dy;
                    let next = [next_x, next_y, next_z];
                    if visited.contains(&next)
                        || !(FIRST_CAVE_Y..WORLD_HEIGHT - 3).contains(&next_y)
                    {
                        continue;
                    }
                    if cached_block(&terrain, &mut chunks, next_x, next_y - 1, next_z).is_solid()
                        && cached_block(&terrain, &mut chunks, next_x, next_y, next_z) == Block::Air
                        && cached_block(&terrain, &mut chunks, next_x, next_y + 1, next_z)
                            == Block::Air
                        && (dy != 1
                            || cached_block(&terrain, &mut chunks, x, y + 2, z) == Block::Air)
                    {
                        visited.insert(next);
                        queue.push_back(next);
                    }
                }
            }
        }
        assert!(
            goals.is_empty(),
            "seed {seed}: unreachable cave sites {goals:?}; deepest={deepest}, visited={}",
            visited.len()
        );
        assert!(deepest < -40);
        assert!(underground_chunks.len() >= 8);
    }
}
