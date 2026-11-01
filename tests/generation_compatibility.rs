//! Golden seeded-world fingerprints, including actual fluid metadata.
//! Updated deliberately for generation version 2's limestone coast and stack occupancy.

use rustcraft::world::{CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain, WORLD_HEIGHT};

#[test]
fn seeded_world_generation_fingerprints_remain_stable() {
    for (terrain, expected) in [
        (Terrain::default(), 0xfe86_4690_7722_cf0b),
        (Terrain::new(42), 0xd8c4_0b8f_927f_7819),
    ] {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for &byte in bytes {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        };
        for coordinate in terrain.spawn_position() {
            feed(&coordinate.to_bits().to_le_bytes());
        }
        for coord in [
            ChunkCoord::new(0, 0),
            ChunkCoord::new(-2, -1),
            ChunkCoord::new(2, -18),
            ChunkCoord::new(28, 3),
            ChunkCoord::new(-18, -4),
            ChunkCoord::new(-18, -24),
        ] {
            let chunk = Chunk::generate(&terrain, coord);
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y..WORLD_HEIGHT {
                        feed(&[chunk.block(x, y, z) as u8]);
                        if let Some(water) = chunk.water(x, y, z) {
                            feed(&water.bottom.to_le_bytes());
                            feed(&water.surface.to_bits().to_le_bytes());
                            for flow in water.flow {
                                feed(&flow.to_bits().to_le_bytes());
                            }
                            feed(&[water.biome as u8]);
                            feed(&water.salinity.to_bits().to_le_bytes());
                        }
                    }
                }
            }
        }
        assert_eq!(
            hash,
            expected,
            "seed {} changed without an intentional generation update",
            terrain.seed()
        );
    }
}
