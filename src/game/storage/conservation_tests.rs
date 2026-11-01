use super::*;
use crate::game::{INVENTORY_SLOTS, Item, Tool};
use crate::world::Block;

fn totals(storage: &Storage, inventory: &Inventory) -> [u32; 4] {
    let mut totals = [0; 4];
    for stack in inventory
        .slots()
        .iter()
        .flatten()
        .chain(
            storage
                .crates
                .values()
                .flat_map(|slots| slots.iter().flatten()),
        )
        .chain(storage.cursor.iter())
    {
        let index = match stack.item {
            Item::Block(Block::Stone) => 0,
            Item::Block(Block::Wood) => 1,
            Item::Tool(Tool::StoneAxe) => 2,
            Item::Tool(Tool::StonePickaxe) => 3,
            _ => panic!("unexpected fixture item"),
        };
        assert!(stack.is_valid());
        totals[index] += u32::from(stack.count);
    }
    totals
}

#[test]
fn mixed_cross_inventory_cursor_operations_and_autoinsertion_conserve_every_item() {
    let mut storage = Storage::default();
    let mut inventory = Inventory::default();
    storage.placed([-16, -30, -17]);
    assert!(storage.open([-16, -30, -17], &mut inventory));
    let items: [Item; 4] = [
        Block::Stone.into(),
        Block::Wood.into(),
        Tool::StoneAxe.into(),
        Tool::StonePickaxe.into(),
    ];
    let mut expected = [0_u32; 4];
    let mut random = 2026_u32;
    for tick in 0..10_000 {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        if tick % 7 == 0 {
            let index = random as usize % 4;
            let count = (random % 100) as u16;
            expected[index] += u32::from(count - inventory.insert(items[index], count));
        } else if tick % 7 == 6 {
            storage.close_cursor(&mut inventory);
        } else {
            let slot = if random & 1 == 0 {
                StorageSlot::Player(random as usize % (INVENTORY_SLOTS + 1))
            } else {
                StorageSlot::Crate(random as usize % (CRATE_SLOTS + 1))
            };
            storage.click(&mut inventory, slot, tick % 3 == 0);
        }
        assert_eq!(totals(&storage, &inventory), expected, "tick {tick}");
    }
    storage.close(&mut inventory);
    assert_eq!(totals(&storage, &inventory), expected);
}

#[test]
fn full_destination_and_split_source_incompatible_swap_are_noops() {
    let mut storage = Storage::default();
    let mut inventory = Inventory::default();
    storage.placed([0, -30, 0]);
    storage.open([0, -30, 0], &mut inventory);
    inventory.insert(Block::Wood, 64);
    storage
        .crates
        .get_mut(&[0, -30, 0])
        .unwrap()
        .fill(Some(ItemStack {
            item: Block::Stone.into(),
            count: 64,
        }));
    storage.click(&mut inventory, StorageSlot::Player(0), true);
    let before = totals(&storage, &inventory);
    let held = storage.cursor;
    storage.click(&mut inventory, StorageSlot::Crate(0), false);
    assert_eq!(storage.cursor, held);
    assert_eq!(totals(&storage, &inventory), before);
    storage.close_cursor(&mut inventory);
    assert_eq!(inventory.slots()[0].unwrap().count, 64);
}
