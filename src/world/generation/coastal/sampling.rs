//! Mainland headlands, a broad reversible cove ramp, and tapered sea stacks.

use super::{
    Coast, CoastalColumn, END_BLEND, HydrologyColumn, MAX_U, MIN_U, SEA_LEVEL, smooth, value_noise,
};
use crate::world::{WaterBiome, WaterColumn};

impl Coast {
    pub(in crate::world::generation) fn column(
        &self,
        seed: u64,
        world: [f64; 2],
        mut profile: HydrologyColumn,
        freshwater_weight: impl FnOnce([f64; 2]) -> f64,
    ) -> Option<HydrologyColumn> {
        let [u, v] = self.local(world);
        let along = (v - self.center_v).abs();
        if !(MIN_U..=MAX_U).contains(&u)
            || along >= self.half_length + END_BLEND
            || profile
                .water
                .is_some_and(|water| !matches!(water.biome, WaterBiome::Ocean))
        {
            return None;
        }
        let distance = u - self.edge(v);
        if !(-90.0..=66.0).contains(&distance) {
            return None;
        }
        let end_weight =
            (1.0 - smooth((along - self.half_length) / END_BLEND)) * freshwater_weight([u, v]);
        if end_weight <= 0.0 {
            return None;
        }
        let original = profile.column.height;
        let roughness = value_noise(
            seed ^ 0x434F_4153_545F_524B,
            world[0] / 13.0,
            world[1] / 13.0,
        );
        let cliff = self.cliff_height + roughness * 1.5 + ((v - self.center_v) / 23.0).cos() * 1.5;
        let cove = 1.0 - smooth(((v - self.cove_v).abs() - 10.0) / 12.0);
        let ramp = f64::from(SEA_LEVEL) + (-distance - 8.0).max(0.0) * 0.36;
        let mainland = cliff + (ramp.min(cliff) - cliff) * cove;
        let mut bed = if distance <= 0.0 {
            let weight = smooth((distance + 90.0) / 62.0);
            f64::from(original) + (mainland - f64::from(original)) * weight
        } else {
            // The cove includes dry beach before gently meeting the supported shelf.
            let beach = f64::from(SEA_LEVEL);
            let apron = 1.0 - smooth((distance - 7.0) / 18.0);
            let cove_bed = f64::from(original) + (beach - f64::from(original)) * apron * cove;
            let shore =
                beach + (f64::from(original).min(beach) - beach) * smooth((distance - 7.0) / 8.0);
            cove_bed.max(shore)
        };
        bed = f64::from(original) + (bed - f64::from(original)) * end_weight;
        let mut stack = false;
        if distance > 8.0 {
            for body in &self.stacks {
                if let Some(height) = body.top([u, v]) {
                    bed = bed.max(f64::from(SEA_LEVEL + height));
                    stack = true;
                }
            }
        }
        let arch = self.arch.project(self, [u, v]);
        if let Some((top, None)) = arch {
            bed = bed.max(f64::from(top));
        } else if arch.is_some() {
            bed = bed.min(f64::from(SEA_LEVEL - 4));
        }
        let height = bed.round() as i32;
        profile.column.height = height;
        profile.seal_from = Some(profile.seal_from.unwrap_or(original - 3).min(height - 3));
        let sediment = !stack && (cove > 0.5 || distance > 0.0) && height <= SEA_LEVEL + 2;
        profile.sediment = sediment.then_some(crate::world::Block::Sand);
        profile.coastal = Some(CoastalColumn {
            ground: height,
            rock_from: if stack || arch.is_some() {
                height.min(SEA_LEVEL - 8).min(original) - 3
            } else {
                original.min(height) - 3
            },
            sediment,
            layer: self.layer,
            roof: arch.and_then(|(_, roof)| roof),
        });
        profile.water = (height + 1 < SEA_LEVEL && distance > 0.0).then_some(WaterColumn {
            bottom: height,
            surface: SEA_LEVEL as f32,
            flow: [
                (self.axis[0] * 0.6 - self.axis[1] * 0.8) as f32 * 0.1,
                0.0,
                (self.axis[1] * 0.6 + self.axis[0] * 0.8) as f32 * 0.1,
            ],
            biome: WaterBiome::Ocean,
            salinity: 1.0,
        });
        Some(profile)
    }
}

impl super::Stack {
    fn top(self, point: [f64; 2]) -> Option<i32> {
        let dx = point[0] - self.center[0];
        let dz = point[1] - self.center[1];
        let (sin, cos) = self.angle.sin_cos();
        let x = (dx * cos + dz * sin) / self.radius[0];
        let z = (-dx * sin + dz * cos) / self.radius[1];
        let radius = x.hypot(z);
        if radius > 1.0 {
            return None;
        }
        let crown = ((x - self.lean * 0.09).hypot(z * 1.1) / 0.70).clamp(0.0, 1.0);
        let taper = 1.0 - smooth((radius - 0.55) / 0.45) * 0.72;
        let ledge = if radius < 0.82 { 1.0 } else { 0.84 };
        Some(
            (f64::from(self.height) * taper * ledge - crown * 2.0)
                .round()
                .max(3.0) as i32,
        )
    }
}
