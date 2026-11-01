//! Display-matched frame deadlines with fractional refresh rates.

use std::time::{Duration, Instant};

pub(super) const FALLBACK_REFRESH_MILLIHERTZ: u32 = 60_000;
pub(super) const REFRESH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct FramePacing {
    pub(super) refresh_millihertz: u32,
    next_refresh_check: Instant,
}

impl FramePacing {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            refresh_millihertz: FALLBACK_REFRESH_MILLIHERTZ,
            next_refresh_check: now,
        }
    }

    pub(super) fn interval(&self) -> Duration {
        // Round up, preserving fractional rates without rendering above the display limit.
        Duration::from_nanos(1_000_000_000_000_u64.div_ceil(u64::from(self.refresh_millihertz)))
    }

    pub(super) fn refresh_due(&self, now: Instant) -> bool {
        now >= self.next_refresh_check
    }

    pub(super) fn update(&mut self, refresh_millihertz: Option<u32>, now: Instant) -> bool {
        let refresh = refresh_millihertz
            .filter(|rate| *rate > 0)
            .unwrap_or(FALLBACK_REFRESH_MILLIHERTZ);
        let changed = self.refresh_millihertz != refresh;
        self.refresh_millihertz = refresh;
        self.next_refresh_check = now + REFRESH_CHECK_INTERVAL;
        changed
    }
}

pub(super) fn next_frame_deadline(
    start: Instant,
    completion: Instant,
    rendered: bool,
    interval: Duration,
) -> Instant {
    if rendered {
        start + interval
    } else {
        completion + interval
    }
}
