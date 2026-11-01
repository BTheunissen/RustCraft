use super::*;
use crate::game::test_support::arena;
use crate::game::{Item, Tool};

#[test]
fn selected_tools_never_place_or_consume_and_only_suitable_tools_accelerate() {
    for tool in Tool::ALL {
        let mut game = arena();
        game.world.edit_block([8, 2, 6], Block::Stone);
        game.inventory_mut().insert(tool, 1);
        let snapshot = game.snapshot();
        assert!(!game.place_selected());
        assert_eq!(game.snapshot(), snapshot);
        assert!(game.take_sound_events().iter().all(Option::is_none));
        let duration = 2.5 / tool.speed_for(Block::Stone);
        let steps = (duration / 0.05).ceil() as usize;
        for _ in 0..steps - 1 {
            game.interact(true, false, 0.05);
        }
        assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Stone));
        game.interact(true, false, 0.05);
        assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Air));
        assert_eq!(
            game.inventory().selected_stack().unwrap().item,
            Item::Tool(tool)
        );
        assert_eq!(game.inventory().selected_stack().unwrap().count, 1);
    }
}

#[test]
fn processed_materials_keep_hand_durations_and_collected_identity() {
    for (block, duration) in [(Block::Planks, 1.5), (Block::DressedLimestone, 2.5)] {
        let mut game = arena();
        game.world.edit_block([8, 2, 6], block);
        for _ in 0..(duration / 0.1_f32).round() as usize - 1 {
            game.interact(true, false, 0.1);
        }
        assert_eq!(game.world.block_at(8, 2, 6), Some(block));
        game.interact(true, false, 0.1);
        assert_eq!(game.pickups()[0].item, block.into());
        assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Air));
    }
}
