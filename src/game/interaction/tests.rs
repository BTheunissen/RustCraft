use super::*;
use crate::game::test_support::{aim, arena};
use crate::player::MovementInput;

#[test]
fn soft_wood_and_rock_mining_require_their_whole_duration_and_spawn_once() {
    for (block, duration) in [
        (Block::Dirt, 0.6),
        (Block::Wood, 1.5),
        (Block::CoalOre, 2.5),
    ] {
        let mut game = arena();
        let position = [8, 2, 6];
        game.world.edit_block(position, block);
        let mut elapsed = 0.0;
        while elapsed + 0.1 < duration - 0.000_001 {
            game.interact(true, false, 0.1);
            elapsed += 0.1;
            assert_eq!(game.world.block_at(8, 2, 6), Some(block));
            assert!(game.pickups().is_empty());
        }
        assert!(game.mining_progress() > 0.0 && game.mining_progress() < 1.0);
        game.interact(true, false, duration - elapsed);
        assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Air));
        assert_eq!(game.pickups().len(), 1);
        assert_eq!(game.pickups()[0].item, block.into());
        assert_eq!(game.pickups()[0].count, 1);
        assert_eq!(game.pickups()[0].age(), 0.0);
        assert_eq!(game.mining_progress(), 0.0);
        for _ in 0..4 {
            game.interact(true, false, 0.1);
        }
        assert_eq!(
            game.pickups()
                .iter()
                .map(|pickup| pickup.count)
                .sum::<u16>(),
            1
        );
    }
}

#[test]
fn release_target_change_material_change_and_pause_restart_mining() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Dirt);
    game.world.edit_block([9, 2, 6], Block::Wood);
    game.interact(true, false, 0.25);
    assert!(game.mining_progress() > 0.4);
    game.interact(false, false, 0.0);
    assert_eq!(game.mining_progress(), 0.0);
    game.interact(true, false, 0.2);
    game.cancel_mining();
    assert_eq!(game.mining_progress(), 0.0);
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 1.0 / 6.0).abs() < 0.000_01);
    aim(&mut game, Vec3::new(9.5, 2.5, 6.5));
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 1.0 / 15.0).abs() < 0.000_01);
    game.world.edit_block([9, 2, 6], Block::Stone);
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 0.04).abs() < 0.000_01);
    game.clear_input();
    assert_eq!(game.mining_progress(), 0.0);
    game.interact(true, false, 0.1);
    assert!((game.mining_progress() - 0.04).abs() < 0.000_01);
}

#[test]
fn dt_is_bounded_finite_and_never_accumulates_invalid_time() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Dirt);
    for dt in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0] {
        game.interact(true, false, dt);
        assert_eq!(game.mining_progress(), 0.0);
    }
    game.interact(true, false, 100.0);
    assert!((game.mining_progress() - 0.25 / 0.6).abs() < 0.000_01);
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Dirt));
}

#[test]
fn every_solid_except_bedrock_has_a_duration_and_liquids_are_not_mined() {
    for block in Block::ALL {
        assert_eq!(
            mining_duration(block).is_some(),
            block.is_solid() && block != Block::Bedrock
        );
    }
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Bedrock);
    for _ in 0..20 {
        game.interact(true, false, 0.25);
    }
    assert_eq!(game.target().unwrap().block, Block::Bedrock);
    assert_eq!(game.mining_progress(), 0.0);
    assert!(game.pickups().is_empty());
    game.world.edit_block([8, 2, 6], Block::Water);
    game.interact(true, false, 0.25);
    assert!(game.target().is_none());
    game.world.edit_block([8, 2, 6], Block::Lava);
    game.interact(true, false, 0.25);
    assert!(game.target().is_none());
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Lava));
}

#[test]
fn target_refreshes_without_clicks_and_unloaded_targets_reset_progress() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Dirt);
    game.interact(false, false, 0.0);
    assert_eq!(game.target().unwrap().position, [8, 2, 6]);
    game.interact(true, false, 0.2);
    game.world.remove(crate::world::ChunkCoord::new(0, 0));
    game.interact(true, false, 0.2);
    assert_eq!(game.target(), None);
    assert_eq!(game.mining_progress(), 0.0);
    assert!(game.pickups().is_empty());
}

#[test]
fn placement_consumes_one_only_on_success_and_only_on_physical_press_pulses() {
    let mut game = arena();
    game.world.edit_block([8, 2, 5], Block::Stone);
    game.inventory.insert(Block::Wood, 3);
    let revision = game.world.edits().revision();
    game.interact(false, true, 0.0);
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Wood));
    assert_ne!(game.world.edits().revision(), revision);
    assert_eq!(game.inventory.selected_stack().unwrap().count, 2);
    for _ in 0..5 {
        game.interact(false, false, 0.1);
    }
    assert_eq!(game.world.block_at(8, 2, 7), Some(Block::Air));
    assert_eq!(game.inventory.selected_stack().unwrap().count, 2);
    game.interact(false, true, 0.0);
    assert_eq!(game.world.block_at(8, 2, 7), Some(Block::Wood));
    assert_eq!(game.inventory.selected_stack().unwrap().count, 1);
    game.interact(false, true, 0.0);
    assert_eq!(game.world.block_at(8, 2, 8), Some(Block::Air));
    assert_eq!(game.inventory.selected_stack().unwrap().count, 1);
}

#[test]
fn invalid_placement_paths_leave_item_counts_and_world_revision_unchanged() {
    let mut game = arena();
    game.world.edit_block([8, 2, 5], Block::Stone);
    assert!(!game.place_selected());
    game.inventory.insert(Block::Dirt, 5);
    for block in [Block::Water, Block::Lava] {
        game.world.edit_block([8, 2, 6], block);
        let revision = game.world.edits().revision();
        assert!(!game.place_selected());
        assert_eq!(game.inventory.selected_stack().unwrap().count, 5);
        assert_eq!(game.world.edits().revision(), revision);
    }
    game.world.edit_block([8, 2, 6], Block::Air);
    game.inventory.click_slot(0);
    assert!(!game.place_selected());
    assert_eq!(game.inventory.cursor_stack().unwrap().count, 5);
    game.inventory.close_cursor();
    game.player.yaw = std::f32::consts::PI;
    assert!(!game.place_selected());
    game.player.yaw = 0.0;
    game.world.remove(crate::world::ChunkCoord::new(0, 0));
    assert!(!game.place_selected());
    assert_eq!(game.inventory.selected_stack().unwrap().count, 5);
}

#[test]
fn placement_excludes_player_body_and_out_of_world_roofs() {
    let mut game = arena();
    game.inventory.insert(Block::Dirt, 2);
    game.world.edit_block([8, 2, 7], Block::Stone);
    assert!(!game.place_selected());
    assert_eq!(game.world.block_at(8, 2, 8), Some(Block::Air));
    game.world
        .edit_block([8, WORLD_HEIGHT - 1, 7], Block::Stone);
    game.player.position = Vec3::new(8.5, WORLD_HEIGHT as f32 + 1.0, 7.5);
    game.player.pitch = -std::f32::consts::FRAC_PI_2;
    assert!(!game.place_selected());
    assert_eq!(game.inventory.selected_stack().unwrap().count, 2);
}

#[test]
fn placement_cannot_entomb_an_uncollected_pickup_or_consume_the_held_stack() {
    let mut game = arena();
    game.world.edit_block([8, 2, 5], Block::Stone);
    game.inventory.insert(Block::Wood, 2);
    game.pickups
        .push(Pickup::new(Vec3::new(8.5, 2.5, 6.5), Block::Dirt, 1));
    let revision = game.world.edits().revision();
    assert!(!game.place_selected());
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Air));
    assert_eq!(game.world.edits().revision(), revision);
    assert_eq!(game.inventory.selected_stack().unwrap().count, 2);
    assert_eq!(game.pickups[0].count, 1);
    game.pickups[0].position.x = 9.5;
    assert!(game.place_selected());
    assert_eq!(game.inventory.selected_stack().unwrap().count, 1);
    assert_eq!(game.pickups[0].count, 1);
}

#[test]
fn mining_release_does_not_rearm_player_jump_but_clear_input_does() {
    let mut game = arena();
    let jump = MovementInput {
        jump: true,
        ..Default::default()
    };
    game.update(jump, 1.0 / 120.0);
    game.cancel_mining();
    for _ in 0..200 {
        game.update(jump, 1.0 / 120.0);
    }
    assert!(game.player.is_grounded());
    game.update(jump, 1.0 / 120.0);
    assert_eq!(game.player.position.y, 1.0);
    game.clear_input();
    game.update(jump, 1.0 / 120.0);
    assert!(game.player.position.y > 1.0);
}
