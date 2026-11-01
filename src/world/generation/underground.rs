//! World-space cave features projected once per column, not once per voxel.

use crate::world::{
    Block, CHUNK_SIZE, CaveBiome, CaveLandform, CaveLandmark, Chunk, ChunkCoord, Column,
    MIN_WORLD_Y, SEA_LEVEL, Terrain, WORLD_DEPTH, WORLD_HEIGHT, split_block_coordinate,
};

const CELL_SIZE: i64 = 80;
const ENTRANCE_RUN: f64 = 128.0;
const VEIN_CELL: i64 = 16;
const VEIN_LAYER: i64 = 12;
const MASK_WORDS: usize = (WORLD_DEPTH as usize).div_ceil(64);
const FIRST_CAVE_Y: i32 = MIN_WORLD_Y + 4;
const NETWORK_REACH: f64 = 178.0;
const GROWTH_CELL: i64 = 10;
const GROWTH_REACH: f64 = 7.0;
const GROWTH_ATTEMPTS: u64 = 2;

use super::noise::{hash_2d, value_noise};
mod column;
mod diagnostics;
mod geology;
mod geometry;
mod growth;
mod landforms;
mod network;
mod pools;
pub(super) use column::sample;
#[cfg(test)]
use diagnostics::{
    cached_block, diagnostic_clear, diagnostic_climate_core, diagnostic_walls_match,
};
pub(super) use diagnostics::{cave_landmark, cave_position, entrance_position};
use geology::{Climate, Geology, Vein};
pub(super) use geology::{bedrock_top, cave_biome};
use geometry::{Bounds, Feature, Mask, Room};
#[cfg(test)]
use geometry::{Shape, segment_projection};
use growth::Growth;
use landforms::RetainedRock;
pub(super) use network::cave_landform;
use network::{Node, entrance, node};
use pools::Pool;

#[derive(Clone, Copy)]
struct Projection {
    void: Mask,
    protected: Mask,
}

/// One bounded feature list shared by every horizontal column of a chunk.
pub(in crate::world::generation) struct UndergroundRegion {
    seed: u64,
    caves: Vec<Feature>,
    routes: Vec<Feature>,
    retained: Vec<RetainedRock>,
    pools: Vec<Pool>,
    veins: Vec<Vein>,
    growths: Vec<Growth>,
}

impl UndergroundRegion {
    pub(in crate::world::generation) fn columns(
        terrain: &Terrain,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) -> Self {
        Self::new(
            terrain,
            Bounds {
                min_x: f64::from(min_x) - 0.5,
                max_x: f64::from(max_x) + 1.5,
                min_z: f64::from(min_z) - 0.5,
                max_z: f64::from(max_z) + 1.5,
                min_y: f64::from(MIN_WORLD_Y),
                max_y: f64::from(WORLD_HEIGHT),
            },
        )
    }

    pub(in crate::world::generation) fn chunk(terrain: &Terrain, x: i32, z: i32) -> Self {
        Self::new(
            terrain,
            Bounds {
                min_x: f64::from(x) - 0.5,
                max_x: f64::from(x) + f64::from(CHUNK_SIZE) + 0.5,
                min_z: f64::from(z) - 0.5,
                max_z: f64::from(z) + f64::from(CHUNK_SIZE) + 0.5,
                min_y: f64::from(MIN_WORLD_Y),
                max_y: f64::from(WORLD_HEIGHT),
            },
        )
    }

    pub(in crate::world::generation) fn vertical_column(terrain: &Terrain, x: i32, z: i32) -> Self {
        Self::new(
            terrain,
            Bounds {
                min_x: f64::from(x) - 0.5,
                max_x: f64::from(x) + 1.5,
                min_z: f64::from(z) - 0.5,
                max_z: f64::from(z) + 1.5,
                min_y: f64::from(MIN_WORLD_Y),
                max_y: f64::from(WORLD_HEIGHT),
            },
        )
    }

    pub(super) fn sample(terrain: &Terrain, x: i32, z: i32) -> Self {
        Self::vertical_column(terrain, x, z)
    }

    fn new(terrain: &Terrain, query: Bounds) -> Self {
        let seed = terrain.seed();
        // Roots need the complete topology in their halo. A Y-clipped feature
        // list would miss a ceiling root while directly sampling its lower tip.
        let topology_query = Bounds {
            min_y: f64::from(MIN_WORLD_Y),
            max_y: f64::from(WORLD_HEIGHT),
            ..query.expanded(GROWTH_REACH * 2.0 + 1.0)
        };
        let mut region = Self {
            seed,
            caves: Vec::with_capacity(32),
            routes: Vec::with_capacity(24),
            retained: Vec::with_capacity(12),
            pools: Vec::with_capacity(4),
            veins: Vec::with_capacity(32),
            growths: Vec::with_capacity(12),
        };

        if query.min_y < 18.0 {
            for cell_z in cells(
                topology_query.min_z - NETWORK_REACH,
                topology_query.max_z + 64.0,
                CELL_SIZE,
            ) {
                for cell_x in cells(
                    topology_query.min_x - NETWORK_REACH,
                    topology_query.max_x + 64.0,
                    CELL_SIZE,
                ) {
                    for deep in [false, true] {
                        let here = node(seed, cell_x, cell_z, deep);
                        region.add_landform(here, topology_query);
                        if let Some(pool) = Pool::from_node(here, deep) {
                            region.push_cave(
                                Feature::room(Room {
                                    origin: [pool.x, f64::from(pool.level), pool.z],
                                    length: pool.radius + 5.0,
                                    width: pool.radius + 4.0,
                                    height: 10.0,
                                    landform: CaveLandform::Gallery,
                                    ..here.room()
                                }),
                                topology_query,
                            );
                            if pool.bounds().intersects(topology_query) {
                                region.pools.push(pool);
                            }
                        }
                        for (dx, dz) in [(1, 0), (0, 1)] {
                            let neighbor = node(seed, cell_x + dx, cell_z + dz, deep);
                            region.connect(
                                here,
                                neighbor,
                                3.3 + ((here.hash >> 4) % 3) as f64 * 0.25,
                                6.8,
                                topology_query,
                            );
                        }
                        if !deep {
                            let lower = node(seed, cell_x + 1, cell_z + 1, true);
                            region.connect(here, lower, 3.8, 7.2, topology_query);
                        }
                    }
                }
            }
        }

        for cell_z in cells(
            topology_query.min_z - 205.0,
            topology_query.max_z + 115.0,
            CELL_SIZE,
        ) {
            for cell_x in cells(
                topology_query.min_x - 205.0,
                topology_query.max_x + 115.0,
                CELL_SIZE,
            ) {
                if let Some(entrance) = entrance(terrain, cell_x, cell_z, topology_query) {
                    let mouth_floor = [
                        entrance.mouth[0],
                        entrance.mouth[1] - 3.0,
                        entrance.mouth[2],
                    ];
                    region.add_passage(
                        entrance.inner.anchor(),
                        entrance.middle,
                        4.7,
                        8.0,
                        0.0,
                        topology_query,
                    );
                    region.add_passage(entrance.middle, mouth_floor, 5.2, 8.0, 1.0, topology_query);
                }
            }
        }

        region.populate_veins(query);
        region.populate_growths(terrain, query);
        region
    }

    fn projection(&self, x: i64, z: i64) -> Projection {
        let x = x as f64 + 0.5;
        let z = z as f64 + 0.5;
        let mut mask = Mask::default();
        for feature in &self.caves {
            if let Some((first, last)) = feature.span(x, z) {
                mask.range(first, last, true);
            }
        }
        for rock in &self.retained {
            rock.apply(&mut mask, x, z);
        }
        let mut floors = [i32::MAX; WORLD_DEPTH as usize];
        let mut protected = Mask::default();
        for feature in &self.routes {
            if let Some((first, last, core)) = feature.passage_span(x, z) {
                mask.range(first, last, true);
                if core {
                    protected.range(first, first + 4, true);
                    if (FIRST_CAVE_Y..WORLD_HEIGHT).contains(&first) {
                        floors[(first - MIN_WORLD_Y) as usize] = first;
                    }
                }
            }
        }
        mask.union(protected);
        // Keep ramp supports even where the surrounding room continues below
        // them. The upper surface of overlapping low routes is a walkable join;
        // deleting its support because another route has air there makes drops.
        for (index, &first) in floors.iter().enumerate() {
            if first == i32::MAX {
                continue;
            }
            let joins_higher = floors[index + 1..(index + 5).min(floors.len())]
                .iter()
                .any(|&floor| floor != i32::MAX);
            if !joins_higher {
                mask.range(MIN_WORLD_Y + index as i32 - 1, first - 1, false);
            }
        }
        for pool in &self.pools {
            if let Some(inside) = pool.column(x, z) {
                pool.apply_mask(&mut mask, inside);
                if inside && protected.contains(pool.level) {
                    // A crossing route has a dry rock bridge over the sealed
                    // basin, reached by the one-block bank on either side.
                    mask.range(pool.level, pool.level, false);
                    mask.range(pool.level + 1, pool.level + 5, true);
                    protected.range(pool.level + 1, pool.level + 5, true);
                }
            }
        }
        Projection {
            void: mask,
            protected,
        }
    }

    fn void_mask(&self, x: i64, z: i64) -> Mask {
        self.projection(x, z).void
    }
}

fn cells(min: f64, max: f64, size: i64) -> std::ops::RangeInclusive<i64> {
    (min.floor() as i64).div_euclid(size)..=(max.floor() as i64).div_euclid(size)
}

fn unit(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1_u64 << 53) as f64
}

fn hash_3d(seed: u64, x: i64, y: i64, z: i64) -> u64 {
    hash_2d(seed ^ (y as u64).wrapping_mul(0xA24B_AED4_963E_E407), x, z)
}

#[cfg(test)]
mod tests;
