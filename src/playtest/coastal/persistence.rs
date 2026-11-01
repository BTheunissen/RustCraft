//! Edited coastal GPU/save round trips use an exclusively owned temporary slot.

use super::*;
use crate::game::{Environment, Game, GameMode};
use crate::mesh::mesh_chunk_with_edits;
use crate::renderer::{FrameDifference, SurvivalFrame};
use crate::save::SaveSlot;
use crate::world::{Block, Chunk, SampledWorld};

pub(super) fn verify(
    renderer: &mut Renderer,
    terrain: Terrain,
    site: &CoastalLandmark,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut game = Game::with_mode(terrain.seed(), GameMode::Survival);
    game.player.respawn(Vec3::from_array(site.arch.view));
    game.headlamp = false;
    let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
    game.world = load_scene(renderer, terrain, center, 3, deadline)?;
    let source = source(&game)?;
    aim_at(
        &mut game.player,
        Vec3::from_array(source.map(|value| value as f32)) + Vec3::splat(0.5),
    );
    game.inventory_mut().insert(Block::Wood, 3);
    let mut original = Game::from_snapshot(game.snapshot()).map_err(io::Error::other)?;
    for dz in -3..=3 {
        for dx in -3..=3 {
            check_deadline(deadline)?;
            original.world.insert(Chunk::generate(
                &terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    let source_material = game
        .world
        .block_at(source[0], source[1], source[2])
        .ok_or_else(|| io::Error::other("missing coastal source"))?;
    mine(&mut game, source, deadline)?;
    let before = original.environment();
    let after = game.environment();
    let difference = renderer.compare_rebuilt_frames(
        &frame(&original, before),
        &frame(&game, after),
        |renderer| upload(renderer, &game, center, deadline),
    )?;
    visible_edit(difference)?;
    capture_scene(renderer, capture, "coast-edited", &frame(&game, after))?;
    game.inventory_mut().click_slot(0);
    let progress = game.snapshot();
    let scratch = super::super::persistence::ScratchDirectory::new()?;
    let slot = SaveSlot::new(scratch.0.join("world.save"));
    slot.write(&progress)?;
    let mut restored = Game::from_snapshot(
        slot.load()?
            .ok_or_else(|| io::Error::other("coastal smoke slot disappeared"))?,
    )
    .map_err(io::Error::other)?;
    if restored.snapshot() != progress
        || restored.inventory().cursor_stack().is_some()
        || restored.pickups().len() != 1
        || restored.pickups()[0].item != source_material.into()
        || SampledWorld::new(&restored.world, &terrain).block_at(source[0], source[1], source[2])
            != Some(Block::Air)
    {
        return Err(io::Error::other(
            "coastal save lost edits, inventory, cursor, pickups, or pose",
        )
        .into());
    }
    for dz in -3..=3 {
        for dx in -3..=3 {
            check_deadline(deadline)?;
            restored.world.insert(Chunk::generate(
                &terrain,
                ChunkCoord::new(center.x + dx, center.z + dz),
            ));
        }
    }
    let after_restore = restored.environment();
    let difference = renderer.compare_rebuilt_frames(
        &frame(&game, after),
        &frame(&restored, after_restore),
        |renderer| upload(renderer, &restored, center, deadline),
    )?;
    super::super::persistence::unchanged_difference(difference)?;
    let source_coord = ChunkCoord::from_world(source[0] as f32, source[2] as f32);
    restored.world.remove(source_coord);
    restored
        .world
        .insert(Chunk::generate(&terrain, source_coord));
    if restored.world.block_at(source[0], source[1], source[2]) != Some(Block::Air)
        || restored.snapshot() != progress
    {
        return Err(
            io::Error::other("coastal edited chunk reload changed committed progress").into(),
        );
    }
    capture_scene(
        renderer,
        capture,
        "coast-resumed",
        &frame(&restored, after_restore),
    )?;
    if native && !renderer.render(&frame(&restored, after_restore))? {
        return Err(io::Error::other("resumed coastal frame was not presented").into());
    }
    println!(
        "Coastal persistence GPU verified: ordinary mining, changed remesh, private-slot edit/inventory/cursor/drop/pose round trip, exact same-pose resumed pixels, and edited eviction/reload"
    );
    Ok(())
}

fn source(game: &Game) -> io::Result<[i32; 3]> {
    let feet = game.player.position.floor().as_ivec3().to_array();
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let point = [feet[0] + dx, feet[1] - 1, feet[2] + dz];
        if game
            .world
            .block_at(point[0], point[1], point[2])
            .is_some_and(|block| block.is_solid() && block != Block::Bedrock)
            && game.world.block_at(point[0], point[1] + 1, point[2]) == Some(Block::Air)
        {
            return Ok(point);
        }
    }
    Err(io::Error::other(
        "no loaded ordinary coastal mining fixture",
    ))
}

fn mine(game: &mut Game, source: [i32; 3], deadline: Instant) -> io::Result<()> {
    game.interact(false, false, 0.0);
    if game.target().is_none_or(|target| target.position != source) {
        return Err(io::Error::other("coastal mining target is not reachable"));
    }
    for _ in 0..30 {
        check_deadline(deadline)?;
        game.interact(true, false, 0.1);
        if game.world.block_at(source[0], source[1], source[2]) == Some(Block::Air) {
            break;
        }
    }
    game.cancel_mining();
    if game.world.block_at(source[0], source[1], source[2]) != Some(Block::Air)
        || game.pickups().len() != 1
        || game.pickups()[0].count != 1
    {
        return Err(io::Error::other(
            "coastal mining did not produce one ordinary drop",
        ));
    }
    Ok(())
}

fn upload(
    renderer: &mut Renderer,
    game: &Game,
    center: ChunkCoord,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    renderer.clear_chunks();
    for dz in -3..=3 {
        for dx in -3..=3 {
            check_deadline(deadline)?;
            let coord = ChunkCoord::new(center.x + dx, center.z + dz);
            let chunk = game
                .world
                .chunk(coord)
                .ok_or_else(|| io::Error::other("coastal remesh lost residency"))?;
            renderer.upload_chunk(
                coord,
                &mesh_chunk_with_edits(chunk, &game.terrain, game.world.edits()),
            );
        }
    }
    Ok(())
}

fn frame(game: &Game, environment: Environment) -> FrameState<'_> {
    FrameState {
        player: &game.player,
        world: &game.world,
        hud: &[],
        elapsed: 2.2,
        time_of_day: game.time().hour(),
        water_time: Some(2.2),
        render_distance: 3,
        underwater: environment.underwater,
        water: environment.water,
        sky_exposure: environment.sky_exposure,
        headlamp: false,
        in_lava: false,
        survival: Some(SurvivalFrame {
            pickups: game.pickups(),
            held: None,
            target: None,
            mining_progress: 0.0,
            show_hand: false,
        }),
    }
}

fn visible_edit(difference: FrameDifference) -> io::Result<()> {
    if !difference.changed_fraction.is_finite()
        || !difference.mean_absolute_error.is_finite()
        || difference.changed_fraction < 0.0001
        || difference.mean_absolute_error < 0.000001
    {
        return Err(io::Error::other(
            "coastal mining/remesh did not change actual GPU pixels",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
