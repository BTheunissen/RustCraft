use crate::game::{Game, GameMode};
use crate::lighting::sky_exposure;
use crate::player::EYE_HEIGHT;
use crate::world::{Block, Chunk, ChunkCoord, Terrain, WorldView};
use glam::Vec3;

fn arch_game(seed: u64) -> (Game, crate::world::CoastalArch) {
    let mut game = Game::with_mode(seed, GameMode::Survival);
    let arch = game.terrain.coastal_landmark().unwrap().arch;
    game.player.position = Vec3::from_array(arch.opening) - Vec3::Y * EYE_HEIGHT;
    let center = ChunkCoord::from_world(arch.opening[0], arch.opening[2]);
    for dz in -1..=1 {
        for dx in -1..=1 {
            game.world.insert(Chunk::generate(
                &game.terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    (game, arch)
}

#[test]
fn real_coastal_overhang_shades_vertically_but_preserves_open_outdoor_atmosphere() {
    for seed in [Terrain::default().seed(), 42, 0, 1, 7, 2026] {
        let (mut game, arch) = arch_game(seed);
        let eye = game.player.eye_position();
        assert_eq!(
            sky_exposure(eye, |x, y, z| game
                .world
                .block_at(x, y, z)
                .unwrap_or(Block::Air)),
            0.0
        );
        let environment = game.environment();
        assert!(!environment.underground && !environment.underwater);
        assert_eq!(environment.sky_exposure, 1.0);
        assert_ne!(
            environment.biome,
            game.terrain
                .cave_biome(
                    arch.opening[0] as i32,
                    arch.opening[1] as i32,
                    arch.opening[2] as i32
                )
                .name()
        );
        assert_eq!(game.environment().sky_exposure, 1.0);
    }
}

#[test]
fn player_enclosure_edits_invalidate_stationary_overhang_then_opening_restores_daylight() {
    let (mut game, arch) = arch_game(Terrain::default().seed());
    let [x, y, z] = arch.opening.map(|value| value.floor() as i32);
    assert_eq!(game.environment().sky_exposure, 1.0);
    for distance in 1..=8 {
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            game.world
                .edit_block([x + dx * distance, y, z + dz * distance], Block::Stone);
        }
    }
    let enclosed = game.environment();
    assert!(enclosed.underground);
    assert_eq!(enclosed.sky_exposure, 0.0);
    for distance in 1..=8 {
        game.world.edit_block([x + distance, y, z], Block::Air);
    }
    assert_eq!(game.environment().sky_exposure, 1.0);
    assert!(!game.environment().underground);
    let center = ChunkCoord::from_world(arch.opening[0], arch.opening[2]);
    game.world.remove(center);
    assert_eq!(game.environment().sky_exposure, 1.0);
}

#[test]
fn excavating_arch_roof_restores_vertical_sky_and_underwater_keeps_marine_metadata() {
    let (mut game, arch) = arch_game(42);
    let [x, _, z] = arch.opening.map(|value| value.floor() as i32);
    for y in arch.roof[0]..=arch.roof[1] {
        assert!(game.world.edit_block([x, y, z], Block::Air));
    }
    assert_eq!(
        sky_exposure(game.player.eye_position(), |x, y, z| game
            .world
            .block_at(x, y, z)
            .unwrap_or(Block::Air)),
        1.0
    );
    assert_eq!(game.environment().sky_exposure, 1.0);
    game.player.position.y = crate::world::SEA_LEVEL as f32 - EYE_HEIGHT - 0.5;
    let environment = game.environment();
    assert!(environment.underwater && !environment.underground);
    assert_eq!(
        environment.water.unwrap().biome,
        crate::world::WaterBiome::Ocean
    );
    assert_eq!(environment.water.unwrap().salinity, 1.0);
    game.player.position.y += 2.0;
    assert!(!game.environment().underwater);
}

#[test]
fn an_added_player_roof_under_arch_takes_precedence_over_outdoor_sky_spill() {
    let (mut game, arch) = arch_game(42);
    let [x, y, z] = arch.opening.map(|value| value.floor() as i32);
    assert_eq!(game.environment().sky_exposure, 1.0);
    assert!(game.world.edit_block([x, y + 2, z], Block::Wood));
    assert!(game.environment().underground);
    assert_eq!(game.environment().sky_exposure, 0.0);
    assert!(game.world.edit_block([x, y + 2, z], Block::Air));
    assert_eq!(game.environment().sky_exposure, 1.0);
}

#[test]
fn missing_coastal_residency_is_not_generated_on_environment_sampling_and_loading_invalidates_cache()
 {
    let terrain = Terrain::default();
    let arch = terrain.coastal_landmark().unwrap().arch;
    let mut game = Game::with_mode(terrain.seed(), GameMode::Survival);
    game.player.position = Vec3::from_array(arch.opening) - Vec3::Y * EYE_HEIGHT;
    assert_eq!(
        game.coastal_sky(arch.opening.map(|value| value.floor() as i32)),
        None
    );
    let center = ChunkCoord::from_world(arch.opening[0], arch.opening[2]);
    for dz in -1..=1 {
        for dx in -1..=1 {
            game.world.insert(Chunk::generate(
                &terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    assert_eq!(
        game.coastal_sky(arch.opening.map(|value| value.floor() as i32)),
        Some(1.0)
    );
    game.world.remove(center);
    assert_eq!(
        game.coastal_sky(arch.opening.map(|value| value.floor() as i32)),
        None
    );
}

#[test]
fn coastal_arch_supports_remain_ordinary_survival_mining_and_placement() {
    let (mut game, arch) = arch_game(Terrain::default().seed());
    let mut found = None;
    'pose: for [sx, sy, sz] in arch.supports {
        for dz in -6..=6 {
            for dx in -6..=6 {
                let cell = [sx + dx, sy, sz + dz];
                if !game
                    .world
                    .block_at(cell[0], cell[1], cell[2])
                    .is_some_and(|block| matches!(block, Block::Limestone | Block::Calcite))
                {
                    continue;
                }
                for normal in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                    let target =
                        Vec3::from_array(cell.map(|value| value as f32)) + Vec3::splat(0.5);
                    let eye = target + normal * 2.5;
                    let feet = eye - Vec3::Y * EYE_HEIGHT;
                    let bounds = crate::game::bounds::Bounds::player(feet).cells().unwrap();
                    let clear = (bounds.0[0]..=bounds.1[0]).all(|x| {
                        (bounds.0[1]..=bounds.1[1]).all(|y| {
                            (bounds.0[2]..=bounds.1[2]).all(|z| {
                                game.world
                                    .block_at(x, y, z)
                                    .is_some_and(|block| !block.is_solid())
                            })
                        })
                    });
                    if clear
                        && crate::game::raycast(&game.world, eye, -normal)
                            .is_some_and(|hit| hit.position == cell && hit.normal != [0; 3])
                    {
                        found = Some((feet, target));
                        break 'pose;
                    }
                }
            }
        }
    }
    let (position, target) = found.expect("clear reachable support face");
    game.player.respawn(position);
    crate::game::test_support::aim(&mut game, target);
    game.interact(false, false, 0.0);
    let mined = game.target().unwrap();
    assert!(matches!(mined.block, Block::Limestone | Block::Calcite));
    for _ in 0..10 {
        game.interact(true, false, 0.25);
    }
    assert_eq!(
        game.world
            .block_at(mined.position[0], mined.position[1], mined.position[2]),
        Some(Block::Air)
    );
    assert_eq!(game.pickups().len(), 1);
    assert_eq!(game.pickups()[0].item, mined.block.into());
    let center_point =
        Vec3::from_array(mined.position.map(|value| value as f32)) + Vec3::splat(0.5);
    let normal = Vec3::from_array(mined.normal.map(|value| value as f32));
    game.player
        .respawn(center_point + normal * 0.85 - Vec3::Y * 0.5);
    for _ in 0..8 {
        game.interact(false, false, 0.1);
    }
    assert!(game.pickups().is_empty());
    assert_eq!(game.inventory().selected_stack().unwrap().count, 1);
    game.player
        .respawn(center_point + normal * 2.5 - Vec3::Y * EYE_HEIGHT);
    crate::game::test_support::aim(&mut game, center_point - normal);
    game.interact(false, false, 0.0);
    assert!(
        game.place_selected(),
        "mined {mined:?}; pose {:?}; target {:?}; inventory {:?}",
        game.player.position,
        game.target(),
        game.inventory().selected_stack()
    );
    assert_eq!(game.inventory().selected_stack(), None);
    assert_eq!(
        game.world
            .block_at(mined.position[0], mined.position[1], mined.position[2]),
        Some(mined.block)
    );
    let center = ChunkCoord::from_world(position.x, position.z);
    let snapshot = game.snapshot();
    game.world.remove(center);
    game.world.insert(Chunk::generate(&game.terrain, center));
    assert_eq!(game.snapshot(), snapshot);
    assert_eq!(
        game.world
            .block_at(mined.position[0], mined.position[1], mined.position[2]),
        Some(mined.block)
    );
}
