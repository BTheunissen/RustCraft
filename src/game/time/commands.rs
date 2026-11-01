//! Deliberately small time-only debug commands, not a general gameplay console.

use super::WorldTime;

/// Successful debug command feedback. Read-only help/query stays visible in the console.
#[derive(Debug, PartialEq, Eq)]
pub struct TimeCommandResult {
    /// Bounded ASCII feedback for the HUD/terminal.
    pub message: String,
    /// A mutation may close the console so its lighting effect is immediately visible.
    pub changed: bool,
}

pub(crate) fn parse_hour(value: &str) -> Result<f64, &'static str> {
    let hour = match value {
        "dawn" => 6.5,
        "day" => crate::lighting::DEFAULT_TIME_HOUR,
        "noon" => 12.0,
        "sunset" => 17.5,
        "dusk" => 18.0,
        "night" => 22.0,
        "midnight" => 0.0,
        _ => {
            if let Some((hour, minute)) = value.split_once(':') {
                let hour: u32 = hour
                    .parse()
                    .map_err(|_| "use HH:MM, an hour, or a named phase")?;
                let minute: u32 = minute
                    .parse()
                    .map_err(|_| "use HH:MM, an hour, or a named phase")?;
                if hour >= 24 || minute >= 60 {
                    return Err("clock time must be between 00:00 and 23:59");
                }
                f64::from(hour) + f64::from(minute) / 60.0
            } else {
                value
                    .parse()
                    .map_err(|_| "use HH:MM, an hour, or a named phase")?
            }
        }
    };
    if !hour.is_finite() || !(0.0..24.0).contains(&hour) {
        return Err("time must be an hour from 0 (inclusive) to 24 (exclusive)");
    }
    Ok(hour)
}

impl WorldTime {
    /// Executes `/time`, `/time set PHASE|HH:MM|HOUR`, or `/time speed 0..120` atomically.
    pub fn command(&mut self, text: &str) -> Result<TimeCommandResult, &'static str> {
        let mut words = text.trim().trim_start_matches('/').split_whitespace();
        if words.next() != Some("time") {
            return Err("unknown command; use /time help");
        }
        let action = words.next();
        let value = words.next();
        if words.next().is_some() {
            return Err("too many arguments; use /time help");
        }
        let changed = match (action, value) {
            (None | Some("query"), None) => false,
            (Some("help"), None) => {
                return Ok(TimeCommandResult {
                    message: "/time set day|noon|dawn|sunset|night|HH:MM; /time speed 0..120"
                        .into(),
                    changed: false,
                });
            }
            (Some("set"), Some(value)) => {
                self.set_hour(parse_hour(value)?)?;
                true
            }
            (Some("speed"), Some(value)) => {
                self.set_speed(
                    value
                        .parse()
                        .map_err(|_| "time speed must be a number from 0 to 120")?,
                )?;
                true
            }
            _ => return Err("use /time set PHASE|HH:MM or /time speed 0..120"),
        };
        Ok(TimeCommandResult {
            message: format!("World time {}", self.label()),
            changed,
        })
    }
}
