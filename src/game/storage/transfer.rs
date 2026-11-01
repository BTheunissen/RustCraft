//! Bounded conservative cursor operations across two explicit slot owners.

use super::{CRATE_SLOTS, Inventory, ItemStack, Storage, StorageSlot};
use crate::game::INVENTORY_SLOTS;

impl Storage {
    pub(in crate::game) fn click(
        &mut self,
        inventory: &mut Inventory,
        target: StorageSlot,
        single: bool,
    ) {
        if self.active.is_none() || !valid(target) {
            return;
        }
        inventory.close_cursor();
        let Some(mut held) = self.cursor else {
            if let Some(mut stack) = self.get(inventory, target) {
                let count = if single {
                    stack.count.div_ceil(2)
                } else {
                    stack.count
                };
                stack.count -= count;
                self.set(inventory, target, (stack.count > 0).then_some(stack));
                self.cursor = Some(ItemStack { count, ..stack });
                self.source = Some(target);
                if let StorageSlot::Player(index) = target {
                    inventory.external_reservation = Some(index);
                }
            }
            return;
        };
        let destination = self.get(inventory, target);
        match destination {
            None => {
                let count = if single { 1 } else { held.count };
                self.set(inventory, target, Some(ItemStack { count, ..held }));
                held.count -= count;
            }
            Some(mut destination) if destination.item == held.item => {
                let requested = if single { 1 } else { held.count };
                let moved = requested.min(held.item.stack_limit() - destination.count);
                destination.count += moved;
                held.count -= moved;
                self.set(inventory, target, Some(destination));
            }
            Some(destination) if !single => {
                if self
                    .source
                    .is_some_and(|source| self.get(inventory, source).is_some())
                {
                    return;
                }
                self.set(inventory, target, Some(held));
                self.cursor = Some(destination);
                return;
            }
            Some(_) => return,
        }
        self.cursor = (held.count > 0).then_some(held);
        if self.cursor.is_none() {
            self.source = None;
            inventory.external_reservation = None;
        }
    }

    pub(in crate::game) fn close_cursor(&mut self, inventory: &mut Inventory) {
        if let (Some(source), Some(held)) = (self.source, self.cursor) {
            let restored = match self.get(inventory, source) {
                Some(stack) => ItemStack {
                    count: stack.count + held.count,
                    ..stack
                },
                None => held,
            };
            self.set(inventory, source, Some(restored));
        }
        self.source = None;
        self.cursor = None;
        inventory.external_reservation = None;
    }

    fn get(&self, inventory: &Inventory, slot: StorageSlot) -> Option<ItemStack> {
        match slot {
            StorageSlot::Player(index) => inventory.slots[index],
            StorageSlot::Crate(index) => self
                .active
                .and_then(|position| self.crates.get(&position))
                .and_then(|slots| slots[index]),
        }
    }

    fn set(&mut self, inventory: &mut Inventory, slot: StorageSlot, stack: Option<ItemStack>) {
        match slot {
            StorageSlot::Player(index) => inventory.slots[index] = stack,
            StorageSlot::Crate(index) => {
                if let Some(slots) = self
                    .active
                    .and_then(|position| self.crates.get_mut(&position))
                {
                    slots[index] = stack;
                }
            }
        }
    }
}

fn valid(slot: StorageSlot) -> bool {
    match slot {
        StorageSlot::Player(index) => index < INVENTORY_SLOTS,
        StorageSlot::Crate(index) => index < CRATE_SLOTS,
    }
}
