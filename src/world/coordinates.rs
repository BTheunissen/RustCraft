//! Chunk and world coordinate contracts.

/// Number of blocks along the horizontal axes of a chunk.
pub const CHUNK_SIZE: i32 = 16;

/// Exclusive upper bound of the world's vertical block coordinates.
pub const WORLD_HEIGHT: i32 = 80;

/// Inclusive lower bound of the world's vertical block coordinates.
pub const MIN_WORLD_Y: i32 = -64;

/// Number of blocks in a complete vertical chunk column.
pub const WORLD_DEPTH: i32 = WORLD_HEIGHT - MIN_WORLD_Y;

/// Mean ocean elevation; inland water uses its own local surface profile.
pub const SEA_LEVEL: i32 = 18;

pub(super) const MIN_TERRAIN_HEIGHT: i32 = -12;
pub(super) const MIN_BASE_TERRAIN_HEIGHT: i32 = 6;
pub(super) const MAX_TERRAIN_HEIGHT: i32 = WORLD_HEIGHT - 10;

/// A horizontal chunk coordinate, spanning `MIN_WORLD_Y..WORLD_HEIGHT` in Y.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkCoord {
    /// Chunk index along X, not a block coordinate.
    pub x: i32,
    /// Chunk index along Z, not a block coordinate.
    pub z: i32,
}

impl ChunkCoord {
    /// Creates a coordinate from horizontal chunk indices.
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Finds the chunk containing a finite world-space position using floor division.
    pub fn from_world(x: f32, z: f32) -> Self {
        Self::new(
            (x / CHUNK_SIZE as f32).floor() as i32,
            (z / CHUNK_SIZE as f32).floor() as i32,
        )
    }
}

/// Splits a world block coordinate into its chunk coordinate and local offset.
///
/// The local offset is always in `0..CHUNK_SIZE`. Negative world coordinates
/// use floor division so that blocks immediately below zero belong to chunk -1.
///
/// ```
/// use rustcraft::world::split_block_coordinate;
///
/// assert_eq!(split_block_coordinate(16), (1, 0));
/// assert_eq!(split_block_coordinate(-1), (-1, 15));
/// ```
pub fn split_block_coordinate(coordinate: i32) -> (i32, i32) {
    (
        coordinate.div_euclid(CHUNK_SIZE),
        coordinate.rem_euclid(CHUNK_SIZE),
    )
}
