use super::*;

#[test]
fn cardinal_controls_move_five_blocks_per_second() {
    let cases = [
        (
            MovementInput {
                forward: true,
                ..Default::default()
            },
            Vec3::NEG_Z,
        ),
        (
            MovementInput {
                backward: true,
                ..Default::default()
            },
            Vec3::Z,
        ),
        (
            MovementInput {
                left: true,
                ..Default::default()
            },
            Vec3::NEG_X,
        ),
        (
            MovementInput {
                right: true,
                ..Default::default()
            },
            Vec3::X,
        ),
    ];
    for (input, direction) in cases {
        let mut player = Player::new(Vec3::ZERO);
        advance(&mut player, input, 1.0, floor);
        assert_position(player.position, direction * 5.0);
        assert!(player.is_grounded());
    }
}

#[test]
fn movement_and_strafing_follow_yaw_not_pitch() {
    let mut player = Player::new(Vec3::ZERO);
    player.yaw = FRAC_PI_2;
    player.pitch = MAX_PITCH;
    player.update(
        MovementInput {
            forward: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    assert_position(player.position, Vec3::X * 0.5);
    player.update(
        MovementInput {
            right: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    assert_position(player.position, Vec3::new(0.5, 0.0, 0.5));
}

#[test]
fn diagonal_movement_is_normalized_at_normal_and_doubled_speed() {
    for (speed_setting, speed) in [
        (crate::player::SpeedMultiplier::Normal, 5.0),
        (crate::player::SpeedMultiplier::Double, 10.0),
    ] {
        let mut player = Player::new(Vec3::ZERO);
        advance(
            &mut player,
            MovementInput {
                forward: true,
                right: true,
                speed: speed_setting,
                ..Default::default()
            },
            1.0,
            floor,
        );
        assert_position(
            player.position,
            Vec3::new(1.0, 0.0, -1.0).normalize() * speed,
        );
    }
}

#[test]
fn opposing_controls_do_not_generate_nan_or_drift() {
    let mut player = Player::new(Vec3::ZERO);
    player.yaw = 0.37;
    player.update(
        MovementInput {
            forward: true,
            backward: true,
            left: true,
            right: true,
            ..Default::default()
        },
        0.25,
        floor,
    );
    assert_position(player.position, Vec3::ZERO);
    assert!(player.is_grounded());
}

#[test]
fn eye_and_look_direction_use_feet_and_camera_conventions() {
    let mut player = Player::new(Vec3::new(2.0, -4.0, 6.0));
    assert_position(player.eye_position(), Vec3::new(2.0, -2.38, 6.0));
    assert_position(player.look_direction(), Vec3::NEG_Z);
    player.yaw = FRAC_PI_2;
    player.pitch = PI / 6.0;
    assert_position(
        player.look_direction(),
        Vec3::new((PI / 6.0).cos(), 0.5, 0.0),
    );
    assert!((player.look_direction().length() - 1.0).abs() < 0.0001);
}

#[test]
fn mouse_look_inverts_y_clamps_pitch_and_wraps_yaw() {
    let mut player = Player::new(Vec3::ZERO);
    player.look(200.0, -100.0);
    assert!((player.yaw - 0.5).abs() < 0.0001);
    assert!((player.pitch - 0.25).abs() < 0.0001);
    player.look(100_000.0, -100_000.0);
    assert!((-PI..=PI).contains(&player.yaw));
    assert_eq!(player.pitch, MAX_PITCH);
    player.look(-200_000.0, 200_000.0);
    assert!((-PI..=PI).contains(&player.yaw));
    assert_eq!(player.pitch, -MAX_PITCH);
}

#[test]
fn mouse_look_rejects_nonfinite_pairs_and_bounds_huge_finite_deltas() {
    let mut player = Player::new(Vec3::ZERO);
    player.look(40.0, -80.0);
    let original = (player.yaw, player.pitch);
    for (x, y) in [
        (f64::NAN, 10.0),
        (10.0, f64::NAN),
        (f64::INFINITY, 10.0),
        (10.0, f64::NEG_INFINITY),
    ] {
        player.look(x, y);
        assert_eq!((player.yaw, player.pitch), original);
    }
    player.look(f64::MAX, -f64::MAX);
    assert!(player.look_direction().is_finite());
    assert!((-PI..=PI).contains(&player.yaw));
    assert_eq!(player.pitch, MAX_PITCH);
}

#[test]
fn jumping_lands_without_repeating_until_space_is_released() {
    let mut player = Player::new(Vec3::ZERO);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    let mut apex = 0.0_f32;
    for _ in 0..240 {
        player.update(jump, MAX_STEP_DT, floor);
        apex = apex.max(player.position.y);
    }
    assert!((1.1..1.25).contains(&apex), "jump apex: {apex}");
    assert_position(player.position, Vec3::ZERO);
    assert!(player.is_grounded());
    player.update(MovementInput::default(), MAX_STEP_DT, floor);
    player.update(jump, MAX_STEP_DT, floor);
    assert!(player.position.y > 0.0);
    assert!(!player.is_grounded());
}

#[test]
fn clearing_application_input_rearms_a_held_jump_after_pause() {
    let mut player = Player::new(Vec3::ZERO);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    advance(&mut player, jump, 2.0, floor);
    assert!(player.is_grounded());
    player.clear_input();
    player.update(jump, MAX_STEP_DT, floor);
    assert!(player.position.y > 0.0);
    assert!(!player.is_grounded());
}

#[test]
fn flight_ignores_collision_and_uses_explicit_ascent_and_descent() {
    let mut player = Player::new(Vec3::ZERO);
    player.pitch = MAX_PITCH;
    player.toggle_flight(|_, _, _| panic!("enabling flight queried world"));
    player.update(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        0.1,
        |_, _, _| panic!("flight queried world"),
    );
    assert_position(player.position, Vec3::Y * 0.5);
    assert!(!player.is_grounded());
    player.update(
        MovementInput {
            descend: true,
            ..Default::default()
        },
        0.1,
        |_, _, _| true,
    );
    assert_position(player.position, Vec3::ZERO);
    player.update(
        MovementInput {
            forward: true,
            ..Default::default()
        },
        0.1,
        |_, _, _| true,
    );
    assert_position(player.position, Vec3::NEG_Z * 0.5);
}

#[test]
fn flight_normalizes_three_axis_speed_and_opposing_vertical_input() {
    let mut player = Player::new(Vec3::ZERO);
    player.toggle_flight(floor);
    advance(
        &mut player,
        MovementInput {
            forward: true,
            right: true,
            jump: true,
            speed: crate::player::SpeedMultiplier::Double,
            ..Default::default()
        },
        1.0,
        floor,
    );
    assert_position(
        player.position,
        Vec3::new(1.0, 1.0, -1.0).normalize() * 10.0,
    );
    let previous = player.position;
    player.update(
        MovementInput {
            jump: true,
            descend: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    assert_position(player.position, previous);
}

#[test]
fn leaving_flight_lifts_an_embedded_body_to_clearance() {
    let column = |x, y, z| x == 0 && z == 0 && (0..3).contains(&y);
    let mut player = Player::new(Vec3::new(0.5, 1.5, 0.5));
    player.toggle_flight(column);
    player.toggle_flight(column);
    assert!(!player.flying);
    assert_position(player.position, Vec3::new(0.5, 3.0, 0.5));
    assert!(!body_overlaps(player.position, &column));
    assert!(player.is_grounded());
    player.update(MovementInput::default(), 0.1, column);
    assert_position(player.position, Vec3::new(0.5, 3.0, 0.5));
}

#[test]
fn flight_exit_with_no_bounded_clearance_keeps_flight_and_position() {
    let mut player = Player::new(Vec3::new(-0.5, -10.5, -0.5));
    player.toggle_flight(|_, _, _| true);
    let previous = player.position;
    player.toggle_flight(|_, _, _| true);
    assert!(player.flying);
    assert!(!player.is_grounded());
    assert_position(player.position, previous);
}

#[test]
fn respawn_resets_motion_flight_and_jump_latch_but_preserves_view() {
    let mut player = Player::new(Vec3::Y * 3.0);
    player.look(200.0, -100.0);
    player.update(MovementInput::default(), 0.1, floor);
    assert!(player.vertical_velocity < 0.0);
    player.toggle_flight(floor);
    player.update(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    let previous_view = (player.yaw, player.pitch);
    player.respawn(Vec3::ZERO);
    assert_position(player.position, Vec3::ZERO);
    assert!(!player.flying);
    assert!(player.is_grounded());
    assert_eq!(player.vertical_velocity, 0.0);
    assert_eq!((player.yaw, player.pitch), previous_view);
    player.update(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        MAX_STEP_DT,
        floor,
    );
    assert!(player.position.y > 0.0);
}
