use super::*;
use std::time::Duration;

#[test]
fn coastal_fixture_mines_one_real_block_and_isolates_saved_cursor_and_pickup_progress() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let site = terrain.coastal_landmark().unwrap();
        let mut game = Game::with_mode(seed, GameMode::Survival);
        game.player.respawn(Vec3::from_array(site.arch.view));
        let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
        for dz in -1..=1 {
            for dx in -1..=1 {
                game.world.insert(Chunk::generate(
                    &terrain,
                    ChunkCoord::new(center.x + dx, center.z + dz),
                ));
            }
        }
        let cell = source(&game).unwrap();
        aim_at(
            &mut game.player,
            Vec3::from_array(cell.map(|value| value as f32)) + Vec3::splat(0.5),
        );
        let material = game.world.block_at(cell[0], cell[1], cell[2]).unwrap();
        game.inventory_mut().insert(Block::Wood, 3);
        mine(&mut game, cell, Instant::now() + Duration::from_secs(10)).unwrap();
        assert_eq!(game.pickups()[0].item, material.into());
        game.inventory_mut().click_slot(0);
        let scratch = super::super::super::persistence::ScratchDirectory::new().unwrap();
        let slot = SaveSlot::new(scratch.0.join("world.save"));
        slot.write(&game.snapshot()).unwrap();
        let restored = Game::from_snapshot(slot.load().unwrap().unwrap()).unwrap();
        assert_eq!(restored.snapshot(), game.snapshot());
        assert!(restored.inventory().cursor_stack().is_none());
        assert_eq!(restored.inventory().slots()[0].unwrap().count, 3);
        assert!(restored.world.is_empty());
        assert_eq!(
            SampledWorld::new(&restored.world, &terrain).block_at(cell[0], cell[1], cell[2]),
            Some(Block::Air)
        );
        let environment = game.environment();
        assert!(!frame(&game, environment).headlamp);
    }
}

#[test]
fn coastal_fixture_and_readback_fail_closed_for_missing_residency_deadline_and_unchanged_pixels() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    assert!(source(&game).is_err());
    assert!(mine(&mut game, [0, 0, 0], Instant::now()).is_err());
    let terrain = Terrain::default();
    let site = terrain.coastal_landmark().unwrap();
    game = Game::with_mode(terrain.seed(), GameMode::Survival);
    game.player.respawn(Vec3::from_array(site.arch.view));
    let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
    for dz in -1..=1 {
        for dx in -1..=1 {
            game.world.insert(Chunk::generate(
                &terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    let cell = source(&game).unwrap();
    aim_at(
        &mut game.player,
        Vec3::from_array(cell.map(|value| value as f32)) + Vec3::splat(0.5),
    );
    let before = game.snapshot();
    assert!(mine(&mut game, cell, Instant::now() - Duration::from_secs(1)).is_err());
    assert_eq!(game.snapshot(), before);
    let visible = FrameDifference {
        changed_fraction: 0.1,
        mean_absolute_error: 0.01,
    };
    assert!(visible_edit(visible).is_ok());
    for difference in [
        FrameDifference {
            changed_fraction: 0.0,
            ..visible
        },
        FrameDifference {
            mean_absolute_error: 0.0,
            ..visible
        },
        FrameDifference {
            changed_fraction: f32::NAN,
            ..visible
        },
        FrameDifference {
            mean_absolute_error: f32::NAN,
            ..visible
        },
    ] {
        assert!(visible_edit(difference).is_err());
    }
}
