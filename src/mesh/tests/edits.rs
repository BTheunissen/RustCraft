use super::*;
use crate::world::{ResidentWorld, WorldView};

fn face_at(mesh: &Mesh, source: [i32; 3], normal: [f32; 3]) -> Option<&[Vertex; 4]> {
    faces(mesh)
        .iter()
        .find(|face| source_block(face) == source && face[0].normal == normal)
}

pub(super) fn assert_same_mesh(first: &Mesh, second: &Mesh) {
    assert_eq!(first.indices, second.indices);
    assert_eq!(
        bytemuck::cast_slice::<Vertex, u8>(&first.vertices),
        bytemuck::cast_slice::<Vertex, u8>(&second.vertices)
    );
}

#[test]
fn empty_edit_snapshots_preserve_no_edit_meshes_for_default_and_seed_42() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let chunk = Chunk::generate(&terrain, ChunkCoord::new(-1, -1));
        let original = mesh_chunk(&chunk, &terrain);
        let edited = mesh_chunk_with_edits(&chunk, &terrain, &WorldEdits::default());
        assert_same_mesh(&original.opaque, &edited.opaque);
        assert_same_mesh(&original.water, &edited.water);
    }
}

#[test]
fn raw_and_overlaid_chunks_mesh_identically_and_cull_edited_negative_seams() {
    let terrain = Terrain::default();
    let first_coord = ChunkCoord::new(-1, -1);
    let second_coord = ChunkCoord::new(0, -1);
    let first = Chunk::generate(&terrain, first_coord);
    let second = Chunk::generate(&terrain, second_coord);
    let source = [-1, WORLD_HEIGHT - 4, -8];
    let neighbor = [0, source[1], -8];
    let mut resident = ResidentWorld::default();
    resident.insert(first.clone());
    resident.insert(second.clone());
    assert!(resident.edit_block(source, Block::Stone));
    assert!(resident.edit_block(neighbor, Block::Wood));
    let first_mesh = mesh_chunk_with_edits(&first, &terrain, resident.edits());
    let second_mesh = mesh_chunk_with_edits(&second, &terrain, resident.edits());
    assert_valid_mesh(&first_mesh.opaque);
    assert_valid_mesh(&second_mesh.opaque);
    assert!(face_at(&first_mesh.opaque, source, [1.0, 0.0, 0.0]).is_none());
    assert!(face_at(&second_mesh.opaque, neighbor, [-1.0, 0.0, 0.0]).is_none());
    let mut overlaid = first.clone();
    resident.edits().apply_to_chunk(&mut overlaid);
    let overlaid_mesh = mesh_chunk_with_edits(&overlaid, &terrain, resident.edits());
    assert_same_mesh(&first_mesh.opaque, &overlaid_mesh.opaque);
    assert_same_mesh(&first_mesh.water, &overlaid_mesh.water);
    assert!(resident.edit_block(neighbor, Block::Air));
    let opened = mesh_chunk_with_edits(&first, &terrain, resident.edits());
    let seam = face_at(&opened.opaque, source, [1.0, 0.0, 0.0]).unwrap();
    assert_eq!(seam[0].light, 1.0);
    assert!(seam.iter().all(|vertex| vertex.position[0] == 0.0));
}

#[test]
fn edited_halo_roofs_match_local_daylight_even_for_open_negative_y_shafts() {
    let terrain = Terrain::new(42);
    let first = Chunk::generate(&terrain, ChunkCoord::new(-1, -1));
    let second = Chunk::generate(&terrain, ChunkCoord::new(0, -1));
    let mut resident = ResidentWorld::default();
    resident.insert(first.clone());
    resident.insert(second.clone());
    let floor_y = -30;
    for y in floor_y + 1..WORLD_HEIGHT {
        for x in [-1, 0] {
            resident.edit_block([x, y, -8], Block::Air);
        }
    }
    resident.edit_block([-1, floor_y + 1, -8], Block::DeepStone);
    resident.edit_block([0, floor_y, -8], Block::DeepStone);
    let source = [-1, floor_y + 1, -8];
    let floor = [0, floor_y, -8];
    for (roof, expected) in [
        (Block::Stone, 0.0),
        (Block::Leaves, 0.65),
        (Block::Air, 1.0),
    ] {
        assert!(resident.edit_block([0, WORLD_HEIGHT - 3, -8], roof));
        let halo_mesh = mesh_chunk_with_edits(&first, &terrain, resident.edits());
        let local_mesh = mesh_chunk_with_edits(&second, &terrain, resident.edits());
        let halo = face_at(&halo_mesh.opaque, source, [1.0, 0.0, 0.0]).unwrap();
        let local = face_at(&local_mesh.opaque, floor, [0.0, 1.0, 0.0]).unwrap();
        assert_eq!(halo[0].light, expected);
        assert_eq!(halo[0].light, local[0].light);
        assert_eq!(
            halo[0].light,
            reference_daylight(floor_y + 1, |y| resident.block_at(0, y, -8).unwrap())
        );
    }
}

#[test]
fn diagonal_edited_ceilings_update_shared_fractional_water_corners() {
    let terrain = Terrain::default();
    let mut found = None;
    'sites: for biome in [WaterBiome::Ocean, WaterBiome::Lake, WaterBiome::River] {
        let Some(site) = terrain.water_landmark(biome) else {
            continue;
        };
        let center_x = site.water[0].div_euclid(CHUNK_SIZE);
        let center_z = site.water[2].div_euclid(CHUNK_SIZE);
        for z in center_z - 2..=center_z + 2 {
            for x in center_x - 2..=center_x + 2 {
                let corner = [x * CHUNK_SIZE, z * CHUNK_SIZE];
                let positions = [
                    [corner[0] - 1, corner[1] - 1],
                    [corner[0], corner[1] - 1],
                    [corner[0] - 1, corner[1]],
                    corner,
                ];
                let Some(profile) = terrain.water_column(corner[0], corner[1]) else {
                    continue;
                };
                if profile.depth() > 2.0
                    && positions.iter().all(|&[x, z]| {
                        terrain.water_column(x, z).is_some_and(|other| {
                            other.surface == profile.surface && other.depth() > 2.0
                        })
                    })
                {
                    found = Some((ChunkCoord::new(x, z), corner, profile));
                    break 'sites;
                }
            }
        }
    }
    let (coord, corner, profile) = found.expect("deep water at a four-chunk corner");
    let diagonal_coord = ChunkCoord::new(coord.x - 1, coord.z - 1);
    let diagonal = [corner[0] - 1, profile.surface.ceil() as i32, corner[1] - 1];
    let chunk = Chunk::generate(&terrain, coord);
    let west = Chunk::generate(&terrain, ChunkCoord::new(coord.x - 1, coord.z));
    let mut resident = ResidentWorld::default();
    let diagonal_chunk = Chunk::generate(&terrain, diagonal_coord);
    resident.insert(diagonal_chunk.clone());
    let original_diagonal = mesh_chunk(&diagonal_chunk, &terrain);
    let original_west = mesh_chunk(&west, &terrain);
    assert!(resident.edit_block(diagonal, Block::Stone));
    resident.remove(diagonal_coord);
    let before = mesh_chunk(&chunk, &terrain);
    let after = mesh_chunk_with_edits(&chunk, &terrain, resident.edits());
    let adjacent = mesh_chunk_with_edits(&west, &terrain, resident.edits());
    let roofed_diagonal = mesh_chunk_with_edits(&diagonal_chunk, &terrain, resident.edits());
    let mut overlaid_diagonal = diagonal_chunk.clone();
    resident.edits().apply_to_chunk(&mut overlaid_diagonal);
    let roofed_overlay = mesh_chunk_with_edits(&overlaid_diagonal, &terrain, resident.edits());
    assert_same_mesh(&roofed_diagonal.water, &roofed_overlay.water);
    let target = [corner[0] as f32, profile.surface, corner[1] as f32];
    let top_corner = |mesh: &Mesh| {
        mesh.vertices
            .iter()
            .find(|vertex| vertex.position == target && vertex.normal[1] == 1.0)
            .copied()
            .unwrap()
    };
    let original = top_corner(&before.water);
    let edited = top_corner(&after.water);
    let shared = top_corner(&adjacent.water);
    assert!(original.water_extra[3] > 0.0);
    assert_eq!(edited.water_extra[3], 0.0);
    assert_eq!(edited.water, shared.water);
    assert_eq!(edited.water_extra, shared.water_extra);
    if profile.surface.fract() != 0.0 {
        let ceiling_cap = top_corner(&roofed_diagonal.water);
        assert_eq!(edited.water, ceiling_cap.water);
        assert_eq!(edited.water_extra, ceiling_cap.water_extra);
    }
    assert_eq!(
        face_count(&roofed_diagonal.water) + usize::from(profile.surface.fract() == 0.0),
        face_count(&original_diagonal.water)
    );
    assert_eq!(face_count(&before.water), face_count(&after.water));
    assert_valid_mesh(&after.water);
    assert_valid_mesh(&adjacent.water);
    resident.insert(Chunk::generate(&terrain, diagonal_coord));
    let reloaded = mesh_chunk_with_edits(
        resident.chunk(diagonal_coord).unwrap(),
        &terrain,
        resident.edits(),
    );
    assert_same_mesh(&roofed_diagonal.water, &reloaded.water);
    assert!(resident.edit_block(diagonal, Block::Air));
    let restored = mesh_chunk_with_edits(&chunk, &terrain, resident.edits());
    let restored_west = mesh_chunk_with_edits(&west, &terrain, resident.edits());
    let restored_diagonal = mesh_chunk_with_edits(&diagonal_chunk, &terrain, resident.edits());
    assert_same_mesh(&before.water, &restored.water);
    assert_same_mesh(&original_west.water, &restored_west.water);
    assert_same_mesh(&original_diagonal.water, &restored_diagonal.water);
    let restored_corner = top_corner(&restored.water);
    assert_eq!(restored_corner.water, original.water);
    assert_eq!(restored_corner.water_extra, original.water_extra);
}

#[test]
fn edited_meshes_keep_unrepresentable_chunk_origins_empty() {
    let terrain = Terrain::new(42);
    let edits = WorldEdits::default();
    for coord in [ChunkCoord::new(i32::MAX, 0), ChunkCoord::new(0, i32::MIN)] {
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk_with_edits(&chunk, &terrain, &edits);
        assert_eq!(face_count(&mesh.opaque), 0);
        assert_eq!(face_count(&mesh.water), 0);
    }
}
