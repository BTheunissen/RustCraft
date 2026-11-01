use super::{ButtonAction, button_action, clicked_capture};
use crate::app::App;
use crate::game::{Game, GameMode};
use crate::hud::{Screen, UiHit};
use crate::input::Action;
use crate::options::LaunchOptions;
use crate::world::Block;
use crate::world::{Chunk, ChunkCoord};
use winit::event::{MouseButton, MouseScrollDelta};
use winit::keyboard::KeyCode;

fn survival() -> App {
    let mut app = App::new(LaunchOptions::default());
    app.game = Game::with_mode(app.options.seed, GameMode::Survival);
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app
}

#[test]
fn ordinary_launch_opens_uncaptured_menu_and_smoke_still_explores() {
    let app = App::new(LaunchOptions::default());
    assert_eq!(app.screen, Screen::MainMenu);
    assert!(!app.captured);
    let smoke = App::new(LaunchOptions {
        smoke_test: true,
        ..Default::default()
    });
    assert_eq!(smoke.screen, Screen::Playing);
}

#[test]
fn menu_camera_moves_but_player_simulation_and_block_actions_do_not_run() {
    let mut app = App::new(LaunchOptions::default());
    let position = app.game.player.position;
    let yaw = app.game.player.yaw;
    app.captured = true;
    app.focused = true;
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.simulate(0.1);
    assert_eq!(app.game.player.position, position);
    assert!((app.game.player.yaw - yaw).rem_euclid(std::f32::consts::TAU) > 0.0);
    app.gameplay_action(&Action::ToggleFlight);
    assert!(!app.game.player.flying);
}

#[test]
fn menu_camera_ignores_invalid_time_and_wraps_long_running_rotation() {
    let mut app = App::new(LaunchOptions::default());
    let yaw = app.game.player.yaw;
    for dt in [f32::NAN, f32::INFINITY, -1.0] {
        app.simulate(dt);
        assert_eq!(app.game.player.yaw, yaw);
    }
    app.game.player.yaw = std::f32::consts::TAU - 0.001;
    app.simulate(1.0);
    assert!((0.0..0.01).contains(&app.game.player.yaw));
}

#[test]
fn inventory_freezes_simulation_clears_buttons_and_restores_cursor_stack() {
    let mut app = survival();
    assert_eq!(app.game.inventory_mut().insert(Block::Dirt, 8), 0);
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.input.mouse_button(MouseButton::Left, true);
    app.input.mouse_button(MouseButton::Right, true);
    let position = app.game.player.position;
    app.toggle_inventory();
    assert_eq!(app.screen, Screen::Inventory);
    assert!(!app.captured);
    assert!(!app.input.mining_held());
    assert!(!app.input.take_place_press());
    app.game.inventory_mut().click_slot(0);
    assert_eq!(app.game.inventory().cursor_stack().unwrap().count, 8);
    app.simulate(0.1);
    assert_eq!(app.game.player.position, position);
    assert_eq!(app.session_time, 0.0);
    app.escape();
    assert!(app.game.inventory().cursor_stack().is_none());
    assert_eq!(app.game.inventory().slots()[0].unwrap().count, 8);
    assert_eq!(app.screen, Screen::Paused);
}

#[test]
fn creative_inventory_key_is_inert_and_focus_loss_does_not_close_survival_inventory() {
    let mut app = App::new(LaunchOptions::default());
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app.toggle_inventory();
    assert_eq!(app.screen, Screen::Playing);
    assert!(app.captured);
    let mut app = survival();
    app.toggle_inventory();
    app.focused = false;
    app.release_pointer();
    assert_eq!(app.screen, Screen::Inventory);
    app.escape();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured);
}

#[test]
fn focus_release_pauses_gameplay_without_erasing_the_inventory() {
    let mut app = survival();
    app.game.inventory_mut().insert(Block::Stone, 3);
    app.release_pointer();
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(app.game.inventory().selected_stack().unwrap().count, 3);
    app.gameplay_action(&Action::SelectSlot(8));
    assert_eq!(app.game.inventory().selected(), 0);
}

#[test]
fn menu_keyboard_focus_wraps_and_cannot_change_gameplay() {
    let mut app = App::new(LaunchOptions::default());
    app.focused_button = 1;
    app.navigate(-1);
    assert_eq!(app.focused_button, 3);
    app.navigate(1);
    assert_eq!(app.focused_button, 1);
    app.screen = Screen::ConfirmReplace(GameMode::Survival);
    app.navigate(-1);
    assert_eq!(app.focused_button, 0);
    app.escape();
    assert_eq!(app.screen, Screen::MainMenu);
    app.screen = Screen::Playing;
    app.navigate(1);
    assert_eq!(app.focused_button, 1);
}

#[test]
fn hotbar_scroll_accumulates_trackpad_deltas_only_during_survival_play() {
    let mut app = survival();
    app.mouse_wheel(MouseScrollDelta::LineDelta(0.0, 0.4));
    assert_eq!(app.game.inventory().selected(), 0);
    app.mouse_wheel(MouseScrollDelta::LineDelta(0.0, 0.7));
    assert_eq!(app.game.inventory().selected(), 8);
    app.mouse_wheel(MouseScrollDelta::LineDelta(0.0, f32::NAN));
    assert_eq!(app.game.inventory().selected(), 8);
    app.release_pointer();
    app.mouse_wheel(MouseScrollDelta::LineDelta(0.0, -3.0));
    assert_eq!(app.game.inventory().selected(), 8);
    app.screen = Screen::Playing;
    app.captured = true;
    app.mouse_wheel(MouseScrollDelta::PixelDelta(
        winit::dpi::PhysicalPosition::new(0.0, -80.0),
    ));
    assert_eq!(app.game.inventory().selected(), 1);
    app.game = Game::new(app.options.seed);
    app.mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0));
    assert_eq!(app.game.inventory().selected(), 0);
}

#[test]
fn loading_waits_for_nearby_chunks_and_never_captures_without_a_click() {
    let mut app = survival();
    app.release_pointer();
    app.screen = Screen::Loading;
    app.resume_requested = true;
    app.finish_loading();
    assert_eq!(app.screen, Screen::Loading);
    let position = app.game.player.position;
    let center = ChunkCoord::from_world(position.x, position.z);
    for z in -1..=1 {
        for x in -1..=1 {
            app.game.world.insert(Chunk::generate(
                &app.game.terrain,
                ChunkCoord::new(center.x + x, center.z + z),
            ));
        }
    }
    app.finish_loading();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured);
    assert!(!app.resume_requested);
}

#[test]
fn restarting_a_scene_replaces_session_data_and_worker_not_the_window() {
    let mut app = survival();
    app.game.inventory_mut().insert(Block::Wood, 20);
    let terrain = app.game.terrain;
    let feet = app.game.player.position;
    let center = ChunkCoord::from_world(feet.x, feet.z);
    app.game.world.insert(Chunk::generate(&terrain, center));
    let source = [
        feet.x.floor() as i32,
        feet.y.floor() as i32 - 1,
        feet.z.floor() as i32,
    ];
    assert!(app.game.world.edit_block(source, Block::Air));
    app.show_main_menu().unwrap();
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(app.game.mode(), GameMode::Creative);
    assert_eq!(app.game.world.edits().revision(), 0);
    assert!(app.game.inventory().slots().iter().all(Option::is_none));
    assert!(!app.captured);
    assert!(!app.resume_requested);
    assert!(app.streamer.is_some());
    app.create_world(GameMode::Survival).unwrap();
    assert_eq!(app.screen, Screen::Loading);
    assert_eq!(app.game.mode(), GameMode::Survival);
    assert!(app.resume_requested);
    assert_eq!(app.game.player.position, feet);
    assert!(app.game.inventory().slots().iter().all(Option::is_none));
}

#[test]
fn menu_buttons_offer_continue_save_on_leave_and_confirm_replacement() {
    assert_eq!(
        button_action(Screen::MainMenu, 0),
        Some(ButtonAction::Continue)
    );
    assert_eq!(
        button_action(Screen::MainMenu, 1),
        Some(ButtonAction::Start(GameMode::Survival))
    );
    assert_eq!(
        button_action(Screen::MainMenu, 2),
        Some(ButtonAction::Start(GameMode::Creative))
    );
    assert_eq!(button_action(Screen::MainMenu, 3), Some(ButtonAction::Quit));
    assert_eq!(button_action(Screen::Paused, 0), Some(ButtonAction::Resume));
    assert_eq!(button_action(Screen::Paused, 1), Some(ButtonAction::Leave));
    assert_eq!(
        button_action(Screen::ConfirmReplace(GameMode::Survival), 0),
        Some(ButtonAction::Replace(GameMode::Survival))
    );
    assert_eq!(
        button_action(Screen::ConfirmReplace(GameMode::Survival), 1),
        Some(ButtonAction::CancelReplace)
    );
    for screen in [Screen::Playing, Screen::Inventory, Screen::Loading] {
        for index in 0..3 {
            assert_eq!(button_action(screen, index), None);
        }
    }
    assert_eq!(button_action(Screen::MainMenu, 4), None);
}

#[test]
fn ui_hit_routing_cannot_fall_through_into_world_actions() {
    let mut app = survival();
    app.game.inventory_mut().insert(Block::Dirt, 3);
    app.toggle_inventory();
    assert_eq!(app.click_hit(Some(UiHit::InventorySlot(0))), None);
    assert_eq!(app.game.inventory().cursor_stack().unwrap().count, 3);
    assert_eq!(app.click_hit(None), None);
    assert_eq!(app.click_hit(Some(UiHit::Button(0))), None);
    assert!(!app.input.mining_held());
    assert!(!app.input.take_place_press());
    app.screen = Screen::Paused;
    assert_eq!(app.click_hit(Some(UiHit::InventorySlot(8))), None);
    assert_eq!(app.click_hit(Some(UiHit::Button(0))), Some(0));
    assert!(app.pointer_authorized);
    assert_eq!(app.click_hit(Some(UiHit::Button(4))), None);
    assert!(!app.pointer_authorized);
    assert_eq!(app.game.inventory().cursor_stack().unwrap().count, 3);
}

#[test]
fn pause_and_respawn_clear_placement_edges_but_keep_selected_blocks() {
    let mut app = survival();
    app.game.inventory_mut().insert(Block::Stone, 4);
    app.gameplay_action(&Action::SelectSlot(0));
    app.input.mouse_button(MouseButton::Right, true);
    app.gameplay_action(&Action::Respawn);
    assert!(!app.input.take_place_press());
    assert_eq!(app.game.inventory().selected_stack().unwrap().count, 4);
    app.input.mouse_button(MouseButton::Right, true);
    app.escape();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.input.take_place_press());
    assert_eq!(app.game.inventory().selected_stack().unwrap().count, 4);
}

#[test]
fn mining_release_between_frames_resets_damage_even_if_button_is_held_again() {
    let mut app = survival();
    let terrain = app.game.terrain;
    let feet = app.game.player.position;
    app.game.world.insert(Chunk::generate(
        &terrain,
        ChunkCoord::from_world(feet.x, feet.z),
    ));
    app.game.look(0.0, 1000.0);
    app.input.mouse_button(MouseButton::Left, true);
    app.simulate(0.1);
    let first = app.game.mining_progress();
    assert!(first > 0.0);
    app.input.mouse_button(MouseButton::Left, false);
    app.input.mouse_button(MouseButton::Left, true);
    app.simulate(0.05);
    assert!(app.game.mining_progress() > 0.0 && app.game.mining_progress() < first);
    app.toggle_inventory();
    assert_eq!(app.game.mining_progress(), 0.0);
}

#[test]
fn a_jump_release_does_not_cancel_an_unrelated_held_mining_action() {
    let mut app = survival();
    let feet = app.game.player.position;
    app.game.world.insert(Chunk::generate(
        &app.game.terrain,
        ChunkCoord::from_world(feet.x, feet.z),
    ));
    app.game.look(0.0, 1000.0);
    app.input.mouse_button(MouseButton::Left, true);
    app.simulate(0.05);
    let progress = app.game.mining_progress();
    assert!(progress > 0.0);
    app.input
        .key_for_mode(KeyCode::Space, false, false, 0.1, false);
    app.simulate(0.05);
    assert!(app.game.mining_progress() > progress);
}

#[test]
fn only_clicked_mode_or_resume_authorizes_capture_and_release_revokes_it() {
    for screen in [
        Screen::MainMenu,
        Screen::Paused,
        Screen::ConfirmReplace(GameMode::Survival),
        Screen::Inventory,
    ] {
        for index in 0..4 {
            assert_eq!(
                clicked_capture(screen, index),
                screen == Screen::MainMenu && index < 3
                    || screen == Screen::Paused && index == 0
                    || matches!(screen, Screen::ConfirmReplace(_)) && index == 0
            );
        }
    }
    let mut app = survival();
    app.pointer_authorized = true;
    app.show_main_menu().unwrap();
    assert!(!app.pointer_authorized);
    assert_eq!(app.click_hit(Some(UiHit::Button(1))), Some(1));
    assert!(app.pointer_authorized);
    app.create_world(GameMode::Survival).unwrap();
    assert!(app.pointer_authorized);
    app.focused = false;
    app.release_pointer();
    assert!(!app.pointer_authorized);
    assert!(!app.inventory_resume_authorized);
}

#[test]
fn inventory_close_remembers_playing_capture_but_focus_loss_revokes_resume() {
    let mut app = survival();
    app.toggle_inventory();
    assert!(app.inventory_resume_authorized);
    app.release_pointer();
    assert!(!app.inventory_resume_authorized);
    app.toggle_inventory();
    assert!(!app.captured);
    assert!(!app.pointer_authorized);
}

#[test]
fn uninterrupted_inventory_close_consumes_both_resume_authorizations() {
    let mut app = survival();
    app.toggle_inventory();
    assert!(app.inventory_resume_authorized);
    assert!(!app.pointer_authorized);
    app.toggle_inventory();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.inventory_resume_authorized);
    assert!(!app.pointer_authorized);
    app.resume_play();
    assert!(!app.pointer_authorized);
    assert!(!app.captured);
}

#[test]
fn disabled_continue_click_is_inert_and_does_not_authorize_pointer_capture() {
    let mut app = App::new(LaunchOptions::default());
    assert_eq!(app.click_hit(Some(UiHit::Button(0))), None);
    assert!(!app.pointer_authorized);
    assert_eq!(app.screen, Screen::MainMenu);
}
