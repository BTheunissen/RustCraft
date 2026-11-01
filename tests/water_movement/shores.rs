use super::*;

#[test]
fn sampling_uses_world_coordinates_and_at_most_three_body_heights_per_step() {
    let queries = Cell::new(0);
    let mut player = Player::new(Vec3::new(-1024.5, -31.4, -512.5));
    let water = |x, y, z| {
        queries.set(queries.get() + 1);
        assert_eq!(x, -1025);
        assert_eq!(z, -513);
        assert!((-32..=-30).contains(&y), "unexpected Y {y}");
        water_at(sample(-30.2, -40, [0.0; 3]), y)
    };
    player.update_with_water(MovementInput::default(), DT, |_, y, _| y < -40, water);
    assert!(queries.get() <= 6, "query count {}", queries.get());
    assert!(player.is_swimming());
    assert!((player.submersion() - 2.0 / 3.0).abs() < 0.0001);
}

#[test]
fn low_bank_exit_requires_space_and_lands_without_repeating_held_jumps() {
    let bank = |x, y, _| y < BOTTOM || (x >= 1 && y < 4);
    let water = |x, y, _| if x >= 1 { None } else { pool(x, y, 0) };
    let mut swimming = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(
        &mut swimming,
        MovementInput {
            right: true,
            ..Default::default()
        },
        2.0,
        bank,
        water,
    );
    assert!((swimming.position.x - 0.7).abs() < 0.001);
    assert!(swimming.is_swimming());
    let held = MovementInput {
        right: true,
        jump: true,
        ..Default::default()
    };
    for _ in 0..360 {
        swimming.update_with_water(held, DT, bank, water);
        assert!(
            body_clear(&swimming, bank),
            "bank exit embedded at {:?}",
            swimming.position
        );
    }
    assert!(
        swimming.position.x > 2.0,
        "failed to exit bank at {:?}",
        swimming.position
    );
    assert!((swimming.position.y - 4.0).abs() < 0.001);
    assert!(swimming.is_grounded());
    assert!(!swimming.is_swimming());
    advance(
        &mut swimming,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        1.0,
        bank,
        water,
    );
    assert!((swimming.position.y - 4.0).abs() < 0.001);
    swimming.update_with_water(MovementInput::default(), DT, bank, water);
    swimming.update_with_water(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        DT,
        bank,
        water,
    );
    assert!(swimming.position.y > 4.0);
}

#[test]
fn a_one_block_high_shoreline_can_be_exited_with_space_without_wall_climbing() {
    for surface in [SURFACE, SURFACE + 0.35] {
        let bank_top = surface.floor() as i32 + 1;
        let bank = |x, y, _| y <= BOTTOM || (x >= 1 && y < bank_top);
        let water = |x, y, _| {
            if x >= 1 {
                None
            } else {
                water_at(sample(surface, BOTTOM, [0.0; 3]), y)
            }
        };
        let mut player = Player::new(Vec3::new(0.5, surface - 1.2, 0.5));
        for _ in 0..360 {
            player.update_with_water(
                MovementInput {
                    right: true,
                    jump: true,
                    ..Default::default()
                },
                DT,
                bank,
                water,
            );
            assert!(
                body_clear(&player, bank),
                "high shoreline embedded at {:?}",
                player.position
            );
        }
        assert!(
            player.position.x > 2.0,
            "shoreline exit failed at {:?}",
            player.position
        );
        assert!((player.position.y - bank_top as f32).abs() < 0.001);
        assert!(player.is_grounded());
        assert!(!player.is_swimming());
    }
}

#[test]
fn shallow_submerged_steps_do_not_consume_the_jump_needed_to_leave_the_shore() {
    let solid = |_x, y, z| {
        y < if z <= -1 {
            18
        } else if z == 0 {
            17
        } else {
            15
        }
    };
    let water = |_, y, z| {
        let bottom = if z <= -1 {
            17
        } else if z == 0 {
            16
        } else {
            14
        };
        water_at(sample(18.0, bottom, [0.027, 0.0, -0.096]), y)
    };
    let mut player = Player::new(Vec3::new(0.5, 16.8, 2.5));
    let stroke = MovementInput {
        forward: true,
        jump: true,
        ..Default::default()
    };
    for _ in 0..960 {
        player.update_with_water(stroke, DT, solid, water);
        assert!(body_clear(&player, solid));
        if player.position.z < -0.6 && player.is_grounded() {
            break;
        }
    }
    assert!(
        player.position.z < -0.6 && (player.position.y - 18.0).abs() < 0.001,
        "shallow-stepped shore blocked the swimmer at {:?}, submersion {}",
        player.position,
        player.submersion()
    );
    assert!(!player.is_swimming());
}

#[test]
fn a_held_shore_exit_rearms_after_landing_on_a_submerged_step() {
    let solid = |_x, y, z| {
        y < if z <= -467 {
            18
        } else if z == -466 {
            17
        } else {
            16
        }
    };
    let water = |_, y, z| {
        let bottom = if z <= -467 {
            17
        } else if z == -466 {
            16
        } else {
            15
        };
        water_at(sample(18.0, bottom, [0.027, 0.0, -0.096]), y)
    };
    let mut player = Player::new(Vec3::new(-210.5, 17.23, -465.58));
    let stroke = MovementInput {
        forward: true,
        jump: true,
        ..Default::default()
    };
    for _ in 0..960 {
        player.update_with_water(stroke, DT, solid, water);
        assert!(body_clear(&player, solid));
        if player.position.z < -467.4 && player.is_grounded() {
            break;
        }
    }
    assert!(
        player.position.z < -467.4 && (player.position.y - 18.0).abs() < 0.001,
        "submerged step kept the shore-exit latch armed at {:?}",
        player.position
    );
    let landed = player.position;
    advance(
        &mut player,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        1.0,
        solid,
        water,
    );
    assert_near(player.position, landed, 0.0001);
    assert!(player.is_grounded() && !player.is_swimming());
}

#[test]
fn tall_bank_and_overhanging_bank_do_not_auto_lift_or_embed_the_player() {
    for overhang in [false, true] {
        let bank = |x, y, _| {
            y < BOTTOM || (x >= 1 && y < if overhang { 4 } else { 8 }) || (overhang && y == 5)
        };
        let water = |x, y, _| if x >= 1 { None } else { pool(x, y, 0) };
        let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
        for _ in 0..360 {
            player.update_with_water(
                MovementInput {
                    right: true,
                    jump: true,
                    ..Default::default()
                },
                DT,
                bank,
                water,
            );
            assert!(body_clear(&player, bank));
        }
        assert!(player.position.x <= 0.701);
        assert!(player.position.y < SURFACE);
        assert!(player.is_swimming());
    }
}

#[test]
fn frame_stalls_are_capped_and_coarse_frames_match_fine_water_steps() {
    let flowing = |_, y, _| water_at(sample(SURFACE, BOTTOM, [0.4, 0.0, -0.2]), y);
    let input = MovementInput {
        forward: true,
        right: true,
        descend: true,
        speed: rustcraft::player::SpeedMultiplier::Double,
        ..Default::default()
    };
    let mut coarse = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut fine = coarse.clone();
    let mut stalled = coarse.clone();
    for _ in 0..8 {
        coarse.update_with_water(input, 0.25, floor, flowing);
        stalled.update_with_water(input, f32::MAX, floor, flowing);
    }
    advance(&mut fine, input, 2.0, floor, flowing);
    assert_near(coarse.position, fine.position, 0.002);
    assert_eq!(coarse.position, stalled.position);
    assert!((coarse.submersion() - fine.submersion()).abs() < 0.001);
}
