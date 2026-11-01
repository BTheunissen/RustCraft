//! Transient, renderer-independent feedback contracts; never part of saved progress.

use glam::Vec3;

use crate::player::{MovementInput, MovementReport};
use crate::world::{Block, WorldView};

mod environment;
mod movement;
mod queue;
pub(super) use environment::Ambience;

/// CPU producer capacity even when no presentation consumer drains feedback.
pub const MAX_SOUND_EVENTS: usize = 64;
use queue::FeedbackQueue;

/// A small acoustic family chosen from actual supporting or edited voxels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundSurface {
    Stone,
    Earth,
    Sand,
    Wood,
}

impl SoundSurface {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Stone => 0,
            Self::Earth => 1,
            Self::Sand => 2,
            Self::Wood => 3,
        }
    }

    /// Liquids and air cannot supply a supported footstep.
    pub const fn from_block(block: Block) -> Option<Self> {
        match block {
            Block::Air | Block::Water | Block::Lava => None,
            Block::Wood | Block::Planks | Block::StorageCrate => Some(Self::Wood),
            Block::Grass | Block::Dirt | Block::Moss | Block::Clay | Block::Leaves => {
                Some(Self::Earth)
            }
            Block::Sand | Block::Snow | Block::Gravel => Some(Self::Sand),
            _ => Some(Self::Stone),
        }
    }
}

/// Explicit gameplay results, rather than a generic application event bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAction {
    Step(SoundSurface),
    Mine(SoundSurface),
    Break(SoundSurface),
    Place(SoundSurface),
    Pickup,
    EnterWater,
    Swim,
}

impl SoundAction {
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::Mine(_) => 0,
            Self::Step(_) | Self::Swim => 1,
            Self::Pickup => 2,
            Self::Break(_) | Self::Place(_) | Self::EnterWater => 3,
        }
    }
}

/// One successful action or distance/transition pulse, in world/block coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundEvent {
    pub action: SoundAction,
    pub position: Vec3,
}

/// Fixed environmental beds; underwater is a listening state, not a fifth source.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoundEnvironment {
    pub wind: f32,
    pub surf: f32,
    pub freshwater: f32,
    pub cave: f32,
    pub underwater: bool,
}

impl SoundEnvironment {
    pub(crate) const fn gains(self) -> [f32; 4] {
        [self.wind, self.surf, self.freshwater, self.cave]
    }
}

#[derive(Default)]
pub(super) struct Feedback {
    queue: FeedbackQueue,
    steps: movement::MovementFeedback,
    mining_cooldown: f64,
}

impl Feedback {
    pub(super) fn movement(
        &mut self,
        report: &MovementReport,
        world: &impl WorldView,
        input: MovementInput,
    ) {
        for step in report.steps.iter().flatten() {
            self.steps.advance(*step, world, input, &mut self.queue);
        }
    }

    pub(super) fn begin_actions(&mut self, dt: f32) {
        self.mining_cooldown = (self.mining_cooldown - f64::from(dt)).max(0.0);
    }

    pub(super) fn mining(&mut self, block: Block, position: Vec3, dt: f32) {
        if dt > 0.0 && self.mining_cooldown <= 0.000_001 {
            self.emit_material(SoundAction::Mine, block, position);
            self.mining_cooldown = 0.20;
        }
    }

    pub(super) fn emit_material(
        &mut self,
        action: impl FnOnce(SoundSurface) -> SoundAction,
        block: Block,
        position: Vec3,
    ) {
        if let Some(surface) = SoundSurface::from_block(block) {
            self.emit(action(surface), position);
        }
    }

    pub(super) fn emit(&mut self, action: SoundAction, position: Vec3) {
        self.queue.push(SoundEvent { action, position });
    }

    pub(super) fn take(&mut self) -> [Option<SoundEvent>; MAX_SOUND_EVENTS] {
        self.queue.take()
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod tests;
