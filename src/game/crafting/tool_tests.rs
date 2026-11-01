use super::*;

#[test]
fn tool_craft_no_capacity_or_missing_second_ingredient_cannot_partially_consume() {
    let slots = std::array::from_fn(|index| {
        Some(ItemStack {
            item: match index {
                0 => Block::Planks.into(),
                _ => Block::Stone.into(),
            },
            count: 64,
        })
    });
    let mut inventory = Inventory::from_saved_slots(slots, 0).unwrap();
    let before = inventory.clone();
    assert_eq!(
        inventory.craft(RecipeId::StonePickaxe.index()),
        Err(CraftError::NoCapacity)
    );
    assert_eq!(inventory, before);
    let mut inventory = Inventory::default();
    inventory.insert(Block::Planks, 4);
    inventory.insert(Block::Stone, 2);
    let before = inventory.clone();
    assert_eq!(
        inventory.craft(RecipeId::StonePickaxe.index()),
        Err(CraftError::MissingIngredients)
    );
    assert_eq!(inventory, before);
    inventory.craft(RecipeId::StoneAxe.index()).unwrap();
    assert_eq!(inventory.slots().iter().flatten().count(), 1);
    assert_eq!(inventory.slots()[0].unwrap().item, Tool::StoneAxe.into());
    assert_eq!(inventory.slots()[0].unwrap().count, 1);
}
