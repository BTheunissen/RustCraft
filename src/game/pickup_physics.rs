//! Small-cube gravity and swept solid collision with conservative residency boundaries.

use super::bounds::Bounds;
use super::pickup::{PICKUP_HALF_SIZE, Pickup};
use crate::world::{MIN_WORLD_Y, ResidentWorld, WORLD_HEIGHT, WorldView};

const GRAVITY: f32 = 24.0;
const TERMINAL_SPEED: f32 = 50.0;
const MAX_STEP_DT: f32 = 1.0 / 120.0;

pub(super) fn advance(pickup: &mut Pickup, world: &ResidentWorld, dt: f32) {
    if !pickup.position.is_finite()
        || !pickup.velocity.is_finite()
        || !dt.is_finite()
        || dt <= 0.0
        || !resident_clear(pickup, world)
    {
        return;
    }
    let mut remaining = dt.min(0.25);
    while remaining > 0.0 {
        let step = remaining.min(MAX_STEP_DT);
        let mut position = pickup.position;
        let mut velocity = pickup.velocity;
        velocity.y = (velocity.y - GRAVITY * step).max(-TERMINAL_SPEED);
        for axis in 0..3 {
            let Some((destination, collided)) =
                move_axis(position, axis, velocity[axis] * step, world)
            else {
                return;
            };
            position[axis] = destination;
            if collided {
                velocity[axis] = 0.0;
            }
        }
        pickup.position = position;
        pickup.velocity = velocity;
        pickup.age += step;
        remaining -= step;
    }
}

pub(super) fn resident_clear(pickup: &Pickup, world: &ResidentWorld) -> bool {
    let Some((min, max)) = Bounds::cube(pickup.position, PICKUP_HALF_SIZE).cells() else {
        return false;
    };
    if min[1] < MIN_WORLD_Y || max[1] >= WORLD_HEIGHT {
        return false;
    }
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                if !world
                    .block_at(x, y, z)
                    .is_some_and(|block| !block.is_solid())
                {
                    return false;
                }
            }
        }
    }
    true
}

fn move_axis(
    position: glam::Vec3,
    axis: usize,
    distance: f32,
    world: &ResidentWorld,
) -> Option<(f32, bool)> {
    if distance == 0.0 {
        return Some((position[axis], false));
    }
    let mut destination = position;
    destination[axis] += distance;
    let start = Bounds::cube(position, PICKUP_HALF_SIZE);
    let end = Bounds::cube(destination, PICKUP_HALF_SIZE);
    let bounds = Bounds {
        min: std::array::from_fn(|axis| start.min[axis].min(end.min[axis])),
        max: std::array::from_fn(|axis| start.max[axis].max(end.max[axis])),
    };
    let (min, max) = bounds.cells()?;
    let requested = destination[axis];
    let mut allowed = requested;
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let solid = if (MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
                    world.block_at(x, y, z)?.is_solid()
                } else {
                    true
                };
                if solid {
                    let voxel = f64::from([x, y, z][axis]);
                    let half = f64::from(PICKUP_HALF_SIZE);
                    if distance > 0.0 {
                        allowed = allowed.min((voxel - half) as f32);
                    } else {
                        allowed = allowed.max((voxel + 1.0 + half) as f32);
                    }
                }
            }
        }
    }
    Some((allowed, allowed != requested))
}
