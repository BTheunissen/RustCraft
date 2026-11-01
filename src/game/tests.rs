use super::*;

#[test]
fn solar_time_preserves_environment_and_survives_respawn_settings_and_input_clear() {
    let mut game = Game::new(42);
    let surface = game.environment();
    game.time_mut().command("/time set night").unwrap();
    game.time_mut().command("/time speed 60").unwrap();
    let night = game.environment();
    assert_eq!(surface.biome, night.biome);
    assert_eq!(surface.sky_exposure, night.sky_exposure);
    assert_eq!(surface.underground, night.underground);
    assert_eq!(surface.water, night.water);
    let time = game.time();
    game.clear_input();
    game.respawn();
    game.toggle_headlamp();
    game.cycle_speed();
    assert_eq!(game.time(), time);
    let snapshot = game.snapshot();
    assert_eq!(snapshot.time_of_day, 22.0);
    assert_eq!(Game::from_snapshot(snapshot).unwrap().time().speed(), 1.0);
}

#[test]
fn speed_cycles_and_survives_pause_respawn_and_movement_mode_changes() {
    let mut game = Game::new(Terrain::default().seed());
    for speed in [
        SpeedMultiplier::Double,
        SpeedMultiplier::Quadruple,
        SpeedMultiplier::Octuple,
        SpeedMultiplier::Sixteen,
        SpeedMultiplier::Normal,
    ] {
        game.cycle_speed();
        assert_eq!(game.speed(), speed);
        game.clear_input();
        assert_eq!(game.speed(), speed);
        game.toggle_flight();
        assert!(game.player.flying);
        assert_eq!(game.speed(), speed);
        game.respawn();
        assert!(!game.player.flying);
        assert_eq!(game.speed(), speed);
    }
}

#[test]
fn selected_speed_changes_displacement_without_scaling_the_simulation_clock() {
    let mut game = Game::new(42);
    game.player.flying = true;
    game.player.position = Vec3::new(0.0, 100.0, 0.0);
    game.player.yaw = 0.0;
    for expected in [1.0, 2.0, 4.0, 8.0, 16.0] {
        let start = game.player.position;
        game.update(
            MovementInput {
                forward: true,
                ..Default::default()
            },
            0.1,
        );
        assert!((game.player.position.z - start.z + 0.5 * expected).abs() < 0.0001);
        assert_eq!(game.player.position.y, 100.0);
        game.cycle_speed();
    }
}

#[test]
fn fall_recovery_and_headlamp_do_not_reset_exploration_settings() {
    let mut game = Game::new(42);
    game.cycle_speed();
    game.toggle_headlamp();
    game.player.position.y = -85.0;
    game.update(MovementInput::default(), 1.0 / 120.0);
    assert_eq!(
        game.player.position,
        Vec3::from_array(game.terrain.spawn_position())
    );
    assert_eq!(game.speed(), SpeedMultiplier::Double);
    assert!(!game.headlamp);
    for y in [-30.0, -64.0, -84.0] {
        assert!(!below_world(y));
    }
}

#[test]
fn underwater_camera_tracks_fractional_water_occupancy_not_whole_voxels() {
    use super::environment::camera_is_underwater;
    use crate::world::{WaterBiome, WaterSample};
    let water = WaterSample {
        bottom: -4,
        surface: 2.25,
        flow: [0.0; 3],
        biome: WaterBiome::Lake,
        salinity: 0.0,
    };
    assert!(camera_is_underwater(Some(water), 2.24));
    for y in [2.25, 2.5, -3.1, f32::NAN] {
        assert!(!camera_is_underwater(Some(water), y));
    }
    assert!(!camera_is_underwater(None, 2.0));
    assert!(!camera_is_underwater(
        Some(WaterSample {
            surface: f32::NAN,
            ..water
        }),
        2.0
    ));
}

#[test]
fn frame_environment_distinguishes_surface_caves_actual_water_and_surfacing() {
    use crate::player::EYE_HEIGHT;
    use crate::world::{CaveBiome, Chunk, ChunkCoord, WaterBiome};
    let mut game = Game::new(Terrain::default().seed());
    let surface = game.environment();
    assert!(!surface.underground && !surface.underwater && !surface.in_lava);
    assert_eq!(surface.sky_exposure, 1.0);
    assert!(surface.water.is_none());
    let cave = Vec3::from_array(game.terrain.cave_position(CaveBiome::Rocky).unwrap());
    game.world.insert(Chunk::generate(
        &game.terrain,
        ChunkCoord::from_world(cave.x, cave.z),
    ));
    game.player.position = cave;
    let environment = game.environment();
    assert!(environment.underground && !environment.underwater);
    assert!(environment.sky_exposure < 0.05);
    assert_eq!(environment.biome, CaveBiome::Rocky.name());
    for kind in [WaterBiome::Lake, WaterBiome::Ocean] {
        let site = game.terrain.water_landmark(kind).unwrap();
        let water = game
            .terrain
            .water_column(site.water[0], site.water[2])
            .unwrap();
        let x = site.water[0] as f32 + 0.5;
        let z = site.water[2] as f32 + 0.5;
        game.world.clear();
        game.world
            .insert(Chunk::generate(&game.terrain, ChunkCoord::from_world(x, z)));
        game.player.position = Vec3::new(x, water.surface - EYE_HEIGHT - 0.5, z);
        let environment = game.environment();
        assert!(environment.underwater && !environment.underground);
        assert_eq!(environment.biome, kind.name());
        assert_eq!(environment.water, Some(water));
        game.player.position.y += 2.0;
        let surfaced = game.environment();
        assert!(!surfaced.underwater && surfaced.water.is_none());
        assert_eq!(surfaced.sky_exposure, 1.0);
        game.world.clear();
        assert_eq!(game.environment().biome, kind.name());
    }
}
