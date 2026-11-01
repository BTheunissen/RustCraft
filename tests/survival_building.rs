//! Survival's public CPU contract, without a window or GPU.

use rustcraft::game::{Game, GameMode};
use rustcraft::player::{MovementInput, SpeedMultiplier};
use rustcraft::world::{Block, Chunk, ChunkCoord, SampledWorld, WorldView};

#[test]
fn mine_collect_move_equip_place_and_reload_preserve_one_block() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let terrain = game.terrain();
    let feet = game.player().position;
    let coord = ChunkCoord::from_world(feet.x, feet.z);
    game.world_mut().insert(Chunk::generate(&terrain, coord));
    let source = [
        feet.x.floor() as i32,
        feet.y.floor() as i32 - 1,
        feet.z.floor() as i32,
    ];
    let original = game
        .world()
        .block_at(source[0], source[1], source[2])
        .unwrap();
    assert!(original.is_solid() && original != Block::Bedrock);
    game.look(0.0, 1000.0);
    game.interact(false, false, 0.0);
    assert_eq!(game.target().unwrap().position, source);

    for _ in 0..80 {
        game.interact(true, false, 0.05);
        if game.world().block_at(source[0], source[1], source[2]) == Some(Block::Air) {
            break;
        }
    }
    assert_eq!(
        game.world().block_at(source[0], source[1], source[2]),
        Some(Block::Air)
    );
    assert_eq!(
        game.pickups()
            .iter()
            .map(|pickup| pickup.count)
            .sum::<u16>(),
        1
    );
    game.cancel_mining();
    game.world_mut().remove(coord);
    assert_eq!(
        SampledWorld::new(game.world(), &terrain).block_at(source[0], source[1], source[2]),
        Some(Block::Air)
    );
    game.world_mut().insert(Chunk::generate(&terrain, coord));
    assert_eq!(
        game.world().block_at(source[0], source[1], source[2]),
        Some(Block::Air)
    );

    for _ in 0..20 {
        game.update(MovementInput::default(), 0.05);
        game.interact(false, false, 0.05);
    }
    assert!(game.pickups().is_empty());
    let stack = game.inventory().selected_stack().unwrap();
    assert_eq!((stack.item, stack.count), (original.into(), 1));
    game.inventory_mut().click_slot(0);
    game.inventory_mut().click_slot(8);
    game.inventory_mut().close_cursor();
    game.inventory_mut().select(8);
    assert_eq!(game.inventory().selected_stack(), Some(stack));

    game.respawn();
    let destination = (0..8)
        .find_map(|index| {
            let yaw = index as f32 * std::f32::consts::TAU / 8.0;
            let pitch = -0.5;
            game.look(
                f64::from(yaw - game.player().yaw) / 0.0025,
                -f64::from(pitch - game.player().pitch) / 0.0025,
            );
            game.interact(false, false, 0.0);
            let hit = game.target()?;
            let destination =
                std::array::from_fn::<_, 3, _>(|axis| hit.position[axis] + hit.normal[axis]);
            game.interact(false, true, 0.0);
            game.inventory()
                .selected_stack()
                .is_none()
                .then_some(destination)
        })
        .expect("a supported spawn must permit nearby block placement");
    assert_eq!(
        game.world()
            .block_at(destination[0], destination[1], destination[2]),
        Some(original)
    );
    game.world_mut().remove(coord);
    game.world_mut().insert(Chunk::generate(&terrain, coord));
    assert_eq!(
        game.world()
            .block_at(destination[0], destination[1], destination[2]),
        Some(original)
    );
    assert_eq!(
        game.world().block_at(source[0], source[1], source[2]),
        Some(Block::Air)
    );
    assert!(!game.player().flying);
}

#[test]
fn new_session_is_empty_and_creative_actions_remain_mode_specific() {
    let mut survival = Game::with_mode(42, GameMode::Survival);
    survival.inventory_mut().insert(Block::Wood, 12);
    survival.toggle_flight();
    survival.cycle_speed();
    assert!(!survival.player().flying);
    assert_eq!(survival.speed(), SpeedMultiplier::Normal);
    survival.respawn();
    assert_eq!(survival.inventory().selected_stack().unwrap().count, 12);
    let fresh = Game::with_mode(42, GameMode::Survival);
    assert!(fresh.inventory().slots().iter().all(Option::is_none));
    assert!(fresh.pickups().is_empty());
    assert_eq!(fresh.world().edits().revision(), 0);
    let mut creative = Game::new(42);
    creative.toggle_flight();
    creative.cycle_speed();
    assert!(creative.player().flying);
    assert_eq!(creative.speed(), SpeedMultiplier::Double);
    creative.interact(true, true, 0.25);
    assert!(creative.pickups().is_empty());
    assert!(creative.target().is_none());
    assert_eq!(creative.world().edits().revision(), 0);
    creative.update(MovementInput::default(), 0.1);
}
