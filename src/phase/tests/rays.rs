use super::*;

#[test]
fn homogeneous_buried_rock_has_four_texturable_faces() {
    let eye = Vec3::splat(0.5);
    let mut visited = Vec::new();
    let layers = phase_layers(eye, Vec3::X, PHASE_RANGE, |x, y, z| {
        visited.push([x, y, z]);
        Some(Block::Stone)
    });

    assert_eq!(layers.len(), PHASE_LAYERS);
    for (index, layer) in layers.iter().enumerate() {
        assert_close(layer.distance, index as f32 + 0.5);
        assert_eq!(layer.block, Block::Stone);
        assert_eq!(layer.normal, Vec3::NEG_X);
        assert_eq!(layer.position, Vec3::new(index as f32 + 1.0, 0.5, 0.5));
    }
    assert_eq!(visited, (0..=4).map(|x| [x, 0, 0]).collect::<Vec<_>>());
}

#[test]
fn transitions_use_each_leaving_geological_material() {
    let materials = [Block::Stone, Block::IronOre, Block::Slate, Block::Basalt];
    let layers = phase_layers(Vec3::splat(0.5), Vec3::X * 7.0, PHASE_RANGE, |x, _, _| {
        Some(materials.get(x as usize).copied().unwrap_or(Block::Air))
    });

    assert_eq!(layers.len(), materials.len());
    for (index, (layer, block)) in layers.iter().zip(materials).enumerate() {
        assert_eq!(layer.block, block);
        assert_close(layer.distance, index as f32 + 0.5);
        assert_eq!(layer.normal, Vec3::NEG_X);
    }
}

#[test]
fn every_solid_material_phases_but_air_and_liquids_do_not() {
    for block in Block::ALL {
        let layers = phase_layers(Vec3::splat(0.5), Vec3::NEG_Z, PHASE_RANGE, |_, _, _| {
            Some(block)
        });
        if block.is_solid() {
            assert_eq!(layers.len(), PHASE_LAYERS, "{block:?}");
            assert!(layers.iter().all(|layer| layer.block == block));
        } else {
            assert!(layers.is_empty(), "{block:?}");
        }
    }
}

#[test]
fn a_one_block_wall_stops_at_the_nearest_cavity_or_liquid() {
    for exit in [Block::Air, Block::Water, Block::Lava] {
        let mut visited = Vec::new();
        let layers = phase_layers(Vec3::splat(0.5), Vec3::X, PHASE_RANGE, |x, _, _| {
            visited.push(x);
            Some(if x == 1 { exit } else { Block::Stone })
        });

        assert_eq!(layers.len(), 1, "{exit:?}");
        assert_close(layers[0].distance, 0.5);
        assert_eq!(layers[0].block, Block::Stone);
        assert_eq!(visited, [0, 1]);
    }
}

#[test]
fn unknown_stops_after_the_last_known_face_without_sampling_beyond_it() {
    let mut visited = Vec::new();
    let layers = phase_layers(Vec3::splat(0.5), Vec3::X, PHASE_RANGE, |x, _, _| {
        visited.push(x);
        (x < 2).then_some(Block::CoalOre)
    });

    assert_eq!(layers.len(), 2);
    assert_eq!(visited, [0, 1, 2]);
    assert!(layers.iter().all(|layer| layer.block == Block::CoalOre));
    assert_close(layers[1].distance, 1.5);
    assert!(phase_layers(Vec3::splat(0.5), Vec3::X, PHASE_RANGE, |_, _, _| None).is_empty());
}

#[test]
fn six_axis_rays_keep_zero_axes_finite_and_normals_camera_facing() {
    let eye = Vec3::new(-2.5, -3.5, -4.5);
    for direction in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        let layers = phase_layers(eye, direction, PHASE_RANGE, |_, _, _| Some(Block::Stone));
        assert_eq!(layers.len(), PHASE_LAYERS);
        for (index, layer) in layers.iter().enumerate() {
            let distance = index as f32 + 0.5;
            assert_close(layer.distance, distance);
            assert_eq!(layer.normal, -direction);
            assert_eq!(layer.position, eye + direction * distance);
            assert!(layer.position.is_finite());
        }
    }
}

#[test]
fn negative_coordinates_use_the_containing_floor_voxel() {
    let eye = Vec3::new(-0.25, -2.25, -3.25);
    for (direction, first_distance, entered_x) in [(Vec3::X, 0.25, 0), (Vec3::NEG_X, 0.75, -2)] {
        let mut visited = Vec::new();
        let layers = phase_layers(eye, direction, 1.0, |x, y, z| {
            visited.push([x, y, z]);
            Some(Block::Stone)
        });
        assert_eq!(layers.len(), 1);
        assert_close(layers[0].distance, first_distance);
        assert_eq!(visited, [[-1, -3, -4], [entered_x, -3, -4]]);
    }
}

#[test]
fn edge_and_corner_ties_advance_all_axes_with_one_deterministic_normal() {
    for (direction, step, normal, length) in [
        (
            Vec3::new(1.0, 1.0, 0.0),
            [1, 1, 0],
            Vec3::NEG_X,
            2.0_f32.sqrt(),
        ),
        (
            Vec3::new(0.0, 1.0, 1.0),
            [0, 1, 1],
            Vec3::NEG_Y,
            2.0_f32.sqrt(),
        ),
        (Vec3::ONE, [1, 1, 1], Vec3::NEG_X, 3.0_f32.sqrt()),
        (Vec3::NEG_ONE, [-1, -1, -1], Vec3::X, 3.0_f32.sqrt()),
    ] {
        let mut visited = Vec::new();
        let eye = Vec3::splat(0.5);
        let layers = phase_layers(eye, direction, PHASE_RANGE, |x, y, z| {
            visited.push([x, y, z]);
            Some(Block::Slate)
        });

        assert_eq!(layers.len(), PHASE_LAYERS);
        assert_eq!(visited.len(), PHASE_LAYERS + 1);
        for (index, layer) in layers.iter().enumerate() {
            assert_close(layer.distance, (index as f32 + 0.5) * length);
            assert_eq!(layer.normal, normal);
        }
        for (index, coordinate) in visited.into_iter().enumerate() {
            assert_eq!(coordinate, step.map(|axis| axis * index as i32));
        }
    }
}

#[test]
fn separated_crossings_visit_intermediate_voxels_in_distance_order() {
    let mut visited = Vec::new();
    let layers = phase_layers(
        Vec3::new(0.5, 0.25, 0.5),
        Vec3::new(1.0, 1.0, 0.0),
        PHASE_RANGE,
        |x, y, z| {
            visited.push([x, y, z]);
            Some(Block::Stone)
        },
    );

    assert_eq!(
        visited,
        [[0, 0, 0], [1, 0, 0], [1, 1, 0], [2, 1, 0], [2, 2, 0]]
    );
    for (layer, (distance, normal)) in layers.iter().zip([
        (0.5, Vec3::NEG_X),
        (0.75, Vec3::NEG_Y),
        (1.5, Vec3::NEG_X),
        (1.75, Vec3::NEG_Y),
    ]) {
        assert_close(layer.distance, distance * 2.0_f32.sqrt());
        assert_eq!(layer.normal, normal);
    }
    assert!(
        layers
            .windows(2)
            .all(|pair| pair[0].distance < pair[1].distance)
    );
}

#[test]
fn nearly_tied_axes_follow_the_shader_epsilon_without_duplicate_faces() {
    let mut visited = Vec::new();
    let layers = phase_layers(
        Vec3::new(0.5, 0.500001, 0.5),
        Vec3::new(1.0, 1.0, 0.0),
        PHASE_RANGE,
        |x, y, z| {
            visited.push([x, y, z]);
            Some(Block::Stone)
        },
    );
    assert_eq!(visited, (0..=4).map(|i| [i, i, 0]).collect::<Vec<_>>());
    assert!(layers.iter().all(|layer| layer.normal == Vec3::NEG_X));
    assert!(
        layers
            .windows(2)
            .all(|pair| pair[0].distance < pair[1].distance)
    );
}

#[test]
fn an_exact_negative_boundary_skips_the_self_face_and_uses_entered_material() {
    let mut visited = Vec::new();
    let layers = phase_layers(
        Vec3::new(0.0, 0.5, 0.5),
        Vec3::NEG_X,
        PHASE_RANGE,
        |x, _, _| {
            visited.push(x);
            Some(if x == 0 { Block::Stone } else { Block::IronOre })
        },
    );

    assert_eq!(layers.len(), PHASE_LAYERS);
    assert_eq!(visited, [0, -1, -2, -3, -4, -5]);
    for (index, layer) in layers.iter().enumerate() {
        assert_close(layer.distance, index as f32 + 1.0);
        assert_eq!(layer.block, Block::IronOre);
        assert_eq!(layer.normal, Vec3::X);
        assert!(layer.position.is_finite());
    }
}

#[test]
fn near_eye_planes_are_skipped_but_do_not_skip_cavity_detection() {
    let eye = Vec3::new(0.99, 0.5, 0.5);
    let layers = phase_layers(eye, Vec3::X, PHASE_RANGE, |_, _, _| Some(Block::Stone));
    assert_eq!(layers.len(), PHASE_LAYERS);
    assert_close(layers[0].distance, 1.01);
    for entered in [
        Some(Block::Air),
        Some(Block::Water),
        Some(Block::Lava),
        None,
    ] {
        let mut visited = Vec::new();
        let layers = phase_layers(eye, Vec3::X, PHASE_RANGE, |x, _, _| {
            visited.push(x);
            if x == 0 { Some(Block::Stone) } else { entered }
        });
        assert!(layers.is_empty());
        assert_eq!(visited, [0, 1]);
    }
}

#[test]
fn negative_integer_corner_has_no_zero_length_neighbor_or_self_face() {
    let mut visited = Vec::new();
    let layers = phase_layers(Vec3::ZERO, Vec3::NEG_ONE, PHASE_RANGE, |x, y, z| {
        visited.push([x, y, z]);
        Some(Block::Calcite)
    });
    assert_eq!(visited, (0..=5).map(|i| [-i; 3]).collect::<Vec<_>>());
    assert_eq!(layers.len(), PHASE_LAYERS);
    assert_close(layers[0].distance, 3.0_f32.sqrt());
    assert!(layers.iter().all(|layer| layer.normal == Vec3::X));
}

#[test]
fn ray_range_is_in_world_units_and_includes_its_exact_boundary() {
    let eye = Vec3::splat(0.5);
    for (limit, expected) in [(0.49, 0), (0.5, 1), (1.49, 1), (1.5, 2)] {
        let layers = phase_layers(eye, Vec3::X * 11.0, limit, |_, _, _| Some(Block::Stone));
        assert_eq!(layers.len(), expected);
        assert!(layers.iter().all(|layer| layer.distance <= limit));
    }
}

#[test]
fn finite_extreme_direction_lengths_normalize_without_overflow_or_underflow() {
    for magnitude in [f32::MAX, f32::MIN_POSITIVE, f32::from_bits(1)] {
        let layers = phase_layers(
            Vec3::splat(0.5),
            Vec3::new(magnitude, -0.0, 0.0),
            PHASE_RANGE,
            |_, _, _| Some(Block::Stone),
        );
        assert_eq!(layers.len(), PHASE_LAYERS);
        assert_close(layers[0].distance, 0.5);
        assert_eq!(layers[0].normal, Vec3::NEG_X);
    }
}

#[test]
fn invalid_ray_inputs_do_not_sample_world_data() {
    let valid_eye = Vec3::splat(0.5);
    for (eye, direction, limit) in [
        (Vec3::NAN, Vec3::X, PHASE_RANGE),
        (Vec3::new(f32::INFINITY, 0.0, 0.0), Vec3::X, PHASE_RANGE),
        (Vec3::new(i32::MAX as f32, 0.0, 0.0), Vec3::X, PHASE_RANGE),
        (Vec3::new(f32::MIN, 0.0, 0.0), Vec3::X, PHASE_RANGE),
        (valid_eye, Vec3::NAN, PHASE_RANGE),
        (valid_eye, Vec3::new(0.0, f32::INFINITY, 0.0), PHASE_RANGE),
        (valid_eye, Vec3::ZERO, PHASE_RANGE),
        (valid_eye, Vec3::new(-0.0, 0.0, -0.0), PHASE_RANGE),
        (valid_eye, Vec3::X, 0.0),
        (valid_eye, Vec3::X, -1.0),
        (valid_eye, Vec3::X, f32::NAN),
        (valid_eye, Vec3::X, f32::INFINITY),
    ] {
        let layers = phase_layers(eye, direction, limit, |_, _, _| {
            panic!("invalid input must not sample: {eye:?}, {direction:?}, {limit}")
        });
        assert!(layers.is_empty());
    }
}

#[test]
fn a_negative_world_edge_stops_before_integer_overflow() {
    let mut visited = Vec::new();
    let layers = phase_layers(
        Vec3::new(i32::MIN as f32, 0.5, 0.5),
        Vec3::NEG_X,
        PHASE_RANGE,
        |x, y, z| {
            visited.push([x, y, z]);
            Some(Block::Stone)
        },
    );
    assert!(layers.is_empty());
    assert_eq!(visited, [[i32::MIN, 0, 0]]);
}

#[test]
fn representable_extreme_cells_can_advance_without_wrapping() {
    for x in [i32::MIN as f32, i32::MAX as f32 - 128.0] {
        let first = world_cell(Vec3::new(x, 0.5, 0.5)).unwrap()[0];
        let mut visited = Vec::new();
        let layers = phase_layers(Vec3::new(x, 0.5, 0.5), Vec3::X, PHASE_RANGE, |x, _, _| {
            visited.push(x);
            Some(Block::Stone)
        });
        assert_eq!(layers.len(), PHASE_LAYERS);
        assert_eq!(
            visited,
            (0..=4).map(|offset| first + offset).collect::<Vec<_>>()
        );
        assert!(layers.iter().all(|layer| layer.position.is_finite()));
    }
}

#[test]
fn upper_y_boundary_is_air_while_unloaded_in_world_y_stays_unknown() {
    let mut world = PhaseWorld::default();
    assert!(world.update_volume(Vec3::new(0.5, WORLD_HEIGHT as f32 - 0.5, 0.5)));
    assert_eq!(
        world.volume().voxel([0, WORLD_HEIGHT - 1, 0]),
        UNKNOWN_VOXEL
    );
    assert_eq!(world.volume().voxel([0, WORLD_HEIGHT, 0]), Block::Air as u8);
}
