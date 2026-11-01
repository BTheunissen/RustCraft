//! Same-pose GPU isolation proves shadows, bounced sunlight, and occluded emissive light.

use super::{aim_at, capture_scene, check_deadline, load_scene};
use crate::lighting::sky_exposure;
use crate::lighting::transport::trace_voxel_ray;
use crate::mesh::mesh_chunk_with_edits;
use crate::player::Player;
use crate::renderer::{FrameDifference, FrameState, LightingFeatures, Renderer};
use crate::world::{Block, ChunkCoord, ResidentWorld, Terrain, WorldView};
use glam::Vec3;
use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

pub(crate) fn verify_lighting(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let site = terrain
        .coastal_landmark()
        .ok_or_else(|| io::Error::other("no coast for sunlight acceptance"))?;
    let mut player = Player::new(Vec3::from_array(site.arch.view));
    aim_at(
        &mut player,
        Vec3::from_array(site.arch.opening) + Vec3::Y * 3.0,
    );
    let world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(player.position.x, player.position.z),
        5,
        deadline,
    )?;
    let coastal_frame = frame(&world, &player, 5, 1.0);
    for (label, features, threshold) in [
        (
            "sunlight shadows",
            LightingFeatures {
                shadows: false,
                ..Default::default()
            },
            0.01,
        ),
        (
            "bounced daylight",
            LightingFeatures {
                bounce: false,
                bloom: false,
                ..Default::default()
            },
            0.0001,
        ),
    ] {
        check_deadline(deadline)?;
        let difference = renderer.with_lighting_features(
            LightingFeatures {
                bloom: false,
                ..Default::default()
            },
            |renderer| {
                renderer.compare_lighting(
                    &coastal_frame,
                    LightingFeatures {
                        bloom: false,
                        ..features
                    },
                )
            },
        )?;
        require_difference(difference, label, threshold)?;
        println!(
            "Lighting GPU verified: {label}, same-pose changed {:.2}% of scene, mean difference {:.5}",
            difference.changed_fraction * 100.0,
            difference.mean_absolute_error
        );
    }
    capture_scene(renderer, capture, "lighting-coast", &coastal_frame)?;
    let saved = player.position;
    player.position += Vec3::X * 0.001;
    let shifted = frame(&world, &player, 5, 1.0);
    let mut stationary_player = player.clone();
    stationary_player.position = saved;
    let stationary = frame(&world, &stationary_player, 5, 1.0);
    let stability = renderer.compare_frames(&stationary, &shifted)?;
    if stability.mean_absolute_error > 0.002 {
        return Err(io::Error::other(format!(
            "sub-texel camera movement destabilized sunlight: {stability:?}"
        ))
        .into());
    }
    println!(
        "Lighting GPU verified: sub-texel camera motion, mean difference {:.6}",
        stability.mean_absolute_error
    );
    let (first, converged) = renderer.compare_progressive_lighting(&stationary)?;
    if converged.changed_fraction > 0.0001 || converged.mean_absolute_error > 0.00001 {
        return Err(io::Error::other(format!(
            "fixed-budget probes do not converge to the full-update reference: {converged:?}"
        ))
        .into());
    }
    println!(
        "Lighting GPU verified: 512 probes/frame converge in 8 frames, initial mean difference {:.5}, final {:.6}",
        first.mean_absolute_error, converged.mean_absolute_error
    );
    super::day_night::verify(renderer, &stationary, capture, native, deadline)?;
    super::stability::verify(renderer, terrain, capture, native, deadline)?;
    verify_emissive_room(renderer, terrain, capture, native, deadline)?;
    Ok(())
}

fn verify_emissive_room(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let center = ChunkCoord::new(0, 0);
    let mut world = load_scene(renderer, terrain, center, 2, deadline)?;
    // A private edited room uses ordinary voxels/remeshing; no generation shortcut or save.
    for z in -9..=9 {
        for x in -9..=9 {
            for y in -51..=-39 {
                let wall = x == -9 || x == 9 || z == -9 || z == 9 || y == -51 || y == -39;
                world.edit_block([x, y, z], if wall { Block::Limestone } else { Block::Air });
            }
        }
    }
    let mut player = Player::new(Vec3::new(-3.5, -50.0, 4.5));
    aim_at(&mut player, Vec3::new(-8.5, -48.0, 4.5));
    let eye = player.eye_position();
    if sky_exposure(eye, |x, y, z| {
        world.block_at(x, y, z).unwrap_or(Block::Stone)
    }) != 0.0
    {
        return Err(io::Error::other("lighting fixture has an unsealed roof").into());
    }
    upload_fixture(renderer, &world, terrain, deadline)?;
    let unlit = renderer.with_lighting_features(
        LightingFeatures {
            emission: false,
            bloom: false,
            ..Default::default()
        },
        |renderer| renderer.measure_frame(&frame(&world, &player, 2, 0.0)),
    )?;
    if unlit.mean_luminance > 0.035 {
        return Err(io::Error::other(format!(
            "sunlight leaked through the buried roof at minimum residency: {unlit:?}"
        ))
        .into());
    }
    println!(
        "Lighting GPU verified: buried roof at render distance 2 blocks sunlight, unlit luminance {:.4}",
        unlit.mean_luminance
    );
    let mut last_color = [0.0; 3];
    for (emitter, label) in [(Block::Lava, "lava"), (Block::Crystal, "crystal")] {
        for z in -8..=8 {
            for y in -50..=-40 {
                world.edit_block([3, y, z], emitter);
            }
        }
        upload_fixture(renderer, &world, terrain, deadline)?;
        let frame = frame(&world, &player, 2, 0.0);
        let features = LightingFeatures {
            bloom: false,
            ..Default::default()
        };
        let (difference, color) = renderer.with_lighting_features(features, |renderer| {
            let difference = renderer.compare_lighting(
                &frame,
                LightingFeatures {
                    emission: false,
                    bloom: false,
                    ..Default::default()
                },
            )?;
            let color = renderer.measure_scene_color(&frame)?;
            if let Some(path) = capture {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                renderer.save_screenshot(
                    &path.with_file_name(format!("{stem}-lighting-{label}-spill.png")),
                    &frame,
                )?;
            }
            Ok((difference, color))
        })?;
        require_difference(difference, label, 0.002)?;
        let color_valid = if emitter == Block::Lava {
            color[0] > color[2] * 1.10
        } else {
            color[2] > color[0] * 1.10
        };
        if !color_valid {
            return Err(io::Error::other(format!(
                "{label} did not tint non-emissive receiver walls: {color:?}"
            ))
            .into());
        }
        let night = FrameState {
            time_of_day: 22.0,
            ..frame
        };
        // Isolate emission from the sun's separately reserved clear-air fog share.
        let unchanged = renderer.with_lighting_features(
            LightingFeatures {
                volumetrics: false,
                ..features
            },
            |renderer| renderer.compare_frames(&frame, &night),
        )?;
        // Half-float/8-bit quantization and bounded sky bounce can move a few LSBs.
        // A 0.001 mean bound is below one output level and cannot hide emitter dimming.
        if unchanged.mean_absolute_error > 0.001 || unchanged.changed_fraction > 0.0001 {
            return Err(io::Error::other(format!(
                "solar phase changed enclosed {label} receiver emission: {unchanged:?}"
            ))
            .into());
        }
        println!(
            "Lighting GPU verified: {label} lights non-emissive rock with headlamp/bloom off, changed {:.2}%, receiver RGB {color:?}",
            difference.changed_fraction * 100.0
        );
        if emitter == Block::Lava {
            let mut highlight_player = player.clone();
            aim_at(&mut highlight_player, Vec3::new(3.5, -46.0, 0.5));
            let highlight_frame = FrameState {
                player: &highlight_player,
                ..frame
            };
            let hdr = renderer.measure_hdr(&highlight_frame)?;
            if hdr.peak_channel <= 1.1 || hdr.highlight_fraction < 0.01 {
                return Err(io::Error::other(format!(
                    "emissive light did not reach the floating-point HDR target: {hdr:?}"
                ))
                .into());
            }
            let medium = FrameState {
                underwater: true,
                ..frame
            };
            let medium_night = FrameState {
                time_of_day: 22.0,
                ..medium
            };
            let submerged = renderer.with_lighting_features(
                LightingFeatures {
                    volumetrics: false,
                    ..features
                },
                |renderer| renderer.compare_frames(&medium, &medium_night),
            )?;
            if submerged.mean_absolute_error > 0.001 || submerged.changed_fraction > 0.0001 {
                return Err(io::Error::other(format!(
                    "solar phase changed enclosed underwater emitter/fog lighting: {submerged:?}"
                ))
                .into());
            }
            println!(
                "Lighting GPU verified: pre-tone-map HDR peak {:.3}, {:.1}% scene highlights exceed 1.0",
                hdr.peak_channel,
                hdr.highlight_fraction * 100.0
            );
        }
        last_color = color;
    }

    let origin = Vec3::new(-3.5, -48.5, -3.5);
    for z in -8..=8 {
        for y in -50..=-40 {
            world.edit_block([0, y, z], Block::Limestone);
        }
    }
    let hit = trace_voxel_ray(origin, Vec3::X, 10.0, |cell| {
        world.block_at(cell[0], cell[1], cell[2])
    });
    if hit != Some(Block::Limestone) {
        return Err(io::Error::other("light-ray reference did not stop at the edited wall").into());
    }
    upload_fixture(renderer, &world, terrain, deadline)?;
    let sealed_frame = frame(&world, &player, 2, 0.0);
    super::stability::verify_edit_frame(renderer, &sealed_frame, "closing the emitter partition")?;
    let blocked = renderer.with_lighting_features(
        LightingFeatures {
            bloom: false,
            ..Default::default()
        },
        |renderer| {
            renderer.compare_lighting(
                &sealed_frame,
                LightingFeatures {
                    emission: false,
                    bloom: false,
                    ..Default::default()
                },
            )
        },
    )?;
    if blocked.changed_fraction > 0.0001 || blocked.mean_absolute_error > 0.00001 {
        return Err(io::Error::other(format!(
            "crystal light leaked through the full-height rock partition: {blocked:?}"
        ))
        .into());
    }
    capture_scene(renderer, capture, "lighting-sealed-wall", &sealed_frame)?;
    if native && !renderer.render(&sealed_frame)? {
        return Err(
            io::Error::other("lighting acceptance completion frame was not presented").into(),
        );
    }
    println!(
        "Lighting GPU verified: sealed voxel wall blocks emitter transport, changed {:.4}%",
        blocked.changed_fraction * 100.0
    );
    for z in -8..=8 {
        for y in -50..=-40 {
            world.edit_block([0, y, z], Block::Air);
        }
    }
    upload_fixture(renderer, &world, terrain, deadline)?;
    let restored = renderer.with_lighting_features(
        LightingFeatures {
            bloom: false,
            ..Default::default()
        },
        |renderer| renderer.measure_scene_color(&frame(&world, &player, 2, 0.0)),
    )?;
    if restored
        .into_iter()
        .zip(last_color)
        .any(|(actual, expected)| (actual - expected).abs() > 0.00001)
    {
        return Err(io::Error::other(
            "removing the partition did not restore the original irradiance",
        )
        .into());
    }
    println!("Lighting GPU verified: removing the edited wall restores identical receiver color");
    for z in -8..=8 {
        for y in -50..=-40 {
            world.edit_block([3, y, z], Block::Air);
        }
    }
    upload_fixture(renderer, &world, terrain, deadline)?;
    super::stability::verify_edit_frame(
        renderer,
        &frame(&world, &player, 2, 0.0),
        "removing emissive voxels",
    )?;
    let removed = renderer.measure_frame(&frame(&world, &player, 2, 0.0))?;
    if (removed.mean_luminance - unlit.mean_luminance).abs() > 0.002 {
        return Err(
            io::Error::other("removed emitters left stale irradiance in a sealed room").into(),
        );
    }
    println!(
        "Lighting GPU verified: emitter removal clears cached light, luminance {:.4}",
        removed.mean_luminance
    );
    Ok(())
}

fn upload_fixture(
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
                .ok_or_else(|| io::Error::other("lighting fixture lost residency"))?;
            renderer.upload_chunk(
                coord,
                &mesh_chunk_with_edits(chunk, &terrain, world.edits()),
            );
        }
    }
    Ok(())
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

fn require_difference(difference: FrameDifference, label: &str, fraction: f32) -> io::Result<()> {
    if !difference.changed_fraction.is_finite()
        || !difference.mean_absolute_error.is_finite()
        || difference.changed_fraction < fraction
        || difference.mean_absolute_error < 0.00001
    {
        return Err(io::Error::other(format!(
            "{label} did not change actual scene pixels: {difference:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_acceptance_rejects_unchanged_nonfinite_and_subthreshold_frames() {
        let visible = FrameDifference {
            changed_fraction: 0.1,
            mean_absolute_error: 0.01,
        };
        assert!(require_difference(visible, "fixture", 0.01).is_ok());
        for difference in [
            FrameDifference {
                changed_fraction: 0.0,
                ..visible
            },
            FrameDifference {
                mean_absolute_error: 0.0,
                ..visible
            },
            FrameDifference {
                changed_fraction: f32::NAN,
                ..visible
            },
            FrameDifference {
                mean_absolute_error: f32::NAN,
                ..visible
            },
        ] {
            assert!(require_difference(difference, "fixture", 0.01).is_err());
        }
    }
}
