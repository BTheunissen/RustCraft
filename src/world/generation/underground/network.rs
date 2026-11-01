//! Seeded network nodes, winding connections, and surface entrances.

use super::super::hydrology::HydrologyRegion;
use super::{
    Bounds, CELL_SIZE, CaveBiome, CaveLandform, Climate, ENTRANCE_RUN, Room, SEA_LEVEL, Terrain,
    UndergroundRegion, WORLD_HEIGHT, hash_2d, unit,
};

#[derive(Clone, Copy)]
pub(super) struct Node {
    pub(super) center: [f64; 3],
    pub(super) radius: [f64; 3],
    pub(super) floor: f64,
    pub(super) axis: [f64; 2],
    pub(super) landform: CaveLandform,
    pub(super) climate: CaveBiome,
    pub(super) hash: u64,
}

impl Node {
    pub(super) fn anchor(self) -> [f64; 3] {
        [self.center[0], self.floor, self.center[2]]
    }

    pub(super) fn point(self, along: f64, across: f64, y: f64) -> [f64; 3] {
        [
            self.center[0] + self.axis[0] * along - self.axis[1] * across,
            y,
            self.center[2] + self.axis[1] * along + self.axis[0] * across,
        ]
    }

    pub(super) fn room(self) -> Room {
        Room {
            origin: self.anchor(),
            axis: self.axis,
            length: self.radius[0],
            width: self.radius[2],
            height: self.radius[1] * 2.0,
            phase: unit(self.hash.rotate_left(13)) * std::f64::consts::TAU,
            landform: self.landform,
            relief: match self.climate {
                CaveBiome::Lush => 0.7,
                CaveBiome::Rocky => 1.1,
                CaveBiome::Crystal => 1.6,
            },
        }
    }
}

pub(super) fn node(seed: u64, cell_x: i64, cell_z: i64, deep: bool) -> Node {
    let hash = hash_2d(
        seed ^ if deep { 0xD33F_CAFE } else { 0xCA7E_50A1 },
        cell_x,
        cell_z,
    );
    let x = (cell_x * CELL_SIZE + 20 + ((hash >> 8) % 40) as i64) as f64 + 0.5;
    let z = (cell_z * CELL_SIZE + 20 + ((hash >> 40) % 40) as i64) as f64 + 0.5;
    let floor = f64::from(if deep { -56 } else { -24 }) + ((hash >> 24) % 5) as f64;
    let landform = match hash % 10 {
        0..=2 => CaveLandform::Gallery,
        3..=4 => CaveLandform::Rift,
        5..=7 => CaveLandform::Cathedral,
        _ => CaveLandform::Labyrinth,
    };
    let climate = Climate::new(seed, x.floor() as i32, z.floor() as i32).biome(floor as i32);
    let (length, width, mut height) = match landform {
        CaveLandform::Gallery => (28.0, 15.0, 9.0),
        CaveLandform::Rift => (29.0, 6.0, 31.0),
        CaveLandform::Cathedral => (28.0, 22.0, 25.0),
        CaveLandform::Labyrinth => (27.0, 19.0, 9.0),
    };
    height += ((hash >> 32) % 5) as f64;
    height *= match climate {
        CaveBiome::Lush => 0.94,
        CaveBiome::Rocky => 1.0,
        CaveBiome::Crystal => 1.08,
    };
    let angle = unit(hash.rotate_left(29)) * std::f64::consts::TAU;
    Node {
        center: [x, floor + height * 0.5, z],
        radius: [
            length + ((hash >> 16) % 6) as f64,
            height * 0.5,
            width + ((hash >> 48) % 4) as f64,
        ],
        floor,
        axis: [angle.cos(), angle.sin()],
        landform,
        climate,
        hash,
    }
}

#[derive(Clone, Copy)]
pub(super) struct Entrance {
    pub(super) inner: Node,
    pub(super) middle: [f64; 3],
    pub(super) mouth: [f64; 3],
    pub(super) direction: [f64; 2],
}

pub(super) fn entrance(
    terrain: &Terrain,
    cell_x: i64,
    cell_z: i64,
    query: Bounds,
) -> Option<Entrance> {
    let inner = node(terrain.seed(), cell_x, cell_z, false);
    if !inner.hash.is_multiple_of(3) && (cell_x, cell_z) != (0, 0) {
        return None;
    }
    let direction = match (inner.hash >> 56) % 4 {
        0 => [1.0, 0.0],
        1 => [-1.0, 0.0],
        2 => [0.0, 1.0],
        _ => [0.0, -1.0],
    };
    let mouth_x = inner.center[0] + direction[0] * ENTRANCE_RUN;
    let mouth_z = inner.center[2] + direction[1] * ENTRANCE_RUN;
    let horizontal_bounds = Bounds {
        min_x: inner.center[0].min(mouth_x) - 18.0,
        max_x: inner.center[0].max(mouth_x) + 18.0,
        min_z: inner.center[2].min(mouth_z) - 18.0,
        max_z: inner.center[2].max(mouth_z) + 18.0,
        min_y: -25.0,
        max_y: f64::from(WORLD_HEIGHT),
    };
    if !horizontal_bounds.intersects(query) {
        return None;
    }
    let (Ok(x), Ok(z)) = (
        i32::try_from(mouth_x.floor() as i64),
        i32::try_from(mouth_z.floor() as i64),
    ) else {
        return None;
    };
    let surface = terrain.column(x, z);
    if surface.height < SEA_LEVEL + 3 || terrain.water_column(x, z).is_some() {
        return None;
    }
    let mouth = [mouth_x, f64::from(surface.height) + 1.5, mouth_z];
    let middle = [
        (inner.center[0] + mouth_x) * 0.5 - direction[1] * 10.0,
        (inner.floor + mouth[1] - 3.0) * 0.5,
        (inner.center[2] + mouth_z) * 0.5 + direction[0] * 10.0,
    ];
    let entrance = Entrance {
        inner,
        middle,
        mouth,
        direction,
    };
    entrance.clears_surface_water(terrain).then_some(entrance)
}

impl Entrance {
    fn clears_surface_water(self, terrain: &Terrain) -> bool {
        let mouth_floor = [self.mouth[0], self.mouth[1] - 3.0, self.mouth[2]];
        let points = [self.inner.anchor(), self.middle, mouth_floor];
        let bounds = |axis| {
            let min = points
                .iter()
                .map(|point| point[axis])
                .fold(f64::INFINITY, f64::min);
            let max = points
                .iter()
                .map(|point| point[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            (min.floor() as i32, max.ceil() as i32)
        };
        let (min_x, max_x) = bounds(0);
        let (min_z, max_z) = bounds(2);
        let hydrology = HydrologyRegion::columns(terrain.seed(), min_x, max_x, min_z, max_z);
        for pair in points.windows(2) {
            let steps =
                ((pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]) / 4.0).ceil() as usize;
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let point: [f64; 3] =
                    std::array::from_fn(|axis| pair[0][axis] + (pair[1][axis] - pair[0][axis]) * t);
                let x = point[0].floor() as i32;
                let z = point[2].floor() as i32;
                // Reject flooded/sealed entrances instead of carving a leak through a basin bed.
                if hydrology
                    .column(terrain, x, z)
                    .seal_from
                    .is_some_and(|first| point[1] + 4.0 >= f64::from(first))
                {
                    return false;
                }
            }
        }
        true
    }
}

impl UndergroundRegion {
    pub(super) fn connect(
        &mut self,
        start: Node,
        end: Node,
        radius: f64,
        height: f64,
        query: Bounds,
    ) {
        let dx = end.center[0] - start.center[0];
        let dz = end.center[2] - start.center[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let offset = 7.0 + ((start.hash >> 36) % 10) as f64;
        let phase = unit(start.hash.rotate_left(7) ^ end.hash) * std::f64::consts::TAU;
        let points: [[f64; 3]; 7] = std::array::from_fn(|index| {
            let t = index as f64 / 6.0;
            let envelope = (t * std::f64::consts::PI).sin();
            let bend = (t * std::f64::consts::TAU + phase).sin() * offset * envelope;
            [
                start.center[0] + dx * t - dz / distance * bend,
                start.floor + (end.floor - start.floor) * t,
                start.center[2] + dz * t + dx / distance * bend,
            ]
        });
        for (index, pair) in points.windows(2).enumerate() {
            self.add_passage(
                pair[0],
                pair[1],
                radius + (index as f64 + phase).sin() * 0.4,
                height + (index % 3) as f64 * 1.2,
                phase + index as f64,
                query,
            );
        }
        if start.floor > -35.0 && end.floor > -35.0 && start.hash & 1 == 0 {
            // A longer flank branch rejoins the main gallery: a genuine loop,
            // not another dead-end sphere attached by a straight cylinder.
            let side = [
                (start.center[0] + end.center[0]) * 0.5 - dz / distance * 19.0,
                (start.floor + end.floor) * 0.5 + 1.0,
                (start.center[2] + end.center[2]) * 0.5 + dx / distance * 19.0,
            ];
            self.add_passage(points[1], side, 4.7, 8.0, phase, query);
            self.add_passage(side, points[5], 5.0, 7.0, phase + 1.0, query);
        }
    }
}

pub(in crate::world::generation) fn cave_landform(
    seed: u64,
    x: i32,
    y: i32,
    z: i32,
) -> CaveLandform {
    let cell_x = i64::from(x).div_euclid(CELL_SIZE);
    let cell_z = i64::from(z).div_euclid(CELL_SIZE);
    let mut best = (f64::INFINITY, CaveLandform::Gallery);
    for dz in -1..=1 {
        for dx in -1..=1 {
            for deep in [false, true] {
                let node = node(seed, cell_x + dx, cell_z + dz, deep);
                let distance = (node.center[0] - f64::from(x)).powi(2)
                    + (node.center[2] - f64::from(z)).powi(2)
                    + (node.floor - f64::from(y)).powi(2) * 2.0;
                if distance < best.0 {
                    best = (distance, node.landform);
                }
            }
        }
    }
    best.1
}
