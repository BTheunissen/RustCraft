use crate::mesh::{Vertex, mesh_chunk, mesh_chunk_with_edits};
use crate::world::{Block, CHUNK_SIZE, Chunk, ChunkCoord, ResidentWorld, Terrain};
use glam::Vec3;
use std::collections::BTreeSet;

#[test]
fn arch_underside_has_correct_winding_real_sky_spill_and_no_rock_water_overlap() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let arch = terrain.coastal_landmark().unwrap().arch;
        let [x, _, z] = arch.opening.map(|value| value.floor() as i32);
        let coord = ChunkCoord::new(x.div_euclid(16), z.div_euclid(16));
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk(&chunk, &terrain);
        let underside = mesh
            .opaque
            .vertices
            .as_chunks::<4>()
            .0
            .iter()
            .find(|face| {
                face[0].normal == [0.0, -1.0, 0.0]
                    && face[0].position == [x as f32, arch.roof[0] as f32, z as f32]
            })
            .unwrap();
        assert!(
            underside
                .iter()
                .all(|vertex| vertex.light >= 0.31 && vertex.light <= 0.49),
            "underside spill {}",
            underside[0].light
        );
        assert!(underside.iter().all(|vertex| vertex.water_extra[0] == 1.0));
        let normal_faces = mesh.opaque.vertices.as_chunks::<4>().0;
        assert!(
            normal_faces
                .iter()
                .filter(|face| {
                    face[0].normal[1] == 0.0
                        && face[0].light > 0.3
                        && face[0].position[1] >= 18.0
                        && face[0].position[1] < arch.roof[0] as f32
                        && (face[0].position[0] - arch.opening[0]).abs() < 8.0
                        && (face[0].position[2] - arch.opening[2]).abs() < 8.0
                })
                .any(|face| face[0].water_extra[0] == 1.0)
        );
        for indices in mesh.opaque.indices.as_chunks::<3>().0 {
            let a = &mesh.opaque.vertices[indices[0] as usize];
            let b = &mesh.opaque.vertices[indices[1] as usize];
            let c = &mesh.opaque.vertices[indices[2] as usize];
            let cross = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
                .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
            assert!(cross.dot(Vec3::from_array(a.normal)) > 0.99);
        }
        assert!(
            mesh.water
                .vertices
                .iter()
                .filter(|vertex| vertex.position[1] >= crate::world::SEA_LEVEL as f32)
                .all(|vertex| vertex.position[1] < arch.roof[0] as f32)
        );
        assert!(
            mesh.opaque
                .vertices
                .iter()
                .all(|vertex| vertex.light.is_finite())
        );
        assert_eq!(
            chunk.block(x.rem_euclid(16), arch.roof[0] - 1, z.rem_euclid(16)),
            Block::Air
        );
    }
}

#[test]
fn same_arch_air_light_matches_local_and_seam_fields_and_edit_reload() {
    let terrain = Terrain::new(42);
    let arch = terrain.coastal_landmark().unwrap().arch;
    let [cx, _, cz] = arch.opening.map(|value| value.floor() as i32);
    let center = ChunkCoord::new(cx.div_euclid(16), cz.div_euclid(16));
    let mut world = ResidentWorld::default();
    let mut coords = BTreeSet::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let coord = ChunkCoord::new(center.x + dx, center.z + dz);
            coords.insert(coord);
            world.insert(Chunk::generate(&terrain, coord));
        }
    }
    let coord = center;
    let original = mesh_chunk(&Chunk::generate(&terrain, coord), &terrain);
    let roof_cell = [cx, arch.roof[0], cz];
    assert!(world.edit_block(roof_cell, Block::Air));
    let edited_chunk = Chunk::generate(&terrain, coord);
    let first = mesh_chunk_with_edits(&edited_chunk, &terrain, world.edits());
    let source = [cx as f32, (arch.roof[0] + 1) as f32, cz as f32];
    let exposed = first
        .opaque
        .vertices
        .as_chunks::<4>()
        .0
        .iter()
        .find(|face| face[0].normal == [0.0, -1.0, 0.0] && face[0].position == source)
        .unwrap();
    assert!(exposed[0].light.is_finite());
    assert!(!same_vertices(
        &original.opaque.vertices,
        &first.opaque.vertices
    ));
    world.remove(coord);
    world.insert(Chunk::generate(&terrain, coord));
    let reloaded = mesh_chunk_with_edits(world.chunk(coord).unwrap(), &terrain, world.edits());
    assert!(same_vertices(
        &first.opaque.vertices,
        &reloaded.opaque.vertices
    ));
    assert_eq!(first.opaque.indices, reloaded.opaque.indices);
    for coord in coords {
        let chunk = Chunk::generate(&terrain, coord);
        let light = super::CoastalLight::prepare(
            &chunk,
            &terrain,
            Some(world.edits()),
            [coord.x * CHUNK_SIZE, coord.z * CHUNK_SIZE],
            |[x, y, z]| chunk.block(x, y, z),
        );
        let mut light = light;
        let air = [cx, arch.roof[0] - 1, cz];
        if (coord.x * 16 - 1..=coord.x * 16 + 16).contains(&cx)
            && (coord.z * 16 - 1..=coord.z * 16 + 16).contains(&cz)
        {
            assert!(light.at(air, 0.0) >= 0.31);
        }
    }
}

fn same_vertices(first: &[Vertex], second: &[Vertex]) -> bool {
    bytemuck::cast_slice::<_, u8>(first) == bytemuck::cast_slice::<_, u8>(second)
}

#[test]
fn coastal_light_seam_samples_match_and_enclosing_edits_do_not_leak_sky() {
    let terrain = Terrain::new(42);
    let arch = terrain.coastal_landmark().unwrap().arch;
    let [cx, _, cz] = arch.opening.map(|value| value.floor() as i32);
    let center = ChunkCoord::new(cx.div_euclid(16), cz.div_euclid(16));
    let mut checked = 0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let coord = ChunkCoord::new(center.x + dx, center.z + dz);
            let chunk = Chunk::generate(&terrain, coord);
            let mut first = super::CoastalLight::prepare(
                &chunk,
                &terrain,
                None,
                [coord.x * 16, coord.z * 16],
                |[x, y, z]| chunk.block(x, y, z),
            );
            for axis in [0, 1] {
                let neighbor = if axis == 0 {
                    ChunkCoord::new(coord.x + 1, coord.z)
                } else {
                    ChunkCoord::new(coord.x, coord.z + 1)
                };
                let chunk = Chunk::generate(&terrain, neighbor);
                let mut second = super::CoastalLight::prepare(
                    &chunk,
                    &terrain,
                    None,
                    [neighbor.x * 16, neighbor.z * 16],
                    |[x, y, z]| chunk.block(x, y, z),
                );
                for along in 0..16 {
                    let [x, z] = if axis == 0 {
                        [neighbor.x * 16, neighbor.z * 16 + along]
                    } else {
                        [neighbor.x * 16 + along, neighbor.z * 16]
                    };
                    if terrain.coastal_roof(x, z).is_some() {
                        let cell = [x, crate::world::SEA_LEVEL + 3, z];
                        assert!((first.at(cell, 0.0) - second.at(cell, 0.0)).abs() < 0.000001);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 0);
    let entries = [[-1, 0], [1, 0], [0, -1], [0, 1]]
        .into_iter()
        .flat_map(|[dx, dz]| {
            (1..=8).flat_map(move |distance| {
                (crate::world::SEA_LEVEL - 1..=crate::world::SEA_LEVEL + 3).map(move |y| {
                    crate::world::BlockEdit {
                        position: [cx + dx * distance, y, cz + dz * distance],
                        block: Block::Stone,
                    }
                })
            })
        });
    let edits = crate::world::WorldEdits::from_entries(entries).unwrap();
    let chunk = Chunk::generate(&terrain, center);
    let mut light = super::CoastalLight::prepare(
        &chunk,
        &terrain,
        Some(&edits),
        [center.x * 16, center.z * 16],
        |[x, y, z]| chunk.block(x, y, z),
    );
    assert_eq!(light.at([cx, crate::world::SEA_LEVEL + 3, cz], 0.0), 0.0);
}
