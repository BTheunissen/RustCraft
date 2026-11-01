//! Basin and channel geometry.

use super::{BANK_APRON, mix, normalized, smoothstep};

#[derive(Clone, Copy)]
pub(super) struct Basin {
    pub(super) center: [f64; 2],
    pub(super) radius: [f64; 2],
    pub(super) surface: f64,
    pub(super) depth: f64,
}

impl Basin {
    pub(super) fn radius_at(self, point: [f64; 2]) -> f64 {
        (((point[0] - self.center[0]) / self.radius[0]).powi(2)
            + ((point[1] - self.center[1]) / self.radius[1]).powi(2))
        .sqrt()
    }

    pub(super) fn bed(self, radius: f64, existing: f64) -> f64 {
        self.bed_with_apron(radius, existing, BANK_APRON)
    }

    pub(super) fn bed_with_apron(self, radius: f64, existing: f64, apron: f64) -> f64 {
        if radius <= 1.0 {
            self.surface - 1.0 - self.depth * (1.0 - radius * radius)
        } else {
            let distance = (radius - 1.0) * self.radius[0].min(self.radius[1]);
            let bank = self.surface - 1.0 + 2.0 * smoothstep(distance / 6.0);
            mix(bank, existing, smoothstep((distance - 4.0) / (apron - 4.0)))
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RiverPoint {
    pub(super) position: [f64; 2],
    pub(super) surface: f64,
    pub(super) progress: f64,
}

#[derive(Clone, Copy)]
pub(super) struct RiverSegment {
    pub(super) start: RiverPoint,
    pub(super) end: RiverPoint,
    pub(super) start_direction: [f64; 2],
    pub(super) end_direction: [f64; 2],
    pub(super) width: f64,
    pub(super) depth: f64,
    pub(super) waterfall: bool,
    pub(super) tributary: bool,
}

#[derive(Clone, Copy)]
pub(super) struct RiverProjection {
    pub(super) distance: f64,
    pub(super) surface: f64,
    pub(super) progress: f64,
    pub(super) direction: [f64; 2],
    pub(super) center: [f64; 2],
    pub(super) grade: f64,
    pub(super) width: f64,
    pub(super) depth: f64,
    pub(super) waterfall: bool,
    pub(super) tributary: bool,
}

impl RiverSegment {
    pub(super) fn project(self, point: [f64; 2]) -> Option<RiverProjection> {
        let dx = self.end.position[0] - self.start.position[0];
        let dz = self.end.position[1] - self.start.position[1];
        let length_squared = dx * dx + dz * dz;
        let t = (((point[0] - self.start.position[0]) * dx
            + (point[1] - self.start.position[1]) * dz)
            / length_squared)
            .clamp(0.0, 1.0);
        let offset_x = point[0] - self.start.position[0] - dx * t;
        let offset_z = point[1] - self.start.position[1] - dz * t;
        let distance_squared = offset_x * offset_x + offset_z * offset_z;
        if distance_squared > (self.width + 7.0 + BANK_APRON).powi(2) {
            return None;
        }
        let length = length_squared.sqrt();
        let progress = mix(self.start.progress, self.end.progress, t);
        let direction = normalized([
            mix(self.start_direction[0], self.end_direction[0], t),
            mix(self.start_direction[1], self.end_direction[1], t),
        ]);
        Some(RiverProjection {
            distance: distance_squared.sqrt(),
            surface: mix(self.start.surface, self.end.surface, t),
            progress,
            direction,
            center: [
                self.start.position[0] + dx * t,
                self.start.position[1] + dz * t,
            ],
            grade: (self.start.surface - self.end.surface) / length,
            width: if self.tributary {
                self.width
            } else {
                self.width + 7.0 * smoothstep((progress - 0.72) / 0.28)
            },
            depth: if self.waterfall {
                self.depth + (self.start.surface - self.end.surface) * (1.0 - t)
            } else {
                self.depth
            },
            waterfall: self.waterfall,
            tributary: self.tributary,
        })
    }
}
