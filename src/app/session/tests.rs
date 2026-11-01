use std::fs;
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::app::App;
use crate::app::persistence::{AUTOSAVE_INTERVAL, Persistence};
use crate::game::{Game, GameMode, SavedPickup};
use crate::hud::{SaveNotice, Screen};
use crate::input::Action;
use crate::options::LaunchOptions;
use crate::save::test_support::TestDirectory;
use crate::world::{Block, BlockEdit, ChunkCoord};

fn app(directory: &TestDirectory, seed: u64) -> App {
    let mut app = App::new(LaunchOptions {
        seed,
        render_distance: 2,
        ..Default::default()
    });
    app.persistence = Persistence::with_slot(directory.slot());
    app
}

#[test]
fn continue_keeps_saved_night_phase_and_does_not_inherit_a_previous_debug_rate() {
    let directory = TestDirectory::new();
    let mut first = app(&directory, 42);
    first.options.time_of_day = 5.5;
    first.create_world(GameMode::Creative).unwrap();
    assert_eq!(first.game.time().hour(), 5.5);
    first.game.time_mut().command("/time set night").unwrap();
    first.game.time_mut().command("/time speed 120").unwrap();
    first.leave_world().unwrap();
    first.continue_world().unwrap();
    assert_eq!(first.game.time().hour(), 22.0);
    assert_eq!(first.game.time().speed(), 1.0);
    drop(first);
    let mut second = app(&directory, 99);
    second.options.time_of_day = 12.0;
    second.options.time_speed = 60.0;
    second.continue_world().unwrap();
    assert_eq!(second.game.time().hour(), 22.0);
    assert_eq!(second.game.time().speed(), 60.0);
    assert_eq!(second.game.terrain().seed(), 42);
}

#[test]
fn new_world_commits_immediately_and_leave_then_relaunch_restores_all_progress() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut first = app(&directory, 42);
    first.request_new_world(GameMode::Survival).unwrap();
    assert_eq!(first.screen, Screen::Loading);
    assert!(first.persistence.can_continue());
    assert_eq!(slot.load().unwrap().unwrap().mode, GameMode::Survival);

    let mut saved = first.game.snapshot();
    saved.player.position = Vec3::new(-8.5, -30.0, -20.5);
    saved.player.yaw = 1.5;
    saved.player.pitch = 0.2;
    saved.edits = vec![
        BlockEdit {
            position: [-8, -31, -21],
            block: Block::Wood,
        },
        BlockEdit {
            position: [-9, -30, -21],
            block: Block::Air,
        },
    ];
    saved.pickups.push(SavedPickup {
        position: Vec3::new(-5.5, -30.5, -20.5),
        item: Block::Stone.into(),
        count: 2,
        velocity: Vec3::new(0.0, -2.0, 0.0),
        age: 0.1,
    });
    first.game = Game::from_snapshot(saved).unwrap();
    first.game.inventory_mut().insert(Block::Clay, 70);
    first.game.inventory_mut().click_slot(1);
    first.game.inventory_mut().click_slot(35);
    first.game.inventory_mut().select(8);
    let progress = first.game.snapshot();
    first.screen = Screen::Paused;
    first.leave_world().unwrap();
    assert_eq!(first.screen, Screen::MainMenu);
    assert_eq!(slot.load().unwrap().unwrap(), progress);
    first.autosave(Instant::now() + AUTOSAVE_INTERVAL * 2);
    assert_eq!(slot.load().unwrap().unwrap(), progress);
    drop(first);

    let mut second = app(&directory, 12345);
    assert!(second.persistence.can_continue());
    second.continue_world().unwrap();
    assert_eq!(second.screen, Screen::Loading);
    assert_eq!(second.game.snapshot(), progress);
    assert_eq!(second.game.terrain().seed(), 42);
    assert!(second.game.world.is_empty());
    assert!(!second.captured);
    assert!(!second.pointer_authorized);
    let deadline = Instant::now() + Duration::from_secs(10);
    let prepared = loop {
        let streamer = second.streamer.as_mut().unwrap();
        if let Some(prepared) = streamer.poll_ready(1).pop() {
            break prepared;
        }
        assert!(!streamer.worker_failed());
        assert!(
            Instant::now() < deadline,
            "restored world streaming stalled"
        );
        std::thread::yield_now();
    };
    assert_eq!(prepared.edit_revision, second.game.world.edits().revision());
    assert_eq!(prepared.coord, ChunkCoord::new(-1, -2));
    assert_eq!(prepared.chunk.block(8, -31, 11), Block::Wood);
    assert_eq!(prepared.chunk.block(7, -30, 11), Block::Air);
}

#[test]
fn occupied_or_corrupt_slots_require_confirmation_and_cancel_never_replaces_them() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let original = Game::with_mode(42, GameMode::Survival).snapshot();
    slot.write(&original).unwrap();
    let bytes = fs::read(slot.path()).unwrap();
    let mut app = app(&directory, 99);
    let preview = app.game.snapshot();
    app.pointer_authorized = true;
    app.request_new_world(GameMode::Creative).unwrap();
    assert_eq!(app.screen, Screen::ConfirmReplace(GameMode::Creative));
    assert_eq!(app.focused_button, 1);
    assert!(!app.pointer_authorized);
    assert_eq!(app.game.snapshot(), preview);
    assert_eq!(fs::read(slot.path()).unwrap(), bytes);
    app.escape();
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(fs::read(slot.path()).unwrap(), bytes);
    fs::write(slot.path(), b"broken but still belongs to the player").unwrap();
    app.request_new_world(GameMode::Survival).unwrap();
    assert_eq!(app.screen, Screen::ConfirmReplace(GameMode::Survival));
    assert_eq!(
        fs::read(slot.path()).unwrap(),
        b"broken but still belongs to the player"
    );
    app.create_world(GameMode::Survival).unwrap();
    assert_eq!(slot.load().unwrap().unwrap().seed, 99);
    assert_eq!(slot.load().unwrap().unwrap().mode, GameMode::Survival);
}

#[test]
fn failed_load_and_failed_new_world_commit_keep_the_existing_slot_and_scene() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    fs::write(slot.path(), b"damaged save").unwrap();
    let mut app = app(&directory, 42);
    let preview = app.game.snapshot();
    assert!(app.continue_world().is_err());
    assert_eq!(app.screen, Screen::MainMenu);
    assert_eq!(app.game.snapshot(), preview);
    assert_eq!(app.persistence.notice, Some(SaveNotice::LoadFailed));
    assert_eq!(fs::read(slot.path()).unwrap(), b"damaged save");
    fs::remove_file(slot.path()).unwrap();
    fs::create_dir(slot.path()).unwrap();
    app.request_new_world(GameMode::Creative).unwrap();
    assert!(app.create_world(GameMode::Creative).is_err());
    assert_eq!(app.screen, Screen::ConfirmReplace(GameMode::Creative));
    assert_eq!(app.game.snapshot(), preview);
    assert_eq!(app.persistence.notice, Some(SaveNotice::SaveFailed));
    assert!(slot.path().is_dir());
}

#[test]
fn incompatible_versions_explain_a_fresh_world_without_replacing_the_occupied_slot() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    slot.write(&Game::new(42).snapshot()).unwrap();
    let original = fs::read(slot.path()).unwrap();
    for offset in [8, 12] {
        let mut incompatible = original.clone();
        incompatible[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(slot.path(), &incompatible).unwrap();
        let mut app = app(&directory, 99);
        let preview = app.game.snapshot();
        let error = app.continue_world().unwrap_err();
        app.scene_error(&error);
        assert_eq!(app.persistence.notice, Some(SaveNotice::Incompatible));
        assert_eq!(app.screen, Screen::MainMenu);
        assert_eq!(app.game.snapshot(), preview);
        assert!(app.persistence.can_continue());
        app.request_new_world(GameMode::Survival).unwrap();
        assert_eq!(app.screen, Screen::ConfirmReplace(GameMode::Survival));
        assert_eq!(app.focused_button, 1);
        app.escape();
        assert_eq!(app.screen, Screen::MainMenu);
        assert_eq!(fs::read(slot.path()).unwrap(), incompatible);
    }
}

#[test]
fn failed_leave_or_quit_keeps_live_progress_open_and_can_be_retried() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut app = app(&directory, 42);
    app.request_new_world(GameMode::Survival).unwrap();
    app.game.inventory_mut().insert(Block::Wood, 12);
    app.game.inventory_mut().click_slot(0);
    app.screen = Screen::Inventory;
    let progress = app.game.snapshot();
    fs::remove_file(slot.path()).unwrap();
    fs::create_dir(slot.path()).unwrap();
    assert!(!app.save_before_exit());
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(app.game.snapshot(), progress);
    assert!(app.game.inventory().cursor_stack().is_none());
    assert_eq!(app.persistence.notice, Some(SaveNotice::SaveFailed));
    assert!(app.leave_world().is_err());
    assert_eq!(app.screen, Screen::Paused);
    fs::remove_dir(slot.path()).unwrap();
    assert!(app.save_before_exit());
    assert_eq!(slot.load().unwrap().unwrap(), progress);
    app.game = Game::new(100);
    assert!(app.save_before_exit());
    assert_eq!(slot.load().unwrap().unwrap(), progress);
}

#[test]
fn autosave_runs_while_paused_or_in_inventory_and_failure_releases_controls() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut app = app(&directory, 42);
    app.request_new_world(GameMode::Survival).unwrap();
    let now = Instant::now();
    app.persistence.begin_world(now);
    app.game.inventory_mut().insert(Block::Dirt, 6);
    app.game.inventory_mut().click_slot(0);
    app.screen = Screen::Inventory;
    app.autosave(now + AUTOSAVE_INTERVAL);
    assert_eq!(app.game.inventory().cursor_stack().unwrap().count, 6);
    assert_eq!(slot.load().unwrap().unwrap(), app.game.snapshot());
    let bytes = fs::read(slot.path()).unwrap();
    app.game.player.pitch = f32::NAN;
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app.autosave(now + AUTOSAVE_INTERVAL * 2);
    assert!(!app.captured);
    assert_eq!(app.screen, Screen::Paused);
    assert_eq!(app.persistence.notice, Some(SaveNotice::SaveFailed));
    assert_eq!(fs::read(slot.path()).unwrap(), bytes);
    app.game.player.pitch = 0.0;
    app.autosave(now + AUTOSAVE_INTERVAL * 3);
    assert_eq!(slot.load().unwrap().unwrap(), app.game.snapshot());
}

#[test]
fn confirmation_preview_and_smoke_launches_never_autosave_a_fake_world() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let original = Game::new(99).snapshot();
    slot.write(&original).unwrap();
    let mut app = app(&directory, 42);
    app.request_new_world(GameMode::Survival).unwrap();
    app.autosave(Instant::now() + Duration::from_secs(600));
    assert!(app.save_before_exit());
    assert_eq!(slot.load().unwrap().unwrap(), original);
    let mut smoke = App::new(LaunchOptions {
        smoke_test: true,
        ..Default::default()
    });
    smoke.persistence.begin_world(Instant::now());
    smoke.autosave(Instant::now() + Duration::from_secs(600));
    assert!(smoke.save_before_exit());
    assert!(!smoke.persistence.can_continue());
    assert_eq!(slot.load().unwrap().unwrap(), original);
}

#[test]
fn continuing_requires_loaded_chunks_even_when_the_player_flies_above_world_height() {
    let directory = TestDirectory::new();
    let mut saved = Game::new(42).snapshot();
    saved.player.flying = true;
    saved.player.position.y = 200.0;
    directory.slot().write(&saved).unwrap();
    let mut app = app(&directory, 99);
    app.continue_world().unwrap();
    app.resume_requested = false;
    app.finish_loading();
    assert_eq!(app.screen, Screen::Loading);
    let center = ChunkCoord::from_world(saved.player.position.x, saved.player.position.z);
    for z in -1..=1 {
        for x in -1..=1 {
            app.game.world.insert(crate::world::Chunk::generate(
                &app.game.terrain,
                ChunkCoord::new(center.x + x, center.z + z),
            ));
        }
    }
    app.finish_loading();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured);
}

#[test]
fn successful_save_does_not_change_held_mining_or_player_controls() {
    let directory = TestDirectory::new();
    let mut app = app(&directory, 42);
    app.request_new_world(GameMode::Survival).unwrap();
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app.gameplay_action(&Action::SelectSlot(8));
    app.input
        .mouse_button(winit::event::MouseButton::Left, true);
    let now = Instant::now();
    app.persistence.begin_world(now);
    app.autosave(now + AUTOSAVE_INTERVAL);
    assert!(app.captured);
    assert!(app.input.mining_held());
    assert_eq!(app.game.inventory().selected(), 8);
}

#[test]
fn direct_native_exit_saves_inventory_and_reports_errors_without_hiding_existing_failures() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut app = app(&directory, 42);
    app.request_new_world(GameMode::Survival).unwrap();
    app.game.inventory_mut().insert(Block::Clay, 10);
    app.game.inventory_mut().click_slot(0);
    let progress = app.game.snapshot();
    app.record_exit();
    assert_eq!(slot.load().unwrap().unwrap(), progress);
    assert!(app.error.is_none());
    app.persistence.begin_world(Instant::now());
    fs::remove_file(slot.path()).unwrap();
    fs::create_dir(slot.path()).unwrap();
    app.record_exit();
    assert!(app.error.as_deref().unwrap().contains("Could not save"));
    app.error = Some("GPU already failed".into());
    app.record_exit();
    assert_eq!(app.error.as_deref(), Some("GPU already failed"));
}
