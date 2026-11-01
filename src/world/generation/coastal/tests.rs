use super::*;
use crate::mesh::mesh_chunk;
use crate::world::{CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, WORLD_HEIGHT, WaterBiome};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const SEEDS: [u64; 6] = [0x5255_5354_4352_4146, 42, 0, 1, 7, 2026];

#[test]
fn seeded_regions_have_dry_supported_views_and_distinct_grounded_stacks() {
    let mut variations = BTreeSet::new();
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let site = terrain
            .coastal_landmark()
            .unwrap_or_else(|| panic!("seed {seed}: no coast"));
        assert_eq!(site, terrain.coastal_landmark().unwrap());
        for feet in [site.viewpoint, site.cove] {
            let [x, y, z] = feet.map(|value| value.floor() as i32);
            assert!(terrain.block_at(x, y - 1, z).is_solid());
            assert!(terrain.water_column(x, z).is_none());
            assert!((0..3).all(|dy| terrain.block_at(x, y + dy, z) == Block::Air));
            assert!((site.bounds[0][0]..=site.bounds[1][0]).contains(&x));
            assert!((site.bounds[0][1]..=site.bounds[1][1]).contains(&z));
        }
        assert!((5..=6).contains(&site.stacks.len()));
        let mut heights = BTreeSet::new();
        for body in &site.stacks {
            let [x, z] = body.center;
            assert!((SEA_LEVEL + 16..=SEA_LEVEL + 34).contains(&body.top));
            heights.insert(body.top);
            assert_eq!(terrain.block_at(x, body.top + 1, z), Block::Air);
            assert!(terrain.water_column(x, z).is_none());
            for y in SEA_LEVEL - 5..=body.top {
                assert!(
                    terrain.block_at(x, y, z).is_solid(),
                    "seed {seed}: unsupported stack ({x},{y},{z})"
                );
                if y > SEA_LEVEL {
                    assert!(matches!(
                        terrain.block_at(x, y, z),
                        Block::Limestone | Block::Calcite
                    ));
                }
            }
            for other in &site.stacks {
                if body != other {
                    assert!(
                        f64::from(x - other.center[0]).hypot(f64::from(z - other.center[1])) > 18.0
                    );
                }
            }
        }
        assert!(
            heights.len() >= 3,
            "seed {seed}: repeated heights {heights:?}"
        );
        variations.insert((site.stacks[0].center, site.stacks[0].top));
    }
    assert_eq!(variations.len(), SEEDS.len());
}

#[test]
fn real_above_water_components_are_separate_and_surrounded_by_supported_marine_water() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let site = terrain.coastal_landmark().unwrap();
        let mut bodies = BTreeSet::new();
        let mut wet_neighbors = 0;
        let mut layers = BTreeSet::new();
        for body in site.stacks {
            let [cx, cz] = body.center;
            let mut solid = BTreeSet::new();
            for z in cz - 12..=cz + 12 {
                for x in cx - 12..=cx + 12 {
                    if terrain.block_at(x, SEA_LEVEL + 1, z).is_solid() {
                        solid.insert([x, z]);
                    }
                    if let Some(water) = terrain.water_column(x, z) {
                        assert_eq!(water.biome, WaterBiome::Ocean);
                        assert_eq!(water.salinity, 1.0);
                        assert_eq!(water.surface, SEA_LEVEL as f32);
                        assert!(water.flow.iter().all(|value| value.is_finite()));
                        assert!(
                            (water.bottom - 3..=water.bottom)
                                .all(|y| terrain.block_at(x, y, z).is_solid())
                        );
                        assert_eq!(terrain.block_at(x, water.bottom + 1, z), Block::Water);
                        assert_eq!(terrain.water_at(x, water.bottom, z), None);
                        wet_neighbors += 1;
                    }
                }
            }
            let mut connected = BTreeSet::from([[cx, cz]]);
            let mut queue = VecDeque::from([[cx, cz]]);
            while let Some([x, z]) = queue.pop_front() {
                for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                    let next = [x + dx, z + dz];
                    if solid.contains(&next) && connected.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            assert!(connected.len() >= 40, "seed {seed}: tiny stack");
            assert!(
                connected
                    .iter()
                    .all(|&[x, z]| (x - cx).abs() < 12 && (z - cz).abs() < 12)
            );
            bodies.insert(*connected.first().unwrap());
            for y in SEA_LEVEL..=body.top {
                layers.insert(terrain.block_at(cx, y, cz) as u8);
            }
        }
        assert!(bodies.len() >= 5);
        assert!(wet_neighbors >= 1_000);
        assert!(
            layers.contains(&(Block::Limestone as u8)) && layers.contains(&(Block::Calcite as u8))
        );
    }
}

#[test]
fn direct_full_batch_dense_and_mesh_halos_agree_on_positive_and_negative_seams() {
    for seed in [SEEDS[0], 42, 7] {
        let terrain = Terrain::new(seed);
        let site = terrain.coastal_landmark().unwrap();
        let mut coords = BTreeSet::new();
        for body in &site.stacks {
            let center = ChunkCoord::new(
                body.center[0].div_euclid(CHUNK_SIZE),
                body.center[1].div_euclid(CHUNK_SIZE),
            );
            coords.insert(center);
            coords.insert(ChunkCoord::new(center.x - 1, center.z));
            coords.insert(ChunkCoord::new(center.x, center.z + 1));
        }
        let chunks: BTreeMap<_, _> = coords
            .iter()
            .map(|&coord| (coord, Chunk::generate(&terrain, coord)))
            .collect();
        for coord in coords.into_iter().rev() {
            let generated = Chunk::generate(&terrain, coord);
            let positions: Vec<_> = (0..16)
                .flat_map(|z| (0..16).map(move |x| [coord.x * 16 + x, coord.z * 16 + z]))
                .collect();
            let columns = terrain.column_block_batch(&positions);
            for (index, (&[x, z], blocks)) in positions.iter().zip(columns).enumerate() {
                let local_x = index as i32 % 16;
                let local_z = index as i32 / 16;
                assert_eq!(blocks, terrain.column_blocks(x, z));
                for y in MIN_WORLD_Y..WORLD_HEIGHT {
                    let block = blocks[(y - MIN_WORLD_Y) as usize];
                    assert_eq!(
                        block,
                        generated.block(local_x, y, local_z),
                        "seed {seed}: ({x},{y},{z})"
                    );
                    assert_eq!(block, chunks[&coord].block(local_x, y, local_z));
                    if y >= SEA_LEVEL - 6 {
                        assert_eq!(block, terrain.block_at(x, y, z));
                    }
                    let water = generated.water(local_x, y, local_z);
                    assert_eq!(water.is_some(), block == Block::Water);
                    if water.is_some() {
                        assert_eq!(water, terrain.water_at(x, y, z));
                    }
                }
            }
            let mesh = mesh_chunk(&generated, &terrain);
            assert!(
                mesh.opaque
                    .vertices
                    .iter()
                    .all(|vertex| vertex.light.is_finite())
            );
            for vertex in &mesh.water.vertices {
                assert!(
                    vertex
                        .water
                        .iter()
                        .chain(&vertex.water_extra)
                        .all(|value| value.is_finite())
                );
                match vertex.water_extra[1] as u8 {
                    value if value == WaterBiome::Ocean as u8 => assert_eq!(vertex.water[3], 1.0),
                    value if value == WaterBiome::CavePool as u8 => {
                        assert_eq!(vertex.water[3], 0.0)
                    }
                    biome => panic!("unexpected water biome {biome} at a coastal stack"),
                }
            }
        }
    }
}

#[test]
fn out_of_envelope_controls_and_extreme_projection_remain_unchanged() {
    use super::super::hydrology::HydrologyRegion;
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        for [x, z] in [
            [0, 0],
            [-17, -1],
            [900, 700],
            [i32::MIN, i32::MAX],
            [i32::MAX, i32::MIN],
        ] {
            let region = HydrologyRegion::columns(seed, x, x, z, z);
            let profile = region.column(&terrain, x, z);
            assert!(
                profile.coastal.is_none(),
                "unexpected control coast ({x},{z})"
            );
            assert_eq!(profile.column, terrain.column(x, z));
            assert_eq!(profile.water, terrain.water_column(x, z));
            assert_eq!(terrain.block_at(x, MIN_WORLD_Y, z), Block::Bedrock);
        }
    }
}

#[test]
fn coastal_blends_leave_freshwater_footprints_and_bank_aprons_unmodified() {
    use super::super::hydrology::HydrologyRegion;
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let mut lake_columns = 0;
        for z in (-260..=260).step_by(3) {
            for x in (-260..=260).step_by(3) {
                if terrain
                    .water_column(x, z)
                    .is_some_and(|water| water.biome == WaterBiome::Lake)
                {
                    let region = HydrologyRegion::columns(seed, x - 2, x + 2, z - 2, z + 2);
                    for [dx, dz] in [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]] {
                        let profile = region.column(&terrain, x + dx, z + dz);
                        assert!(
                            profile.coastal.is_none(),
                            "seed {seed}: coast intrudes on lake at ({x},{z})"
                        );
                    }
                    lake_columns += 1;
                }
            }
        }
        assert!(lake_columns >= 100);
    }
}

#[test]
fn intentional_golden_changes_intersect_only_the_bounded_coastal_columns() {
    use super::super::hydrology::HydrologyRegion;
    let coords = [
        ChunkCoord::new(0, 0),
        ChunkCoord::new(-2, -1),
        ChunkCoord::new(2, -18),
        ChunkCoord::new(28, 3),
        ChunkCoord::new(-18, -4),
        ChunkCoord::new(-18, -24),
    ];
    for seed in [SEEDS[0], 42] {
        let terrain = Terrain::new(seed);
        let mut counts = Vec::new();
        for coord in coords {
            let x = coord.x * CHUNK_SIZE;
            let z = coord.z * CHUNK_SIZE;
            let region = HydrologyRegion::columns(seed, x, x + 15, z, z + 15);
            let count = (0..16)
                .flat_map(|dz| (0..16).map(move |dx| [x + dx, z + dz]))
                .filter(|&[x, z]| region.column(&terrain, x, z).coastal.is_some())
                .count();
            counts.push(count);
        }
        if seed == SEEDS[0] {
            assert!(counts.iter().all(|&count| count == 0));
        } else {
            assert!(counts.iter().sum::<usize>() > 0);
        }
        println!(
            "generation-2 golden coverage seed={seed}: coastal column counts {counts:?} at {coords:?}"
        );
    }
}
