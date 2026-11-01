use super::SurvivalGeometry;
use crate::game::{BlockTarget, Game, GameMode, Pickup};
use crate::player::Player;
use crate::renderer::survival::geometry::MAX_PICKUPS;
use crate::renderer::{FrameState, SurvivalFrame};
use crate::world::{Block, Chunk, ChunkCoord, ResidentWorld, WORLD_HEIGHT};
use glam::Vec3;

fn scene() -> (Game, Pickup) {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let chunk = Chunk::generate(&game.terrain(), ChunkCoord::new(0, 0));
    game.world_mut().insert(chunk);
    for x in 7..10 {
        for z in 5..10 {
            for y in WORLD_HEIGHT - 7..WORLD_HEIGHT {
                game.world_mut().edit_block([x, y, z], Block::Air);
            }
        }
    }
    game.player
        .respawn(Vec3::new(8.5, (WORLD_HEIGHT - 6) as f32, 8.5));
    game.player.yaw = 0.0;
    game.player.pitch = 0.0;
    game.world_mut()
        .edit_block([8, WORLD_HEIGHT - 5, 6], Block::Stone);
    for _ in 0..10 {
        game.interact(true, false, 0.25);
    }
    assert_eq!(game.pickups().len(), 1);
    let pickup = game.pickups()[0].clone();
    (game, pickup)
}

fn frame<'a>(
    world: &'a ResidentWorld,
    player: &'a Player,
    survival: Option<SurvivalFrame<'a>>,
) -> FrameState<'a> {
    FrameState {
        world,
        player,
        hud: &[],
        survival,
        elapsed: 1.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: None,
        render_distance: 2,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        headlamp: true,
        in_lava: false,
    }
}

fn survival(pickups: &[Pickup]) -> SurvivalFrame<'_> {
    SurvivalFrame {
        pickups,
        held: Some(Block::Grass.into()),
        target: None,
        mining_progress: 0.0,
        show_hand: true,
    }
}

#[test]
fn staging_filters_invalid_drops_and_preserves_the_first_person_visibility_contract() {
    let (game, pickup) = scene();
    let mut pickups = vec![pickup.clone(); 6];
    pickups[1].count = 0;
    pickups[2].item = Block::Air.into();
    pickups[3].item = Block::Water.into();
    pickups[4].position.x = f32::NAN;
    pickups[5].position.x = -0.1;
    let mut geometry = SurvivalGeometry::default();
    geometry.stage(
        &frame(game.world(), game.player(), Some(survival(&pickups))),
        1.6,
    );
    assert_eq!(geometry.pickups.len(), 1);
    assert_eq!(geometry.hand_count, 3);
    assert!(!geometry.has_target);

    geometry.stage(
        &frame(
            game.world(),
            game.player(),
            Some(SurvivalFrame {
                show_hand: false,
                ..survival(&pickups)
            }),
        ),
        1.6,
    );
    assert_eq!(geometry.pickups.len(), 1);
    assert_eq!(geometry.hand_count, 0);

    geometry.stage(
        &frame(
            game.world(),
            game.player(),
            Some(SurvivalFrame {
                held: None,
                ..survival(&pickups)
            }),
        ),
        1.6,
    );
    assert_eq!(geometry.hand_count, 2);
    geometry.stage(&frame(game.world(), game.player(), None), 1.6);
    assert!(geometry.pickups.is_empty());
    assert!(geometry.lights.is_empty());
    assert_eq!(geometry.hand_count, 0);
    assert!(!geometry.has_target);
}

#[test]
fn pickup_staging_is_capped_and_reuses_its_cpu_allocations_and_cached_daylight() {
    let (game, pickup) = scene();
    let pickups = vec![pickup; MAX_PICKUPS + 17];
    let mut geometry = SurvivalGeometry::default();
    let staged = frame(game.world(), game.player(), Some(survival(&pickups)));
    geometry.stage(&staged, 1.6);
    assert_eq!(geometry.pickups.len(), MAX_PICKUPS);
    assert_eq!(geometry.lights.len(), MAX_PICKUPS);
    assert_eq!(geometry.pickups.capacity(), MAX_PICKUPS);
    assert_eq!(geometry.lights.capacity(), MAX_PICKUPS);
    let pickup_ptr = geometry.pickups.as_ptr();
    let light_ptr = geometry.lights.as_ptr();
    let previous = geometry.pickups[0];
    geometry.stage(
        &FrameState {
            elapsed: 3.2,
            ..staged
        },
        1.6,
    );
    assert_eq!(geometry.pickups.as_ptr(), pickup_ptr);
    assert_eq!(geometry.lights.as_ptr(), light_ptr);
    assert_ne!(geometry.pickups[0].model, previous.model);
    assert_eq!(geometry.pickups[0].light, previous.light);
}

#[test]
fn pickup_daylight_updates_on_voxel_revision_movement_and_residency_changes() {
    let (mut game, mut pickup) = scene();
    let mut geometry = SurvivalGeometry::default();
    let mut stage = |game: &Game, pickup: &Pickup| {
        geometry.stage(
            &frame(
                game.world(),
                game.player(),
                Some(survival(std::slice::from_ref(pickup))),
            ),
            1.6,
        );
        (
            geometry.pickups.first().map(|instance| instance.light),
            geometry.lights.first().map(|light| light.revision),
        )
    };
    let (initial_light, initial_revision) = stage(&game, &pickup);
    assert_eq!(initial_light, Some(1.0));
    assert_eq!(stage(&game, &pickup), (initial_light, initial_revision));
    assert!(
        game.world_mut()
            .edit_block([8, WORLD_HEIGHT - 2, 6], Block::Stone)
    );
    let (roofed_light, roofed_revision) = stage(&game, &pickup);
    assert_eq!(roofed_light, Some(0.0));
    assert_ne!(initial_revision, roofed_revision);
    pickup.position.x += 1.0;
    assert_eq!(stage(&game, &pickup).0, Some(1.0));
    game.world_mut().remove(ChunkCoord::new(0, 0));
    assert_eq!(stage(&game, &pickup), (None, None));
    let chunk = Chunk::generate(&game.terrain(), ChunkCoord::new(0, 0));
    game.world_mut().insert(chunk);
    let (restored_light, restored_revision) = stage(&game, &pickup);
    assert_eq!(restored_light, Some(1.0));
    assert_ne!(restored_revision, roofed_revision);
}

#[test]
fn mining_updates_only_the_uniform_and_rejects_targets_after_an_edit_or_eviction() {
    let (mut game, pickup) = scene();
    let target = BlockTarget {
        position: [8, WORLD_HEIGHT - 5, 6],
        normal: [0, 0, 1],
        block: Block::Stone,
        distance: 2.0,
    };
    game.world_mut().edit_block(target.position, target.block);
    let pickups = [pickup];
    let mut geometry = SurvivalGeometry::default();
    let presentation = SurvivalFrame {
        target: Some(target),
        show_hand: false,
        ..survival(&pickups)
    };
    let idle = geometry.stage(&frame(game.world(), game.player(), Some(presentation)), 1.6);
    assert!(geometry.has_target);
    let progress = geometry.stage(
        &frame(
            game.world(),
            game.player(),
            Some(SurvivalFrame {
                mining_progress: 0.75,
                ..presentation
            }),
        ),
        1.6,
    );
    assert!(geometry.has_target);
    assert_eq!(idle.target_model, progress.target_model);
    assert_eq!(idle.hand_projection, progress.hand_projection);
    assert_ne!(idle.mining, progress.mining);
    assert_eq!(geometry.hand_count, 0);

    game.world_mut().edit_block(target.position, Block::Dirt);
    let stale = geometry.stage(&frame(game.world(), game.player(), Some(presentation)), 1.6);
    assert!(!geometry.has_target);
    assert_eq!(stale.mining[0], 0.0);
    game.world_mut().remove(ChunkCoord::new(0, 0));
    geometry.stage(&frame(game.world(), game.player(), Some(presentation)), 1.6);
    assert!(!geometry.has_target);
    assert!(geometry.pickups.is_empty());
}
