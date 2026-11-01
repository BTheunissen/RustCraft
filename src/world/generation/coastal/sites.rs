//! Stable bounded diagnostics use the same catchment roots and real occupancy.

use super::{Coast, CoastalLandmark, CoastalStack, END_BLEND, MAX_U, MIN_U, Terrain};

impl Coast {
    pub(in crate::world::generation) fn landmark(
        &self,
        terrain: &Terrain,
    ) -> Option<CoastalLandmark> {
        let min_v = self.center_v - self.half_length - END_BLEND;
        let max_v = self.center_v + self.half_length + END_BLEND;
        let corners = [
            [MIN_U, min_v],
            [MAX_U, min_v],
            [MIN_U, max_v],
            [MAX_U, max_v],
        ]
        .map(|point| self.world(point));
        let bounds = [
            [
                corners
                    .iter()
                    .map(|point| point[0])
                    .fold(f64::INFINITY, f64::min)
                    .floor() as i32,
                corners
                    .iter()
                    .map(|point| point[1])
                    .fold(f64::INFINITY, f64::min)
                    .floor() as i32,
            ],
            [
                corners
                    .iter()
                    .map(|point| point[0])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil() as i32,
                corners
                    .iter()
                    .map(|point| point[1])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil() as i32,
            ],
        ];
        let cove = self.feet(terrain, [self.edge(self.cove_v) + 3.0, self.cove_v])?;
        let stacks = self
            .stacks
            .iter()
            .map(|stack| {
                let center = self.world(stack.center).map(|value| value.floor() as i32);
                CoastalStack {
                    center,
                    top: terrain.column(center[0], center[1]).height,
                }
            })
            .collect::<Vec<_>>();
        let viewpoint = [-23.0, -27.0, -31.0, -19.0, 0.0, 23.0, 31.0, -47.0, 47.0]
            .into_iter()
            .find_map(|offset| {
                let v = self.cove_v + offset;
                [2.0, 3.5, 5.0].into_iter().find_map(|inland| {
                    let feet = self.feet(terrain, [self.edge(v) - inland, v])?;
                    let visible = stacks
                        .iter()
                        .filter(|stack| visible_stack(terrain, feet, **stack))
                        .count();
                    (visible >= 3).then_some(feet)
                })
            })?;
        Some(CoastalLandmark {
            bounds,
            viewpoint,
            cove,
            stacks,
            arch: self.arch.landmark(self, terrain)?,
        })
    }

    fn feet(&self, terrain: &Terrain, local: [f64; 2]) -> Option<[f32; 3]> {
        let [x, z] = self.world(local).map(|value| value.floor() as i32);
        let feet = terrain.column(x, z).height + 1;
        if terrain.water_column(x, z).is_some()
            || !terrain.block_at(x, feet - 1, z).is_solid()
            || !(0..3).all(|dy| terrain.block_at(x, feet + dy, z) == crate::world::Block::Air)
        {
            return None;
        }
        Some([x as f32 + 0.5, feet as f32, z as f32 + 0.5])
    }
}

fn visible_stack(terrain: &Terrain, feet: [f32; 3], stack: CoastalStack) -> bool {
    let eye = [
        f64::from(feet[0]),
        f64::from(feet[1]) + 1.62,
        f64::from(feet[2]),
    ];
    let target = [
        f64::from(stack.center[0]) + 0.5,
        f64::from(stack.top + 18) * 0.5,
        f64::from(stack.center[1]) + 0.5,
    ];
    let delta: [f64; 3] = std::array::from_fn(|axis| target[axis] - eye[axis]);
    let distance = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    let steps = (distance * 2.0).ceil() as usize;
    for step in 1..steps {
        let point: [f64; 3] =
            std::array::from_fn(|axis| eye[axis] + delta[axis] * step as f64 / steps as f64);
        let [x, y, z] = point.map(|value| value.floor() as i32);
        if terrain.block_at(x, y, z).is_solid() {
            return (point[0] - target[0]).hypot(point[2] - target[2]) <= 10.0;
        }
    }
    true
}
