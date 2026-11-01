//! Fixed-capacity handoff with expiry and success-priority overflow handling.

use std::time::{Duration, Instant};

use crate::game::{SoundAction, SoundEvent};

pub(super) const EVENT_LIMIT: usize = 64;
pub(super) const EVENT_LIFETIME: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
pub(super) struct PendingEvent {
    pub(super) event: SoundEvent,
    pub(super) created: Instant,
}

pub(super) struct EventQueue {
    events: [Option<PendingEvent>; EVENT_LIMIT],
    len: usize,
}

impl Default for EventQueue {
    fn default() -> Self {
        Self {
            events: [None; EVENT_LIMIT],
            len: 0,
        }
    }
}

impl EventQueue {
    pub(super) fn push(&mut self, pending: PendingEvent) {
        if !pending.event.position.is_finite() {
            return;
        }
        self.expire(pending.created);
        if let Some(previous) = self.events[..self.len]
            .iter_mut()
            .flatten()
            .find(|previous| {
                previous.event.action == pending.event.action
                    && (pending.event.action == SoundAction::Pickup
                        || (pending
                            .event
                            .position
                            .distance_squared(previous.event.position)
                            < 0.25
                            && pending.created.saturating_duration_since(previous.created)
                                < Duration::from_millis(30)))
            })
        {
            *previous = pending;
            return;
        }
        if self.len == EVENT_LIMIT {
            let victim = self
                .events
                .iter()
                .flatten()
                .map(|event| event.event.action.priority())
                .min()
                .unwrap_or(0);
            if pending.event.action.priority() < victim {
                return;
            }
            if let Some(index) = self.events.iter().position(|event| {
                event.is_some_and(|event| event.event.action.priority() == victim)
            }) {
                self.remove(index);
            }
        }
        self.events[self.len] = Some(pending);
        self.len += 1;
    }

    pub(super) fn take(&mut self, now: Instant) -> [Option<PendingEvent>; EVENT_LIMIT] {
        self.expire(now);
        self.len = 0;
        std::mem::replace(&mut self.events, [None; EVENT_LIMIT])
    }

    pub(super) fn clear(&mut self) {
        self.events.fill(None);
        self.len = 0;
    }

    fn expire(&mut self, now: Instant) {
        let mut index = 0;
        while index < self.len {
            if self.events[index]
                .is_some_and(|event| now.saturating_duration_since(event.created) > EVENT_LIFETIME)
            {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn remove(&mut self, index: usize) {
        self.events.copy_within(index + 1..self.len, index);
        self.len -= 1;
        self.events[self.len] = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::SoundSurface;
    use glam::Vec3;

    fn event(action: SoundAction, index: usize, created: Instant) -> PendingEvent {
        PendingEvent {
            event: SoundEvent {
                action,
                position: Vec3::X * index as f32,
            },
            created,
        }
    }

    #[test]
    fn saturation_keeps_successes_and_coalesces_pickup_bursts() {
        let mut queue = EventQueue::default();
        let now = Instant::now();
        for index in 0..EVENT_LIMIT {
            queue.push(event(SoundAction::Step(SoundSurface::Stone), index, now));
        }
        queue.push(event(SoundAction::Break(SoundSurface::Wood), 100, now));
        assert_eq!(queue.len, EVENT_LIMIT);
        let events = queue.take(now);
        assert_eq!(
            events
                .into_iter()
                .flatten()
                .filter(|event| event.event.action == SoundAction::Break(SoundSurface::Wood))
                .count(),
            1
        );
        for index in 0..100 {
            queue.push(event(SoundAction::Pickup, index, now));
        }
        assert_eq!(queue.len, 1);
        queue.clear();
        assert!(queue.take(now).iter().all(Option::is_none));
    }

    #[test]
    fn expired_invalid_and_lower_priority_events_cannot_stall_or_grow_the_queue() {
        let mut queue = EventQueue::default();
        let now = Instant::now();
        for index in 0..EVENT_LIMIT + 10 {
            queue.push(event(SoundAction::Place(SoundSurface::Stone), index, now));
        }
        queue.push(event(SoundAction::Mine(SoundSurface::Stone), 100, now));
        assert!(
            queue
                .take(now)
                .iter()
                .flatten()
                .all(|event| event.event.action.priority() == 3)
        );
        queue.push(PendingEvent {
            event: SoundEvent {
                action: SoundAction::Pickup,
                position: Vec3::NAN,
            },
            created: now,
        });
        queue.push(event(SoundAction::Pickup, 0, now));
        assert!(
            queue
                .take(now + EVENT_LIFETIME + Duration::from_millis(1))
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn nearby_rapid_feedback_coalesces_but_distinct_actions_and_old_pulses_do_not() {
        let now = Instant::now();
        let mut queue = EventQueue::default();
        queue.push(event(SoundAction::Step(SoundSurface::Stone), 0, now));
        queue.push(event(
            SoundAction::Step(SoundSurface::Stone),
            0,
            now + Duration::from_millis(10),
        ));
        assert_eq!(queue.len, 1);
        queue.push(event(
            SoundAction::Step(SoundSurface::Stone),
            0,
            now + Duration::from_millis(50),
        ));
        queue.push(event(
            SoundAction::Break(SoundSurface::Stone),
            0,
            now + Duration::from_millis(50),
        ));
        assert_eq!(queue.len, 3);
    }
}
