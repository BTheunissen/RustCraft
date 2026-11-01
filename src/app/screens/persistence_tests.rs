use std::fs;

use crate::app::App;
use crate::app::persistence::Persistence;
use crate::game::{Game, GameMode};
use crate::hud::{SaveNotice, Screen, UiHit};
use crate::options::LaunchOptions;
use crate::save::test_support::TestDirectory;
use crate::world::Block;

fn menu(directory: &TestDirectory) -> App {
    let mut app = App::new(LaunchOptions {
        seed: 42,
        render_distance: 2,
        ..Default::default()
    });
    app.persistence = Persistence::with_slot(directory.slot());
    app
}

#[test]
fn mode_button_commits_empty_slot_but_disabled_continue_and_loading_buttons_are_inert() {
    for (index, mode) in [(1, GameMode::Survival), (2, GameMode::Creative)] {
        let directory = TestDirectory::new();
        let mut app = menu(&directory);
        let preview = app.game.snapshot();
        assert!(!app.apply_button(0).unwrap());
        assert_eq!(app.game.snapshot(), preview);
        assert!(!app.apply_button(99).unwrap());
        assert!(!app.apply_button(index).unwrap());
        assert_eq!(app.screen, Screen::Loading);
        assert_eq!(app.game.mode(), mode);
        assert_eq!(directory.slot().load().unwrap().unwrap().mode, mode);
        assert!(!app.apply_button(0).unwrap());
        assert_eq!(app.screen, Screen::Loading);
    }
}

#[test]
fn only_confirmed_replacement_changes_slot_and_cancel_is_the_default_action() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let original = Game::new(99).snapshot();
    slot.write(&original).unwrap();
    let mut app = menu(&directory);
    let index = app.click_hit(Some(UiHit::Button(1))).unwrap();
    assert!(app.pointer_authorized);
    assert!(!app.apply_button(index).unwrap());
    assert_eq!(app.screen, Screen::ConfirmReplace(GameMode::Survival));
    assert!(!app.pointer_authorized);
    assert!(!app.apply_button(app.focused_button).unwrap());
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(slot.load().unwrap().unwrap(), original);
    app.apply_button(1).unwrap();
    let index = app.click_hit(Some(UiHit::Button(0))).unwrap();
    assert!(!app.apply_button(index).unwrap());
    assert_eq!(app.screen, Screen::Loading);
    assert!(app.pointer_authorized);
    assert_eq!(slot.load().unwrap().unwrap(), app.game.snapshot());
}

#[test]
fn save_menu_continue_resume_and_quit_buttons_preserve_progress_and_clear_world_inputs() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut app = menu(&directory);
    app.apply_button(1).unwrap();
    app.game.inventory_mut().insert(Block::Clay, 12);
    app.game.inventory_mut().select(8);
    let saved = app.game.snapshot();
    app.screen = Screen::Paused;
    assert!(!app.apply_button(0).unwrap());
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.apply_button(1).unwrap());
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(slot.load().unwrap().unwrap(), saved);
    let index = app.click_hit(Some(UiHit::Button(0))).unwrap();
    assert!(!app.apply_button(index).unwrap());
    assert_eq!(app.game.snapshot(), saved);
    assert_eq!(app.screen, Screen::Loading);
    assert!(app.pointer_authorized);
    assert!(!app.input.mining_held());
    assert!(!app.input.take_place_press());
    app.screen = Screen::Paused;
    assert!(app.apply_button(2).unwrap());
    assert_eq!(slot.load().unwrap().unwrap(), saved);
    app.screen = Screen::MainMenu;
    assert!(app.apply_button(3).unwrap());
    assert_eq!(slot.load().unwrap().unwrap(), saved);
}

#[test]
fn failed_menu_actions_keep_slot_scene_and_progress_available_for_retry() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    fs::write(slot.path(), b"incompatible world").unwrap();
    let mut app = menu(&directory);
    let preview = app.game.snapshot();
    let error = app.apply_button(0).unwrap_err();
    app.scene_error(&error);
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(app.game.snapshot(), preview);
    assert_eq!(app.persistence.notice, Some(SaveNotice::LoadFailed));
    assert_eq!(fs::read(slot.path()).unwrap(), b"incompatible world");
    app.apply_button(1).unwrap();
    app.apply_button(0).unwrap();
    app.game.inventory_mut().insert(Block::Wood, 8);
    let progress = app.game.snapshot();
    app.screen = Screen::Paused;
    fs::remove_file(slot.path()).unwrap();
    fs::create_dir(slot.path()).unwrap();
    assert!(!app.apply_button(2).unwrap());
    let error = app.apply_button(1).unwrap_err();
    app.scene_error(&error);
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(app.game.snapshot(), progress);
    assert_eq!(app.persistence.notice, Some(SaveNotice::SaveFailed));
    fs::remove_dir(slot.path()).unwrap();
    assert!(!app.apply_button(1).unwrap());
    assert_eq!(slot.load().unwrap().unwrap(), progress);
}
