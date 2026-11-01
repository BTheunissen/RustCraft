use super::test_support::{aim, arena};
use super::*;
use crate::world::{Block, Chunk, ChunkCoord};

#[test]
fn survival_disables_flight_and_speed_actions_without_disabling_headlamp_or_recovery() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    assert_eq!(game.mode(), GameMode::Survival);
    assert_eq!(Game::new(42).mode(), GameMode::Creative);
    for _ in 0..6 {
        game.toggle_flight();
        game.cycle_speed();
        assert!(!game.player().flying);
        assert_eq!(game.speed(), SpeedMultiplier::Normal);
    }
    game.inventory_mut().insert(Block::Wood, 3);
    game.toggle_headlamp();
    game.respawn();
    assert!(!game.headlamp());
    assert_eq!(game.inventory().selected_stack().unwrap().count, 3);
    assert_eq!(game.player().position, game.original_spawn);
}

#[test]
fn creative_sessions_do_not_target_mine_place_or_advance_pickups() {
    let mut game = arena();
    game.mode = GameMode::Creative;
    game.world.edit_block([8, 2, 6], Block::Dirt);
    game.inventory_mut().insert(Block::Wood, 4);
    let revision = game.world.edits().revision();
    for _ in 0..10 {
        game.interact(true, true, 0.25);
    }
    assert_eq!(game.target(), None);
    assert_eq!(game.mining_progress(), 0.0);
    assert!(game.pickups().is_empty());
    assert!(!game.place_selected());
    assert_eq!(game.world.edits().revision(), revision);
    assert_eq!(game.inventory().selected_stack().unwrap().count, 4);
}

#[test]
fn complete_mine_collect_and_place_loop_conserves_the_block_and_revisions() {
    let mut game = arena();
    game.world.edit_block([8, 2, 6], Block::Dirt);
    game.world.edit_block([8, 2, 5], Block::Bedrock);
    let initial_revision = game.world.edits().revision();
    for _ in 0..3 {
        game.interact(true, false, 0.2);
    }
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Air));
    assert_eq!(game.pickups().len(), 1);
    assert!(game.inventory().selected_stack().is_none());
    let mined_revision = game.world.edits().revision();
    assert_ne!(mined_revision, initial_revision);
    game.clear_input();
    game.player.position.z = 7.5;
    for _ in 0..6 {
        game.interact(false, false, 0.1);
    }
    assert!(game.pickups().is_empty());
    assert_eq!(
        game.inventory().selected_stack(),
        Some(ItemStack {
            item: Block::Dirt.into(),
            count: 1
        })
    );
    assert!(game.place_selected());
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Dirt));
    assert_eq!(game.inventory().selected_stack(), None);
    assert_ne!(game.world.edits().revision(), mined_revision);
    assert!(!game.place_selected());
    assert_eq!(game.world.block_at(8, 2, 6), Some(Block::Dirt));
}

#[test]
fn respawn_and_fall_recovery_respect_evicted_edits_and_preserve_inventory_and_headlamp() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let original = game.original_spawn;
    let [x, y, z] = original.floor().as_ivec3().to_array();
    let coord = ChunkCoord::from_world(original.x, original.z);
    game.world.insert(Chunk::generate(&game.terrain, coord));
    assert!(game.world.edit_block([x, y, z], Block::Stone));
    game.inventory_mut().insert(Block::GoldOre, 5);
    game.toggle_headlamp();
    game.world.remove(coord);
    game.respawn();
    assert_eq!(game.player.position, original + Vec3::Y);
    assert!(!game.headlamp());
    assert_eq!(game.inventory().selected_stack().unwrap().count, 5);
    game.player.position.y = -85.0;
    game.update(MovementInput::default(), 1.0 / 120.0);
    assert_eq!(game.player.position, original + Vec3::Y);
    assert!(!game.player.flying);
}

#[test]
fn edited_surface_roof_invalidates_stationary_camera_light_then_removal_restores_it() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let [x, y, z] = game.player.eye_position().floor().as_ivec3().to_array();
    let coord = ChunkCoord::from_world(x as f32, z as f32);
    game.world.insert(Chunk::generate(&game.terrain, coord));
    let outdoors = game.environment();
    assert!(!outdoors.underground);
    assert_eq!(outdoors.sky_exposure, 1.0);
    let roof = [x, y + 2, z];
    assert!(game.world.edit_block(roof, Block::Wood));
    let covered = game.environment();
    assert!(covered.underground);
    assert_eq!(covered.sky_exposure, 0.0);
    assert_eq!(covered.biome, outdoors.biome);
    assert_eq!(game.environment().sky_exposure, 0.0);
    game.world.remove(coord);
    assert_eq!(game.environment().sky_exposure, 0.0);
    game.world.insert(Chunk::generate(&game.terrain, coord));
    assert!(game.world.edit_block(roof, Block::Air));
    let restored = game.environment();
    assert!(!restored.underground);
    assert_eq!(restored.sky_exposure, 1.0);
}

#[test]
fn added_leaves_preserve_the_existing_outdoor_foliage_atmosphere() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let [x, y, z] = game.player.eye_position().floor().as_ivec3().to_array();
    let coord = ChunkCoord::from_world(x as f32, z as f32);
    game.world.insert(Chunk::generate(&game.terrain, coord));
    game.world.edit_block([x, y + 2, z], Block::Leaves);
    let environment = game.environment();
    assert!(!environment.underground);
    assert_eq!(environment.sky_exposure, 1.0);
    game.player.pitch = 0.0;
    aim(
        &mut game,
        Vec3::new(x as f32 + 0.5, y as f32 + 2.5, z as f32 + 0.5),
    );
    game.interact(false, false, 0.0);
    assert_eq!(game.target().unwrap().block, Block::Leaves);
}
