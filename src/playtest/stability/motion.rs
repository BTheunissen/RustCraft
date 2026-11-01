//! Canopy, cave-mouth, and coast views isolate subpixel air motion with animation frozen.

use crate::player::Player;
use crate::playtest::{aim_at, check_deadline, load_scene};
use crate::renderer::{FrameState, Renderer};
use crate::world::{ChunkCoord, ResidentWorld, Terrain};
use glam::Vec3;
use std::error::Error;
use std::io;
use std::time::Instant;

pub(super) fn verify(
    renderer: &mut Renderer,
    terrain: Terrain,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let coast = terrain
        .coastal_landmark()
        .ok_or_else(|| io::Error::other("no stability coast"))?;
    let entrance = Vec3::from_array(
        terrain
            .entrance_position()
            .ok_or_else(|| io::Error::other("no stability entrance"))?,
    );
    let forest = crate::playtest::volumetrics::forest_view(terrain)
        .ok_or_else(|| io::Error::other("no stability canopy"))?;
    for (label, position, target) in [
        ("canopy", forest.0, forest.1),
        (
            "entrance",
            entrance + Vec3::Y * 1.5,
            entrance + Vec3::new(0.0, -3.0, -4.0),
        ),
        (
            "coast",
            Vec3::from_array(coast.arch.view),
            Vec3::from_array(coast.arch.opening) + Vec3::Y * 3.0,
        ),
    ] {
        check_deadline(deadline)?;
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(position.x, position.z),
            4,
            deadline,
        )?;
        let mut player = Player::new(position);
        aim_at(&mut player, target);
        if label == "entrance" {
            player.yaw = crate::playtest::caves::entrance_yaw(terrain, player.eye_position());
            player.pitch = -0.45;
        }
        let first = frame(&world, &player);
        let (stationary, _) = renderer.compare_interactive_frame(&first)?;
        let next = renderer.read_interactive_frame(&first)?;
        let static_error =
            super::walking::difference(&stationary, &next, renderer.size().0, renderer.size().1);
        let mut worst_air = 0.0_f32;
        for offset in [0.001, -0.001, 0.004] {
            check_deadline(deadline)?;
            let mut shifted = player.clone();
            shifted.position += player.look_direction().with_y(0.0).normalize_or_zero() * offset;
            let error = renderer.measure_air_motion(&first, &frame(&world, &shifted))?;
            worst_air = worst_air.max(error);
        }
        super::require_motion_error(static_error, 0.00005, &format!("{label} stationary"))?;
        super::require_motion_error(worst_air, 0.0004, &format!("{label} air motion"))?;
        println!(
            "Image stability GPU: {label} frozen animation, stationary error {static_error:.6}, ±0.001/0.004-block air error {worst_air:.6}"
        );
    }
    Ok(())
}

fn frame<'a>(world: &'a ResidentWorld, player: &'a Player) -> FrameState<'a> {
    FrameState {
        world,
        player,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: Some(2.2),
        render_distance: 4,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        headlamp: false,
        in_lava: false,
    }
}
