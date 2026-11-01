use super::*;

#[test]
fn swimming_drag_accelerates_and_damps_without_lingering_no_input_drift() {
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let input = MovementInput {
        forward: true,
        ..Default::default()
    };
    let start = player.position;
    player.update_with_water(input, 0.1, floor, pool);
    let initial = start.z - player.position.z;
    player.update_with_water(input, 0.1, floor, pool);
    let faster = start.z - initial - player.position.z;
    assert!(faster > initial * 1.5);
    advance(&mut player, input, 1.0, floor, pool);
    let released = player.position;
    player.clear_input();
    advance(&mut player, MovementInput::default(), 2.0, floor, pool);
    assert!(released.z - player.position.z < 0.43);
    let settled = player.position;
    advance(&mut player, MovementInput::default(), 1.0, floor, pool);
    assert_near(player.position, settled, 0.001);
}

#[test]
fn idle_swimmers_drift_along_the_shared_current_vector_at_river_speed() {
    let current = Vec3::new(0.9, 0.0, -0.8).normalize() * 1.39;
    let flowing = |_, y, _| water_at(sample(SURFACE, BOTTOM, current.to_array()), y);
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(&mut player, MovementInput::default(), 1.0, floor, flowing);
    let settled = player.position;
    advance(&mut player, MovementInput::default(), 1.0, floor, flowing);
    assert_near(player.position - settled, current, 0.005);
    assert!((player.position.y - FLOAT_FEET).abs() < 0.001);
    assert!(player.is_swimming());
}

#[test]
fn swimming_with_the_current_is_faster_than_swimming_against_it() {
    let flowing = |_, y, _| water_at(sample(SURFACE, BOTTOM, [1.39, 0.0, 0.0]), y);
    let mut with = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let mut against = with.clone();
    advance(
        &mut with,
        MovementInput {
            right: true,
            ..Default::default()
        },
        1.0,
        floor,
        flowing,
    );
    advance(
        &mut against,
        MovementInput {
            left: true,
            ..Default::default()
        },
        1.0,
        floor,
        flowing,
    );
    let with_distance = with.position.x - 0.5;
    let against_distance = 0.5 - against.position.x;
    assert!(against_distance > 0.8 && against_distance < 1.2);
    assert!(with_distance > 3.0 && with_distance > against_distance * 3.0);
}

#[test]
fn current_and_swimming_slide_on_walls_without_pushing_inside_the_collider() {
    for (origin, wall_x) in [
        (Vec3::new(0.5, FLOAT_FEET, 0.5), 1),
        (Vec3::new(-512.5, -30.2, -512.5), -512),
    ] {
        let surface = origin.y + 1.2;
        let bottom = origin.y.floor() as i32 - 8;
        let wall = |x, y, _| y < bottom || x == wall_x;
        let flowing = |x, y, _| {
            if x == wall_x {
                None
            } else {
                water_at(sample(surface, bottom, [1.39, 0.0, -0.5]), y)
            }
        };
        let mut player = Player::new(origin);
        for _ in 0..240 {
            player.update_with_water(
                MovementInput {
                    forward: true,
                    right: true,
                    ..Default::default()
                },
                DT,
                wall,
                flowing,
            );
            assert!(
                body_clear(&player, wall),
                "embedded at {:?}",
                player.position
            );
        }
        assert!((player.position.x - (wall_x as f32 - PLAYER_HALF_WIDTH)).abs() < 0.001);
        assert!(player.position.z < origin.z - 3.0);
    }
}

#[test]
fn descent_and_vertical_currents_cannot_pass_through_a_pool_floor_or_ceiling() {
    let solid = |_, y, _| y < 0 || y == 6;
    let down = |_, y, _| water_at(sample(6.0, -1, [0.0, -2.0, 0.0]), y);
    let up = |_, y, _| water_at(sample(6.0, -1, [0.0, 2.0, 0.0]), y);
    let mut player = Player::new(Vec3::new(0.5, 2.0, 0.5));
    advance(
        &mut player,
        MovementInput {
            descend: true,
            ..Default::default()
        },
        3.0,
        solid,
        down,
    );
    assert!((player.position.y - 0.0).abs() < 0.001);
    assert!(player.is_grounded());
    assert!(body_clear(&player, solid));
    advance(
        &mut player,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        3.0,
        solid,
        up,
    );
    assert!((player.position.y - (6.0 - PLAYER_HEIGHT)).abs() < 0.001);
    assert!(!player.is_grounded());
    assert!(body_clear(&player, solid));
}

#[test]
fn space_can_escape_a_downward_waterfall_even_while_moving_horizontally() {
    let waterfall = |_, y, _| water_at(sample(SURFACE, BOTTOM, [0.0, -2.0, 0.0]), y);
    for input in [
        MovementInput {
            jump: true,
            ..Default::default()
        },
        MovementInput {
            jump: true,
            forward: true,
            right: true,
            ..Default::default()
        },
    ] {
        let mut player = Player::new(Vec3::new(0.5, -5.0, 0.5));
        advance(&mut player, input, 2.0, floor, waterfall);
        assert!(
            player.position.y > -4.2,
            "Space failed against current: {:?}",
            player.position
        );
    }
}

#[test]
fn descending_can_overcome_an_upward_current_at_each_selected_speed() {
    let upwelling = |_, y, _| water_at(sample(SURFACE, BOTTOM, [0.0, 2.0, 0.0]), y);
    for speed in [
        rustcraft::player::SpeedMultiplier::Normal,
        rustcraft::player::SpeedMultiplier::Double,
    ] {
        let mut player = Player::new(Vec3::new(0.5, -3.0, 0.5));
        advance(
            &mut player,
            MovementInput {
                forward: true,
                right: true,
                descend: true,
                speed,
                ..Default::default()
            },
            2.0,
            floor,
            upwelling,
        );
        assert!(
            player.position.y < -3.8,
            "dive reversed: {:?}",
            player.position
        );
    }
}

#[test]
fn downward_current_samples_below_zero_use_world_surface_and_depth() {
    let mut surface = Player::new(Vec3::new(0.5, -3.0, 0.5));
    let mut cave = surface.clone();
    cave.position.y -= 32.0;
    let surface_water = |_, y, _| water_at(sample(SURFACE, BOTTOM, [0.3, -1.0, 0.1]), y);
    let cave_water = |_, y, _| water_at(sample(SURFACE - 32.0, BOTTOM - 32, [0.3, -1.0, 0.1]), y);
    let input = MovementInput {
        jump: true,
        ..Default::default()
    };
    advance(&mut surface, input, 2.0, floor, surface_water);
    advance(
        &mut cave,
        input,
        2.0,
        |_, y, _| y <= BOTTOM - 32,
        cave_water,
    );
    assert_near(cave.position + Vec3::Y * 32.0, surface.position, 0.003);
    assert!(cave.position.y > -33.0);
    assert!(cave.is_swimming());
}

#[test]
fn leaving_water_restores_dry_speed_and_does_not_reuse_old_current_momentum() {
    let water = |x, y, _| if x == 0 { pool(x, y, 0) } else { None };
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let input = MovementInput {
        right: true,
        ..Default::default()
    };
    advance(&mut player, input, 1.0, floor, water);
    assert!(player.position.x > 3.0);
    assert_eq!(player.submersion(), 0.0);
    assert!(!player.is_swimming());
    let start = player.position;
    player.update_with_water(input, 0.1, floor, water);
    assert!((player.position.x - start.x - 0.5).abs() < 0.001);
    player.position = Vec3::new(0.5, FLOAT_FEET, 0.5);
    player.toggle_flight(floor);
    assert!(!player.is_swimming());
    assert_eq!(player.submersion(), 0.0);
    player.toggle_flight(floor);
    player.update_with_water(MovementInput::default(), DT, floor, pool);
    assert!((player.position.x - 0.5).abs() < 0.0001);
    assert!(player.is_swimming());
}

#[test]
fn huge_finite_currents_are_bounded_before_vector_math_and_collision() {
    let wall = |x, y, _| y < BOTTOM || x == 1;
    let flowing = |_, y, _| water_at(sample(SURFACE, BOTTOM, [f32::MAX, f32::MAX, -f32::MAX]), y);
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let start = player.position;
    advance(&mut player, MovementInput::default(), 1.0, wall, flowing);
    assert!(body_clear(&player, wall));
    assert!(player.position.x <= 0.7);
    assert!(player.position.with_y(0.0).distance(start.with_y(0.0)) < 3.0);
    assert!(player.position.y < start.y + 2.0);
}

#[test]
fn opposing_swim_controls_do_not_generate_nan_or_horizontal_drift() {
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let start = player.position;
    advance(
        &mut player,
        MovementInput {
            forward: true,
            backward: true,
            left: true,
            right: true,
            jump: true,
            descend: true,
            speed: rustcraft::player::SpeedMultiplier::Double,
        },
        1.0,
        floor,
        pool,
    );
    assert_near(player.position, start, 0.0001);
}
