use super::*;
use rustcraft::mesh::Vertex;

fn quad(material: u32) -> Mesh {
    Mesh {
        vertices: [
            [0.0, -4.0, 0.0],
            [1.0, -4.0, 0.0],
            [1.0, -3.0, 0.0],
            [0.0, -3.0, 0.0],
        ]
        .map(|position| Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            material,
            light: 0.25,
            water: [0.0; 4],
            water_extra: [0.0; 4],
            local_light: [0.0; 3],
        })
        .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

#[test]
fn parses_seed_and_rejects_incomplete_or_unknown_options() {
    let parse =
        |args: &[&str]| seed_argument(&args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>());
    assert_eq!(parse(&[]).unwrap(), Some(Terrain::default().seed()));
    assert_eq!(parse(&["--seed", "42"]).unwrap(), Some(42));
    assert_eq!(parse(&["--help"]).unwrap(), None);
    assert_eq!(parse(&["-h"]).unwrap(), None);
    for args in [
        &["--seed"][..],
        &["--seed", "-1"],
        &["--seed", "bad"],
        &["42"],
        &["--help", "--seed", "42"],
    ] {
        assert!(parse(args).is_err(), "args={args:?}");
    }
}

#[test]
fn survey_neighborhoods_are_deduplicated_negative_coordinate_safe_and_capped() {
    let sites: Vec<_> = (0..7)
        .map(|index| Vec3::new((index * 80 - 240) as f32 + 0.5, -20.0, -0.5))
        .collect();
    let supports = [Vec3::new(500.5, 26.0, 1.5), Vec3::new(-500.5, 26.0, 1.5)];
    assert_eq!(survey_coords(&sites, &supports).len(), 65);
    let point = Vec3::new(-16.5, -20.0, -0.5);
    let coords = survey_coords(&[point, point], &[point]);
    assert_eq!(coords.len(), 9);
    assert_eq!(coords.first(), Some(&ChunkCoord::new(-3, -2)));
    assert_eq!(coords.last(), Some(&ChunkCoord::new(-1, 0)));
    let complete = survey_coords(&[point; 7], &[]);
    assert_eq!(complete.len(), 45);
    assert!(survey_coords(&[], &[]).is_empty());
}

#[test]
fn sampled_camera_ray_respects_opacity_not_collision_or_water() {
    let eye = Vec3::new(-3.5, -3.5, -0.5);
    let target = [0, -4, -1];
    for (obstacle, visible) in [
        (Block::Air, true),
        (Block::Water, true),
        (Block::Lava, false),
        (Block::Crystal, false),
    ] {
        assert_eq!(
            clear_ray_to(eye, target, |point| {
                if point == target {
                    Block::Moss
                } else if point[0] == -2 {
                    obstacle
                } else {
                    Block::Air
                }
            }),
            visible,
        );
    }
    assert!(clear_ray_to(block_center(target), target, |_| Block::Stone));
    let target_point = block_center(target);
    assert!(clear_ray_between(eye, target_point, |_| Block::Water));
    assert!(!clear_ray_between(eye, target_point, |_| Block::Lava));
}

#[test]
fn negative_y_air_intervals_include_both_edges_without_merging_rock_separators() {
    assert_eq!(
        air_intervals(MIN_WORLD_Y..0, |y| {
            y == MIN_WORLD_Y || (-51..=-48).contains(&y) || (-3..0).contains(&y)
        }),
        [(MIN_WORLD_Y, MIN_WORLD_Y), (-51, -48), (-3, -1)]
    );
    assert_eq!(air_intervals(-4..0, |_| true), [(-4, -1)]);
    assert!(air_intervals(-4..0, |_| false).is_empty());
    assert!(air_intervals(0..0, |_| true).is_empty());
}

#[test]
fn landmark_camera_targets_can_be_clear_air_or_terminal_rock_but_not_hidden_or_liquid() {
    let eye = Vec3::new(-15.5, -24.38, -0.5);
    let target = Vec3::new(-3.9, -24.2, -0.1);
    let terminal = [-4, -25, -1];
    for (material, clear) in [
        (Block::Air, true),
        (Block::Limestone, true),
        (Block::Water, false),
        (Block::Lava, false),
    ] {
        assert_eq!(
            clear_landmark_ray(eye, target, |point| {
                if point == terminal {
                    material
                } else {
                    Block::Air
                }
            }),
            clear
        );
    }
    for obstruction in [Block::Slate, Block::Water, Block::Lava] {
        assert!(!clear_landmark_ray(eye, target, |point| {
            if point[0] == -9 {
                obstruction
            } else {
                Block::Air
            }
        }));
    }
    assert!(!clear_landmark_ray(Vec3::splat(f32::NAN), target, |_| {
        Block::Air
    }));
    assert!(!clear_landmark_ray(eye, Vec3::splat(f32::INFINITY), |_| {
        Block::Air
    }));
}

#[test]
fn cross_section_runs_stop_at_solids_liquids_and_the_observation_boundary() {
    let center = [-17, -31, -1];
    for obstacle in [Block::Stone, Block::Water, Block::Lava, Block::Calcite] {
        let block = |point: [i32; 3]| {
            if (-20..=-12).contains(&point[0]) {
                Block::Air
            } else {
                obstacle
            }
        };
        assert_eq!(air_run(center, 0, 20, block), 9);
        assert_eq!(air_run(center, 0, 2, block), 5);
    }
    assert_eq!(air_run(center, 0, 20, |_| Block::Limestone), 0);
    assert_eq!(air_run(center, 2, 2, |_| Block::Air), 5);
}

#[test]
fn diagnostic_headroom_checks_the_player_width_dryness_and_fractional_ceiling() {
    let feet = Vec3::new(-16.5, -22.0, -0.5);
    assert!(dry_headroom(feet, 3.4, |[_, y, _]| {
        if y < -22 { Block::Stone } else { Block::Air }
    }));
    for obstruction in [Block::Stone, Block::Water, Block::Lava] {
        assert!(!dry_headroom(feet, 3.4, |[_, y, _]| {
            if y == -19 { obstruction } else { Block::Air }
        }));
    }
    assert!(!dry_headroom(feet.with_x(-16.1), 3.4, |[x, _, _]| {
        if x == -16 { Block::Stone } else { Block::Air }
    }));
    for invalid_height in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(!dry_headroom(feet, invalid_height, |_| Block::Air));
    }
    assert!(!dry_headroom(Vec3::splat(f32::NAN), 3.4, |_| Block::Air));
}

#[test]
fn calcite_tip_support_rejects_floating_and_out_of_world_runs() {
    let tip = [-17, -12, -1];
    let anchored = |[_, y, _]: [i32; 3]| match y {
        -12..=-9 => Block::Calcite,
        -8 => Block::Limestone,
        _ => Block::Air,
    };
    assert!(formation_is_supported(tip, 1, anchored));
    assert!(!formation_is_supported(tip, -1, anchored));
    assert!(!formation_is_supported(tip, 1, |[_, y, _]| {
        if (-12..=-9).contains(&y) {
            Block::Calcite
        } else {
            Block::Water
        }
    }));
    assert!(!formation_is_supported([0, MIN_WORLD_Y + 1, 0], -1, |_| {
        Block::Calcite
    }));
    assert!(!formation_is_supported([0, WORLD_HEIGHT - 2, 0], 1, |_| {
        Block::Calcite
    }));
}

#[test]
fn exposed_feature_counts_distinguish_attached_calcite_and_spatial_clusters() {
    let mut features = Features::default();
    features.record([0, -10, 0], Block::Calcite, |[_, y, _]| match y {
        -10..=-8 => Block::Calcite,
        -7 => Block::Limestone,
        _ => Block::Air,
    });
    features.record([10, -30, 0], Block::Calcite, |_| Block::Air);
    features.record([2, -10, 0], Block::Moss, |[x, _, _]| {
        if x == 3 { Block::Moss } else { Block::Stone }
    });
    features.record([3, -10, 0], Block::Crystal, |[_, _, z]| {
        if z == -1 { Block::Crystal } else { Block::Air }
    });
    features.record([4, -10, 0], Block::Crystal, |_| Block::Air);
    assert_eq!(features.exposed[Block::Calcite as usize], 2);
    assert_eq!(features.exposed[Block::Crystal as usize], 2);
    assert_eq!(features.upper_calcite, 1);
    assert_eq!(features.supported_calcite_tips, 1);
    assert_eq!(features.unsupported_calcite_tips, 1);
    assert_eq!(features.clustered_moss, 1);
    assert_eq!(features.clustered_crystal, 1);
    assert_eq!(features.attached_crystal, 0);
    assert_eq!(features.unsupported_crystal, 2);
    let mut separated = Features::default();
    separated.record([0, -10, 0], Block::Calcite, |[x, y, z]| {
        if y == -10 && z == 0 && [0, 6].contains(&x) {
            Block::Calcite
        } else if y == -11 {
            Block::Limestone
        } else {
            Block::Air
        }
    });
    assert_eq!(separated.calcite_air_gaps, 1);
}

#[test]
fn broad_pool_measurement_handles_seams_and_rejects_thin_or_roofed_liquid() {
    let center = [-16, -20, -1];
    for liquid in [Block::Water, Block::Lava] {
        let block = |[x, y, z]: [i32; 3]| {
            if (-17..=-15).contains(&x) && (-2..=0).contains(&z) && y == -20 {
                liquid
            } else if y < -20 {
                Block::Basalt
            } else {
                Block::Air
            }
        };
        assert!(broad_pool_surface(center, liquid, block));
        for obstacle in [Block::Stone, Block::Calcite, liquid] {
            assert!(!broad_pool_surface(center, liquid, |point| {
                if point == [-15, -18, 0] {
                    obstacle
                } else {
                    block(point)
                }
            }));
        }
        assert!(!broad_pool_surface(center, liquid, |point| {
            if point == [-15, -20, 0] {
                Block::Stone
            } else {
                block(point)
            }
        }));
        let mut features = Features::default();
        features.record(center, liquid, block);
        assert_eq!(
            features.broad_liquid_surfaces[usize::from(liquid == Block::Lava)],
            1
        );
    }
}

#[test]
fn calcite_component_attachment_accepts_side_roots_but_not_liquid_support_or_world_edges() {
    let root = [-17, -23, -1];
    let mut cache = BTreeMap::new();
    let block = |[x, y, z]: [i32; 3]| {
        if y == -23 && z == -1 && (-19..=-17).contains(&x) {
            Block::Calcite
        } else if [x, y, z] == [-20, -23, -1] {
            Block::Slate
        } else {
            Block::Air
        }
    };
    assert!(material_is_attached(
        root,
        Block::Calcite,
        block,
        &mut cache
    ));
    assert!(material_is_attached(
        root,
        Block::Calcite,
        |_| panic!("cached attachment was resampled"),
        &mut cache
    ));
    for support in [Block::Water, Block::Lava, Block::Air] {
        assert!(!material_is_attached(
            root,
            Block::Calcite,
            |point| {
                if point == root {
                    Block::Calcite
                } else {
                    support
                }
            },
            &mut BTreeMap::new()
        ));
    }
    assert!(!material_is_attached(
        [0, MIN_WORLD_Y, 0],
        Block::Calcite,
        |point| {
            if point[1] < MIN_WORLD_Y {
                Block::Stone
            } else if point == [0, MIN_WORLD_Y, 0] {
                Block::Calcite
            } else {
                Block::Air
            }
        },
        &mut BTreeMap::new()
    ));
    for material in [Block::Calcite, Block::Crystal] {
        assert!(material_is_attached(
            root,
            material,
            |point| {
                if point == root {
                    material
                } else if point == [-17, -24, -1] {
                    Block::Slate
                } else {
                    Block::Air
                }
            },
            &mut BTreeMap::new()
        ));
    }
}

#[test]
fn narrow_corridor_loops_are_not_wide_room_pillars_or_boundary_clipping() {
    let min = [-12, -12];
    let max = [12, 12];
    let ring = |[x, z]: [i32; 2]| (3..=5).contains(&x.abs().max(z.abs()));
    assert_eq!(enclosed_rock_islands(min, max, ring, ring), 1);
    assert_eq!(enclosed_rock_islands(min, max, ring, |_| false), 0);
    assert_eq!(
        enclosed_rock_islands(min, max, |point| ring(point) && point[0] != 0, ring),
        0
    );
    assert_eq!(
        enclosed_rock_islands([-2, -2], [2, 2], |_| false, |_| true),
        0
    );
}

#[test]
fn geometry_sampler_observes_split_air_levels_tall_columns_and_real_attachments() {
    let feet = Vec3::new(-16.5, -40.0, -0.5);
    let block = |[x, y, z]: [i32; 3]| {
        if x == -17 && z == -1 && (-20..=-17).contains(&y) {
            return Block::Calcite;
        }
        if (-25..=-9).contains(&x)
            && (-9..=7).contains(&z)
            && ((-40..=-31).contains(&y) || (-28..=-18).contains(&y))
        {
            Block::Air
        } else {
            Block::Limestone
        }
    };
    let mut connected = HashSet::new();
    for z in -9..=7 {
        for x in -25..=-9 {
            for y in MIN_WORLD_Y..0 {
                if block([x, y, z]) == Block::Air && block([x, y + 1, z]) == Block::Air {
                    connected.insert([x, y, z]);
                }
            }
        }
    }
    let observed = Morphology::inspect(feet, &connected, block);
    assert!(observed.split_columns > 50, "{observed:?}");
    assert_eq!(observed.headroom_max, 11);
    assert_eq!(observed.width_max, 17);
    assert_eq!(observed.headroom_quantiles, [10, 10, 11]);
    assert_eq!(observed.width_quantiles, [17; 3]);
    assert!(observed.wide_cells > 0);
    assert!(observed.features.exposed[Block::Limestone as usize] > 0);
    assert_eq!(observed.features.upper_calcite, 2);
    assert_eq!(observed.features.supported_calcite_tips, 1);
    assert_eq!(observed.features.unsupported_calcite_tips, 0);
    let tall_block = |[x, y, z]: [i32; 3]| {
        if (-25..=-9).contains(&x) && (-9..=7).contains(&z) && (-40..=-11).contains(&y) {
            Block::Air
        } else {
            Block::Basalt
        }
    };
    let tall = Morphology::inspect(feet, &connected, tall_block);
    assert_eq!(tall.headroom_max, 30);
    assert!(tall.tall_columns > 50);
    assert!(tall.aspect_max > observed.aspect_max);
    let empty = Morphology::inspect(feet, &HashSet::new(), |_| Block::Stone);
    assert_eq!(empty.connected_columns, 0);
    assert_eq!(empty.headroom_min, 0);
    assert_eq!(empty.width_min, 0);
    assert_eq!(empty.aspect_min, 0.0);
    assert_eq!(empty.headroom_quantiles, [0; 3]);
    assert_eq!(empty.aspect_quantiles, [0.0; 3]);
}

#[test]
fn counts_underground_lava_in_opaque_pass_and_water_separately() {
    let mut survey = MeshSurvey::default();
    survey.record(&Mesh::default(), false).unwrap();
    survey.record(&quad(17), false).unwrap();
    survey.record(&quad(9), true).unwrap();
    assert_eq!(survey.vertices, [4, 4]);
    assert_eq!(survey.triangles, [2, 2]);
    assert_eq!(survey.underground_faces[17], 1);
    assert_eq!(survey.underground_faces[9], 1);
    assert_eq!(survey.exposed_liquids, [Some([0, -4, -1]); 2]);
    assert_eq!((survey.light_min, survey.light_max), (0.25, 0.25));
    assert!(survey.record(&quad(17), true).is_err());
    assert!(survey.record(&quad(9), false).is_err());
    assert!(survey.record(&quad(TILE_COUNT), false).is_err());
}

#[test]
fn new_geology_faces_are_valid_opaque_atlas_tiles_not_block_ids() {
    let mut survey = MeshSurvey::default();
    for material in 18..TILE_COUNT {
        survey.record(&quad(material), false).unwrap();
        assert_eq!(survey.underground_faces[material as usize], 1);
        assert!(survey.record(&quad(material), true).is_err());
    }
    assert_eq!(survey.exposed_liquids, [None; 2]);
}

#[test]
fn rejects_bad_light_indices_and_face_winding() {
    for light in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        let mut mesh = quad(11);
        mesh.vertices[0].light = light;
        assert!(MeshSurvey::default().record(&mesh, false).is_err());
    }
    let mut mesh = quad(11);
    mesh.indices.push(0);
    assert!(MeshSurvey::default().record(&mesh, false).is_err());
    mesh.indices = vec![0, 1, 4];
    assert!(MeshSurvey::default().record(&mesh, false).is_err());
    mesh.indices = vec![0, 2, 1];
    assert!(MeshSurvey::default().record(&mesh, false).is_err());
    mesh.indices = vec![0, 0, 0];
    assert!(MeshSurvey::default().record(&mesh, false).is_err());
    mesh.indices = vec![0, 1, 2];
    mesh.vertices[0].position[0] = f32::NAN;
    assert!(MeshSurvey::default().record(&mesh, false).is_err());
}

#[test]
fn generated_diagnostic_neighborhood_has_real_clearance_and_exposed_features() {
    let terrain = Terrain::default();
    let feet = Vec3::from_array(terrain.cave_position(CaveBiome::Crystal).unwrap());
    let (survey, _) = Survey::generate(terrain, &[feet], &[]);
    assert_eq!(survey.chunks.len(), 9);
    assert!(survey.validate_resting(feet).unwrap().is_solid());
    let counts = survey.underground_blocks();
    assert_eq!(
        counts.iter().sum::<usize>(),
        9 * 16 * 16 * (-MIN_WORLD_Y) as usize
    );
    assert!(counts[Block::Air as usize] > 0);
    let connectivity = survey.connectivity(feet).unwrap();
    assert!(connectivity.negative_cells > 8);
    assert!(
        connectivity
            .exposed
            .iter()
            .any(|&point| survey.block(point) == Block::Crystal)
    );
    assert!(
        survey
            .visible_feature(feet, CaveBiome::Crystal, &connectivity)
            .is_some()
    );
    assert!(survey.validate_resting(Vec3::splat(f32::NAN)).is_err());
    assert!(
        survey
            .validate_resting(feet.with_y((WORLD_HEIGHT + 1) as f32))
            .is_err()
    );
    assert!(survey.connectivity(feet - Vec3::Y).is_err());
}

#[test]
fn cpu_cave_probe_reports_both_seeds_and_rejects_corrupt_landmark_poses_and_targets() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        run(terrain.seed()).unwrap();
    }
    let terrain = Terrain::default();
    let landmark = terrain.cave_landmark(CaveLandform::Cathedral).unwrap();
    let feet = Vec3::from_array(landmark.position);
    let (survey, _) = Survey::generate(terrain, &[feet], &[]);
    survey
        .validate_landmark(landmark, CaveLandform::Cathedral)
        .unwrap();
    assert!(
        survey
            .validate_landmark(landmark, CaveLandform::Rift)
            .is_err()
    );
    let invalid = [
        CaveLandmark {
            position: [f32::NAN; 3],
            ..landmark
        },
        CaveLandmark {
            target: [f32::INFINITY; 3],
            ..landmark
        },
        CaveLandmark {
            target: (feet + Vec3::Y * EYE_HEIGHT).to_array(),
            ..landmark
        },
        CaveLandmark {
            position: (feet + Vec3::X * 500.0).to_array(),
            ..landmark
        },
        CaveLandmark {
            position: feet.with_y(0.0).to_array(),
            ..landmark
        },
        CaveLandmark {
            position: (feet - Vec3::Y).to_array(),
            ..landmark
        },
    ];
    for invalid in invalid {
        assert!(
            survey
                .validate_landmark(invalid, CaveLandform::Cathedral)
                .is_err()
        );
    }
}

#[test]
fn discovered_pool_and_ore_views_have_supported_clear_feet_and_unobstructed_rays() {
    let terrain = Terrain::default();
    let sites: Vec<_> = BIOMES
        .into_iter()
        .map(|biome| Vec3::from_array(terrain.cave_position(biome).unwrap()))
        .chain(
            LANDFORMS
                .into_iter()
                .map(|kind| Vec3::from_array(terrain.cave_landmark(kind).unwrap().position)),
        )
        .collect();
    let (survey, _) = Survey::generate(terrain, &sites, &[]);
    for material in [Block::Water, Block::Lava] {
        let view = survey
            .pool_viewpoint(material)
            .unwrap_or_else(|| panic!("no supported clear 3x3 {material:?} pool viewpoint"));
        assert!(view.block[1] < 0);
        assert!(survey.validate_resting(view.feet).unwrap().is_solid());
        for dz in -1..=1 {
            for dx in -1..=1 {
                assert_eq!(
                    survey.block([view.block[0] + dx, view.block[1], view.block[2] + dz]),
                    material
                );
                assert_eq!(
                    survey.block([view.block[0] + dx, view.block[1] + 1, view.block[2] + dz]),
                    Block::Air
                );
            }
        }
        assert!(clear_ray_between(
            view.feet + Vec3::Y * EYE_HEIGHT,
            view.target,
            |point| survey.block(point)
        ));
    }
    for material in [Block::CoalOre, Block::IronOre, Block::GoldOre] {
        let view = survey.ore_viewpoint(material).unwrap();
        assert!(view.block[1] < 0);
        assert_eq!(survey.block(view.block), material);
        assert!(survey.validate_resting(view.feet).unwrap().is_solid());
        assert!(clear_ray_to(
            view.feet + Vec3::Y * EYE_HEIGHT,
            view.block,
            |point| survey.block(point)
        ));
    }
    assert!(survey.pool_viewpoint(Block::Bedrock).is_none());
    assert!(survey.ore_viewpoint(Block::Grass).is_none());
}
