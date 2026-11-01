//! Underground climate, geological hosts, and decorative ore veins.

use super::{
    Block, Bounds, CaveBiome, Feature, MIN_WORLD_Y, UndergroundRegion, VEIN_CELL, VEIN_LAYER,
    cells, hash_2d, hash_3d, unit, value_noise,
};

#[derive(Clone, Copy)]
pub(super) struct Climate {
    pub(super) phase: f64,
}

impl Climate {
    pub(super) fn new(seed: u64, x: i32, z: i32) -> Self {
        let offset = unit(hash_2d(seed ^ 0xCA7E_C11A, 0, 0)) * std::f64::consts::TAU;
        Self {
            phase: f64::from(x) / 57.0
                + f64::from(z) / 93.0
                + offset
                + value_noise(seed ^ 0xCA7E_021D, f64::from(x) / 96.0, f64::from(z) / 96.0) * 0.55,
        }
    }

    pub(super) fn biome(self, y: i32) -> CaveBiome {
        let climate = (self.phase + f64::from(y) / 96.0).sin();
        if climate < -0.4 {
            CaveBiome::Lush
        } else if climate > 0.4 {
            CaveBiome::Crystal
        } else {
            CaveBiome::Rocky
        }
    }
}

pub(in crate::world::generation) fn cave_biome(seed: u64, x: i32, y: i32, z: i32) -> CaveBiome {
    Climate::new(seed, x, z).biome(y)
}

pub(in crate::world::generation) fn bedrock_top(seed: u64, x: i32, z: i32) -> i32 {
    MIN_WORLD_Y + (hash_2d(seed ^ 0xBED0_CAFE, i64::from(x), i64::from(z)) % 3) as i32
}

#[derive(Clone, Copy)]
pub(super) struct Geology {
    pub(super) bed_offset: f64,
    pub(super) deep_boundary: f64,
    pub(super) regional: f64,
    pub(super) dike: bool,
}

impl Geology {
    pub(super) fn new(seed: u64, x: i32, z: i32) -> Self {
        let x = f64::from(x);
        let z = f64::from(z);
        let phase = unit(hash_2d(seed ^ 0x6E01_091C, 0, 0)) * std::f64::consts::TAU;
        let fold = (x / 73.0 + z / 127.0 + phase).sin() * 5.0;
        let regional = value_noise(seed ^ 0x6E01_1A7E, x / 192.0, z / 192.0);
        let fault = (x * 0.72 - z * 0.46) / 109.0 + phase;
        Self {
            bed_offset: x * 0.09 + z * 0.055 + fold,
            deep_boundary: -26.0 + regional * 9.0 + fold * 0.7,
            regional,
            dike: fault.sin().abs() < 0.028,
        }
    }

    pub(super) fn host(self, y: i32) -> Block {
        let bed = (f64::from(y) + self.bed_offset).floor() as i64;
        if f64::from(y) < self.deep_boundary {
            if self.dike || self.regional < -0.32 || bed.rem_euclid(31) <= 2 {
                Block::Basalt
            } else if bed.rem_euclid(23) <= 4 {
                Block::Slate
            } else {
                Block::DeepStone
            }
        } else {
            match bed.rem_euclid(27) {
                0..=4 => Block::Slate,
                5..=20 => Block::Limestone,
                _ => Block::Stone,
            }
        }
    }
}

pub(super) struct Vein {
    pub(super) feature: Feature,
    pub(super) block: Block,
}

impl UndergroundRegion {
    pub(super) fn populate_veins(&mut self, query: Bounds) {
        for cell_z in cells(query.min_z - 8.0, query.max_z + 8.0, VEIN_CELL) {
            for cell_x in cells(query.min_x - 8.0, query.max_x + 8.0, VEIN_CELL) {
                for layer in cells(query.min_y - 4.0, query.max_y + 4.0, VEIN_LAYER) {
                    let hash = hash_3d(self.seed ^ 0x0AE5_7E11, cell_x, layer, cell_z);
                    if hash % 100 >= 68 {
                        continue;
                    }
                    let y = (layer * VEIN_LAYER + 3 + ((hash >> 8) % 6) as i64) as f64;
                    let block = match y as i32 {
                        -24..=14 if hash & 1 == 0 => Block::CoalOre,
                        -56..=4 if !hash.is_multiple_of(5) => Block::IronOre,
                        -58..=-28 => Block::GoldOre,
                        _ => continue,
                    };
                    let center = [
                        (cell_x * VEIN_CELL + 3 + ((hash >> 16) % 10) as i64) as f64 + 0.5,
                        y,
                        (cell_z * VEIN_CELL + 3 + ((hash >> 32) % 10) as i64) as f64 + 0.5,
                    ];
                    let angle = unit(hash.rotate_left(21)) * std::f64::consts::TAU;
                    let half_length = 2.5 + ((hash >> 48) % 4) as f64;
                    let slope = unit(hash.rotate_left(37)) - 0.5;
                    let offset = [
                        angle.cos() * half_length,
                        slope * 2.0,
                        angle.sin() * half_length,
                    ];
                    let feature = Feature::tunnel(
                        std::array::from_fn(|axis| center[axis] - offset[axis]),
                        std::array::from_fn(|axis| center[axis] + offset[axis]),
                        1.6,
                        1.7,
                    );
                    if feature.bounds.intersects(query) {
                        self.veins.push(Vein { feature, block });
                    }
                }
            }
        }
    }
}
