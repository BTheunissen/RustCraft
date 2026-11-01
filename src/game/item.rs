//! Stable item identity, distinct from voxel materials and atlas tiles.

use super::inventory::{MAX_STACK, block_name};
use crate::world::Block;

/// Permanent convenience tools; IDs are independent of voxel IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Tool {
    StonePickaxe = 0,
    StoneAxe = 1,
}

impl Tool {
    /// Stable tool-ID order used by explicit save tags.
    pub const ALL: [Self; 2] = [Self::StonePickaxe, Self::StoneAxe];

    /// Convenience only: all ordinary mineable blocks remain gatherable by hand.
    pub const fn speed_for(self, block: Block) -> f32 {
        match (self, block) {
            (
                Self::StonePickaxe,
                Block::Stone
                | Block::DeepStone
                | Block::Limestone
                | Block::Slate
                | Block::Basalt
                | Block::Calcite
                | Block::CoalOre
                | Block::IronOre
                | Block::GoldOre
                | Block::Crystal
                | Block::DressedLimestone,
            )
            | (Self::StoneAxe, Block::Wood | Block::Planks | Block::StorageCrate) => 3.0,
            _ => 1.0,
        }
    }
}

/// A placeable block or a non-placeable tool, never an atlas index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Block(Block),
    Tool(Tool),
}

impl Item {
    /// Placeable voxel identity only; tools never enter block-placement paths.
    pub const fn block(self) -> Option<Block> {
        match self {
            Self::Block(block) => Some(block),
            Self::Tool(_) => None,
        }
    }

    /// Invalid scenery materials have no inventory capacity.
    pub const fn stack_limit(self) -> u16 {
        match self {
            Self::Block(Block::Air | Block::Water | Block::Lava | Block::Bedrock) => 0,
            Self::Block(_) => MAX_STACK,
            Self::Tool(_) => 1,
        }
    }

    /// Shared original UI name, independent of render assets.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Block(block) => block_name(block),
            Self::Tool(Tool::StonePickaxe) => "Stone Pickaxe",
            Self::Tool(Tool::StoneAxe) => "Stone Axe",
        }
    }
}

impl From<Block> for Item {
    fn from(block: Block) -> Self {
        Self::Block(block)
    }
}

impl From<Tool> for Item {
    fn from(tool: Tool) -> Self {
        Self::Tool(tool)
    }
}

/// An item and its nonzero count, bounded by its own stack limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: Item,
    pub count: u16,
}

impl ItemStack {
    pub const fn is_valid(self) -> bool {
        self.count > 0 && self.count <= self.item.stack_limit()
    }
}
