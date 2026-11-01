//! Underground family, lighting, entrance, and scenery acceptance.

use super::cutaway::verify_phasing;
use super::{
    Error, FrameState, Instant, Path, Player, Renderer, Terrain, Vec3, aim_at,
    capture_geometry_scene, capture_scene, check_deadline, io, load_scene,
};
use crate::hud::{HudInfo, build_hud};
use crate::lighting::sky_exposure;
use crate::player::MovementInput;
use crate::world::{
    Block, CHUNK_SIZE, CaveBiome, CaveLandform, Chunk, ChunkCoord, MIN_WORLD_Y, WorldView,
};

/// Verifies each cave biome with headlamp on/off, using actual GPU shaders and meshes.
/// When a capture path is supplied, cave frames use `<stem>-cave-<biome>[-dark].png`.
pub fn verify_underground(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    renderer.set_diagnostic_deadline(deadline);
    for (biome, label) in [
        (CaveBiome::Rocky, "rocky"),
        (CaveBiome::Lush, "lush"),
        (CaveBiome::Crystal, "crystal"),
    ] {
        check_deadline(deadline)?;
        let position = terrain
            .cave_position(biome)
            .map(Vec3::from_array)
            .ok_or_else(|| io::Error::other(format!("no safe {label} cave found")))?;
        check_deadline(deadline)?;
        let mut player = verify_cave_pose(terrain, position, label)?;
        player.yaw = scenic_yaw(&terrain, player.eye_position());
        player.pitch = -0.08;
        if biome == CaveBiome::Crystal
            && let Some(target) =
                nearby_visible_growth(terrain, player.eye_position(), Block::Crystal)
        {
            aim_at(&mut player, target);
        }
        let center = ChunkCoord::from_world(position.x, position.z);
        let radius = 3;
        let world = load_scene(renderer, terrain, center, radius, deadline)?;
        let eye = player.eye_position();
        let exposure = sky_exposure(eye, |x, y, z| terrain.block_at(x, y, z));
        if exposure > 0.05 {
            return Err(io::Error::other(format!(
                "{label} diagnostic room is not roofed underground"
            ))
            .into());
        }
        let eye_block = terrain.block_at(
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );
        let mut lit_stats = None;
        for headlamp in [true, false] {
            check_deadline(deadline)?;
            let hud = build_hud(
                width,
                height,
                &HudInfo {
                    captured: true,
                    flying: false,
                    swimming: false,
                    phasing: false,
                    biome: biome.name(),
                    headlamp,
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
                render_distance: radius,
                underwater: eye_block == Block::Water,
                water: world.water_at(
                    eye.x.floor() as i32,
                    eye.y.floor() as i32,
                    eye.z.floor() as i32,
                ),
                in_lava: eye_block == Block::Lava,
                sky_exposure: exposure,
                headlamp,
            };
            let suffix = if headlamp { "" } else { "-dark" };
            let stats = capture_geometry_scene(
                renderer,
                capture,
                &format!("cave-{label}{suffix}"),
                &frame,
            )?;
            check_deadline(deadline)?;
            if headlamp {
                lit_stats = Some(stats);
            } else if let Some(lit) = lit_stats {
                verify_lamp_readback(lit, stats, label)?;
            }
            println!(
                "Cave readback: {label}, headlamp={headlamp}, scene luminance={:.3}",
                stats.mean_luminance
            );
        }
        check_deadline(deadline)?;
        renderer.verify_gpu()?;
        check_deadline(deadline)?;
        println!(
            "Cave GPU verified: {label}, feet {:?}, headlamp on/off",
            player.position.to_array()
        );
    }
    check_deadline(deadline)?;
    if let Some(position) = terrain.entrance_position().map(Vec3::from_array) {
        let mut player = Player::new(position + Vec3::Y * 1.5);
        player.flying = true;
        player.yaw = entrance_yaw(terrain, player.eye_position());
        player.pitch = -0.45;
        let center = ChunkCoord::from_world(position.x, position.z);
        let radius = 4;
        let world = load_scene(renderer, terrain, center, radius, deadline)?;
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying: true,
                swimming: false,
                phasing: false,
                biome: "Cave Entrance",
                headlamp: true,
                underground: false,
                position: player.position.to_array(),
                fps: 60.0,
                loaded_chunks: 81,
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
            render_distance: radius,
            underwater: false,
            water: None,
            in_lava: false,
            sky_exposure: 1.0,
            headlamp: true,
        };
        renderer.render_offscreen(&frame)?;
        if let Some(path) = capture {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            renderer.save_screenshot(
                &path.with_file_name(format!("{stem}-cave-entrance.png")),
                &frame,
            )?;
        }
        check_deadline(deadline)?;
        println!(
            "Cave GPU verified: natural entrance, feet {:?}",
            position.to_array()
        );
    } else {
        return Err(io::Error::other("no natural cave entrance found for verification").into());
    }
    for (liquid, label) in [(Block::Water, "water"), (Block::Lava, "lava")] {
        check_deadline(deadline)?;
        let position = liquid_view_position(terrain, liquid, deadline)?.ok_or_else(|| {
            io::Error::other(format!("no exposed underground {label} pool found"))
        })?;
        let mut player = Player::new(position);
        player.flying = true;
        player.pitch = -1.15;
        let center = ChunkCoord::from_world(position.x, position.z);
        let radius = 2;
        let world = load_scene(renderer, terrain, center, radius, deadline)?;
        let eye = player.eye_position();
        let biome = terrain.cave_biome(
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying: true,
                swimming: false,
                phasing: false,
                biome: biome.name(),
                headlamp: true,
                underground: true,
                position: position.to_array(),
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
            render_distance: radius,
            underwater: false,
            water: None,
            in_lava: false,
            headlamp: true,
            sky_exposure: sky_exposure(eye, |x, y, z| terrain.block_at(x, y, z)),
        };
        renderer.render_offscreen(&frame)?;
        if let Some(path) = capture {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            renderer.save_screenshot(
                &path.with_file_name(format!("{stem}-cave-{label}.png")),
                &frame,
            )?;
        }
        check_deadline(deadline)?;
        println!(
            "Cave GPU verified: underground {label} pool, camera feet {:?}",
            position.to_array()
        );
    }
    verify_landforms(renderer, terrain, capture, deadline)?;
    verify_phasing(renderer, terrain, capture, deadline)?;
    Ok(())
}

pub(super) fn verify_landforms(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    for (landform, label) in [
        (CaveLandform::Gallery, "gallery"),
        (CaveLandform::Rift, "rift"),
        (CaveLandform::Cathedral, "cathedral"),
        (CaveLandform::Labyrinth, "labyrinth"),
    ] {
        check_deadline(deadline)?;
        let site = terrain
            .cave_landmark(landform)
            .ok_or_else(|| io::Error::other(format!("no nearby {} landmark", landform.name())))?;
        let mut player = verify_cave_pose(terrain, Vec3::from_array(site.position), label)?;
        aim_at(&mut player, Vec3::from_array(site.target));
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(player.position.x, player.position.z),
            3,
            deadline,
        )?;
        let hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                flying: false,
                swimming: false,
                phasing: false,
                biome: landform.name(),
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
        let stats =
            capture_geometry_scene(renderer, capture, &format!("landform-{label}"), &frame)?;
        if stats.visible_fraction < 0.02 {
            return Err(io::Error::other(format!(
                "{label} landmark is not visible in GPU readback"
            ))
            .into());
        }
        check_deadline(deadline)?;
        if landform == CaveLandform::Rift || landform == CaveLandform::Cathedral {
            let overhead_eye = player.eye_position() + Vec3::Y * 4.0;
            let overhead_block = terrain.block_at(
                overhead_eye.x.floor() as i32,
                overhead_eye.y.floor() as i32,
                overhead_eye.z.floor() as i32,
            );
            if overhead_block == Block::Air {
                player.position.y += 4.0;
                player.flying = true;
                aim_at(&mut player, Vec3::from_array(site.target) + Vec3::Y * 4.0);
                let scenic_frame = FrameState {
                    survival: None,
                    world: &world,
                    player: &player,
                    hud: &[],
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
                capture_scene(
                    renderer,
                    capture,
                    &format!("landform-{label}-flight"),
                    &scenic_frame,
                )?;
                check_deadline(deadline)?;
            }
        }
        println!(
            "Landform GPU verified: {label}, feet {:?}, scene luminance {:.3}",
            site.position, stats.mean_luminance
        );
    }
    Ok(())
}

pub(super) fn nearby_visible_growth(terrain: Terrain, eye: Vec3, material: Block) -> Option<Vec3> {
    let cell = eye.floor().as_ivec3().to_array();
    let mut columns = std::collections::HashMap::new();
    let mut sample = |[x, y, z]: [i32; 3]| {
        if !(MIN_WORLD_Y..crate::world::WORLD_HEIGHT).contains(&y) {
            return Block::Air;
        }
        let blocks = columns
            .entry((x, z))
            .or_insert_with(|| terrain.column_blocks(x, z));
        blocks[(y - MIN_WORLD_Y) as usize]
    };
    let mut targets = Vec::new();
    for z in cell[2] - 18..=cell[2] + 18 {
        for x in cell[0] - 18..=cell[0] + 18 {
            for y in cell[1] - 6..=cell[1] + 14 {
                if sample([x, y, z]) != material {
                    continue;
                }
                let target = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let distance = eye.distance_squared(target);
                if (2.0_f32.powi(2)..=22.0_f32.powi(2)).contains(&distance) {
                    targets.push((distance, target));
                }
            }
        }
    }
    targets.sort_by(|a, b| a.0.total_cmp(&b.0));
    targets.into_iter().map(|(_, point)| point).find(|point| {
        let delta = *point - eye;
        let count = (delta.length() * 6.0).ceil() as usize;
        let target_cell = point.floor().as_ivec3().to_array();
        (0..count).all(|step| {
            let position = (eye + delta * (step as f32 / count as f32))
                .floor()
                .as_ivec3()
                .to_array();
            position == target_cell || !sample(position).is_opaque()
        })
    })
}

pub(super) fn entrance_yaw(terrain: Terrain, eye: Vec3) -> f32 {
    let mut best = (0.0, 0.0);
    for index in 0..32 {
        let yaw = index as f32 * std::f32::consts::TAU / 32.0;
        let direction = Vec3::new(yaw.sin(), -0.5, -yaw.cos());
        let mut clear = 0.0;
        for step in 1..=16 {
            let point = eye + direction * step as f32;
            if terrain
                .block_at(
                    point.x.floor() as i32,
                    point.y.floor() as i32,
                    point.z.floor() as i32,
                )
                .is_solid()
            {
                break;
            }
            clear += 1.0;
        }
        if clear > best.1 {
            best = (yaw, clear);
        }
    }
    best.0
}

pub(super) fn liquid_view_position(
    terrain: Terrain,
    liquid: Block,
    deadline: Instant,
) -> Result<Option<Vec3>, io::Error> {
    let spawn = terrain.spawn_position();
    let center = ChunkCoord::from_world(spawn[0], spawn[2]);
    let mut coords: Vec<_> = (-8..=8)
        .flat_map(|z| (-8..=8).map(move |x| (x, z)))
        .collect();
    coords.sort_by_key(|(x, z)| x * x + z * z);
    for (dx, dz) in coords {
        check_deadline(deadline)?;
        let coord = ChunkCoord::new(center.x + dx, center.z + dz);
        let chunk = Chunk::generate(&terrain, coord);
        for y in MIN_WORLD_Y + 4..-4 {
            for z in 2..CHUNK_SIZE - 2 {
                for x in 2..CHUNK_SIZE - 2 {
                    if chunk.block(x, y, z) != liquid || chunk.block(x, y + 1, z) != Block::Air {
                        continue;
                    }
                    let pool = (-1..=1)
                        .all(|dz| (-1..=1).all(|dx| chunk.block(x + dx, y, z + dz) == liquid));
                    let clear = (1..=6).all(|dy| {
                        (-1..=1).all(|dz| {
                            (-1..=1).all(|dx| chunk.block(x + dx, y + dy, z + dz) == Block::Air)
                        })
                    });
                    if pool && clear {
                        return Ok(Some(Vec3::new(
                            (coord.x * CHUNK_SIZE + x) as f32 + 0.5,
                            y as f32 + 3.0,
                            (coord.z * CHUNK_SIZE + z) as f32 + 0.5,
                        )));
                    }
                }
            }
        }
    }
    Ok(None)
}

pub(super) fn verify_lamp_readback(
    lit: crate::renderer::FrameStats,
    dark: crate::renderer::FrameStats,
    label: &str,
) -> Result<(), io::Error> {
    if lit.visible_fraction < 0.02 {
        return Err(io::Error::other(format!(
            "{label} headlamp frame is nearly black"
        )));
    }
    if dark.mean_luminance >= lit.mean_luminance {
        return Err(io::Error::other(format!(
            "{label} disabling the headlamp did not darken the frame"
        )));
    }
    Ok(())
}

pub(super) fn verify_cave_pose(
    terrain: Terrain,
    position: Vec3,
    label: &str,
) -> Result<Player, io::Error> {
    let mut player = Player::new(position);
    let solid = |x, y, z| terrain.block_at(x, y, z).is_solid();
    for _ in 0..10 {
        player.update(MovementInput::default(), 1.0 / 60.0, solid);
    }
    if !player.is_grounded() || (player.position.y - position.y).abs() > 0.1 {
        return Err(io::Error::other(format!(
            "{label} cave pose has no dry support"
        )));
    }
    let ground = player.position.y;
    let mut apex = ground;
    for step in 0..100 {
        player.update(
            MovementInput {
                jump: step == 0,
                ..Default::default()
            },
            1.0 / 60.0,
            solid,
        );
        apex = apex.max(player.position.y);
    }
    if apex < ground + 0.5 || !player.is_grounded() || (player.position.y - ground).abs() > 0.1 {
        return Err(io::Error::other(format!(
            "{label} cave does not support jumping and landing"
        )));
    }
    Ok(player)
}

pub(super) fn scenic_yaw(terrain: &Terrain, eye: Vec3) -> f32 {
    let mut best = (0.0, 0);
    for index in 0..16 {
        let yaw = index as f32 * std::f32::consts::TAU / 16.0;
        let direction = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        let mut clear = 0;
        for step in 1..=24 {
            let point = eye + direction * step as f32;
            if terrain
                .block_at(
                    point.x.floor() as i32,
                    point.y.floor() as i32,
                    point.z.floor() as i32,
                )
                .is_opaque()
            {
                break;
            }
            clear = step;
        }
        if clear > best.1 {
            best = (yaw, clear);
        }
    }
    best.0
}
