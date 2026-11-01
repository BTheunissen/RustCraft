//! A preloaded original sample bank, with no codecs or per-frame asset I/O.

use crate::game::{SoundAction, SoundSurface};

use super::synthesis;

pub(super) const SOURCE_RATE: u32 = 48_000;
pub(super) const ACTION_CLIPS: usize = 29;
pub(super) const LOOP_COUNT: usize = 4;
pub(super) const CLIP_COUNT: usize = ACTION_CLIPS + LOOP_COUNT;

pub(super) struct SampleBank {
    clips: [Box<[f32]>; CLIP_COUNT],
}

impl SampleBank {
    pub(super) fn generate() -> Self {
        Self {
            clips: std::array::from_fn(synthesis::clip),
        }
    }

    pub(super) fn clip(&self, index: usize) -> &[f32] {
        &self.clips[index]
    }

    pub(super) fn bytes(&self) -> usize {
        self.clips
            .iter()
            .map(|clip| size_of_val(clip.as_ref()))
            .sum()
    }
}

pub(super) fn action_clip(action: SoundAction, variant: usize) -> usize {
    match action {
        SoundAction::Step(surface) => surface.index() * 3 + variant % 3,
        SoundAction::Mine(surface) => 12 + surface.index(),
        SoundAction::Break(surface) => 16 + surface.index(),
        SoundAction::Place(surface) => 20 + surface.index(),
        SoundAction::Pickup => 24,
        SoundAction::EnterWater => 25,
        SoundAction::Swim => 26 + variant % 3,
    }
}

pub(super) const SURFACES: [SoundSurface; 4] = [
    SoundSurface::Stone,
    SoundSurface::Earth,
    SoundSurface::Sand,
    SoundSurface::Wood,
];

#[cfg(test)]
pub(super) fn test_bank() -> std::sync::Arc<SampleBank> {
    static BANK: std::sync::OnceLock<std::sync::Arc<SampleBank>> = std::sync::OnceLock::new();
    std::sync::Arc::clone(BANK.get_or_init(|| std::sync::Arc::new(SampleBank::generate())))
}

#[cfg(test)]
mod tests;
