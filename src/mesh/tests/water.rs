use super::*;

mod ceilings;

#[test]
fn fractional_water_caps_and_side_edges_share_the_mean_surface() {
    let profile = water_profile(2.35, WaterBiome::River);
    let mesh = profile_mesh([0; 3], [3, 4, 3], &[([1, 1], profile)]);
    assert_eq!(face_count(&mesh.water), 9);
    assert_valid_mesh(&mesh.water);
    assert_valid_mesh(&mesh.opaque);
    let top = faces(&mesh.water)
        .iter()
        .find(|face| face[0].normal[1] == 1.0)
        .unwrap();
    assert!(
        top.iter()
            .all(|vertex| vertex.position[1] == profile.surface)
    );
    assert!(top.iter().all(|vertex| {
        vertex.water[0] == 0.35
            && vertex.water[1] == -0.2
            && (vertex.water[2] - 1.35).abs() < 0.000001
            && vertex.water[3] == 0.0
    }));
    assert!(
        top.iter()
            .all(|vertex| vertex.water_extra[1] == WaterBiome::River as u8 as f32)
    );
    assert!(
        top.iter()
            .all(|vertex| vertex.water_extra[2] > 0.0 && vertex.water_extra[3] == 0.0)
    );
    assert!(
        mesh.water
            .vertices
            .iter()
            .all(|vertex| vertex.position[1] <= profile.surface)
    );
    let upper_sides: Vec<_> = mesh
        .water
        .vertices
        .iter()
        .filter(|vertex| vertex.normal[1] == 0.0 && vertex.position[1] == profile.surface)
        .collect();
    assert_eq!(upper_sides.len(), 8);
    for side in upper_sides {
        let top_corner = top
            .iter()
            .find(|vertex| vertex.position == side.position)
            .unwrap();
        assert_eq!(side.water, top_corner.water);
        assert_eq!(side.water_extra, top_corner.water_extra);
    }
}

#[test]
fn fractional_neighbors_emit_only_the_upper_exposed_strip() {
    let first = water_profile(1.75, WaterBiome::Lake);
    let mut second = water_profile(1.25, WaterBiome::Lake);
    second.flow = [-0.15, 0.0, 0.1];
    let mesh = profile_mesh([0; 3], [2, 3, 1], &[([0, 0], first), ([1, 0], second)]);
    assert_valid_mesh(&mesh.water);
    let interfaces: Vec<_> = faces(&mesh.water)
        .iter()
        .filter(|face| {
            face.iter().all(|vertex| vertex.position[0] == 1.0) && face[0].normal[0].abs() == 1.0
        })
        .collect();
    assert_eq!(interfaces.len(), 1);
    let interface = interfaces[0];
    assert_eq!(interface[0].normal, [1.0, 0.0, 0.0]);
    assert_eq!(
        interface
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::MAX, f32::min),
        second.surface
    );
    assert_eq!(
        interface
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::MIN, f32::max),
        first.surface
    );
    assert!(interface.iter().all(|vertex| vertex.water_extra[3] == 0.0));

    let same = profile_mesh([0; 3], [2, 3, 1], &[([0, 0], first), ([1, 0], first)]);
    assert_eq!(face_count(&mesh.water), face_count(&same.water) + 1);
    assert!(faces(&same.water).iter().all(|face| {
        face[0].normal[0] == 0.0 || !face.iter().all(|vertex| vertex.position[0] == 1.0)
    }));
}

#[test]
fn submillimeter_water_steps_are_not_erased_into_unsealed_edges() {
    let first = water_profile(1.75, WaterBiome::River);
    let second = water_profile(1.74998, WaterBiome::River);
    let mesh = profile_mesh([0; 3], [2, 3, 1], &[([0, 0], first), ([1, 0], second)]);
    let interface = faces(&mesh.water)
        .iter()
        .find(|face| face[0].normal == [1.0, 0.0, 0.0] && face[0].position[0] == 1.0)
        .unwrap();
    assert_valid_mesh(&mesh.water);
    assert_eq!(interface[0].position[1], second.surface);
    assert_eq!(interface[2].position[1], first.surface);
}

#[test]
fn water_metadata_scans_actual_runs_once_and_sanitizes_profile_inputs() {
    let blocks = [
        Block::Clay,
        Block::Water,
        Block::Water,
        Block::Air,
        Block::Stone,
        Block::Water,
        Block::Water,
        Block::Stone,
    ];
    let calls = Cell::new(0);
    let mut metadata = WaterMetadata::default();
    metadata.add_column([-17, -1], -4, &blocks, |last| {
        calls.set(calls.get() + 1);
        (last == -2).then_some(WaterSample {
            bottom: -30,
            surface: -1.4,
            flow: [f32::NAN, -20.0, f32::INFINITY],
            biome: WaterBiome::Estuary,
            salinity: 2.0,
        })
    });
    assert_eq!(calls.get(), 2);
    let first = metadata.at([-17, -3, -1]).unwrap();
    assert_eq!(first.first, -3);
    assert_eq!(first.last, -2);
    assert_eq!(first.sample.bottom, -4);
    assert_eq!(first.sample.surface, -1.4);
    assert_eq!(first.sample.flow, [0.0, -8.0, 0.0]);
    assert_eq!(first.sample.salinity, 1.0);
    assert!(first.exposed);
    let second = metadata.at([-17, 1, -1]).unwrap();
    assert_eq!(second.sample.biome, WaterBiome::CavePool);
    assert_eq!(second.sample.surface, 3.0);
    assert_eq!(second.sample.bottom, 0);
    assert!(!second.exposed);
    assert!(metadata.at([-17, -1, -1]).is_none());
    assert!(metadata.at([-16, -3, -1]).is_none());
    metadata.add_column([0, 0], 0, &[Block::Air, Block::Stone], |_| {
        panic!("dry columns need no metadata")
    });
    assert!(!metadata.columns.contains_key(&[0, 0]));
    metadata.add_column([1, 0], 0, &[Block::Water], |_| {
        Some(water_profile(f32::NAN, WaterBiome::Ocean))
    });
    assert_eq!(
        metadata.at([1, 0, 0]).unwrap().sample.biome,
        WaterBiome::CavePool
    );
}

#[test]
fn shared_corners_blend_current_and_salinity_but_attenuate_to_the_shallowest_bed() {
    let mut metadata = WaterMetadata::default();
    for (index, position) in [[-1, -1], [0, -1], [-1, 0], [0, 0]].into_iter().enumerate() {
        let mut profile = water_profile(4.35, WaterBiome::Ocean);
        profile.flow = [index as f32 * 0.2, 0.0, -(index as f32) * 0.1];
        profile.salinity = index as f32 / 3.0;
        let first = if index == 0 { 4 } else { 1 };
        let blocks: Vec<_> = (0..6)
            .map(|y| {
                if (first..=4).contains(&y) {
                    Block::Water
                } else if y < first {
                    Block::Clay
                } else {
                    Block::Air
                }
            })
            .collect();
        metadata.add_column(position, 0, &blocks, |_| Some(profile));
    }
    let corner = metadata.corner(0, 0, 4.35);
    assert!((corner.data[0] - 0.3).abs() < 0.000001);
    assert!((corner.data[1] + 0.15).abs() < 0.000001);
    assert!((corner.data[2] - 2.6).abs() < 0.000001);
    assert!((corner.data[3] - 0.5).abs() < 0.000001);
    assert!(corner.weight > 0.0 && corner.weight < 0.05);
    assert!(corner.foam > 0.4);
    assert_eq!(metadata.corners.len(), 1);
    assert_eq!(metadata.corner(0, 0, 4.35).data, corner.data);
    assert_eq!(metadata.corners.len(), 1);
    assert_eq!(metadata.corner(-1, -1, 4.35).weight, 0.0);
}

#[test]
fn descending_river_corners_blend_flow_without_painting_every_step_with_foam() {
    let profiles: Vec<_> = (-1..=2)
        .flat_map(|z| {
            (-1..=2).map(move |x| {
                let mut profile = water_profile(4.5 - x as f32 * 0.18, WaterBiome::River);
                profile.flow = [0.4 + x as f32 * 0.05, -0.02, z as f32 * 0.06];
                profile.salinity = (x + 1) as f32 * 0.1;
                ([x, z], profile)
            })
        })
        .collect();
    let mesh = profile_mesh([0; 3], [2, 6, 2], &profiles);
    assert_valid_mesh(&mesh.water);
    let corner_vertices: Vec<_> = mesh
        .water
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.normal[1] == 1.0 && vertex.position[0] == 1.0 && vertex.position[2] == 1.0
        })
        .collect();
    assert_eq!(corner_vertices.len(), 4);
    for vertex in corner_vertices {
        assert!((vertex.water[0] - 0.425).abs() < 0.000001);
        assert!((vertex.water[1] - 0.03).abs() < 0.000001);
        assert!((vertex.water[3] - 0.15).abs() < 0.000001);
        assert_eq!(vertex.water_extra[2], 0.0);
        assert_eq!(vertex.water_extra[3], 0.0);
    }
}

#[test]
fn falling_curtains_retain_downward_currents_and_foam_without_surface_waves() {
    let mut profile = water_profile(4.35, WaterBiome::Waterfall);
    profile.flow = [0.4, -3.0, 0.0];
    let mesh = profile_mesh([0; 3], [1, 6, 1], &[([0, 0], profile)]);
    assert_valid_mesh(&mesh.water);
    assert!(mesh.water.vertices.iter().all(|vertex| {
        vertex.water_extra[0] == -3.0
            && vertex.water_extra[1] == 4.0
            && vertex.water_extra[3] == 0.0
    }));
    assert!(
        mesh.water
            .vertices
            .iter()
            .any(|vertex| vertex.position[1] == 1.0 && vertex.water_extra[2] == 1.0)
    );
    assert!(
        mesh.water
            .vertices
            .iter()
            .any(|vertex| vertex.normal[1] == 0.0
                && vertex.position[1] > 3.0
                && vertex.water_extra[2] == 0.0)
    );
}

#[test]
fn roofed_water_does_not_move_and_open_cave_pools_use_sheltered_wave_weights() {
    let mut metadata = WaterMetadata::default();
    for position in [[-1, -1], [0, -1], [-1, 0], [0, 0]] {
        metadata.add_column(
            position,
            12,
            &[Block::Clay, Block::Water, Block::Water, Block::Stone],
            |_| {
                Some(WaterSample {
                    bottom: 12,
                    surface: 15.0,
                    flow: [0.0; 3],
                    biome: WaterBiome::CavePool,
                    salinity: 0.0,
                })
            },
        );
    }
    assert_eq!(metadata.corner(0, 0, 15.0).weight, 0.0);
    let mesh = mesh_volume_with_water(
        [-1, 12, -1],
        [2, 4, 2],
        |[_, y, _]| match y {
            0 => Block::Clay,
            1 | 2 => Block::Water,
            _ => Block::Stone,
        },
        |_| Block::Air,
        Some(&mut metadata),
        None,
        None,
    );
    assert_valid_mesh(&mesh.water);
    assert_valid_mesh(&mesh.opaque);
    assert!(
        mesh.water
            .vertices
            .iter()
            .all(|vertex| vertex.water_extra[3] == 0.0)
    );
    assert!(
        mesh.water
            .vertices
            .iter()
            .all(|vertex| vertex.normal[1] == 0.0)
    );
    let profiles: Vec<_> = (-1..=2)
        .flat_map(|z| (-1..=2).map(move |x| ([x, z], water_profile(3.0, WaterBiome::CavePool))))
        .collect();
    let open = profile_mesh([0; 3], [2, 4, 2], &profiles);
    assert!(
        open.water
            .vertices
            .iter()
            .any(|vertex| vertex.water_extra[3] == 0.2)
    );
    assert!(
        open.water
            .vertices
            .iter()
            .all(|vertex| vertex.water_extra[3] <= 0.2)
    );
}

#[test]
fn world_limit_water_corners_remain_finite_and_pinned() {
    let mut metadata = WaterMetadata::default();
    for edge in [i32::MIN, i32::MAX] {
        metadata.add_column(
            [edge, edge],
            -64,
            &[Block::Clay, Block::Water, Block::Air],
            |_| {
                Some(WaterSample {
                    bottom: -64,
                    surface: -62.0,
                    flow: [0.0; 3],
                    biome: WaterBiome::Ocean,
                    salinity: 1.0,
                })
            },
        );
        let corner = metadata.corner(i64::from(edge), i64::from(edge), -62.0);
        assert!(corner.data.into_iter().all(f32::is_finite));
        assert_eq!(corner.weight, 0.0);
        let outside = metadata.corner(i64::from(edge) + 1, i64::from(edge) + 1, -62.0);
        assert!(outside.data.into_iter().all(f32::is_finite));
        assert_eq!(outside.weight, 0.0);
    }
}

#[test]
fn water_neighbors_hide_shared_faces() {
    for offset in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
        let mesh = mesh_blocks([2; 3], &[([0; 3], Block::Water), (offset, Block::Water)]);
        assert_eq!(face_count(&mesh.opaque), 0);
        assert_eq!(face_count(&mesh.water), 10);
        assert_valid_mesh(&mesh.water);
        let interface = offset.map(|axis| 0.5 + axis as f32 * 0.5);
        assert!(
            faces(&mesh.water)
                .iter()
                .all(|face| face_center(face) != interface)
        );
    }
}

#[test]
fn enclosed_water_column_only_emits_its_surface() {
    let mesh = mesh_volume(
        [0; 3],
        [3; 3],
        |[x, y, z]| {
            if y == 0 || x != 1 || z != 1 {
                Block::Sand
            } else {
                Block::Water
            }
        },
        |_| Block::Air,
    );
    assert_eq!(face_count(&mesh.water), 1);
    assert_valid_mesh(&mesh.water);
    for vertex in &mesh.water.vertices {
        assert_eq!(vertex.position[1], 3.0);
        assert_eq!(vertex.normal, [0.0, 1.0, 0.0]);
    }
}

#[test]
fn opaque_water_contact_emits_only_the_opaque_interface() {
    let mesh = mesh_blocks(
        [2, 1, 1],
        &[([0, 0, 0], Block::Sand), ([1, 0, 0], Block::Water)],
    );
    assert_eq!(face_count(&mesh.opaque), 6);
    assert_eq!(face_count(&mesh.water), 5);
    assert_valid_mesh(&mesh.opaque);
    assert_valid_mesh(&mesh.water);
    assert!(
        faces(&mesh.opaque).iter().any(|face| {
            face_center(face) == [1.0, 0.5, 0.5] && face[0].normal == [1.0, 0.0, 0.0]
        })
    );
    assert!(
        faces(&mesh.water)
            .iter()
            .all(|face| face_center(face) != [1.0, 0.5, 0.5])
    );
}

#[test]
fn lava_water_contact_has_only_one_opaque_interface_along_every_axis() {
    for offset in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
        let mesh = mesh_blocks([2; 3], &[([0; 3], Block::Lava), (offset, Block::Water)]);
        assert_eq!(face_count(&mesh.opaque), 6);
        assert_eq!(face_count(&mesh.water), 5);
        assert_valid_mesh(&mesh.opaque);
        assert_valid_mesh(&mesh.water);
        let interface = offset.map(|axis| 0.5 + axis as f32 * 0.5);
        let contact: Vec<_> = faces(&mesh.opaque)
            .iter()
            .filter(|face| face_center(face) == interface)
            .collect();
        assert_eq!(contact.len(), 1);
        assert_eq!(contact[0][0].normal, offset.map(|axis| axis as f32));
        assert_eq!(contact[0][0].material, 17);
        assert!(
            faces(&mesh.water)
                .iter()
                .all(|face| face_center(face) != interface)
        );
    }
}

#[test]
fn water_surrounded_by_opaque_blocks_has_no_geometry() {
    let mesh = mesh_blocks(
        [3; 3],
        &[
            ([1, 1, 1], Block::Water),
            ([2, 1, 1], Block::Grass),
            ([0, 1, 1], Block::Dirt),
            ([1, 2, 1], Block::Stone),
            ([1, 0, 1], Block::Sand),
            ([1, 1, 2], Block::Snow),
            ([1, 1, 0], Block::Leaves),
        ],
    );
    assert_eq!(face_count(&mesh.water), 0);
    assert_eq!(face_count(&mesh.opaque), 36);
    assert_valid_mesh(&mesh.opaque);
}
