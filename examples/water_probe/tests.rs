use super::*;
use rustcraft::mesh::Vertex;

fn sample() -> WaterSample {
    WaterSample {
        bottom: -4,
        surface: 18.25,
        flow: [0.0; 3],
        biome: WaterBiome::Ocean,
        salinity: 1.0,
    }
}

fn quad(water: bool) -> Mesh {
    Mesh {
        vertices: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
        .map(|position| Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            material: if water { 9 } else { 22 },
            local_light: [0.0; 3],
            light: 1.0,
            water: if water {
                [0.5, -0.25, 4.0, 0.75]
            } else {
                [0.0; 4]
            },
            water_extra: if water {
                [0.0, WaterBiome::Estuary as u8 as f32, 0.5, 1.0]
            } else {
                [0.0; 4]
            },
        })
        .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

#[test]
fn parses_default_explicit_and_invalid_seeds() {
    let parse =
        |args: &[&str]| seed_argument(&args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>());
    assert_eq!(parse(&[]).unwrap(), Some(Terrain::default().seed()));
    assert_eq!(parse(&["--seed", "42"]).unwrap(), Some(42));
    assert_eq!(
        parse(&["--seed", "18446744073709551615"]).unwrap(),
        Some(u64::MAX)
    );
    assert_eq!(parse(&["--help"]).unwrap(), None);
    assert_eq!(parse(&["-h"]).unwrap(), None);
    for args in [
        &["--seed"][..],
        &["--seed", "-1"],
        &["--seed", "bad"],
        &["42"],
        &["--help", "42"],
    ] {
        assert!(parse(args).is_err(), "args={args:?}");
    }
}

#[test]
fn profile_statistics_measure_physical_depth_flow_and_materials_in_f64() {
    let mut stats = Profiles::default();
    stats.record(sample(), Some(Block::Clay)).unwrap();
    stats
        .record(
            WaterSample {
                flow: [3.0, -4.0, 0.0],
                surface: 19.5,
                ..sample()
            },
            Some(Block::Gravel),
        )
        .unwrap();
    assert_eq!(stats.columns, 2);
    assert_eq!(stats.depths, [21.25, 22.5]);
    assert_eq!(range(&stats.speed), [0.0, 5.0]);
    assert_eq!(stats.flow_min, [0.0, -4.0, 0.0]);
    assert_eq!(stats.flow_max, [3.0, 0.0, 0.0]);
    assert_eq!(stats.beds[Block::Clay as usize], 1);
    assert_eq!(stats.beds[Block::Gravel as usize], 1);
    assert_eq!(quantiles(&[4.0, 1.0, 9.0, 2.0, 3.0]), [1.0, 3.0, 4.0]);
    assert_eq!(quantiles(&[]), [0.0; 3]);
    assert_eq!(range(&[]), [0.0; 2]);
    assert!(
        Profiles::default()
            .record(sample(), Some(Block::Water))
            .is_err()
    );
}

#[test]
fn rejects_nonfinite_dry_out_of_world_and_out_of_range_profiles() {
    for invalid in [
        WaterSample {
            surface: f32::NAN,
            ..sample()
        },
        WaterSample {
            surface: f32::INFINITY,
            ..sample()
        },
        WaterSample {
            surface: -3.0,
            ..sample()
        },
        WaterSample {
            bottom: MIN_WORLD_Y - 1,
            ..sample()
        },
        WaterSample {
            surface: WORLD_HEIGHT as f32 + 0.5,
            ..sample()
        },
        WaterSample {
            flow: [f32::NAN; 3],
            ..sample()
        },
        WaterSample {
            salinity: -0.01,
            ..sample()
        },
        WaterSample {
            salinity: 1.01,
            ..sample()
        },
        WaterSample {
            salinity: f32::INFINITY,
            ..sample()
        },
    ] {
        assert!(validate_sample(invalid).is_err(), "sample={invalid:?}");
    }
}

#[test]
fn deduplicates_negative_coordinate_neighborhoods_and_enforces_full_survey_bounds() {
    let landmark = WaterLandmark {
        biome: WaterBiome::Lake,
        position: [-16.5, 24.0, -0.5],
        target: [-16.5, 22.0, -0.5],
        water: [-17, 21, -1],
    };
    let feet = Vec3::from_array(landmark.position);
    let coords = survey_coords(&[landmark], feet, feet);
    assert_eq!(coords.len(), 9);
    assert_eq!(coords.first(), Some(&ChunkCoord::new(-3, -2)));
    assert_eq!(coords.last(), Some(&ChunkCoord::new(-1, 0)));
    assert_eq!(survey_coords(&[landmark; 5], feet, feet).len(), 45);
    let separated: Vec<_> = (0..5)
        .map(|index| WaterLandmark {
            water: [index * 96 - 192, 21, -1],
            position: [(index * 96 - 192) as f32 + 64.5, 24.0, 96.5],
            ..landmark
        })
        .collect();
    assert_eq!(
        survey_coords(
            &separated,
            Vec3::new(500.5, -22.0, 500.5),
            Vec3::new(-500.5, 24.0, -500.5)
        )
        .len(),
        60
    );
}

#[test]
fn validates_sediment_tiles_water_metadata_indices_and_winding() {
    assert_eq!(
        mesh_materials(&Mesh::default(), false).unwrap(),
        [0; TILE_COUNT as usize]
    );
    assert_eq!(mesh_materials(&quad(false), false).unwrap()[22], 4);
    assert_eq!(mesh_materials(&quad(true), true).unwrap()[9], 4);
    assert!(mesh_materials(&quad(true), false).is_err());
    assert!(mesh_materials(&quad(false), true).is_err());
    let mut mesh = quad(false);
    mesh.vertices[0].water[0] = 0.1;
    assert!(mesh_materials(&mesh, false).is_err());
    mesh = quad(false);
    mesh.vertices[0].material = TILE_COUNT;
    assert!(mesh_materials(&mesh, false).is_err());
    mesh = quad(false);
    mesh.indices[2] = 4;
    assert!(mesh_materials(&mesh, false).is_err());
    mesh = quad(false);
    mesh.indices.swap(1, 2);
    assert!(mesh_materials(&mesh, false).is_err());
    mesh = quad(false);
    mesh.indices.push(0);
    assert!(mesh_materials(&mesh, false).is_err());
    for (field, value) in [(2, 0.0), (3, 1.01), (0, f32::NAN)] {
        mesh = quad(true);
        mesh.vertices[0].water[field] = value;
        assert!(mesh_materials(&mesh, true).is_err());
    }
    for (field, value) in [
        (1, 6.0),
        (1, 0.5),
        (2, -0.01),
        (3, 1.01),
        (0, f32::INFINITY),
    ] {
        mesh = quad(true);
        mesh.vertices[0].water_extra[field] = value;
        assert!(mesh_materials(&mesh, true).is_err());
    }
}

#[test]
fn cpu_probe_surveys_real_water_and_retained_cave_pools_for_both_milestone_seeds() {
    for seed in [Terrain::default().seed(), 42] {
        run(seed).unwrap();
    }
    let terrain = Terrain::default();
    let spawn = Vec3::from_array(terrain.spawn_position());
    let landmark = terrain.water_landmark(WaterBiome::Lake).unwrap();
    for invalid in [
        WaterLandmark {
            position: [f32::NAN; 3],
            ..landmark
        },
        WaterLandmark {
            target: [f32::INFINITY; 3],
            ..landmark
        },
        WaterLandmark {
            water: [0, WORLD_HEIGHT, 0],
            ..landmark
        },
        WaterLandmark {
            position: (spawn + Vec3::X * 641.0).to_array(),
            ..landmark
        },
        WaterLandmark {
            biome: WaterBiome::Ocean,
            ..landmark
        },
    ] {
        assert!(validate_landmark(&terrain, spawn, WaterBiome::Lake, invalid).is_err());
    }
}
