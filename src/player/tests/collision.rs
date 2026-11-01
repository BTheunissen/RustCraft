use super::*;

#[test]
fn airborne_jump_does_not_cancel_gravity_or_buffer_a_landing_jump() {
    let mut player = Player::new(Vec3::Y * 3.0);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    player.update(jump, 0.1, floor);
    assert!(player.position.y < 3.0);
    assert!(!player.is_grounded());
    advance(&mut player, jump, 2.0, floor);
    assert_position(player.position, Vec3::ZERO);
    assert!(player.is_grounded());
}

#[test]
fn gravity_lands_and_exact_floor_contact_stays_grounded() {
    let mut player = Player::new(Vec3::Y * 5.0);
    player.update(MovementInput::default(), 0.1, floor);
    assert!(player.position.y < 5.0);
    assert!(!player.is_grounded());
    advance(&mut player, MovementInput::default(), 2.0, floor);
    for dt in [0.1, 0.00001, MAX_STEP_DT, 0.25] {
        player.update(MovementInput::default(), dt, floor);
        assert_position(player.position, Vec3::ZERO);
        assert!(player.is_grounded());
    }
}

#[test]
fn walls_stop_the_body_and_allow_sliding() {
    let wall = |x, y, _z| y < 0 || x == 1;
    let mut player = Player::new(Vec3::ZERO);
    advance(
        &mut player,
        MovementInput {
            right: true,
            forward: true,
            ..Default::default()
        },
        1.0,
        wall,
    );
    assert_position(player.position, Vec3::new(0.7, 0.0, -5.0 / 2.0_f32.sqrt()));
    assert!(player.is_grounded());
    assert!(!body_overlaps(player.position, &wall));
}

#[test]
fn touching_faces_at_large_coordinates_do_not_block_sliding_or_flight_exit() {
    let wall = |x, y, _z| y < 512 || x == 1025;
    let mut player = Player::new(Vec3::new(1024.5, 512.0, 1024.5));
    player.update(
        MovementInput {
            right: true,
            ..Default::default()
        },
        0.1,
        wall,
    );
    assert_eq!(player.position.x, 1025.0 - PLAYER_HALF_WIDTH);
    assert!(!body_overlaps(player.position, &wall));
    let previous = player.position;
    player.update(
        MovementInput {
            forward: true,
            ..Default::default()
        },
        0.1,
        wall,
    );
    assert_eq!(player.position.x, previous.x);
    assert_eq!(player.position.y, previous.y);
    assert!((player.position.z - previous.z + 0.5).abs() < 0.002);
    let previous = player.position;
    player.toggle_flight(wall);
    player.toggle_flight(wall);
    assert!(!player.flying);
    assert_eq!(player.position, previous);
}

#[test]
fn ceiling_stops_jump_then_gravity_returns_to_floor() {
    let ceiling = |_x, y, _z| y < 0 || y == 2;
    let mut player = Player::new(Vec3::ZERO);
    let mut apex = 0.0_f32;
    for _ in 0..120 {
        player.update(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            MAX_STEP_DT,
            ceiling,
        );
        apex = apex.max(player.position.y);
        assert!(!body_overlaps(player.position, &ceiling));
    }
    assert!((apex - 0.2).abs() < 0.0001);
    assert_position(player.position, Vec3::ZERO);
    assert!(player.is_grounded());
}

#[test]
fn negative_coordinates_use_floor_and_do_not_include_touching_voxels() {
    let wall = |x, y, _z| y < -2 || x == -2;
    let mut player = Player::new(Vec3::new(-0.5, -2.0, -0.5));
    advance(
        &mut player,
        MovementInput {
            left: true,
            ..Default::default()
        },
        1.0,
        wall,
    );
    assert_position(player.position, Vec3::new(-0.7, -2.0, -0.5));
    assert!(player.is_grounded());
    assert!(!body_overlaps(player.position, &wall));
    player.update(
        MovementInput {
            right: true,
            ..Default::default()
        },
        0.1,
        wall,
    );
    assert_position(player.position, Vec3::new(-0.2, -2.0, -0.5));
}

#[test]
fn walking_off_an_edge_clears_grounded_and_starts_falling() {
    let ledge = |x, y, _z| y < 0 && x <= 0;
    let mut player = Player::new(Vec3::ZERO);
    advance(
        &mut player,
        MovementInput {
            right: true,
            ..Default::default()
        },
        0.5,
        ledge,
    );
    assert!(player.position.x > 1.3);
    assert!(player.position.y < 0.0);
    assert!(!player.is_grounded());
}

#[test]
fn frame_stalls_are_bounded_and_cannot_tunnel_through_a_wall_or_floor() {
    let input = MovementInput {
        right: true,
        ..Default::default()
    };
    let mut stalled = Player::new(Vec3::ZERO);
    let mut bounded = stalled.clone();
    stalled.update(input, f32::MAX, floor);
    bounded.update(input, MAX_FRAME_DT, floor);
    assert_position(stalled.position, bounded.position);
    assert_position(stalled.position, Vec3::X * 1.25);

    let wall = |x, y, _z| y < 0 || x == 1;
    let mut player = Player::new(Vec3::ZERO);
    player.update(input, 10.0, wall);
    assert_position(player.position, Vec3::X * 0.7);

    let platform = |_x, y, _z| y == 0;
    let mut falling = Player::new(Vec3::Y * 100.0);
    for _ in 0..40 {
        falling.update(MovementInput::default(), 10.0, platform);
    }
    assert_position(falling.position, Vec3::Y);
    assert!(falling.is_grounded());
}

#[test]
fn nonpositive_or_nonfinite_dt_is_ignored_including_jump_edges() {
    let mut player = Player::new(Vec3::ZERO);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        player.update(jump, dt, |_, _, _| panic!("invalid dt queried world"));
        assert_position(player.position, Vec3::ZERO);
        assert!(player.is_grounded());
        assert!(!player.jump_held);
    }
    player.update(jump, MAX_STEP_DT, floor);
    assert!(player.position.y > 0.0);
}

#[test]
fn exiting_flight_in_clear_air_resumes_gravity_without_another_jump() {
    let mut player = Player::new(Vec3::Y * 3.0);
    player.toggle_flight(floor);
    player.update(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    player.toggle_flight(floor);
    assert!(!player.flying);
    assert!(!player.is_grounded());
    let previous_y = player.position.y;
    player.update(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        0.1,
        floor,
    );
    assert!(player.position.y < previous_y);
    advance(
        &mut player,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        2.0,
        floor,
    );
    assert_position(player.position, Vec3::ZERO);
    assert!(player.is_grounded());
}
