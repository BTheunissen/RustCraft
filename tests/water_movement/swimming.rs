use super::*;

#[test]
fn idle_players_float_near_the_surface_instead_of_sinking_to_the_bed() {
    for feet in [FLOAT_FEET, FLOAT_FEET - 2.0, FLOAT_FEET + 0.3] {
        let mut player = Player::new(Vec3::new(0.5, feet, 0.5));
        advance(&mut player, MovementInput::default(), 5.0, floor, pool);
        assert!(
            (player.position.y - FLOAT_FEET).abs() < 0.08,
            "started at {feet}, ended at {:?}",
            player.position
        );
        assert!(player.eye_position().y > SURFACE + 0.3);
        assert!(player.is_swimming());
        assert!(!player.is_grounded());
        assert_near(
            player.position.with_y(0.0),
            Vec3::new(0.5, 0.0, 0.5),
            0.0001,
        );
        let mut low = f32::MAX;
        let mut high = f32::MIN;
        for _ in 0..240 {
            player.update_with_water(MovementInput::default(), DT, floor, pool);
            low = low.min(player.position.y);
            high = high.max(player.position.y);
        }
        assert!(high - low < 0.03, "surface bob range {}", high - low);
    }
}

#[test]
fn held_space_ascends_and_shift_dives_then_release_recovers_buoyancy() {
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(
        &mut player,
        MovementInput {
            descend: true,
            ..Default::default()
        },
        1.5,
        floor,
        pool,
    );
    let submerged = player.position.y;
    assert!(submerged < FLOAT_FEET - 3.0);
    assert!(player.is_swimming());
    advance(
        &mut player,
        MovementInput {
            jump: true,
            ..Default::default()
        },
        2.0,
        floor,
        pool,
    );
    assert!(player.position.y > submerged + 2.5);
    assert!(
        player.position.y < SURFACE,
        "ascent should not fly out of open water"
    );
    advance(&mut player, MovementInput::default(), 4.0, floor, pool);
    assert!((player.position.y - FLOAT_FEET).abs() < 0.1);
    assert!(player.is_swimming());
}

#[test]
fn releasing_dive_stops_downward_motion_without_reaching_a_deep_seabed() {
    let mut player = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(
        &mut player,
        MovementInput {
            descend: true,
            ..Default::default()
        },
        1.0,
        floor,
        pool,
    );
    let released = player.position.y;
    let mut lowest = released;
    for _ in 0..120 {
        player.update_with_water(MovementInput::default(), DT, floor, pool);
        lowest = lowest.min(player.position.y);
    }
    assert!(
        released - lowest < 0.3,
        "release coast {}",
        released - lowest
    );
    assert!(player.position.y > released + 0.8);
    assert!(!player.is_grounded());
}

#[test]
fn shallow_partial_water_wades_closer_to_dry_speed_than_full_swimming() {
    let input = MovementInput {
        forward: true,
        ..Default::default()
    };
    let ground = |_, y, _| y < 0;
    let shallow = |_, y, _| water_at(sample(0.25, -1, [0.0; 3]), y);
    let mut dry = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut wading = dry.clone();
    let mut swimmer = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    advance(&mut dry, input, 1.0, ground, |_, _, _| None);
    advance(&mut wading, input, 1.0, ground, shallow);
    advance(&mut swimmer, input, 1.0, floor, pool);
    let dry_distance = 0.5 - dry.position.z;
    let wade_distance = 0.5 - wading.position.z;
    let swim_distance = 0.5 - swimmer.position.z;
    assert!(wade_distance < dry_distance && wade_distance > swim_distance);
    assert!((dry_distance - wade_distance) < (wade_distance - swim_distance));
    assert!(wading.is_grounded());
    assert!(!wading.is_swimming());
    assert!((wading.submersion() - 0.25 / PLAYER_HEIGHT).abs() < 0.0001);
    assert!(swimmer.is_swimming());
}

#[test]
fn a_wading_jump_stays_edge_triggered_and_clearing_input_rearms_it() {
    let ground = |_, y, _| y < 0;
    let shallow = |_, y, _| water_at(sample(0.25, -1, [0.0; 3]), y);
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    let mut player = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut peak = 0.0_f32;
    for _ in 0..360 {
        player.update_with_water(jump, DT, ground, shallow);
        peak = peak.max(player.position.y);
    }
    assert!((1.1..1.25).contains(&peak), "wading jump peak {peak}");
    assert_eq!(player.position.y, 0.0);
    assert!(player.is_grounded());
    assert!(!player.is_swimming());
    player.clear_input();
    player.update_with_water(jump, DT, ground, shallow);
    assert!(player.position.y > 0.0);
    assert!(!player.is_grounded());
}

#[test]
fn swimming_uses_yaw_not_pitch_and_normalizes_diagonals_at_both_speeds() {
    for speed in [
        rustcraft::player::SpeedMultiplier::Normal,
        rustcraft::player::SpeedMultiplier::Double,
    ] {
        let mut forward = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
        forward.yaw = FRAC_PI_2;
        forward.pitch = 1.2;
        let mut diagonal = forward.clone();
        let start = forward.position;
        advance(
            &mut forward,
            MovementInput {
                forward: true,
                speed,
                ..Default::default()
            },
            1.0,
            floor,
            pool,
        );
        advance(
            &mut diagonal,
            MovementInput {
                forward: true,
                right: true,
                speed,
                ..Default::default()
            },
            1.0,
            floor,
            pool,
        );
        let straight_move = forward.position - start;
        let diagonal_move = diagonal.position - start;
        assert_near(straight_move.normalize(), Vec3::X, 0.0001);
        assert_near(
            diagonal_move.normalize(),
            Vec3::new(1.0, 0.0, 1.0).normalize(),
            0.0001,
        );
        assert!((straight_move.length() - diagonal_move.length()).abs() < 0.001);
        let range = if speed == rustcraft::player::SpeedMultiplier::Double {
            4.0..5.0
        } else {
            2.0..2.5
        };
        assert!(
            range.contains(&straight_move.length()),
            "swim distance {straight_move:?}"
        );
    }
}

#[test]
fn three_axis_swimming_is_normalized_and_speed_scales_the_entire_stroke() {
    let mut normal = Player::new(Vec3::new(0.5, -4.0, 0.5));
    let mut boosted = normal.clone();
    let start = normal.position;
    let diagonal = MovementInput {
        forward: true,
        right: true,
        descend: true,
        ..Default::default()
    };
    advance(&mut normal, diagonal, 1.0, floor, pool);
    advance(
        &mut boosted,
        MovementInput {
            speed: rustcraft::player::SpeedMultiplier::Double,
            ..diagonal
        },
        1.0,
        floor,
        pool,
    );
    let normal_move = normal.position - start;
    let boosted_move = boosted.position - start;
    assert!(normal_move.length() < 2.5);
    assert!(boosted_move.length() < 5.0);
    assert_near(
        normal_move.normalize(),
        Vec3::new(1.0, -1.0, -1.0).normalize(),
        0.001,
    );
    assert_near(boosted_move, normal_move * 2.0, 0.001);
}

#[test]
fn partial_surface_and_actual_body_water_are_not_confused_with_remote_water() {
    let partial = sample(4.35, 0, [0.0; 3]);
    let mut player = Player::new(Vec3::new(0.5, 3.2, 0.5));
    player.update_with_water(
        MovementInput::default(),
        0.000_01,
        |_, _, _| false,
        |_, y, _| water_at(partial, y),
    );
    assert!((player.submersion() - 1.15 / PLAYER_HEIGHT).abs() < 0.0001);
    assert!(player.is_swimming());
    let remote = |_, y, _| {
        if y >= 10 {
            Some(sample(20.0, 10, [1.39, 0.0, 0.0]))
        } else {
            None
        }
    };
    let mut dry = Player::new(Vec3::new(0.5, 0.0, 0.5));
    let mut legacy = dry.clone();
    dry.update_with_water(MovementInput::default(), 0.25, |_, y, _| y < 0, remote);
    legacy.update(MovementInput::default(), 0.25, |_, y, _| y < 0);
    assert_near(dry.position, legacy.position, 0.0001);
    assert_eq!(dry.submersion(), 0.0);
    assert!(!dry.is_swimming());
}

#[test]
fn an_air_gap_in_a_water_column_does_not_count_as_full_body_immersion() {
    let floating_layer = |_, y, _| (y == 1).then_some(sample(10.0, 0, [0.0; 3]));
    let mut player = Player::new(Vec3::new(0.5, 0.0, 0.5));
    player.update_with_water(
        MovementInput::default(),
        0.000_01,
        |_, y, _| y < 0,
        floating_layer,
    );
    assert!((player.submersion() - 0.8 / PLAYER_HEIGHT).abs() < 0.0001);
}

#[test]
fn samples_above_their_mean_surface_or_inside_the_solid_bed_are_not_water() {
    for stale in [
        sample(-0.1, -12, [1.39, 0.0, 0.0]),
        sample(4.0, 2, [1.39, 0.0, 0.0]),
    ] {
        let mut player = Player::new(Vec3::new(0.5, 0.0, 0.5));
        player.update_with_water(
            MovementInput::default(),
            0.25,
            |_, y, _| y < 0,
            |_, _, _| Some(stale),
        );
        assert_eq!(player.position, Vec3::new(0.5, 0.0, 0.5));
        assert_eq!(player.submersion(), 0.0);
        assert!(!player.is_swimming());
    }
}

#[test]
fn negative_y_pools_have_the_same_buoyancy_and_flow_as_surface_pools() {
    let mut surface = Player::new(Vec3::new(-512.5, FLOAT_FEET - 1.0, -1024.5));
    let mut cave = surface.clone();
    cave.position.y -= 32.0;
    let current = [-0.5, 0.0, 0.7];
    let surface_water = |_, y, _| water_at(sample(SURFACE, BOTTOM, current), y);
    let cave_water = |_, y, _| water_at(sample(SURFACE - 32.0, BOTTOM - 32, current), y);
    advance(
        &mut surface,
        MovementInput::default(),
        4.0,
        floor,
        surface_water,
    );
    advance(
        &mut cave,
        MovementInput::default(),
        4.0,
        |_, y, _| y < BOTTOM - 32,
        cave_water,
    );
    assert_near(cave.position + Vec3::Y * 32.0, surface.position, 0.003);
    assert!((surface.submersion() - cave.submersion()).abs() < 0.001);
    assert!(cave.is_swimming());
}

#[test]
fn freshwater_and_ocean_metadata_both_support_the_same_exploration_controls() {
    let mut fresh = Player::new(Vec3::new(0.5, FLOAT_FEET, 0.5));
    let mut ocean = fresh.clone();
    let marine = WaterSample {
        biome: WaterBiome::Ocean,
        salinity: 1.0,
        ..sample(SURFACE, BOTTOM, [0.0; 3])
    };
    let input = MovementInput {
        forward: true,
        speed: rustcraft::player::SpeedMultiplier::Double,
        ..Default::default()
    };
    advance(&mut fresh, input, 1.0, floor, pool);
    advance(&mut ocean, input, 1.0, floor, |_, y, _| water_at(marine, y));
    assert_near(ocean.position, fresh.position, 0.0001);
    assert!(fresh.is_swimming() && ocean.is_swimming());
}

#[test]
fn falling_into_water_loses_terminal_momentum_before_reaching_the_pool_bed() {
    let mut player = Player::new(Vec3::new(0.5, 50.0, 0.5));
    let mut lowest = player.position.y;
    for _ in 0..720 {
        player.update_with_water(MovementInput::default(), DT, floor, pool);
        lowest = lowest.min(player.position.y);
        assert!(body_clear(&player, floor));
    }
    assert!(lowest > SURFACE - 3.0, "water entry sank to {lowest}");
    assert!((player.position.y - FLOAT_FEET).abs() < 0.1);
    assert!(player.is_swimming());
    assert!(!player.is_grounded());
}
