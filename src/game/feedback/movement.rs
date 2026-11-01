//! Cadence from collision-step displacement and real supporting footprints.

use glam::Vec3;

use super::queue::FeedbackQueue;
use super::{SoundAction, SoundEvent, SoundSurface};
use crate::player::{MovementInput, MovementStep, PLAYER_HALF_WIDTH};
use crate::world::WorldView;

#[derive(Default)]
pub(super) struct MovementFeedback {
    distance: f64,
    swim_distance: f64,
    stroke_cooldown: f64,
    wet: Option<bool>,
}

impl MovementFeedback {
    pub(super) fn advance(
        &mut self,
        step: MovementStep,
        world: &impl WorldView,
        input: MovementInput,
        queue: &mut FeedbackQueue,
    ) {
        if !step.position.is_finite()
            || !step.before.is_finite()
            || !step.dt.is_finite()
            || step.dt <= 0.0
        {
            return;
        }
        let wet = self.wet.get_or_insert(step.previous_submersion >= 0.10);
        if !*wet && step.submersion >= 0.10 {
            queue.push(SoundEvent {
                action: SoundAction::EnterWater,
                position: step.position,
            });
            *wet = true;
        } else if step.submersion <= 0.02 {
            *wet = false;
        }
        self.stroke_cooldown = (self.stroke_cooldown - f64::from(step.dt)).max(0.0);
        let displacement = step.position.as_dvec3() - step.before.as_dvec3();
        let swimming = step.submersion.max(step.previous_submersion) >= 0.40;
        if swimming {
            self.distance = 0.0;
            let stroking = input.forward != input.backward
                || input.left != input.right
                || input.jump != input.descend;
            let moved = displacement.length();
            if stroking {
                self.swim_distance = (self.swim_distance + moved).min(1.25);
            }
            if stroking
                && moved > 0.000_001
                && self.swim_distance + 0.0001 >= 1.25
                && self.stroke_cooldown == 0.0
            {
                queue.push(SoundEvent {
                    action: SoundAction::Swim,
                    position: step.position,
                });
                self.swim_distance = (self.swim_distance - 1.25).max(0.0);
                self.stroke_cooldown = 1.0 / 3.0;
            }
            return;
        }
        self.swim_distance = 0.0;
        if !step.supported {
            return;
        }
        self.distance += displacement.x.hypot(displacement.z);
        if self.distance + 0.0001 < 2.0 {
            return;
        }
        let Some(surface) = supporting_surface(world, step.position) else {
            self.distance = 0.0;
            return;
        };
        while self.distance + 0.0001 >= 2.0 {
            queue.push(SoundEvent {
                action: SoundAction::Step(surface),
                position: step.position,
            });
            self.distance = (self.distance - 2.0).max(0.0);
        }
    }
}

pub(super) fn supporting_surface(world: &impl WorldView, position: Vec3) -> Option<SoundSurface> {
    if !position.is_finite()
        || position
            .to_array()
            .into_iter()
            .any(|value| f64::from(value).abs() > f64::from(i32::MAX) - 2.0)
    {
        return None;
    }
    let half = f64::from(PLAYER_HALF_WIDTH);
    let x = f64::from(position.x);
    let z = f64::from(position.z);
    let first_x = (x - half + 0.00001).floor() as i32;
    let last_x = (x + half - 0.00001).floor() as i32;
    let first_z = (z - half + 0.00001).floor() as i32;
    let last_z = (z + half - 0.00001).floor() as i32;
    let y = (f64::from(position.y) - 0.001).floor() as i32;
    let mut area = [0.0_f64; 4];
    for cell_x in first_x..=last_x {
        for cell_z in first_z..=last_z {
            let Some(surface) = world
                .block_at(cell_x, y, cell_z)
                .and_then(SoundSurface::from_block)
            else {
                continue;
            };
            let width = (x + half).min(f64::from(cell_x) + 1.0) - (x - half).max(f64::from(cell_x));
            let depth = (z + half).min(f64::from(cell_z) + 1.0) - (z - half).max(f64::from(cell_z));
            area[surface.index()] += width * depth;
        }
    }
    let surfaces = [
        SoundSurface::Stone,
        SoundSurface::Earth,
        SoundSurface::Sand,
        SoundSurface::Wood,
    ];
    (0..4)
        .filter(|&index| area[index] > 0.0)
        .max_by(|&first, &second| area[first].total_cmp(&area[second]))
        .map(|index| surfaces[index])
}
