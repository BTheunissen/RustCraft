//! Bounded eight-step warm transport through edited voxels, separate from skylight/probes.

use crate::world::{Block, WORLD_DEPTH};

pub(crate) const LANTERN_REACH: i32 = 8;
const MAX_CELLS: usize = 32 * 32 * WORLD_DEPTH as usize;

pub(crate) struct LanternField {
    origin: [i32; 3],
    size: [i32; 3],
    levels: Vec<u8>,
}

impl LanternField {
    pub(crate) fn build(
        origin: [i32; 3],
        size: [i32; 3],
        blocks: &[Block],
    ) -> Result<Self, &'static str> {
        if size.into_iter().any(|length| length <= 0)
            || size[0] > 32
            || size[2] > 32
            || size[1] > WORLD_DEPTH
            || (0..3)
                .any(|axis| i64::from(origin[axis]) + i64::from(size[axis]) > i64::from(i32::MAX))
        {
            return Err("lantern field bounds exceed the fixed light halo");
        }
        let cells = size[0] as usize * size[1] as usize * size[2] as usize;
        if cells > MAX_CELLS || blocks.len() != cells {
            return Err("invalid lantern field occupancy");
        }
        let mut levels = vec![0_u8; cells];
        for (index, block) in blocks.iter().enumerate() {
            if *block == Block::Lantern {
                levels[index] = LANTERN_REACH as u8;
            }
        }
        // Descending integer levels process each cell at most eight times and
        // cannot grow a work queue when many overlapping sources are placed.
        for level in (1..=LANTERN_REACH as u8).rev() {
            for index in 0..cells {
                if levels[index] != level {
                    continue;
                }
                let x = index % size[0] as usize;
                let y = index / size[0] as usize % size[1] as usize;
                let z = index / (size[0] as usize * size[1] as usize);
                for [dx, dy, dz] in [
                    [-1, 0, 0],
                    [1, 0, 0],
                    [0, -1, 0],
                    [0, 1, 0],
                    [0, 0, -1],
                    [0, 0, 1],
                ] {
                    let cell = [x as i32 + dx, y as i32 + dy, z as i32 + dz];
                    if !(0..size[0]).contains(&cell[0])
                        || !(0..size[1]).contains(&cell[1])
                        || !(0..size[2]).contains(&cell[2])
                    {
                        continue;
                    }
                    let neighbor = ((cell[2] * size[1] + cell[1]) * size[0] + cell[0]) as usize;
                    let cost = match blocks[neighbor] {
                        Block::Air => 1,
                        Block::Water => 2,
                        _ => continue,
                    };
                    levels[neighbor] = levels[neighbor].max(level.saturating_sub(cost));
                }
            }
        }
        Ok(Self {
            origin,
            size,
            levels,
        })
    }

    pub(crate) fn sample(&self, world: [i32; 3]) -> [f32; 3] {
        let local = std::array::from_fn::<_, 3, _>(|axis| {
            i64::from(world[axis]) - i64::from(self.origin[axis])
        });
        if (0..3).any(|axis| !(0..i64::from(self.size[axis])).contains(&local[axis])) {
            return [0.0; 3];
        }
        let index = ((local[2] * i64::from(self.size[1]) + local[1]) * i64::from(self.size[0])
            + local[0]) as usize;
        let level = f32::from(self.levels[index]) / LANTERN_REACH as f32;
        let radiance = level * level * 1.8;
        [radiance, radiance * 0.64, radiance * 0.30]
    }
}

#[cfg(test)]
mod tests;
