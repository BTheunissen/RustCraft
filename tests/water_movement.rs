//! Public water-controller checks without a renderer, event loop or generated world.

use glam::Vec3;
use rustcraft::player::{MovementInput, PLAYER_HALF_WIDTH, PLAYER_HEIGHT, Player};
use rustcraft::world::{WaterBiome, WaterSample};
use std::cell::Cell;
use std::f32::consts::FRAC_PI_2;

const DT: f32 = 1.0 / 120.0;
const SURFACE: f32 = 4.0;
const BOTTOM: i32 = -12;
const FLOAT_FEET: f32 = SURFACE - 1.2;

fn sample(surface: f32, bottom: i32, flow: [f32; 3]) -> WaterSample {
    WaterSample {
        surface,
        bottom,
        flow,
        biome: WaterBiome::Lake,
        salinity: 0.0,
    }
}

fn water_at(sample: WaterSample, y: i32) -> Option<WaterSample> {
    (y > sample.bottom && (y as f32) < sample.surface).then_some(sample)
}

fn pool(_x: i32, y: i32, _z: i32) -> Option<WaterSample> {
    water_at(sample(SURFACE, BOTTOM, [0.0; 3]), y)
}

fn floor(_x: i32, y: i32, _z: i32) -> bool {
    y <= BOTTOM
}

fn advance(
    player: &mut Player,
    input: MovementInput,
    seconds: f32,
    solid: impl Fn(i32, i32, i32) -> bool,
    water: impl Fn(i32, i32, i32) -> Option<WaterSample>,
) {
    for _ in 0..(seconds / DT).round() as u32 {
        player.update_with_water(input, DT, &solid, &water);
        assert!(player.position.is_finite());
        assert!((0.0..=1.0).contains(&player.submersion()));
    }
}

fn assert_near(actual: Vec3, expected: Vec3, tolerance: f32) {
    assert!(
        actual.distance(expected) < tolerance,
        "expected {expected:?}, got {actual:?} (tolerance {tolerance})"
    );
}

fn body_clear(player: &Player, solid: impl Fn(i32, i32, i32) -> bool) -> bool {
    let min = player.position - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = player.position + Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|v| (f64::from(v) + 0.000_01).floor() as i32);
    let last = max
        .to_array()
        .map(|v| (f64::from(v) - 0.000_01).floor() as i32);
    (first[0]..=last[0])
        .all(|x| (first[1]..=last[1]).all(|y| (first[2]..=last[2]).all(|z| !solid(x, y, z))))
}

#[path = "water_movement/currents.rs"]
mod currents;
#[path = "water_movement/shores.rs"]
mod shores;
#[path = "water_movement/swimming.rs"]
mod swimming;
#[path = "water_movement/transitions.rs"]
mod transitions;
