use super::*;
use crate::game::Tool;

fn totals(inventory: &Inventory) -> [u32; Block::COUNT + 2] {
    let mut totals = [0; Block::COUNT + 2];
    for stack in inventory
        .slots
        .iter()
        .flatten()
        .chain(inventory.cursor.iter())
    {
        let index = match stack.item {
            Item::Block(block) => block as usize,
            Item::Tool(tool) => Block::COUNT + tool as usize,
        };
        totals[index] += u32::from(stack.count);
        assert!(stack.is_valid());
    }
    totals
}

#[test]
fn tools_are_nonstackable_and_insertion_preserves_all_overflow() {
    let mut inventory = Inventory::default();
    assert_eq!(inventory.insert(Tool::StonePickaxe, 40), 4);
    assert!(inventory.slots.iter().all(|slot| {
        slot.is_some_and(|stack| stack.item == Item::Tool(Tool::StonePickaxe) && stack.count == 1)
    }));
    assert_eq!(inventory.insert(Tool::StoneAxe, 1), 1);
    assert!(
        ItemStack {
            item: Tool::StoneAxe.into(),
            count: 1
        }
        .is_valid()
    );
    assert!(
        !ItemStack {
            item: Tool::StoneAxe.into(),
            count: 2
        }
        .is_valid()
    );
    assert_eq!(Item::Tool(Tool::StonePickaxe).block(), None);
    assert_ne!(
        Item::Tool(Tool::StonePickaxe).name(),
        Item::Tool(Tool::StoneAxe).name()
    );
}

#[test]
fn split_lifts_rounded_half_places_one_and_close_restores_only_the_remainder() {
    for count in [1, 2, 3, 63, 64] {
        let mut inventory = Inventory::default();
        inventory.insert(Block::Wood, count);
        inventory.right_click_slot(0);
        assert_eq!(inventory.cursor_stack().unwrap().count, count.div_ceil(2));
        assert_eq!(inventory.slots[0].map_or(0, |stack| stack.count), count / 2);
        assert!(inventory.selected_stack().is_none());
        assert!(inventory.consume_selected().is_none());
        inventory.right_click_slot(35);
        assert_eq!(inventory.slots[35].unwrap().count, 1);
        inventory.close_cursor();
        assert_eq!(inventory.slots[0].map_or(0, |stack| stack.count), count - 1);
        assert_eq!(totals(&inventory)[Block::Wood as usize], u32::from(count));
    }
}

#[test]
fn a_populated_split_reservation_blocks_insertion_and_incompatible_swap() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Wood, 64);
    inventory.insert(Tool::StoneAxe, 1);
    inventory.right_click_slot(0);
    let before = inventory.clone();
    inventory.click_slot(1);
    assert_eq!(inventory, before);
    inventory.right_click_slot(1);
    assert_eq!(inventory, before);
    inventory.insert(Block::Wood, 32);
    assert_eq!(inventory.slots[0].unwrap().count, 32);
    inventory.close_cursor();
    assert_eq!(inventory.slots[0].unwrap().count, 64);
    assert_eq!(totals(&inventory)[Block::Wood as usize], 96);
}

#[test]
fn full_stack_tool_swap_then_single_placement_can_still_restore_on_close() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Wood, 64);
    inventory.insert(Tool::StonePickaxe, 1);
    let expected = totals(&inventory);
    inventory.click_slot(0);
    inventory.click_slot(1);
    assert_eq!(
        inventory.cursor_stack().unwrap().item,
        Tool::StonePickaxe.into()
    );
    inventory.right_click_slot(2);
    inventory.close_cursor();
    assert_eq!(totals(&inventory), expected);
    assert_eq!(inventory.slots[2].unwrap().item, Tool::StonePickaxe.into());
}

#[test]
fn mixed_whole_split_merge_swaps_and_full_automatic_insertion_are_lossless() {
    let mut inventory = Inventory::default();
    let items = [
        Item::Block(Block::Stone),
        Item::Block(Block::Wood),
        Item::Tool(Tool::StoneAxe),
        Item::Tool(Tool::StonePickaxe),
    ];
    let mut expected = [0_u32; Block::COUNT + 2];
    let mut random = 2026_u32;
    for iteration in 0..8000 {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let index = random as usize % 39;
        match iteration % 5 {
            0 => {
                let item = items[random as usize % items.len()];
                let count = (random % 100) as u16;
                let item_index = match item {
                    Item::Block(block) => block as usize,
                    Item::Tool(tool) => Block::COUNT + tool as usize,
                };
                expected[item_index] += u32::from(count - inventory.insert(item, count));
            }
            1 => inventory.click_slot(index),
            2 | 3 => inventory.right_click_slot(index),
            _ => inventory.close_cursor(),
        }
        assert_eq!(totals(&inventory), expected);
    }
    inventory.close_cursor();
    assert_eq!(totals(&inventory), expected);
}
