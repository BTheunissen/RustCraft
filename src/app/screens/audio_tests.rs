use super::*;
use crate::input::Action;
use crate::options::LaunchOptions;
use crate::save::test_support::TestDirectory;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

fn paused() -> App {
    let mut app = App::new(LaunchOptions::default());
    app.screen = Screen::Paused;
    app.focused = true;
    app
}

#[test]
fn opening_adjusting_and_closing_audio_never_captures_or_leaks_held_input() {
    let mut app = paused();
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.input.mouse_button(MouseButton::Left, true);
    app.input.mouse_button(MouseButton::Right, true);
    assert!(!app.apply_button(3).unwrap());
    assert!(matches!(app.screen, Screen::Audio(_)));
    assert!(!app.captured && !app.pointer_authorized);
    assert!(!app.input.mining_held() && !app.input.take_place_press());
    assert_eq!(app.click_hit(Some(UiHit::Button(0))), Some(0));
    assert!(!app.pointer_authorized);
    app.apply_button(0).unwrap();
    assert!(app.audio.settings().muted);
    app.apply_button(1).unwrap();
    assert!((app.audio.settings().master - 0.5).abs() < 0.00001);
    let pose = app.game.player.position;
    app.gameplay_action(&Action::Respawn);
    app.simulate(0.1);
    assert_eq!(app.game.player.position, pose);
    app.escape();
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(app.focused_button, 3);
    assert!(!app.captured && !app.pointer_authorized);
    assert!(!app.input.movement(false).forward);
}

#[test]
fn independent_gains_are_clamped_and_keyboard_focus_wraps_without_automatic_resume() {
    let mut app = paused();
    app.apply_button(3).unwrap();
    app.navigate(-1);
    assert_eq!(app.focused_button, 7);
    for _ in 0..100 {
        app.adjust_audio(2);
        app.adjust_audio(3);
        app.adjust_audio(6);
    }
    let settings = app.audio.settings();
    assert_eq!(
        (settings.master, settings.actions, settings.ambience),
        (1.0, 0.0, 1.0)
    );
    for _ in 0..100 {
        app.adjust_audio(1);
        app.adjust_audio(4);
        app.adjust_audio(5);
    }
    let settings = app.audio.settings();
    assert_eq!(
        (settings.master, settings.actions, settings.ambience),
        (0.0, 1.0, 0.0)
    );
    app.apply_button(7).unwrap();
    assert_eq!(app.screen, Screen::Paused);
    app.resume_play();
    assert!(!app.captured && !app.pointer_authorized);
}

#[test]
fn controls_persist_separately_and_failed_preferences_cannot_break_world_saves() {
    let directory = TestDirectory::new();
    let prefs = directory.path.join("audio.prefs");
    let mut app = paused();
    app.audio.preferences = crate::app::audio::preferences::tests::isolated(prefs.clone());
    app.apply_button(3).unwrap();
    app.apply_button(0).unwrap();
    app.apply_button(6).unwrap();
    assert!(prefs.exists());
    let saved = app.audio.settings();
    let mut second = crate::app::audio::preferences::tests::isolated(prefs.clone());
    assert_eq!(second.load(), saved);
    app.persistence = crate::app::persistence::Persistence::with_slot(directory.slot());
    app.persistence.begin_world(std::time::Instant::now());
    std::fs::remove_file(&prefs).unwrap();
    std::fs::create_dir(&prefs).unwrap();
    app.apply_button(1).unwrap();
    let Screen::Audio(controls) = app.screen else {
        panic!("controls closed on failure")
    };
    assert!(controls.preferences_unavailable);
    let snapshot = app.game.snapshot();
    assert!(app.save_before_exit());
    assert_eq!(directory.slot().load().unwrap().unwrap(), snapshot);
}

#[test]
fn a_failed_world_save_leaves_audio_panel_for_the_visible_paused_error_notice() {
    let directory = TestDirectory::new();
    std::fs::create_dir(directory.slot().path()).unwrap();
    let mut app = paused();
    app.persistence = crate::app::persistence::Persistence::with_slot(directory.slot());
    app.persistence.begin_world(std::time::Instant::now());
    app.apply_button(3).unwrap();
    assert!(!app.save_before_exit());
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(
        app.persistence.notice,
        Some(crate::hud::SaveNotice::SaveFailed)
    );
    assert!(!app.captured && !app.pointer_authorized);
}
