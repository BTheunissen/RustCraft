use super::*;
use crate::world::{BlockEdit, Chunk, ChunkCoord, ResidentWorld};

#[test]
fn sparse_halo_sources_fields_and_payloads_reload_consistently_across_negative_seams() {
    let terrain = Terrain::new(42);
    let mut entries = Vec::new();
    for z in -4..=4 {
        for x in -20..=4 {
            for y in -32..=-27 {
                entries.push(BlockEdit {
                    position: [x, y, z],
                    block: if y == -32 { Block::Stone } else { Block::Air },
                });
            }
        }
    }
    entries
        .iter_mut()
        .find(|edit| edit.position == [-1, -30, 0])
        .unwrap()
        .block = Block::Lantern;
    let edits = WorldEdits::from_entries(entries).unwrap();
    assert_eq!(edits.lantern_sources([-16, 0]), vec![[-1, -30, 0]]);
    assert!(edits.lantern_sources([1000, 1000]).is_empty());
    let left = prepare([-16, 0], &terrain, Some(&edits)).unwrap();
    let right = prepare([0, 0], &terrain, Some(&edits)).unwrap();
    for x in -4..=4 {
        assert_eq!(left.sample([x, -30, 0]), right.sample([x, -30, 0]));
    }
    let mut blocked_entries = edits.entries();
    for edit in &mut blocked_entries {
        if edit.position[0] == 0 && edit.position[1] > -32 {
            edit.block = Block::Stone;
        }
    }
    let blocked = WorldEdits::from_entries(blocked_entries).unwrap();
    let blocked_field = prepare([0, 0], &terrain, Some(&blocked)).unwrap();
    assert_eq!(blocked_field.sample([1, -30, 0]), [0.0; 3]);
    let coord = ChunkCoord::new(-1, 0);
    let chunk = Chunk::generate(&terrain, coord);
    let mesh = crate::mesh::mesh_chunk_with_edits(&chunk, &terrain, &edits);
    assert!(
        mesh.opaque
            .vertices
            .iter()
            .any(|vertex| vertex.local_light[0] > 0.0)
    );
    let mut resident = ResidentWorld::with_edits(edits.clone());
    resident.insert(chunk);
    resident.remove(coord);
    let reloaded = Chunk::generate(&terrain, coord);
    let restored = crate::mesh::mesh_chunk_with_edits(&reloaded, &terrain, resident.edits());
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&mesh.opaque.vertices),
        bytemuck::cast_slice::<_, u8>(&restored.opaque.vertices)
    );
    let mut removed = edits.entries();
    removed
        .iter_mut()
        .find(|edit| edit.position == [-1, -30, 0])
        .unwrap()
        .block = Block::Air;
    let removed = WorldEdits::from_entries(removed).unwrap();
    assert!(prepare([-16, 0], &terrain, Some(&removed)).is_none());
    let unlit = crate::mesh::mesh_chunk_with_edits(&reloaded, &terrain, &removed);
    assert!(
        unlit
            .opaque
            .vertices
            .iter()
            .all(|vertex| vertex.local_light == [0.0; 3])
    );
}
