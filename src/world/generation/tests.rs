use super::chunks::{BLOCK_COUNT, COLUMN_COUNT};
use super::{
    Biome, Block, CHUNK_SIZE, Chunk, ChunkCoord, MAX_TERRAIN_HEIGHT, MIN_BASE_TERRAIN_HEIGHT,
    MIN_TERRAIN_HEIGHT, MIN_WORLD_Y, SEA_LEVEL, TREE_RADIUS, Terrain, Tree, TreeCandidate,
    WORLD_DEPTH, WORLD_HEIGHT, split_block_coordinate, terrain_block, tree_cells,
};
use crate::world::chunk::block_index;
use crate::world::{CaveLandform, Column};

fn biome_index(biome: Biome) -> usize {
    match biome {
        Biome::Plains => 0,
        Biome::Forest => 1,
        Biome::Desert => 2,
        Biome::Alpine => 3,
    }
}

fn assert_chunk_matches(terrain: &Terrain, coord: ChunkCoord) {
    let chunk = Chunk::generate(terrain, coord);
    assert_eq!(chunk.coord(), coord);
    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            for y in MIN_WORLD_Y..WORLD_HEIGHT {
                assert_eq!(
                    chunk.block(x, y, z),
                    terrain.block_at(coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z),
                    "chunk {coord:?}, local ({x}, {y}, {z})"
                );
            }
        }
    }
}

fn find_seam_tree(terrain: &Terrain, negative: bool, along_x: bool) -> Tree {
    for cell_z in -40..=40 {
        for cell_x in -40..=40 {
            let Some(candidate) = terrain.tree_candidate(cell_x, cell_z) else {
                continue;
            };
            let coordinate = if along_x { candidate.x } else { candidate.z };
            if (coordinate < 0) != negative {
                continue;
            }
            let local = split_block_coordinate(coordinate).1;
            if local >= TREE_RADIUS as i32 && local < CHUNK_SIZE - TREE_RADIUS as i32 {
                continue;
            }
            if let Some(tree) =
                Tree::from_candidate(candidate, terrain.column(candidate.x, candidate.z))
                && terrain.block_at(tree.x, tree.base_y - 1, tree.z).is_solid()
            {
                return tree;
            }
        }
    }
    panic!("expected a tree crossing a chunk seam (negative={negative}, along_x={along_x})");
}

mod coordinates;
mod decorations;
mod sampling;
mod terrain;
