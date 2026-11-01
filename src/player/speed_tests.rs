use super::*;

fn speeds() -> [SpeedMultiplier; 5] {
    [
        SpeedMultiplier::Normal,
        SpeedMultiplier::Double,
        SpeedMultiplier::Quadruple,
        SpeedMultiplier::Octuple,
        SpeedMultiplier::Sixteen,
    ]
}

#[test]
fn every_speed_normalizes_walking_and_three_axis_flight() {
    for speed in speeds() {
        for flying in [false, true] {
            let mut player = Player::new(Vec3::ZERO);
            player.flying = flying;
            player.update(
                MovementInput {
                    forward: true,
                    right: true,
                    jump: flying,
                    speed,
                    ..Default::default()
                },
                0.1,
                |_, y, _| y < 0,
            );
            let direction = Vec3::new(1.0, if flying { 1.0 } else { 0.0 }, -1.0).normalize();
            let expected = direction * 0.5 * speed.factor();
            assert!(
                (player.position - expected).length() < 0.0001,
                "{speed:?}, flying={flying}: {:?}",
                player.position
            );
        }
    }
}

#[test]
fn maximum_speed_sweeps_thin_positive_and_negative_walls_after_frame_stalls() {
    for sign in [-1.0, 1.0] {
        let mut player = Player::new(Vec3::new(0.5, 0.0, 0.5));
        let solid = |x, y, _| y < 0 || x == if sign > 0.0 { 4 } else { -4 };
        player.update(
            MovementInput {
                right: sign > 0.0,
                left: sign < 0.0,
                speed: SpeedMultiplier::Sixteen,
                ..Default::default()
            },
            10.0,
            solid,
        );
        let expected = if sign > 0.0 {
            4.0 - PLAYER_HALF_WIDTH
        } else {
            -3.0 + PLAYER_HALF_WIDTH
        };
        assert!((player.position.x - expected).abs() < 0.0001);
        assert!(!body_overlaps(player.position, &solid));
    }
}

#[test]
fn speed_does_not_scale_gravity_jumps_buoyancy_or_idle_current_drift() {
    use crate::world::WaterBiome;
    let water = |_, y, _| {
        (y > -30 && y < 8).then_some(WaterSample {
            bottom: -30,
            surface: 8.0,
            flow: [0.5, 0.0, -0.2],
            biome: WaterBiome::River,
            salinity: 0.0,
        })
    };
    for wet in [false, true] {
        let mut normal = Player::new(Vec3::new(0.5, if wet { 6.8 } else { 3.0 }, 0.5));
        let mut fast = normal.clone();
        for _ in 0..120 {
            for (player, speed) in [
                (&mut normal, SpeedMultiplier::Normal),
                (&mut fast, SpeedMultiplier::Sixteen),
            ] {
                player.update_with_water(
                    MovementInput {
                        speed,
                        ..Default::default()
                    },
                    MAX_STEP_DT,
                    |_, y, _| y < if wet { -29 } else { 0 },
                    |x, y, z| if wet { water(x, y, z) } else { None },
                );
            }
        }
        assert_eq!(fast.position, normal.position);
    }
    let mut normal = Player::new(Vec3::ZERO);
    let mut fast = normal.clone();
    for _ in 0..120 {
        normal.update(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            MAX_STEP_DT,
            |_, y, _| y < 0,
        );
        fast.update(
            MovementInput {
                jump: true,
                speed: SpeedMultiplier::Sixteen,
                ..Default::default()
            },
            MAX_STEP_DT,
            |_, y, _| y < 0,
        );
        assert_eq!(normal.position.y, fast.position.y);
    }
}

#[test]
fn maximum_speed_swimming_scales_strokes_and_keeps_swept_walls_and_beds_solid() {
    use crate::world::WaterBiome;
    let water = |_, y, _| {
        (y > -100 && y < 100).then_some(WaterSample {
            bottom: -100,
            surface: 100.0,
            flow: [0.0; 3],
            biome: WaterBiome::Ocean,
            salinity: 1.0,
        })
    };
    let input = MovementInput {
        forward: true,
        right: true,
        descend: true,
        ..Default::default()
    };
    let mut normal = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut fast = normal.clone();
    for _ in 0..120 {
        normal.update_with_water(input, MAX_STEP_DT, |_, _, _| false, water);
        fast.update_with_water(
            MovementInput {
                speed: SpeedMultiplier::Sixteen,
                ..input
            },
            MAX_STEP_DT,
            |_, _, _| false,
            water,
        );
    }
    let start = Vec3::new(0.5, 0.0, 0.5);
    assert!(((fast.position - start) - (normal.position - start) * 16.0).length() < 0.001);
    let solid = |x, y, _| x == 4 || y < -5;
    fast = Player::new(start);
    for _ in 0..30 {
        fast.update_with_water(
            MovementInput {
                right: true,
                descend: true,
                speed: SpeedMultiplier::Sixteen,
                ..Default::default()
            },
            0.1,
            solid,
            water,
        );
    }
    assert!(!body_overlaps(fast.position, &solid));
    assert!((fast.position.y + 5.0).abs() < 0.0001);
    assert!((fast.position.x - (4.0 - PLAYER_HALF_WIDTH)).abs() < 0.0001);
}

#[test]
fn idle_fall_into_water_is_identical_at_normal_and_maximum_selected_speed() {
    use crate::world::WaterBiome;
    let water = |_, y, _| {
        (y > -50 && y < 4).then_some(WaterSample {
            bottom: -50,
            surface: 4.0,
            flow: [0.0; 3],
            biome: WaterBiome::Lake,
            salinity: 0.0,
        })
    };
    let mut normal = Player::new(Vec3::new(0.5, 20.0, 0.5));
    let mut fast = normal.clone();
    for _ in 0..600 {
        normal.update_with_water(
            MovementInput::default(),
            MAX_STEP_DT,
            |_, y, _| y < -49,
            water,
        );
        fast.update_with_water(
            MovementInput {
                speed: SpeedMultiplier::Sixteen,
                ..Default::default()
            },
            MAX_STEP_DT,
            |_, y, _| y < -49,
            water,
        );
        assert_eq!(normal.position, fast.position);
    }
}
