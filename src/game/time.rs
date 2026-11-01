//! A bounded simulation clock; accelerated solar time never speeds up physics or animation.

mod commands;
pub use commands::TimeCommandResult;
pub(crate) use commands::parse_hour;

/// Real simulation seconds in one ordinary 24-hour day, including night.
pub const DAY_NIGHT_SECONDS: f64 = 20.0 * 60.0;
/// Fastest debug rate: one complete day every ten simulation seconds.
pub const MAX_TIME_SPEED: f64 = 120.0;

/// World-local solar time. Paused applications simply do not advance this clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTime {
    hour: f64,
    speed: f64,
}

impl Default for WorldTime {
    fn default() -> Self {
        Self {
            hour: crate::lighting::DEFAULT_TIME_HOUR,
            speed: 1.0,
        }
    }
}

impl WorldTime {
    /// Solar hour in `0..24`, with sunrise at 06:00 and sunset at 18:00.
    pub const fn hour(self) -> f64 {
        self.hour
    }

    /// Debug multiplier relative to the twenty-minute cycle; zero freezes only solar time.
    pub const fn speed(self) -> f64 {
        self.speed
    }

    /// Advances by a valid simulation step, rejecting invalid deltas and bounding hitches.
    pub fn advance(&mut self, seconds: f32) {
        if seconds.is_finite() && seconds > 0.0 {
            self.hour = (self.hour
                + f64::from(seconds.min(0.25)) * self.speed * 24.0 / DAY_NIGHT_SECONDS)
                .rem_euclid(24.0);
        }
    }

    /// Sets an exact phase without changing the debug rate. Invalid values leave it unchanged.
    pub fn set_hour(&mut self, hour: f64) -> Result<(), &'static str> {
        if !hour.is_finite() || !(0.0..24.0).contains(&hour) {
            return Err("time must be an hour from 0 (inclusive) to 24 (exclusive)");
        }
        self.hour = hour;
        Ok(())
    }

    /// Sets a session-only rate; it is intentionally not part of durable world progress.
    pub fn set_speed(&mut self, speed: f64) -> Result<(), &'static str> {
        if !speed.is_finite() || !(0.0..=MAX_TIME_SPEED).contains(&speed) {
            return Err("time speed must be between 0 and 120; 1 is normal");
        }
        self.speed = speed;
        Ok(())
    }

    /// Convenient debug-key presets, independent of creative movement speed.
    pub fn cycle_speed(&mut self) {
        self.speed = match self.speed {
            1.0 => 10.0,
            10.0 => 60.0,
            60.0 => MAX_TIME_SPEED,
            MAX_TIME_SPEED => 0.0,
            _ => 1.0,
        };
    }

    /// Stable, bounded clock/rate text for both HUD modes and command feedback.
    pub fn label(self) -> String {
        let minutes = (self.hour * 60.0).floor() as u32;
        let rate = if self.speed.fract() == 0.0 {
            format!("{:.0}", self.speed)
        } else {
            format!("{:.2}", self.speed)
        };
        format!("{:02}:{:02} {rate}X", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests;
