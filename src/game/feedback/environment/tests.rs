use super::*;
use crate::game::{Game, GameMode};
use crate::world::{Block, Chunk, ChunkCoord, Terrain};

fn environment() -> Environment {
    Environment {
        biome: "Ocean",
        underground: false,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        in_lava: false,
    }
}

fn water(biome: WaterBiome) -> WaterSample {
    WaterSample {
        bottom: 0,
        surface: 8.0,
        flow: [0.0; 3],
        biome,
        salinity: 0.7,
    }
}

#[test]
fn source_sampling_is_bounded_negative_safe_and_does_not_infer_water_from_a_label() {
    let position = Vec3::new(-0.5, 30.0, -16.5);
    let mut reads = 0;
    let dry = nearby_water(position, |_, _| {
        reads += 1;
        None
    });
    assert_eq!(reads, 289);
    assert_eq!(dry, [0.0; 2]);
    assert_eq!(profile(environment(), dry).surf, 0.0);
    for position in [Vec3::NAN, Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)] {
        let mut reads = 0;
        assert_eq!(
            nearby_water(position, |_, _| {
                reads += 1;
                None
            }),
            [0.0; 2]
        );
        assert!(reads <= 289);
    }
}

#[test]
fn marine_freshwater_and_estuary_mix_follow_real_metadata_not_names() {
    let position = Vec3::new(-0.5, 9.0, -0.5);
    for (biome, marine, fresh) in [
        (WaterBiome::Ocean, true, false),
        (WaterBiome::Lake, false, true),
        (WaterBiome::River, false, true),
        (WaterBiome::Waterfall, false, true),
        (WaterBiome::CavePool, false, false),
        (WaterBiome::Estuary, true, true),
    ] {
        let gains = nearby_water(position, |x, z| {
            (x == -1 && z == -1).then_some(water(biome))
        });
        assert_eq!(gains[0] > 0.0, marine, "{biome:?}");
        assert_eq!(gains[1] > 0.0, fresh, "{biome:?}");
    }
    let dry_camera = environment();
    let surf = profile(
        dry_camera,
        nearby_water(position, |_, _| Some(water(WaterBiome::Ocean))),
    );
    assert!(surf.surf > 0.9 && !surf.underwater);
}

#[test]
fn roof_cave_and_camera_underwater_state_are_independent_of_headlamp_and_labels() {
    let open = profile(environment(), [1.0; 2]);
    let enclosed = profile(
        Environment {
            underground: true,
            sky_exposure: 0.0,
            ..environment()
        },
        [1.0; 2],
    );
    assert!(open.wind > 0.0 && open.surf > 0.0 && open.cave == 0.0);
    assert_eq!(
        (enclosed.wind, enclosed.surf, enclosed.freshwater),
        (0.0, 0.0, 0.0)
    );
    assert!(enclosed.cave > 0.0);
    assert!(
        profile(
            Environment {
                underwater: true,
                ..environment()
            },
            [1.0; 2]
        )
        .underwater
    );
    assert!(
        !profile(
            Environment {
                biome: "Underwater Ocean",
                ..environment()
            },
            [1.0; 2]
        )
        .underwater
    );
    for sky_exposure in [f32::NAN, f32::INFINITY, -10.0, 10.0] {
        assert!(
            profile(
                Environment {
                    sky_exposure,
                    ..environment()
                },
                [1.0; 2]
            )
            .gains()
            .into_iter()
            .all(|gain| gain.is_finite() && (0.0..=1.0).contains(&gain))
        );
    }
}

#[test]
fn real_coast_has_surf_from_dry_land_and_eviction_removes_it() {
    for seed in [Terrain::default().seed(), 42] {
        let mut game = Game::with_mode(seed, GameMode::Survival);
        let site = game.terrain.coastal_landmark().unwrap();
        game.player.respawn(Vec3::from_array(site.cove));
        let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
        for dz in -3..=3 {
            for dx in -3..=3 {
                game.world.insert(Chunk::generate(
                    &game.terrain,
                    ChunkCoord::new(center.x + dx, center.z + dz),
                ));
            }
        }
        let environment = game.environment();
        assert!(!environment.underwater);
        let audible = game.sound_environment(environment, 0.0);
        assert!(audible.surf > 0.25, "seed {seed}: {audible:?}");
        assert_eq!(audible.freshwater, 0.0);
        game.player.respawn(Vec3::from_array(site.viewpoint));
        let dry_cliff = game.environment();
        let cliff_sound = game.sound_environment(dry_cliff, 0.25);
        assert!(
            !dry_cliff.underwater && cliff_sound.surf > 0.05,
            "seed {seed}: cliff {cliff_sound:?}"
        );
        game.world.clear();
        let silent = game.sound_environment(environment, 0.25);
        assert_eq!((silent.surf, silent.freshwater), (0.0, 0.0));
    }
}

#[test]
fn resident_surface_reads_reject_edited_dry_voxels_and_unknown_chunks() {
    let terrain = Terrain::default();
    let site = terrain.water_landmark(WaterBiome::Lake).unwrap();
    let [x, _, z] = site.water;
    let coord = ChunkCoord::from_world(x as f32, z as f32);
    let mut world = ResidentWorld::default();
    assert_eq!(world.surface_water_at(x, z), None);
    world.insert(Chunk::generate(&terrain, coord));
    let source = world.surface_water_at(x, z).unwrap();
    assert_eq!(source.biome, WaterBiome::Lake);
    let top = source.surface.ceil() as i32 - 1;
    assert!(world.edit_block([x, top, z], Block::Wood));
    assert_eq!(world.surface_water_at(x, z), None);
    world.remove(coord);
    assert_eq!(world.surface_water_at(x, z), None);
}

#[test]
fn actual_freshwater_viewpoint_and_camera_surfacing_use_real_water_state() {
    let mut game = Game::with_mode(Terrain::default().seed(), GameMode::Survival);
    let site = game.terrain.water_landmark(WaterBiome::Lake).unwrap();
    game.player.respawn(Vec3::from_array(site.position));
    let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
    for dz in -3..=3 {
        for dx in -3..=3 {
            game.world.insert(Chunk::generate(
                &game.terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    let view = game.environment();
    let sound = game.sound_environment(view, 0.0);
    assert!(sound.freshwater > 0.1 && sound.surf == 0.0);
    let [x, y, z] = site.water;
    game.player.respawn(Vec3::new(
        x as f32 + 0.5,
        y as f32 - crate::player::EYE_HEIGHT + 0.5,
        z as f32 + 0.5,
    ));
    let wet = game.environment();
    assert!(wet.underwater && game.sound_environment(wet, 0.25).underwater);
    let water = game.world.surface_water_at(x, z).unwrap();
    game.player.respawn(Vec3::new(
        x as f32 + 0.5,
        water.surface + 0.2,
        z as f32 + 0.5,
    ));
    let dry = game.environment();
    assert!(!dry.underwater && !game.sound_environment(dry, 0.25).underwater);
}

#[test]
fn bounded_refresh_has_no_catchup_burst_or_nan_time_and_matches_partitions() {
    let world = ResidentWorld::default();
    let mut fast = Ambience::default();
    let mut slow = Ambience::default();
    for _ in 0..25 {
        fast.sample(&world, Vec3::ZERO, environment(), 0.01);
    }
    slow.sample(&world, Vec3::ZERO, environment(), 0.25);
    assert_eq!(fast.water, slow.water);
    slow.sample(&world, Vec3::ZERO, environment(), 1000.0);
    assert_eq!(slow.elapsed, 0.0);
    for dt in [f32::NAN, f32::INFINITY, -1.0] {
        slow.sample(&world, Vec3::ZERO, environment(), dt);
    }
    assert_eq!(slow.elapsed, 0.0);
    for invalid in [
        WaterSample {
            surface: f32::NAN,
            ..water(WaterBiome::Ocean)
        },
        WaterSample {
            salinity: f32::INFINITY,
            ..water(WaterBiome::Ocean)
        },
        WaterSample {
            surface: -10.0,
            ..water(WaterBiome::Ocean)
        },
    ] {
        assert_eq!(nearby_water(Vec3::ZERO, |_, _| Some(invalid)), [0.0; 2]);
    }
}
