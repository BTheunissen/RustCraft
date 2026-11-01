//! Buried rock cutaway and ordinary-rendering restoration checks.

use super::caves::scenic_yaw;
use super::{
    Error, FrameState, Instant, Path, Player, Renderer, Terrain, Vec3, aim_at,
    capture_geometry_scene, capture_scene, check_deadline, io, load_scene,
};
use crate::hud::{HudInfo, build_hud};
use crate::phase::{PhaseCache, phase_layers};
use crate::player::EYE_HEIGHT;
use crate::world::{CaveBiome, ChunkCoord, MIN_WORLD_Y};

pub(super) fn verify_phasing(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    let cave = Vec3::from_array(
        terrain
            .cave_position(CaveBiome::Rocky)
            .ok_or_else(|| io::Error::other("no cave for phase verification"))?,
    );
    let cave_eye = cave + Vec3::Y * EYE_HEIGHT;
    let embedded = embedded_cave_eye(terrain, cave_eye)
        .ok_or_else(|| io::Error::other("no solid cave wall for phase verification"))?;
    let world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(embedded.x, embedded.z),
        3,
        deadline,
    )?;
    let mut player = Player::new(embedded - Vec3::Y * EYE_HEIGHT);
    aim_at(&mut player, cave_eye);
    let mut normal_stats = None;
    let mut phase_stats = None;
    for flying in [false, true] {
        player.flying = flying;
        let phasing = PhaseCache::phasing(&world, player.eye_position(), flying);
        if phasing != flying {
            return Err(
                io::Error::other("phase effect did not follow flight/solid occupancy").into(),
            );
        }
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying,
                swimming: false,
                phasing,
                biome: "Inside Rock",
                headlamp: true,
                underground: true,
                position: player.position.to_array(),
                fps: 60.0,
                loaded_chunks: 49,
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
            elapsed: 1.0,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: None,
            render_distance: 3,
            underwater: false,
            water: None,
            in_lava: false,
            sky_exposure: 0.0,
            headlamp: true,
        };
        let label = if flying {
            "phase-cutaway"
        } else {
            "phase-reference"
        };
        let stats = capture_geometry_scene(renderer, capture, label, &frame)?;
        if flying {
            let reference =
                normal_stats.ok_or_else(|| io::Error::other("missing phase reference"))?;
            verify_phase_readback(reference, stats)?;
            phase_stats = Some(stats);
        } else {
            normal_stats = Some(stats);
        }
        check_deadline(deadline)?;
    }
    let rays = phase_layers(
        embedded,
        (cave_eye - embedded).normalize(),
        crate::phase::PHASE_RANGE,
        |x, y, z| Some(terrain.block_at(x, y, z)),
    );
    if rays.is_empty() {
        return Err(
            io::Error::other("phase reference did not contain actual buried rock faces").into(),
        );
    }
    if let Some(stats) = phase_stats {
        println!(
            "Phase cutaway readback: scene luminance {:.3}, contrast {:.3}, visible {:.1}%",
            stats.mean_luminance,
            stats.luminance_deviation,
            stats.visible_fraction * 100.0
        );
    }
    let mut comparison_eye = embedded;
    comparison_eye.y = (MIN_WORLD_Y + 12) as f32 + 0.5;
    let deep_eye = solid_phase_eye(terrain, comparison_eye)
        .ok_or_else(|| io::Error::other("no fully buried rock for phase verification"))?;
    {
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(deep_eye.x, deep_eye.z),
            2,
            deadline,
        )?;
        player.position = deep_eye - Vec3::Y * EYE_HEIGHT;
        player.flying = true;
        player.yaw = 0.65;
        player.pitch = -0.18;
        if !PhaseCache::phasing(&world, deep_eye, true) {
            return Err(io::Error::other("deep rock did not activate the creative cutaway").into());
        }
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying: true,
                swimming: false,
                phasing: true,
                biome: "Deep Rock Cutaway",
                headlamp: false,
                underground: true,
                position: player.position.to_array(),
                fps: 60.0,
                loaded_chunks: 25,
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
            elapsed: 1.0,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: None,
            render_distance: 2,
            underwater: false,
            water: None,
            in_lava: false,
            sky_exposure: 0.0,
            headlamp: false,
        };
        let stats = capture_geometry_scene(renderer, capture, "phase-deep-rock", &frame)?;
        if stats.visible_fraction < 0.05 || stats.luminance_deviation < 0.012 {
            return Err(io::Error::other(
                "deep solid phase readback was an empty or flat-tinted void",
            )
            .into());
        }
        check_deadline(deadline)?;
        println!(
            "Phase GPU verified: deep solid with lamp off, scene luminance {:.3}, contrast {:.3}",
            stats.mean_luminance, stats.luminance_deviation
        );
        let saved_pose = (player.position, player.yaw, player.pitch);
        let mut times = Vec::with_capacity(48);
        for step in 0..48 {
            player.position = deep_eye - Vec3::Y * EYE_HEIGHT + Vec3::X * (step as f32 * 0.04);
            player.yaw = 0.65 + step as f32 * 0.006;
            let frame = FrameState {
                survival: None,
                world: &world,
                player: &player,
                hud: &[],
                elapsed: 1.0,
                time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
                water_time: None,
                render_distance: 2,
                underwater: false,
                water: None,
                in_lava: false,
                sky_exposure: 0.0,
                headlamp: false,
            };
            let started = Instant::now();
            renderer.render_offscreen(&frame)?;
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            check_deadline(deadline)?;
        }
        times.sort_by(f64::total_cmp);
        println!(
            "Phase GPU frame cost: median {:.2} ms, p95 {:.2} ms (48 headless render + completion samples)",
            times[times.len() / 2],
            times[times.len() * 95 / 100]
        );
        (player.position, player.yaw, player.pitch) = saved_pose;
    }
    player.position = cave;
    let world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(cave.x, cave.z),
        2,
        deadline,
    )?;
    if PhaseCache::phasing(&world, player.eye_position(), true) {
        return Err(io::Error::other("phase effect stayed active after leaving rock").into());
    }
    player.pitch = -0.08;
    player.yaw = scenic_yaw(&terrain, player.eye_position());
    let mut ordinary = None;
    for flying in [false, true] {
        player.flying = flying;
        let frame = FrameState {
            survival: None,
            world: &world,
            player: &player,
            hud: &[],
            elapsed: 1.0,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: None,
            render_distance: 2,
            underwater: false,
            water: None,
            in_lava: false,
            sky_exposure: 0.0,
            headlamp: true,
        };
        let stats = if flying {
            capture_scene(renderer, capture, "phase-restored", &frame)?
        } else {
            renderer.measure_frame(&frame)?
        };
        if let Some(reference) = ordinary {
            verify_restored_readback(reference, stats)?;
        } else {
            ordinary = Some(stats);
        }
        check_deadline(deadline)?;
    }
    println!(
        "Phase GPU verified: textured solid interior {:?}, {} local voxel layers; restored ordinary cave view",
        embedded.to_array(),
        rays.len()
    );
    Ok(())
}

pub(super) fn verify_restored_readback(
    ordinary: crate::renderer::FrameStats,
    restored: crate::renderer::FrameStats,
) -> Result<(), io::Error> {
    if (ordinary.mean_luminance - restored.mean_luminance).abs() > 0.001
        || (ordinary.luminance_deviation - restored.luminance_deviation).abs() > 0.001
    {
        return Err(io::Error::other(
            "flight in clear air changed the ordinary cave rendering",
        ));
    }
    Ok(())
}

pub(super) fn solid_phase_eye(terrain: Terrain, center: Vec3) -> Option<Vec3> {
    for radius in 0_i32..=12 {
        for z in -radius..=radius {
            for x in -radius..=radius {
                if x.abs().max(z.abs()) != radius {
                    continue;
                }
                let eye = center + Vec3::new(x as f32, 0.0, z as f32);
                if (-2..=2).all(|dz| {
                    (-2..=2).all(|dx| {
                        (-2..=2).all(|dy| {
                            terrain
                                .block_at(
                                    eye.x.floor() as i32 + dx,
                                    eye.y.floor() as i32 + dy,
                                    eye.z.floor() as i32 + dz,
                                )
                                .is_solid()
                        })
                    })
                }) {
                    return Some(eye);
                }
            }
        }
    }
    None
}

pub(super) fn embedded_cave_eye(terrain: Terrain, eye: Vec3) -> Option<Vec3> {
    for index in 0..16 {
        let yaw = index as f32 * std::f32::consts::TAU / 16.0;
        let direction = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        for step in 2..=24 {
            let point = eye + direction * step as f32;
            if terrain
                .block_at(
                    point.x.floor() as i32,
                    point.y.floor() as i32,
                    point.z.floor() as i32,
                )
                .is_solid()
            {
                let inside = point + direction * 1.5;
                if terrain
                    .block_at(
                        inside.x.floor() as i32,
                        inside.y.floor() as i32,
                        inside.z.floor() as i32,
                    )
                    .is_solid()
                {
                    return Some(inside);
                }
                break;
            }
        }
    }
    None
}

pub(super) fn verify_phase_readback(
    reference: crate::renderer::FrameStats,
    phased: crate::renderer::FrameStats,
) -> Result<(), io::Error> {
    if phased.visible_fraction < 0.05
        || phased.luminance_deviation < 0.012
        || (phased.mean_luminance - reference.mean_luminance).abs() < 0.008
    {
        return Err(io::Error::other(
            "creative phase view did not reveal visible, distinct rock geometry",
        ));
    }
    Ok(())
}
