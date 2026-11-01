use super::*;

#[test]
fn vertex_layout_matches_the_renderer_contract() {
    assert_eq!(std::mem::size_of::<Vertex>(), 84);
    assert_eq!(std::mem::align_of::<Vertex>(), 4);
    assert_eq!(std::mem::offset_of!(Vertex, position), 0);
    assert_eq!(std::mem::offset_of!(Vertex, normal), 12);
    assert_eq!(std::mem::offset_of!(Vertex, uv), 24);
    assert_eq!(std::mem::offset_of!(Vertex, material), 32);
    assert_eq!(std::mem::offset_of!(Vertex, light), 36);
    assert_eq!(std::mem::offset_of!(Vertex, water), 40);
    assert_eq!(std::mem::offset_of!(Vertex, water_extra), 56);
    assert_eq!(std::mem::offset_of!(Vertex, local_light), 72);
    let zeroed: Vertex = bytemuck::Zeroable::zeroed();
    assert!(bytemuck::bytes_of(&zeroed).iter().all(|byte| *byte == 0));
}

#[test]
fn empty_volume_emits_no_geometry_or_boundary_queries() {
    let mesh = mesh_volume(
        [0; 3],
        [3; 3],
        |_| Block::Air,
        |_| panic!("empty blocks must not sample outside the volume"),
    );
    assert_eq!(face_count(&mesh.opaque), 0);
    assert_eq!(face_count(&mesh.water), 0);
}

#[test]
fn non_positive_dimensions_emit_nothing_without_sampling() {
    for size in [[0, 3, 3], [3, 0, 3], [3, 3, 0], [-1, 3, 3]] {
        let mesh = mesh_volume(
            [0; 3],
            size,
            |_| panic!("an empty volume must not sample local blocks"),
            |_| panic!("an empty volume must not sample neighbors"),
        );
        assert_eq!(face_count(&mesh.opaque), 0);
        assert_eq!(face_count(&mesh.water), 0);
    }
}

#[test]
fn isolated_cube_has_six_outward_faces_and_valid_triangles() {
    let mesh = mesh_volume(
        [-5, 0, -9],
        [3; 3],
        |position| {
            if position == [1; 3] {
                Block::Stone
            } else {
                Block::Air
            }
        },
        |_| panic!("interior neighbors must use dense local blocks"),
    );
    assert_eq!(face_count(&mesh.opaque), 6);
    assert_eq!(face_count(&mesh.water), 0);
    assert_valid_mesh(&mesh.opaque);

    for face in faces(&mesh.opaque) {
        let center = face_center(face);
        for (axis, coordinate) in center.into_iter().enumerate() {
            assert_eq!(
                coordinate,
                [-3.5, 1.5, -7.5][axis] + face[0].normal[axis] * 0.5
            );
        }
        for vertex in face {
            assert_eq!(vertex.material, 3);
            if vertex.normal[1] == 0.0 {
                assert_eq!(vertex.uv[1], 2.0 - vertex.position[1]);
            }
        }
    }
}

#[test]
fn adjacent_cubes_cull_shared_faces_along_every_axis() {
    for offset in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let neighbor = offset.map(|axis| 1 + axis);
        let mesh = mesh_blocks([3; 3], &[([1; 3], Block::Stone), (neighbor, Block::Stone)]);
        assert_eq!(face_count(&mesh.opaque), 10, "offset: {offset:?}");
        assert_valid_mesh(&mesh.opaque);
        let interface = offset.map(|axis| 1.5 + axis as f32 * 0.5);
        assert!(
            faces(&mesh.opaque)
                .iter()
                .all(|face| face_center(face) != interface)
        );
    }
}

#[test]
fn solid_volume_only_samples_its_boundary_and_emits_its_shell() {
    let local_queries = Cell::new(0);
    let mut outside_queries = Vec::new();
    let mesh = mesh_volume(
        [-3, 0, -3],
        [3; 3],
        |position| {
            assert!(position.into_iter().all(|axis| (0..3).contains(&axis)));
            local_queries.set(local_queries.get() + 1);
            Block::Stone
        },
        |position| {
            let local: [i32; 3] = std::array::from_fn(|axis| position[axis] - [-3, 0, -3][axis]);
            assert_eq!(
                local.iter().filter(|axis| !(0..3).contains(*axis)).count(),
                1
            );
            outside_queries.push(position);
            Block::Air
        },
    );
    assert_eq!(local_queries.get(), 27);
    assert_eq!(outside_queries.len(), 54);
    assert_eq!(
        outside_queries
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        outside_queries.len()
    );
    assert_eq!(face_count(&mesh.opaque), 54);
    assert_valid_mesh(&mesh.opaque);
    for face in faces(&mesh.opaque) {
        assert!((0..3).any(|axis| {
            let origin = [-3.0, 0.0, -3.0][axis];
            face.iter().all(|v| v.position[axis] == origin)
                || face.iter().all(|v| v.position[axis] == origin + 3.0)
        }));
    }
}

#[test]
fn fully_occluded_volume_emits_no_faces() {
    let mesh = mesh_volume([0; 3], [3; 3], |_| Block::Stone, |_| Block::Stone);
    assert_eq!(face_count(&mesh.opaque), 0);
    assert_eq!(face_count(&mesh.water), 0);
}

#[test]
fn atlas_materials_distinguish_tops_bottoms_and_sides() {
    for (block, side, top, bottom) in [
        (Block::Grass, 1, 0, 2),
        (Block::Dirt, 2, 2, 2),
        (Block::Stone, 3, 3, 3),
        (Block::Sand, 4, 4, 4),
        (Block::Snow, 5, 5, 5),
        (Block::Wood, 6, 7, 7),
        (Block::Leaves, 8, 8, 8),
        (Block::Water, 9, 9, 9),
        (Block::Bedrock, 10, 10, 10),
        (Block::DeepStone, 11, 11, 11),
        (Block::CoalOre, 12, 12, 12),
        (Block::IronOre, 13, 13, 13),
        (Block::GoldOre, 14, 14, 14),
        (Block::Crystal, 15, 15, 15),
        (Block::Moss, 16, 16, 16),
        (Block::Lava, 17, 17, 17),
        (Block::Limestone, 18, 18, 18),
        (Block::Slate, 19, 19, 19),
        (Block::Basalt, 20, 20, 20),
        (Block::Calcite, 21, 21, 21),
        (Block::Gravel, 22, 22, 22),
        (Block::Clay, 23, 23, 23),
    ] {
        let mesh = mesh_blocks([1; 3], &[([0; 3], block)]);
        let (drawn, other) = if block == Block::Water {
            (&mesh.water, &mesh.opaque)
        } else {
            (&mesh.opaque, &mesh.water)
        };
        assert_eq!(face_count(drawn), 6, "block: {block:?}");
        assert_eq!(face_count(other), 0);
        assert_valid_mesh(drawn);
        for vertex in &drawn.vertices {
            let expected = if vertex.normal[1] == 1.0 {
                top
            } else if vertex.normal[1] == -1.0 {
                bottom
            } else {
                side
            };
            assert_eq!(vertex.material, expected, "block: {block:?}");
        }
    }
}

#[test]
fn leaves_are_opaque_and_hide_shared_faces() {
    let mesh = mesh_blocks(
        [2, 1, 1],
        &[([0, 0, 0], Block::Stone), ([1, 0, 0], Block::Leaves)],
    );
    assert_eq!(face_count(&mesh.opaque), 10);
    assert_eq!(face_count(&mesh.water), 0);
    assert_eq!(
        mesh.opaque
            .vertices
            .iter()
            .filter(|v| v.material == 8)
            .count(),
        20
    );
    assert_valid_mesh(&mesh.opaque);
}

#[test]
fn lava_and_new_solids_hide_their_shared_opaque_faces() {
    for neighbor in [
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
        Block::Gravel,
        Block::Clay,
    ] {
        let mesh = mesh_blocks(
            [2, 1, 1],
            &[([0, 0, 0], Block::Lava), ([1, 0, 0], neighbor)],
        );
        assert_eq!(face_count(&mesh.opaque), 10, "neighbor: {neighbor:?}");
        assert_eq!(face_count(&mesh.water), 0);
        assert_valid_mesh(&mesh.opaque);
        assert!(
            faces(&mesh.opaque)
                .iter()
                .all(|face| face_center(face) != [1.0, 0.5, 0.5])
        );
    }
}
