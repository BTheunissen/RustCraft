//! Bounded CPU movement results; consumers cannot change the controller through these.

use glam::Vec3;

#[derive(Clone, Copy)]
pub(crate) struct MovementStep {
    pub(crate) before: Vec3,
    pub(crate) position: Vec3,
    pub(crate) dt: f32,
    pub(crate) supported: bool,
    pub(crate) previous_submersion: f32,
    pub(crate) submersion: f32,
}

#[derive(Default)]
pub(crate) struct MovementReport {
    pub(crate) steps: [Option<MovementStep>; 30],
}
