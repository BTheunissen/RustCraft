//! Small declarative recipes with copied, all-or-nothing inventory transactions.

use super::{Inventory, Item, ItemStack, Tool};
use crate::world::Block;

/// Recipe IDs are stable within gameplay, not voxel or atlas IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RecipeId {
    Planks = 0,
    DressedLimestone = 1,
    StonePickaxe = 2,
    StoneAxe = 3,
    StorageCrate = 4,
    Lantern = 5,
}

/// One craft's exact ingredients and output; only delivered recipes are listed.
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: &'static str,
    pub inputs: &'static [ItemStack],
    pub output: ItemStack,
}

const PLANKS: [ItemStack; 1] = [ItemStack {
    item: Item::Block(Block::Wood),
    count: 1,
}];
const DRESSED: [ItemStack; 1] = [ItemStack {
    item: Item::Block(Block::Limestone),
    count: 1,
}];
const PICKAXE: [ItemStack; 2] = [
    ItemStack {
        item: Item::Block(Block::Planks),
        count: 4,
    },
    ItemStack {
        item: Item::Block(Block::Stone),
        count: 3,
    },
];
const AXE: [ItemStack; 2] = [
    ItemStack {
        item: Item::Block(Block::Planks),
        count: 4,
    },
    ItemStack {
        item: Item::Block(Block::Stone),
        count: 2,
    },
];
const CRATE: [ItemStack; 1] = [ItemStack {
    item: Item::Block(Block::Planks),
    count: 8,
}];
const LANTERN: [ItemStack; 2] = [
    ItemStack {
        item: Item::Block(Block::Planks),
        count: 2,
    },
    ItemStack {
        item: Item::Block(Block::Crystal),
        count: 1,
    },
];
pub const RECIPES: [Recipe; 6] = [
    Recipe {
        id: RecipeId::Planks,
        name: "Planks",
        inputs: &PLANKS,
        output: ItemStack {
            item: Item::Block(Block::Planks),
            count: 4,
        },
    },
    Recipe {
        id: RecipeId::DressedLimestone,
        name: "Dressed Limestone",
        inputs: &DRESSED,
        output: ItemStack {
            item: Item::Block(Block::DressedLimestone),
            count: 1,
        },
    },
    Recipe {
        id: RecipeId::StonePickaxe,
        name: "Stone Pickaxe",
        inputs: &PICKAXE,
        output: ItemStack {
            item: Item::Tool(Tool::StonePickaxe),
            count: 1,
        },
    },
    Recipe {
        id: RecipeId::StoneAxe,
        name: "Stone Axe",
        inputs: &AXE,
        output: ItemStack {
            item: Item::Tool(Tool::StoneAxe),
            count: 1,
        },
    },
    Recipe {
        id: RecipeId::StorageCrate,
        name: "Storage Crate",
        inputs: &CRATE,
        output: ItemStack {
            item: Item::Block(Block::StorageCrate),
            count: 1,
        },
    },
    Recipe {
        id: RecipeId::Lantern,
        name: "Lantern",
        inputs: &LANTERN,
        output: ItemStack {
            item: Item::Block(Block::Lantern),
            count: 1,
        },
    },
];

/// A failed craft is a no-op, never a dropped overflow stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    UnknownRecipe,
    MissingIngredients,
    NoCapacity,
}

impl RecipeId {
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl Inventory {
    /// Plans capacity after consumption, excluding cursor-held/reserved quantities.
    pub fn can_craft(&self, index: usize) -> Result<(), CraftError> {
        self.craft_transaction(index).map(|_| ())
    }

    /// Commits one complete craft only after every cost and output fits.
    pub fn craft(&mut self, index: usize) -> Result<(), CraftError> {
        *self = self.craft_transaction(index)?;
        Ok(())
    }

    fn craft_transaction(&self, index: usize) -> Result<Self, CraftError> {
        let recipe = RECIPES.get(index).ok_or(CraftError::UnknownRecipe)?;
        let mut transaction = self.clone();
        for ingredient in recipe.inputs {
            if transaction.consume_available(ingredient.item, ingredient.count) != 0 {
                return Err(CraftError::MissingIngredients);
            }
        }
        if transaction.insert(recipe.output.item, recipe.output.count) != 0 {
            return Err(CraftError::NoCapacity);
        }
        Ok(transaction)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tool_tests;
