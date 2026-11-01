use std::fs;

use super::test_support::TestDirectory;
use super::*;
use crate::game::{Game, GameMode};

#[test]
fn absent_slots_are_normal_and_inspection_does_not_create_directories() {
    let directory = TestDirectory::new();
    let slot = SaveSlot::new(directory.path.join("not-created/world.save"));
    assert!(!slot.occupied().unwrap());
    assert!(slot.load().unwrap().is_none());
    assert!(!slot.path().parent().unwrap().exists());
    let local = SaveSlot::local().unwrap();
    assert!(local.path().is_absolute());
    assert_eq!(local.path().file_name().unwrap(), "world.save");
}

#[test]
fn writes_create_one_slot_and_atomically_replace_its_progress() {
    let directory = TestDirectory::new();
    let slot = SaveSlot::new(directory.path.join("progress/world.save"));
    let mut game = Game::with_mode(42, GameMode::Survival);
    game.inventory_mut().insert(crate::world::Block::Wood, 64);
    slot.write(&game.snapshot()).unwrap();
    assert!(slot.occupied().unwrap());
    assert_eq!(slot.load().unwrap().unwrap(), game.snapshot());
    game.inventory_mut().consume_selected();
    game.look(130.0, -20.0);
    slot.write(&game.snapshot()).unwrap();
    assert_eq!(slot.load().unwrap().unwrap(), game.snapshot());
    assert_eq!(
        fs::read_dir(slot.path().parent().unwrap()).unwrap().count(),
        1
    );
}

#[test]
fn tagged_tools_and_split_cursor_are_atomic_and_old_format_remains_occupied() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut game = Game::with_mode(42, GameMode::Survival);
    game.inventory_mut()
        .insert(crate::game::Tool::StonePickaxe, 1);
    game.inventory_mut().insert(crate::game::Tool::StoneAxe, 1);
    game.inventory_mut().insert(crate::world::Block::Wood, 7);
    game.inventory_mut().right_click_slot(2);
    slot.write(&game.snapshot()).unwrap();
    assert_eq!(slot.load().unwrap().unwrap(), game.snapshot());
    assert_eq!(game.inventory().cursor_stack().unwrap().count, 4);
    let mut bytes = fs::read(slot.path()).unwrap();
    bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
    fs::write(slot.path(), &bytes).unwrap();
    assert_eq!(slot.load().unwrap_err().kind(), io::ErrorKind::Unsupported);
    assert!(slot.occupied().unwrap());
    assert_eq!(fs::read(slot.path()).unwrap(), bytes);
}

#[test]
fn interrupted_partial_write_preserves_previous_save_and_cleans_its_temporary() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let saved = Game::new(42).snapshot();
    slot.write(&saved).unwrap();
    let previous = fs::read(slot.path()).unwrap();
    let error = atomic::replace(slot.path(), |file| {
        file.write_all(b"partial new save")?;
        Err(io::Error::other("simulated interrupted write"))
    })
    .unwrap_err();
    assert!(error.to_string().contains("interrupted"));
    assert_eq!(fs::read(slot.path()).unwrap(), previous);
    assert_eq!(slot.load().unwrap().unwrap(), saved);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn abandoned_staging_files_are_never_loaded_or_overwritten_as_worlds() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let abandoned = directory.path.join("world.save.crashed.tmp");
    fs::write(&abandoned, b"incomplete bytes").unwrap();
    assert!(!slot.occupied().unwrap());
    assert!(slot.load().unwrap().is_none());
    let saved = Game::new(42).snapshot();
    slot.write(&saved).unwrap();
    assert_eq!(slot.load().unwrap().unwrap(), saved);
    assert_eq!(fs::read(abandoned).unwrap(), b"incomplete bytes");
}

#[test]
fn invalid_progress_is_rejected_before_touching_a_previous_slot() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut saved = Game::new(42).snapshot();
    slot.write(&saved).unwrap();
    let previous = fs::read(slot.path()).unwrap();
    saved.player.position.x = f32::NAN;
    assert_eq!(
        slot.write(&saved).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(slot.path()).unwrap(), previous);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn bad_or_incompatible_files_remain_occupied_and_are_not_silently_reset() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    fs::write(slot.path(), b"damaged world").unwrap();
    assert!(slot.occupied().unwrap());
    assert_eq!(slot.load().unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert_eq!(fs::read(slot.path()).unwrap(), b"damaged world");
    let mut future = format::encode(&Game::new(42).snapshot()).unwrap();
    future[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    fs::write(slot.path(), &future).unwrap();
    assert!(slot.load().unwrap_err().to_string().contains("version"));
    assert_eq!(fs::read(slot.path()).unwrap(), future);
}

#[test]
fn oversized_files_are_rejected_without_reading_their_entire_contents() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let file = File::create(slot.path()).unwrap();
    file.set_len(format::MAX_SAVE_BYTES as u64 + 1).unwrap();
    assert!(slot.load().unwrap_err().to_string().contains("too large"));
    assert!(slot.occupied().unwrap());
}

#[test]
fn filesystem_and_commit_failures_leave_existing_entries_intact() {
    let directory = TestDirectory::new();
    let blocked = directory.path.join("file-not-directory");
    fs::write(&blocked, b"do not replace me").unwrap();
    let slot = SaveSlot::new(blocked.join("world.save"));
    assert!(slot.occupied().is_err());
    assert!(slot.load().is_err());
    assert!(slot.write(&Game::new(42).snapshot()).is_err());
    assert_eq!(fs::read(&blocked).unwrap(), b"do not replace me");

    let slot = directory.slot();
    fs::create_dir(slot.path()).unwrap();
    fs::write(slot.path().join("marker"), b"existing directory").unwrap();
    assert!(slot.occupied().unwrap());
    assert!(slot.load().is_err());
    assert!(slot.write(&Game::new(42).snapshot()).is_err());
    assert_eq!(
        fs::read(slot.path().join("marker")).unwrap(),
        b"existing directory"
    );
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 2);
}

#[test]
fn selected_filenames_commit_and_paths_without_a_filename_fail() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    atomic::replace(slot.path(), |file| file.write_all(b"complete")).unwrap();
    assert_eq!(fs::read(slot.path()).unwrap(), b"complete");
    assert!(atomic::replace(&directory.path.join(".."), |_| Ok(())).is_err());
}
