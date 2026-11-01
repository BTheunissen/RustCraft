//! Resident-world movement and bounded aquatic frame cost checks.

use super::{Error, FrameState, Instant, Player, Renderer, Vec3, check_deadline, io};
use crate::player::MovementInput;
use crate::world::{Block, ResidentWorld, WorldView};

pub(super) fn verify_swimming_scene(
    world: &ResidentWorld,
    position: Vec3,
    deadline: Instant,
) -> Result<(), io::Error> {
    let solid = |x, y, z| world.block_at(x, y, z).is_none_or(Block::is_solid);
    let water = |x, y, z| world.water_at(x, y, z);
    let mut player = Player::new(position);
    for _ in 0..45 {
        player.update_with_water(MovementInput::default(), 1.0 / 60.0, solid, water);
        check_deadline(deadline)?;
    }
    if !player.is_swimming() {
        return Err(io::Error::other("generated lake did not activate swimming"));
    }
    let starting_y = player.position.y;
    for _ in 0..45 {
        player.update_with_water(
            MovementInput {
                descend: true,
                ..Default::default()
            },
            1.0 / 60.0,
            solid,
            water,
        );
    }
    if player.position.y >= starting_y - 0.2 {
        return Err(io::Error::other(
            "Shift did not dive in generated freshwater",
        ));
    }
    let submerged_y = player.position.y;
    for _ in 0..90 {
        player.update_with_water(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            1.0 / 60.0,
            solid,
            water,
        );
        check_deadline(deadline)?;
    }
    if player.position.y <= submerged_y + 0.5 {
        return Err(io::Error::other(
            "Space did not raise the swimmer toward the surface",
        ));
    }
    player.toggle_flight(solid);
    let before = player.position;
    let queried_water = std::cell::Cell::new(false);
    player.update_with_water(MovementInput::default(), 0.2, solid, |_, _, _| {
        queried_water.set(true);
        None
    });
    if !player.flying || player.position != before || queried_water.get() {
        return Err(io::Error::other(
            "creative flight did not ignore water forces",
        ));
    }
    println!(
        "Water movement verified: freshwater immersion, diving, ascent, and unaffected creative flight"
    );
    Ok(())
}

pub(super) fn verify_current_scene(
    world: &ResidentWorld,
    sample: crate::world::WaterSample,
    cell: [i32; 3],
    deadline: Instant,
) -> Result<(), io::Error> {
    let direction = Vec3::from_array(sample.flow)
        .with_y(0.0)
        .normalize_or_zero();
    if direction.length_squared() < 0.9 {
        return Err(io::Error::other(
            "generated river has no horizontal current",
        ));
    }
    let feet_y = (sample.surface - 1.3).max((sample.bottom + 1) as f32 + 0.05);
    let start = Vec3::new(cell[0] as f32 + 0.5, feet_y, cell[2] as f32 + 0.5);
    let solid = |x, y, z| world.block_at(x, y, z).is_none_or(Block::is_solid);
    let water = |x, y, z| world.water_at(x, y, z);
    let mut drifting = Player::new(start);
    for _ in 0..30 {
        drifting.update_with_water(MovementInput::default(), 1.0 / 60.0, solid, water);
        check_deadline(deadline)?;
    }
    let carried = (drifting.position - start).dot(direction);
    if carried < 0.04 {
        return Err(io::Error::other(format!(
            "generated river current did not carry a swimmer downstream: {carried:.3}"
        )));
    }
    println!(
        "Water current verified: swimmer carried {carried:.2} blocks downstream in 0.5 seconds"
    );
    Ok(())
}

pub(super) fn verify_water_frame_cost(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut times = Vec::with_capacity(32);
    for step in 0..32 {
        let sample = FrameState {
            hud: &[],
            water_time: Some(2.2 + step as f32 * 0.04),
            ..*frame
        };
        let started = Instant::now();
        renderer.render_offscreen(&sample)?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        check_deadline(deadline)?;
    }
    times.sort_by(f64::total_cmp);
    println!(
        "Water GPU frame cost: median {:.2} ms, p95 {:.2} ms (32 ocean render + completion samples)",
        times[times.len() / 2],
        times[times.len() * 95 / 100]
    );
    Ok(())
}
