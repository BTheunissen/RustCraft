//! Per-column projection and the material/carving precedence contract.

use super::{
    Block, CaveBiome, Climate, Column, FIRST_CAVE_Y, Geology, MIN_WORLD_Y, Mask, Terrain,
    UndergroundRegion, WORLD_DEPTH, WORLD_HEIGHT, bedrock_top, value_noise,
};

impl UndergroundRegion {
    pub(in crate::world::generation) fn column(
        &self,
        x: i32,
        z: i32,
        surface: Column,
    ) -> UndergroundColumn {
        self.column_with_surface_seal(
            x,
            z,
            surface,
            Terrain::new(self.seed).hydrology_column(x, z).seal_from,
        )
    }

    pub(in crate::world::generation) fn column_with_surface_seal(
        &self,
        x: i32,
        z: i32,
        surface: Column,
        seal_from: Option<i32>,
    ) -> UndergroundColumn {
        let projection = self.projection(i64::from(x), i64::from(z));
        let void = projection.void;
        let mut neighbors = Mask::default();
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            neighbors.union(self.void_mask(i64::from(x) + dx, i64::from(z) + dz));
        }
        let mut column = UndergroundColumn {
            surface,
            seal_from,
            bedrock_top: bedrock_top(self.seed, x, z),
            climate: Climate::new(self.seed, x, z),
            geology: Geology::new(self.seed, x, z),
            moss: value_noise(
                self.seed ^ 0x1055_CAFE,
                f64::from(x) / 11.0,
                f64::from(z) / 11.0,
            ),
            void,
            neighbors,
            ores: [Block::Air; WORLD_DEPTH as usize],
            liquids: [Block::Air; WORLD_DEPTH as usize],
            decorations: [Block::Air; WORLD_DEPTH as usize],
        };
        for vein in &self.veins {
            if let Some((first, last)) = vein.feature.span(f64::from(x) + 0.5, f64::from(z) + 0.5) {
                for y in
                    first.max(FIRST_CAVE_Y)..=last.min(surface.height - 4).min(WORLD_HEIGHT - 1)
                {
                    // Ordering is world-cell order, independent of which region
                    // requested the features; overlaps therefore sample identically.
                    column.ores[(y - MIN_WORLD_Y) as usize] = vein.block;
                }
            }
        }
        for pool in &self.pools {
            if pool.column(f64::from(x) + 0.5, f64::from(z) + 0.5) == Some(true) {
                for y in pool.level - 2..pool.level {
                    if (FIRST_CAVE_Y..WORLD_HEIGHT).contains(&y) {
                        column.liquids[(y - MIN_WORLD_Y) as usize] = pool.liquid;
                    }
                }
            }
        }
        for growth in &self.growths {
            if !growth
                .bounds()
                .contains_column(f64::from(x) + 0.5, f64::from(z) + 0.5)
            {
                continue;
            }
            let first = growth.bounds().min_y.floor() as i32;
            let last = growth.bounds().max_y.ceil() as i32;
            for y in first.max(FIRST_CAVE_Y)..=last.min(surface.height - 4) {
                let index = (y - MIN_WORLD_Y) as usize;
                if void.contains(y)
                    && !projection.protected.contains(y)
                    && column.liquids[index] == Block::Air
                    && growth.contains(f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5)
                {
                    column.decorations[index] = growth.block;
                }
            }
        }
        column
    }
}

pub(in crate::world::generation) struct UndergroundColumn {
    pub(super) surface: Column,
    pub(super) seal_from: Option<i32>,
    pub(super) bedrock_top: i32,
    pub(super) climate: Climate,
    pub(super) geology: Geology,
    pub(super) moss: f64,
    pub(super) void: Mask,
    pub(super) neighbors: Mask,
    pub(super) ores: [Block; WORLD_DEPTH as usize],
    pub(super) liquids: [Block; WORLD_DEPTH as usize],
    pub(super) decorations: [Block; WORLD_DEPTH as usize],
}

impl UndergroundColumn {
    pub(in crate::world::generation) fn block(&self, y: i32, base: Block) -> Block {
        if y <= self.bedrock_top {
            return Block::Bedrock;
        }
        let index = (y - MIN_WORLD_Y) as usize;
        // Local raised lakes need the same four-block seal as the ocean floor.
        // Surface hydrology never fills the underlying cave network.
        let capped = self.seal_from.is_some_and(|first| y >= first);
        if capped {
            return base;
        }
        if self.void.contains(y) {
            if self.decorations[index] != Block::Air {
                return self.decorations[index];
            }
            return match self.liquids[index] {
                Block::Air => Block::Air,
                liquid => liquid,
            };
        }
        if base != Block::Stone {
            return base;
        }
        // Preserve the original four-layer soil/snow cap and its immediate
        // supporting stone; folded regional geology begins further below it.
        let rock = if self.surface.height - y <= 4 {
            Block::Stone
        } else {
            self.geology.host(y)
        };
        if self.ores[index] != Block::Air {
            return self.ores[index];
        }
        let floor = self.void.contains(y + 1);
        let exposed = floor || self.void.contains(y - 1) || self.neighbors.contains(y);
        if !exposed {
            return rock;
        }
        match self.climate.biome(y) {
            CaveBiome::Lush
                if self.moss > -0.08
                    && (floor
                        || (self.moss > 0.26
                            && (f64::from(y) * 0.3 + self.moss * 4.0).sin() > -0.15)) =>
            {
                Block::Moss
            }
            _ => rock,
        }
    }
}

pub(in crate::world::generation) fn sample(
    terrain: &Terrain,
    x: i32,
    y: i32,
    z: i32,
    column: Column,
    base: Block,
    seal_from: Option<i32>,
) -> Block {
    if y <= bedrock_top(terrain.seed(), x, z) {
        return Block::Bedrock;
    }
    if seal_from.is_some_and(|first| y >= first) {
        return base;
    }
    UndergroundRegion::sample(terrain, x, z)
        .column_with_surface_seal(x, z, column, seal_from)
        .block(y, base)
}
