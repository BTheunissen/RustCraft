use super::*;
use crate::game::test_support::arena;
use crate::game::{Game, StorageSlot};

#[test]
fn negative_crate_contents_and_both_cursor_origins_round_trip_exactly() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory_mut().insert(Block::StorageCrate, 1);
    assert!(game.place_selected());
    assert!(game.try_open_storage());
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.storage_click(StorageSlot::Player(0), false);
    let player_held = game.snapshot();
    assert_eq!(decode(&encode(&player_held).unwrap()).unwrap(), player_held);
    game.storage_click(StorageSlot::Crate(0), false);
    game.storage_click(StorageSlot::Crate(0), true);
    let crate_held = game.snapshot();
    assert_eq!(decode(&encode(&crate_held).unwrap()).unwrap(), crate_held);
    let restored = Game::from_snapshot(crate_held.clone()).unwrap();
    assert_eq!(restored.snapshot(), crate_held);
    let mut too_many = encode(&player_held).unwrap();
    let crates_offset = HEADER_BYTES
        + 33
        + INVENTORY_SLOTS * 4
        + 4
        + player_held.edits.len() * EDIT_BYTES
        + 4
        + player_held.pickups.len() * PICKUP_BYTES;
    too_many[crates_offset..crates_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let hash = checksum(&too_many[HEADER_BYTES..]);
    too_many[24..32].copy_from_slice(&hash.to_le_bytes());
    assert!(decode(&too_many).is_err());
}

#[test]
fn actual_negative_seam_crate_records_restore_without_residency() {
    let mut snapshot = Game::with_mode(42, GameMode::Survival).snapshot();
    snapshot.edits.push(BlockEdit {
        position: [-16, -30, -17],
        block: Block::StorageCrate,
    });
    let mut slots = [None; CRATE_SLOTS];
    slots[0] = Some(ItemStack {
        item: Tool::StonePickaxe.into(),
        count: 1,
    });
    slots[17] = Some(ItemStack {
        item: Block::Crystal.into(),
        count: 64,
    });
    snapshot.crates.push(SavedCrate {
        position: [-16, -30, -17],
        slots,
    });
    let bytes = encode(&snapshot).unwrap();
    assert_eq!(decode(&bytes).unwrap(), snapshot);
    assert_eq!(
        Game::from_snapshot(snapshot.clone()).unwrap().snapshot(),
        snapshot
    );
    for version in [1_u32, 2] {
        let mut old = bytes.clone();
        old[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(decode(&old).unwrap_err().kind(), io::ErrorKind::Unsupported);
    }
}
