//! Air-only GPU evidence plus generated canopy, entrance, and coastal views.

mod fixture;
#[cfg(test)]
mod tests;
mod views;
pub(super) use views::forest_view;

use super::{aim_at, capture_scene, check_deadline, load_scene};
use crate::player::Player;
use crate::renderer::{
    AirEdgeStats, AirStats, FrameDifference, FrameState, LightingFeatures, Renderer,
};
use crate::world::{ChunkCoord, ResidentWorld, Terrain};
use glam::Vec3;
use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

pub(crate) fn verify_volumetrics(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    renderer.with_lighting_features(
        LightingFeatures {
            bloom: false,
            ..Default::default()
        },
        |renderer| {
            fixture::verify(renderer, terrain, capture, native, deadline)?;
            verify_generated(renderer, terrain, capture, deadline)
        },
    )
}

fn verify_generated(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let coast = terrain
        .coastal_landmark()
        .ok_or_else(|| io::Error::other("no coast for air acceptance"))?;
    let entrance = terrain
        .entrance_position()
        .ok_or_else(|| io::Error::other("no natural entrance for air acceptance"))?;
    let (forest, forest_target) = views::forest_view(terrain)
        .ok_or_else(|| io::Error::other("no visible forest canopy for air acceptance"))?;
    for (label, position, target, radius) in [
        (
            "coast",
            Vec3::from_array(coast.arch.view),
            Vec3::from_array(coast.arch.opening) + Vec3::Y * 3.0,
            5,
        ),
        (
            "entrance",
            Vec3::from_array(entrance) + Vec3::Y * 1.5,
            Vec3::from_array(entrance) + Vec3::new(0.0, -3.0, -4.0),
            4,
        ),
        ("forest", forest, forest_target, 4),
    ] {
        check_deadline(deadline)?;
        let mut player = Player::new(position);
        aim_at(&mut player, target);
        if label == "entrance" {
            player.yaw = super::caves::entrance_yaw(terrain, player.eye_position());
            player.pitch = -0.45;
        }
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(position.x, position.z),
            radius,
            deadline,
        )?;
        let frame = frame(&world, &player, radius, 1.0);
        if label == "entrance" && terrain.seed() == Terrain::default().seed() {
            let peak = renderer.measure_entrance_background(&frame)?;
            if !peak.is_finite() || peak > 0.035 {
                return Err(io::Error::other(format!(
                    "unloaded cave entrance was backlit by the sky: HDR peak {peak:.5}"
                ))
                .into());
            }
            println!(
                "Volumetric GPU verified: natural entrance interior background stays dark, HDR peak {peak:.5}"
            );
        }
        let air = renderer.measure_air(&frame)?;
        require_readable_air(air, label)?;
        let difference = renderer.compare_lighting(
            &frame,
            LightingFeatures {
                volumetrics: false,
                bloom: false,
                ..Default::default()
            },
        )?;
        if !difference.mean_absolute_error.is_finite() || difference.mean_absolute_error < 0.00001 {
            return Err(io::Error::other(format!(
                "{label} air did not reach the displayed HDR composition: {difference:?}"
            ))
            .into());
        }
        let upscaled = renderer.compare_lighting(
            &frame,
            LightingFeatures {
                volumetric_reference: true,
                bloom: false,
                ..Default::default()
            },
        )?;
        require_upscale(upscaled)?;
        if label == "forest" {
            let unshadowed = renderer.with_lighting_features(
                LightingFeatures {
                    shadows: false,
                    bloom: false,
                    ..Default::default()
                },
                |renderer| renderer.measure_air(&frame),
            )?;
            if unshadowed.mean_radiance < air.mean_radiance * 1.05 {
                return Err(io::Error::other(format!("canopy geometry did not attenuate air sunlight: shadowed {air:?}, unshadowed {unshadowed:?}")).into());
            }
            println!(
                "Volumetric GPU verified: actual canopy shadows reduce air energy from {:.5} to {:.5}",
                unshadowed.mean_radiance, air.mean_radiance
            );
        }
        let edges = renderer.measure_air_edges(&frame)?;
        require_edges(edges)?;
        let stats = capture_scene(renderer, capture, &format!("air-{label}"), &frame)?;
        if stats.luminance_deviation < 0.04 {
            return Err(io::Error::other(format!(
                "{label} clear-air view collapsed to a fog bank: {stats:?}"
            ))
            .into());
        }
        if let Some(path) = capture {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            renderer.with_lighting_features(
                LightingFeatures {
                    volumetrics: false,
                    bloom: false,
                    ..Default::default()
                },
                |renderer| {
                    renderer.save_screenshot(
                        &path.with_file_name(format!("{stem}-air-{label}-ambient.png")),
                        &frame,
                    )
                },
            )?;
        }
        println!(
            "Volumetric GPU verified: generated {label}, air mean {:.5}, peak {:.5}, variation {:.5}, composite difference {:.5}, half/full-resolution error {:.6}; {} edge pixels, peak HDR error {:.6}",
            air.mean_radiance,
            air.peak_radiance,
            air.deviation,
            difference.mean_absolute_error,
            upscaled.mean_absolute_error,
            edges.pixels,
            edges.peak_error
        );
        if label == "coast" {
            let baseline = renderer.with_lighting_features(
                LightingFeatures {
                    volumetrics: false,
                    bloom: false,
                    ..Default::default()
                },
                |renderer| render_cost(renderer, &frame, deadline),
            )?;
            let shadowed = render_cost(renderer, &frame, deadline)?;
            println!(
                "Volumetric render+completion cost: {}x{}, ambient {:.2} ms, shadowed {:.2} ms (median of 7; full probe updates, not isolated GPU timestamps)",
                renderer.size().0,
                renderer.size().1,
                baseline,
                shadowed
            );
        }
        if label == "entrance" {
            verify_inside_entrance(renderer, capture, &frame)?;
        }
    }
    Ok(())
}

fn verify_inside_entrance(
    renderer: &mut Renderer,
    capture: Option<&Path>,
    outside: &FrameState<'_>,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    let (point, target) =
        views::entrance_view(outside.world, outside.player, width as f32 / height as f32)
            .ok_or_else(|| {
                io::Error::other("natural entrance has no clear resident roofed interior view")
            })?;
    let mut player = Player::new(point - Vec3::Y * crate::player::EYE_HEIGHT);
    player.flying = true;
    aim_at(&mut player, target);
    let inside = FrameState {
        player: &player,
        sky_exposure: 0.0,
        ..*outside
    };
    let air = renderer.measure_air(&inside)?;
    require_readable_air(air, "entrance interior")?;
    let reference = renderer.compare_lighting(
        &inside,
        LightingFeatures {
            bloom: false,
            volumetric_reference: true,
            ..Default::default()
        },
    )?;
    require_upscale(reference)?;
    capture_scene(renderer, capture, "air-entrance-inside", &inside)?;
    println!(
        "Volumetric GPU verified: roofed natural entrance looking outward, air mean {:.5}, variation {:.5}, peak {:.5}, half/full-resolution error {:.6}",
        air.mean_radiance, air.deviation, air.peak_radiance, reference.mean_absolute_error
    );
    Ok(())
}

fn render_cost(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    deadline: Instant,
) -> Result<f32, Box<dyn Error>> {
    let mut samples = [0.0_f32; 7];
    for sample in &mut samples {
        check_deadline(deadline)?;
        let started = Instant::now();
        renderer.render_offscreen(frame)?;
        *sample = started.elapsed().as_secs_f32() * 1000.0;
    }
    samples.sort_by(f32::total_cmp);
    Ok(samples[3])
}

fn frame<'a>(
    world: &'a ResidentWorld,
    player: &'a Player,
    render_distance: i32,
    sky_exposure: f32,
) -> FrameState<'a> {
    FrameState {
        world,
        player,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: Some(2.2),
        render_distance,
        underwater: false,
        water: None,
        sky_exposure,
        headlamp: false,
        in_lava: false,
    }
}

fn require_readable_air(air: AirStats, label: &str) -> io::Result<()> {
    if !air.mean_radiance.is_finite()
        || !air.peak_radiance.is_finite()
        || !air.deviation.is_finite()
        || !air.lit_fraction.is_finite()
        || air.mean_radiance < 0.0001
        || air.peak_radiance > 0.23
        || air.lit_fraction < 0.01
        || air.deviation < 0.0001
    {
        return Err(io::Error::other(format!(
            "{label} air is absent, flat, excessive, or invalid: {air:?}"
        )));
    }
    Ok(())
}

fn require_upscale(difference: FrameDifference) -> io::Result<()> {
    if !difference.mean_absolute_error.is_finite()
        || !difference.changed_fraction.is_finite()
        || difference.mean_absolute_error > 0.002
        || difference.changed_fraction > 0.05
    {
        return Err(io::Error::other(format!(
            "depth-aware air upscale diverged from full-resolution rays: {difference:?}"
        )));
    }
    Ok(())
}

fn require_edges(edges: AirEdgeStats) -> io::Result<()> {
    if edges.pixels < 10
        || !edges.mean_error.is_finite()
        || !edges.peak_error.is_finite()
        || edges.mean_error > 0.002
        || edges.peak_error > 0.035
    {
        return Err(io::Error::other(format!(
            "air at block edges disagrees with full-resolution HDR rays: {edges:?}"
        )));
    }
    Ok(())
}
