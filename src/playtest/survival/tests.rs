use super::{Game, GameMode, Vec3, block_center, mining_site, visible_difference};
use crate::renderer::FrameDifference;
use crate::world::{Chunk, ChunkCoord, WorldView};

#[test]
fn generated_mining_sites_have_reachable_exposed_supported_blocks() {
    for seed in [42, crate::world::Terrain::default().seed()] {
        let mut game = Game::with_mode(seed, GameMode::Survival);
        let feet = game.player.position;
        let center = ChunkCoord::from_world(feet.x, feet.z);
        for z in -1..=1 {
            for x in -1..=1 {
                game.world.insert(Chunk::generate(
                    &game.terrain,
                    ChunkCoord::new(center.x + x, center.z + z),
                ));
            }
        }
        let source = mining_site(&game).unwrap();
        super::aim_at(&mut game.player, block_center(source));
        game.interact(false, false, 0.0);
        assert_eq!(game.target().unwrap().position, source);
        assert!(
            game.world
                .block_at(source[0], source[1] - 1, source[2])
                .unwrap()
                .is_solid()
        );
        assert!(game.player.eye_position().distance(block_center(source)) < 4.5);
    }
}

#[test]
fn missing_residency_cannot_become_a_mining_fixture() {
    let game = Game::with_mode(42, GameMode::Survival);
    assert!(mining_site(&game).is_none());
    assert_eq!(block_center([-1, -64, 0]), Vec3::new(-0.5, -63.5, 0.5));
}

#[test]
fn gpu_acceptance_rejects_unchanged_or_nonfinite_readbacks() {
    for difference in [
        FrameDifference {
            changed_fraction: 0.0,
            mean_absolute_error: 0.0,
        },
        FrameDifference {
            changed_fraction: 0.5,
            mean_absolute_error: 0.0,
        },
        FrameDifference {
            changed_fraction: f32::NAN,
            mean_absolute_error: 0.5,
        },
        FrameDifference {
            changed_fraction: 0.5,
            mean_absolute_error: f32::NAN,
        },
    ] {
        assert!(visible_difference(difference, "fixture").is_err());
    }
    assert!(
        visible_difference(
            FrameDifference {
                changed_fraction: 0.01,
                mean_absolute_error: 0.005
            },
            "fixture"
        )
        .is_ok()
    );
}
