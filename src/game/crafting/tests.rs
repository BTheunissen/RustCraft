use super::*;
use crate::game::{HOTBAR_SLOTS, INVENTORY_SLOTS};

#[test]
fn exact_cost_output_and_repeat_crafts_are_lossless() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Wood, 2);
    inventory.insert(Block::Limestone, 1);
    for _ in 0..2 {
        inventory.craft(RecipeId::Planks.index()).unwrap();
    }
    inventory.craft(RecipeId::DressedLimestone.index()).unwrap();
    assert_eq!(
        inventory
            .slots()
            .iter()
            .flatten()
            .filter(|stack| stack.item == Block::Planks.into())
            .map(|stack| stack.count)
            .sum::<u16>(),
        8
    );
    assert_eq!(
        inventory
            .slots()
            .iter()
            .flatten()
            .filter(|stack| stack.item == Block::DressedLimestone.into())
            .map(|stack| stack.count)
            .sum::<u16>(),
        1
    );
    let before = inventory.clone();
    assert_eq!(
        inventory.craft(RecipeId::Planks.index()),
        Err(CraftError::MissingIngredients)
    );
    assert_eq!(inventory, before);
    assert_eq!(inventory.craft(usize::MAX), Err(CraftError::UnknownRecipe));
    assert_eq!(inventory, before);
}

#[test]
fn consumed_slots_count_toward_capacity_and_partial_inputs_do_not_lose_overflow() {
    let slots = std::array::from_fn(|index| {
        Some(ItemStack {
            item: if index == 0 {
                Block::Wood.into()
            } else {
                Block::Stone.into()
            },
            count: if index == 0 { 1 } else { 64 },
        })
    });
    let mut inventory = Inventory::from_saved_slots(slots, 0).unwrap();
    inventory.craft(0).unwrap();
    assert_eq!(
        inventory.slots()[0],
        Some(ItemStack {
            item: Block::Planks.into(),
            count: 4
        })
    );
    let mut slots = *inventory.slots();
    slots[0] = Some(ItemStack {
        item: Block::Wood.into(),
        count: 2,
    });
    let mut blocked = Inventory::from_saved_slots(slots, HOTBAR_SLOTS - 1).unwrap();
    let before = blocked.clone();
    assert_eq!(blocked.can_craft(0), Err(CraftError::NoCapacity));
    assert_eq!(blocked.craft(0), Err(CraftError::NoCapacity));
    assert_eq!(blocked, before);
}

#[test]
fn output_merges_without_touching_cursor_or_its_split_reservation() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Wood, 4);
    inventory.insert(Block::Planks, 62);
    inventory.right_click_slot(0);
    let before = inventory.clone();
    assert_eq!(inventory.craft(0), Err(CraftError::MissingIngredients));
    assert_eq!(inventory, before);
    inventory.close_cursor();
    inventory.craft(0).unwrap();
    assert_eq!(inventory.slots()[1].unwrap().count, 64);
    assert_eq!(
        inventory
            .slots()
            .iter()
            .flatten()
            .filter(|stack| stack.item == Block::Planks.into())
            .map(|stack| stack.count)
            .sum::<u16>(),
        66
    );
    let source = inventory
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Block::Planks.into()))
        .unwrap();
    inventory.click_slot(source);
    let held = inventory.cursor_stack();
    inventory.craft(1).unwrap_err();
    assert_eq!(inventory.cursor_stack(), held);
    assert_eq!(inventory.slots().len(), INVENTORY_SLOTS);
}

#[test]
fn crate_and_lantern_exact_costs_make_complete_outputs_without_fuel_or_gates() {
    let mut inventory = Inventory::default();
    inventory.insert(Block::Planks, 10);
    inventory.insert(Block::Crystal, 1);
    inventory.craft(RecipeId::StorageCrate.index()).unwrap();
    inventory.craft(RecipeId::Lantern.index()).unwrap();
    assert_eq!(
        inventory
            .slots()
            .iter()
            .flatten()
            .map(|stack| stack.count)
            .sum::<u16>(),
        2
    );
    assert!(
        inventory
            .slots()
            .iter()
            .flatten()
            .any(|stack| stack.item == Block::StorageCrate.into())
    );
    assert!(
        inventory
            .slots()
            .iter()
            .flatten()
            .any(|stack| stack.item == Block::Lantern.into())
    );
    let before = inventory.clone();
    assert_eq!(
        inventory.craft(RecipeId::Lantern.index()),
        Err(CraftError::MissingIngredients)
    );
    assert_eq!(inventory, before);
}
