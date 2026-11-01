//! Survival interaction orchestration over resident voxels only.

use glam::Vec3;

use super::bounds::{Bounds, block_distance_squared};
use super::pickup;
use super::{BlockTarget, Game, GameMode, Inventory, Item, Pickup, Tool, raycast};
use crate::world::{Block, MIN_WORLD_Y, WORLD_HEIGHT, WorldView};

pub(super) struct Mining {
    position: [i32; 3],
    block: Block,
    elapsed: f64,
    tool: Option<Tool>,
    duration: f32,
}

impl Game {
    /// Advances survival interactions separately from movement, once per playing frame.
    /// `place_pressed` is a physical-press pulse, not a held-button state.
    pub fn interact(&mut self, mining_held: bool, place_pressed: bool, dt: f32) {
        self.interact_with_modifier(mining_held, place_pressed, false, dt);
    }

    /// Shift+right-click places against a crate; ordinary right-click opens it.
    pub fn interact_with_modifier(
        &mut self,
        mining_held: bool,
        place_pressed: bool,
        place_against: bool,
        dt: f32,
    ) {
        if self.mode != GameMode::Survival {
            return;
        }
        if self.storage.view().is_some() {
            return;
        }
        self.interaction_notice = None;
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.refresh_target();
        self.feedback.begin_actions(dt);
        // Existing drops advance before mining so a new drop starts at age zero.
        if let Some(position) = pickup::update(
            &mut self.pickups,
            &self.world,
            &mut self.inventory,
            self.player.position,
            dt,
        ) {
            self.feedback.emit(super::SoundAction::Pickup, position);
        }
        self.mine(mining_held, dt);
        if place_pressed {
            if !place_against && self.try_open_storage() {
                return;
            }
            self.place_selected();
        }
    }

    /// Current view target, refreshed even on frames without button actions.
    pub const fn target(&self) -> Option<BlockTarget> {
        self.target
    }

    /// Held mining time normalized to the current material's duration.
    pub fn mining_progress(&self) -> f32 {
        self.mining.as_ref().map_or(0.0, |mining| {
            if mining.tool != self.active_tool() {
                0.0
            } else {
                (mining.elapsed / f64::from(mining.duration)).clamp(0.0, 1.0) as f32
            }
        })
    }

    /// Mouse-release edge handling without disturbing jump or swimming latches.
    pub fn cancel_mining(&mut self) {
        self.mining = None;
    }

    /// Survival inventory; creative sessions leave it unused.
    pub const fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// Inventory mutation clears held mining damage, including selection changes
    /// that switch away and back entirely between simulation updates.
    pub fn inventory_mut(&mut self) -> &mut Inventory {
        self.cancel_mining();
        &mut self.inventory
    }

    /// Resident or suspended dropped stacks retained for this session.
    pub fn pickups(&self) -> &[Pickup] {
        &self.pickups
    }

    /// Places one selected solid on the target face, consuming only a successful edit.
    pub fn place_selected(&mut self) -> bool {
        if self.mode != GameMode::Survival || self.storage.view().is_some() {
            return false;
        }
        self.refresh_target();
        let Some(target) = self.target else {
            return false;
        };
        let Some(stack) = self.inventory.selected_stack() else {
            return false;
        };
        let Some(block) = stack.item.block() else {
            return false;
        };
        if !stack.is_valid() || target.normal == [0; 3] {
            return false;
        }
        let mut position = [0; 3];
        for (axis, coordinate) in position.iter_mut().enumerate() {
            let Some(adjacent) = target.position[axis].checked_add(target.normal[axis]) else {
                return false;
            };
            *coordinate = adjacent;
        }
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&position[1])
            || self.world.block_at(position[0], position[1], position[2]) != Some(Block::Air)
            || Bounds::player(self.player.position).overlaps_block(position)
            || self.pickups.iter().any(|pickup| {
                pickup.count > 0
                    && Bounds::cube(pickup.position, pickup::PICKUP_HALF_SIZE)
                        .overlaps_block(position)
            })
            || block_distance_squared(self.player.eye_position(), position)
                > f64::from(super::INTERACTION_REACH).powi(2)
        {
            return false;
        }
        if block == Block::StorageCrate && !self.storage.can_place(position) {
            self.interaction_notice = Some("CRATE LIMIT REACHED");
            return false;
        }
        if !self.world.edit_block(position, block) {
            return false;
        }
        if block == Block::StorageCrate {
            self.storage.placed(position);
        }
        self.inventory.consume_selected();
        self.feedback.emit_material(
            super::SoundAction::Place,
            block,
            Vec3::from_array(position.map(|coordinate| coordinate as f32)) + Vec3::splat(0.5),
        );
        self.cancel_mining();
        self.refresh_target();
        true
    }

    fn refresh_target(&mut self) {
        self.target = raycast(
            &self.world,
            self.player.eye_position(),
            self.player.look_direction(),
        );
    }

    fn mine(&mut self, held: bool, dt: f32) {
        let tool = self.active_tool();
        let Some((target, duration)) = self.target.filter(|_| held).and_then(|target| {
            mining_duration(target.block).map(|duration| {
                (
                    target,
                    duration / tool.map_or(1.0, |tool| tool.speed_for(target.block)),
                )
            })
        }) else {
            self.cancel_mining();
            return;
        };
        if target.block == Block::StorageCrate && !self.storage.removable(target.position) {
            self.interaction_notice = Some("EMPTY THE CRATE BEFORE MINING");
            self.cancel_mining();
            return;
        }
        if self.mining.as_ref().is_none_or(|mining| {
            mining.position != target.position
                || mining.block != target.block
                || mining.tool != tool
        }) {
            self.mining = Some(Mining {
                position: target.position,
                block: target.block,
                elapsed: 0.0,
                tool,
                duration,
            });
        }
        let Some(mining) = self.mining.as_mut() else {
            return;
        };
        mining.elapsed += f64::from(dt);
        if mining.elapsed + 0.000_001 < f64::from(duration) {
            self.feedback.mining(
                target.block,
                Vec3::from_array(target.position.map(|coordinate| coordinate as f32))
                    + Vec3::splat(0.5),
                dt,
            );
            return;
        }
        if self.world.edit_block(target.position, Block::Air) {
            if target.block == Block::StorageCrate {
                self.storage.removed(target.position);
            }
            self.feedback.emit_material(
                super::SoundAction::Break,
                target.block,
                Vec3::from_array(target.position.map(|coordinate| coordinate as f32))
                    + Vec3::splat(0.5),
            );
            self.pickups.push(Pickup::new(
                Vec3::from_array(target.position.map(|coordinate| coordinate as f32))
                    + Vec3::splat(0.5),
                target.block,
                1,
            ));
        }
        self.cancel_mining();
        self.refresh_target();
    }

    fn active_tool(&self) -> Option<Tool> {
        match self.inventory.selected_stack().map(|stack| stack.item) {
            Some(Item::Tool(tool)) => Some(tool),
            _ => None,
        }
    }
}

pub(super) fn mining_duration(block: Block) -> Option<f32> {
    match block {
        Block::Air | Block::Water | Block::Lava | Block::Bedrock => None,
        Block::Grass
        | Block::Dirt
        | Block::Sand
        | Block::Snow
        | Block::Leaves
        | Block::Moss
        | Block::Gravel
        | Block::Clay => Some(0.6),
        Block::Wood | Block::Planks | Block::StorageCrate => Some(1.5),
        _ => Some(2.5),
    }
}

#[cfg(test)]
mod item_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tool_tests;
