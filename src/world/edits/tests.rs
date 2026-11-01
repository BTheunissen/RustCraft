use super::*;
use crate::world::CHUNK_SIZE;

#[test]
fn sparse_overrides_validate_y_and_use_euclidean_negative_chunks() {
    let mut edits = WorldEdits::default();
    assert_eq!(edits.revision(), 0);
    for y in [i32::MIN, MIN_WORLD_Y - 1, WORLD_HEIGHT, i32::MAX] {
        assert!(!edits.record([-1, y, -17], Block::Stone));
        assert_eq!(edits.block_at(-1, y, -17), None);
    }
    assert_eq!(edits.revision(), 0);
    assert!(edits.record([-1, MIN_WORLD_Y, -17], Block::Stone));
    let first_revision = edits.revision();
    assert!(first_revision > 0);
    assert_eq!(edits.chunks.len(), 1);
    assert_eq!(edits.block_at(-1, MIN_WORLD_Y, -17), Some(Block::Stone));
    assert_eq!(edits.block_at(-1, MIN_WORLD_Y + 1, -17), None);
    assert_eq!(edits.block_at(-1, MIN_WORLD_Y, -16), None);
    assert!(!edits.record([-1, MIN_WORLD_Y, -17], Block::Stone));
    assert_eq!(edits.revision(), first_revision);
    assert!(edits.record([-1, WORLD_HEIGHT - 1, -17], Block::Wood));
    assert!(edits.revision() > first_revision);
    assert_eq!(edits.chunks.len(), 1);
    assert!(edits.chunks.contains_key(&ChunkCoord::new(-1, -2)));
}

#[test]
fn cloned_snapshots_copy_only_the_changed_sparse_chunk() {
    let mut edits = WorldEdits::default();
    edits.record([-1, -10, -1], Block::Air);
    edits.record([16, 2, 16], Block::Stone);
    let previous = edits.clone();
    assert!(edits.record([-1, -10, -1], Block::Wood));
    assert_eq!(previous.block_at(-1, -10, -1), Some(Block::Air));
    assert_eq!(edits.block_at(-1, -10, -1), Some(Block::Wood));
    assert_ne!(edits.revision(), previous.revision());
    for (coord, shared) in [
        (ChunkCoord::new(-1, -1), false),
        (ChunkCoord::new(1, 1), true),
    ] {
        assert_eq!(
            Arc::ptr_eq(&edits.chunks[&coord], &previous.chunks[&coord]),
            shared
        );
    }
}

#[test]
fn lantern_source_wall_and_removal_invalidate_negative_neighbor_dependency_snapshots() {
    let mut edits = WorldEdits::default();
    let previous = edits.clone();
    edits.record([-1, -30, -1], Block::Lantern);
    assert_eq!(edits.lantern_sources([-16, -16]), vec![[-1, -30, -1]]);
    assert_eq!(edits.lantern_sources([0, 0]), vec![[-1, -30, -1]]);
    let source_revision = edits.halo_revision(ChunkCoord::new(0, 0));
    assert!(source_revision > previous.halo_revision(ChunkCoord::new(0, 0)));
    assert!(
        edits
            .affected_chunks(&previous)
            .contains(&ChunkCoord::new(0, 0))
    );
    let source = edits.clone();
    edits.record([0, -30, -1], Block::Stone);
    assert!(
        edits.halo_revision(ChunkCoord::new(-1, -1))
            > source.halo_revision(ChunkCoord::new(-1, -1))
    );
    edits.record([-1, -30, -1], Block::Air);
    assert!(edits.lantern_sources([0, 0]).is_empty());
    assert_eq!(source.lantern_sources([0, 0]), vec![[-1, -30, -1]]);
}

#[test]
fn chunk_and_column_overlays_match_without_touching_other_columns_or_water_profiles() {
    let terrain = Terrain::new(42);
    let coord = ChunkCoord::new(-1, -2);
    let mut chunk = Chunk::generate(&terrain, coord);
    let original = chunk.clone();
    let mut edits = WorldEdits::default();
    for (position, block) in [
        ([-16, MIN_WORLD_Y, -32], Block::Clay),
        ([-1, WORLD_HEIGHT - 1, -17], Block::Wood),
        ([-1, -20, -17], Block::Air),
        ([0, 2, -17], Block::Stone),
    ] {
        assert!(edits.record(position, block));
    }
    edits.apply_to_chunk(&mut chunk);
    assert_eq!(chunk.water_columns, original.water_columns);
    for local_z in 0..CHUNK_SIZE {
        for local_x in 0..CHUNK_SIZE {
            let mut column = std::array::from_fn(|offset| {
                original.block(local_x, MIN_WORLD_Y + offset as i32, local_z)
            });
            let world = [-CHUNK_SIZE + local_x, -2 * CHUNK_SIZE + local_z];
            edits.apply_to_column(world, &mut column);
            for (offset, &block) in column.iter().enumerate() {
                assert_eq!(
                    chunk.block(local_x, MIN_WORLD_Y + offset as i32, local_z),
                    block
                );
            }
        }
    }
    let untouched = chunk.blocks.clone();
    edits.apply_to_chunk(&mut chunk);
    assert_eq!(chunk.blocks, untouched);
    let mut unknown_column = [Block::Sand; WORLD_DEPTH as usize];
    edits.apply_to_column([16, 16], &mut unknown_column);
    assert_eq!(unknown_column, [Block::Sand; WORLD_DEPTH as usize]);
}

#[test]
fn changed_chunks_include_diagonal_negative_seam_dependencies_but_not_distant_edits() {
    let mut edits = WorldEdits::default();
    edits.record([160, 1, 160], Block::Air);
    let previous = edits.clone();
    assert!(edits.affected_chunks(&previous).is_empty());
    edits.record([-1, -30, -1], Block::Stone);
    let affected = edits.affected_chunks(&previous);
    let expected: BTreeSet<_> = (-2..=0)
        .flat_map(|z| (-2..=0).map(move |x| ChunkCoord::new(x, z)))
        .collect();
    assert_eq!(affected, expected);
    assert_eq!(edits.halo_revision(ChunkCoord::new(0, 0)), edits.revision());
    assert_eq!(
        edits.halo_revision(ChunkCoord::new(-2, -2)),
        edits.revision()
    );
    assert_eq!(edits.halo_revision(ChunkCoord::new(1, 1)), 0);
    assert_eq!(
        edits.halo_revision(ChunkCoord::new(10, 10)),
        previous.revision()
    );
}

#[test]
fn clear_invalidates_removed_edits_and_distinguishes_new_scenes() {
    let mut edits = WorldEdits::default();
    edits.clear();
    assert_eq!(edits.revision(), 0);
    edits.record([1, 1, 1], Block::Stone);
    let previous = edits.clone();
    let mut other = WorldEdits::default();
    other.record([1, 1, 1], Block::Wood);
    assert_ne!(other.revision(), previous.revision());
    assert_eq!(other.affected_chunks(&previous).len(), 9);
    edits.clear();
    assert!(edits.revision() > previous.revision());
    assert_eq!(edits.block_at(1, 1, 1), None);
    assert_eq!(edits.affected_chunks(&previous).len(), 9);
    let cleared_revision = edits.revision();
    edits.clear();
    assert_eq!(edits.revision(), cleared_revision);
}

#[test]
fn dependency_halos_do_not_wrap_at_coordinate_limits() {
    for edge in [i32::MIN, i32::MAX] {
        let center = ChunkCoord::new(edge, edge);
        let neighbors: Vec<_> = halo(center).collect();
        assert_eq!(neighbors.len(), 4);
        assert!(neighbors.contains(&center));
        assert!(neighbors.iter().all(|coord| {
            (i64::from(coord.x) - i64::from(edge)).abs() <= 1
                && (i64::from(coord.z) - i64::from(edge)).abs() <= 1
        }));
    }
    let mut edits = WorldEdits::default();
    for edge in [i32::MIN, i32::MAX] {
        assert!(edits.record([edge, -1, edge], Block::Wood));
        assert_eq!(edits.block_at(edge, -1, edge), Some(Block::Wood));
    }
}

#[test]
fn persistent_entries_round_trip_stably_across_negative_and_extreme_coordinates() {
    let entries = [
        BlockEdit {
            position: [i32::MAX, WORLD_HEIGHT - 1, i32::MIN],
            block: Block::Wood,
        },
        BlockEdit {
            position: [-1, MIN_WORLD_Y, -17],
            block: Block::Air,
        },
        BlockEdit {
            position: [i32::MIN, -1, i32::MAX],
            block: Block::Clay,
        },
    ];
    let restored = WorldEdits::from_entries(entries).unwrap();
    let mut expected = entries.to_vec();
    expected.sort_unstable_by_key(|entry| entry.position);
    assert_eq!(restored.entries(), expected);
    assert_eq!(
        WorldEdits::from_entries(restored.entries())
            .unwrap()
            .entries(),
        expected
    );
    for entry in entries {
        let [x, y, z] = entry.position;
        assert_eq!(restored.block_at(x, y, z), Some(entry.block));
    }
    assert!(WorldEdits::from_entries([entries[0]; 2]).is_err());
    assert!(
        WorldEdits::from_entries([BlockEdit {
            position: [0, WORLD_HEIGHT, 0],
            block: Block::Air,
        }])
        .is_err()
    );
    assert!(WorldEdits::from_entries([]).unwrap().entries().is_empty());
}
