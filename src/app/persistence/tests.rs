use super::*;
use crate::game::GameMode;
use crate::save::test_support::TestDirectory;
use std::fs;

#[test]
fn periodic_saves_use_wall_clock_and_save_only_an_active_world() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut persistence = Persistence::with_slot(slot.clone());
    let now = Instant::now();
    let game = Game::with_mode(42, GameMode::Survival);
    assert!(!persistence.can_continue());
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL * 2));
    assert!(persistence.wake_deadline().is_none());
    persistence.save_active(&game, now).unwrap();
    assert!(!slot.occupied().unwrap());
    persistence.begin_world(now);
    assert_eq!(persistence.wake_deadline(), Some(now + AUTOSAVE_INTERVAL));
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL - Duration::from_nanos(1)));
    assert!(persistence.due(now + AUTOSAVE_INTERVAL));
    persistence
        .save_active(&game, now + AUTOSAVE_INTERVAL)
        .unwrap();
    assert_eq!(slot.load().unwrap().unwrap(), game.snapshot());
    assert!(persistence.can_continue());
    assert_eq!(persistence.notice, Some(SaveNotice::Saved));
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL));
    assert!(persistence.due(now + AUTOSAVE_INTERVAL * 2));
    persistence.end_world();
    assert!(persistence.wake_deadline().is_none());
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL * 3));
    persistence.save_active(&Game::new(100), now).unwrap();
    assert_eq!(slot.load().unwrap().unwrap().seed, 42);
}

#[test]
fn cursor_held_stacks_are_saved_without_mutating_the_inventory() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut persistence = Persistence::with_slot(slot.clone());
    let now = Instant::now();
    let mut game = Game::with_mode(42, GameMode::Survival);
    game.inventory_mut().insert(crate::world::Block::Wood, 12);
    game.inventory_mut().click_slot(0);
    persistence.begin_world(now);
    persistence.save_active(&game, now).unwrap();
    assert_eq!(game.inventory().cursor_stack().unwrap().count, 12);
    let resumed = Game::from_snapshot(slot.load().unwrap().unwrap()).unwrap();
    assert!(resumed.inventory().cursor_stack().is_none());
    assert_eq!(resumed.inventory().slots()[0].unwrap().count, 12);
}

#[test]
fn failed_autosaves_preserve_data_and_back_off_instead_of_retrying_each_frame() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut persistence = Persistence::with_slot(slot.clone());
    let now = Instant::now();
    persistence.begin_world(now);
    let game = Game::new(42);
    persistence.save_active(&game, now).unwrap();
    let previous = fs::read(slot.path()).unwrap();
    let mut invalid = game.snapshot();
    invalid.player.pitch = f32::NAN;
    assert!(
        persistence
            .write(&invalid, now + AUTOSAVE_INTERVAL)
            .is_err()
    );
    assert_eq!(fs::read(slot.path()).unwrap(), previous);
    assert_eq!(persistence.notice, Some(SaveNotice::SaveFailed));
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL + Duration::from_secs(1)));
    assert!(persistence.due(now + AUTOSAVE_INTERVAL * 2));
    persistence
        .save_active(&game, now + AUTOSAVE_INTERVAL * 2)
        .unwrap();
    assert_eq!(persistence.notice, Some(SaveNotice::Saved));
}

#[test]
fn previews_do_not_modify_slots_and_failed_loads_preserve_bad_files() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut persistence = Persistence::with_slot(slot.clone());
    assert!(persistence.load().is_err());
    assert_eq!(persistence.notice, Some(SaveNotice::LoadFailed));
    fs::write(slot.path(), b"unknown world format").unwrap();
    assert!(persistence.check_occupied().unwrap());
    assert!(persistence.can_continue());
    assert!(persistence.load().is_err());
    persistence
        .save_active(&Game::new(42), Instant::now())
        .unwrap();
    assert_eq!(fs::read(slot.path()).unwrap(), b"unknown world format");
}

#[test]
fn smoke_and_disabled_persistence_never_make_a_slot_available() {
    let mut persistence = Persistence::disabled();
    let now = Instant::now();
    assert!(!persistence.check_occupied().unwrap());
    assert!(!persistence.can_continue());
    assert!(persistence.load().is_err());
    persistence.begin_world(now);
    assert!(!persistence.due(now + AUTOSAVE_INTERVAL * 100));
    persistence.save_active(&Game::new(42), now).unwrap();
    assert!(!persistence.can_continue());
}

#[test]
fn inaccessible_local_directory_fails_closed_and_reports_the_error() {
    let directory = TestDirectory::new();
    let blocked = directory.path.join("blocked");
    fs::write(&blocked, b"existing file").unwrap();
    let mut persistence = Persistence::with_slot(SaveSlot::new(blocked.join("world.save")));
    assert_eq!(persistence.notice, Some(SaveNotice::Unavailable));
    assert!(persistence.check_occupied().is_err());
    assert!(
        persistence
            .write(&Game::new(42).snapshot(), Instant::now())
            .is_err()
    );
    assert_eq!(persistence.notice, Some(SaveNotice::SaveFailed));
    assert_eq!(fs::read(blocked).unwrap(), b"existing file");

    let mut missing = Persistence {
        storage: Storage::Unavailable("no home directory".into()),
        ..Persistence::disabled()
    };
    assert!(!missing.can_continue());
    assert!(missing.check_occupied().is_err());
    assert!(
        missing
            .write(&Game::new(42).snapshot(), Instant::now())
            .is_err()
    );
}
