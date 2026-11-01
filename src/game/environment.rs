//! Camera environment sampling without any window or GPU dependency.

use super::Game;
use crate::lighting::atmosphere_exposure;
use crate::world::{Block, MIN_WORLD_Y, SampledWorld, WaterBiome, WaterSample, WorldView};

pub(super) mod overhang;

/// World-derived frame information; presentation decides how to draw it.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    /// Active terrestrial, aquatic, or underground biome label.
    pub biome: &'static str,
    /// Camera lies below the local terrain cap or beneath an added solid roof.
    pub underground: bool,
    /// Camera lies within actual water below its fractional mean surface.
    pub underwater: bool,
    /// Camera-fluid metadata, absent in air even above a basin.
    pub water: Option<WaterSample>,
    /// Occluded daylight, normalized to `0..=1`.
    pub sky_exposure: f32,
    /// Camera lies in non-damaging lava scenery.
    pub in_lava: bool,
}

impl Game {
    /// Samples the camera's actual material and mean waterline, preferring CPU residency.
    pub fn environment(&mut self) -> Environment {
        let eye = self.player.eye_position();
        let [x, y, z] = eye.floor().as_ivec3().to_array();
        let column = self.terrain.column(x, z);
        let revision = self.world.edits().revision();
        if self.exposure_revision != revision {
            self.exposure.clear();
            self.outdoor_roof = None;
            self.coastal_exposure = None;
            self.exposure_revision = revision;
        }
        let below_terrain = eye.y < column.height as f32 + 1.0;
        let coastal = self.coastal_sky([x, y, z]);
        let added_roof = self.added_roof([x, y, z]);
        let underground =
            below_terrain || added_roof || coastal.is_some_and(|exposure| exposure < 0.05);
        let world = SampledWorld::new(&self.world, &self.terrain);
        let exposure = if let Some(coastal) = coastal.filter(|_| !below_terrain && !added_roof) {
            coastal
        } else if underground {
            self.exposure
                .sample(eye, |x, y, z| world.block_at(x, y, z).unwrap_or(Block::Air))
        } else {
            1.0
        };
        let surface_water = self
            .world
            .water_at(x, (column.height + 1).max(MIN_WORLD_Y), z)
            .or_else(|| self.terrain.water_column(x, z))
            .filter(|water| water.biome != WaterBiome::CavePool);
        let biome = if below_terrain {
            self.terrain.cave_biome(x, y, z).name()
        } else if let Some(water) = surface_water {
            water.biome.name()
        } else {
            column.biome.name()
        };
        let water = world.water_at(x, y, z);
        Environment {
            biome,
            underground,
            underwater: camera_is_underwater(water, eye.y),
            water,
            sky_exposure: atmosphere_exposure(underground, exposure),
            in_lava: world.block_at(x, y, z) == Some(Block::Lava),
        }
    }

    fn added_roof(&mut self, cell: [i32; 3]) -> bool {
        if let Some((previous, covered)) = self.outdoor_roof
            && previous == cell
        {
            return covered;
        }
        let [x, first_y, z] = cell;
        let covered = (first_y.max(MIN_WORLD_Y)..crate::world::WORLD_HEIGHT).any(|y| {
            self.world.edits().block_at(x, y, z).is_some_and(|block| {
                block.is_solid()
                    && block != Block::Leaves
                    && block != self.terrain.block_at(x, y, z)
            })
        });
        self.outdoor_roof = Some((cell, covered));
        covered
    }
}

pub(super) fn camera_is_underwater(water: Option<WaterSample>, eye_y: f32) -> bool {
    eye_y.is_finite()
        && water.is_some_and(|water| {
            water.surface.is_finite()
                && eye_y < water.surface
                && eye_y >= (i64::from(water.bottom) + 1) as f32
        })
}
