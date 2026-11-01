//! Edit-aware bounded coastal sky sampling, cached at camera-cell boundaries.

use std::collections::BTreeMap;

use crate::game::Game;
use crate::lighting::{COASTAL_SKY_REACH, lateral_exposure, sky_exposure};
use crate::world::{Block, CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, WORLD_DEPTH, WORLD_HEIGHT};

pub(in crate::game) struct CachedExposure {
    cell: [i32; 3],
    revisions: [Option<u64>; 9],
    exposure: Option<f32>,
}

impl Game {
    pub(super) fn coastal_sky(&mut self, cell: [i32; 3]) -> Option<f32> {
        let [x, y, z] = cell;
        let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        let revisions = std::array::from_fn(|index| {
            self.world.chunk_revision(ChunkCoord::new(
                coord.x.checked_add(index as i32 % 3 - 1)?,
                coord.z.checked_add(index as i32 / 3 - 1)?,
            ))
        });
        if let Some(cached) = &self.coastal_exposure
            && cached.cell == cell
            && cached.revisions == revisions
        {
            return cached.exposure;
        }
        let roof = if let Some(chunk) = self.world.chunk(coord) {
            chunk.coastal_roof(x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE))
        } else {
            None
        };
        let exposure = roof
            .filter(|[_, last]| y <= *last)
            .map(|_| self.overhang_exposure(cell));
        self.coastal_exposure = Some(CachedExposure {
            cell,
            revisions,
            exposure,
        });
        exposure
    }

    fn overhang_exposure(&self, cell: [i32; 3]) -> f32 {
        let [x, _, z] = cell;
        let mut positions = vec![[x, z]];
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            for distance in 1..=COASTAL_SKY_REACH {
                if let (Some(x), Some(z)) =
                    (x.checked_add(dx * distance), z.checked_add(dz * distance))
                {
                    positions.push([x, z]);
                }
            }
        }
        let columns: BTreeMap<_, _> = positions
            .into_iter()
            .filter_map(|[x, z]| {
                let chunk = self.world.chunk(ChunkCoord::new(
                    x.div_euclid(CHUNK_SIZE),
                    z.div_euclid(CHUNK_SIZE),
                ))?;
                let mut blocks: [Block; WORLD_DEPTH as usize] = std::array::from_fn(|index| {
                    chunk.block(
                        x.rem_euclid(CHUNK_SIZE),
                        MIN_WORLD_Y + index as i32,
                        z.rem_euclid(CHUNK_SIZE),
                    )
                });
                self.world.edits().apply_to_column([x, z], &mut blocks);
                Some(([x, z], blocks))
            })
            .collect();
        let block = |[x, y, z]: [i32; 3]| {
            if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
                return Block::Air;
            }
            columns
                .get(&[x, z])
                .map_or(Block::Stone, |blocks| blocks[(y - MIN_WORLD_Y) as usize])
        };
        lateral_exposure(cell, COASTAL_SKY_REACH, block, |position| {
            sky_exposure(
                glam::Vec3::from_array(position.map(|value| value as f32 + 0.5)),
                |x, y, z| block([x, y, z]),
            )
        })
    }
}

#[cfg(test)]
mod tests;
