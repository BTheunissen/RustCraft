//! Fixed transient capacity independent of whether presentation consumes events.

use super::{MAX_SOUND_EVENTS, SoundAction, SoundEvent};

pub(super) struct FeedbackQueue {
    events: [Option<SoundEvent>; MAX_SOUND_EVENTS],
    len: usize,
}

impl Default for FeedbackQueue {
    fn default() -> Self {
        Self {
            events: [None; MAX_SOUND_EVENTS],
            len: 0,
        }
    }
}

impl FeedbackQueue {
    pub(super) fn push(&mut self, event: SoundEvent) {
        if !event.position.is_finite() {
            return;
        }
        if event.action == SoundAction::Pickup
            && self.events[..self.len]
                .iter()
                .flatten()
                .any(|previous| previous.action == SoundAction::Pickup)
        {
            return;
        }
        if self.len == MAX_SOUND_EVENTS {
            let priority = self
                .events
                .iter()
                .flatten()
                .map(|event| event.action.priority())
                .min()
                .unwrap_or(0);
            if event.action.priority() < priority {
                return;
            }
            if let Some(victim) = self
                .events
                .iter()
                .position(|event| event.is_some_and(|event| event.action.priority() == priority))
            {
                self.events.copy_within(victim + 1..self.len, victim);
                self.len -= 1;
            }
        }
        self.events[self.len] = Some(event);
        self.len += 1;
    }

    pub(super) fn take(&mut self) -> [Option<SoundEvent>; MAX_SOUND_EVENTS] {
        self.len = 0;
        std::mem::replace(&mut self.events, [None; MAX_SOUND_EVENTS])
    }
}
