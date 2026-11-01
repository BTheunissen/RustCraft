//! Deterministic steady-state drainage, coupled to its lake beds and coastlines.
//!
//! Catchments have a bounded world-space footprint. Their two source basins,
//! confluence, descending channel, rock lip, and marine outlet are generated
//! together; chunk loading never advances a fluid simulation or changes a river.

use crate::world::{
    Block, Column, MIN_TERRAIN_HEIGHT, SEA_LEVEL, Terrain, WORLD_HEIGHT, WaterBiome, WaterColumn,
    WaterLandmark,
};

const CATCHMENT_CELL: i64 = 1_536;
const CATCHMENT_REACH: f64 = 760.0;
const COLUMN_HALO: f64 = 4.0;
const BANK_APRON: f64 = 64.0;

use super::coastal::{Coast, CoastalColumn};
use super::noise::{hash_2d, value_noise};
mod catchment;
mod features;
mod landmarks;
mod sampling;
use catchment::Catchment;

#[cfg(test)]
pub(in crate::world::generation) fn diagnostic_coast(seed: u64) -> Coast {
    Catchment::new(seed, 0, 0).coast
}
use features::{Basin, RiverPoint, RiverProjection, RiverSegment};
pub(super) use landmarks::water_landmark;

#[derive(Clone, Copy)]
pub(in crate::world::generation) struct HydrologyColumn {
    pub(in crate::world::generation) column: Column,
    pub(in crate::world::generation) water: Option<WaterColumn>,
    pub(in crate::world::generation) sediment: Option<Block>,
    /// Raised fill keeps its original foundation; lowered beds keep four layers.
    pub(in crate::world::generation) seal_from: Option<i32>,
    pub(in crate::world::generation) coastal: Option<CoastalColumn>,
    rock_lip: bool,
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    max_x: f64,
    min_z: f64,
    max_z: f64,
}

impl Bounds {
    fn intersects(self, other: Self) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_z <= other.max_z
            && self.max_z >= other.min_z
    }

    fn around(center: [f64; 2], reach: f64) -> Self {
        Self {
            min_x: center[0] - reach,
            max_x: center[0] + reach,
            min_z: center[1] - reach,
            max_z: center[1] + reach,
        }
    }
}

/// Bounded horizontal feature setup shared by dense chunks and boundary rays.
pub(in crate::world::generation) struct HydrologyRegion {
    seed: u64,
    catchments: Vec<Catchment>,
}

impl HydrologyRegion {
    pub(in crate::world::generation) fn columns(
        seed: u64,
        min_x: i32,
        max_x: i32,
        min_z: i32,
        max_z: i32,
    ) -> Self {
        let query = Bounds {
            min_x: f64::from(min_x) - COLUMN_HALO,
            max_x: f64::from(max_x) + COLUMN_HALO + 1.0,
            min_z: f64::from(min_z) - COLUMN_HALO,
            max_z: f64::from(max_z) + COLUMN_HALO + 1.0,
        };
        let mut catchments = Vec::with_capacity(2);
        let cells = |min: f64, max: f64| {
            ((min - CATCHMENT_REACH).floor() as i64).div_euclid(CATCHMENT_CELL)
                ..=((max + CATCHMENT_REACH).ceil() as i64).div_euclid(CATCHMENT_CELL)
        };
        for cell_z in cells(query.min_z, query.max_z) {
            for cell_x in cells(query.min_x, query.max_x) {
                let origin = [
                    (cell_x * CATCHMENT_CELL) as f64,
                    (cell_z * CATCHMENT_CELL) as f64,
                ];
                if Bounds::around(origin, CATCHMENT_REACH).intersects(query) {
                    catchments.push(Catchment::new(seed, cell_x, cell_z));
                }
            }
        }
        Self { seed, catchments }
    }

    pub(in crate::world::generation) fn column(
        &self,
        terrain: &Terrain,
        x: i32,
        z: i32,
    ) -> HydrologyColumn {
        let profile = self.filtered_column(terrain, x, z);
        let world = [f64::from(x) + 0.5, f64::from(z) + 0.5];
        self.catchments
            .iter()
            .find_map(|catchment| {
                catchment.coast.column(self.seed, world, profile, |point| {
                    catchment.coastal_clearance(point)
                })
            })
            .unwrap_or(profile)
    }

    pub(in crate::world::generation) fn coastal_roof(
        &self,
        terrain: &Terrain,
        x: i32,
        z: i32,
    ) -> Option<[i32; 2]> {
        let world = [f64::from(x) + 0.5, f64::from(z) + 0.5];
        if !self
            .catchments
            .iter()
            .any(|catchment| catchment.coast.roof_at(world).is_some())
        {
            return None;
        }
        self.column(terrain, x, z)
            .coastal
            .and_then(|coast| coast.roof)
    }

    fn filtered_column(&self, terrain: &Terrain, x: i32, z: i32) -> HydrologyColumn {
        let mut profile = self.raw_column(terrain, x, z);
        if profile.seal_from.is_none() || profile.rock_lip {
            return profile;
        }
        let mut total = 0;
        let mut count = 0;
        let mut shoreline = MIN_TERRAIN_HEIGHT;
        let mut shore_distance = false;
        let mut shore_cap = i32::MAX;
        let mut shore_floor = MIN_TERRAIN_HEIGHT;
        let mut neighbor_ceiling = WORLD_HEIGHT - 10;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (Some(neighbor_x), Some(neighbor_z)) = (x.checked_add(dx), z.checked_add(dz))
                else {
                    continue;
                };
                let neighbor = if dx == 0 && dz == 0 {
                    profile
                } else {
                    self.raw_column(terrain, neighbor_x, neighbor_z)
                };
                total += neighbor.column.height;
                count += 1;
                if dx.abs() + dz.abs() == 1
                    && let Some(water) = neighbor.water
                {
                    shoreline = shoreline.max(water.surface.ceil() as i32 - 2);
                }
                shore_distance |= neighbor.water.is_none();
                if dx.abs() + dz.abs() == 1 && neighbor.water.is_none() {
                    shore_floor = shore_floor.max(neighbor.column.height - 2);
                    neighbor_ceiling = neighbor_ceiling.min(neighbor.column.height + 2);
                }
                if profile.water.is_none()
                    && let Some(water) = neighbor.water
                {
                    shore_cap = shore_cap.min(water.surface.ceil() as i32);
                }
            }
        }
        // A tiny bounded bed filter removes digital seams at overlapping
        // tributaries without smoothing away the explicitly generated rock lip.
        let mut height = (f64::from(total) / f64::from(count)).round() as i32;
        if let Some(mut water) = profile.water {
            height = if shore_distance {
                height
                    .max(profile.column.height)
                    .max(shore_floor)
                    .min(water.surface.ceil() as i32 - 2)
            } else {
                height.min(water.surface.ceil() as i32 - 2)
            };
            water.bottom = height;
            profile.water = Some(water);
        } else {
            height = height
                .min(profile.column.height)
                .min(shore_cap)
                .min(neighbor_ceiling)
                .max(shoreline);
        }
        profile.column.height = height;
        profile.seal_from = Some(terrain.base_column(x, z).height.min(height) - 3);
        profile
    }

    fn raw_column(&self, terrain: &Terrain, x: i32, z: i32) -> HydrologyColumn {
        let base = terrain.base_column(x, z);
        let world = [f64::from(x) + 0.5, f64::from(z) + 0.5];
        self.catchments
            .iter()
            .find_map(|catchment| catchment.sample(self.seed, world, base))
            .unwrap_or(HydrologyColumn {
                column: base,
                water: None,
                sediment: None,
                seal_from: None,
                coastal: None,
                rock_lip: false,
            })
    }
}

impl Terrain {
    /// Reads the early coast's real supported geometry; never moves the player.
    pub fn coastal_landmark(&self) -> Option<crate::world::CoastalLandmark> {
        Catchment::new(self.seed(), 0, 0).coast.landmark(self)
    }
}

fn mix(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn unit(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1_u64 << 53) as f64
}

fn normalized(vector: [f64; 2]) -> [f64; 2] {
    let length = vector[0].hypot(vector[1]);
    [vector[0] / length, vector[1] / length]
}

fn route_direction(points: &[RiverPoint], index: usize) -> [f64; 2] {
    let before = points[index.saturating_sub(1)].position;
    let after = points[(index + 1).min(points.len() - 1)].position;
    normalized([after[0] - before[0], after[1] - before[1]])
}

#[cfg(test)]
mod bias_tests;
#[cfg(test)]
mod tests;
