//! Fixed-work nearby resident water, independent of labels and render frequency.

use glam::Vec3;

use super::SoundEnvironment;
use crate::game::Environment;
use crate::world::{ResidentWorld, WaterBiome, WaterSample};

const SAMPLE_INTERVAL: f64 = 0.25;
const SOURCE_RADIUS: f64 = 34.0;

#[derive(Default)]
pub(in crate::game) struct Ambience {
    elapsed: f64,
    initialized: bool,
    water: [f32; 2],
}

impl Ambience {
    pub(in crate::game) fn sample(
        &mut self,
        world: &ResidentWorld,
        position: Vec3,
        environment: Environment,
        dt: f32,
    ) -> SoundEnvironment {
        if dt.is_finite() && dt > 0.0 {
            self.elapsed += f64::from(dt.min(0.25));
        }
        if !self.initialized || self.elapsed + 0.000_001 >= SAMPLE_INTERVAL {
            self.water = nearby_water(position, |x, z| world.surface_water_at(x, z));
            self.elapsed = 0.0;
            self.initialized = true;
        }
        profile(environment, self.water)
    }
}

fn nearby_water(
    position: Vec3,
    mut sample: impl FnMut(i32, i32) -> Option<WaterSample>,
) -> [f32; 2] {
    if !position.is_finite()
        || position
            .to_array()
            .into_iter()
            .any(|value| f64::from(value).abs() > f64::from(i32::MAX) - 64.0)
    {
        return [0.0; 2];
    }
    let mut result = [0.0_f32; 2];
    let x = f64::from(position.x).floor() as i64;
    let z = f64::from(position.z).floor() as i64;
    for (dx, dz) in (-8..=8).flat_map(|dz| (-8..=8).map(move |dx| (dx * 4, dz * 4))) {
        let (Ok(cell_x), Ok(cell_z)) = (i32::try_from(x + dx), i32::try_from(z + dz)) else {
            continue;
        };
        let Some(water) = sample(cell_x, cell_z) else {
            continue;
        };
        contribution(&mut result, position, [cell_x, cell_z], water);
    }
    // The grid includes the exact listener column, so there is no duplicate read.
    result
}

fn contribution(result: &mut [f32; 2], position: Vec3, [x, z]: [i32; 2], water: WaterSample) {
    if !water.surface.is_finite()
        || !water.salinity.is_finite()
        || f64::from(water.surface) <= f64::from(water.bottom) + 1.0
    {
        return;
    }
    let horizontal = (f64::from(position.x) - (f64::from(x) + 0.5))
        .hypot(f64::from(position.z) - (f64::from(z) + 0.5));
    let falloff = (1.0 - horizontal / SOURCE_RADIUS).clamp(0.0, 1.0);
    let height = (f64::from(position.y) - f64::from(water.surface)).abs();
    let smooth = falloff * falloff * (3.0 - 2.0 * falloff);
    let strength = (smooth / (1.0 + height / 32.0)) as f32;
    let marine = match water.biome {
        WaterBiome::Ocean => 1.0,
        WaterBiome::Estuary => water.salinity.clamp(0.0, 1.0),
        WaterBiome::Lake | WaterBiome::River | WaterBiome::Waterfall => 0.0,
        WaterBiome::CavePool => return,
    };
    result[0] = result[0].max(strength * marine);
    result[1] = result[1].max(strength * (1.0 - marine));
}

fn profile(environment: Environment, water: [f32; 2]) -> SoundEnvironment {
    let sky = if environment.sky_exposure.is_finite() {
        environment.sky_exposure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    SoundEnvironment {
        wind: sky * 0.65,
        surf: water[0] * sky,
        freshwater: water[1] * sky,
        cave: if environment.underground {
            (1.0 - sky) * 0.65
        } else {
            0.0
        },
        underwater: environment.underwater,
    }
}

#[cfg(test)]
mod tests;
