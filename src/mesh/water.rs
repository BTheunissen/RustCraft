//! Fractional water faces and shared-corner fluid/wave metadata.

use super::{Block, Face, HashMap, Mesh, UVS, Vertex, WaterBiome, WaterSample, biome_wave_weight};

#[derive(Clone, Copy)]
pub(super) struct WaterRun {
    pub(super) first: i32,
    pub(super) last: i32,
    pub(super) sample: WaterSample,
    /// Overhead voxel is air: wave eligibility, not fractional-cap visibility.
    pub(super) exposed: bool,
}

impl WaterRun {
    fn surface_visible(self) -> bool {
        self.exposed || self.sample.surface < self.last as f32 + 1.0
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct WaterCorner {
    pub(super) data: [f32; 4],
    pub(super) vertical_flow: f32,
    pub(super) foam: f32,
    pub(super) weight: f32,
}

#[derive(Default)]
pub(super) struct WaterMetadata {
    pub(super) columns: HashMap<[i32; 2], Vec<WaterRun>>,
    pub(super) corners: HashMap<(i64, i64, u32), WaterCorner>,
}

impl WaterMetadata {
    pub(super) fn add_column(
        &mut self,
        position: [i32; 2],
        origin_y: i32,
        blocks: &[Block],
        mut profile: impl FnMut(i32) -> Option<WaterSample>,
    ) {
        let mut runs = Vec::new();
        let mut index = 0;
        while index < blocks.len() {
            if blocks[index] != Block::Water {
                index += 1;
                continue;
            }
            let first = origin_y + index as i32;
            while index + 1 < blocks.len() && blocks[index + 1] == Block::Water {
                index += 1;
            }
            let last = origin_y + index as i32;
            let mut sample = profile(last)
                .filter(|water| {
                    water.surface.is_finite()
                        && water.surface > last as f32
                        && water.surface <= last as f32 + 1.0
                })
                .unwrap_or(WaterSample {
                    bottom: first.saturating_sub(1),
                    surface: last as f32 + 1.0,
                    flow: [0.0; 3],
                    biome: WaterBiome::CavePool,
                    salinity: 0.0,
                });
            // Carved bed shelves and separate cave pools override a nominal
            // hydrology bed: attenuation must use the actual contiguous liquid.
            sample.bottom = first.saturating_sub(1);
            sample.salinity = finite_clamp(sample.salinity, 0.0, 1.0);
            sample.flow = sample.flow.map(|flow| finite_clamp(flow, -8.0, 8.0));
            runs.push(WaterRun {
                first,
                last,
                sample,
                exposed: blocks.get(index + 1).copied().unwrap_or(Block::Air) == Block::Air,
            });
            index += 1;
        }
        if !runs.is_empty() {
            self.columns.insert(position, runs);
        }
    }

    pub(super) fn at(&self, world: [i32; 3]) -> Option<WaterRun> {
        self.columns
            .get(&[world[0], world[2]])?
            .iter()
            .find(|run| (run.first..=run.last).contains(&world[1]))
            .copied()
    }

    pub(super) fn corner(&mut self, x: i64, z: i64, surface: f32) -> WaterCorner {
        let key = (x, z, surface.to_bits());
        if let Some(corner) = self.corners.get(&key) {
            return *corner;
        }
        let mut corner = WaterCorner::default();
        let mut wet = 0;
        let mut level = 0;
        let mut minimum_depth = f32::MAX;
        let mut weight = 1.0_f32;
        for position in [[x - 1, z - 1], [x, z - 1], [x - 1, z], [x, z]] {
            let [Ok(x), Ok(z)] = position.map(i32::try_from) else {
                continue;
            };
            let Some(run) = self.columns.get(&[x, z]).and_then(|runs| {
                runs.iter().find(|run| {
                    run.surface_visible() && (run.sample.surface - surface).abs() <= 1.0
                })
            }) else {
                continue;
            };
            wet += 1;
            level += usize::from(run.exposed && run.sample.surface == surface);
            let depth = (run.sample.surface - run.first as f32).max(0.0);
            minimum_depth = minimum_depth.min(depth);
            weight = weight.min(biome_wave_weight(run.sample.biome));
            corner.data[0] += run.sample.flow[0];
            corner.data[1] += run.sample.flow[2];
            corner.data[2] += depth;
            corner.data[3] += run.sample.salinity;
            corner.vertical_flow += run.sample.flow[1];
        }
        if wet > 0 {
            corner.data = corner.data.map(|value| value / wet as f32);
            corner.vertical_flow /= wet as f32;
            let shallow = 1.0 - smoothstep(0.1, 1.8, minimum_depth);
            corner.foam = ((4 - wet) as f32 * 0.24 + shallow * 0.48).min(1.0);
            // A corner shared with dry terrain, a ceiling, or a different level
            // is pinned for every copy, including the upper edge of side faces.
            if level == 4 {
                corner.weight = weight * smoothstep(0.2, 1.4, minimum_depth);
            }
        }
        self.corners.insert(key, corner);
        corner
    }
}

pub(super) fn finite_clamp(value: f32, low: f32, high: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        0.0_f32.clamp(low, high)
    }
}

pub(super) fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let fraction = ((value - low) / (high - low)).clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

impl Mesh {
    pub(super) fn push_water_face(
        &mut self,
        world: [i32; 3],
        face: &Face,
        light: f32,
        run: WaterRun,
        heights: [f32; 2],
        metadata: &mut WaterMetadata,
    ) {
        let base = self.vertices.len() as u32;
        for (corner, uv) in face.corners.into_iter().zip(UVS) {
            let position = [
                world[0] as f32 + corner[0],
                heights[usize::from(corner[1] > 0.5)],
                world[2] as f32 + corner[2],
            ];
            let mut water = [
                run.sample.flow[0],
                run.sample.flow[2],
                (run.sample.surface - run.first as f32).max(0.0),
                run.sample.salinity,
            ];
            let mut water_extra = [run.sample.flow[1], run.sample.biome as u8 as f32, 0.0, 0.0];
            if run.surface_visible() && position[1] == run.sample.surface {
                let shared = metadata.corner(
                    i64::from(world[0]) + corner[0] as i64,
                    i64::from(world[2]) + corner[2] as i64,
                    run.sample.surface,
                );
                water = shared.data;
                water_extra[0] = shared.vertical_flow;
                water_extra[2] = shared.foam;
                water_extra[3] = shared.weight;
            } else if run.sample.biome == WaterBiome::Waterfall {
                let depth = run.sample.surface - run.first as f32;
                let from_bed = position[1] - run.first as f32;
                water_extra[2] = 1.0 - smoothstep(0.0, depth.clamp(0.1, 2.0), from_bed);
            }
            self.vertices.push(Vertex {
                position,
                normal: face.offset.map(|axis| axis as f32),
                uv,
                material: 9,
                light,
                water,
                water_extra,
                local_light: [0.0; 3],
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
