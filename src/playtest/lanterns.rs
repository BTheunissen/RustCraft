//! Same-pose headlamp-off receiver lighting, wall occlusion, and removal readback.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

use glam::Vec3;

use super::{aim_at, capture_scene, check_deadline, load_scene};
use crate::mesh::mesh_chunk_with_edits;
use crate::player::Player;
use crate::renderer::{FrameState, LightingFeatures, Renderer};
use crate::world::{Block, ChunkCoord, ResidentWorld, Terrain};

pub(crate) fn verify(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut world = load_scene(renderer, terrain, ChunkCoord::new(0, 0), 2, deadline)?;
    for z in -6..=6 {
        for x in -10..=10 {
            for y in -40..=-34 {
                let boundary = x == -10 || x == 10 || z == -6 || z == 6 || y == -40 || y == -34;
                world.edit_block([x, y, z], if boundary { Block::Stone } else { Block::Air });
            }
        }
    }
    let source = [-4, -38, 2];
    let mut player = Player::new(Vec3::new(-5.5, -39.0, 3.5));
    aim_at(&mut player, Vec3::new(-9.8, -37.5, 3.5));
    upload(renderer, &world, terrain, deadline)?;
    let features = LightingFeatures {
        emission: false,
        bounce: false,
        bloom: false,
        volumetrics: false,
        ..Default::default()
    };
    renderer.with_lighting_features(features, |renderer| {
        let unlit = renderer.measure_frame(&frame(&world, &player))?;
        capture_scene(renderer, capture, "lantern-unlit-room", &frame(&world, &player))?;
        world.edit_block(source, Block::Lantern);
        upload(renderer, &world, terrain, deadline)?;
        let lit = renderer.measure_frame(&frame(&world, &player))?;
        let color = renderer.measure_scene_color(&frame(&world, &player))?;
        capture_scene(renderer, capture, "lantern-lit-receivers", &frame(&world, &player))?;
        if lit.mean_luminance <= unlit.mean_luminance + 0.02 || lit.luminance_deviation < 0.01 || color[0] <= color[2] * 1.15 {
            return Err(io::Error::other(format!("lantern did not warm textured receiver walls: {unlit:?} → {lit:?}, {color:?}")).into());
        }
        let day = frame(&world, &player);
        let night = FrameState { time_of_day: 22.0, ..day };
        let unchanged = renderer.compare_frames(&day, &night)?;
        if unchanged.mean_absolute_error > 0.001 || unchanged.changed_fraction > 0.0001 {
            return Err(io::Error::other(format!("night dimmed sealed-room lantern receivers: {unchanged:?}")).into());
        }
        capture_scene(renderer, capture, "time-night-lantern", &night)?;
        let mut distant = Player::new(Vec3::new(7.5, -39.0, -3.5));
        aim_at(&mut distant, Vec3::new(9.8, -37.5, -3.5));
        let distant_lit = renderer.measure_scene_color(&frame(&world, &distant))?;
        world.edit_block(source, Block::Air);
        upload(renderer, &world, terrain, deadline)?;
        let distant_dark = renderer.measure_scene_color(&frame(&world, &distant))?;
        if distant_lit.iter().zip(distant_dark).any(|(lit, dark)| (*lit - dark).abs() > 0.00001) { return Err(io::Error::other("lantern brightened a distant room beyond its reach").into()); }
        world.edit_block(source, Block::Lantern);
        upload(renderer, &world, terrain, deadline)?;
        // An entire sealed plane blocks flood transport, including all paths around it.
        for z in -6..=6 { for y in -40..=-34 { world.edit_block([2, y, z], Block::Stone); } }
        let mut far = Player::new(Vec3::new(4.5, -39.0, 2.5));
        aim_at(&mut far, Vec3::new(2.1, -37.5, 2.5));
        upload(renderer, &world, terrain, deadline)?;
        let sealed_lit = renderer.measure_scene_color(&frame(&world, &far))?;
        capture_scene(renderer, capture, "lantern-sealed-far-side", &frame(&world, &far))?;
        world.edit_block(source, Block::Air);
        upload(renderer, &world, terrain, deadline)?;
        let sealed_dark = renderer.measure_scene_color(&frame(&world, &far))?;
        if sealed_lit.iter().zip(sealed_dark).any(|(lit, dark)| (*lit - dark).abs() > 0.00001) { return Err(io::Error::other("lantern leaked through the solid partition").into()); }
        for z in -6..=6 { for y in -40..=-34 { world.edit_block([2, y, z], if z == -6 || z == 6 || y == -40 || y == -34 { Block::Stone } else { Block::Air }); } }
        upload(renderer, &world, terrain, deadline)?;
        let removed = renderer.measure_frame(&frame(&world, &player))?;
        capture_scene(renderer, capture, "lantern-removed-room", &frame(&world, &player))?;
        if (removed.mean_luminance - unlit.mean_luminance).abs() > 0.00001 { return Err(io::Error::other("removed lantern retained light").into()); }
        if native && !renderer.render(&frame(&world, &player))? { return Err(io::Error::other("lantern completion frame not presented").into()); }
        println!("Lantern GPU verified: headlamp/source out of view, receiver luminance {:.4} → {:.4}; warm RGB {color:?}, distant/ sealed far side unchanged, removal restores darkness", unlit.mean_luminance, lit.mean_luminance);
        Ok(())
    })
}

fn upload(
    renderer: &mut Renderer,
    world: &ResidentWorld,
    terrain: Terrain,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    for z in -1..=0 {
        for x in -1..=0 {
            check_deadline(deadline)?;
            let coord = ChunkCoord::new(x, z);
            let chunk = world
                .chunk(coord)
                .ok_or_else(|| io::Error::other("lantern fixture missing residency"))?;
            renderer.upload_chunk(
                coord,
                &mesh_chunk_with_edits(chunk, &terrain, world.edits()),
            );
        }
    }
    Ok(())
}

fn frame<'a>(world: &'a ResidentWorld, player: &'a Player) -> FrameState<'a> {
    FrameState {
        world,
        player,
        hud: &[],
        survival: None,
        elapsed: 1.0,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: Some(1.0),
        render_distance: 2,
        underwater: false,
        water: None,
        sky_exposure: 0.0,
        headlamp: false,
        in_lava: false,
    }
}
