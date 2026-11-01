//! A finite rock ridge with grounded piers and an explicit above-water roof span.

use super::{Coast, SEA_LEVEL, Terrain};
use crate::world::CoastalArch;

pub(super) struct Arch {
    v: f64,
    half_width: f64,
    half_depth: f64,
    opening_half: f64,
    top: i32,
}

impl Arch {
    pub(super) fn new(hash: u64, cove_v: f64) -> Self {
        Self {
            v: cove_v + 40.0 + ((hash >> 4) % 5) as f64,
            half_width: 13.0 + ((hash >> 20) % 3) as f64,
            half_depth: 4.0 + ((hash >> 32) % 2) as f64,
            opening_half: 5.0 + ((hash >> 40) % 2) as f64,
            top: SEA_LEVEL + 18 + ((hash >> 56) % 5) as i32,
        }
    }

    fn center(&self, coast: &Coast) -> [f64; 2] {
        [coast.edge(self.v) + 13.0, self.v]
    }

    pub(super) fn project(
        &self,
        coast: &Coast,
        point: [f64; 2],
    ) -> Option<(i32, Option<[i32; 2]>)> {
        let center = self.center(coast);
        let du = (point[0] - center[0]).abs();
        let dv = (point[1] - center[1]).abs();
        if dv > self.half_width || du > self.half_depth - (dv / self.half_width).powi(2) {
            return None;
        }
        let top = self.top - (dv / self.half_width * 4.0 + du * 0.15).floor() as i32;
        let roof = (dv < self.opening_half).then(|| {
            let first =
                SEA_LEVEL + 6 + ((1.0 - (dv / self.opening_half).powi(2)) * 4.0).floor() as i32;
            [first, top]
        });
        Some((top, roof))
    }

    pub(super) fn landmark(&self, coast: &Coast, terrain: &Terrain) -> Option<CoastalArch> {
        let center = self.center(coast);
        let [x, z] = coast.world(center).map(|value| value.floor() as i32);
        let local = coast.local([f64::from(x) + 0.5, f64::from(z) + 0.5]);
        let roof = self.project(coast, local)?.1?;
        let supports = [-1.0, 1.0].map(|side| {
            let [x, z] = coast
                .world([center[0], center[1] + side * (self.opening_half + 3.0)])
                .map(|value| value.floor() as i32);
            [x, SEA_LEVEL + 2, z]
        });
        let opening = [x as f32 + 0.5, (SEA_LEVEL + 3) as f32, z as f32 + 0.5];
        if terrain.block_at(x, SEA_LEVEL + 3, z) != crate::world::Block::Air
            || !terrain.block_at(x, roof[0], z).is_solid()
            || !supports
                .iter()
                .all(|&[x, y, z]| terrain.block_at(x, y, z).is_solid())
        {
            return None;
        }
        Some(CoastalArch {
            opening,
            roof,
            supports,
            view: {
                let point = coast.world([coast.edge(self.v) + 3.5, center[1]]);
                let [x, z] = point.map(|value| value.floor() as i32);
                [
                    x as f32 + 0.5,
                    (terrain.column(x, z).height + 1) as f32,
                    z as f32 + 0.5,
                ]
            },
        })
    }
}

#[cfg(test)]
mod tests;
