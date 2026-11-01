//! Bounded roof-neighborhood sky spill prepared once per coastal chunk mesh.

use std::collections::{BTreeSet, HashMap};

use crate::lighting::{COASTAL_SKY_REACH, lateral_exposure};
use crate::world::{
    Block, CHUNK_SIZE, Chunk, MIN_WORLD_Y, Terrain, WORLD_DEPTH, WORLD_HEIGHT, WorldEdits,
};

use super::SkylightColumn;

#[derive(Default)]
pub(super) struct CoastalLight {
    columns: HashMap<[i32; 2], SkylightColumn>,
    roofs: HashMap<[i32; 2], [i32; 2]>,
    samples: HashMap<[i32; 3], f32>,
}

#[cfg(test)]
mod tests;

impl CoastalLight {
    pub(super) fn prepare(
        chunk: &Chunk,
        terrain: &Terrain,
        edits: Option<&WorldEdits>,
        origin: [i32; 2],
        local_block: impl Fn([i32; 3]) -> Block,
    ) -> Self {
        let mut result = Self::default();
        let mut positions = BTreeSet::new();
        let boundary: Vec<_> = (-1..=CHUNK_SIZE)
            .flat_map(|z| (-1..=CHUNK_SIZE).map(move |x| [x, z]))
            .filter(|&[x, z]| x == -1 || z == -1 || x == CHUNK_SIZE || z == CHUNK_SIZE)
            .filter_map(|[x, z]| Some([origin[0].checked_add(x)?, origin[1].checked_add(z)?]))
            .collect();
        let roofs: HashMap<_, _> = boundary
            .iter()
            .copied()
            .zip(terrain.coastal_roof_batch(&boundary))
            .collect();
        for local_z in -1..=CHUNK_SIZE {
            for local_x in -1..=CHUNK_SIZE {
                let (Some(x), Some(z)) = (
                    origin[0].checked_add(local_x),
                    origin[1].checked_add(local_z),
                ) else {
                    continue;
                };
                let inside =
                    (0..CHUNK_SIZE).contains(&local_x) && (0..CHUNK_SIZE).contains(&local_z);
                let roof = if inside {
                    chunk.coastal_roof(local_x, local_z)
                } else {
                    roofs[&[x, z]]
                };
                if let Some(roof) = roof {
                    result.roofs.insert([x, z], roof);
                    positions.insert([x, z]);
                    for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                        for distance in 1..=COASTAL_SKY_REACH {
                            if let (Some(x), Some(z)) =
                                (x.checked_add(dx * distance), z.checked_add(dz * distance))
                            {
                                positions.insert([x, z]);
                            }
                        }
                    }
                }
            }
        }
        let positions: Vec<_> = positions.into_iter().collect();
        let missing: Vec<_> = positions
            .iter()
            .copied()
            .filter(|&[x, z]| {
                !(0..i64::from(CHUNK_SIZE)).contains(&(i64::from(x) - i64::from(origin[0])))
                    || !(0..i64::from(CHUNK_SIZE)).contains(&(i64::from(z) - i64::from(origin[1])))
            })
            .collect();
        let generated: HashMap<_, _> = missing
            .iter()
            .copied()
            .zip(terrain.column_block_batch(&missing))
            .collect();
        for [x, z] in positions {
            let local_x = i64::from(x) - i64::from(origin[0]);
            let local_z = i64::from(z) - i64::from(origin[1]);
            let mut blocks: [Block; WORLD_DEPTH as usize] = if (0..i64::from(CHUNK_SIZE))
                .contains(&local_x)
                && (0..i64::from(CHUNK_SIZE)).contains(&local_z)
            {
                std::array::from_fn(|index| {
                    local_block([local_x as i32, MIN_WORLD_Y + index as i32, local_z as i32])
                })
            } else {
                generated[&[x, z]]
            };
            if let Some(edits) = edits {
                edits.apply_to_column([x, z], &mut blocks);
            }
            result.columns.insert(
                [x, z],
                SkylightColumn::sample(WORLD_DEPTH, |index| blocks[index as usize]),
            );
        }
        result
    }

    pub(super) fn at(&mut self, cell: [i32; 3], vertical: f32) -> f32 {
        let [x, y, z] = cell;
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y)
            || !self.roofs.get(&[x, z]).is_some_and(|[first, _]| y < *first)
        {
            return vertical;
        }
        let columns = &self.columns;
        let sample = |[x, y, z]: [i32; 3]| {
            columns
                .get(&[x, z])
                .map_or((Block::Stone, 0.0), |column| column.at(y - MIN_WORLD_Y))
        };
        let spill = *self.samples.entry(cell).or_insert_with(|| {
            let mut spill = 0.0_f32;
            let mut transmission = 1.0;
            // A rounded crown can occlude a horizontal ray right at its underside.
            // Open air below it still spills daylight up the same clear column.
            for down in 0..=4 {
                let point = [x, y - down, z];
                if point[1] < MIN_WORLD_Y {
                    break;
                }
                transmission = crate::lighting::daylight_through(transmission, sample(point).0);
                if transmission == 0.0 {
                    break;
                }
                spill = spill.max(
                    lateral_exposure(
                        point,
                        COASTAL_SKY_REACH,
                        |point| sample(point).0,
                        |point| sample(point).1,
                    ) * transmission
                        * (0.48 - down as f32 * 0.04),
                );
            }
            spill
        });
        vertical.max(spill)
    }

    pub(super) fn outdoor(&self, [x, y, z]: [i32; 3], light: f32) -> bool {
        light > 0.0 && self.roofs.get(&[x, z]).is_some_and(|[_, top]| y <= *top)
    }
}
