use super::*;
use crate::world::{CHUNK_SIZE, WORLD_DEPTH, WaterColumn};

fn empty_chunk(coord: ChunkCoord) -> Chunk {
    Chunk {
        coord,
        blocks: vec![Block::Air; (CHUNK_SIZE * CHUNK_SIZE * WORLD_DEPTH) as usize]
            .into_boxed_slice(),
        water_columns: Box::new([None; (CHUNK_SIZE * CHUNK_SIZE) as usize]),
        coastal_roofs: Box::new([None; (CHUNK_SIZE * CHUNK_SIZE) as usize]),
    }
}

#[test]
fn edits_accept_loaded_non_noops_and_preserve_all_axis_bounds() {
    let coord = ChunkCoord::new(-1, -1);
    let mut world = ResidentWorld::default();
    assert!(!world.edit_block([-1, 1, -1], Block::Stone));
    world.insert(empty_chunk(coord));
    let revision = world.chunk_revision(coord);
    assert!(!world.edit_block([-1, 1, -1], Block::Air));
    for position in [
        [-1, MIN_WORLD_Y - 1, -1],
        [-1, WORLD_HEIGHT, -1],
        [-17, -1, -1],
        [-1, -1, 0],
        [i32::MIN, 1, i32::MAX],
    ] {
        assert!(!world.edit_block(position, Block::Stone));
    }
    assert_eq!(world.edits().revision(), 0);
    assert_eq!(world.chunk_revision(coord), revision);
    for position in [[-16, MIN_WORLD_Y, -16], [-1, WORLD_HEIGHT - 1, -1]] {
        assert!(world.edit_block(position, Block::Wood));
        assert_eq!(
            world.block_at(position[0], position[1], position[2]),
            Some(Block::Wood)
        );
        assert_eq!(
            world
                .edits()
                .block_at(position[0], position[1], position[2]),
            Some(Block::Wood)
        );
    }
    let edit_revision = world.edits().revision();
    let dense_revision = world.chunk_revision(coord);
    assert_ne!(dense_revision, revision);
    assert!(!world.edit_block([-1, WORLD_HEIGHT - 1, -1], Block::Wood));
    assert_eq!(world.edits().revision(), edit_revision);
    assert_eq!(world.chunk_revision(coord), dense_revision);
    let mut chunk = empty_chunk(coord);
    for local in [
        [-1, 0, 0],
        [CHUNK_SIZE, 0, 0],
        [0, MIN_WORLD_Y - 1, 0],
        [0, 0, CHUNK_SIZE],
    ] {
        assert!(!chunk.set_block(local, Block::Stone));
    }
}

#[test]
fn voxel_edits_invalidate_only_the_owning_dense_chunk_revision() {
    let first = ChunkCoord::new(-1, 0);
    let second = ChunkCoord::new(0, 0);
    let mut world = ResidentWorld::default();
    world.insert(empty_chunk(first));
    world.insert(empty_chunk(second));
    let first_revision = world.chunk_revision(first);
    let second_revision = world.chunk_revision(second);
    assert!(world.edit_block([-1, -1, 0], Block::Stone));
    assert_ne!(world.chunk_revision(first), first_revision);
    assert_eq!(world.chunk_revision(second), second_revision);
    let edit_revision = world.edits().revision();
    world.remove(second);
    assert_eq!(world.edits().revision(), edit_revision);
    world.insert(empty_chunk(second));
    assert_eq!(world.edits().revision(), edit_revision);
    assert!(world.edit_block([-1, -1, 0], Block::Air));
    assert!(world.edits().revision() > edit_revision);
    assert_eq!(world.edits().block_at(-1, -1, 0), Some(Block::Air));
}

#[test]
fn eviction_keeps_edits_in_fallback_and_defensively_overrides_stale_insertions() {
    let terrain = Terrain::new(42);
    let coord = ChunkCoord::new(-1, -1);
    let position = [-1, -20, -1];
    let original = terrain.block_at(position[0], position[1], position[2]);
    let block = if original == Block::Air {
        Block::Wood
    } else {
        Block::Air
    };
    let pristine = Chunk::generate(&terrain, coord);
    let mut world = ResidentWorld::default();
    world.insert(pristine.clone());
    assert!(world.edit_block(position, block));
    let revision = world.edits().revision();
    world.remove(coord);
    assert_eq!(world.block_at(position[0], position[1], position[2]), None);
    assert!(!world.edit_block(position, Block::Stone));
    assert_eq!(world.edits().revision(), revision);
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(
        sampled.block_at(position[0], position[1], position[2]),
        Some(block)
    );
    assert_eq!(
        sampled.block_at(-2, -20, -1),
        Some(terrain.block_at(-2, -20, -1))
    );
    assert_eq!(
        sampled.water_at(position[0], position[1], position[2]),
        None
    );
    world.insert(pristine);
    assert_eq!(
        world.block_at(position[0], position[1], position[2]),
        Some(block)
    );
    assert_eq!(world.edits().revision(), revision);
    world.remove(coord);
    world.clear();
    assert!(world.edits().revision() > revision);
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(
        sampled.block_at(position[0], position[1], position[2]),
        Some(original)
    );
}

#[test]
fn fractional_static_water_survives_solid_roof_edits_and_eviction() {
    let terrain = Terrain::default();
    let [x, y, z] = terrain.water_landmark(WaterBiome::River).unwrap().water;
    let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
    let profile = terrain.water_at(x, y, z).unwrap();
    let roof_y = profile.surface.ceil() as i32 + 1;
    let mut world = ResidentWorld::default();
    world.insert(Chunk::generate(&terrain, coord));
    assert!(world.edit_block([x, roof_y, z], Block::Stone));
    assert_eq!(world.water_at(x, y, z), Some(profile));
    world.remove(coord);
    let sampled = SampledWorld::new(&world, &terrain);
    assert_eq!(sampled.block_at(x, y, z), Some(Block::Water));
    assert_eq!(sampled.water_at(x, y, z), Some(profile));
    assert_eq!(sampled.water_at(x, roof_y, z), None);
    for outside in [MIN_WORLD_Y - 1, WORLD_HEIGHT] {
        assert_eq!(sampled.water_at(x, outside, z), None);
    }
    world.insert(Chunk::generate(&terrain, coord));
    assert_eq!(world.water_at(x, y, z), Some(profile));
    assert_eq!(world.block_at(x, roof_y, z), Some(Block::Stone));
}

#[test]
fn dense_water_reads_require_occupancy_and_keep_unmodified_fractional_metadata() {
    let coord = ChunkCoord::new(-1, -1);
    let mut chunk = empty_chunk(coord);
    let profile = WaterColumn {
        bottom: -4,
        surface: -0.35,
        flow: [0.3, 0.0, -0.1],
        biome: WaterBiome::River,
        salinity: 0.15,
    };
    chunk.water_columns[(15 * CHUNK_SIZE + 15) as usize] = Some(profile);
    for y in -3..=0 {
        chunk.set_block([15, y, 15], Block::Water);
    }
    let mut world = ResidentWorld::default();
    world.insert(chunk);
    assert_eq!(world.water_at(-1, -1, -1), Some(profile));
    assert!(world.edit_block([-1, -1, -1], Block::Clay));
    assert_eq!(world.water_at(-1, -1, -1), None);
    assert_eq!(world.water_at(-1, -2, -1), Some(profile));
    let revision = world.edits().revision();
    world.clear();
    assert!(world.is_empty());
    assert_eq!(world.water_at(-1, -2, -1), None);
    assert!(world.edits().revision() > revision);
}

#[test]
fn edited_cave_pool_fallback_matches_resident_water_and_known_dry_columns() {
    let terrain = Terrain::default();
    let mut found = None;
    'chunks: for chunk_z in -6..=6 {
        for chunk_x in -6..=6 {
            let coord = ChunkCoord::new(chunk_x, chunk_z);
            let chunk = Chunk::generate(&terrain, coord);
            for local_z in 0..CHUNK_SIZE {
                for local_x in 0..CHUNK_SIZE {
                    if let Some((y, profile)) = (MIN_WORLD_Y..0).find_map(|y| {
                        chunk
                            .water(local_x, y, local_z)
                            .filter(|water| water.biome == WaterBiome::CavePool)
                            .map(|profile| (y, profile))
                    }) {
                        found = Some((
                            chunk,
                            [
                                chunk_x * CHUNK_SIZE + local_x,
                                y,
                                chunk_z * CHUNK_SIZE + local_z,
                            ],
                            profile,
                        ));
                        break 'chunks;
                    }
                }
            }
        }
    }
    let (chunk, [x, y, z], profile) = found.expect("nearby generated cave pool");
    let coord = chunk.coord();
    let mut resident = ResidentWorld::default();
    resident.insert(chunk);
    assert!(resident.edit_block([x, WORLD_HEIGHT - 1, z], Block::Wood));
    assert_eq!(resident.water_at(x, y, z), Some(profile));
    resident.remove(coord);
    let sampled = SampledWorld::new(&resident, &terrain);
    assert_eq!(sampled.water_at(x, y, z), Some(profile));
    assert_eq!(sampled.water_at(x, WORLD_HEIGHT - 2, z), None);
    assert_eq!(sampled.water_at(x, WORLD_HEIGHT - 1, z), None);
    assert_eq!(sampled.water_at(x + CHUNK_SIZE, WORLD_HEIGHT - 1, z), None);
    resident.insert(Chunk::generate(&terrain, coord));
    assert_eq!(resident.water_at(x, y, z), Some(profile));
}
