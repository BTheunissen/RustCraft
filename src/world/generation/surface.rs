//! Blended surface relief and material caps.

use super::Terrain;
use super::hydrology::HydrologyColumn;
use super::noise::{hash_2d, smoothstep, value_noise};
use crate::world::{Biome, Block, Column, MAX_TERRAIN_HEIGHT, MIN_BASE_TERRAIN_HEIGHT, SEA_LEVEL};

impl Terrain {
    pub(in crate::world::generation) fn base_column(&self, x: i32, z: i32) -> Column {
        let x = f64::from(x);
        let z = f64::from(z);
        let (offset_x, offset_z) = self.biome_offset();
        let warped_x = x + offset_x + value_noise(self.seed ^ 0xA13F, x / 112.0, z / 112.0) * 20.0;
        let warped_z = z + offset_z + value_noise(self.seed ^ 0xB27D, x / 112.0, z / 112.0) * 20.0;

        // Broad, warped climate bands keep every biome near the central junction
        // while still providing an unbounded world instead of four fixed quadrants.
        let east = smoothstep((warped_x / 136.0).sin() / 0.6 + 0.5);
        let north = smoothstep((warped_z / 136.0).sin() / 0.6 + 0.5);
        let weights = [
            (1.0 - east) * north,
            east * north,
            east * (1.0 - north),
            (1.0 - east) * (1.0 - north),
        ];
        let biome = match (east >= 0.5, north >= 0.5) {
            (false, true) => Biome::Plains,
            (true, true) => Biome::Forest,
            (true, false) => Biome::Desert,
            (false, false) => Biome::Alpine,
        };

        let broad = value_noise(self.seed ^ 0xC39B, x / 112.0, z / 112.0) * 0.6
            + value_noise(self.seed ^ 0xD4B9, x / 56.0, z / 56.0) * 0.28
            + value_noise(self.seed ^ 0xE5D7, x / 28.0, z / 28.0) * 0.12;
        let detail = value_noise(self.seed ^ 0xF6F5, x / 20.0, z / 20.0);
        let dune_warp = value_noise(self.seed ^ 0x17013, x / 72.0, z / 72.0) * 9.0;
        let dunes = ((x + dune_warp) / 17.0 + z / 61.0).sin();
        let desert_shelves = value_noise(self.seed ^ 0x17014, x / 47.0, z / 89.0);
        let ridge_field = value_noise(self.seed ^ 0x18131, x / 112.0, z / 112.0);
        let ridge = 1.0 - ridge_field.abs();
        let mountain_saddles = value_noise(self.seed ^ 0x18132, x / 96.0, z / 96.0);
        let heights = [
            21.0 + broad * 5.0 + detail,
            24.0 + broad * 7.0 + detail * 1.5,
            23.0 + broad * 4.0 + dunes * 4.0 + desert_shelves * 3.0,
            38.0 + broad * 10.0 + ridge.powi(2) * 24.0 + mountain_saddles * 6.0 + detail * 1.5,
        ];
        let height = weights
            .into_iter()
            .zip(heights)
            .map(|(weight, height)| weight * height)
            .sum::<f64>();
        let channel = (z / 137.0
            + value_noise(self.seed ^ 0x18133, x / 127.0, z / 173.0) * 1.1
            + (x / 211.0).sin() * 0.7)
            .sin()
            .abs();
        let channel = (1.0 - channel / 0.22).clamp(0.0, 1.0);
        let valley = (-broad - 0.15).max(0.0) * 12.0 + channel * channel * (3.0 + weights[3] * 5.0);
        let height = (height - valley).round() as i32;

        Column {
            height: height.clamp(MIN_BASE_TERRAIN_HEIGHT, MAX_TERRAIN_HEIGHT),
            biome,
        }
    }

    pub(super) fn biome_offset(&self) -> (f64, f64) {
        let offset = hash_2d(self.seed ^ 0x1924F, 0, 0);
        (
            (offset & 0xFFFF) as f64 / 65_535.0 * 48.0 - 24.0,
            ((offset >> 16) & 0xFFFF) as f64 / 65_535.0 * 48.0 - 24.0,
        )
    }
}

pub(super) fn terrain_block(column: Column, y: i32) -> Block {
    if y > column.height {
        return Block::Air;
    }
    let depth = column.height - y;
    if depth > 3 {
        return Block::Stone;
    }
    let surface = if column.height < SEA_LEVEL + 1 || column.biome == Biome::Desert {
        Block::Sand
    } else {
        match column.biome {
            Biome::Plains | Biome::Forest => Block::Grass,
            Biome::Alpine if column.height >= 40 => Block::Snow,
            Biome::Alpine => Block::Stone,
            Biome::Desert => Block::Sand,
        }
    };
    if depth == 0 {
        surface
    } else {
        match surface {
            Block::Grass => Block::Dirt,
            Block::Sand => Block::Sand,
            _ => Block::Stone,
        }
    }
}

pub(super) fn hydrology_block(profile: HydrologyColumn, y: i32) -> Block {
    if let Some(block) = profile.coastal.and_then(|coastal| coastal.block(y)) {
        return block;
    }
    if y > profile.column.height {
        return if profile.water.is_some_and(|water| water.contains_y(y)) {
            Block::Water
        } else {
            Block::Air
        };
    }
    if profile.column.height - y <= 3
        && let Some(sediment) = profile.sediment
    {
        return sediment;
    }
    terrain_block(profile.column, y)
}
