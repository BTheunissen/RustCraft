//! Item-specific caps and reserved cursor transfers without item deletion.

use super::{Item, ItemStack};
use crate::world::Block;

/// The first nine slots are the selectable hotbar.
pub const HOTBAR_SLOTS: usize = 9;
/// The hotbar and storage together contain thirty-six slots.
pub const INVENTORY_SLOTS: usize = 36;
/// Maximum number of blocks in one inventory or pickup stack.
pub const MAX_STACK: u16 = 64;

/// An empty-by-default inventory with a lossless whole-stack cursor.
///
/// Lifting a stack reserves its empty source slot until the cursor is cleared.
/// Automatic insertion cannot occupy it, so closing always restores the cursor,
/// even after swaps and partial merges while all other slots are full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub(super) slots: [Option<ItemStack>; INVENTORY_SLOTS],
    selected: usize,
    cursor: Option<ItemStack>,
    cursor_source: Option<usize>,
    pub(super) external_reservation: Option<usize>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [None; INVENTORY_SLOTS],
            selected: 0,
            cursor: None,
            cursor_source: None,
            external_reservation: None,
        }
    }
}

impl Inventory {
    pub(super) fn from_saved_slots(
        slots: [Option<ItemStack>; INVENTORY_SLOTS],
        selected: usize,
    ) -> Result<Self, &'static str> {
        if selected >= HOTBAR_SLOTS {
            return Err("selected inventory slot is outside the hotbar");
        }
        if slots.iter().flatten().any(|stack| !stack.is_valid()) {
            return Err("invalid inventory stack");
        }
        Ok(Self {
            slots,
            selected,
            cursor: None,
            cursor_source: None,
            external_reservation: None,
        })
    }

    /// Slots `0..9` are the hotbar; `9..36` are storage.
    pub const fn slots(&self) -> &[Option<ItemStack>; INVENTORY_SLOTS] {
        &self.slots
    }

    /// Selected hotbar index, always in `0..9`.
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Stack available for placement; held cursor items are not available.
    pub const fn selected_stack(&self) -> Option<ItemStack> {
        if let Some(source) = self.external_reservation
            && source == self.selected
        {
            return None;
        }
        if let Some(source) = self.cursor_source
            && source == self.selected
        {
            return None;
        }
        self.slots[self.selected]
    }

    /// Stack currently held by a whole-stack inventory transfer.
    pub const fn cursor_stack(&self) -> Option<ItemStack> {
        self.cursor
    }

    /// Fills matching stacks before empty slots, returning any uninserted count.
    /// This never modifies a held cursor or its reserved source slot.
    /// Air, liquids, and bedrock are not collectible and are returned unchanged.
    pub fn insert(&mut self, item: impl Into<Item>, mut count: u16) -> u16 {
        let item = item.into();
        let limit = item.stack_limit();
        if count == 0 || limit == 0 {
            return count;
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let Some(stack) = slot else {
                continue;
            };
            if stack.item == item
                && self.cursor_source != Some(index)
                && self.external_reservation != Some(index)
            {
                let moved = count.min(limit - stack.count);
                stack.count += moved;
                count -= moved;
            }
            if count == 0 {
                return 0;
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if count == 0 {
                break;
            }
            if slot.is_none()
                && self.cursor_source != Some(index)
                && self.external_reservation != Some(index)
            {
                let moved = count.min(limit);
                *slot = Some(ItemStack { item, count: moved });
                count -= moved;
            }
        }
        count
    }

    /// Selects a hotbar slot; storage and out-of-range indexes are ignored.
    pub fn select(&mut self, index: usize) {
        if index < HOTBAR_SLOTS {
            self.selected = index;
        }
    }

    /// Moves selection by signed slots, wrapping safely even for extreme deltas.
    pub fn cycle(&mut self, delta: i32) {
        self.selected = (self.selected as i32 + delta.rem_euclid(HOTBAR_SLOTS as i32))
            .rem_euclid(HOTBAR_SLOTS as i32) as usize;
    }

    /// Lifts, moves, merges, or safely swaps stacks. Outside clicks do nothing.
    pub fn click_slot(&mut self, index: usize) {
        if index >= INVENTORY_SLOTS || self.external_reservation.is_some() {
            return;
        }
        let Some(mut held) = self.cursor else {
            if let Some(stack) = self.slots[index].take() {
                self.cursor = Some(stack);
                self.cursor_source = Some(index);
            }
            return;
        };
        match self.slots[index] {
            None => {
                self.slots[index] = Some(held);
                self.clear_cursor();
            }
            Some(mut destination) if destination.item == held.item => {
                let moved = held.count.min(held.item.stack_limit() - destination.count);
                destination.count += moved;
                held.count -= moved;
                self.slots[index] = Some(destination);
                if held.count == 0 {
                    self.clear_cursor();
                } else {
                    self.cursor = Some(held);
                }
            }
            Some(destination) => {
                if self
                    .cursor_source
                    .is_some_and(|source| self.slots[source].is_some() && index != source)
                {
                    return;
                }
                if self.cursor_source == Some(index) && destination.item != held.item {
                    return;
                }
                self.slots[index] = Some(held);
                self.cursor = Some(destination);
            }
        }
    }

    /// Returns held items to their reserved source without dropping or deleting them.
    pub fn close_cursor(&mut self) {
        if let Some(source) = self.cursor_source
            && let Some(held) = self.cursor
        {
            if let Some(stack) = &mut self.slots[source] {
                stack.count += held.count;
            } else {
                self.slots[source] = Some(held);
            }
            self.clear_cursor();
        }
    }

    /// Lifts half (rounded up), or places one held item into an empty/matching slot.
    /// A split reservation prevents swaps that could not be restored losslessly.
    pub fn right_click_slot(&mut self, index: usize) {
        if index >= INVENTORY_SLOTS || self.external_reservation.is_some() {
            return;
        }
        let Some(mut held) = self.cursor else {
            if let Some(mut stack) = self.slots[index] {
                let count = stack.count.div_ceil(2);
                stack.count -= count;
                self.slots[index] = (stack.count > 0).then_some(stack);
                self.cursor = Some(ItemStack { count, ..stack });
                self.cursor_source = Some(index);
            }
            return;
        };
        match &mut self.slots[index] {
            None => self.slots[index] = Some(ItemStack { count: 1, ..held }),
            Some(destination)
                if destination.item == held.item && destination.count < held.item.stack_limit() =>
            {
                destination.count += 1
            }
            _ => return,
        }
        held.count -= 1;
        if held.count == 0 {
            self.clear_cursor();
        } else {
            self.cursor = Some(held);
        }
    }

    /// Removes one selected item. Gameplay uses this only after successful block placement.
    pub fn consume_selected(&mut self) -> Option<Item> {
        if self.cursor_source == Some(self.selected)
            || self.external_reservation == Some(self.selected)
        {
            return None;
        }
        let stack = self.slots[self.selected].as_mut()?;
        let item = stack.item;
        stack.count -= 1;
        if stack.count == 0 {
            self.slots[self.selected] = None;
        }
        Some(item)
    }

    fn clear_cursor(&mut self) {
        self.cursor = None;
        self.cursor_source = None;
    }

    pub(super) fn consume_available(&mut self, item: Item, mut count: u16) -> u16 {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if self.cursor_source == Some(index) || self.external_reservation == Some(index) {
                continue;
            }
            let Some(stack) = slot else {
                continue;
            };
            if stack.item != item {
                continue;
            }
            let moved = stack.count.min(count);
            stack.count -= moved;
            count -= moved;
            if stack.count == 0 {
                *slot = None;
            }
            if count == 0 {
                break;
            }
        }
        count
    }
}

/// Stable display names shared by inventory and presentation.
pub const fn block_name(block: Block) -> &'static str {
    match block {
        Block::Air => "Air",
        Block::Grass => "Grass",
        Block::Dirt => "Dirt",
        Block::Stone => "Stone",
        Block::Sand => "Sand",
        Block::Snow => "Snow",
        Block::Wood => "Wood",
        Block::Leaves => "Leaves",
        Block::Water => "Water",
        Block::Bedrock => "Bedrock",
        Block::DeepStone => "Deep Stone",
        Block::CoalOre => "Coal Ore",
        Block::IronOre => "Iron Ore",
        Block::GoldOre => "Gold Ore",
        Block::Crystal => "Crystal",
        Block::Moss => "Moss",
        Block::Lava => "Lava",
        Block::Limestone => "Limestone",
        Block::Slate => "Slate",
        Block::Basalt => "Basalt",
        Block::Calcite => "Calcite",
        Block::Gravel => "Gravel",
        Block::Clay => "Clay",
        Block::Planks => "Planks",
        Block::DressedLimestone => "Dressed Limestone",
        Block::StorageCrate => "Storage Crate",
        Block::Lantern => "Lantern",
    }
}

#[cfg(test)]
mod item_tests;
#[cfg(test)]
mod tests;
