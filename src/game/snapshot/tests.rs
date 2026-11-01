use super::*;
use crate::game::test_support::{aim, arena, empty_chunk};
use crate::player::MovementInput;
use crate::world::{Chunk, ChunkCoord, SampledWorld, WorldView};

#[test]
fn restore_preserves_progress_but_not_residency_mining_or_ui_transfers() {
    let mut game = arena();
    game.world.edit_block([8, -10, 8], Block::Wood);
    game.world.edit_block([8, 1, 6], Block::Stone);
    game.inventory.insert(Block::Dirt, 70);
    game.inventory.insert(Block::Clay, 8);
    game.inventory.click_slot(2);
    game.inventory.click_slot(35);
    game.inventory.select(8);
    game.headlamp = false;
    game.pickups.push(Pickup {
        position: Vec3::new(-1.5, -30.0, -17.5),
        item: Block::Crystal.into(),
        count: 64,
        velocity: Vec3::new(0.0, -8.0, 0.0),
        age: 0.1,
    });
    aim(&mut game, Vec3::new(8.5, 1.5, 6.5));
    game.interact(true, false, 0.1);
    assert!(game.mining_progress() > 0.0);
    game.inventory.click_slot(0);
    let held = game.inventory.cursor_stack().unwrap();
    let snapshot = game.snapshot();
    assert_eq!(game.inventory.cursor_stack(), Some(held));
    assert_eq!(snapshot.slots[0], Some(held));
    let restored = Game::from_snapshot(snapshot.clone()).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert!(restored.world.is_empty());
    assert!(restored.inventory.cursor_stack().is_none());
    assert_eq!(restored.mining_progress(), 0.0);
    assert!(restored.target().is_none());
    assert_eq!(
        SampledWorld::new(&restored.world, &restored.terrain).block_at(8, -10, 8),
        Some(Block::Wood)
    );
}

#[test]
fn negative_coordinate_edits_survive_eviction_save_restore_and_regeneration() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let coord = ChunkCoord::new(-1, -2);
    game.world.insert(Chunk::generate(&game.terrain, coord));
    for (position, block) in [([-1, -63, -17], Block::Wood), ([-16, 79, -32], Block::Clay)] {
        assert!(game.world.edit_block(position, block));
    }
    game.world.remove(coord);
    let snapshot = game.snapshot();
    let mut restored = Game::from_snapshot(snapshot.clone()).unwrap();
    assert_ne!(
        game.world.edits().revision(),
        restored.world.edits().revision()
    );
    restored
        .world
        .insert(Chunk::generate(&restored.terrain, coord));
    for edit in snapshot.edits {
        let [x, y, z] = edit.position;
        assert_eq!(restored.world.block_at(x, y, z), Some(edit.block));
    }
    assert_eq!(restored.original_spawn, game.original_spawn);
}

#[test]
fn restored_drops_keep_collection_delays_suspend_when_unloaded_and_collect_once() {
    let mut game = arena();
    game.pickups.push(Pickup {
        position: Vec3::new(8.5, 1.125, 8.5),
        item: Block::Dirt.into(),
        count: 3,
        velocity: Vec3::ZERO,
        age: 0.1,
    });
    let mut restored = Game::from_snapshot(game.snapshot()).unwrap();
    restored.interact(false, false, 0.25);
    assert_eq!(restored.pickups.len(), 1);
    assert_eq!(restored.pickups[0].age, 0.1);
    restored.world.insert(empty_chunk(ChunkCoord::new(0, 0)));
    restored.interact(false, false, 0.05);
    assert_eq!(restored.pickups.len(), 1);
    for _ in 0..20 {
        restored.interact(false, false, 0.05);
    }
    assert!(restored.pickups.is_empty());
    assert_eq!(restored.inventory.selected_stack().unwrap().count, 3);
    restored.interact(false, false, 0.25);
    assert_eq!(restored.inventory.selected_stack().unwrap().count, 3);
}

#[test]
fn creative_flight_view_headlamp_and_speed_resume_without_input_latches() {
    let mut game = Game::new(42);
    game.player.position = Vec3::new(-512.5, 180.0, -16.5);
    game.toggle_flight();
    game.cycle_speed();
    game.cycle_speed();
    game.toggle_headlamp();
    game.look(40.0, 25.0);
    let snapshot = game.snapshot();
    let mut restored = Game::from_snapshot(snapshot.clone()).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    restored.update(MovementInput::default(), 0.1);
    assert_eq!(restored.player.position, snapshot.player.position);
    restored.respawn();
    assert_eq!(restored.speed(), SpeedMultiplier::Quadruple);
    assert!(!restored.headlamp());
}

#[test]
fn full_inventory_cursor_swap_is_lossless_and_keeps_storage_order() {
    let mut game = arena();
    game.inventory.insert(Block::Dirt, 35 * 64);
    game.inventory.insert(Block::Wood, 64);
    game.inventory.click_slot(0);
    game.inventory.click_slot(35);
    let restored = Game::from_snapshot(game.snapshot()).unwrap();
    assert_eq!(
        restored.inventory.slots()[0].unwrap().item,
        Block::Wood.into()
    );
    assert_eq!(
        restored.inventory.slots()[35].unwrap().item,
        Block::Dirt.into()
    );
    assert_eq!(
        restored
            .inventory
            .slots()
            .iter()
            .flatten()
            .map(|s| u32::from(s.count))
            .sum::<u32>(),
        36 * 64
    );
    assert_eq!(
        game.inventory.cursor_stack().unwrap().item,
        Block::Wood.into()
    );
}

#[test]
fn hostile_player_poses_and_survival_creative_settings_are_rejected() {
    let original = Game::with_mode(42, GameMode::Survival).snapshot();
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        let mut saved = original.clone();
        saved.player.position.x = invalid;
        assert!(Game::from_snapshot(saved).is_err());
    }
    for (yaw, pitch) in [(f32::NAN, 0.0), (0.0, f32::INFINITY), (0.0, 2.0)] {
        let mut saved = original.clone();
        saved.player.yaw = yaw;
        saved.player.pitch = pitch;
        assert!(saved.validate().is_err());
    }
    let mut saved = original.clone();
    saved.player.flying = true;
    assert!(saved.validate().is_err());
    let mut saved = original;
    saved.speed = SpeedMultiplier::Double;
    assert!(saved.validate().is_err());
}

#[test]
fn inventory_edit_and_pickup_validation_rejects_bad_counts_materials_and_coordinates() {
    let original = Game::with_mode(42, GameMode::Survival).snapshot();
    let mut saved = original.clone();
    saved.selected_slot = 9;
    assert!(saved.validate().is_err());
    for (block, count) in [
        (Block::Air, 1),
        (Block::Water, 1),
        (Block::Bedrock, 1),
        (Block::Dirt, 0),
        (Block::Wood, 65),
    ] {
        let mut saved = original.clone();
        saved.slots[35] = Some(ItemStack {
            item: block.into(),
            count,
        });
        assert!(saved.validate().is_err());
    }
    for (position, block) in [
        ([0, -65, 0], Block::Air),
        ([0, 80, 0], Block::Dirt),
        ([0, 0, 0], Block::Water),
        ([0, 0, 0], Block::Bedrock),
    ] {
        let mut saved = original.clone();
        saved.edits.push(BlockEdit { position, block });
        assert!(saved.validate().is_err());
    }
    let mut saved = original.clone();
    saved.edits = vec![
        BlockEdit {
            position: [0, 0, 0],
            block: Block::Air
        };
        2
    ];
    assert!(saved.validate().is_err());
    for (position, velocity, age, block, count) in [
        (Vec3::ZERO, Vec3::ZERO, -1.0, Block::Dirt, 1),
        (Vec3::ZERO, Vec3::ZERO, f32::NAN, Block::Dirt, 1),
        (Vec3::ZERO, Vec3::splat(f32::INFINITY), 1.0, Block::Dirt, 1),
        (Vec3::ZERO, Vec3::splat(65.0), 1.0, Block::Dirt, 1),
        (Vec3::splat(f32::NAN), Vec3::ZERO, 1.0, Block::Dirt, 1),
        (Vec3::new(0.0, 80.0, 0.0), Vec3::ZERO, 1.0, Block::Dirt, 1),
        (Vec3::ZERO, Vec3::ZERO, 1.0, Block::Air, 1),
        (Vec3::ZERO, Vec3::ZERO, 1.0, Block::Dirt, 0),
    ] {
        let mut saved = original.clone();
        saved.pickups.push(SavedPickup {
            position,
            velocity,
            age,
            item: block.into(),
            count,
        });
        assert!(Game::from_snapshot(saved).is_err());
    }
}

#[test]
fn save_size_limits_reject_progress_instead_of_truncating_it() {
    let mut saved = Game::new(42).snapshot();
    saved.edits = vec![
        BlockEdit {
            position: [0, 0, 0],
            block: Block::Air
        };
        MAX_SAVED_EDITS + 1
    ];
    assert!(saved.validate().unwrap_err().contains("limit"));
    saved.edits.clear();
    saved.pickups = vec![
        SavedPickup {
            position: Vec3::ZERO,
            item: Block::Dirt.into(),
            count: 1,
            velocity: Vec3::ZERO,
            age: 1.0,
        };
        MAX_SAVED_PICKUPS + 1
    ];
    assert!(saved.validate().unwrap_err().contains("limit"));
}
