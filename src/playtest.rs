//! Bounded generated-world GPU scenes for smoke verification, never player shortcuts.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

use glam::Vec3;

use crate::mesh::mesh_chunk;
use crate::player::Player;
use crate::renderer::{FrameState, Renderer};
use crate::world::{Chunk, ChunkCoord, ResidentWorld, Terrain};

mod caves;
mod coastal;
mod cutaway;
mod day_night;
mod items;
mod lanterns;
mod lighting;
mod persistence;
mod stability;
mod storage;
pub(crate) use lanterns::verify as verify_lanterns;
pub(crate) mod surface;
mod survival;
mod swimming;
mod volumetrics;
mod water;

pub(crate) use caves::verify_underground;
pub(crate) use coastal::verify_coastal;
pub(crate) use lighting::verify_lighting;
pub(crate) use surface::verify_surface;
pub(crate) use survival::verify_survival;
pub(crate) use volumetrics::verify_volumetrics;
pub(crate) use water::verify_water;

pub(super) fn load_scene(
    renderer: &mut Renderer,
    terrain: Terrain,
    center: ChunkCoord,
    radius: i32,
    deadline: Instant,
) -> Result<ResidentWorld, io::Error> {
    renderer.clear_chunks();
    let mut world = ResidentWorld::default();
    for z in -radius..=radius {
        for x in -radius..=radius {
            check_deadline(deadline)?;
            let coord = ChunkCoord::new(center.x + x, center.z + z);
            let chunk = Chunk::generate(&terrain, coord);
            let mesh = mesh_chunk(&chunk, &terrain);
            renderer.upload_chunk(coord, &mesh);
            world.insert(chunk);
        }
    }
    Ok(world)
}

pub(super) fn capture_scene(
    renderer: &mut Renderer,
    capture: Option<&Path>,
    suffix: &str,
    frame: &FrameState<'_>,
) -> Result<crate::renderer::FrameStats, Box<dyn Error>> {
    if let Some(path) = capture {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        renderer.capture_frame(&path.with_file_name(format!("{stem}-{suffix}.png")), frame)
    } else {
        renderer.measure_frame(frame)
    }
}

pub(super) fn capture_geometry_scene(
    renderer: &mut Renderer,
    capture: Option<&Path>,
    suffix: &str,
    frame: &FrameState<'_>,
) -> Result<crate::renderer::FrameStats, Box<dyn Error>> {
    let geometry = FrameState { hud: &[], ..*frame };
    let stats = renderer.measure_frame(&geometry)?;
    if capture.is_some() {
        capture_scene(renderer, capture, suffix, frame)?;
    }
    Ok(stats)
}

pub(super) fn aim_at(player: &mut Player, target: Vec3) {
    let direction = (target - player.eye_position()).normalize_or_zero();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin().clamp(-1.4, 1.4);
}

pub(super) fn check_deadline(deadline: Instant) -> Result<(), io::Error> {
    if Instant::now() >= deadline {
        return Err(io::Error::other(
            "generated-world GPU verification exceeded the smoke deadline",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
