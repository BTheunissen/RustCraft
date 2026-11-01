//! Diagnostic candidate selection validates actual fluid occupancy.

use super::{Block, Catchment, Terrain, WORLD_HEIGHT, WaterBiome, WaterLandmark};

/// Diagnostic selection searches real generated water; feature templates only
/// supply candidates, never a different world or a forced camera clearing.
pub(in crate::world::generation) fn water_landmark(
    terrain: &Terrain,
    biome: WaterBiome,
) -> Option<WaterLandmark> {
    if biome == WaterBiome::CavePool {
        return None;
    }
    let catchment = Catchment::new(terrain.seed(), 0, 0);
    let candidate = match biome {
        WaterBiome::Lake => catchment.lakes[0].center,
        WaterBiome::River => [295.0, -146.0],
        WaterBiome::Estuary => [395.0, -38.0],
        WaterBiome::Ocean => [465.0, 0.0],
        WaterBiome::Waterfall => [346.0, -125.0],
        WaterBiome::CavePool => return None,
    };
    for dz in -10..=10 {
        for dx in -10..=10 {
            let world =
                catchment.world([candidate[0] + f64::from(dx), candidate[1] + f64::from(dz)]);
            let x = world[0].floor() as i32;
            let z = world[1].floor() as i32;
            let Some(water) = terrain.water_column(x, z).filter(|water| {
                water.biome == biome
                    && water.depth()
                        >= if matches!(biome, WaterBiome::Lake | WaterBiome::Ocean) {
                            4.0
                        } else {
                            3.0
                        }
            }) else {
                continue;
            };
            let y = water.surface.ceil() as i32 - 1;
            if terrain.block_at(x, y, z) != Block::Water {
                continue;
            }
            for distance in [18.0, 24.0, 12.0, 28.0] {
                for side in [1.0, -1.0, 0.0] {
                    let view = catchment.world([
                        candidate[0] + f64::from(dx) - distance * 0.25,
                        candidate[1] + f64::from(dz) + distance * side,
                    ]);
                    let camera_x = view[0].floor() as i32;
                    let camera_z = view[1].floor() as i32;
                    let view_distance = f64::from(camera_x - x).hypot(f64::from(camera_z - z));
                    if !(8.0..=30.0).contains(&view_distance) {
                        continue;
                    }
                    let ground = terrain.column(camera_x, camera_z);
                    let camera_water = terrain.water_column(camera_x, camera_z);
                    let top = camera_water
                        .map_or((ground.height + 1) as f32, |water| water.surface)
                        .max(water.surface);
                    let feet = top.ceil() as i32 + 3;
                    if feet + 3 >= WORLD_HEIGHT
                        || !(0..=3)
                            .all(|dy| terrain.block_at(camera_x, feet + dy, camera_z) == Block::Air)
                    {
                        continue;
                    }
                    return Some(WaterLandmark {
                        biome,
                        position: [camera_x as f32 + 0.5, feet as f32, camera_z as f32 + 0.5],
                        target: [x as f32 + 0.5, water.surface, z as f32 + 0.5],
                        water: [x, y, z],
                    });
                }
            }
        }
    }
    None
}
