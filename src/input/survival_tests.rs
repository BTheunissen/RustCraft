use super::{Action, InputState};
use winit::{event::MouseButton, keyboard::KeyCode};

#[test]
fn survival_double_space_keeps_jump_edges_without_enabling_flight() {
    let mut input = InputState::default();
    input.key_for_mode(KeyCode::Space, true, false, 0.0, false);
    input.key_for_mode(KeyCode::Space, false, false, 0.02, false);
    assert_eq!(
        input.key_for_mode(KeyCode::Space, true, false, 0.1, false),
        None
    );
    assert!(input.take_jump_release());
    assert!(input.movement(false).jump);
    assert_eq!(
        input.key_for_mode(KeyCode::KeyF, true, false, 0.2, false),
        None
    );
    assert_eq!(
        input.key_for_mode(KeyCode::ControlLeft, true, false, 0.3, false),
        None
    );
}

#[test]
fn repeated_inventory_keys_and_hotbar_releases_do_not_emit_actions() {
    let mut input = InputState::default();
    assert_eq!(
        input.key(KeyCode::KeyI, true, false, 0.0),
        Some(Action::Inventory)
    );
    assert_eq!(input.key(KeyCode::KeyI, true, true, 0.1), None);
    assert_eq!(
        input.key(KeyCode::Digit9, true, false, 0.2),
        Some(Action::SelectSlot(8))
    );
    assert_eq!(input.key(KeyCode::Digit9, false, false, 0.3), None);
    input.key(KeyCode::ShiftLeft, true, false, 0.4);
    assert_eq!(
        input.key(KeyCode::Tab, true, false, 0.5),
        Some(Action::Navigate(-1))
    );
}

#[test]
fn each_hotbar_key_and_menu_navigation_emit_one_logical_action() {
    let mut input = InputState::default();
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            input.key(key, true, false, 0.0),
            Some(Action::SelectSlot(index))
        );
        assert_eq!(input.key(key, false, false, 0.0), None);
    }
    for (key, action) in [
        (KeyCode::ArrowUp, Action::Navigate(-1)),
        (KeyCode::ArrowDown, Action::Navigate(1)),
        (KeyCode::Tab, Action::Navigate(1)),
        (KeyCode::Enter, Action::Activate),
        (KeyCode::NumpadEnter, Action::Activate),
    ] {
        assert_eq!(input.key(key, true, false, 0.0), Some(action));
        assert_eq!(input.key(key, false, false, 0.0), None);
    }
}

#[test]
fn placement_is_edge_triggered_and_fast_mining_release_is_not_lost() {
    let mut input = InputState::default();
    input.mouse_button(MouseButton::Left, true);
    input.mouse_button(MouseButton::Right, true);
    assert!(input.mining_held());
    assert!(input.take_place_press());
    input.mouse_button(MouseButton::Right, true);
    assert!(!input.take_place_press());
    input.mouse_button(MouseButton::Left, false);
    input.mouse_button(MouseButton::Left, true);
    assert!(input.mining_held());
    assert!(input.take_mining_release());
    assert!(!input.take_mining_release());
    input.mouse_button(MouseButton::Right, false);
    input.mouse_button(MouseButton::Right, true);
    assert!(input.take_place_press());
    input.clear();
    assert!(!input.mining_held());
    assert!(!input.take_place_press());
    assert!(!input.take_mining_release());
}

#[test]
fn a_quick_placement_click_is_preserved_without_repeating_while_held() {
    let mut input = InputState::default();
    input.mouse_button(MouseButton::Right, true);
    input.mouse_button(MouseButton::Right, false);
    assert!(input.take_place_press());
    assert!(!input.take_place_press());
    input.mouse_button(MouseButton::Middle, true);
    assert!(!input.mining_held());
    assert!(!input.take_place_press());
}
#[test]
fn crate_placement_modifier_is_captured_at_press_even_when_shift_changes_between_updates() {
    use winit::event::MouseButton;
    let mut input = super::InputState::default();
    input.key_for_mode(winit::keyboard::KeyCode::ShiftLeft, true, false, 0.0, false);
    input.mouse_button(MouseButton::Right, true);
    input.mouse_button(MouseButton::Right, false);
    input.key_for_mode(
        winit::keyboard::KeyCode::ShiftLeft,
        false,
        false,
        0.01,
        false,
    );
    assert!(input.take_place_press() && input.place_modifier());
    input.mouse_button(MouseButton::Right, true);
    input.key_for_mode(
        winit::keyboard::KeyCode::ShiftLeft,
        true,
        false,
        0.02,
        false,
    );
    assert!(input.take_place_press() && !input.place_modifier());
    input.clear();
    assert!(!input.take_place_press() && !input.place_modifier());
}
