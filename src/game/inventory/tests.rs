use super::*;

fn total(inventory: &Inventory) -> u32 {
    inventory
        .slots()
        .iter()
        .copied()
        .flatten()
        .chain(inventory.cursor_stack())
        .map(|stack| u32::from(stack.count))
        .sum()
}

#[test]
fn insertion_merges_before_empty_slots_and_returns_all_capacity_overflow() {
    let mut inventory = Inventory::default();
    assert_eq!(inventory.insert(Block::Stone, 70), 0);
    assert_eq!(inventory.slots()[0].unwrap().count, 64);
    assert_eq!(inventory.slots()[1].unwrap().count, 6);
    assert_eq!(inventory.insert(Block::Stone, 10), 0);
    assert_eq!(inventory.slots()[1].unwrap().count, 16);
    assert_eq!(inventory.insert(Block::Stone, u16::MAX), 63_311);
    assert!(
        inventory
            .slots()
            .iter()
            .all(|slot| slot.unwrap().count == MAX_STACK)
    );
    assert_eq!(total(&inventory), 36 * 64);
    assert_eq!(inventory.insert(Block::Stone, 0), 0);
    assert_eq!(inventory.insert(Block::Dirt, 3), 3);
}

#[test]
fn noncollectible_materials_and_zero_counts_do_not_create_invalid_stacks() {
    let mut inventory = Inventory::default();
    for block in [Block::Air, Block::Water, Block::Lava, Block::Bedrock] {
        assert_eq!(inventory.insert(block, 5), 5);
        assert!(inventory.slots().iter().all(Option::is_none));
    }
    assert_eq!(inventory.insert(Block::Dirt, 0), 0);
    assert!(inventory.slots().iter().all(Option::is_none));
}

#[test]
fn empty_inventory_selection_wraps_only_the_hotbar_and_consumes_exactly_one() {
    let mut inventory = Inventory::default();
    assert!(inventory.slots().iter().all(Option::is_none));
    assert_eq!(inventory.consume_selected(), None);
    inventory.select(8);
    inventory.cycle(1);
    assert_eq!(inventory.selected(), 0);
    inventory.cycle(-1);
    assert_eq!(inventory.selected(), 8);
    inventory.cycle(i32::MAX);
    assert_eq!(inventory.selected(), 0);
    inventory.cycle(i32::MIN);
    assert_eq!(inventory.selected(), 7);
    inventory.select(9);
    inventory.select(usize::MAX);
    assert_eq!(inventory.selected(), 7);
    inventory.select(0);
    assert_eq!(inventory.insert(Block::Wood, 2), 0);
    assert_eq!(inventory.consume_selected(), Some(Block::Wood.into()));
    assert_eq!(inventory.selected_stack().unwrap().count, 1);
    assert_eq!(inventory.consume_selected(), Some(Block::Wood.into()));
    assert_eq!(inventory.selected_stack(), None);
}

#[test]
fn whole_stack_move_merge_and_swap_preserve_each_material() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Dirt, 70);
    inventory.insert(Block::Wood, 8);
    inventory.click_slot(1);
    inventory.click_slot(4);
    assert_eq!(inventory.slots()[4].unwrap().count, 6);
    assert_eq!(inventory.cursor_stack(), None);
    inventory.click_slot(4);
    inventory.click_slot(2);
    assert_eq!(
        inventory.slots()[2],
        Some(ItemStack {
            item: Block::Dirt.into(),
            count: 6
        })
    );
    assert_eq!(
        inventory.cursor_stack(),
        Some(ItemStack {
            item: Block::Wood.into(),
            count: 8
        })
    );
    inventory.close_cursor();
    assert_eq!(inventory.slots()[4].unwrap().item, Block::Wood.into());
    inventory.click_slot(2);
    inventory.click_slot(0);
    assert_eq!(inventory.cursor_stack().unwrap().count, 6);
    inventory.select(0);
    for _ in 0..4 {
        inventory.consume_selected();
    }
    inventory.click_slot(0);
    assert_eq!(inventory.slots()[0].unwrap().count, 64);
    assert_eq!(inventory.cursor_stack().unwrap().count, 2);
    inventory.close_cursor();
    assert_eq!(inventory.slots()[2].unwrap().count, 2);
    assert_eq!(total(&inventory), 74);
}

#[test]
fn reserved_source_guarantees_close_after_swap_and_concurrent_full_insertion() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Dirt, 36 * 64);
    inventory.select(1);
    for _ in 0..64 {
        inventory.consume_selected();
    }
    inventory.insert(Block::Wood, 64);
    inventory.click_slot(0);
    inventory.click_slot(1);
    assert_eq!(inventory.cursor_stack().unwrap().item, Block::Wood.into());
    assert_eq!(inventory.insert(Block::Stone, 64), 64);
    let count = total(&inventory);
    inventory.click_slot(36);
    inventory.click_slot(usize::MAX);
    inventory.close_cursor();
    inventory.close_cursor();
    assert_eq!(total(&inventory), count);
    assert_eq!(inventory.slots()[0].unwrap().item, Block::Wood.into());
    assert!(inventory.slots().iter().all(Option::is_some));
    assert_eq!(inventory.cursor_stack(), None);
}

#[test]
fn returning_to_the_source_releases_its_reservation() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Stone, 1);
    inventory.click_slot(35);
    assert_eq!(inventory.cursor_stack(), None);
    inventory.click_slot(0);
    assert_eq!(inventory.selected_stack(), None);
    assert_eq!(inventory.consume_selected(), None);
    inventory.click_slot(0);
    assert_eq!(inventory.selected_stack().unwrap().count, 1);
    assert_eq!(inventory.insert(Block::Stone, 63), 0);
    assert_eq!(inventory.selected_stack().unwrap().count, 64);
    assert_eq!(total(&inventory), 64);
}

#[test]
fn arbitrary_transfers_insertions_and_consumption_conserve_the_entire_inventory() {
    let mut inventory = Inventory::default();
    let mut expected = [0_u32; Block::COUNT];
    let mut state = 42_u32;
    for step in 0..4000 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        match step % 5 {
            0 => {
                let count = (state % 129) as u16;
                let block = [Block::Dirt, Block::Wood, Block::Stone][state as usize % 3];
                expected[block as usize] += u32::from(count - inventory.insert(block, count));
            }
            1 | 2 => inventory.click_slot(state as usize % 38),
            3 => inventory.close_cursor(),
            _ => {
                inventory.select(state as usize % 9);
                if let Some(item) = inventory.consume_selected() {
                    expected[item.block().unwrap() as usize] -= 1;
                }
            }
        }
        let mut actual = [0_u32; Block::COUNT];
        for stack in inventory
            .slots()
            .iter()
            .copied()
            .flatten()
            .chain(inventory.cursor_stack())
        {
            actual[stack.item.block().unwrap() as usize] += u32::from(stack.count);
        }
        assert_eq!(actual, expected);
        assert!(
            inventory
                .slots()
                .iter()
                .flatten()
                .chain(inventory.cursor_stack().iter())
                .all(|stack| (1..=MAX_STACK).contains(&stack.count))
        );
    }
    inventory.close_cursor();
    assert_eq!(total(&inventory), expected.into_iter().sum());
}

#[test]
fn every_material_has_a_stable_distinct_display_name() {
    for block in Block::ALL {
        assert!(!block_name(block).is_empty());
        for other in Block::ALL {
            if block != other {
                assert_ne!(block_name(block), block_name(other));
            }
        }
    }
}
