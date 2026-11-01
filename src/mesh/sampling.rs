//! Cached daylight and boundary columns for exposed-face meshing.

use super::{Block, daylight_through};

pub(super) struct SkylightColumn {
    pub(super) blocks: Vec<Block>,
    pub(super) light: Vec<f32>,
}

impl SkylightColumn {
    pub(super) fn sample(height: i32, mut block_at: impl FnMut(i32) -> Block) -> Self {
        let mut blocks = vec![Block::Air; height as usize];
        let mut light = vec![0.0; height as usize];
        let mut daylight = 1.0;
        for y in (0..height).rev() {
            let block = block_at(y);
            daylight = daylight_through(daylight, block);
            blocks[y as usize] = block;
            light[y as usize] = daylight;
        }
        Self { blocks, light }
    }

    pub(super) fn at(&self, y: i32) -> (Block, f32) {
        (self.blocks[y as usize], self.light[y as usize])
    }
}

pub(super) struct BoundarySamples {
    origin: [i32; 3],
    size: [i32; 3],
    sides: [Vec<Option<SkylightColumn>>; 4],
    caps: [Vec<Option<Block>>; 2],
}

impl BoundarySamples {
    pub(super) fn new(origin: [i32; 3], size: [i32; 3]) -> Self {
        Self {
            origin,
            size,
            sides: std::array::from_fn(|side| {
                let count = if side < 2 { size[2] } else { size[0] };
                (0..count).map(|_| None).collect()
            }),
            caps: std::array::from_fn(|_| vec![None; (size[0] * size[2]) as usize]),
        }
    }

    pub(super) fn sample(
        &mut self,
        neighbor: [i32; 3],
        bottom_light: f32,
        outside_block: &mut impl FnMut([i32; 3]) -> Block,
    ) -> (Block, f32) {
        if !(0..self.size[1]).contains(&neighbor[1]) {
            let top = neighbor[1] >= self.size[1];
            let index = (neighbor[2] * self.size[0] + neighbor[0]) as usize;
            let block = *self.caps[usize::from(top)][index]
                .get_or_insert_with(|| world_neighbor(self.origin, neighbor, outside_block));
            // Below the finite volume, continue the bottom column's shadow;
            // treating out-of-world air as sky would backlight the bedrock floor.
            let light = daylight_through(if top { 1.0 } else { bottom_light }, block);
            return (block, light);
        }

        let (side, index) = if neighbor[0] < 0 {
            (0, neighbor[2])
        } else if neighbor[0] >= self.size[0] {
            (1, neighbor[2])
        } else if neighbor[2] < 0 {
            (2, neighbor[0])
        } else {
            (3, neighbor[0])
        };
        let column = self.sides[side][index as usize].get_or_insert_with(|| {
            SkylightColumn::sample(self.size[1], |y| {
                world_neighbor(self.origin, [neighbor[0], y, neighbor[2]], outside_block)
            })
        });
        column.at(neighbor[1])
    }
}

pub(super) fn world_neighbor(
    origin: [i32; 3],
    local: [i32; 3],
    outside_block: &mut impl FnMut([i32; 3]) -> Block,
) -> Block {
    match std::array::from_fn(|axis| origin[axis].checked_add(local[axis])) {
        [Some(x), Some(y), Some(z)] => outside_block([x, y, z]),
        _ => Block::Air,
    }
}
