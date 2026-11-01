//! Bounded readback of generated mainland views; diagnostic placement is not a route.

use super::{
    Error, FrameState, Instant, Path, Player, Renderer, Terrain, Vec3, aim_at, capture_scene,
    check_deadline, io, load_scene,
};
use crate::renderer::FrameStats;
use crate::world::{ChunkCoord, CoastalLandmark, WorldView};

mod persistence;

pub(crate) fn verify_coastal(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let site = terrain
        .coastal_landmark()
        .ok_or_else(|| io::Error::other("early coast has no supported land view/cove"))?;
    let stack = site
        .stacks
        .get(1)
        .ok_or_else(|| io::Error::other("coast lacks detached stacks"))?;
    for (label, feet, radius) in [
        ("coast-land", site.viewpoint, 8),
        ("coast-cove", site.cove, 8),
        ("coast-distance-2", site.cove, 2),
        ("coast-distance-12", site.viewpoint, 12),
    ] {
        check_deadline(deadline)?;
        let mut player = Player::new(Vec3::from_array(feet));
        aim_at(
            &mut player,
            Vec3::new(
                stack.center[0] as f32 + 0.5,
                (stack.top + 18) as f32 * 0.5,
                stack.center[1] as f32 + 0.5,
            ),
        );
        let world = load_scene(
            renderer,
            terrain,
            ChunkCoord::from_world(player.position.x, player.position.z),
            radius,
            deadline,
        )?;
        let frame = FrameState {
            player: &player,
            world: &world,
            hud: &[],
            elapsed: 2.2,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: Some(2.2),
            render_distance: radius,
            underwater: false,
            water: None,
            sky_exposure: 1.0,
            headlamp: false,
            in_lava: false,
            survival: None,
        };
        let stats = capture_scene(renderer, capture, label, &frame)?;
        verify_coastal_readback(stats)?;
        if native && !renderer.render(&frame)? {
            return Err(io::Error::other("coastal completion frame was not presented").into());
        }
        println!(
            "Coastal GPU verified: {label}, supported feet {feet:?}, {} main stacks, scene contrast {:.3}",
            site.stacks.len(),
            stats.luminance_deviation
        );
        if label == "coast-land" {
            verify_frame_cost(renderer, &frame, deadline)?;
        }
    }
    verify_arch(renderer, terrain, &site, capture, native, deadline)?;
    verify_stack_foundation(renderer, terrain, &site, capture, native, deadline)?;
    persistence::verify(renderer, terrain, &site, capture, native, deadline)?;
    Ok(())
}

fn verify_stack_foundation(
    renderer: &mut Renderer,
    terrain: Terrain,
    site: &CoastalLandmark,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let body = site.stacks[0];
    let offset = (Vec3::from_array(site.cove)
        - Vec3::new(body.center[0] as f32, 18.0, body.center[1] as f32))
    .with_y(0.0)
    .normalize_or_zero();
    let mut player = Player::new(
        Vec3::new(
            body.center[0] as f32 + 0.5,
            18.0,
            body.center[1] as f32 + 0.5,
        ) + offset * 13.0,
    );
    let [x, z] = [
        player.position.x.floor() as i32,
        player.position.z.floor() as i32,
    ];
    let water = terrain
        .water_column(x, z)
        .ok_or_else(|| io::Error::other("stack foundation camera has no marine water"))?;
    player.position.y = (water.bottom + 2) as f32;
    aim_at(
        &mut player,
        Vec3::new(
            body.center[0] as f32 + 0.5,
            (water.bottom + 4) as f32,
            body.center[1] as f32 + 0.5,
        ),
    );
    let world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(player.position.x, player.position.z),
        4,
        deadline,
    )?;
    let [x, y, z] = player.eye_position().floor().as_ivec3().to_array();
    let sample = world
        .water_at(x, y, z)
        .ok_or_else(|| io::Error::other("foundation camera is not in resident water"))?;
    let frame = FrameState {
        player: &player,
        world: &world,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: Some(2.2),
        render_distance: 4,
        underwater: true,
        water: Some(sample),
        sky_exposure: 1.0,
        headlamp: false,
        in_lava: false,
    };
    let stats = capture_scene(renderer, capture, "coast-stack-foundation", &frame)?;
    super::water::verify_water_scene(stats, "stack foundation")?;
    if native && !renderer.render(&frame)? {
        return Err(io::Error::other("stack foundation completion frame not presented").into());
    }
    println!(
        "Coastal GPU verified: stack foundation, center {:?}, bed {}, salinity {}, contrast {:.3}",
        body.center, sample.bottom, sample.salinity, stats.luminance_deviation
    );
    Ok(())
}

fn verify_frame_cost(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut times = Vec::with_capacity(32);
    for _ in 0..32 {
        check_deadline(deadline)?;
        let start = Instant::now();
        renderer.render_offscreen(frame)?;
        times.push(start.elapsed().as_secs_f64() * 1_000.0);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "Coastal GPU frame cost: median {:.2} ms, p95 {:.2} ms (32 fixed-pose land render + completion samples)",
        times[16], times[30]
    );
    Ok(())
}

fn verify_arch(
    renderer: &mut Renderer,
    terrain: Terrain,
    site: &CoastalLandmark,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let opening = Vec3::from_array(site.arch.opening);
    let shore_view = Vec3::from_array(site.arch.view);
    let direction = (opening - shore_view).with_y(0.0).normalize_or_zero();
    let water = terrain
        .water_column(opening.x.floor() as i32, opening.z.floor() as i32)
        .ok_or_else(|| io::Error::other("arch aperture lost its marine seabed"))?;
    let world = load_scene(
        renderer,
        terrain,
        ChunkCoord::from_world(opening.x, opening.z),
        5,
        deadline,
    )?;
    for (label, feet, target, underwater) in [
        ("coast-arch", shore_view, opening + Vec3::Y * 3.0, false),
        (
            "coast-arch-underside",
            opening - Vec3::Y * crate::player::EYE_HEIGHT,
            opening + direction * 8.0 + Vec3::Y * 6.0,
            false,
        ),
        (
            "coast-arch-marine",
            Vec3::new(opening.x, (water.bottom + 2) as f32, opening.z),
            opening + direction * 8.0 - Vec3::Y * 8.0,
            true,
        ),
    ] {
        check_deadline(deadline)?;
        let mut player = Player::new(feet);
        aim_at(&mut player, target);
        let [x, y, z] = player.eye_position().floor().as_ivec3().to_array();
        if world
            .block_at(x, y, z)
            .is_some_and(|block| block.is_solid())
        {
            return Err(io::Error::other(format!("{label} camera is inside rock")).into());
        }
        let frame = FrameState {
            player: &player,
            world: &world,
            hud: &[],
            elapsed: 2.2,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
            water_time: Some(2.2),
            render_distance: 5,
            underwater,
            water: if underwater {
                world.water_at(x, y, z)
            } else {
                None
            },
            sky_exposure: 1.0,
            headlamp: false,
            in_lava: false,
            survival: None,
        };
        let stats = capture_scene(renderer, capture, label, &frame)?;
        if underwater {
            super::water::verify_water_scene(stats, label)?;
        } else {
            verify_coastal_readback(stats)?;
        }
        if native && !renderer.render(&frame)? {
            return Err(io::Error::other("arch completion frame was not presented").into());
        }
        println!(
            "Coastal GPU verified: {label}, aperture {:?}, roof {:?}, contrast {:.3}",
            site.arch.opening, site.arch.roof, stats.luminance_deviation
        );
    }
    Ok(())
}

fn verify_coastal_readback(stats: FrameStats) -> io::Result<()> {
    if !stats.mean_luminance.is_finite()
        || !stats.luminance_deviation.is_finite()
        || !stats.visible_fraction.is_finite()
        || stats.mean_luminance < 0.08
        || stats.luminance_deviation < 0.03
        || stats.visible_fraction < 0.25
    {
        return Err(io::Error::other(
            "coast readback has no visible, contrasting outdoor geometry",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coastal_readback_rejects_flat_dark_empty_and_invalid_frames() {
        let visible = FrameStats {
            mean_luminance: 0.4,
            luminance_deviation: 0.1,
            visible_fraction: 0.9,
        };
        assert!(verify_coastal_readback(visible).is_ok());
        for stats in [
            FrameStats {
                mean_luminance: 0.0,
                ..visible
            },
            FrameStats {
                luminance_deviation: 0.0,
                ..visible
            },
            FrameStats {
                visible_fraction: 0.0,
                ..visible
            },
            FrameStats {
                mean_luminance: f32::NAN,
                ..visible
            },
            FrameStats {
                luminance_deviation: f32::NAN,
                ..visible
            },
            FrameStats {
                visible_fraction: f32::NAN,
                ..visible
            },
        ] {
            assert!(verify_coastal_readback(stats).is_err());
        }
    }
}
