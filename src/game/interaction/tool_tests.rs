use super::*;
use crate::game::test_support::arena;
use crate::game::{RecipeId, SoundAction};

fn mine_time(block: Block, tool: Option<Tool>) -> (f32, Game) {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], block);
    if let Some(tool) = tool {
        game.inventory_mut().insert(tool, 1);
    }
    let mut time = 0.0;
    while game.world.block_at(8, 2, 6) == Some(block) && time < 4.0 {
        game.interact(true, false, 0.01);
        time += 0.01;
    }
    (time, game)
}

#[test]
fn permanent_tools_are_three_times_faster_only_for_the_frozen_material_groups() {
    for (block, tool, appropriate) in [
        (Block::Stone, Tool::StonePickaxe, true),
        (Block::CoalOre, Tool::StonePickaxe, true),
        (Block::DressedLimestone, Tool::StonePickaxe, true),
        (Block::Crystal, Tool::StonePickaxe, true),
        (Block::Wood, Tool::StoneAxe, true),
        (Block::Planks, Tool::StoneAxe, true),
        (Block::Dirt, Tool::StoneAxe, false),
        (Block::Stone, Tool::StoneAxe, false),
        (Block::Wood, Tool::StonePickaxe, false),
        (Block::Gravel, Tool::StonePickaxe, false),
    ] {
        let (hand, _) = mine_time(block, None);
        let (equipped, mut game) = mine_time(block, Some(tool));
        let factor = if appropriate { 3.0 } else { 1.0 };
        assert!(
            (hand / factor - equipped).abs() <= 0.015,
            "{block:?} {tool:?}: {hand}/{equipped}"
        );
        assert_eq!(game.pickups().len(), 1);
        assert_eq!(game.pickups()[0].item, block.into());
        assert_eq!(game.inventory().selected_stack().unwrap().count, 1);
        assert_eq!(
            game.take_sound_events()
                .iter()
                .flatten()
                .filter(|event| matches!(event.action, SoundAction::Break(_)))
                .count(),
            1
        );
    }
    for block in Block::ALL {
        assert!([1.0, 3.0].contains(&Tool::StonePickaxe.speed_for(block)));
    }
}

#[test]
fn switching_active_tools_resets_damage_and_normalized_progress_uses_current_duration() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory_mut().insert(Tool::StonePickaxe, 1);
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.interact(true, false, 0.2);
    assert!((game.mining_progress() - 0.24).abs() < 0.00001);
    game.inventory_mut().select(1);
    assert_eq!(game.mining_progress(), 0.0);
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 0.04).abs() < 0.00001);
    game.clear_input();
    assert_eq!(game.mining_progress(), 0.0);
    for block in [Block::Bedrock, Block::Water, Block::Lava] {
        game.world.edit_block([8, 2, 6], block);
        game.interact(true, false, 0.25);
        assert_eq!(game.world.block_at(8, 2, 6), Some(block));
    }
}

#[test]
fn live_tool_recipes_have_exact_costs_and_do_not_require_the_tool_they_create() {
    let mut game = arena();
    game.inventory_mut().insert(Block::Wood, 2);
    game.inventory_mut().insert(Block::Stone, 5);
    for _ in 0..2 {
        game.inventory_mut()
            .craft(RecipeId::Planks.index())
            .unwrap();
    }
    game.inventory_mut()
        .craft(RecipeId::StonePickaxe.index())
        .unwrap();
    game.inventory_mut()
        .craft(RecipeId::StoneAxe.index())
        .unwrap();
    let snapshot = game.snapshot();
    let restored = Game::from_snapshot(snapshot.clone()).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        game.inventory()
            .slots()
            .iter()
            .flatten()
            .filter(|stack| matches!(stack.item, Item::Tool(_)))
            .count(),
        2
    );
    assert_eq!(
        game.inventory()
            .slots()
            .iter()
            .flatten()
            .map(|stack| stack.count)
            .sum::<u16>(),
        2
    );
}

#[test]
fn switching_away_and_back_between_updates_still_resets_active_damage() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Stone);
    game.inventory_mut().insert(Tool::StonePickaxe, 1);
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.interact(true, false, 0.2);
    assert!(game.mining_progress() > 0.2);
    game.inventory_mut().select(1);
    game.inventory_mut().select(0);
    assert_eq!(game.mining_progress(), 0.0);
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 0.12).abs() < 0.00001);
}
