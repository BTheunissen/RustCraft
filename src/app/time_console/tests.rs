use super::*;
use crate::game::{Game, GameMode};
use crate::input::Action;
use crate::options::LaunchOptions;
use winit::keyboard::KeyCode;

fn playing(mode: GameMode) -> App {
    let mut app = App::new(LaunchOptions::default());
    app.game = Game::with_mode(42, mode);
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app
}

#[test]
fn time_console_pauses_simulation_clears_input_and_runs_only_time_commands() {
    for mode in [GameMode::Creative, GameMode::Survival] {
        let mut app = playing(mode);
        app.input.key(KeyCode::KeyW, true, false, 0.0);
        app.input
            .mouse_button(winit::event::MouseButton::Left, true);
        let position = app.game.player.position;
        let hour = app.game.time().hour();
        app.open_time_console();
        assert_eq!(app.screen, Screen::TimeConsole);
        assert!(!app.captured && app.inventory_resume_authorized);
        assert!(!app.input.movement(false).forward && !app.input.mining_held());
        app.simulate(0.25);
        assert_eq!(app.game.player.position, position);
        assert_eq!(app.game.time().hour(), hour);
        assert_eq!(app.session_time, 0.0);
        app.time_console.line = "/time speed 60".into();
        app.submit_time_command();
        assert_eq!(app.game.time().speed(), 60.0);
        assert_eq!(app.game.time().hour(), hour);
        assert_eq!(app.screen, Screen::Paused);
        assert!(!app.captured);
        assert_eq!(app.game.player.position, position);
    }
}

#[test]
fn console_query_errors_focus_loss_and_cancel_cannot_authorize_gameplay() {
    let mut app = playing(GameMode::Creative);
    app.release_pointer();
    app.open_time_console();
    assert!(!app.inventory_resume_authorized);
    app.time_console.line = "/time set night".into();
    app.focused = false;
    app.submit_time_command();
    assert_eq!(app.game.time().hour(), 10.0);
    app.focused = true;
    for command in ["/time", "/time help", "/time speed nan", "/quit"] {
        app.time_console.line = command.into();
        app.submit_time_command();
        assert_eq!(app.screen, Screen::TimeConsole);
        assert!(!app.captured && !app.pointer_authorized);
        assert!(app.time_console.notice.is_some());
        assert_eq!(app.game.time().speed(), 1.0);
    }
    app.escape();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured && !app.pointer_authorized);
    app.time_console.line = "/time set night".into();
    app.submit_time_command();
    assert_eq!(app.game.time().hour(), 10.0);
}

#[test]
fn only_an_active_or_paused_world_can_open_the_debug_console() {
    let mut app = playing(GameMode::Creative);
    for screen in [
        Screen::MainMenu,
        Screen::Loading,
        Screen::Inventory,
        Screen::Crafting,
        Screen::Storage,
    ] {
        app.screen = screen;
        app.open_time_console();
        assert_eq!(app.screen, screen);
    }
    app.screen = Screen::Playing;
    app.captured = false;
    app.open_time_console();
    assert_eq!(app.screen, Screen::Playing);
    app.captured = true;
    app.options.smoke_test = true;
    app.open_time_console();
    assert_eq!(app.screen, Screen::Playing);
    app.options.smoke_test = false;
    app.focused = false;
    app.open_time_console();
    assert_eq!(app.screen, Screen::Playing);
}

#[test]
fn command_text_has_a_fixed_ascii_bound_and_preserves_ordinary_spaces() {
    let mut console = TimeConsole::default();
    console.push_text("speed 60\n\r\t\u{0}é");
    assert_eq!(console.line, "/time speed 60");
    console.push_text(&"x".repeat(1000));
    assert_eq!(console.line.len(), MAX_COMMAND_CHARS);
    console.push_text("ignored");
    assert_eq!(console.line.len(), MAX_COMMAND_CHARS);
}

#[test]
fn time_speed_keys_are_capture_gated_and_cannot_change_movement_or_animation_speed() {
    let mut app = playing(GameMode::Creative);
    let movement_speed = app.game.speed();
    app.gameplay_action(&Action::CycleTimeSpeed);
    assert_eq!(app.game.time().speed(), 10.0);
    assert_eq!(app.game.speed(), movement_speed);
    app.game.player.flying = true;
    let position = app.game.player.position;
    let hour = app.game.time().hour();
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.simulate(0.1);
    assert!(((app.game.player.position - position).length() - 0.5).abs() < 0.0001);
    assert!((app.session_time - 0.1).abs() < 0.00001);
    assert!((app.game.time().hour() - hour - 0.02).abs() < 0.00001);
    app.release_pointer();
    app.gameplay_action(&Action::CycleTimeSpeed);
    assert_eq!(app.game.time().speed(), 10.0);
    let frozen = app.game.time();
    app.simulate(0.25);
    assert_eq!(app.game.time(), frozen);
}

#[test]
fn every_overlay_and_focus_loss_freezes_the_solar_clock() {
    let mut app = playing(GameMode::Survival);
    app.game.time_mut().set_speed(120.0).unwrap();
    for screen in [
        Screen::Paused,
        Screen::Inventory,
        Screen::Crafting,
        Screen::Storage,
        Screen::TimeConsole,
        Screen::Loading,
    ] {
        app.screen = screen;
        let time = app.game.time();
        app.simulate(0.25);
        assert_eq!(app.game.time(), time, "{screen:?}");
    }
    app.screen = Screen::Playing;
    app.focused = false;
    let time = app.game.time();
    app.simulate(0.25);
    assert_eq!(app.game.time(), time);
}

#[test]
fn logical_keyboard_text_backspace_repeats_and_enter_never_reach_world_actions() {
    let mut app = playing(GameMode::Survival);
    app.open_time_console();
    app.console_key(&Key::Character("s".into()), Some("speed 60"), true, false);
    assert_eq!(app.time_console.line, "/time speed 60");
    app.console_key(&Key::Named(NamedKey::Enter), None, true, true);
    app.console_key(&Key::Named(NamedKey::Enter), None, false, false);
    assert_eq!(app.game.time().speed(), 1.0);
    app.console_key(&Key::Named(NamedKey::ArrowUp), None, true, false);
    app.console_key(&Key::Character("q".into()), None, true, false);
    assert_eq!(app.screen, Screen::TimeConsole);
    app.console_key(&Key::Named(NamedKey::Backspace), None, true, true);
    assert_eq!(app.time_console.line, "/time speed 6");
    app.console_key(&Key::Character("0".into()), Some("0"), true, false);
    app.console_key(&Key::Named(NamedKey::Enter), None, true, false);
    assert_eq!(app.game.time().speed(), 60.0);
    assert_eq!(app.screen, Screen::Paused);
    app.console_key(&Key::Character("w".into()), Some("w"), true, false);
    assert_eq!(app.time_console.line, "/time speed 60");
}

#[test]
fn focus_loss_while_typing_removes_recapture_authorization_and_blocks_text() {
    let mut app = playing(GameMode::Creative);
    app.open_time_console();
    assert!(app.inventory_resume_authorized);
    app.focused = false;
    app.release_pointer();
    assert!(!app.inventory_resume_authorized && !app.pointer_authorized);
    let line = app.time_console.line.clone();
    app.console_key(&Key::Character("x".into()), Some("x"), true, false);
    assert_eq!(app.time_console.line, line);
    app.focused = true;
    app.console_key(&Key::Named(NamedKey::Escape), None, true, true);
    assert_eq!(app.screen, Screen::TimeConsole);
    app.console_key(&Key::Named(NamedKey::Escape), None, true, false);
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured && !app.pointer_authorized);
}
