use super::*;
use rustcraft::mesh::Vertex;

fn triangle(material: u32) -> Mesh {
    Mesh {
        vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .map(|position| Vertex {
                position,
                normal: [0.0, 0.0, 1.0],
                uv: [0.0; 2],
                material,
                light: 1.0,
                water: [0.0; 4],
                water_extra: [0.0; 4],
                local_light: [0.0; 3],
            })
            .to_vec(),
        indices: vec![0, 1, 2],
    }
}

#[test]
fn finds_nearest_with_euclidean_limit_and_stable_ties() {
    let center = [-8, -12];
    let candidates = [[-8, -16], [-10, -12], [-6, -12], [-5, -9]];
    assert_eq!(
        nearest_matching(center, 4, |point| candidates.contains(&point)),
        Some([-10, -12])
    );
    assert_eq!(nearest_matching(center, 4, |point| point == [-5, -9]), None);
    assert_eq!(
        nearest_matching(center, 4, |point| point == [-8, -16]),
        Some([-8, -16])
    );
    assert_eq!(nearest_matching(center, 0, |_| true), Some(center));
    assert_eq!(nearest_matching(center, -1, |_| true), None);
    assert_eq!(
        nearest_matching([i32::MAX, i32::MIN], 1, |_| true),
        Some([i32::MAX, i32::MIN])
    );
}

#[test]
fn aggregates_surfaces_and_actual_trunks_separately_from_biome_labels() {
    let mut coverage = Coverage::default();
    for (biome, height, surface, above) in [
        (Biome::Plains, 22, Block::Grass, Block::Air),
        (Biome::Forest, 23, Block::Grass, Block::Wood),
        (Biome::Forest, 24, Block::Grass, Block::Leaves),
        (Biome::Desert, 18, Block::Sand, Block::Air),
        (Biome::Alpine, 35, Block::Stone, Block::Air),
        (Biome::Alpine, 44, Block::Snow, Block::Air),
    ] {
        coverage.record(Column { height, biome }, surface, above);
    }
    assert_eq!(coverage.columns, [1, 2, 1, 2]);
    assert_eq!(coverage.trunks, [0, 1, 0, 0]);
    assert_eq!(coverage.surfaces[Block::Grass as usize], 3);
    assert_eq!(coverage.surfaces[Block::Stone as usize], 1);
    assert_eq!(coverage.surfaces[Block::Snow as usize], 1);
    assert_eq!((coverage.min_height, coverage.max_height), (18, 44));
}

#[test]
fn sample_landing_filter_rejects_overhead_canopies_not_only_standing_collisions() {
    let feet = Vec3::new(-8.5, 27.0, -0.5);
    assert!(landing_descent_clear(feet, |_, _, _| Block::Air));
    assert!(landing_descent_clear(feet, |_, _, _| Block::Water));
    for obstruction in [Block::Wood, Block::Leaves, Block::Stone] {
        assert!(!landing_descent_clear(feet, |_, y, _| {
            if y == 31 { obstruction } else { Block::Air }
        }));
    }
    assert!(!landing_descent_clear(feet.with_x(-8.1), |x, _, _| {
        if x == -8 { Block::Leaves } else { Block::Air }
    }));
    assert!(!landing_descent_clear(Vec3::splat(f32::NAN), |_, _, _| {
        Block::Air
    }));
}

#[test]
fn accepts_empty_and_valid_meshes_and_counts_atlas_not_block_ids() {
    assert_eq!(
        material_counts(&Mesh::default(), true).unwrap(),
        [0; TILE_COUNT as usize]
    );
    let counts = material_counts(&triangle(8), false).unwrap();
    assert_eq!(counts[8], 3);
    assert_eq!(counts.iter().sum::<usize>(), 3);
    assert_eq!(material_counts(&triangle(9), true).unwrap()[9], 3);
    for material in 10..TILE_COUNT {
        assert_eq!(
            material_counts(&triangle(material), false).unwrap()[material as usize],
            3
        );
        assert!(material_counts(&triangle(material), true).is_err());
    }
}

#[test]
fn rejects_invalid_drawable_geometry_and_materials() {
    assert!(material_counts(&triangle(TILE_COUNT), false).is_err());
    assert!(material_counts(&triangle(9), false).is_err());
    assert!(material_counts(&triangle(8), true).is_err());
    let mut mesh = triangle(0);
    mesh.indices.push(0);
    assert!(material_counts(&mesh, false).is_err());
    mesh.indices = vec![0, 1, 3];
    assert!(material_counts(&mesh, false).is_err());
    mesh.indices = vec![0, 2, 1];
    assert!(material_counts(&mesh, false).is_err());
    mesh.indices = vec![0, 0, 0];
    assert!(material_counts(&mesh, false).is_err());
    mesh.indices = vec![0, 1, 2];
    mesh.vertices[0].position[0] = f32::NAN;
    assert!(material_counts(&mesh, false).is_err());
    mesh.vertices[0].position[0] = 0.0;
    for light in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        mesh.vertices[0].light = light;
        assert!(material_counts(&mesh, false).is_err());
    }
}

#[test]
fn representative_filter_and_landing_work_on_real_decorated_terrain() {
    let terrain = Terrain::default();
    let spawn = terrain.spawn_position();
    let origin = [spawn[0].floor() as i32, spawn[2].floor() as i32];
    let point = nearest_matching(origin, 128, |point| {
        representative_site(&terrain, point, Biome::Forest, Block::Grass)
    })
    .unwrap();
    assert!(!representative_site(
        &terrain,
        [point[0] + 1, point[1]],
        Biome::Forest,
        Block::Grass,
    ));
    assert!(!representative_site(
        &terrain,
        point,
        Biome::Desert,
        Block::Sand,
    ));
    assert!(!representative_site(
        &terrain,
        point,
        Biome::Forest,
        Block::Sand,
    ));
    validate_landing(&terrain, Vec3::from_array(spawn)).unwrap();
    assert!(validate_landing(&terrain, Vec3::from_array(spawn) - Vec3::Y,).is_err());
    let coverage = inspect_region(&terrain, ChunkCoord::from_world(spawn[0], spawn[2]), 0);
    assert_eq!(coverage.columns.iter().sum::<usize>(), 256);
    assert_eq!(coverage.surfaces.iter().sum::<usize>(), 256);
}

#[test]
fn cpu_probe_validates_all_biomes_water_and_spawn() {
    main().unwrap();
    run(Terrain::new(42)).unwrap();
    let terrain = Terrain::default();
    let spawn = terrain.spawn_position();
    let origin = [spawn[0].floor() as i32, spawn[2].floor() as i32];
    assert!(report_site(&terrain, origin, origin, "missing water", &[9]).is_err());
}
