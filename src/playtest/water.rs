//! Aquatic surface, optics, animation, and waterline acceptance.

use super::swimming::{verify_current_scene, verify_swimming_scene, verify_water_frame_cost};
use super::{
    Error, FrameState, Instant, Path, Player, Renderer, Terrain, Vec3, aim_at,
    capture_geometry_scene, capture_scene, check_deadline, io, load_scene,
};
use crate::hud::{HudInfo, build_hud};
use crate::phase::PhaseCache;
use crate::player::EYE_HEIGHT;
use crate::world::{ChunkCoord, WaterBiome, WorldView};

/// Verifies generated aquatic scenes, actual temporal shader changes, and swimming.
pub fn verify_water(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    for (biome, label) in [
        (WaterBiome::Lake, "lake"),
        (WaterBiome::River, "river"),
        (WaterBiome::Waterfall, "waterfall"),
        (WaterBiome::Estuary, "estuary"),
        (WaterBiome::Ocean, "ocean"),
    ] {
        check_deadline(deadline)?;
        let site = terrain
            .water_landmark(biome)
            .ok_or_else(|| io::Error::other(format!("no generated {label} water landmark")))?;
        let sample = terrain
            .water_at(site.water[0], site.water[1], site.water[2])
            .ok_or_else(|| {
                io::Error::other(format!("{label} landmark contains no actual water"))
            })?;
        if sample.biome != biome {
            return Err(io::Error::other(format!(
                "{label} landmark describes {:?} water",
                sample.biome
            ))
            .into());
        }
        let mut player = Player::new(Vec3::from_array(site.position));
        player.flying = true;
        aim_at(&mut player, Vec3::from_array(site.target));
        let radius = if biome == WaterBiome::Ocean { 6 } else { 4 };
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(player.position.x, player.position.z),
            radius,
            deadline,
        )?;
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying: true,
                swimming: false,
                phasing: false,
                headlamp: true,
                underground: false,
                biome: biome.name(),
                position: player.position.to_array(),
                fps: 60.0,
                loaded_chunks: ((radius * 2 + 1).pow(2)) as usize,
                pending_chunks: 0,
                speed: crate::player::SpeedMultiplier::Normal,
                time: crate::game::WorldTime::default(),
            },
        );
        let frame = FrameState {
            survival: None,
            world: &world,
            player: &player,
            hud: &hud,
            elapsed: 2.2,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: Some(2.2),
            render_distance: radius,
            underwater: false,
            water: None,
            in_lava: false,
            sky_exposure: 1.0,
            headlamp: true,
        };
        let stats = capture_geometry_scene(renderer, capture, &format!("water-{label}"), &frame)?;
        verify_water_scene(stats, label)?;
        verify_water_pass(renderer.measure_water_pass(&frame)?, label)?;
        let later = FrameState {
            water_time: Some(3.6),
            ..frame
        };
        let difference = renderer.compare_frames(&frame, &later)?;
        verify_water_animation(difference, label)?;
        if biome == WaterBiome::Ocean {
            let mut coastal = player.clone();
            coastal.position.y += 18.0;
            coastal.pitch = -0.65;
            let overview = FrameState {
                world: &world,
                player: &coastal,
                hud: &[],
                ..frame
            };
            capture_scene(renderer, capture, "water-ocean-coast", &overview)?;
        }
        check_deadline(deadline)?;
        if matches!(biome, WaterBiome::Lake | WaterBiome::Ocean) {
            let position = submerged_water_pose(sample, site.water)?;
            let mut submerged = Player::new(position);
            submerged.flying = true;
            aim_at(
                &mut submerged,
                Vec3::new(
                    position.x + 8.0,
                    (sample.bottom + 1) as f32,
                    position.z - 6.0,
                ),
            );
            let eye = submerged.eye_position();
            let [x, y, z] = eye.floor().as_ivec3().to_array();
            let water = world.water_at(x, y, z).ok_or_else(|| {
                io::Error::other(format!("{label} submerged camera is not in resident water"))
            })?;
            if eye.y >= water.surface || PhaseCache::phasing(&world, eye, true) {
                return Err(io::Error::other(format!(
                    "{label} underwater camera has invalid water/phase state"
                ))
                .into());
            }
            let underwater = FrameState {
                world: &world,
                player: &submerged,
                hud: &[],
                underwater: true,
                water: Some(water),
                ..frame
            };
            let stats = capture_scene(
                renderer,
                capture,
                &format!("water-{label}-underwater"),
                &underwater,
            )?;
            verify_water_scene(stats, label)?;
            let color = renderer.measure_scene_color(&underwater)?;
            let other = crate::world::WaterSample {
                salinity: if biome == WaterBiome::Lake { 1.0 } else { 0.0 },
                ..water
            };
            let other_frame = FrameState {
                water: Some(other),
                ..underwater
            };
            let other_color = renderer.measure_scene_color(&other_frame)?;
            verify_water_palette(color, other_color)?;
            println!(
                "Underwater GPU verified: {label}, mean RGB {:?}, contrast {:.3}, alternate-palette RGB {:?}",
                color, stats.luminance_deviation, other_color
            );
            if biome == WaterBiome::Lake {
                verify_swimming_scene(&world, position, deadline)?;
            }
            let below_eye = water.surface - 0.8;
            let mut upward = submerged.clone();
            upward.position.y = below_eye - EYE_HEIGHT;
            upward.pitch = 1.05;
            let below_water = world
                .water_at(x, below_eye.floor() as i32, z)
                .ok_or_else(|| {
                    io::Error::other(format!(
                        "{label} surface-below camera is not in resident water"
                    ))
                })?;
            let upward_frame = FrameState {
                world: &world,
                player: &upward,
                water: Some(below_water),
                ..underwater
            };
            let reverse_surface = renderer.measure_water_pass(&upward_frame)?;
            verify_water_pass(reverse_surface, &format!("{label} surface from below"))?;
            capture_scene(
                renderer,
                capture,
                &format!("water-{label}-surface-below"),
                &upward_frame,
            )?;
            let mut restored = upward.clone();
            restored.position.y = water.surface - EYE_HEIGHT + 1.0;
            restored.pitch = -0.35;
            let restored_frame = FrameState {
                world: &world,
                player: &restored,
                underwater: false,
                water: None,
                ..upward_frame
            };
            capture_scene(
                renderer,
                capture,
                &format!("water-{label}-surfaced"),
                &restored_frame,
            )?;
            let transition = renderer.compare_frames(&upward_frame, &restored_frame)?;
            verify_water_animation(transition, "waterline transition")?;
            let ordinary = FrameState {
                water: Some(water),
                ..restored_frame
            };
            verify_surfaced_readback(renderer.compare_frames(&restored_frame, &ordinary)?)?;
            println!(
                "Waterline GPU verified: {label}, reverse surface changes {:.1}% of scene; ordinary optics restored after surfacing",
                reverse_surface.changed_fraction * 100.0
            );
            if biome == WaterBiome::Ocean {
                verify_water_frame_cost(renderer, &frame, deadline)?;
            }
        }
        if biome == WaterBiome::River {
            verify_current_scene(&world, sample, site.water, deadline)?;
        }
        println!(
            "Water GPU verified: {label}, surface {:.2}, depth {:.2}, flow {:?}, salinity {:.2}, changed {:.1}%",
            sample.surface,
            sample.surface - (sample.bottom + 1) as f32,
            sample.flow,
            sample.salinity,
            difference.changed_fraction * 100.0
        );
    }
    Ok(())
}

pub(super) fn submerged_water_pose(
    sample: crate::world::WaterSample,
    water: [i32; 3],
) -> Result<Vec3, io::Error> {
    let depth = sample.surface - (sample.bottom + 1) as f32;
    if !depth.is_finite() || depth < 3.2 {
        return Err(io::Error::other(
            "water diagnostic is too shallow for underwater exploration",
        ));
    }
    Ok(Vec3::new(
        water[0] as f32 + 0.5,
        ((sample.bottom + 1) as f32 + 1.0).min(sample.surface - EYE_HEIGHT - 0.8),
        water[2] as f32 + 0.5,
    ))
}

pub(super) fn verify_water_scene(
    stats: crate::renderer::FrameStats,
    label: &str,
) -> Result<(), io::Error> {
    if !stats.mean_luminance.is_finite()
        || !stats.visible_fraction.is_finite()
        || !stats.luminance_deviation.is_finite()
        || stats.visible_fraction < 0.04
        || stats.luminance_deviation < 0.008
    {
        return Err(io::Error::other(format!(
            "{label} water scene is empty, dark, or uniformly tinted: luminance {:.3}, contrast {:.4}, visible {:.1}%",
            stats.mean_luminance,
            stats.luminance_deviation,
            stats.visible_fraction * 100.0,
        )));
    }
    Ok(())
}

pub(super) fn verify_water_animation(
    difference: crate::renderer::FrameDifference,
    label: &str,
) -> Result<(), io::Error> {
    if !difference.changed_fraction.is_finite()
        || !difference.mean_absolute_error.is_finite()
        || difference.changed_fraction < 0.003
        || difference.mean_absolute_error < 0.00002
    {
        return Err(io::Error::other(format!(
            "{label} water did not visibly change between shader times"
        )));
    }
    Ok(())
}

pub(super) fn verify_water_pass(
    difference: crate::renderer::FrameDifference,
    label: &str,
) -> Result<(), io::Error> {
    if !difference.mean_absolute_error.is_finite()
        || !difference.changed_fraction.is_finite()
        || difference.mean_absolute_error < 0.00005
        || difference.changed_fraction < 0.005
    {
        return Err(io::Error::other(format!(
            "{label} has no visible water surface: mean difference {:.5}, changed {:.2}%",
            difference.mean_absolute_error,
            difference.changed_fraction * 100.0
        )));
    }
    Ok(())
}

pub(super) fn verify_surfaced_readback(
    difference: crate::renderer::FrameDifference,
) -> Result<(), io::Error> {
    if !difference.mean_absolute_error.is_finite()
        || !difference.changed_fraction.is_finite()
        || difference.mean_absolute_error > 0.00001
        || difference.changed_fraction > 0.0001
    {
        return Err(io::Error::other(
            "underwater camera optics remained active after surfacing",
        ));
    }
    Ok(())
}

pub(super) fn verify_water_palette(actual: [f32; 3], alternate: [f32; 3]) -> Result<(), io::Error> {
    let difference = actual
        .into_iter()
        .zip(alternate)
        .map(|(actual, alternate)| (actual - alternate).abs())
        .sum::<f32>();
    if !difference.is_finite() || difference < 0.025 {
        return Err(io::Error::other(
            "freshwater and marine camera optics did not produce distinct GPU palettes",
        ));
    }
    Ok(())
}
