use super::*;
use crate::game::Tool;
use crate::game::test_support::arena;
use crate::save::test_support::TestDirectory;
use crate::world::Block;

#[test]
fn invalid_container_records_keep_previous_contents_and_never_touch_disk() {
    let directory = TestDirectory::new();
    let slot = directory.slot();
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory_mut().insert(Block::StorageCrate, 1);
    assert!(game.place_selected() && game.try_open_storage());
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.storage_click(StorageSlot::Player(0), false);
    game.storage_click(StorageSlot::Crate(0), false);
    slot.write(&game.snapshot()).unwrap();
    let original = std::fs::read(slot.path()).unwrap();
    let mut invalid = game.snapshot();
    invalid.crates[0].slots[0].as_mut().unwrap().count = 2;
    assert!(slot.write(&invalid).is_err());
    assert_eq!(std::fs::read(slot.path()).unwrap(), original);
    game.storage_click(StorageSlot::Crate(0), true);
    assert_eq!(game.snapshot(), slot.load().unwrap().unwrap());
    game.clear_input();
    assert!(game.storage_view().unwrap().cursor.is_none());
}
