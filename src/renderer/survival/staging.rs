//! CPU frame staging and revision-aware resident daylight, without per-frame allocation.

use super::geometry::{
    BlockInstance, HAND_INSTANCES, MAX_PICKUPS, SurvivalUniform, hand_instances, mining_parameters,
    pickup_instance, target_is_current, target_model,
};
use crate::camera::view_projection;
use crate::game::Item;
use crate::lighting::sky_exposure;
use crate::renderer::FrameState;
use crate::world::{Block, ChunkCoord, WorldView};
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};

struct PickupLight {
    cell: [i32; 3],
    revision: u64,
    light: f32,
}

pub(super) struct SurvivalGeometry {
    pub(super) pickups: Vec<BlockInstance>,
    pub(super) hand: [BlockInstance; HAND_INSTANCES],
    pub(super) hand_count: u32,
    pub(super) has_target: bool,
    lights: Vec<PickupLight>,
}

impl Default for SurvivalGeometry {
    fn default() -> Self {
        Self {
            pickups: Vec::with_capacity(MAX_PICKUPS),
            hand: [BlockInstance::zeroed(); HAND_INSTANCES],
            hand_count: 0,
            has_target: false,
            lights: Vec::with_capacity(MAX_PICKUPS),
        }
    }
}

impl SurvivalGeometry {
    pub(super) fn stage(&mut self, frame: &FrameState<'_>, aspect: f32) -> SurvivalUniform {
        self.pickups.clear();
        let mut light_slots = 0;
        self.hand_count = 0;
        self.has_target = false;
        let mut uniform = SurvivalUniform {
            hand_projection: view_projection(Vec3::ZERO, Vec3::NEG_Z, aspect, 8.0)
                .to_cols_array_2d(),
            target_model: Mat4::IDENTITY.to_cols_array_2d(),
            mining: mining_parameters(0.0),
        };
        if let Some(survival) = frame.survival {
            for pickup in survival.pickups {
                let instances = if matches!(pickup.item, Item::Tool(_)) {
                    3
                } else {
                    1
                };
                if self.pickups.len() + instances > MAX_PICKUPS {
                    break;
                }
                if pickup.count == 0
                    || pickup.count > pickup.item.stack_limit()
                    || !pickup.position.is_finite()
                {
                    continue;
                }
                let coord = ChunkCoord::from_world(pickup.position.x, pickup.position.z);
                let Some(revision) = frame.world.chunk_revision(coord) else {
                    continue;
                };
                let cell = pickup.position.floor().as_ivec3().to_array();
                let slot = light_slots;
                light_slots += 1;
                let cached = self
                    .lights
                    .get(slot)
                    .filter(|cached| cached.cell == cell && cached.revision == revision);
                let light = if let Some(cached) = cached {
                    cached.light
                } else {
                    let light = sky_exposure(pickup.position, |x, y, z| {
                        frame.world.block_at(x, y, z).unwrap_or(Block::Stone)
                    });
                    let cached = PickupLight {
                        cell,
                        revision,
                        light,
                    };
                    if slot < self.lights.len() {
                        self.lights[slot] = cached;
                    } else {
                        self.lights.push(cached);
                    }
                    light
                };
                match pickup.item {
                    Item::Block(block) => self.pickups.push(pickup_instance(
                        pickup.position,
                        block,
                        pickup.age(),
                        frame.elapsed,
                        light,
                    )),
                    Item::Tool(tool) => self.pickups.extend(super::tools::pickup(
                        tool,
                        pickup.position,
                        pickup.age(),
                        frame.elapsed,
                        light,
                    )),
                }
            }
            if let Some(target) = survival.target.filter(|target| {
                target_is_current(*target, |x, y, z| frame.world.block_at(x, y, z))
            }) {
                uniform.target_model = target_model(target.position).to_cols_array_2d();
                uniform.mining = mining_parameters(survival.mining_progress);
                self.has_target = true;
            }
            if survival.show_hand {
                (self.hand, self.hand_count) = hand_instances(
                    survival.held,
                    aspect,
                    frame.elapsed,
                    survival.mining_progress,
                );
            }
        }
        self.lights.truncate(light_slots);
        uniform
    }
}

#[cfg(test)]
mod tests;
