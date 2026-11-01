//! Cached resident fixtures using the world's ownership boundary.

use glam::Vec3;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{Game, GameMode};
use crate::world::{
    Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, ResidentWorld, Terrain, WORLD_HEIGHT,
};

pub(super) fn empty_chunk(coord: ChunkCoord) -> Chunk {
    static EMPTY: OnceLock<Mutex<HashMap<ChunkCoord, Chunk>>> = OnceLock::new();
    let mut chunks = EMPTY.get_or_init(Mutex::default).lock().unwrap();
    chunks
        .entry(coord)
        .or_insert_with(|| {
            let mut world = ResidentWorld::default();
            world.insert(Chunk::generate(&Terrain::new(42), coord));
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y..WORLD_HEIGHT {
                        world.edit_block(
                            [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z],
                            Block::Air,
                        );
                    }
                }
            }
            world.chunk(coord).unwrap().clone()
        })
        .clone()
}

pub(super) fn world(coords: &[ChunkCoord], blocks: &[([i32; 3], Block)]) -> ResidentWorld {
    let mut world = ResidentWorld::default();
    for &coord in coords {
        world.insert(empty_chunk(coord));
    }
    for &(position, block) in blocks {
        assert!(world.edit_block(position, block));
    }
    world
}

pub(crate) fn arena() -> Game {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let coord = ChunkCoord::new(0, 0);
    game.world.insert(empty_chunk(coord));
    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            game.world.edit_block([x, 0, z], Block::Stone);
        }
    }
    game.player.respawn(Vec3::new(8.5, 1.0, 8.5));
    game.player.yaw = 0.0;
    game.player.pitch = 0.0;
    game
}

pub(super) fn aim(game: &mut Game, position: Vec3) {
    let direction = (position - game.player.eye_position()).normalize();
    game.player.yaw = direction.x.atan2(-direction.z);
    game.player.pitch = direction.y.asin();
}
