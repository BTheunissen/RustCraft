use super::*;
use crate::game::MAX_STACK;
use crate::game::Tool;
use crate::game::pickup_physics;
use crate::game::test_support::{arena, empty_chunk, world};
use crate::world::Block;
use crate::world::{ChunkCoord, WorldView};

fn drop_at(position: Vec3, block: Block, count: u16) -> Pickup {
    let mut pickup = Pickup::new(position, block, count);
    pickup.velocity = Vec3::ZERO;
    pickup
}

#[test]
fn dropped_tools_keep_individual_caps_and_capacity_remainders() {
    let game = arena();
    let mut inventory = Inventory::default();
    inventory.insert(Block::Dirt, 35 * MAX_STACK);
    let mut pickups = vec![
        Pickup::new(Vec3::new(8.5, 1.3, 8.5), Tool::StonePickaxe, 1),
        Pickup::new(Vec3::new(8.5, 1.3, 8.5), Tool::StonePickaxe, 1),
    ];
    for pickup in &mut pickups {
        pickup.age = 1.0;
        pickup.velocity = Vec3::ZERO;
    }
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.0,
    );
    assert_eq!(
        inventory.slots()[35].unwrap().item,
        Tool::StonePickaxe.into()
    );
    assert_eq!(inventory.slots()[35].unwrap().count, 1);
    assert_eq!(pickups.len(), 1);
    assert_eq!(pickups[0].count, 1);
}

#[test]
fn automatic_collection_waits_for_delay_and_keeps_capacity_remainder() {
    let game = arena();
    let mut inventory = Inventory::default();
    inventory.insert(Block::Dirt, 36 * MAX_STACK - 2);
    let mut pickups = vec![drop_at(Vec3::new(8.5, 1.2, 8.5), Block::Dirt, 5)];
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.2,
    );
    assert_eq!(pickups[0].count, 5);
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.2,
    );
    assert_eq!(pickups[0].count, 3);
    assert!(
        inventory
            .slots()
            .iter()
            .all(|slot| slot.unwrap().count == MAX_STACK)
    );
    let total = pickups[0].count;
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.2,
    );
    assert_eq!(pickups[0].count, total);
    inventory.select(0);
    inventory.consume_selected();
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.1,
    );
    assert_eq!(pickups[0].count, 2);
}

#[test]
fn nearby_pickups_merge_matching_stacks_only_and_never_exceed_capacity() {
    let game = arena();
    let mut pickups = vec![
        drop_at(Vec3::new(3.5, 1.2, 3.5), Block::Stone, 60),
        drop_at(Vec3::new(3.6, 1.2, 3.5), Block::Stone, 10),
        drop_at(Vec3::new(3.5, 1.2, 3.7), Block::Wood, 3),
        drop_at(Vec3::new(5.5, 1.2, 3.5), Block::Stone, 5),
    ];
    update(
        &mut pickups,
        &game.world,
        &mut Inventory::default(),
        game.player.position,
        0.0,
    );
    assert_eq!(pickups[0].count, 64);
    assert_eq!(pickups[1].count, 6);
    assert_eq!(pickups.iter().map(|pickup| pickup.count).sum::<u16>(), 78);
    assert_eq!(pickups[2].item, Block::Wood.into());
    assert_eq!(pickups[3].count, 5);
}

#[test]
fn merging_a_new_drop_preserves_its_collection_delay() {
    let game = arena();
    let mut old = drop_at(Vec3::new(8.5, 1.2, 8.5), Block::Dirt, 2);
    old.age = 3.0;
    let new = drop_at(Vec3::new(8.6, 1.2, 8.5), Block::Dirt, 1);
    let mut pickups = vec![old, new];
    let mut inventory = Inventory::default();
    update(
        &mut pickups,
        &game.world,
        &mut inventory,
        game.player.position,
        0.0,
    );
    assert_eq!(pickups.len(), 1);
    assert_eq!(pickups[0].count, 3);
    assert_eq!(pickups[0].age(), 0.0);
    assert!(inventory.selected_stack().is_none());
}

#[test]
fn pickups_cannot_be_collected_through_a_wall_even_inside_collection_radius() {
    let mut game = arena();
    game.player.position = Vec3::new(7.99, 1.0, 8.5);
    for y in 1..=3 {
        game.world.edit_block([8, y, 8], Block::Stone);
    }
    let mut pickup = drop_at(Vec3::new(9.13, 1.2, 8.5), Block::Dirt, 1);
    pickup.age = 1.0;
    let mut pickups = vec![pickup];
    update(
        &mut pickups,
        &game.world,
        &mut game.inventory,
        game.player.position,
        0.1,
    );
    assert_eq!(pickups.len(), 1);
    assert!(game.inventory.selected_stack().is_none());
    for y in 1..=3 {
        game.world.edit_block([8, y, 8], Block::Air);
    }
    update(
        &mut pickups,
        &game.world,
        &mut game.inventory,
        game.player.position,
        0.1,
    );
    assert!(pickups.is_empty());
    assert_eq!(game.inventory.selected_stack().unwrap().count, 1);
}

#[test]
fn collection_visibility_cannot_pass_through_diagonal_wall_corners() {
    let mut game = arena();
    game.player.position = Vec3::new(8.65, 1.0, 8.65);
    for y in 1..=3 {
        game.world.edit_block([9, y, 8], Block::Stone);
        game.world.edit_block([8, y, 9], Block::Stone);
    }
    let mut drop = drop_at(Vec3::new(9.5, 1.5, 9.5), Block::Dirt, 1);
    drop.age = 1.0;
    let mut pickups = vec![drop];
    update(
        &mut pickups,
        &game.world,
        &mut game.inventory,
        game.player.position,
        0.0,
    );
    assert_eq!(pickups.len(), 1);
    assert!(game.inventory.selected_stack().is_none());
    for y in 1..=3 {
        game.world.edit_block([9, y, 8], Block::Air);
        game.world.edit_block([8, y, 9], Block::Air);
    }
    update(
        &mut pickups,
        &game.world,
        &mut game.inventory,
        game.player.position,
        0.0,
    );
    assert!(pickups.is_empty());
    assert_eq!(game.inventory.selected_stack().unwrap().count, 1);
}

#[test]
fn gravity_is_substepped_and_swept_collision_stops_floor_ceiling_and_negative_wall() {
    let world = world(
        &[ChunkCoord::new(-1, -1)],
        &[
            ([-2, 0, -2], Block::Stone),
            ([-2, 4, -2], Block::Stone),
            ([-3, 1, -2], Block::Stone),
        ],
    );
    let mut falling = drop_at(Vec3::new(-1.5, 3.0, -1.5), Block::Dirt, 1);
    falling.velocity.y = -50.0;
    pickup_physics::advance(&mut falling, &world, 100.0);
    assert_eq!(falling.position.y, 1.125);
    assert_eq!(falling.velocity.y, 0.0);
    assert!((falling.age() - 0.25).abs() < 0.000_01);
    let mut rising = drop_at(Vec3::new(-1.5, 3.5, -1.5), Block::Dirt, 1);
    rising.velocity.y = 50.0;
    pickup_physics::advance(&mut rising, &world, 1.0 / 120.0);
    assert_eq!(rising.position.y, 3.875);
    let mut sliding = drop_at(Vec3::new(-1.5, 1.125, -1.5), Block::Dirt, 1);
    sliding.velocity.x = -50.0;
    pickup_physics::advance(&mut sliding, &world, 0.1);
    assert_eq!(sliding.position.x, -1.875);
    assert_eq!(sliding.velocity.x, 0.0);
}

#[test]
fn chunk_eviction_suspends_drops_and_reinsertion_resumes_without_loss() {
    let mut game = arena();
    let mut pickup = drop_at(Vec3::new(8.5, 4.0, 8.5), Block::Stone, 4);
    pickup_physics::advance(&mut pickup, &game.world, 0.1);
    let position = pickup.position;
    let age = pickup.age();
    let velocity = pickup.velocity;
    game.world.remove(ChunkCoord::new(0, 0));
    pickup_physics::advance(&mut pickup, &game.world, 0.25);
    assert_eq!(pickup.position, position);
    assert_eq!(pickup.age(), age);
    assert_eq!(pickup.velocity, velocity);
    game.world.insert(empty_chunk(ChunkCoord::new(0, 0)));
    pickup_physics::advance(&mut pickup, &game.world, 0.1);
    assert!(pickup.position.y < position.y);
    assert!(pickup.age() > age);
    assert_eq!(pickup.count, 4);
    assert_eq!(game.world.block_at(8, 0, 8), Some(Block::Stone));
}

#[test]
fn collision_with_an_unloaded_boundary_suspends_the_substep_transactionally() {
    let world = world(&[ChunkCoord::new(0, 0)], &[]);
    let mut pickup = drop_at(Vec3::new(15.875, 4.0, 8.5), Block::Stone, 1);
    pickup.velocity.x = 20.0;
    let position = pickup.position;
    pickup_physics::advance(&mut pickup, &world, 0.1);
    assert_eq!(pickup.position, position);
    assert_eq!(pickup.age(), 0.0);
    assert_eq!(pickup.velocity.x, 20.0);
}

#[test]
fn liquids_do_not_destroy_drops_and_invalid_dt_cannot_advance_them() {
    let world = world(&[ChunkCoord::new(0, 0)], &[([8, 2, 8], Block::Lava)]);
    let mut pickup = drop_at(Vec3::new(8.5, 2.5, 8.5), Block::Wood, 3);
    for dt in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
        pickup_physics::advance(&mut pickup, &world, dt);
        assert_eq!(pickup.age(), 0.0);
    }
    pickup_physics::advance(&mut pickup, &world, 0.1);
    assert!(pickup.position.y < 2.5);
    assert_eq!(pickup.count, 3);
}
