//! Maximum-speed exploration across real generated water and bounded chunk streaming.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use glam::Vec3;
use rustcraft::game::Game;
use rustcraft::player::{MovementInput, SpeedMultiplier};
use rustcraft::streaming::ChunkStreamer;
use rustcraft::world::{Block, ChunkCoord, Terrain, WaterBiome, WorldView};

#[test]
fn max_speed_flight_reaches_freshwater_and_ocean_from_the_default_surface_spawn() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        for kind in [WaterBiome::Lake, WaterBiome::Ocean] {
            let mut game = Game::new(terrain.seed());
            let site = terrain.water_landmark(kind).unwrap();
            let profile = terrain.water_column(site.water[0], site.water[2]).unwrap();
            let destination_y = profile.surface + 5.0;
            let target = Vec3::new(
                site.water[0] as f32 + 0.5,
                destination_y,
                site.water[2] as f32 + 0.5,
            );
            let delta = target - game.player().position;
            let yaw = delta.x.atan2(-delta.z);
            let look_delta = f64::from(yaw - game.player().yaw) / 0.0025;
            game.look(look_delta, 0.0);
            game.toggle_flight();
            for _ in 0..4 {
                game.cycle_speed();
            }
            assert_eq!(game.speed(), SpeedMultiplier::Sixteen);
            while game.player().position.y < destination_y - 0.7 {
                game.update(
                    MovementInput {
                        jump: true,
                        ..Default::default()
                    },
                    1.0 / 120.0,
                );
            }
            let mut visited = BTreeSet::new();
            let mut arrived = false;
            for _ in 0..1_200 {
                game.update(
                    MovementInput {
                        forward: true,
                        ..Default::default()
                    },
                    1.0 / 120.0,
                );
                let pose = game.player().position;
                visited.insert(ChunkCoord::from_world(pose.x, pose.z));
                if (pose - target).with_y(0.0).length() < 0.8 {
                    arrived = true;
                    break;
                }
            }
            assert!(
                arrived,
                "seed {}, {} flight did not arrive",
                terrain.seed(),
                kind.name()
            );
            assert!(visited.len() >= 10);
            assert_eq!(
                terrain.block_at(site.water[0], site.water[1], site.water[2]),
                Block::Water
            );
            assert_eq!(game.environment().biome, kind.name());
            game.respawn();
            assert_eq!(game.speed(), SpeedMultiplier::Sixteen);
            assert!(!game.player().flying);
        }
    }
}

#[test]
fn rapid_flight_reprioritizes_bounded_streaming_and_keeps_cpu_residency_in_sync() {
    let mut game = Game::new(42);
    game.toggle_flight();
    for _ in 0..4 {
        game.cycle_speed();
    }
    let spawn = game.player().position;
    let mut streamer =
        ChunkStreamer::new(game.terrain(), ChunkCoord::from_world(spawn.x, spawn.z), 1).unwrap();
    for _ in 0..300 {
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            1.0 / 120.0,
        );
        let position = game.player().position;
        let center = ChunkCoord::from_world(position.x, position.z);
        for coord in streamer.update_center(center) {
            game.world_mut().remove(coord);
        }
        for prepared in streamer.poll_ready(4) {
            game.world_mut().insert(prepared.chunk);
        }
        assert_eq!(game.world().len(), streamer.loaded_count());
        assert!(game.world().len() <= 9);
        assert!(!streamer.worker_failed());
    }
    let destination = game.player().position;
    assert!((destination - spawn).length() > 190.0);
    let center = ChunkCoord::from_world(destination.x, destination.z);
    let deadline = Instant::now() + Duration::from_secs(10);
    while streamer.pending_count() > 0 {
        for coord in streamer.update_center(center) {
            game.world_mut().remove(coord);
        }
        for prepared in streamer.poll_ready(4) {
            game.world_mut().insert(prepared.chunk);
        }
        assert!(!streamer.worker_failed());
        assert!(
            Instant::now() < deadline,
            "maximum-speed streaming failed to catch up"
        );
        std::thread::yield_now();
    }
    assert_eq!(game.world().len(), 9);
    let [x, y, z] = destination.floor().as_ivec3().to_array();
    assert_eq!(
        game.world().block_at(x, y, z),
        Some(game.terrain().block_at(x, y, z))
    );
}
