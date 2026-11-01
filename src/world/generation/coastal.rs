//! Seeded coastal occupancy projected once per column, sharing the marine catchment.

use crate::world::{Block, CoastalLandmark, CoastalStack, SEA_LEVEL, Terrain};

use super::hydrology::HydrologyColumn;
use super::noise::{hash_2d, value_noise};

mod arch;
mod sampling;
mod sites;
#[cfg(test)]
mod tests;

const MIN_U: f64 = 232.0;
const MAX_U: f64 = 512.0;
const END_BLEND: f64 = 12.0;

/// An explicit extra rock span permits an arch without raising its seabed cap.
#[derive(Clone, Copy, Debug)]
pub(in crate::world::generation) struct CoastalColumn {
    pub(in crate::world::generation) ground: i32,
    pub(in crate::world::generation) rock_from: i32,
    pub(in crate::world::generation) sediment: bool,
    pub(in crate::world::generation) layer: i32,
    pub(in crate::world::generation) roof: Option<[i32; 2]>,
}

impl CoastalColumn {
    pub(in crate::world::generation) fn block(self, y: i32) -> Option<Block> {
        let roof = self
            .roof
            .is_some_and(|[first, last]| (first..=last).contains(&y));
        if !roof && (y < self.rock_from || y > self.ground) {
            return None;
        }
        Some(if !roof && self.sediment && self.ground - y <= 2 {
            Block::Sand
        } else if (y + self.layer).rem_euclid(9) == 0 {
            Block::Calcite
        } else {
            Block::Limestone
        })
    }
}

#[derive(Clone, Copy)]
struct Stack {
    center: [f64; 2],
    radius: [f64; 2],
    height: i32,
    lean: f64,
    angle: f64,
}

/// Fixed-size roots; neither voxels nor generation order are retained here.
pub(in crate::world::generation) struct Coast {
    origin: [f64; 2],
    axis: [f64; 2],
    marine_radius: [f64; 2],
    center_v: f64,
    half_length: f64,
    cliff_height: f64,
    cove_v: f64,
    layer: i32,
    stacks: Vec<Stack>,
    arch: arch::Arch,
}

impl Coast {
    pub(in crate::world::generation) fn new(
        seed: u64,
        hash: u64,
        origin: [f64; 2],
        axis: [f64; 2],
        marine_radius: [f64; 2],
    ) -> Self {
        let center_v = 146.0 + ((hash >> 12) % 9) as f64 - 4.0;
        let half_length = 76.0 + ((hash >> 28) % 9) as f64;
        let mut coast = Self {
            origin,
            axis,
            marine_radius,
            center_v,
            half_length,
            cliff_height: f64::from(SEA_LEVEL) + 23.0 + ((hash >> 36) % 6) as f64,
            cove_v: center_v - 32.0 + ((hash >> 44) % 7) as f64,
            layer: ((hash >> 48) % 9) as i32,
            stacks: Vec::with_capacity(6),
            arch: arch::Arch::new(hash, center_v - 32.0 + ((hash >> 44) % 7) as f64),
        };
        let count = 5 + (hash % 2) as usize;
        for index in 0..count {
            let variation = hash_2d(seed ^ 0x434F_4153_545F_5354, hash as i64, index as i64);
            let v = center_v - 63.0
                + index as f64 * 126.0 / (count - 1) as f64
                + ((variation >> 8) % 7) as f64
                - 3.0;
            coast.stacks.push(Stack {
                center: [coast.edge(v) + 36.0 + ((variation >> 16) % 12) as f64, v],
                radius: [
                    5.0 + ((variation >> 24) % 4) as f64,
                    4.0 + ((variation >> 32) % 4) as f64,
                ],
                height: 18 + ((variation >> 40) % 17) as i32,
                lean: ((variation >> 48) % 5) as f64 - 2.0,
                angle: ((variation >> 52) % 64) as f64 / 64.0 * std::f64::consts::PI,
            });
        }
        coast
    }

    fn local(&self, world: [f64; 2]) -> [f64; 2] {
        let dx = world[0] - self.origin[0];
        let dz = world[1] - self.origin[1];
        [
            dx * self.axis[0] + dz * self.axis[1],
            -dx * self.axis[1] + dz * self.axis[0],
        ]
    }

    fn world(&self, point: [f64; 2]) -> [f64; 2] {
        [
            self.origin[0] + point[0] * self.axis[0] - point[1] * self.axis[1],
            self.origin[1] + point[0] * self.axis[1] + point[1] * self.axis[0],
        ]
    }

    fn edge(&self, v: f64) -> f64 {
        let across = ((v - 64.0) / self.marine_radius[1]).clamp(-0.995, 0.995);
        483.0 - self.marine_radius[0] * (1.0 - across * across).sqrt()
            + ((v - self.center_v) / 19.0).sin() * 3.0
    }

    pub(in crate::world::generation) fn roof_at(&self, world: [f64; 2]) -> Option<[i32; 2]> {
        self.arch
            .project(self, self.local(world))
            .and_then(|(_, roof)| roof)
    }
}

fn smooth(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}
