//! Version-independent CPU progress snapshots; file encoding belongs to `save`.

use std::collections::HashSet;

use glam::Vec3;

use super::{
    Game, GameMode, INVENTORY_SLOTS, Inventory, Item, ItemStack, MAX_CRATES, Pickup, SavedCrate,
};
use crate::player::{Player, SpeedMultiplier};
use crate::world::{Block, BlockEdit, MIN_WORLD_Y, ResidentWorld, WORLD_HEIGHT, WorldEdits};

pub(crate) const MAX_SAVED_EDITS: usize = 1_000_000;
pub(crate) const MAX_SAVED_PICKUPS: usize = 100_000;

/// Feet-centered pose. Movement impulses and physical input are reset on resume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedPlayer {
    /// Center of the feet in world/block coordinates, not the camera eye.
    pub position: Vec3,
    /// Heading in radians, with yaw zero looking toward -Z.
    pub yaw: f32,
    /// View angle in radians, bounded to +/-89 degrees.
    pub pitch: f32,
    /// Creative flight persists so resuming inside rock does not force walking.
    pub flying: bool,
}

/// A dropped stack, including its collection delay and falling motion.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedPickup {
    /// Center of the physical dropped cube in world/block coordinates.
    pub position: Vec3,
    pub item: Item,
    pub count: u16,
    /// Falling motion resumes only after the surrounding chunks are resident.
    pub velocity: Vec3,
    /// Simulated resident time, including the pre-collection delay.
    pub age: f32,
}

/// Durable progress only: no resident chunks, meshes, mining damage, or UI state.
#[derive(Clone, Debug, PartialEq)]
pub struct GameSnapshot {
    /// Seed for the compatible terrain generator; dense chunks are regenerated.
    pub seed: u64,
    pub mode: GameMode,
    pub player: SavedPlayer,
    pub headlamp: bool,
    pub speed: SpeedMultiplier,
    /// Persisted solar hour. Debug acceleration is session-only and resumes at 1x.
    pub time_of_day: f64,
    /// Sparse overrides in integer world coordinates, including unloaded chunks.
    pub edits: Vec<BlockEdit>,
    /// The original slot order: nine hotbar slots, then twenty-seven storage slots.
    pub slots: [Option<ItemStack>; INVENTORY_SLOTS],
    pub selected_slot: usize,
    /// Resident and dormant dropped stacks; they do not expire during loading.
    pub pickups: Vec<SavedPickup>,
    pub crates: Vec<SavedCrate>,
}

impl GameSnapshot {
    /// Rejects malformed progress before it reaches generation, physics, or inventory.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.time_of_day.is_finite() || !(0.0..24.0).contains(&self.time_of_day) {
            return Err("invalid saved solar time");
        }
        if !valid_position(self.player.position)
            || !self.player.yaw.is_finite()
            || !self.player.pitch.is_finite()
            || self.player.pitch.abs() > 89.0 * std::f32::consts::PI / 180.0
        {
            return Err("invalid saved player pose");
        }
        if self.mode == GameMode::Survival
            && (self.player.flying || self.speed != SpeedMultiplier::Normal)
        {
            return Err("survival save contains creative movement settings");
        }
        Inventory::from_saved_slots(self.slots, self.selected_slot)?;
        if self.edits.len() > MAX_SAVED_EDITS || self.pickups.len() > MAX_SAVED_PICKUPS {
            return Err("world progress exceeds the local save size limit");
        }
        if self.crates.len() > MAX_CRATES {
            return Err("crate count exceeds the local save size limit");
        }
        let mut positions = HashSet::with_capacity(self.edits.len());
        for edit in &self.edits {
            if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&edit.position[1])
                || edit.position.iter().any(|&coordinate| {
                    !(i64::from(i32::MIN) + 512..=i64::from(i32::MAX) - 512)
                        .contains(&i64::from(coordinate))
                })
                || (edit.block != Block::Air
                    && (!edit.block.is_solid() || edit.block == Block::Bedrock))
            {
                return Err("invalid saved block edit");
            }
            if !positions.insert(edit.position) {
                return Err("duplicate saved block edit coordinate");
            }
        }
        for pickup in &self.pickups {
            if !valid_position(pickup.position)
                || !(MIN_WORLD_Y as f32..WORLD_HEIGHT as f32).contains(&pickup.position.y)
                || !pickup.velocity.is_finite()
                || pickup.velocity.abs().max_element() > 64.0
                || !pickup.age.is_finite()
                || pickup.age < 0.0
                || !(ItemStack {
                    item: pickup.item,
                    count: pickup.count,
                })
                .is_valid()
            {
                return Err("invalid saved dropped item");
            }
        }
        let crate_edits: HashSet<_> = self
            .edits
            .iter()
            .filter(|edit| edit.block == Block::StorageCrate)
            .map(|edit| edit.position)
            .collect();
        let mut crates = HashSet::new();
        for saved in &self.crates {
            if self.mode != GameMode::Survival {
                return Err("creative progress contains survival storage");
            }
            if !crate_edits.contains(&saved.position)
                || !crates.insert(saved.position)
                || saved.slots.iter().flatten().any(|stack| !stack.is_valid())
            {
                return Err("invalid, duplicate, or orphan saved crate");
            }
        }
        if crates != crate_edits {
            return Err("placed crate is missing its saved contents record");
        }
        Ok(())
    }
}

impl Game {
    /// Captures progress without disturbing a live inventory transfer. Cursor-held
    /// items return to their reserved source in the snapshot, never disappearing.
    pub fn snapshot(&self) -> GameSnapshot {
        let mut inventory = self.inventory.clone();
        inventory.close_cursor();
        let crates = self.storage.normalized(&mut inventory);
        GameSnapshot {
            seed: self.terrain.seed(),
            mode: self.mode,
            player: SavedPlayer {
                position: self.player.position,
                yaw: self.player.yaw,
                pitch: self.player.pitch,
                flying: self.player.flying,
            },
            headlamp: self.headlamp,
            speed: self.speed,
            time_of_day: self.time.hour(),
            edits: self.world.edits().entries(),
            slots: *inventory.slots(),
            selected_slot: inventory.selected(),
            pickups: self
                .pickups
                .iter()
                .map(|pickup| SavedPickup {
                    position: pickup.position,
                    item: pickup.item,
                    count: pickup.count,
                    velocity: pickup.velocity,
                    age: pickup.age,
                })
                .collect(),
            crates,
        }
    }

    /// Restores progress into empty CPU residency, ready for edit-aware streaming.
    pub fn from_snapshot(snapshot: GameSnapshot) -> Result<Self, &'static str> {
        snapshot.validate()?;
        let mut game = Self::with_mode(snapshot.seed, snapshot.mode);
        game.player = Player::new(snapshot.player.position);
        game.player.yaw = snapshot.player.yaw;
        game.player.pitch = snapshot.player.pitch;
        game.player.flying = snapshot.player.flying;
        game.headlamp = snapshot.headlamp;
        game.speed = snapshot.speed;
        game.time.set_hour(snapshot.time_of_day)?;
        game.world = ResidentWorld::with_edits(WorldEdits::from_entries(snapshot.edits)?);
        game.inventory = Inventory::from_saved_slots(snapshot.slots, snapshot.selected_slot)?;
        game.storage = super::storage::Storage::from_saved(snapshot.crates);
        game.pickups = snapshot
            .pickups
            .into_iter()
            .map(|pickup| Pickup {
                position: pickup.position,
                item: pickup.item,
                count: pickup.count,
                velocity: pickup.velocity,
                age: pickup.age,
            })
            .collect();
        Ok(game)
    }
}

fn valid_position(position: Vec3) -> bool {
    // Leave room for streaming halos and body/eye coordinates before i32 conversion.
    position.is_finite()
        && position.to_array().into_iter().all(|coordinate| {
            (f64::from(i32::MIN) + 512.0..=f64::from(i32::MAX) - 512.0)
                .contains(&f64::from(coordinate))
        })
}

#[cfg(test)]
mod tests;
