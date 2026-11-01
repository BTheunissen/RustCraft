use super::*;
use std::f32::consts::FRAC_PI_2;

fn floor(_x: i32, y: i32, _z: i32) -> bool {
    y < 0
}

fn advance(
    player: &mut Player,
    input: MovementInput,
    seconds: f32,
    is_solid: impl Fn(i32, i32, i32) -> bool,
) {
    for _ in 0..(seconds / MAX_STEP_DT).round() as u32 {
        player.update(input, MAX_STEP_DT, &is_solid);
    }
}

fn assert_position(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 0.0001,
        "expected {expected:?}, got {actual:?}"
    );
}

mod collision;
mod movement;
