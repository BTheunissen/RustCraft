//! CPU capstone gathers through actual interactions, not fixture inventory grants.

use super::*;
use crate::game::test_support::arena;
use crate::world::WorldView;

#[test]
fn real_gathering_supplies_the_complete_small_recipe_list_without_access_gates() {
    let mut game = arena();
    for (block, count) in [
        (crate::world::Block::Wood, 5),
        (crate::world::Block::Stone, 5),
        (crate::world::Block::Crystal, 1),
    ] {
        for _ in 0..count {
            game.world.edit_block([8, 2, 7], block);
            for _ in 0..60 {
                game.interact(true, false, 0.05);
                if game.world.block_at(8, 2, 7) == Some(crate::world::Block::Air) {
                    break;
                }
            }
            for _ in 0..15 {
                game.interact(false, false, 0.05);
            }
        }
    }
    for _ in 0..5 {
        game.inventory_mut()
            .craft(RecipeId::Planks.index())
            .unwrap();
    }
    for recipe in [
        RecipeId::StonePickaxe,
        RecipeId::StoneAxe,
        RecipeId::StorageCrate,
        RecipeId::Lantern,
    ] {
        game.inventory_mut().craft(recipe.index()).unwrap();
    }
    assert!(game.pickups().is_empty());
    assert_eq!(
        game.inventory()
            .slots()
            .iter()
            .flatten()
            .filter(|stack| matches!(stack.item, Item::Tool(_)))
            .count(),
        2
    );
    assert!(game.snapshot().validate().is_ok());
}
