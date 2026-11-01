//! Bounded cave shapes and vertical occupancy masks.

use super::{CaveLandform, FIRST_CAVE_Y, MASK_WORDS, MIN_WORLD_Y, WORLD_HEIGHT};

#[derive(Clone, Copy, Default)]
pub(super) struct Mask {
    pub(super) words: [u64; MASK_WORDS],
}

impl Mask {
    pub(super) fn contains(self, y: i32) -> bool {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return false;
        }
        let index = (y - MIN_WORLD_Y) as usize;
        self.words[index / 64] & (1 << (index % 64)) != 0
    }

    pub(super) fn range(&mut self, first: i32, last: i32, open: bool) {
        let first = first.max(FIRST_CAVE_Y);
        let last = last.min(WORLD_HEIGHT - 1);
        if first > last {
            return;
        }
        let first = (first - MIN_WORLD_Y) as usize;
        let last = (last - MIN_WORLD_Y) as usize;
        for word in first / 64..=last / 64 {
            let low = first.saturating_sub(word * 64);
            let high = (last - word * 64).min(63);
            let width = high - low + 1;
            let bits = if width == 64 {
                u64::MAX
            } else {
                ((1_u64 << width) - 1) << low
            };
            if open {
                self.words[word] |= bits;
            } else {
                self.words[word] &= !bits;
            }
        }
    }

    pub(super) fn union(&mut self, other: Self) {
        for (word, other) in self.words.iter_mut().zip(other.words) {
            *word |= other;
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub(super) min_x: f64,
    pub(super) max_x: f64,
    pub(super) min_z: f64,
    pub(super) max_z: f64,
    pub(super) min_y: f64,
    pub(super) max_y: f64,
}

impl Bounds {
    pub(super) fn expanded(self, distance: f64) -> Self {
        Self {
            min_x: self.min_x - distance,
            max_x: self.max_x + distance,
            min_z: self.min_z - distance,
            max_z: self.max_z + distance,
            ..self
        }
    }

    pub(super) fn intersects(self, other: Self) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_z <= other.max_z
            && self.max_z >= other.min_z
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }

    pub(super) fn contains_column(self, x: f64, z: f64) -> bool {
        (self.min_x..=self.max_x).contains(&x) && (self.min_z..=self.max_z).contains(&z)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Shape {
    Tunnel {
        start: [f64; 3],
        end: [f64; 3],
        radius: f64,
        height: f64,
    },
    Room(Room),
    Passage {
        start: [f64; 3],
        end: [f64; 3],
        width: f64,
        height: f64,
        phase: f64,
    },
}

#[derive(Clone, Copy)]
pub(super) struct Feature {
    pub(super) shape: Shape,
    pub(super) bounds: Bounds,
}

impl Feature {
    pub(super) fn tunnel(start: [f64; 3], end: [f64; 3], radius: f64, height: f64) -> Self {
        Self {
            shape: Shape::Tunnel {
                start,
                end,
                radius,
                height,
            },
            bounds: Bounds {
                min_x: start[0].min(end[0]) - radius,
                max_x: start[0].max(end[0]) + radius,
                min_z: start[2].min(end[2]) - radius,
                max_z: start[2].max(end[2]) + radius,
                min_y: start[1].min(end[1]) - height,
                max_y: start[1].max(end[1]) + height,
            },
        }
    }

    pub(super) fn room(room: Room) -> Self {
        Self {
            bounds: room.bounds(),
            shape: Shape::Room(room),
        }
    }

    pub(super) fn passage(
        start: [f64; 3],
        end: [f64; 3],
        width: f64,
        height: f64,
        phase: f64,
    ) -> Self {
        Self {
            shape: Shape::Passage {
                start,
                end,
                width,
                height,
                phase,
            },
            bounds: Bounds {
                min_x: start[0].min(end[0]) - width,
                max_x: start[0].max(end[0]) + width,
                min_z: start[2].min(end[2]) - width,
                max_z: start[2].max(end[2]) + width,
                min_y: start[1].min(end[1]) - 2.0,
                max_y: start[1].max(end[1]) + height + 2.0,
            },
        }
    }

    pub(super) fn passage_span(self, x: f64, z: f64) -> Option<(i32, i32, bool)> {
        if !self.bounds.contains_column(x, z) {
            return None;
        }
        let Shape::Passage {
            start,
            end,
            width,
            height,
            phase,
        } = self.shape
        else {
            return None;
        };
        let (t, distance_squared) = segment_projection(start, end, x, z);
        let remaining = 1.0 - distance_squared / (width * width);
        if remaining < 0.0 {
            return None;
        }
        let arch = remaining.sqrt();
        let floor = start[1] + (end[1] - start[1]) * t + (1.0 - arch) * 2.0;
        let first = floor.round() as i32;
        let roof_relief = (t * std::f64::consts::PI + phase).sin() * 0.65;
        let last = (floor + height * arch + roof_relief - 0.5).floor() as i32;
        (first <= last).then_some((first, last, distance_squared <= 2.6_f64.powi(2)))
    }

    pub(super) fn span(self, x: f64, z: f64) -> Option<(i32, i32)> {
        if !self.bounds.contains_column(x, z) {
            return None;
        }
        let (center, extent) = match self.shape {
            Shape::Room(room) => return room.span(x, z),
            Shape::Passage { .. } => {
                return self
                    .passage_span(x, z)
                    .map(|(first, last, _)| (first, last));
            }
            Shape::Tunnel {
                start,
                end,
                radius,
                height,
            } => {
                let (t, distance_squared) = segment_projection(start, end, x, z);
                let remaining = 1.0 - distance_squared / (radius * radius);
                if remaining < 0.0 {
                    return None;
                }
                (
                    start[1] + (end[1] - start[1]) * t,
                    height * remaining.sqrt(),
                )
            }
        };
        let first = (center - extent - 0.5).ceil() as i32;
        let last = (center + extent - 0.5).floor() as i32;
        (first <= last).then_some((first, last))
    }
}

/// A rotated, bounded-warp template with a bedded floor and an irregular roof.
/// Different cross sections matter more here than adding octaves to a sphere.
#[derive(Clone, Copy)]
pub(super) struct Room {
    pub(super) origin: [f64; 3],
    pub(super) axis: [f64; 2],
    pub(super) length: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) phase: f64,
    pub(super) landform: CaveLandform,
    pub(super) relief: f64,
}

impl Room {
    pub(super) fn bounds(self) -> Bounds {
        let reach_x =
            self.axis[0].abs() * (self.length + 2.5) + self.axis[1].abs() * (self.width + 4.5);
        let reach_z =
            self.axis[1].abs() * (self.length + 2.5) + self.axis[0].abs() * (self.width + 4.5);
        Bounds {
            min_x: self.origin[0] - reach_x,
            max_x: self.origin[0] + reach_x,
            min_z: self.origin[2] - reach_z,
            max_z: self.origin[2] + reach_z,
            min_y: self.origin[1] - 2.0,
            max_y: self.origin[1] + self.height + 2.0,
        }
    }

    pub(super) fn span(self, x: f64, z: f64) -> Option<(i32, i32)> {
        let dx = x - self.origin[0];
        let dz = z - self.origin[2];
        let along = dx * self.axis[0] + dz * self.axis[1];
        let across = -dx * self.axis[1] + dz * self.axis[0];
        let u = along + (across / 11.0 + self.phase).sin() * 1.8;
        let v = across + (along / 13.0 + self.phase).sin() * 2.7;
        let lobes = (u / 8.0 + self.phase * 2.0).sin() * 0.07;
        let plan = (u / self.length).powi(4) + (v / self.width).powi(2) + lobes;
        if plan >= 1.0 {
            return None;
        }
        let arch = (1.0 - plan.max(0.0)).sqrt();
        let floor_relief =
            (u / 15.0 + self.phase).sin() * 0.45 + (v / 12.0 - self.phase).sin() * 0.35;
        let bank = (plan - 0.55).max(0.0) * 3.0;
        let first = (self.origin[1] + floor_relief + bank).round() as i32;
        let roof_relief = ((u / 7.0 + self.phase).sin() * (v / 9.0 - self.phase).cos()
            + (u / 19.0 - v / 13.0).sin() * 0.6)
            * self.relief;
        let profile = match self.landform {
            CaveLandform::Rift => arch.powi(2),
            CaveLandform::Cathedral => arch,
            CaveLandform::Gallery | CaveLandform::Labyrinth => 0.35 + arch * 0.65,
        };
        let last = (self.origin[1] + self.height * profile + roof_relief - 0.5).floor() as i32;
        (last - first >= 1).then_some((first, last))
    }
}

pub(super) fn segment_projection(start: [f64; 3], end: [f64; 3], x: f64, z: f64) -> (f64, f64) {
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    let length_squared = dx * dx + dz * dz;
    let t = if length_squared < 0.000_001 {
        0.0
    } else {
        (((x - start[0]) * dx + (z - start[2]) * dz) / length_squared).clamp(0.0, 1.0)
    };
    let offset_x = x - (start[0] + dx * t);
    let offset_z = z - (start[2] + dz * t);
    (t, offset_x * offset_x + offset_z * offset_z)
}
