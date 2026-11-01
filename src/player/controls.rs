//! Renderer- and keyboard-independent movement intent.

/// Persistent exploration speed selected by the application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SpeedMultiplier {
    /// Ordinary exploration speed.
    #[default]
    Normal = 1,
    /// Twice normal speed.
    Double = 2,
    /// Four times normal speed.
    Quadruple = 4,
    /// Eight times normal speed.
    Octuple = 8,
    /// Sixteen times normal speed.
    Sixteen = 16,
}

impl SpeedMultiplier {
    /// Returns the multiplier applied to deliberate movement, not gravity or currents.
    pub const fn factor(self) -> f32 {
        self as u8 as f32
    }

    /// Cycles all supported speeds, including a return to normal speed.
    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::Double,
            Self::Double => Self::Quadruple,
            Self::Quadruple => Self::Octuple,
            Self::Octuple => Self::Sixteen,
            Self::Sixteen => Self::Normal,
        }
    }
}

/// Logical controls for one movement update, independent of keyboard bindings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovementInput {
    /// Move along the horizontal facing direction.
    pub forward: bool,
    /// Move opposite the horizontal facing direction.
    pub backward: bool,
    /// Strafe left relative to yaw.
    pub left: bool,
    /// Strafe right relative to yaw.
    pub right: bool,
    /// Held flight/swimming ascent; a false-to-true transition requests a walking jump.
    pub jump: bool,
    /// Held flight/swimming descent; ignored when walking on dry ground.
    pub descend: bool,
    /// Selected speed for walking, three-axis flight, and swimming strokes.
    pub speed: SpeedMultiplier,
}

pub(super) fn input_axis(positive: bool, negative: bool) -> f32 {
    f32::from(u8::from(positive)) - f32::from(u8::from(negative))
}
