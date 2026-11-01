use super::*;

#[test]
fn creative_flight_is_exactly_legacy_flight_and_never_queries_either_callback() {
    let mut flying = Player::new(Vec3::new(0.5, -3.0, 0.5));
    flying.toggle_flight(|_, _, _| panic!("enabling flight queried solids"));
    let mut legacy = flying.clone();
    for input in [
        MovementInput {
            forward: true,
            right: true,
            jump: true,
            speed: rustcraft::player::SpeedMultiplier::Double,
            ..Default::default()
        },
        MovementInput {
            descend: true,
            left: true,
            ..Default::default()
        },
        MovementInput {
            jump: true,
            descend: true,
            ..Default::default()
        },
    ] {
        flying.update_with_water(
            input,
            0.25,
            |_, _, _| panic!("flight queried solids"),
            |_, _, _| panic!("flight queried water"),
        );
        legacy.update(input, 0.25, |_, _, _| {
            panic!("legacy flight queried solids")
        });
        assert_eq!(flying.position, legacy.position);
        assert!(!flying.is_swimming());
        assert_eq!(flying.submersion(), 0.0);
    }
}

#[test]
fn exiting_flight_underwater_swims_next_update_without_lifting_to_air() {
    let mut player = Player::new(Vec3::new(0.5, -3.0, 0.5));
    player.toggle_flight(floor);
    player.toggle_flight(floor);
    assert!(!player.flying);
    assert_eq!(player.position.y, -3.0);
    assert!(!player.is_swimming());
    player.update_with_water(MovementInput::default(), DT, floor, pool);
    assert!(player.is_swimming());
    assert!(player.position.y > -3.0 && player.position.y < -2.99);
    let solid = |_, y, _| y < -6;
    player.toggle_flight(solid);
    player.position.y = -7.5;
    player.toggle_flight(solid);
    assert!(!player.flying);
    assert_eq!(player.position.y, -6.0);
    assert!(body_clear(&player, solid));
    player.update_with_water(MovementInput::default(), DT, solid, pool);
    assert!(player.is_swimming());
    assert!(player.position.y < -5.99);
}

#[test]
fn respawn_clears_fluid_momentum_and_rearms_the_dry_jump() {
    let flowing = |_, y, _| water_at(sample(SURFACE, BOTTOM, [1.39, 0.0, 0.0]), y);
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(
        &mut player,
        MovementInput {
            forward: true,
            jump: true,
            ..Default::default()
        },
        1.0,
        floor,
        flowing,
    );
    player.respawn(Vec3::new(0.5, FLOAT_FEET, 0.5));
    assert!(!player.is_swimming());
    assert_eq!(player.submersion(), 0.0);
    player.update_with_water(MovementInput::default(), DT, floor, pool);
    assert_near(player.position, Vec3::new(0.5, FLOAT_FEET, 0.5), 0.0001);
    player.respawn(Vec3::new(0.5, 0.0, 0.5));
    player.update_with_water(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        DT,
        |_, y, _| y < 0,
        |_, _, _| None,
    );
    assert!(player.position.y > 0.0);
    assert!(!player.is_swimming());
}

#[test]
fn a_missing_water_callback_preserves_legacy_walking_jumps_walls_and_gravity() {
    let world = |x, y, _| y < 0 || x == 2;
    let mut legacy = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut new = legacy.clone();
    for frame in 0..360 {
        let input = MovementInput {
            forward: frame < 120,
            right: (120..240).contains(&frame),
            jump: (20..160).contains(&frame) || frame >= 200,
            descend: true,
            speed: if frame > 250 {
                rustcraft::player::SpeedMultiplier::Double
            } else {
                rustcraft::player::SpeedMultiplier::Normal
            },
            ..Default::default()
        };
        legacy.update(input, DT, world);
        new.update_with_water(input, DT, world, |_, _, _| None);
        assert_eq!(legacy.position, new.position);
        assert_eq!(legacy.is_grounded(), new.is_grounded());
        assert!(!new.is_swimming());
    }
}

#[test]
fn the_legacy_solidity_only_api_still_sinks_through_scenery_water_and_lava() {
    let mut legacy = Player::new(Vec3::new(0.5, 2.0, 0.5));
    let mut no_water = legacy.clone();
    for _ in 0..240 {
        legacy.update(MovementInput::default(), DT, |_, y, _| y < 0);
        no_water.update_with_water(
            MovementInput::default(),
            DT,
            |_, y, _| y < 0,
            |_, _, _| None,
        );
    }
    assert_eq!(legacy.position, Vec3::new(0.5, 0.0, 0.5));
    assert_eq!(no_water.position, legacy.position);
    assert!(legacy.is_grounded());
    assert!(!legacy.is_swimming());
}

#[test]
fn invalid_dt_ignores_world_callbacks_fluid_state_and_jump_latches() {
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    player.update_with_water(MovementInput::default(), DT, floor, pool);
    let position = player.position;
    let submerged = player.submersion();
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        player.update_with_water(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            dt,
            |_, _, _| panic!("invalid dt queried solids"),
            |_, _, _| panic!("invalid dt queried water"),
        );
        assert_eq!(player.position, position);
        assert_eq!(player.submersion(), submerged);
        assert!(player.is_swimming());
    }
    player.position.y = 0.0;
    player.update_with_water(
        MovementInput {
            jump: true,
            ..Default::default()
        },
        DT,
        |_, y, _| y < 0,
        |_, _, _| None,
    );
    assert!(player.position.y > 0.0);
}

#[test]
fn nonfinite_or_inverted_water_metadata_is_ignored_without_poisoning_motion() {
    let valid = sample(SURFACE, BOTTOM, [0.0; 3]);
    let mut invalid = vec![
        WaterSample {
            surface: f32::NAN,
            ..valid
        },
        WaterSample {
            surface: f32::INFINITY,
            ..valid
        },
        WaterSample {
            surface: f32::NEG_INFINITY,
            ..valid
        },
        WaterSample {
            surface: BOTTOM as f32,
            ..valid
        },
        WaterSample {
            surface: BOTTOM as f32 - 1.0,
            ..valid
        },
        WaterSample { bottom: 8, ..valid },
        WaterSample {
            salinity: f32::NAN,
            ..valid
        },
        WaterSample {
            salinity: f32::INFINITY,
            ..valid
        },
    ];
    for axis in 0..3 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = valid;
            bad.flow[axis] = value;
            invalid.push(bad);
        }
    }
    for bad in invalid {
        let input = MovementInput {
            right: true,
            ..Default::default()
        };
        let mut dry = Player::new(Vec3::new(0.5, 0.0, 0.5));
        let mut player = dry.clone();
        dry.update(input, 0.25, |_, y, _| y < 0);
        player.update_with_water(input, 0.25, |_, y, _| y < 0, |_, _, _| Some(bad));
        assert_eq!(player.position, dry.position, "bad metadata {bad:?}");
        assert!(!player.is_swimming());
        assert_eq!(player.submersion(), 0.0);
    }
}
