use super::*;
use crate::world::{ResidentWorld, WorldView};

use crate::mesh::tests::edits::assert_same_mesh;

fn surface_faces(mesh: &Mesh, source: [i32; 3]) -> Vec<&[Vertex; 4]> {
    faces(mesh)
        .iter()
        .filter(|face| source_block(face) == source && face[0].normal == [0.0, 1.0, 0.0])
        .collect()
}

fn ceiling_profile_mesh(
    origin: [i32; 3],
    size: [i32; 3],
    profiles: &[([i32; 2], WaterSample)],
    ceilings: &[([i32; 3], Block)],
) -> ChunkMesh {
    let profiles: HashMap<_, _> = profiles.iter().copied().collect();
    let block_at = |position: [i32; 3]| {
        let [x, y, z] = position;
        if let Some((_, block)) = ceilings.iter().find(|(roof, _)| *roof == position) {
            *block
        } else if let Some(profile) = profiles.get(&[x, z]) {
            if y <= profile.bottom {
                Block::Clay
            } else if (y as f32) < profile.surface {
                Block::Water
            } else {
                Block::Air
            }
        } else {
            Block::Air
        }
    };
    let mut metadata = WaterMetadata::default();
    for (&[x, z], &profile) in &profiles {
        let blocks: Vec<_> = (origin[1]..origin[1] + size[1])
            .map(|y| block_at([x, y, z]))
            .collect();
        metadata.add_column([x, z], origin[1], &blocks, |_| Some(profile));
    }
    mesh_volume_with_water(
        origin,
        size,
        |local| block_at(std::array::from_fn(|axis| origin[axis] + local[axis])),
        block_at,
        Some(&mut metadata),
        None,
        None,
    )
}

fn varied_profiles(surface: f32) -> Vec<([i32; 2], WaterSample)> {
    (-3..=3)
        .flat_map(|z| {
            (-3..=3).map(move |x| {
                let mut profile = water_profile(surface, WaterBiome::River);
                profile.flow = [
                    0.35 + x as f32 * 0.07,
                    -0.04 + z as f32 * 0.01,
                    -0.2 + z as f32 * 0.05,
                ];
                profile.salinity = 0.2 + (x + 3) as f32 * 0.04;
                ([x, z], profile)
            })
        })
        .collect()
}

fn assert_shared_surface_inputs(mesh: &Mesh, top: &[Vertex; 4]) {
    for corner in top {
        for face in faces(mesh) {
            let upper = face
                .iter()
                .map(|vertex| vertex.position[1])
                .fold(f32::MIN, f32::max);
            if face[0].normal[1] >= 0.0 && upper == corner.position[1] {
                for copy in face
                    .iter()
                    .filter(|vertex| vertex.position == corner.position)
                {
                    assert_eq!(copy.water, corner.water);
                    assert_eq!(copy.water_extra, corner.water_extra);
                }
            }
        }
    }
}

#[test]
fn fractional_caps_keep_the_air_gap_below_opaque_ceilings_and_pin_shared_waves() {
    let surface = 3.45;
    let profiles = varied_profiles(surface);
    let source = [1, 3, 1];
    let roof = [1, 4, 1];
    let open = ceiling_profile_mesh([0; 3], [3, 5, 3], &profiles, &[]);
    let open_caps = surface_faces(&open.water, source);
    assert_eq!(open_caps.len(), 1);
    let original = open_caps[0];
    assert!(original.iter().all(|corner| corner.water_extra[3] > 0.0));

    for block in [Block::Stone, Block::Clay, Block::Leaves, Block::Lava] {
        let mesh = ceiling_profile_mesh([0; 3], [3, 5, 3], &profiles, &[(roof, block)]);
        assert_valid_mesh(&mesh.water);
        assert_valid_mesh(&mesh.opaque);
        let caps = surface_faces(&mesh.water, source);
        assert_eq!(caps.len(), 1, "ceiling {block:?}");
        let top = caps[0];
        let light = daylight_through(1.0, block);
        assert!(top.iter().all(|corner| {
            corner.position[1] == surface && corner.light == light && corner.water_extra[3] == 0.0
        }));
        let ceiling = faces(&mesh.opaque)
            .iter()
            .find(|face| source_block(face) == roof && face[0].normal == [0.0, -1.0, 0.0])
            .unwrap();
        assert!(ceiling.iter().all(|vertex| vertex.position[1] == 4.0));
        assert!(top[0].position[1] < ceiling[0].position[1]);
        for (before, after) in original.iter().zip(top) {
            assert_eq!(after.water, before.water);
            assert_eq!(after.water_extra[..3], before.water_extra[..3]);
        }
        assert_shared_surface_inputs(&mesh.water, top);
    }
}

#[test]
fn fully_roofed_fractional_water_keeps_shared_currents_depth_and_salinity_without_waves() {
    let surface = 3.45;
    let profiles = varied_profiles(surface);
    let ceilings: Vec<_> = profiles
        .iter()
        .map(|&([x, z], _)| ([x, 4, z], Block::Stone))
        .collect();
    let open = ceiling_profile_mesh([0; 3], [2, 5, 2], &profiles, &[]);
    let roofed = ceiling_profile_mesh([0; 3], [2, 5, 2], &profiles, &ceilings);
    assert_eq!(face_count(&roofed.water), face_count(&open.water));
    for before in open
        .water
        .vertices
        .iter()
        .filter(|vertex| vertex.normal[1] == 1.0)
    {
        let after = roofed
            .water
            .vertices
            .iter()
            .find(|vertex| vertex.position == before.position && vertex.normal[1] == 1.0)
            .unwrap();
        assert_eq!(after.water, before.water);
        assert_eq!(after.water_extra[..3], before.water_extra[..3]);
        assert_eq!(after.water_extra[3], 0.0);
        assert!(before.water_extra[3] > 0.0);
    }
    assert_valid_mesh(&roofed.water);
    assert_valid_mesh(&roofed.opaque);
}

#[test]
fn negative_y_fractional_caps_keep_the_gap_below_world_coordinate_ceilings() {
    let surface = -0.45;
    let mut profiles = varied_profiles(surface);
    for (_, profile) in &mut profiles {
        profile.bottom = -4;
    }
    let mesh = ceiling_profile_mesh(
        [0, -4, 0],
        [3, 6, 3],
        &profiles,
        &[([1, 0, 1], Block::Stone)],
    );
    let caps = surface_faces(&mesh.water, [1, -1, 1]);
    assert_eq!(caps.len(), 1);
    assert!(caps[0].iter().all(|corner| {
        corner.position[1] == surface && corner.light == 0.0 && corner.water_extra[3] == 0.0
    }));
    assert_shared_surface_inputs(&mesh.water, caps[0]);
    assert_valid_mesh(&mesh.water);
    assert_valid_mesh(&mesh.opaque);
}

#[test]
fn integer_height_water_touching_an_opaque_ceiling_culls_the_contact_cap() {
    let profiles = varied_profiles(4.0);
    let source = [1, 3, 1];
    let roof = [1, 4, 1];
    let open = ceiling_profile_mesh([0; 3], [3, 5, 3], &profiles, &[]);
    assert_eq!(surface_faces(&open.water, source).len(), 1);
    let mesh = ceiling_profile_mesh([0; 3], [3, 5, 3], &profiles, &[(roof, Block::Stone)]);
    assert!(surface_faces(&mesh.water, source).is_empty());
    assert_eq!(face_count(&mesh.water) + 1, face_count(&open.water));
    let contacts: Vec<_> = faces(&mesh.opaque)
        .iter()
        .filter(|face| source_block(face) == roof && face[0].normal == [0.0, -1.0, 0.0])
        .collect();
    assert_eq!(contacts.len(), 1);
    assert!(contacts[0].iter().all(|vertex| vertex.position[1] == 4.0));
    assert_valid_mesh(&mesh.water);
    assert_valid_mesh(&mesh.opaque);
}

#[test]
fn fractional_ceiling_caps_share_pinned_top_and_side_corners_across_negative_seams() {
    let mut profiles = varied_profiles(3.45);
    profiles
        .iter_mut()
        .find(|(position, _)| *position == [-2, -1])
        .unwrap()
        .1
        .surface = 3.0;
    let source = [-1, 3, -1];
    let roof = [-1, 4, -1];
    let first_origin = [-2, 0, -2];
    let second_origin = [0, 0, -2];
    let size = [2, 5, 2];
    let open = ceiling_profile_mesh(first_origin, size, &profiles, &[]);
    let open_neighbor = ceiling_profile_mesh(second_origin, size, &profiles, &[]);
    let roofed = ceiling_profile_mesh(first_origin, size, &profiles, &[(roof, Block::Stone)]);
    let neighbor = ceiling_profile_mesh(second_origin, size, &profiles, &[(roof, Block::Stone)]);
    let caps = surface_faces(&roofed.water, source);
    assert_eq!(caps.len(), 1);
    assert_shared_surface_inputs(&roofed.water, caps[0]);
    assert!(
        faces(&roofed.water)
            .iter()
            .any(|face| { source_block(face) == source && face[0].normal == [-1.0, 0.0, 0.0] })
    );
    for left in caps[0].iter().filter(|corner| corner.position[0] == 0.0) {
        let right = neighbor
            .water
            .vertices
            .iter()
            .find(|corner| corner.position == left.position && corner.normal[1] == 1.0)
            .unwrap();
        let before = open_neighbor
            .water
            .vertices
            .iter()
            .find(|corner| corner.position == left.position && corner.normal[1] == 1.0)
            .unwrap();
        assert_eq!(left.water, right.water);
        assert_eq!(left.water_extra, right.water_extra);
        assert_eq!(left.water_extra[3], 0.0);
        assert!(before.water_extra[3] > 0.0);
        assert_eq!(left.water, before.water);
    }
    assert_valid_mesh(&roofed.water);
    assert_valid_mesh(&neighbor.water);
    let restored = ceiling_profile_mesh(first_origin, size, &profiles, &[]);
    let restored_neighbor = ceiling_profile_mesh(second_origin, size, &profiles, &[]);
    assert_same_mesh(&restored.water, &open.water);
    assert_same_mesh(&restored_neighbor.water, &open_neighbor.water);
}

#[test]
fn reported_river_ceiling_cap_and_metadata_survive_raw_overlay_eviction_and_removal() {
    let terrain = Terrain::default();
    let source = [18, 41, -221];
    let roof = [18, 42, -221];
    let coord = ChunkCoord::new(1, -14);
    let chunk = Chunk::generate(&terrain, coord);
    let profile = terrain.water_at(source[0], source[1], source[2]).unwrap();
    assert_eq!(profile.biome, WaterBiome::River);
    assert!((profile.surface - 41.54934).abs() < 0.00001);
    assert_eq!(terrain.block_at(roof[0], roof[1], roof[2]), Block::Air);
    let mut resident = ResidentWorld::default();
    resident.insert(chunk.clone());
    let original = mesh_chunk_with_edits(&chunk, &terrain, resident.edits());
    assert_eq!(surface_faces(&original.water, source).len(), 1);
    assert!(resident.edit_block(roof, Block::Stone));
    assert_eq!(
        resident.water_at(source[0], source[1], source[2]),
        Some(profile)
    );
    let roofed = mesh_chunk_with_edits(&chunk, &terrain, resident.edits());
    let overlaid =
        mesh_chunk_with_edits(resident.chunk(coord).unwrap(), &terrain, resident.edits());
    assert_same_mesh(&roofed.water, &overlaid.water);
    assert_same_mesh(&roofed.opaque, &overlaid.opaque);
    let caps = surface_faces(&roofed.water, source);
    assert_eq!(caps.len(), 1);
    assert!(caps[0].iter().all(|corner| {
        corner.position[1] == profile.surface && corner.light == 0.0 && corner.water_extra[3] == 0.0
    }));
    assert_shared_surface_inputs(&roofed.water, caps[0]);
    resident.remove(coord);
    let regenerated = Chunk::generate(&terrain, coord);
    let evicted = mesh_chunk_with_edits(&regenerated, &terrain, resident.edits());
    assert_same_mesh(&roofed.water, &evicted.water);
    resident.insert(regenerated);
    let reloaded =
        mesh_chunk_with_edits(resident.chunk(coord).unwrap(), &terrain, resident.edits());
    assert_same_mesh(&roofed.water, &reloaded.water);
    assert_eq!(
        resident.water_at(source[0], source[1], source[2]),
        Some(profile)
    );
    assert!(resident.edit_block(roof, Block::Air));
    let restored = mesh_chunk_with_edits(&chunk, &terrain, resident.edits());
    let restored_overlay =
        mesh_chunk_with_edits(resident.chunk(coord).unwrap(), &terrain, resident.edits());
    assert_same_mesh(&original.water, &restored.water);
    assert_same_mesh(&original.opaque, &restored.opaque);
    assert_same_mesh(&restored.water, &restored_overlay.water);
    resident.remove(coord);
    resident.insert(Chunk::generate(&terrain, coord));
    let restored_reload =
        mesh_chunk_with_edits(resident.chunk(coord).unwrap(), &terrain, resident.edits());
    assert_same_mesh(&original.water, &restored_reload.water);
}
