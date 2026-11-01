use super::*;
use crate::game::test_support::arena;
use crate::game::{Item, Tool};
use crate::world::{Block, ChunkCoord};

fn crate_game() -> Game {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory_mut().insert(Block::StorageCrate, 1);
    assert!(game.place_selected());
    assert!(game.try_open_storage());
    game
}

fn total(game: &Game, item: Item) -> u32 {
    let snapshot = game.snapshot();
    snapshot
        .slots
        .iter()
        .flatten()
        .chain(
            snapshot
                .crates
                .iter()
                .flat_map(|saved| saved.slots.iter().flatten()),
        )
        .filter(|stack| stack.item == item)
        .map(|stack| u32::from(stack.count))
        .sum()
}

#[test]
fn bidirectional_split_swaps_and_close_preserve_items_and_reservations() {
    let mut game = crate_game();
    game.inventory_mut().insert(Block::Wood, 7);
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.storage_click(StorageSlot::Player(0), true);
    game.storage_click(StorageSlot::Crate(0), true);
    assert_eq!(game.storage_view().unwrap().slots[0].unwrap().count, 1);
    assert_eq!(total(&game, Block::Wood.into()), 7);
    game.close_storage_cursor();
    assert_eq!(game.inventory().slots()[0].unwrap().count, 6);
    game.storage_click(StorageSlot::Player(1), false);
    game.storage_click(StorageSlot::Crate(0), false);
    assert_eq!(
        game.storage_view().unwrap().slots[0].unwrap().item,
        Tool::StoneAxe.into()
    );
    assert_eq!(
        game.storage_view().unwrap().cursor.unwrap().item,
        Block::Wood.into()
    );
    game.close_storage();
    assert_eq!(total(&game, Block::Wood.into()), 7);
    assert_eq!(total(&game, Tool::StoneAxe.into()), 1);
    assert!(game.snapshot().validate().is_ok());
}

#[test]
fn nonempty_crate_rejects_mining_and_empty_crate_drops_exactly_once() {
    let mut game = crate_game();
    game.inventory_mut().insert(Block::Wood, 1);
    game.storage_click(StorageSlot::Player(0), false);
    game.storage_click(StorageSlot::Crate(0), false);
    game.close_storage();
    let before = game.snapshot();
    game.interact(true, false, 0.25);
    assert_eq!(game.snapshot(), before);
    assert_eq!(
        game.interaction_notice(),
        Some("EMPTY THE CRATE BEFORE MINING")
    );
    assert!(game.try_open_storage());
    game.storage_click(StorageSlot::Crate(0), false);
    game.storage_click(StorageSlot::Player(0), false);
    game.close_storage();
    for _ in 0..6 {
        game.interact(true, false, 0.25);
    }
    assert!(game.snapshot().crates.is_empty());
    assert_eq!(game.pickups().len(), 1);
    assert_eq!(game.pickups()[0].item, Block::StorageCrate.into());
}

#[test]
fn contents_survive_eviction_snapshot_and_reload_and_stale_access_closes_safely() {
    let mut game = crate_game();
    game.inventory_mut().insert(Block::Crystal, 3);
    game.storage_click(StorageSlot::Player(0), false);
    game.storage_click(StorageSlot::Crate(0), false);
    game.storage_click(StorageSlot::Crate(0), true);
    let snapshot = game.snapshot();
    assert_eq!(snapshot.crates[0].slots[0].unwrap().count, 3);
    game.world.remove(ChunkCoord::new(0, 0));
    game.storage_click(StorageSlot::Player(1), false);
    assert!(game.storage_view().is_none());
    let restored = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert!(!Game::from_snapshot(snapshot).unwrap().try_open_storage());
}

#[test]
fn duplicate_orphan_missing_invalid_and_oversized_crate_snapshots_are_rejected() {
    let game = crate_game();
    let original = game.snapshot();
    let mut missing = original.clone();
    missing.crates.clear();
    assert!(missing.validate().is_err());
    let mut duplicate = original.clone();
    duplicate.crates.push(duplicate.crates[0].clone());
    assert!(duplicate.validate().is_err());
    let mut orphan = original.clone();
    orphan.crates[0].position[0] += 1;
    assert!(orphan.validate().is_err());
    let mut invalid = original.clone();
    invalid.crates[0].slots[0] = Some(ItemStack {
        item: Tool::StoneAxe.into(),
        count: 2,
    });
    assert!(invalid.validate().is_err());
    let mut oversized = original;
    oversized.crates = vec![oversized.crates[0].clone(); MAX_CRATES + 1];
    assert!(oversized.validate().is_err());
}

#[test]
fn shift_place_on_crate_is_distinct_from_open_and_unloaded_target_is_inert() {
    let mut game = crate_game();
    game.close_storage();
    game.player.position.z = 10.5;
    game.inventory_mut().insert(Block::Wood, 1);
    game.interact_with_modifier(false, true, true, 0.0);
    assert!(game.storage_view().is_none());
    assert!(game.inventory().selected_stack().is_none());
    game.world.remove(ChunkCoord::new(0, 0));
    assert!(!game.try_open_storage());
}

#[test]
fn full_crate_limit_missing_association_and_out_of_reach_access_fail_closed() {
    let mut storage = Storage::default();
    for x in 0..MAX_CRATES {
        storage.placed([x as i32, -30, -17]);
    }
    assert!(!storage.can_place([-1, -30, -17]));
    let mut inventory = Inventory::default();
    assert!(!storage.open([-1, -30, -17], &mut inventory));
    let mut game = crate_game();
    game.inventory_mut().insert(Block::Wood, 1);
    game.storage_click(StorageSlot::Player(0), false);
    let before = game.snapshot();
    game.player.position.z += 10.0;
    game.storage_click(StorageSlot::Crate(0), false);
    assert!(game.storage_view().is_none());
    assert_eq!(game.inventory().slots()[0].unwrap().count, 1);
    assert_eq!(game.snapshot().crates, before.crates);
    assert!(!game.try_open_storage());
}
