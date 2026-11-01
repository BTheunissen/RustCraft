use super::{Action, InputState};
use winit::keyboard::KeyCode;

#[test]
fn time_debug_actions_are_edge_triggered_and_available_in_both_modes() {
    for creative in [false, true] {
        let mut input = InputState::default();
        for (key, action) in [
            (KeyCode::F6, Action::CycleTimeSpeed),
            (KeyCode::Backquote, Action::TimeConsole),
            (KeyCode::Slash, Action::TimeConsole),
        ] {
            assert_eq!(input.key_for_mode(key, false, false, 0.0, creative), None);
            assert_eq!(input.key_for_mode(key, true, true, 0.0, creative), None);
            assert_eq!(
                input.key_for_mode(key, true, false, 0.0, creative),
                Some(action)
            );
        }
        assert!(!input.movement(false).forward);
    }
}

#[test]
fn movement_keys_are_independent_and_releases_apply() {
    let mut state = InputState::default();
    state.key(KeyCode::KeyW, true, false, 0.0);
    state.key(KeyCode::KeyD, true, false, 0.0);
    let input = state.movement(false);
    assert!(input.forward && input.right);
    assert!(!input.backward && !input.left);
    state.key(KeyCode::KeyW, false, false, 0.1);
    let input = state.movement(false);
    assert!(!input.forward && input.right);
}

#[test]
fn releasing_one_modifier_does_not_release_the_other() {
    let mut state = InputState::default();
    for key in [
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ] {
        state.key(key, true, false, 0.0);
    }
    state.key(KeyCode::ShiftLeft, false, false, 0.1);
    state.key(KeyCode::ControlLeft, false, false, 0.1);
    let input = state.movement(true);
    assert!(input.descend);
}

#[test]
fn swimming_gets_held_ascent_and_descent_without_requiring_flight() {
    let mut state = InputState::default();
    state.key(KeyCode::Space, true, false, 1.0);
    state.key(KeyCode::ShiftLeft, true, false, 1.0);
    for _ in 0..3 {
        let input = state.movement(false);
        assert!(input.jump && input.descend);
    }
    state.key(KeyCode::Space, false, false, 2.0);
    state.key(KeyCode::ShiftLeft, false, false, 2.0);
    let input = state.movement(false);
    assert!(!input.jump && !input.descend);
}

#[test]
fn quick_jump_taps_survive_between_frames_and_flight_uses_held_input() {
    let mut state = InputState::default();
    state.key(KeyCode::Space, true, false, 0.0);
    state.key(KeyCode::Space, false, false, 0.1);
    assert!(state.movement(false).jump);
    assert!(!state.movement(false).jump);
    state.key(KeyCode::Space, true, false, 1.0);
    assert!(state.movement(true).jump);
    assert!(state.movement(true).jump);
    state.key(KeyCode::Space, false, false, 1.1);
    assert!(!state.movement(true).jump);
}

#[test]
fn double_space_toggles_once_but_repeat_does_not() {
    let mut state = InputState::default();
    assert_eq!(state.key(KeyCode::Space, true, false, 1.0), None);
    state.key(KeyCode::Space, false, false, 1.05);
    assert_eq!(
        state.key(KeyCode::Space, true, false, 1.2),
        Some(Action::ToggleFlight)
    );
    assert_eq!(state.key(KeyCode::Space, true, true, 1.21), None);
    state.key(KeyCode::Space, false, false, 1.22);
    assert_eq!(state.key(KeyCode::Space, true, false, 2.0), None);
}

#[test]
fn headlamp_toggles_only_on_nonrepeated_presses() {
    let mut state = InputState::default();
    assert_eq!(state.key(KeyCode::KeyH, false, false, 0.0), None);
    assert_eq!(
        state.key(KeyCode::KeyH, true, false, 0.1),
        Some(Action::ToggleHeadlamp)
    );
    assert_eq!(state.key(KeyCode::KeyH, true, true, 0.2), None);
    assert_eq!(state.key(KeyCode::KeyH, false, false, 0.3), None);
    assert_eq!(
        state.key(KeyCode::KeyH, true, false, 0.4),
        Some(Action::ToggleHeadlamp)
    );
    assert!(!state.movement(false).jump);
    assert!(!state.take_jump_release());
}

#[test]
fn headlamp_actions_preserve_pending_jump_edges_and_double_space() {
    let mut state = InputState::default();
    state.key(KeyCode::Space, true, false, 1.0);
    state.key(KeyCode::Space, false, false, 1.04);
    assert_eq!(
        state.key(KeyCode::KeyH, true, false, 1.05),
        Some(Action::ToggleHeadlamp)
    );
    state.key(KeyCode::KeyH, true, true, 1.06);
    state.key(KeyCode::KeyH, false, false, 1.07);
    assert!(state.take_jump_release());
    assert!(!state.take_jump_release());
    assert!(state.movement(false).jump);
    assert!(!state.movement(false).jump);
    assert_eq!(
        state.key(KeyCode::Space, true, false, 1.2),
        Some(Action::ToggleFlight)
    );
}

#[test]
fn release_then_repress_between_updates_rearms_the_controller() {
    use crate::player::{MovementInput, Player};
    use glam::Vec3;
    let mut state = InputState::default();
    let mut player = Player::new(Vec3::ZERO);
    let solid = |_x, y, _z| y < 0;
    state.key(KeyCode::Space, true, false, 0.0);
    for _ in 0..120 {
        player.update(state.movement(false), 1.0 / 60.0, solid);
    }
    assert!(player.is_grounded());
    state.key(KeyCode::Space, false, false, 2.0);
    state.key(KeyCode::Space, true, false, 2.01);
    assert!(state.take_jump_release());
    player.clear_input();
    player.update(state.movement(false), 1.0 / 60.0, solid);
    assert!(player.position.y > 0.0);
    assert!(!state.take_jump_release());
    player.update(MovementInput::default(), 1.0 / 60.0, solid);
}

#[test]
fn actions_only_fire_on_press_and_clear_drops_all_keys() {
    let mut state = InputState::default();
    for (key, expected) in [
        (KeyCode::Escape, Action::Pause),
        (KeyCode::KeyF, Action::ToggleFlight),
        (KeyCode::KeyH, Action::ToggleHeadlamp),
        (KeyCode::KeyR, Action::Respawn),
        (KeyCode::F11, Action::Fullscreen),
        (KeyCode::KeyQ, Action::Quit),
    ] {
        assert_eq!(state.key(key, false, false, 0.0), None);
        assert_eq!(state.key(key, true, true, 0.0), None);
        assert_eq!(state.key(key, true, false, 0.0), Some(expected));
    }
    state.key(KeyCode::KeyW, true, false, 0.0);
    state.key(KeyCode::Space, true, false, 0.0);
    state.key(KeyCode::ShiftLeft, true, false, 0.0);
    state.clear();
    let input = state.movement(true);
    assert!(!input.forward && !input.jump && !input.descend);
}

#[test]
fn control_cycles_once_per_physical_press_not_repeat_release_or_dual_modifiers() {
    let mut input = InputState::default();
    assert_eq!(
        input.key(KeyCode::ControlLeft, true, false, 0.0),
        Some(Action::CycleSpeed)
    );
    assert_eq!(input.key(KeyCode::ControlLeft, true, true, 0.1), None);
    assert_eq!(input.key(KeyCode::ControlLeft, true, false, 0.2), None);
    assert_eq!(input.key(KeyCode::ControlRight, true, false, 0.3), None);
    assert_eq!(input.key(KeyCode::ControlLeft, false, false, 0.4), None);
    assert_eq!(input.key(KeyCode::ControlRight, false, false, 0.5), None);
    assert_eq!(
        input.key(KeyCode::ControlRight, true, false, 0.6),
        Some(Action::CycleSpeed)
    );
    input.clear();
    assert_eq!(
        input.key(KeyCode::ControlLeft, true, false, 0.7),
        Some(Action::CycleSpeed)
    );
    assert_eq!(
        input.movement(false).speed,
        crate::player::SpeedMultiplier::Normal
    );
}

#[test]
fn control_action_preserves_jump_release_press_and_double_space_edges() {
    let mut input = InputState::default();
    input.key(KeyCode::Space, true, false, 1.0);
    input.key(KeyCode::Space, false, false, 1.02);
    assert_eq!(
        input.key(KeyCode::ControlLeft, true, false, 1.03),
        Some(Action::CycleSpeed)
    );
    assert!(input.take_jump_release());
    assert!(input.movement(false).jump);
    assert_eq!(
        input.key(KeyCode::Space, true, false, 1.2),
        Some(Action::ToggleFlight)
    );
}

#[test]
fn held_control_across_many_frames_never_sets_a_transient_speed_or_repeats_actions() {
    let mut input = InputState::default();
    assert_eq!(
        input.key(KeyCode::ControlLeft, true, false, 0.0),
        Some(Action::CycleSpeed)
    );
    for frame in 0..120 {
        assert_eq!(
            input.key(KeyCode::ControlLeft, true, true, frame as f64 / 60.0),
            None
        );
        assert_eq!(
            input.movement(true).speed,
            crate::player::SpeedMultiplier::Normal
        );
    }
    assert_eq!(input.key(KeyCode::ControlLeft, false, false, 3.0), None);
    assert_eq!(
        input.key(KeyCode::ControlLeft, true, false, 3.1),
        Some(Action::CycleSpeed)
    );
}
