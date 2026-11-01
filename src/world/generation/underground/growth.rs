//! Rooted mineral formations clipped to preserve walkable routes.

use super::{
    Block, Bounds, CaveBiome, Climate, FIRST_CAVE_Y, GROWTH_ATTEMPTS, GROWTH_CELL, GROWTH_REACH,
    Geology, Terrain, UndergroundRegion, bedrock_top, cells, hash_2d, unit,
};

#[derive(Clone, Copy)]
pub(super) struct Growth {
    pub(super) root: [f64; 3],
    pub(super) direction: [f64; 3],
    pub(super) length: f64,
    pub(super) radius: f64,
    pub(super) block: Block,
    pub(super) column: bool,
}

impl Growth {
    pub(super) fn bounds(self) -> Bounds {
        let end: [f64; 3] =
            std::array::from_fn(|axis| self.root[axis] + self.direction[axis] * self.length);
        Bounds {
            min_x: self.root[0].min(end[0]) - self.radius,
            max_x: self.root[0].max(end[0]) + self.radius,
            min_z: self.root[2].min(end[2]) - self.radius,
            max_z: self.root[2].max(end[2]) + self.radius,
            min_y: self.root[1].min(end[1]) - 0.6,
            max_y: self.root[1].max(end[1]) + 0.6,
        }
    }

    pub(super) fn contains(self, x: f64, y: f64, z: f64) -> bool {
        let t = (y - self.root[1]) / (self.direction[1] * self.length);
        if !(0.0..=1.0).contains(&t) {
            return false;
        }
        let radius = self.radius_at(t);
        let dx = x - self.root[0] - self.direction[0] * self.length * t;
        let dz = z - self.root[2] - self.direction[2] * self.length * t;
        if dx * dx + dz * dz <= radius * radius {
            return true;
        }
        // A sub-voxel conical tip on a slanted spear can otherwise disappear
        // for one Y slice and reappear as a floating voxel. Connect each slice
        // to its predecessor with a deterministic six-neighbor digital spine.
        let previous_t = (t - 1.0 / self.length).max(0.0);
        let center = |axis: usize, t: f64| {
            (self.root[axis] + self.direction[axis] * self.length * t).floor() as i64
        };
        let (current_x, current_z) = (center(0, t), center(2, t));
        let (previous_x, previous_z) = (center(0, previous_t), center(2, previous_t));
        let voxel_x = x.floor() as i64;
        let voxel_z = z.floor() as i64;
        (voxel_z == current_z
            && (previous_x.min(current_x)..=previous_x.max(current_x)).contains(&voxel_x))
            || (voxel_x == previous_x
                && (previous_z.min(current_z)..=previous_z.max(current_z)).contains(&voxel_z))
    }

    pub(super) fn radius_at(self, t: f64) -> f64 {
        self.radius
            * if self.column {
                0.48 + (t * 2.0 - 1.0).powi(2) * 0.52
            } else {
                1.0 - t * 0.92
            }
    }
}

impl UndergroundRegion {
    pub(super) fn populate_growths(&mut self, terrain: &Terrain, query: Bounds) {
        for cell_z in cells(
            query.min_z - GROWTH_REACH,
            query.max_z + GROWTH_REACH,
            GROWTH_CELL,
        ) {
            for cell_x in cells(
                query.min_x - GROWTH_REACH,
                query.max_x + GROWTH_REACH,
                GROWTH_CELL,
            ) {
                for attempt in 0..GROWTH_ATTEMPTS {
                    let hash = hash_2d(
                        self.seed ^ 0x57A1_AC71 ^ attempt.wrapping_mul(0xD1B5_4A32_D192_ED03),
                        cell_x,
                        cell_z,
                    );
                    let (Ok(x), Ok(z)) = (
                        i32::try_from(cell_x * GROWTH_CELL + 1 + ((hash >> 8) % 8) as i64),
                        i32::try_from(cell_z * GROWTH_CELL + 1 + ((hash >> 24) % 8) as i64),
                    ) else {
                        continue;
                    };
                    let root = self.projection(i64::from(x), i64::from(z));
                    let surface = terrain.column(x, z);
                    let geology = Geology::new(self.seed, x, z);
                    let climate = Climate::new(self.seed, x, z);
                    let mut first = FIRST_CAVE_Y;
                    while first < surface.height - 4 {
                        if !root.void.contains(first) {
                            first += 1;
                            continue;
                        }
                        let mut last = first;
                        while last + 1 < surface.height - 4 && root.void.contains(last + 1) {
                            last += 1;
                        }
                        let clearance = last - first + 1;
                        if clearance >= 6 && first < -3 {
                            let crystal = climate.biome(first) == CaveBiome::Crystal;
                            let carbonate_floor =
                                first >= -34 && geology.host(first - 1) == Block::Limestone;
                            let carbonate_ceiling =
                                last >= -34 && geology.host(last + 1) == Block::Limestone;
                            let calcite = carbonate_floor || carbonate_ceiling;
                            let pool_nearby = self.pools.iter().any(|pool| {
                                pool.bounds()
                                    .expanded(GROWTH_REACH)
                                    .contains_column(f64::from(x) + 0.5, f64::from(z) + 0.5)
                            });
                            if !pool_nearby && (crystal || calcite) {
                                let block = if calcite && (!crystal || hash.is_multiple_of(3)) {
                                    Block::Calcite
                                } else {
                                    Block::Crystal
                                };
                                let column = block == Block::Calcite
                                    && carbonate_floor
                                    && clearance >= 9
                                    && hash.is_multiple_of(11);
                                let upward = column || hash & 1 == 0;
                                let root_y = if upward { first - 1 } else { last + 1 };
                                let route_near_root =
                                    (-2..=2).any(|dy| root.protected.contains(root_y + dy));
                                if !route_near_root
                                    && root_y > bedrock_top(self.seed, x, z)
                                    && root_y <= surface.height - 4
                                    && !root.void.contains(root_y)
                                    && (!column || !root.void.contains(last + 1))
                                {
                                    let growth = Growth {
                                        root: [
                                            f64::from(x) + 0.5,
                                            f64::from(root_y) + 0.5,
                                            f64::from(z) + 0.5,
                                        ],
                                        direction: [
                                            if block == Block::Crystal {
                                                (unit(hash.rotate_left(19)) - 0.5) * 0.6
                                            } else {
                                                0.0
                                            },
                                            if upward { 1.0 } else { -1.0 },
                                            if block == Block::Crystal {
                                                (unit(hash.rotate_left(41)) - 0.5) * 0.6
                                            } else {
                                                0.0
                                            },
                                        ],
                                        length: if column {
                                            f64::from(clearance + 1)
                                        } else {
                                            (3.5 + ((hash >> 40) % 5) as f64)
                                                .min(f64::from(clearance - 3))
                                        },
                                        radius: if column {
                                            2.5
                                        } else {
                                            1.7 + ((hash >> 48) % 3) as f64 * 0.35
                                        },
                                        block,
                                        column,
                                    };
                                    if !growth.bounds().expanded(2.0).intersects(query) {
                                        first = last + 1;
                                        continue;
                                    }
                                    let Some(growth) = self.route_safe_growth(growth) else {
                                        first = last + 1;
                                        continue;
                                    };
                                    let reach = growth.bounds().expanded(1.0);
                                    if reach.intersects(query) {
                                        self.growths.push(growth);
                                    }
                                    if growth.block == Block::Crystal && hash.is_multiple_of(3) {
                                        let companion = Growth {
                                            length: growth.length * 0.65,
                                            radius: growth.radius * 0.68,
                                            direction: [
                                                growth.direction[0] - 0.16,
                                                growth.direction[1],
                                                growth.direction[2] + 0.12,
                                            ],
                                            ..growth
                                        };
                                        if let Some(companion) = self.route_safe_growth(companion)
                                            && companion.bounds().expanded(1.0).intersects(query)
                                        {
                                            self.growths.push(companion);
                                        }
                                    }
                                }
                            }
                        }
                        first = last + 1;
                    }
                }
            }
        }
    }

    pub(super) fn route_safe_growth(&self, mut growth: Growth) -> Option<Growth> {
        let bounds = growth.bounds();
        let mut maximum_length = growth.length;
        // Test only nearby protected spans. Rebuilding a full mask for every
        // footprint voxel would multiply population cost for every halo ray.
        for route in &self.routes {
            if !route.bounds.intersects(bounds) {
                continue;
            }
            for z in bounds.min_z.max(route.bounds.min_z).floor() as i64
                ..=bounds.max_z.min(route.bounds.max_z).ceil() as i64
            {
                for x in bounds.min_x.max(route.bounds.min_x).floor() as i64
                    ..=bounds.max_x.min(route.bounds.max_x).ceil() as i64
                {
                    let Some((first, _, true)) = route.passage_span(x as f64 + 0.5, z as f64 + 0.5)
                    else {
                        continue;
                    };
                    for y in first..=first + 4 {
                        if growth.contains(x as f64 + 0.5, f64::from(y) + 0.5, z as f64 + 0.5) {
                            if growth.column {
                                return None;
                            }
                            maximum_length = maximum_length
                                .min((f64::from(y) + 0.5 - growth.root[1]).abs() - 1.0);
                        }
                    }
                }
            }
        }
        if maximum_length < 2.0 {
            return None;
        }
        growth.length = maximum_length;
        Some(growth)
    }
}
