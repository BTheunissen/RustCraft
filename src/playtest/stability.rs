//! Seeded walking and anchor crossings exercise the real interactive lighting schedule.

mod motion;
mod walking;

use super::{check_deadline, load_scene};
use crate::game::Game;
use crate::renderer::{FrameState, LightingFeatures, Renderer};
use crate::world::{ChunkCoord, Terrain};
use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;
use winit::dpi::PhysicalSize;

pub(super) fn verify(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let size = renderer.size();
    renderer.resize(PhysicalSize::new(960, 608));
    let result = renderer.with_lighting_features(
        LightingFeatures {
            bloom: false,
            ..Default::default()
        },
        |renderer| {
            verify_walk(renderer, terrain, capture, native, deadline)?;
            motion::verify(renderer, terrain, deadline)
        },
    );
    renderer.resize(PhysicalSize::new(size.0, size.1));
    result
}

fn verify_walk(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut game = Game::new(terrain.seed());
    game.world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(game.player.position.x, game.player.position.z),
        4,
        deadline,
    )?;
    let route = walking::route(&game.world, &game.player)?;
    let first = frame(&mut game);
    renderer.measure_frame(&first)?;
    let textures = renderer.measure_texture_stability(&first)?;
    if textures.pixels < 100
        || ![
            textures.filtered_error,
            textures.nearest_error,
            textures.filtered_variation,
            textures.nearest_variation,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
        || textures.filtered_error >= textures.nearest_error * 0.9
        || textures.filtered_variation >= textures.nearest_variation * 0.9
    {
        return Err(io::Error::other(format!(
            "minification did not stabilize seeded textures: {textures:?}"
        ))
        .into());
    }
    println!(
        "Image stability GPU: {} minified surface pixels, 16-sample reference error nearest {:.6}, filtered {:.6}; subpixel variation nearest {:.6}, filtered {:.6}",
        textures.pixels,
        textures.nearest_error,
        textures.filtered_error,
        textures.nearest_variation,
        textures.filtered_variation
    );
    let mut previous_anchor = renderer.light_volume_origin();
    let mut shifts = 0;
    let mut worst = 0.0_f32;
    let mut reverse_worst = 0.0_f32;
    let mut frames = Vec::new();
    for player in &route {
        check_deadline(deadline)?;
        game.player = player.clone();
        let frame = frame(&mut game);
        let (pixels, difference) = renderer.compare_interactive_frame(&frame)?;
        let anchor = renderer.light_volume_origin();
        if anchor != previous_anchor {
            shifts += 1;
            previous_anchor = anchor;
        }
        if difference.mean_absolute_error > worst {
            worst = difference.mean_absolute_error;
        }
        require_motion_error(difference.mean_absolute_error, 0.0025, "forward walking")?;
        if capture.is_some() {
            frames.push(pixels);
        }
    }
    if shifts == 0 {
        return Err(io::Error::other("seeded walking did not cross a lighting anchor").into());
    }
    if let Some(path) = capture {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        renderer.save_motion_capture(
            &path.with_file_name(format!("{stem}-stability-walk.png")),
            &frames,
            30,
        )?;
    }
    for player in route.iter().rev().step_by(3) {
        check_deadline(deadline)?;
        game.player = player.clone();
        let (_, difference) = renderer.compare_interactive_frame(&frame(&mut game))?;
        reverse_worst = reverse_worst.max(difference.mean_absolute_error);
        require_motion_error(difference.mean_absolute_error, 0.0025, "reverse walking")?;
    }
    let frame = frame(&mut game);
    let first = renderer.read_interactive_frame(&frame)?;
    let (width, height) = renderer.size();
    let mut settling = 0.0_f32;
    for _ in 0..8 {
        check_deadline(deadline)?;
        let next = renderer.read_interactive_frame(&frame)?;
        let difference = walking::difference(&first, &next, width, height);
        settling = settling.max(difference);
    }
    require_motion_error(settling, 0.0015, "stationary probe sweep")?;
    if native && !renderer.with_interactive_lighting(|renderer| renderer.render(&frame))? {
        return Err(io::Error::other("image-stability completion was not presented").into());
    }
    println!(
        "Image stability GPU: seed {}, {} walked frames, {shifts} anchor crossings, worst interactive/reference error {worst:.6}, reverse {reverse_worst:.6}, stationary settling error {settling:.6}",
        terrain.seed(),
        route.len()
    );
    Ok(())
}

fn require_motion_error(error: f32, limit: f32, label: &str) -> io::Result<()> {
    if !error.is_finite() || error < 0.0 || error > limit {
        return Err(io::Error::other(format!(
            "{label} image stability error {error:.6} exceeds {limit:.6} or is invalid"
        )));
    }
    Ok(())
}

pub(super) fn verify_edit_frame(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let (_, difference) = renderer.compare_interactive_frame(frame)?;
    if !difference.mean_absolute_error.is_finite()
        || difference.mean_absolute_error > 0.00001
        || difference.changed_fraction > 0.0001
    {
        return Err(io::Error::other(format!(
            "{label} retained stale or missing irradiance on its first interactive frame: {difference:?}"
        )).into());
    }
    println!(
        "Image stability GPU: {label} refreshes irradiance completely before its first displayed frame"
    );
    Ok(())
}

fn frame(game: &mut Game) -> FrameState<'_> {
    let environment = game.environment();
    FrameState {
        world: &game.world,
        player: &game.player,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: Some(2.2),
        render_distance: 4,
        underwater: environment.underwater,
        water: environment.water,
        sky_exposure: environment.sky_exposure,
        headlamp: false,
        in_lava: environment.in_lava,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_acceptance_rejects_flashes_and_invalid_evidence_not_ordinary_variation() {
        for value in [0.0, 0.0008, 0.0025] {
            assert!(require_motion_error(value, 0.0025, "walk").is_ok());
        }
        for value in [0.026586, 0.00251, -0.001, f32::NAN, f32::INFINITY] {
            assert!(require_motion_error(value, 0.0025, "walk").is_err());
        }
    }
}
