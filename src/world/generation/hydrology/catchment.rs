//! Seeded drainage topology and catchment coordinates.

use super::{
    Basin, CATCHMENT_CELL, Coast, RiverPoint, RiverSegment, SEA_LEVEL, hash_2d, mix,
    route_direction, unit,
};

// Move the connected system toward the continent without compressing river slopes or ocean depth.
const DRAINAGE_INLAND_SHIFT: f64 = 128.0;

pub(super) struct Catchment {
    pub(super) origin: [f64; 2],
    pub(super) axis: [f64; 2],
    pub(super) ocean: Basin,
    pub(super) island: Basin,
    pub(super) lakes: [Basin; 2],
    pub(super) nearby_lakes: [Basin; 2],
    pub(super) rivers: Vec<RiverSegment>,
    pub(super) coast: Coast,
}

impl Catchment {
    pub(super) fn new(seed: u64, cell_x: i64, cell_z: i64) -> Self {
        let hash = hash_2d(seed ^ 0x4859_4452_4F4C_4F47, cell_x, cell_z);
        let angle = unit(hash) * std::f64::consts::TAU;
        let axis = [angle.cos(), angle.sin()];
        let source_level = 39.0 + ((hash >> 24) % 6) as f64;
        let high_level = source_level + 10.0 + ((hash >> 40) % 4) as f64;
        let lakes = [
            Basin {
                center: [220.0, -180.0],
                radius: [35.0 + ((hash >> 8) % 5) as f64, 31.0],
                surface: source_level,
                depth: 9.0,
            },
            Basin {
                center: [192.0, -280.0],
                radius: [27.0, 23.0 + ((hash >> 16) % 4) as f64],
                surface: high_level,
                depth: 7.0,
            },
        ];
        let positions = [
            [220.0, -180.0],
            [262.0, -177.0],
            [274.0, -167.0],
            [297.0, -144.0],
            [324.0, -151.0],
            [344.0, -128.0],
            [348.0, -122.0],
            [358.0, -96.0],
            [366.0, -70.0],
            [396.0, -37.0],
            [420.0, 0.0],
        ];
        let sea = f64::from(SEA_LEVEL);
        let levels = [
            source_level,
            source_level,
            source_level,
            source_level - 3.0,
            source_level - 5.0,
            source_level - 7.0,
            source_level - 17.0,
            source_level - 19.0,
            sea,
            sea,
            sea,
        ];
        let main: [RiverPoint; 11] = std::array::from_fn(|index| RiverPoint {
            position: positions[index],
            surface: levels[index].max(sea),
            progress: index as f64 / 10.0,
        });
        let mut rivers = Vec::with_capacity(15);
        for (index, pair) in main.windows(2).enumerate() {
            rivers.push(RiverSegment {
                start: pair[0],
                end: pair[1],
                start_direction: route_direction(&main, index),
                end_direction: route_direction(&main, index + 1),
                width: 5.8,
                depth: 4.0,
                waterfall: index == 5,
                tributary: false,
            });
        }
        let tributary_positions = [
            lakes[1].center,
            [234.0, -288.0],
            [282.0, -266.0],
            [303.0, -229.0],
            [314.0, -186.0],
            main[3].position,
        ];
        let tributary: [RiverPoint; 6] = std::array::from_fn(|index| RiverPoint {
            position: tributary_positions[index],
            surface: if index == 0 {
                high_level
            } else {
                mix(high_level, main[3].surface, (index - 1) as f64 / 4.0)
            },
            progress: index as f64 / 5.0 * main[3].progress,
        });
        for (index, pair) in tributary.windows(2).enumerate() {
            rivers.push(RiverSegment {
                start: pair[0],
                end: pair[1],
                start_direction: route_direction(&tributary, index),
                end_direction: if index + 1 == tributary.len() - 1 {
                    route_direction(&main, 3)
                } else {
                    route_direction(&tributary, index + 1)
                },
                width: 3.4,
                depth: 2.8,
                waterfall: false,
                tributary: true,
            });
        }
        let origin = [
            (cell_x * CATCHMENT_CELL) as f64 - axis[0] * DRAINAGE_INLAND_SHIFT,
            (cell_z * CATCHMENT_CELL) as f64 - axis[1] * DRAINAGE_INLAND_SHIFT,
        ];
        let marine_radius = [164.0 + ((hash >> 32) % 12) as f64, 178.0];
        Self {
            origin,
            axis,
            ocean: Basin {
                center: [483.0, 64.0],
                radius: marine_radius,
                surface: sea,
                depth: 29.0,
            },
            island: Basin {
                center: [512.0, 132.0],
                radius: [24.0 + ((hash >> 48) % 8) as f64, 34.0],
                surface: 31.0,
                depth: 0.0,
            },
            lakes,
            nearby_lakes: [
                Basin {
                    center: [84.0 + DRAINAGE_INLAND_SHIFT, 76.0],
                    radius: [38.0 + ((hash >> 12) % 5) as f64, 32.0],
                    surface: sea + 7.0 + ((hash >> 20) % 3) as f64,
                    depth: 5.0,
                },
                Basin {
                    center: [-60.0 + DRAINAGE_INLAND_SHIFT, -100.0],
                    radius: [36.0, 30.0 + ((hash >> 44) % 4) as f64],
                    surface: sea + 5.0 + ((hash >> 28) % 3) as f64,
                    depth: 4.5,
                },
            ],
            rivers,
            coast: Coast::new(seed, hash, origin, axis, marine_radius),
        }
    }

    pub(super) fn local(&self, point: [f64; 2]) -> [f64; 2] {
        let dx = point[0] - self.origin[0];
        let dz = point[1] - self.origin[1];
        [
            dx * self.axis[0] + dz * self.axis[1],
            -dx * self.axis[1] + dz * self.axis[0],
        ]
    }

    pub(super) fn world(&self, point: [f64; 2]) -> [f64; 2] {
        [
            self.origin[0] + point[0] * self.axis[0] - point[1] * self.axis[1],
            self.origin[1] + point[0] * self.axis[1] + point[1] * self.axis[0],
        ]
    }

    pub(super) fn flow(&self, direction: [f64; 2], speed: f64, vertical: f64) -> [f32; 3] {
        [
            ((direction[0] * self.axis[0] - direction[1] * self.axis[1]) * speed) as f32,
            vertical as f32,
            ((direction[0] * self.axis[1] + direction[1] * self.axis[0]) * speed) as f32,
        ]
    }
}
