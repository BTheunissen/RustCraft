use super::*;

#[test]
fn independently_meshed_negative_chunk_seams_share_corner_wave_inputs() {
    let profiles: Vec<_> = (-3..=0)
        .flat_map(|z| {
            (-3..=2).map(move |x| {
                let mut profile = water_profile(3.45, WaterBiome::Ocean);
                profile.flow = [x as f32 * 0.12, 0.0, z as f32 * 0.07];
                profile.salinity = (x + 3) as f32 / 5.0;
                ([x, z], profile)
            })
        })
        .collect();
    let first = profile_mesh([-2, 0, -2], [2, 5, 2], &profiles);
    let second = profile_mesh([0, 0, -2], [2, 5, 2], &profiles);
    assert_valid_mesh(&first.water);
    assert_valid_mesh(&second.water);
    let first_corners: Vec<_> = first
        .water
        .vertices
        .iter()
        .filter(|vertex| vertex.normal[1] == 1.0 && vertex.position[0] == 0.0)
        .collect();
    assert!(!first_corners.is_empty());
    for left in first_corners {
        let right = second
            .water
            .vertices
            .iter()
            .find(|vertex| vertex.normal[1] == 1.0 && vertex.position == left.position)
            .unwrap();
        assert_eq!(left.water, right.water);
        assert_eq!(left.water_extra, right.water_extra);
        assert_eq!(left.water_extra[3], 1.0);
        let height = |vertex: &Vertex| {
            crate::water::wave_displacement(
                [vertex.position[0], vertex.position[2]],
                3.6,
                vertex.water[3],
                vertex.water[2],
                vertex.water_extra[3],
            )
        };
        assert_eq!(height(left), height(right));
    }
    for mesh in [&first, &second] {
        assert!(faces(&mesh.water).iter().all(|face| {
            face[0].normal[0] == 0.0 || !face.iter().all(|vertex| vertex.position[0] == 0.0)
        }));
    }
}

#[test]
fn generated_mesh_halo_batches_every_needed_ray_without_repeating_corners() {
    let terrain = Terrain::default();
    let chunk = Chunk::generate(&terrain, ChunkCoord::new(-1, -2));
    let positions = occupied_halo_columns(&chunk, [-CHUNK_SIZE, -2 * CHUNK_SIZE]);
    assert_eq!(positions.len(), (CHUNK_SIZE * 4) as usize);
    let distinct: std::collections::HashSet<_> = positions.iter().copied().collect();
    assert_eq!(distinct.len(), positions.len());
    let blocks = terrain.column_block_batch(&positions);
    for (&[x, z], column) in positions.iter().zip(blocks) {
        for y in [MIN_WORLD_Y, -54, -25, 0, 60] {
            assert_eq!(
                column[(y - MIN_WORLD_Y) as usize],
                terrain.block_at(x, y, z)
            );
        }
    }
    let empty = Chunk::generate(&terrain, ChunkCoord::new(i32::MAX, i32::MAX));
    assert!(occupied_halo_columns(&empty, [0, 0]).is_empty());
}

#[test]
fn negative_chunk_boundaries_use_world_neighbors_for_occlusion() {
    let origin = [-2 * CHUNK_SIZE, MIN_WORLD_Y, -3 * CHUNK_SIZE];
    for (local, offset) in [
        ([0, 5, 8], [-1, 0, 0]),
        ([CHUNK_SIZE - 1, 5, 8], [1, 0, 0]),
        ([8, 5, 0], [0, 0, -1]),
        ([8, 5, CHUNK_SIZE - 1], [0, 0, 1]),
    ] {
        let world: [i32; 3] = std::array::from_fn(|axis| origin[axis] + local[axis]);
        let neighbor: [i32; 3] = std::array::from_fn(|axis| world[axis] + offset[axis]);
        for outside in [
            Block::Stone,
            Block::Leaves,
            Block::Water,
            Block::Air,
            Block::Bedrock,
            Block::Lava,
        ] {
            let mut queries = Vec::new();
            let mesh = mesh_volume(
                origin,
                [CHUNK_SIZE, WORLD_DEPTH, CHUNK_SIZE],
                |position| {
                    if position == local {
                        Block::Stone
                    } else {
                        Block::Air
                    }
                },
                |position| {
                    queries.push(position);
                    outside
                },
            );
            assert_eq!(queries.len(), WORLD_DEPTH as usize);
            queries.sort();
            assert_eq!(
                queries,
                (MIN_WORLD_Y..WORLD_HEIGHT)
                    .map(|y| [neighbor[0], y, neighbor[2]])
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                face_count(&mesh.opaque),
                if outside.is_opaque() { 5 } else { 6 }
            );
            assert_valid_mesh(&mesh.opaque);
            let interface: [f32; 3] =
                std::array::from_fn(|axis| world[axis] as f32 + 0.5 + offset[axis] as f32 * 0.5);
            let has_interface = faces(&mesh.opaque)
                .iter()
                .any(|face| face_center(face) == interface);
            assert_eq!(has_interface, !outside.is_opaque());
        }
    }
}

#[test]
fn water_chunk_boundaries_only_expose_faces_to_air() {
    for outside in [
        Block::Air,
        Block::Water,
        Block::Stone,
        Block::Leaves,
        Block::Lava,
    ] {
        let mut queries = Vec::new();
        let mesh = mesh_volume(
            [-CHUNK_SIZE, MIN_WORLD_Y, -CHUNK_SIZE],
            [CHUNK_SIZE, WORLD_DEPTH, CHUNK_SIZE],
            |position| {
                if position == [CHUNK_SIZE - 1, 5, 8] {
                    Block::Water
                } else {
                    Block::Air
                }
            },
            |position| {
                queries.push(position);
                outside
            },
        );
        queries.sort();
        assert_eq!(
            queries,
            (MIN_WORLD_Y..WORLD_HEIGHT)
                .map(|y| [0, y, -CHUNK_SIZE + 8])
                .collect::<Vec<_>>()
        );
        assert_eq!(face_count(&mesh.opaque), 0);
        assert_eq!(
            face_count(&mesh.water),
            if outside == Block::Air { 6 } else { 5 }
        );
        assert_valid_mesh(&mesh.water);
    }
}

#[test]
fn generated_chunks_emit_every_exposed_face_and_no_occluded_faces() {
    let terrain = Terrain::new(0x5255_5354);
    for coord in [ChunkCoord::new(0, 0), ChunkCoord::new(-1, -2)] {
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk(&chunk, &terrain);
        assert_valid_mesh(&mesh.opaque);
        assert_valid_mesh(&mesh.water);
        assert!(mesh.opaque.vertices.iter().any(|v| v.position[1] < 0.0));
        assert!(
            mesh.opaque
                .vertices
                .iter()
                .chain(&mesh.water.vertices)
                .all(|v| { (MIN_WORLD_Y as f32..=WORLD_HEIGHT as f32).contains(&v.position[1]) })
        );
        let mut expected_opaque = 0;
        let mut expected_water = 0;

        for y in MIN_WORLD_Y..WORLD_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let block = chunk.block(x, y, z);
                    if block == Block::Air {
                        continue;
                    }
                    for face in &FACES {
                        let neighbor = terrain.block_at(
                            coord.x * CHUNK_SIZE + x + face.offset[0],
                            y + face.offset[1],
                            coord.z * CHUNK_SIZE + z + face.offset[2],
                        );
                        if block == Block::Water {
                            expected_water += usize::from(water_face_visible(
                                &terrain,
                                [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z],
                                face,
                            ));
                        } else {
                            expected_opaque += usize::from(!neighbor.is_opaque());
                        }
                    }
                }
            }
        }
        assert_eq!(face_count(&mesh.opaque), expected_opaque);
        assert_eq!(face_count(&mesh.water), expected_water);
        assert!(expected_opaque > 0);

        for (geometry, is_water) in [(&mesh.opaque, false), (&mesh.water, true)] {
            for face in faces(geometry) {
                let block_position = source_block(face);
                let block = chunk.block(
                    block_position[0] - coord.x * CHUNK_SIZE,
                    block_position[1],
                    block_position[2] - coord.z * CHUNK_SIZE,
                );
                assert_eq!(block == Block::Water, is_water);
                assert!(block.is_opaque() || block == Block::Water);
                assert_eq!(
                    face[0].material,
                    material_for_face(block, face[0].normal[1] as i32)
                );
                let neighbor_position: [i32; 3] =
                    std::array::from_fn(|axis| block_position[axis] + face[0].normal[axis] as i32);
                let neighbor = terrain.block_at(
                    neighbor_position[0],
                    neighbor_position[1],
                    neighbor_position[2],
                );
                if is_water {
                    assert!(
                        neighbor == Block::Air
                            || neighbor == Block::Water
                            || (face[0].normal[1] == 1.0
                                && face[0].position[1] < neighbor_position[1] as f32)
                    );
                    if neighbor == Block::Water {
                        let water = terrain
                            .water_at(
                                neighbor_position[0],
                                neighbor_position[1],
                                neighbor_position[2],
                            )
                            .unwrap();
                        let upper = face
                            .iter()
                            .map(|vertex| vertex.position[1])
                            .fold(f32::MIN, f32::max);
                        let lower = face
                            .iter()
                            .map(|vertex| vertex.position[1])
                            .fold(f32::MAX, f32::min);
                        assert!(water.surface < upper && water.surface <= lower);
                    }
                    if face[0].normal[1] == 1.0 {
                        let water = terrain
                            .water_at(block_position[0], block_position[1], block_position[2])
                            .unwrap();
                        assert!(
                            face.iter()
                                .all(|vertex| vertex.position[1] == water.surface)
                        );
                    }
                } else {
                    assert!(!neighbor.is_opaque());
                }
                if neighbor_position[1] < MIN_WORLD_Y {
                    assert_eq!(face[0].light, 0.0);
                } else {
                    let expected = reference_daylight(neighbor_position[1], |y| {
                        terrain.block_at(neighbor_position[0], y, neighbor_position[2])
                    });
                    assert_eq!(face[0].light, expected, "neighbor {neighbor_position:?}");
                }
            }
        }
    }
}

#[test]
fn adjoining_generated_chunks_cull_both_sides_of_their_shared_seam() {
    let terrain = Terrain::new(7);
    let first = ChunkCoord::new(-1, -1);
    for (axis, second) in [(0, ChunkCoord::new(0, -1)), (2, ChunkCoord::new(-1, 0))] {
        let first_chunk = Chunk::generate(&terrain, first);
        let second_chunk = Chunk::generate(&terrain, second);
        let first_mesh = mesh_chunk(&first_chunk, &terrain);
        let second_mesh = mesh_chunk(&second_chunk, &terrain);
        let seam_faces = |geometry: &Mesh, normal: f32| {
            faces(geometry)
                .iter()
                .filter(|face| {
                    face[0].normal[axis] == normal
                        && face.iter().all(|vertex| vertex.position[axis] == 0.0)
                })
                .count()
        };
        let mut shared_opaque_blocks = 0;
        let mut expected_first = [0; 2];
        let mut expected_second = [0; 2];
        for y in MIN_WORLD_Y..WORLD_HEIGHT {
            for horizontal in 0..CHUNK_SIZE {
                let (a, b) = if axis == 0 {
                    (
                        first_chunk.block(CHUNK_SIZE - 1, y, horizontal),
                        second_chunk.block(0, y, horizontal),
                    )
                } else {
                    (
                        first_chunk.block(horizontal, y, CHUNK_SIZE - 1),
                        second_chunk.block(horizontal, y, 0),
                    )
                };
                shared_opaque_blocks += usize::from(a.is_opaque() && b.is_opaque());
                expected_first[0] += usize::from(a.is_opaque() && !b.is_opaque());
                let first_position = if axis == 0 {
                    [-1, y, -CHUNK_SIZE + horizontal]
                } else {
                    [-CHUNK_SIZE + horizontal, y, -1]
                };
                let mut second_position = first_position;
                second_position[axis] += 1;
                let forward = if axis == 0 { &FACES[0] } else { &FACES[4] };
                let backward = if axis == 0 { &FACES[1] } else { &FACES[5] };
                expected_first[1] += usize::from(
                    a == Block::Water && water_face_visible(&terrain, first_position, forward),
                );
                expected_second[0] += usize::from(b.is_opaque() && !a.is_opaque());
                expected_second[1] += usize::from(
                    b == Block::Water && water_face_visible(&terrain, second_position, backward),
                );
            }
        }
        assert!(shared_opaque_blocks > 0);
        assert_eq!(seam_faces(&first_mesh.opaque, 1.0), expected_first[0]);
        assert_eq!(seam_faces(&first_mesh.water, 1.0), expected_first[1]);
        assert_eq!(seam_faces(&second_mesh.opaque, -1.0), expected_second[0]);
        assert_eq!(seam_faces(&second_mesh.water, -1.0), expected_second[1]);
        for (mesh, neighbor_chunk, normal) in [
            (&first_mesh, &second_chunk, 1.0),
            (&second_mesh, &first_chunk, -1.0),
        ] {
            let neighbor_coord = neighbor_chunk.coord();
            for geometry in [&mesh.opaque, &mesh.water] {
                for face in faces(geometry).iter().filter(|face| {
                    face[0].normal[axis] == normal
                        && face.iter().all(|vertex| vertex.position[axis] == 0.0)
                }) {
                    let source = source_block(face);
                    let neighbor: [i32; 3] =
                        std::array::from_fn(|i| source[i] + face[0].normal[i] as i32);
                    let expected = reference_daylight(neighbor[1], |y| {
                        neighbor_chunk.block(
                            neighbor[0] - neighbor_coord.x * CHUNK_SIZE,
                            y,
                            neighbor[2] - neighbor_coord.z * CHUNK_SIZE,
                        )
                    });
                    assert_eq!(face[0].light, expected, "seam neighbor {neighbor:?}");
                }
            }
        }
    }
}

#[test]
fn unrepresentable_chunk_origins_have_empty_meshes() {
    let terrain = Terrain::new(1);
    for coord in [
        ChunkCoord::new(i32::MAX, 0),
        ChunkCoord::new(i32::MIN, 0),
        ChunkCoord::new(0, i32::MAX),
        ChunkCoord::new(0, i32::MIN),
    ] {
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk(&chunk, &terrain);
        assert_eq!(face_count(&mesh.opaque), 0);
        assert_eq!(face_count(&mesh.water), 0);
    }
}

#[test]
fn neighbors_beyond_the_i32_world_are_air_without_overflow() {
    for edge in [i32::MIN, i32::MAX] {
        let queries = Cell::new(0);
        let mesh = mesh_volume(
            [edge, 0, edge],
            [1; 3],
            |_| Block::Stone,
            |_| {
                queries.set(queries.get() + 1);
                Block::Stone
            },
        );
        assert_eq!(queries.get(), 4);
        assert_eq!(face_count(&mesh.opaque), 2);
        assert_eq!(face_count(&mesh.water), 0);
    }
}
