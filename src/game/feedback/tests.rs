use super::*;
use crate::game::test_support::{aim, arena};
use crate::game::{Game, GameMode, MAX_STACK, Pickup};
use crate::player::{MovementStep, SpeedMultiplier};
use crate::world::{ChunkCoord, WaterSample};

fn events(game: &mut Game) -> Vec<SoundEvent> {
    game.take_sound_events().into_iter().flatten().collect()
}

fn walking(partition: f32) -> (Vec3, Vec<SoundEvent>) {
    let mut game = arena();
    let mut result = Vec::new();
    for _ in 0..(1.20 / partition).round() as usize {
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            partition,
        );
        result.extend(events(&mut game));
    }
    (game.player.position, result)
}

#[test]
fn footsteps_follow_actual_supported_distance_across_frame_partitions() {
    let (slow_pose, slow) = walking(0.20);
    let (fast_pose, fast) = walking(0.01);
    assert!((slow_pose - fast_pose).length() < 0.001);
    assert_eq!(slow.len(), 3);
    assert_eq!(slow.len(), fast.len());
    assert!(
        slow.iter()
            .chain(&fast)
            .all(|event| event.action == SoundAction::Step(SoundSurface::Stone))
    );
    for (slow, fast) in slow.iter().zip(fast) {
        assert!((slow.position - fast.position).length() < 0.06);
    }
}

#[test]
fn stationary_walls_air_flight_and_invalid_time_do_not_make_false_steps() {
    let mut game = arena();
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, 0.25] {
        game.update(MovementInput::default(), dt);
    }
    assert!(events(&mut game).is_empty());
    for x in 0..16 {
        for y in 1..=3 {
            game.world.edit_block([x, y, 6], Block::Stone);
        }
    }
    for _ in 0..30 {
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            0.1,
        );
    }
    assert!(events(&mut game).is_empty());
    game.player.respawn(Vec3::new(8.5, 20.0, 8.5));
    game.update(
        MovementInput {
            forward: true,
            ..Default::default()
        },
        0.25,
    );
    assert!(events(&mut game).is_empty());
    game.player.flying = true;
    game.update(
        MovementInput {
            forward: true,
            speed: SpeedMultiplier::Sixteen,
            ..Default::default()
        },
        0.25,
    );
    assert!(events(&mut game).is_empty());
}

#[test]
fn edited_support_and_negative_edge_footprints_choose_actual_material() {
    let mut game = arena();
    for x in 0..16 {
        for z in 0..16 {
            game.world.edit_block([x, 0, z], Block::Wood);
        }
    }
    for _ in 0..5 {
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            0.1,
        );
    }
    assert_eq!(
        events(&mut game)[0].action,
        SoundAction::Step(SoundSurface::Wood)
    );
    let world = crate::game::test_support::world(
        &[ChunkCoord::new(-1, -1), ChunkCoord::new(0, -1)],
        &[([-1, -1, -1], Block::Sand)],
    );
    assert_eq!(
        movement::supporting_surface(&world, Vec3::new(-0.1, 0.0, -0.5)),
        Some(SoundSurface::Sand)
    );
    assert_eq!(movement::supporting_surface(&world, Vec3::NAN), None);
    for block in Block::ALL {
        assert_eq!(SoundSurface::from_block(block).is_some(), block.is_solid());
    }
}

#[test]
fn water_entry_has_hysteresis_and_current_only_swimming_has_no_stroke() {
    let mut feedback = Feedback::default();
    let world = arena().world;
    let mut step = MovementStep {
        before: Vec3::ZERO,
        position: Vec3::X * 0.1,
        dt: 0.01,
        supported: false,
        previous_submersion: 0.0,
        submersion: 0.2,
    };
    feedback
        .steps
        .advance(step, &world, MovementInput::default(), &mut feedback.queue);
    step.previous_submersion = 0.2;
    step.submersion = 0.15;
    for _ in 0..100 {
        feedback
            .steps
            .advance(step, &world, MovementInput::default(), &mut feedback.queue);
    }
    assert_eq!(
        feedback
            .take()
            .into_iter()
            .flatten()
            .filter(|event| event.action == SoundAction::EnterWater)
            .count(),
        1
    );
    step.previous_submersion = 0.8;
    step.submersion = 0.8;
    for _ in 0..100 {
        feedback
            .steps
            .advance(step, &world, MovementInput::default(), &mut feedback.queue);
    }
    assert!(feedback.take().iter().all(Option::is_none));
    for _ in 0..100 {
        feedback.steps.advance(
            step,
            &world,
            MovementInput {
                forward: true,
                ..Default::default()
            },
            &mut feedback.queue,
        );
    }
    let strokes = feedback
        .take()
        .into_iter()
        .flatten()
        .filter(|event| event.action == SoundAction::Swim)
        .count();
    assert!((1..=3).contains(&strokes));
    step.submersion = 0.0;
    feedback
        .steps
        .advance(step, &world, MovementInput::default(), &mut feedback.queue);
    step.previous_submersion = 0.0;
    step.submersion = 0.2;
    feedback
        .steps
        .advance(step, &world, MovementInput::default(), &mut feedback.queue);
    assert_eq!(
        feedback
            .take()
            .into_iter()
            .flatten()
            .filter(|event| event.action == SoundAction::EnterWater)
            .count(),
        1
    );
}

#[test]
fn actual_resident_pool_emits_entry_once_and_never_emits_dry_steps() {
    let mut game = arena();
    for x in 7..=9 {
        for z in 3..=7 {
            for y in 1..=3 {
                game.world.edit_block([x, y, z], Block::Water);
            }
        }
    }
    let mut heard = Vec::new();
    for _ in 0..20 {
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            0.05,
        );
        heard.extend(events(&mut game));
    }
    assert!(game.player.is_swimming());
    assert_eq!(
        heard
            .iter()
            .filter(|event| event.action == SoundAction::EnterWater)
            .count(),
        1
    );
    assert!(
        !heard
            .iter()
            .any(|event| matches!(event.action, SoundAction::Step(_)))
    );
}

#[test]
fn mining_success_and_rejected_interactions_have_exact_feedback() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Dirt);
    let mut heard = Vec::new();
    for _ in 0..60 {
        game.interact(true, false, 0.01);
        heard.extend(events(&mut game));
    }
    assert_eq!(
        heard
            .iter()
            .filter(|event| matches!(event.action, SoundAction::Break(_)))
            .count(),
        1
    );
    let ticks = heard
        .iter()
        .filter(|event| matches!(event.action, SoundAction::Mine(_)))
        .count();
    assert!((1..=3).contains(&ticks));
    assert!(matches!(
        heard.last().unwrap().action,
        SoundAction::Break(SoundSurface::Earth)
    ));
    assert!(!game.place_selected());
    assert!(events(&mut game).is_empty());
    game.world.edit_block([8, 2, 6], Block::Bedrock);
    game.interact(true, false, 0.25);
    assert!(events(&mut game).is_empty());
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory.insert(Block::Wood, 1);
    assert!(game.place_selected());
    let placed = events(&mut game);
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].action, SoundAction::Place(SoundSurface::Wood));
    assert!(!game.place_selected());
    assert!(events(&mut game).is_empty());
}

#[test]
fn target_switching_cannot_bypass_mining_cooldown_and_bad_dt_is_silent() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.world.edit_block([9, 2, 6], Block::Wood);
    let mut ticks = 0;
    for index in 0..100 {
        aim(
            &mut game,
            Vec3::new(if index % 2 == 0 { 8.5 } else { 9.5 }, 2.5, 6.5),
        );
        game.cancel_mining();
        game.interact(true, false, 0.01);
        ticks += events(&mut game)
            .iter()
            .filter(|event| matches!(event.action, SoundAction::Mine(_)))
            .count();
    }
    assert!(ticks <= 5);
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        game.interact(true, false, dt);
        assert!(events(&mut game).is_empty());
    }
}

#[test]
fn pickup_only_sounds_after_positive_transfer_and_coalesces_multiple_stacks() {
    let mut game = arena();
    game.pickups.push(Pickup {
        position: game.player.position + Vec3::Y * 0.4,
        item: Block::Stone.into(),
        count: 1,
        velocity: Vec3::ZERO,
        age: 1.0,
    });
    game.pickups.push(Pickup {
        position: game.player.position + Vec3::Y * 0.4,
        item: Block::Wood.into(),
        count: 1,
        velocity: Vec3::ZERO,
        age: 1.0,
    });
    game.interact(false, false, 0.01);
    assert_eq!(
        events(&mut game)
            .iter()
            .filter(|event| event.action == SoundAction::Pickup)
            .count(),
        1
    );
    assert!(game.pickups.is_empty());
    game.inventory.insert(Block::Sand, MAX_STACK * 36);
    game.inventory.insert(Block::Stone, MAX_STACK);
    game.inventory.insert(Block::Wood, MAX_STACK);
    game.pickups.push(Pickup {
        position: game.player.position + Vec3::Y * 0.4,
        item: Block::Dirt.into(),
        count: 1,
        velocity: Vec3::ZERO,
        age: 1.0,
    });
    game.interact(false, false, 0.01);
    assert!(events(&mut game).is_empty());
    assert_eq!(game.pickups[0].count, 1);
}

#[test]
fn feedback_is_bounded_without_a_consumer_and_never_survives_snapshot_or_pause() {
    let mut feedback = Feedback::default();
    for index in 0..1000 {
        feedback.emit(
            SoundAction::Step(SoundSurface::Stone),
            Vec3::X * index as f32,
        );
    }
    feedback.emit(SoundAction::Break(SoundSurface::Wood), Vec3::ZERO);
    let heard: Vec<_> = feedback.take().into_iter().flatten().collect();
    assert_eq!(heard.len(), MAX_SOUND_EVENTS);
    assert!(
        heard
            .iter()
            .any(|event| event.action == SoundAction::Break(SoundSurface::Wood))
    );
    feedback.emit(SoundAction::Pickup, Vec3::NAN);
    assert!(feedback.take().iter().all(Option::is_none));
    let mut game = arena();
    game.feedback.emit(SoundAction::Pickup, Vec3::ZERO);
    let snapshot = game.snapshot();
    let mut restored = Game::from_snapshot(snapshot).unwrap();
    assert!(events(&mut restored).is_empty());
    game.clear_input();
    assert!(events(&mut game).is_empty());
    game.feedback.emit(SoundAction::Pickup, Vec3::ZERO);
    game.respawn();
    assert!(events(&mut game).is_empty());
    let mut creative = Game::with_mode(42, GameMode::Creative);
    creative.interact(true, true, 0.25);
    assert!(events(&mut creative).is_empty());
}

#[test]
fn player_report_bounds_stall_time_without_querying_water_during_flight() {
    let mut player = crate::player::Player::new(Vec3::new(0.5, 1.0, 0.5));
    let report = player.update_observed(
        MovementInput {
            forward: true,
            ..Default::default()
        },
        100.0,
        |_, y, _| y == 0,
        |_, _, _| None::<WaterSample>,
    );
    assert_eq!(report.steps.iter().flatten().count(), 30);
    assert!(
        (report
            .steps
            .iter()
            .flatten()
            .map(|step| step.dt)
            .sum::<f32>()
            - 0.25)
            .abs()
            < 0.000_001
    );
    player.flying = true;
    let report = player.update_observed(
        MovementInput::default(),
        0.1,
        |_, _, _| panic!("flight queried solids"),
        |_, _, _| panic!("flight queried water"),
    );
    assert!(report.steps.iter().all(Option::is_none));
}
