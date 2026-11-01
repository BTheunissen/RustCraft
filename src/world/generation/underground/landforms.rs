//! Family-specific rooms, pillars, shelves, and protected passages.

use super::{Bounds, CaveLandform, Feature, Mask, Node, Room, UndergroundRegion};

#[derive(Clone, Copy)]
pub(super) enum RetainedRock {
    Pillar {
        room: Room,
        x: f64,
        z: f64,
        radius: f64,
    },
    Shelf {
        room: Room,
        bridge: bool,
    },
}

impl RetainedRock {
    pub(super) fn bounds(self) -> Bounds {
        match self {
            Self::Pillar { room, x, z, radius } => Bounds {
                min_x: x - radius,
                max_x: x + radius,
                min_z: z - radius,
                max_z: z + radius,
                ..room.bounds()
            },
            Self::Shelf { room, .. } => room.bounds(),
        }
    }

    pub(super) fn apply(self, mask: &mut Mask, x: f64, z: f64) {
        if !self.bounds().contains_column(x, z) {
            return;
        }
        match self {
            Self::Pillar {
                room,
                x: root_x,
                z: root_z,
                radius,
            } => {
                let distance = ((x - root_x).powi(2) + (z - root_z).powi(2)).sqrt();
                if distance > radius {
                    return;
                }
                let Some((first, last)) = room.span(x, z) else {
                    return;
                };
                let waist = radius * 0.6;
                if distance <= waist {
                    mask.range(first - 1, last + 1, false);
                } else {
                    let fraction = ((distance - waist) / (radius - waist)).sqrt();
                    let height = f64::from(last - first);
                    mask.range(
                        first - 1,
                        first + (height * (1.0 - fraction) * 0.5) as i32,
                        false,
                    );
                    mask.range(
                        last - (height * (1.0 - fraction) * 0.5) as i32,
                        last + 1,
                        false,
                    );
                }
            }
            Self::Shelf { room, bridge } => {
                let dx = x - room.origin[0];
                let dz = z - room.origin[2];
                let along = dx * room.axis[0] + dz * room.axis[1];
                let across = -dx * room.axis[1] + dz * room.axis[0];
                let ledge = if bridge {
                    along.abs() < 2.2 || across.abs() > room.width * 0.48
                } else {
                    across.abs() > room.width * 0.52
                };
                if ledge && along.abs() < room.length * 0.74 {
                    let height = if bridge { 11 } else { 3 };
                    let first = room.origin[1] as i32 + height;
                    mask.range(first, first + 1, false);
                }
            }
        }
    }
}

impl UndergroundRegion {
    pub(super) fn push_cave(&mut self, feature: Feature, query: Bounds) {
        if feature.bounds.intersects(query) {
            self.caves.push(feature);
        }
    }

    pub(super) fn add_passage(
        &mut self,
        start: [f64; 3],
        end: [f64; 3],
        width: f64,
        height: f64,
        phase: f64,
        query: Bounds,
    ) {
        let feature = Feature::passage(start, end, width, height, phase);
        if feature.bounds.intersects(query) {
            self.routes.push(feature);
        }
    }

    pub(super) fn retain(&mut self, rock: RetainedRock, query: Bounds) {
        if rock.bounds().intersects(query) {
            self.retained.push(rock);
        }
    }

    pub(super) fn add_landform(&mut self, node: Node, query: Bounds) {
        let room = node.room();
        let bounds = Bounds {
            max_y: room.bounds().max_y + 16.0,
            ..room.bounds().expanded(12.0)
        };
        if !bounds.intersects(query) {
            return;
        }
        if node.landform != CaveLandform::Labyrinth {
            self.push_cave(Feature::room(room), query);
        }
        match node.landform {
            CaveLandform::Gallery => {
                for (along, across) in [(-11.0, -10.0), (10.0, 11.0)] {
                    self.push_cave(
                        Feature::room(Room {
                            origin: node.point(along, across, node.floor + 1.0),
                            length: 17.0,
                            width: 9.0,
                            height: 7.0,
                            phase: room.phase + across * 0.2,
                            ..room
                        }),
                        query,
                    );
                }
                self.retain(
                    RetainedRock::Shelf {
                        room,
                        bridge: false,
                    },
                    query,
                );
                // A remnant bedding level sits above one flank, leaving real
                // rock between the active low gallery and the abandoned passage.
                self.push_cave(
                    Feature::room(Room {
                        origin: node.point(-4.0, -9.0, node.floor + 14.0),
                        length: 19.0,
                        width: 7.0,
                        height: 5.0,
                        phase: room.phase + 1.0,
                        ..room
                    }),
                    query,
                );
            }
            CaveLandform::Rift => {
                self.push_cave(
                    Feature::room(Room {
                        width: room.width + 3.0,
                        height: 7.0,
                        landform: CaveLandform::Gallery,
                        ..room
                    }),
                    query,
                );
                self.retain(RetainedRock::Shelf { room, bridge: true }, query);
            }
            CaveLandform::Cathedral => {
                for (along, across) in [(-12.0, 10.0), (12.0, -9.0)] {
                    self.push_cave(
                        Feature::room(Room {
                            origin: node.point(along, across, node.floor + 1.0),
                            length: 18.0,
                            width: 13.0,
                            height: room.height * 0.7,
                            phase: room.phase + 0.8,
                            ..room
                        }),
                        query,
                    );
                }
                for (index, (along, across)) in
                    [(-12.0, -9.0), (-9.0, 9.0), (9.0, -10.0), (12.0, 10.0)]
                        .into_iter()
                        .enumerate()
                {
                    let root = node.point(along, across, node.floor);
                    self.retain(
                        RetainedRock::Pillar {
                            room,
                            x: root[0],
                            z: root[2],
                            radius: 2.6 + ((node.hash >> (index * 7)) % 3) as f64 * 0.55,
                        },
                        query,
                    );
                }
            }
            CaveLandform::Labyrinth => {
                let points: [[f64; 3]; 9] = std::array::from_fn(|index| {
                    let angle = index as f64 * std::f64::consts::TAU / 8.0;
                    node.point(
                        angle.cos() * room.length * 0.72,
                        angle.sin() * room.width * 0.72,
                        node.floor + (angle * 2.0 + room.phase).sin() * 0.6,
                    )
                });
                for (index, pair) in points.windows(2).enumerate() {
                    self.add_passage(
                        pair[0],
                        pair[1],
                        4.4 + (index as f64 + room.phase).sin() * 0.8,
                        7.4 + (index % 3) as f64,
                        room.phase + index as f64,
                        query,
                    );
                }
                for side in [-1.0, 1.0] {
                    self.push_cave(
                        Feature::passage(
                            node.point(-11.0, side * 13.0, node.floor),
                            node.point(-22.0, side * 21.0, node.floor + 1.0),
                            4.5,
                            6.5,
                            room.phase,
                        ),
                        query,
                    );
                }
                self.push_cave(
                    Feature::passage(
                        node.point(-18.0, 8.0, node.floor + 12.0),
                        node.point(17.0, -9.0, node.floor + 12.0),
                        5.0,
                        6.0,
                        room.phase,
                    ),
                    query,
                );
            }
        }
        for (start, end) in [
            (
                node.point(-room.length * 0.8, 0.0, node.floor),
                node.point(room.length * 0.8, 0.0, node.floor),
            ),
            (
                node.point(0.0, -room.width * 0.7, node.floor),
                node.point(0.0, room.width * 0.7, node.floor),
            ),
        ] {
            self.add_passage(start, end, 4.0, 7.0, room.phase, query);
        }
    }
}
