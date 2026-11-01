use super::*;

#[test]
fn spawn_and_bottom_bedrock_support_the_real_walking_collider() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = GeneratedWorld::new(terrain);
        let spawn = Vec3::from_array(terrain.spawn_position());
        assert!(safe_floor(&world, spawn).is_solid());
        assert!(!safe_floor(&world, spawn).is_liquid());
        let mut player = Player::new(spawn);
        settle(&world, &mut player);
        assert_position(player.position, spawn);

        let [x, _, z] = spawn.to_array().map(|coordinate| coordinate.floor() as i32);
        for dz in -1..=1 {
            for dx in -1..=1 {
                assert_eq!(world.block([x + dx, MIN_WORLD_Y, z + dz]), Block::Bedrock);
                assert_eq!(world.block([x + dx, MIN_WORLD_Y - 1, z + dz]), Block::Air);
            }
        }
        // Keep generated bedrock as the only collider to isolate its lower-bound support.
        let bedrock = |x, y, z| {
            (MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) && world.block([x, y, z]) == Block::Bedrock
        };
        let mut falling = Player::new(Vec3::new(spawn.x, (MIN_WORLD_Y + 8) as f32, spawn.z));
        for _ in 0..360 {
            falling.update(MovementInput::default(), DT, bedrock);
        }
        assert!(falling.is_grounded());
        assert!(falling.position.y >= (MIN_WORLD_Y + 1) as f32);
        assert_eq!(safe_floor(&world, falling.position), Block::Bedrock);
        let resting = falling.position;
        falling.update(MovementInput::default(), 0.25, bedrock);
        assert_position(falling.position, resting);
    }
}

#[test]
fn all_cave_biomes_have_nearby_generated_space_for_walking_jumping_and_landing() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = GeneratedWorld::new(terrain);
        let spawn = Vec3::from_array(terrain.spawn_position());
        let mut distinct = BTreeSet::new();
        for (biome, feet) in world.poses() {
            let point = feet.to_array().map(|coordinate| coordinate.floor() as i32);
            assert!((MIN_WORLD_Y + 1..0).contains(&point[1]));
            assert!((feet - spawn).with_y(0.0).length() <= 250.0);
            assert_eq!(terrain.cave_biome(point[0], point[1], point[2]), biome);
            assert!(
                distinct.insert(point),
                "biomes share diagnostic pose {feet:?}"
            );
            assert!(safe_floor(&world, feet).is_solid());
            assert!(body_is_clear(feet, |sample| world.block(sample)));
            let mut player = Player::new(feet);
            settle(&world, &mut player);
            assert_position(player.position, feet);

            let mut apex = feet.y;
            for frame in 0..240 {
                player.update(
                    MovementInput {
                        jump: frame == 0,
                        ..Default::default()
                    },
                    DT,
                    |x, y, z| world.solid(x, y, z),
                );
                apex = apex.max(player.position.y);
                assert_safe(&world, &player);
            }
            assert!(
                apex > feet.y + 0.1,
                "{} jump was blocked at {feet:?}",
                biome.name()
            );
            assert!(player.is_grounded());
            assert_position(player.position, feet);

            let (start, direction) = nearby_flat_walk(&world, feet).unwrap_or_else(|| {
                panic!(
                    "seed {}, {} has no nearby two-block walking segment",
                    terrain.seed(),
                    biome.name()
                )
            });
            player.respawn(start);
            player.yaw = direction.x.atan2(-direction.z);
            for _ in 0..48 {
                player.update(
                    MovementInput {
                        forward: true,
                        ..Default::default()
                    },
                    DT,
                    |x, y, z| world.solid(x, y, z),
                );
                assert_safe(&world, &player);
            }
            assert!(player.is_grounded());
            assert_position(player.position, start + direction * 2.0);
        }
    }
}

#[test]
fn cave_flight_exit_resumes_gravity_and_recovers_an_embedded_floor_pose() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = GeneratedWorld::new(terrain);
        for (_, feet) in world.poses() {
            let solid = |x, y, z| world.solid(x, y, z);
            let mut player = Player::new(feet);
            player.toggle_flight(solid);
            for _ in 0..12 {
                player.update(
                    MovementInput {
                        jump: true,
                        ..Default::default()
                    },
                    DT,
                    solid,
                );
            }
            assert!(player.flying);
            assert_position(player.position, feet + Vec3::Y * 0.5);
            assert_safe(&world, &player);
            player.toggle_flight(solid);
            assert!(!player.flying);
            assert!(!player.is_grounded());
            let airborne = player.position.y;
            player.update(MovementInput::default(), DT, solid);
            assert!(player.position.y < airborne);
            settle(&world, &mut player);
            assert_position(player.position, feet);

            player.toggle_flight(solid);
            for _ in 0..12 {
                player.update(
                    MovementInput {
                        descend: true,
                        ..Default::default()
                    },
                    DT,
                    solid,
                );
            }
            assert!(!body_is_clear(player.position, |point| world.block(point)));
            player.toggle_flight(solid);
            assert!(!player.flying);
            assert_position(player.position, feet);
            settle(&world, &mut player);
            assert_position(player.position, feet);
        }
    }
}

#[test]
fn nearby_dense_caves_have_exposed_ores_decorations_and_both_liquids_with_valid_light() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = GeneratedWorld::new(terrain);
        let coords = world.feature_coords();
        let mut counts = [0; Block::COUNT];
        let mut exposed = [0; TILE_COUNT as usize];
        let mut dark_cave_faces = 0;
        let mut clustered_materials = [BTreeSet::new(), BTreeSet::new()];
        for coord in coords {
            let chunk = Chunk::generate(&terrain, coord);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    assert_eq!(chunk.block(x, MIN_WORLD_Y, z), Block::Bedrock);
                    for y in MIN_WORLD_Y..0 {
                        let material = chunk.block(x, y, z);
                        counts[material as usize] += 1;
                        if let Some(index) = [Block::Moss, Block::Crystal]
                            .into_iter()
                            .position(|block| block == material)
                        {
                            let point = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                            let visible = [
                                [-1, 0, 0],
                                [1, 0, 0],
                                [0, -1, 0],
                                [0, 1, 0],
                                [0, 0, -1],
                                [0, 0, 1],
                            ]
                            .into_iter()
                            .any(|offset| {
                                world.block(std::array::from_fn(|axis| point[axis] + offset[axis]))
                                    == Block::Air
                            });
                            if visible {
                                clustered_materials[index].insert(point);
                            }
                        }
                    }
                }
            }
            let mesh = mesh_chunk(&chunk, &terrain);
            for (geometry, water) in [(&mesh.opaque, false), (&mesh.water, true)] {
                let materials = underground_mesh_materials(geometry, water);
                for (total, count) in exposed.iter_mut().zip(materials) {
                    *total += count;
                }
                for face in geometry.vertices.as_chunks::<4>().0 {
                    dark_cave_faces += usize::from(
                        face[0].position[1] < -1.0
                            && face[0].position[1] > (MIN_WORLD_Y + 1) as f32
                            && face[0].light < 0.5,
                    );
                }
            }
        }
        assert!(counts[Block::Air as usize] > 0);
        for (block, material) in UNDERGROUND_MATERIALS {
            assert!(
                counts[block as usize] > 0,
                "seed {}, missing {block:?} volume",
                terrain.seed()
            );
            assert!(
                exposed[material] > 0,
                "seed {}, {block:?} has no exposed negative-Y faces",
                terrain.seed()
            );
        }
        assert!(
            dark_cave_faces > 0,
            "seed {}, no roofed cave darkness",
            terrain.seed()
        );
        for (material, points) in [Block::Moss, Block::Crystal]
            .into_iter()
            .zip(clustered_materials)
        {
            let total = points.len();
            let clusters = exposed_clusters(&world, material, points);
            let clustered: usize = clusters.iter().filter(|&&count| count >= 4).sum();
            assert!(
                total >= 20 && clusters.iter().any(|&count| count >= 8) && clustered * 2 >= total,
                "seed {}, {material:?} exposed={total}, cluster sizes={clusters:?}",
                terrain.seed()
            );
        }
    }
}

#[test]
fn observed_negative_y_passage_agrees_at_chunk_seams_and_can_be_walked() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let world = GeneratedWorld::new(terrain);
        let (a, b) = observed_seam_passage(&world).unwrap_or_else(|| {
            panic!("seed {} has no observed walkable cave seam", terrain.seed())
        });
        for x in a[0].min(b[0]) - 1..=a[0].max(b[0]) + 1 {
            for z in a[2].min(b[2]) - 1..=a[2].max(b[2]) + 1 {
                for y in a[1] - 2..=a[1] + 3 {
                    assert_eq!(
                        world.block([x, y, z]),
                        terrain.block_at(x, y, z),
                        "seed {}, cave seam at ({x}, {y}, {z})",
                        terrain.seed()
                    );
                }
            }
        }
        let feet = Vec3::new(a[0] as f32 + 0.5, a[1] as f32, a[2] as f32 + 0.5);
        let destination = Vec3::new(b[0] as f32 + 0.5, b[1] as f32, b[2] as f32 + 0.5);
        let direction = (destination - feet).normalize();
        let mut player = Player::new(feet);
        player.yaw = direction.x.atan2(-direction.z);
        for _ in 0..24 {
            player.update(
                MovementInput {
                    forward: true,
                    ..Default::default()
                },
                DT,
                |x, y, z| world.solid(x, y, z),
            );
            assert_safe(&world, &player);
        }
        assert!(player.is_grounded());
        assert_position(player.position, destination);
        assert_ne!(
            ChunkCoord::from_world(feet.x, feet.z),
            ChunkCoord::from_world(player.position.x, player.position.z)
        );
    }
}

#[test]
fn underground_liquids_do_not_collide_but_ores_and_new_geology_are_walkable_solids() {
    let terrain = Terrain::default();
    let world = GeneratedWorld::new(terrain);
    let mut found = [None; 9];
    let materials = [
        Block::Water,
        Block::Lava,
        Block::CoalOre,
        Block::IronOre,
        Block::GoldOre,
        Block::Limestone,
        Block::Slate,
        Block::Basalt,
        Block::Calcite,
    ];
    for coord in world.feature_coords() {
        let chunk = Chunk::generate(&terrain, coord);
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                for y in MIN_WORLD_Y + 1..-2 {
                    let block = chunk.block(local_x, y, local_z);
                    if let Some(index) = materials.iter().position(|&material| material == block) {
                        found[index].get_or_insert([
                            coord.x * CHUNK_SIZE + local_x,
                            y,
                            coord.z * CHUNK_SIZE + local_z,
                        ]);
                    }
                }
            }
        }
    }
    for (material, point) in materials.into_iter().zip(found) {
        let point = point.unwrap_or_else(|| panic!("no generated {material:?} sample"));
        assert_eq!(world.block(point), material);
        let liquid = material.is_liquid();
        assert_eq!(material.is_solid(), !liquid);
        assert_eq!(material.is_opaque(), material != Block::Water);
        let feet = Vec3::new(
            point[0] as f32 + 0.5,
            point[1] as f32 - 0.9,
            point[2] as f32 + 0.5,
        );
        assert_eq!(
            body_is_clear(feet, |sample| {
                if sample == point {
                    world.block(sample)
                } else {
                    Block::Air
                }
            }),
            liquid
        );
        let mut player = Player::new(feet);
        player.toggle_flight(|x, y, z| [x, y, z] == point && world.solid(x, y, z));
        player.toggle_flight(|x, y, z| [x, y, z] == point && world.solid(x, y, z));
        assert!(!player.flying);
        if liquid {
            assert_position(player.position, feet);
            assert!(!player.is_grounded());
        } else {
            assert_position(player.position, feet.with_y((point[1] + 1) as f32));
            assert!(player.is_grounded());
        }
        assert!(body_is_clear(player.position, |sample| {
            if sample == point {
                world.block(sample)
            } else {
                Block::Air
            }
        }));
    }
}
