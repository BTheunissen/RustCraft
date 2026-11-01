//! Session-wide dropped items, kept independently of chunk residency.

use glam::Vec3;

use super::pickup_physics;
use super::raycast::segment_clear;
use super::{Inventory, Item};
use crate::player::PLAYER_HEIGHT;
use crate::world::ResidentWorld;

pub(super) const PICKUP_HALF_SIZE: f32 = 0.125;
const COLLECTION_DELAY: f32 = 0.35;
const COLLECTION_RADIUS: f32 = 1.5;
const MERGE_RADIUS: f32 = 0.75;

/// A bounded dropped item. Residency loss suspends it rather than deleting it.
#[derive(Clone, Debug)]
pub struct Pickup {
    /// Center of the physical cube, in world/block coordinates.
    pub position: Vec3,
    pub item: Item,
    pub count: u16,
    pub(super) velocity: Vec3,
    pub(super) age: f32,
}

impl Pickup {
    pub(super) fn new(position: Vec3, item: impl Into<Item>, count: u16) -> Self {
        Self {
            position,
            item: item.into(),
            count,
            velocity: Vec3::Y * 2.0,
            age: 0.0,
        }
    }

    /// Simulated resident time; usable for visual animation without advancing gameplay.
    pub const fn age(&self) -> f32 {
        self.age
    }
}

pub(super) fn update(
    pickups: &mut Vec<Pickup>,
    world: &ResidentWorld,
    inventory: &mut Inventory,
    player_position: Vec3,
    dt: f32,
) -> Option<Vec3> {
    for pickup in pickups.iter_mut() {
        pickup_physics::advance(pickup, world, dt);
    }
    merge(pickups, world);
    let collector = player_position + Vec3::Y * (PLAYER_HEIGHT * 0.5);
    let mut collected = None;
    pickups.retain_mut(|pickup| {
        if pickup.count > 0
            && pickup.age >= COLLECTION_DELAY
            && collector.distance_squared(pickup.position) <= COLLECTION_RADIUS.powi(2)
            && pickup_physics::resident_clear(pickup, world)
            && segment_clear(world, collector, pickup.position)
        {
            let remainder = inventory.insert(pickup.item, pickup.count);
            if remainder < pickup.count {
                collected.get_or_insert(pickup.position);
            }
            pickup.count = remainder;
        }
        pickup.count > 0
    });
    collected
}

fn merge(pickups: &mut [Pickup], world: &ResidentWorld) {
    for index in 0..pickups.len() {
        let (before, after) = pickups.split_at_mut(index + 1);
        let first = &mut before[index];
        if first.count == 0
            || first.count >= first.item.stack_limit()
            || !pickup_physics::resident_clear(first, world)
        {
            continue;
        }
        for second in after {
            if first.item != second.item
                || second.count == 0
                || first.position.distance_squared(second.position) > MERGE_RADIUS.powi(2)
                || !pickup_physics::resident_clear(second, world)
                || !segment_clear(world, first.position, second.position)
            {
                continue;
            }
            let moved = second.count.min(first.item.stack_limit() - first.count);
            first.count += moved;
            second.count -= moved;
            // A new drop must not bypass its collection delay by joining an old one.
            first.age = first.age.min(second.age);
            if first.count == first.item.stack_limit() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests;
