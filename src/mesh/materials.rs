//! Material-to-atlas mapping shared with alternative presentation paths.

use super::Block;

pub(crate) fn material_for_face(block: Block, vertical_normal: i32) -> u32 {
    match block {
        Block::Air => 0,
        Block::Grass => match vertical_normal {
            1 => 0,
            -1 => 2,
            _ => 1,
        },
        Block::Dirt => 2,
        Block::Stone => 3,
        Block::Sand => 4,
        Block::Snow => 5,
        Block::Wood => {
            if vertical_normal == 0 {
                6
            } else {
                7
            }
        }
        Block::Leaves => 8,
        Block::Water => 9,
        Block::Bedrock => 10,
        Block::DeepStone => 11,
        Block::CoalOre => 12,
        Block::IronOre => 13,
        Block::GoldOre => 14,
        Block::Crystal => 15,
        Block::Moss => 16,
        Block::Lava => 17,
        Block::Limestone => 18,
        Block::Slate => 19,
        Block::Basalt => 20,
        Block::Calcite => 21,
        Block::Gravel => 22,
        Block::Clay => 23,
        Block::Planks => 24,
        Block::DressedLimestone => 25,
        Block::StorageCrate => 26,
        Block::Lantern => 27,
    }
}
