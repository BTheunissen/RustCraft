use super::*;

#[test]
fn gravel_and_clay_roofs_occlude_daylight_without_duplicate_water_interfaces() {
    for roof in [Block::Gravel, Block::Clay] {
        let mesh = mesh_blocks(
            [1, 4, 1],
            &[
                ([0, 0, 0], Block::Stone),
                ([0, 1, 0], Block::Water),
                ([0, 3, 0], roof),
            ],
        );
        let floor = faces(&mesh.opaque)
            .iter()
            .find(|face| face[0].normal[1] == 1.0 && face[0].position[1] == 1.0)
            .unwrap();
        assert_eq!(floor[0].light, 0.0);
        assert!(
            mesh.water
                .vertices
                .iter()
                .filter(|vertex| vertex.normal[1] == 1.0)
                .all(|vertex| vertex.light == 0.0)
        );
        assert!(
            faces(&mesh.water)
                .iter()
                .all(|face| face[0].normal[1] != -1.0)
        );
        assert_valid_mesh(&mesh.opaque);
        assert_valid_mesh(&mesh.water);
    }
}

#[test]
fn a_roofed_positive_y_cave_is_dark_but_its_rooftop_is_lit() {
    let origin = [-7, 12, -3];
    let mesh = mesh_volume(
        origin,
        [5, 6, 5],
        |[x, y, z]| {
            if y <= 4 && (y == 0 || y == 4 || x == 0 || x == 4 || z == 0 || z == 4) {
                Block::Stone
            } else {
                Block::Air
            }
        },
        |_| Block::Air,
    );
    assert_valid_mesh(&mesh.opaque);
    let interior: Vec<_> = faces(&mesh.opaque)
        .iter()
        .filter(|face| {
            let center = face_center(face);
            (origin[0] as f32 + 1.0..=origin[0] as f32 + 4.0).contains(&center[0])
                && (origin[1] as f32 + 1.0..=origin[1] as f32 + 4.0).contains(&center[1])
                && (origin[2] as f32 + 1.0..=origin[2] as f32 + 4.0).contains(&center[2])
        })
        .collect();
    assert_eq!(interior.len(), 54);
    assert!(interior.iter().all(|face| face[0].light == 0.0));
    let rooftop: Vec<_> = faces(&mesh.opaque)
        .iter()
        .filter(|face| {
            face[0].normal == [0.0, 1.0, 0.0] && face_center(face)[1] == origin[1] as f32 + 5.0
        })
        .collect();
    assert_eq!(rooftop.len(), 25);
    assert!(rooftop.iter().all(|face| face[0].light == 1.0));
}

#[test]
fn an_open_negative_y_pit_receives_daylight_instead_of_depth_darkness() {
    let origin = [-7, MIN_WORLD_Y + 10, -3];
    let mesh = mesh_volume(
        origin,
        [5, 6, 5],
        |[x, y, z]| {
            if y <= 4 && (y == 0 || x == 0 || x == 4 || z == 0 || z == 4) {
                Block::DeepStone
            } else {
                Block::Air
            }
        },
        |_| Block::Air,
    );
    assert_valid_mesh(&mesh.opaque);
    let floor: Vec<_> = faces(&mesh.opaque)
        .iter()
        .filter(|face| {
            face[0].normal == [0.0, 1.0, 0.0] && face_center(face)[1] == origin[1] as f32 + 1.0
        })
        .collect();
    assert_eq!(floor.len(), 9);
    assert!(floor.iter().all(|face| face[0].light == 1.0));
    assert!(mesh.opaque.vertices.iter().all(|v| v.position[1] < 0.0));
    assert_eq!(
        mesh.opaque
            .vertices
            .iter()
            .map(|v| v.position[1] as i32)
            .min(),
        Some(origin[1])
    );
}

#[test]
fn daylight_attenuates_through_canopies_and_water_above_clear_headroom() {
    let block_at = |[x, y, z]: [i32; 3]| {
        if y == 0 {
            Block::Stone
        } else if z == 1 && (3..=4).contains(&y) {
            match x {
                1 => Block::Leaves,
                2 => Block::Water,
                3 => Block::Stone,
                _ => Block::Air,
            }
        } else {
            Block::Air
        }
    };
    let mesh = mesh_volume([0; 3], [5, 6, 3], block_at, |_| Block::Air);
    assert_valid_mesh(&mesh.opaque);
    assert_valid_mesh(&mesh.water);
    for (x, expected) in [(0, 1.0), (1, 0.65 * 0.65), (2, 0.9 * 0.9), (3, 0.0)] {
        for y in 1..=2 {
            assert_eq!(block_at([x, y, 1]), Block::Air);
        }
        let floor = faces(&mesh.opaque)
            .iter()
            .find(|face| {
                face_center(face) == [x as f32 + 0.5, 1.0, 1.5] && face[0].normal == [0.0, 1.0, 0.0]
            })
            .unwrap();
        assert!((floor[0].light - expected).abs() < f32::EPSILON);
    }
}

#[test]
fn the_finite_world_underside_does_not_receive_skylight_from_outside_air() {
    let mesh = mesh_volume(
        [0, MIN_WORLD_Y, 0],
        [1; 3],
        |_| Block::Bedrock,
        |_| Block::Air,
    );
    assert_valid_mesh(&mesh.opaque);
    let bottom = faces(&mesh.opaque)
        .iter()
        .find(|face| face[0].normal == [0.0, -1.0, 0.0])
        .unwrap();
    assert_eq!(bottom[0].light, 0.0);
    assert!(bottom.iter().all(|v| v.position[1] == MIN_WORLD_Y as f32));
    let top = faces(&mesh.opaque)
        .iter()
        .find(|face| face[0].normal == [0.0, 1.0, 0.0])
        .unwrap();
    assert_eq!(top[0].light, 1.0);
}

#[test]
fn cached_boundary_columns_include_roofs_above_the_exposed_neighbor() {
    for (local, offset) in [
        ([0, 1, 1], [-1, 0, 0]),
        ([2, 1, 1], [1, 0, 0]),
        ([1, 1, 0], [0, 0, -1]),
        ([1, 1, 2], [0, 0, 1]),
    ] {
        let origin = [-4, 12, -4];
        let world: [i32; 3] = std::array::from_fn(|axis| origin[axis] + local[axis]);
        let neighbor: [i32; 3] = std::array::from_fn(|axis| world[axis] + offset[axis]);
        let mut queries = Vec::new();
        let mesh = mesh_volume(
            origin,
            [3, 6, 3],
            |position| {
                if position == local {
                    Block::Stone
                } else {
                    Block::Air
                }
            },
            |position| {
                queries.push(position);
                if position[1] == origin[1] + 4 {
                    Block::Stone
                } else {
                    Block::Air
                }
            },
        );
        assert_valid_mesh(&mesh.opaque);
        assert_eq!(queries.len(), 6);
        assert!(
            queries
                .iter()
                .all(|p| p[0] == neighbor[0] && p[2] == neighbor[2])
        );
        let shaded = faces(&mesh.opaque)
            .iter()
            .find(|face| face[0].normal == offset.map(|axis| axis as f32))
            .unwrap();
        assert_eq!(shaded[0].light, 0.0);
        let opposite = faces(&mesh.opaque)
            .iter()
            .find(|face| face[0].normal == offset.map(|axis| -axis as f32))
            .unwrap();
        assert_eq!(opposite[0].light, 1.0);
    }
}

#[test]
fn daylight_matches_when_a_column_is_local_or_in_an_adjacent_halo() {
    for axis in [0, 2] {
        let first_origin = [-4, MIN_WORLD_Y, -4];
        let mut second_origin = first_origin;
        second_origin[axis] += 4;
        let seam = second_origin[axis];
        let block_at = |position: [i32; 3]| {
            let along = position[2 - axis] - first_origin[2 - axis];
            let y = position[1] - MIN_WORLD_Y;
            if position[axis] == seam - 1 && (1..=3).contains(&y) {
                Block::DeepStone
            } else if position[axis] == seam {
                if y == 1 {
                    Block::Stone
                } else if y == 4 {
                    match along {
                        1 => Block::Leaves,
                        2 => Block::Water,
                        3 => Block::Stone,
                        _ => Block::Air,
                    }
                } else {
                    Block::Air
                }
            } else {
                Block::Air
            }
        };
        let make_mesh = |origin: [i32; 3]| {
            mesh_volume(
                origin,
                [4, WORLD_DEPTH, 4],
                |local| block_at(std::array::from_fn(|i| origin[i] + local[i])),
                block_at,
            )
        };
        let first = make_mesh(first_origin);
        let second = make_mesh(second_origin);
        assert_valid_mesh(&first.opaque);
        assert_valid_mesh(&second.opaque);
        assert_valid_mesh(&second.water);
        for (along, expected) in [(0, 1.0), (1, 0.65), (2, 0.9), (3, 0.0)] {
            let mut target = second_origin;
            target[1] += 2;
            target[2 - axis] += along;
            let mut normal = [0.0; 3];
            normal[axis] = 1.0;
            let side_center: [f32; 3] =
                std::array::from_fn(|i| target[i] as f32 + if i == axis { 0.0 } else { 0.5 });
            let halo_face = faces(&first.opaque)
                .iter()
                .find(|face| face_center(face) == side_center && face[0].normal == normal)
                .unwrap();
            let local_center = target.map(|coordinate| coordinate as f32 + 0.5);
            let local_face = faces(&second.opaque)
                .iter()
                .find(|face| {
                    face_center(face) == [local_center[0], local_center[1] - 0.5, local_center[2]]
                        && face[0].normal == [0.0, 1.0, 0.0]
                })
                .unwrap();
            assert_eq!(halo_face[0].light, expected, "axis {axis}, column {along}");
            assert_eq!(halo_face[0].light, local_face[0].light);
        }
    }
}
