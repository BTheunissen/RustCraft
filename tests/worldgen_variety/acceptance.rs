use super::*;

#[test]
fn landmarks_have_dry_jump_clearance_long_views_and_large_bounded_air_components() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = World::new(terrain);
        let mut unique = HashSet::new();
        for kind in LANDFORMS {
            let landmark = world.landmark(kind);
            assert_landmark_clear(&world, landmark, kind);
            assert_eq!(terrain.cave_landmark(kind), Some(landmark));
            let feet = Vec3::from_array(landmark.position);
            let voxel = feet.to_array().map(|coordinate| coordinate.floor() as i32);
            assert!(unique.insert(voxel), "reused landmark: {landmark:?}");
            let cave = LocalCave::sample(&world, feet);
            assert!(
                cave.negative_cells >= 512,
                "{landmark:?}: {} cells",
                cave.negative_cells
            );
            assert!(
                cave.span[0].max(cave.span[2]) >= 20,
                "{landmark:?}: {:?}",
                cave.span
            );
            assert!(
                cave.span[0].min(cave.span[2]) >= 8,
                "{landmark:?}: {:?}",
                cave.span
            );
            for [dx, dy, dz] in NEIGHBORS {
                let sample = [voxel[0] + dx, voxel[1] + dy, voxel[2] + dz];
                assert_eq!(
                    world.block(sample),
                    terrain.block_at(sample[0], sample[1], sample[2])
                );
            }
        }
    }
}

#[test]
fn observed_sections_distinguish_low_galleries_tall_rifts_roomy_cathedrals_and_branching_loops() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = World::new(terrain);
        let observations = LANDFORMS.map(|kind| {
            let landmark = world.landmark(kind);
            let feet = Vec3::from_array(landmark.position);
            let cave = LocalCave::sample(&world, feet);
            let sections = Sections::observe(&cave, feet);
            assert!(
                sections.heights.len() >= 24,
                "{landmark:?}: {} observed intervals",
                sections.heights.len()
            );
            eprintln!(
                "seed={} {}: {:?}",
                terrain.seed(),
                kind.name(),
                sections.summary()
            );
            sections
        });
        let [gallery, rift, cathedral, labyrinth] = &observations;
        assert!(gallery.width_quantile(90) >= 12, "{:?}", gallery.summary());
        assert!(
            gallery.width_quantile(90) >= gallery.width_quantile(10) + 4,
            "{:?}",
            gallery.summary()
        );
        assert!(rift.height_quantile(90) >= 22, "{:?}", rift.summary());
        assert!(
            rift.height_quantile(90) >= gallery.height_quantile(90) + 8,
            "gallery={:?}, rift={:?}",
            gallery.summary(),
            rift.summary()
        );
        assert!(
            rift.aspect_quantile(75) >= gallery.aspect_quantile(75) * 1.5,
            "gallery={:?}, rift={:?}",
            gallery.summary(),
            rift.summary()
        );
        assert!(cathedral.wide >= 40, "{:?}", cathedral.summary());
        assert!(
            cathedral.height_quantile(90) >= 18,
            "{:?}",
            cathedral.summary()
        );
        assert!(
            cathedral.split_rows >= 4 || cathedral.split_columns >= 4,
            "no observed supports/ledges: {:?}",
            cathedral.summary()
        );
        assert!(labyrinth.narrow >= 80, "{:?}", labyrinth.summary());
        assert!(labyrinth.junctions >= 4, "{:?}", labyrinth.summary());
        assert!(
            labyrinth.corridor_loops >= 1,
            "no observed narrow loop: {:?}",
            labyrinth.summary()
        );
        assert!(labyrinth.split_rows >= 4, "{:?}", labyrinth.summary());
        assert!(
            labyrinth.narrow * (cathedral.narrow + cathedral.wide)
                >= cathedral.narrow * (labyrinth.narrow + labyrinth.wide) * 2,
            "cathedral={:?}, labyrinth={:?}",
            cathedral.summary(),
            labyrinth.summary()
        );
    }
}

#[test]
fn exposed_calcite_has_supported_tips_air_gaps_and_upper_geology_instead_of_floating_voxels() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = World::new(terrain);
        let mut exposed = HashSet::new();
        for coord in world.feature_coords() {
            let chunk = Chunk::generate(&terrain, coord);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y + 1..0 {
                        let point = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                        if chunk.block(x, y, z) == Block::Calcite
                            && NEIGHBORS.into_iter().any(|offset| {
                                world.block(std::array::from_fn(|axis| point[axis] + offset[axis]))
                                    == Block::Air
                            })
                        {
                            exposed.insert(point);
                        }
                    }
                }
            }
        }
        let mut supported_stems = 0;
        let mut upper = 0;
        let mut open_separations = 0;
        let mut attachments = BTreeMap::new();
        for point in exposed.iter().copied() {
            assert!(
                material_component_attached(&world, Block::Calcite, point, &mut attachments),
                "seed {}, floating calcite component at {point:?}",
                terrain.seed()
            );
            upper += usize::from(point[1] >= -20);
            let mut stem = false;
            for direction in [-1, 1] {
                if world.block([point[0], point[1] - direction, point[2]]) != Block::Air {
                    continue;
                }
                let mut length = 1;
                while length < WORLD_HEIGHT - MIN_WORLD_Y
                    && world.block([point[0], point[1] + direction * length, point[2]])
                        == Block::Calcite
                {
                    length += 1;
                }
                let root = world.block([point[0], point[1] + direction * length, point[2]]);
                stem |= length >= 2 && root.is_solid();
            }
            supported_stems += usize::from(stem);
            for offset in NEIGHBORS {
                let mut gap = 1;
                let next =
                    |distance| std::array::from_fn(|axis| point[axis] + offset[axis] * distance);
                while gap <= 20 && world.block(next(gap)) == Block::Air {
                    gap += 1;
                }
                open_separations += usize::from(
                    (3..=20).contains(&gap) && world.block(next(gap)) == Block::Calcite,
                );
            }
        }
        eprintln!(
            "seed={} calcite: exposed={}, upper={upper}, supported stems={supported_stems}, air-gap samples={open_separations}",
            terrain.seed(),
            exposed.len()
        );
        assert!(
            exposed.len() >= 20,
            "seed {}, exposed calcite={}",
            terrain.seed(),
            exposed.len()
        );
        assert!(upper >= 4, "seed {}, upper calcite={upper}", terrain.seed());
        assert!(
            supported_stems >= 4,
            "seed {}, supported stems={supported_stems}",
            terrain.seed()
        );
        assert!(
            open_separations >= 1,
            "seed {}, calcite air-gap pairs={open_separations}",
            terrain.seed()
        );
    }
}

#[test]
fn exposed_crystal_clusters_are_attached_to_the_generated_cave_not_floating_in_air() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = World::new(terrain);
        let feet = Vec3::from_array(terrain.cave_position(CaveBiome::Crystal).unwrap());
        let cave = LocalCave::sample(&world, feet);
        let mut attachments = BTreeMap::new();
        let mut exposed = 0;
        for z in cave.min[2]..cave.min[2] + cave.size[2] {
            for x in cave.min[0]..cave.min[0] + cave.size[0] {
                for y in MIN_WORLD_Y + 1..0 {
                    let point = [x, y, z];
                    if cave.block(point) != Block::Crystal
                        || !NEIGHBORS.into_iter().any(|offset| {
                            cave.connected(std::array::from_fn(|axis| point[axis] + offset[axis]))
                        })
                    {
                        continue;
                    }
                    exposed += 1;
                    assert!(
                        material_component_attached(
                            &world,
                            Block::Crystal,
                            point,
                            &mut attachments
                        ),
                        "seed {}, floating crystal cluster at {point:?}",
                        terrain.seed()
                    );
                }
            }
        }
        assert!(
            exposed >= 16,
            "seed {}, exposed crystal={exposed}",
            terrain.seed()
        );
    }
}

#[test]
fn local_sampling_preserves_negative_y_indices_and_clips_only_at_the_observation_edge() {
    let cave = LocalCave {
        min: [-17, MIN_WORLD_Y + 1, -1],
        size: [3, 4, 2],
        blocks: (0..24)
            .map(|index| Block::ALL[index % Block::COUNT])
            .collect(),
        connected: (0_usize..24).map(|index| index.is_multiple_of(2)).collect(),
        negative_cells: 0,
        span: [0; 3],
    };
    assert_eq!(cave.index([-17, MIN_WORLD_Y + 1, -1]), Some(0));
    assert_eq!(cave.index([-15, MIN_WORLD_Y + 4, 0]), Some(23));
    assert_eq!(cave.block([-16, MIN_WORLD_Y + 1, 0]), Block::ALL[13]);
    assert!(cave.connected([-17, MIN_WORLD_Y + 1, -1]));
    assert!(!cave.connected([-16, MIN_WORLD_Y + 1, -1]));
    for outside in [
        [-18, MIN_WORLD_Y + 1, -1],
        [-14, MIN_WORLD_Y + 1, -1],
        [-17, MIN_WORLD_Y, -1],
        [-17, MIN_WORLD_Y + 5, -1],
        [-17, MIN_WORLD_Y + 1, -2],
        [-17, MIN_WORLD_Y + 1, 1],
    ] {
        assert_eq!(cave.index(outside), None);
        assert!(!cave.connected(outside));
        assert_eq!(cave.block(outside), Block::Stone);
    }
}

#[test]
fn nearby_buried_columns_show_multiple_geological_layers_and_seed_variation() {
    let mut fingerprints = Vec::new();
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = World::new(terrain);
        let spawn = terrain.spawn_position();
        let center = [spawn[0].floor() as i32, spawn[2].floor() as i32];
        let mut layers = [0_usize; 3];
        let mut layered_columns = 0;
        let mut fingerprint = 0_u64;
        for dz in (-20..=20).step_by(2) {
            for dx in (-20..=20).step_by(2) {
                let mut observed = [false; 3];
                for y in MIN_WORLD_Y + 4..0 {
                    let point = [center[0] + dx, y, center[1] + dz];
                    let block = world.block(point);
                    let layer = match block {
                        Block::Limestone => Some(0),
                        Block::Slate => Some(1),
                        Block::Basalt => Some(2),
                        _ => None,
                    };
                    if let Some(layer) = layer {
                        layers[layer] += 1;
                        observed[layer] = true;
                    }
                    fingerprint = fingerprint.rotate_left(5) ^ u64::from(block as u8);
                }
                layered_columns +=
                    usize::from(observed.into_iter().filter(|&seen| seen).count() >= 2);
            }
        }
        assert!(
            layers.into_iter().all(|count| count >= 100),
            "seed {}, layers={layers:?}",
            terrain.seed()
        );
        assert!(
            layered_columns >= 40,
            "seed {}, layered columns={layered_columns}",
            terrain.seed()
        );
        fingerprints.push(fingerprint);
    }
    assert_ne!(fingerprints[0], fingerprints[1]);
}
