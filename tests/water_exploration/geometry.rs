use super::*;

#[test]
fn append_only_materials_keep_legacy_collision_and_atlas_ids_distinct() {
    let legacy = [
        Block::Air,
        Block::Grass,
        Block::Dirt,
        Block::Stone,
        Block::Sand,
        Block::Snow,
        Block::Wood,
        Block::Leaves,
        Block::Water,
        Block::Bedrock,
        Block::DeepStone,
        Block::CoalOre,
        Block::IronOre,
        Block::GoldOre,
        Block::Crystal,
        Block::Moss,
        Block::Lava,
        Block::Limestone,
        Block::Slate,
        Block::Basalt,
        Block::Calcite,
    ];
    assert_eq!(&Block::ALL[..legacy.len()], &legacy);
    assert_eq!(Block::COUNT, 27);
    assert_eq!(TILE_COUNT, 28);
    assert_eq!(
        &Block::ALL[21..],
        &[
            Block::Gravel,
            Block::Clay,
            Block::Planks,
            Block::DressedLimestone,
            Block::StorageCrate,
            Block::Lantern
        ]
    );
    for (id, material) in Block::ALL.into_iter().enumerate() {
        assert_eq!(material as usize, id);
        assert_eq!(
            material.is_solid(),
            !matches!(material, Block::Air | Block::Water | Block::Lava)
        );
        assert_eq!(
            material.is_opaque(),
            !matches!(material, Block::Air | Block::Water)
        );
        assert_eq!(
            material.is_liquid(),
            matches!(material, Block::Water | Block::Lava)
        );
    }
    assert_eq!(Block::Gravel as u8, 21);
    assert_eq!(Block::Clay as u8, 22);
    let kinds = [
        WaterBiome::Lake,
        WaterBiome::River,
        WaterBiome::Estuary,
        WaterBiome::Ocean,
        WaterBiome::Waterfall,
        WaterBiome::CavePool,
    ];
    for (id, kind) in kinds.into_iter().enumerate() {
        assert_eq!(kind as usize, id);
        assert!(!kind.name().is_empty());
    }
}

#[test]
fn exposed_water_mesh_has_valid_fractional_geometry_and_metadata_from_actual_wet_voxels() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let reference = ResidentWorld::new(terrain);
        let mut seen_water = [0_usize; 6];
        let mut exposed_cave_pool = None;
        let mut sediments = [0_usize; 2];
        let mut partial_top_faces = 0;
        let mut shallow = 0;
        let mut deep = 0;
        let mut foam = 0;
        let mut coords = BTreeSet::new();
        for kind in SURFACE_BIOMES {
            let site = landmark(&terrain, kind);
            let center = ChunkCoord::new(
                site.water[0].div_euclid(CHUNK_SIZE),
                site.water[2].div_euclid(CHUNK_SIZE),
            );
            coords.insert(center);
            coords.insert(ChunkCoord::new(center.x + 1, center.z));
        }
        coords.extend(cave_survey_coords(&terrain));
        assert!(coords.len() <= 37);
        for coord in coords {
            let chunk = Chunk::generate(&terrain, coord);
            let mesh = mesh_chunk(&chunk, &terrain);
            validate_geometry(&mesh.opaque, false);
            validate_geometry(&mesh.water, true);
            for vertex in &mesh.opaque.vertices {
                if vertex.material == 22 {
                    sediments[0] += 1;
                }
                if vertex.material == 23 {
                    sediments[1] += 1;
                }
            }
            for face in mesh.water.vertices.as_chunks::<4>().0 {
                let [x, y, z] = source_voxel(face);
                let actual = chunk.water(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
                    .unwrap_or_else(|| panic!("seed {seed}, water face lacks resident source at ({x}, {y}, {z}): {face:?}"));
                assert_eq!(terrain.block_at(x, y, z), Block::Water);
                seen_water[actual.biome as usize] += 1;
                if actual.biome == WaterBiome::CavePool {
                    assert!(y < 0);
                    assert_eq!(actual.flow, [0.0; 3]);
                    assert_eq!(actual.salinity, 0.0);
                    assert!(reference.block([x, actual.bottom, z]).is_solid());
                    exposed_cave_pool.get_or_insert([x, y, z]);
                }
                if face[0].normal[1] > 0.5 {
                    assert!(
                        face.iter()
                            .all(|vertex| (vertex.position[1] - actual.surface).abs() < 0.001),
                        "seed {seed}, mean-line top mismatch: {actual:?}, face={face:?}"
                    );
                    partial_top_faces += usize::from(actual.surface.fract().abs() > 0.001);
                }
                for vertex in face {
                    let (expected, vertical) = expected_vertex_water(&reference, actual, vertex);
                    for (value, expected) in vertex.water.into_iter().zip(expected) {
                        assert!(
                            (value - expected).abs() < METADATA_TOLERANCE,
                            "seed {seed}, metadata disagrees with actual wet corner contributors: value={value}, expected={expected}, vertex={vertex:?}"
                        );
                    }
                    assert!((vertex.water_extra[0] - vertical).abs() < METADATA_TOLERANCE);
                    assert_eq!(vertex.water_extra[1], actual.biome as u8 as f32);
                    assert!(
                        vertex.water[2] > 0.0
                            && vertex.water[2] <= WORLD_HEIGHT as f32 - MIN_WORLD_Y as f32
                    );
                    assert!((0.0..=1.0).contains(&vertex.water[3]));
                    assert!((0.0..=1.0).contains(&vertex.water_extra[2]));
                    assert!((0.0..=1.0).contains(&vertex.water_extra[3]));
                    if actual.biome == WaterBiome::Waterfall {
                        assert_eq!(vertex.water_extra[3], 0.0);
                    }
                    shallow += usize::from(vertex.water[2] < 2.0);
                    deep += usize::from(vertex.water[2] >= 8.0);
                    foam += usize::from(vertex.water_extra[2] > 0.0);
                }
            }
        }
        for (kind, count) in SURFACE_BIOMES.into_iter().zip(seen_water) {
            assert!(
                count > 0,
                "seed {seed}, missing exposed {} faces",
                kind.name()
            );
        }
        assert!(
            seen_water[WaterBiome::CavePool as usize] > 0 && exposed_cave_pool.is_some(),
            "seed {seed}, bounded three-biome cave survey lacks an actual exposed calm freshwater pool"
        );
        assert!(
            sediments.into_iter().all(|count| count > 0),
            "seed {seed}, gravel/clay exposed vertices={sediments:?}"
        );
        assert!(
            partial_top_faces > 0 && shallow > 0 && deep > 0 && foam > 0,
            "seed {seed}, partial={partial_top_faces}, shallow={shallow}, deep={deep}, foam={foam}"
        );
    }
}

#[test]
fn phase_residency_keeps_water_metadata_sediment_ids_and_evicted_chunks_unknown() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let mut phase = PhaseWorld::default();
        for kind in SURFACE_BIOMES {
            let site = landmark(&terrain, kind);
            let [x, y, z] = site.water;
            let profile = terrain.water_at(x, y, z).unwrap();
            let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
            assert!(phase.water_at(x, y, z).is_none());
            let chunk = Chunk::generate(&terrain, coord);
            let bed = chunk.block(
                x.rem_euclid(CHUNK_SIZE),
                profile.bottom,
                z.rem_euclid(CHUNK_SIZE),
            );
            phase.insert(chunk);
            assert_water_eq(phase.water_at(x, y, z), Some(profile));
            assert_eq!(phase.block_at(x, y, z), Some(Block::Water));
            assert_eq!(phase.block_at(x, profile.bottom, z), Some(bed));
            assert!(phase.water_at(x, profile.bottom, z).is_none());
            let eye = Vec3::new(x as f32 + 0.5, profile.bottom as f32 + 0.5, z as f32 + 0.5);
            assert!(phase.phasing(eye, true));
            assert!(!phase.phasing(eye, false));
            assert!(!phase.phasing(
                Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                true
            ));
            assert!(phase.update_volume(eye));
            assert_eq!(phase.volume().voxel([x, profile.bottom, z]), bed as u8);
            assert_eq!(phase.volume().voxel([x, y, z]), Block::Water as u8);
            phase.remove(coord);
            assert!(phase.water_at(x, y, z).is_none());
            assert_eq!(phase.block_at(x, y, z), None);
            assert!(!phase.phasing(eye, true));
            assert!(phase.update_volume(eye));
            assert_eq!(phase.volume().voxel([x, profile.bottom, z]), UNKNOWN_VOXEL);
            assert_eq!(phase.volume().voxel([x, y, z]), UNKNOWN_VOXEL);
            assert!(terrain.water_at(x, y, z).is_some());
        }
        for y in [MIN_WORLD_Y - 1, WORLD_HEIGHT] {
            assert!(phase.water_at(-1, y, -17).is_none());
            assert_eq!(phase.block_at(-1, y, -17), Some(Block::Air));
        }
        let cave_feet = terrain.cave_position(CaveBiome::Lush).unwrap();
        let coord = ChunkCoord::from_world(cave_feet[0], cave_feet[2]);
        phase.insert(Chunk::generate(&terrain, coord));
        let feet = cave_feet.map(|value| value.floor() as i32);
        assert_eq!(phase.block_at(feet[0], feet[1], feet[2]), Some(Block::Air));
        assert!(
            phase
                .block_at(feet[0], feet[1] - 1, feet[2])
                .unwrap()
                .is_solid()
        );
        assert!(phase.water_at(feet[0], feet[1], feet[2]).is_none());
        assert!(!phase.phasing(Vec3::from_array(cave_feet), true));
        phase.clear();
        assert_eq!(phase.block_at(feet[0], feet[1], feet[2]), None);
    }
}
