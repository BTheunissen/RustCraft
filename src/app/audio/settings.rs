//! Small finite gains; persistence and UI stay outside the sample callback.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct AudioSettings {
    pub(in crate::app) muted: bool,
    pub(in crate::app) master: f32,
    pub(in crate::app) actions: f32,
    pub(in crate::app) ambience: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            muted: false,
            master: 0.55,
            actions: 0.70,
            ambience: 0.50,
        }
    }
}

impl AudioSettings {
    pub(super) fn normalized(self) -> Self {
        Self {
            muted: self.muted,
            master: gain(self.master),
            actions: gain(self.actions),
            ambience: gain(self.ambience),
        }
    }
}

pub(super) fn gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_preferences_cannot_reach_output_as_nan_or_unbounded_gain() {
        let settings = AudioSettings {
            muted: true,
            master: f32::NAN,
            actions: f32::INFINITY,
            ambience: -10.0,
        }
        .normalized();
        assert!(settings.muted);
        assert_eq!(
            (settings.master, settings.actions, settings.ambience),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(gain(10.0), 1.0);
    }
}
