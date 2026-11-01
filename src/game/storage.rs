//! Sparse saved crates and a single lossless cross-inventory cursor.

use std::collections::BTreeMap;

use super::{Game, GameMode};
use super::{Inventory, ItemStack};
use crate::world::{Block, WorldView};

mod transfer;

/// Fixed slots in one sparse crate, separate from player inventory.
pub const CRATE_SLOTS: usize = 18;
/// Maximum durable container records, independent of terrain residency.
pub const MAX_CRATES: usize = 4096;

/// Sparse durable contents; position is the placed crate's world voxel coordinate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCrate {
    pub position: [i32; 3],
    pub slots: [Option<ItemStack>; CRATE_SLOTS],
}

/// Explicit slot ownership prevents player/crate slot numbers being confused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageSlot {
    Player(usize),
    Crate(usize),
}

/// Borrowed UI state, independent of native events, meshes, and disk.
#[derive(Clone, Copy)]
pub struct StorageView<'a> {
    pub position: [i32; 3],
    pub slots: &'a [Option<ItemStack>; CRATE_SLOTS],
    pub cursor: Option<ItemStack>,
}

#[derive(Clone, Default)]
pub(super) struct Storage {
    crates: BTreeMap<[i32; 3], [Option<ItemStack>; CRATE_SLOTS]>,
    active: Option<[i32; 3]>,
    cursor: Option<ItemStack>,
    source: Option<StorageSlot>,
}

impl Storage {
    pub(super) fn from_saved(crates: Vec<SavedCrate>) -> Self {
        Self {
            crates: crates
                .into_iter()
                .map(|saved| (saved.position, saved.slots))
                .collect(),
            ..Self::default()
        }
    }

    pub(super) fn can_place(&self, position: [i32; 3]) -> bool {
        self.crates.len() < MAX_CRATES && !self.crates.contains_key(&position)
    }

    pub(super) fn placed(&mut self, position: [i32; 3]) {
        self.crates.insert(position, [None; CRATE_SLOTS]);
    }

    pub(super) fn removable(&self, position: [i32; 3]) -> bool {
        self.crates
            .get(&position)
            .is_some_and(|slots| slots.iter().all(Option::is_none))
            && !(self.active == Some(position) && self.cursor.is_some())
    }

    pub(super) fn removed(&mut self, position: [i32; 3]) {
        self.crates.remove(&position);
    }

    pub(super) fn open(&mut self, position: [i32; 3], inventory: &mut Inventory) -> bool {
        self.close(inventory);
        if !self.crates.contains_key(&position) {
            return false;
        }
        inventory.close_cursor();
        self.active = Some(position);
        true
    }

    pub(super) fn view(&self) -> Option<StorageView<'_>> {
        let position = self.active?;
        Some(StorageView {
            position,
            slots: self.crates.get(&position)?,
            cursor: self.cursor,
        })
    }

    pub(super) fn close(&mut self, inventory: &mut Inventory) {
        self.close_cursor(inventory);
        self.active = None;
    }

    pub(super) fn normalized(&self, inventory: &mut Inventory) -> Vec<SavedCrate> {
        let mut storage = self.clone();
        storage.close(inventory);
        storage
            .crates
            .into_iter()
            .map(|(position, slots)| SavedCrate { position, slots })
            .collect()
    }
}

impl Game {
    /// Opens only a reachable, resident, actually placed crate with durable metadata.
    pub fn try_open_storage(&mut self) -> bool {
        if self.mode != GameMode::Survival {
            return false;
        }
        let Some(target) = super::raycast(
            &self.world,
            self.player.eye_position(),
            self.player.look_direction(),
        ) else {
            return false;
        };
        if target.block != Block::StorageCrate {
            return false;
        }
        self.cancel_mining();
        self.feedback.clear();
        self.player.clear_input();
        self.storage.open(target.position, &mut self.inventory)
    }

    pub fn storage_view(&self) -> Option<StorageView<'_>> {
        self.storage.view()
    }

    /// Lossless whole/half/single cursor transfer; stale or evicted storage closes safely.
    pub fn storage_click(&mut self, slot: StorageSlot, single: bool) {
        if let Some(view) = self.storage.view() {
            let [x, y, z] = view.position;
            let visible = super::raycast(
                &self.world,
                self.player.eye_position(),
                self.player.look_direction(),
            )
            .is_some_and(|target| {
                target.position == view.position && target.block == Block::StorageCrate
            });
            if self.world.block_at(x, y, z) != Some(Block::StorageCrate) || !visible {
                self.close_storage();
                return;
            }
        }
        self.storage.click(&mut self.inventory, slot, single);
    }

    pub fn close_storage_cursor(&mut self) {
        self.storage.close_cursor(&mut self.inventory);
    }

    pub fn close_storage(&mut self) {
        self.storage.close(&mut self.inventory);
    }

    /// Restrained failure feedback, not a sound claiming success.
    pub const fn interaction_notice(&self) -> Option<&'static str> {
        self.interaction_notice
    }
}

#[cfg(test)]
mod conservation_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod tests;
