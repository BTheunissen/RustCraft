use super::*;
use crate::game::test_support::arena;
use crate::player::MovementStep;

#[test]
fn held_swim_input_against_a_wall_does_not_drain_old_stroke_distance() {
    let mut feedback = Feedback::default();
    let world = arena().world;
    let input = MovementInput {
        forward: true,
        ..Default::default()
    };
    let mut step = MovementStep {
        before: Vec3::ZERO,
        position: Vec3::X * 0.05,
        dt: 0.01,
        supported: false,
        previous_submersion: 0.8,
        submersion: 0.8,
    };
    for _ in 0..40 {
        feedback
            .steps
            .advance(step, &world, input, &mut feedback.queue);
    }
    feedback.take();
    step.position = step.before;
    for _ in 0..100 {
        feedback
            .steps
            .advance(step, &world, input, &mut feedback.queue);
    }
    assert!(feedback.take().iter().all(Option::is_none));
}

#[test]
fn jumped_airborne_distance_does_not_turn_into_landing_footsteps() {
    let mut game = arena();
    for _ in 0..10 {
        game.update(
            MovementInput {
                forward: true,
                jump: true,
                ..Default::default()
            },
            0.05,
        );
    }
    let heard = game.take_sound_events();
    assert!(heard.iter().all(Option::is_none));
    assert!(!game.player.is_grounded());
    game.clear_input();
    game.player.respawn(Vec3::new(8.5, 1.0, 8.5));
    game.update(MovementInput::default(), 0.1);
    assert!(game.take_sound_events().iter().all(Option::is_none));
}

#[test]
fn cpu_queue_keeps_successes_and_single_collection_when_low_priority_overflows() {
    let mut feedback = Feedback::default();
    for index in 0..MAX_SOUND_EVENTS {
        feedback.emit(
            SoundAction::Place(SoundSurface::Wood),
            Vec3::X * index as f32,
        );
    }
    feedback.emit(SoundAction::Step(SoundSurface::Stone), Vec3::ZERO);
    assert!(
        feedback
            .take()
            .iter()
            .flatten()
            .all(|event| matches!(event.action, SoundAction::Place(_)))
    );
    for index in 0..20 {
        feedback.emit(SoundAction::Pickup, Vec3::X * index as f32);
    }
    assert_eq!(feedback.take().into_iter().flatten().count(), 1);
}
