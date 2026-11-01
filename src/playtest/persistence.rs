//! Bounded save/readback checks using a private slot, never the player's local world.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::game::{Game, GameMode};
use crate::hud::{HudInfo, SaveNotice, Screen, UiState, build_interface};
use crate::mesh::mesh_chunk_with_edits;
use crate::player::Player;
use crate::renderer::{FrameDifference, FrameState, Renderer, SurvivalFrame};
use crate::save::SaveSlot;
use crate::world::{Chunk, SampledWorld, WorldView};

use super::{capture_scene, check_deadline};

pub(super) fn verify_round_trip(
    renderer: &mut Renderer,
    game: &Game,
    source: [i32; 3],
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    check_deadline(deadline)?;
    let directory = ScratchDirectory::new()?;
    let slot = SaveSlot::new(directory.0.join("world.save"));
    let progress = game.snapshot();
    slot.write(&progress)?;
    let mut restored = Game::from_snapshot(
        slot.load()?
            .ok_or_else(|| io::Error::other("smoke save disappeared"))?,
    )
    .map_err(io::Error::other)?;
    if restored.snapshot() != progress
        || SampledWorld::new(restored.world(), &restored.terrain())
            .block_at(source[0], source[1], source[2])
            != Some(crate::world::Block::Air)
        || restored.pickups().len() != 1
    {
        return Err(io::Error::other("save/resume lost survival progress").into());
    }
    let center =
        crate::world::ChunkCoord::from_world(game.player.position.x, game.player.position.z);
    for z in -3..=3 {
        for x in -3..=3 {
            check_deadline(deadline)?;
            let coord = crate::world::ChunkCoord::new(center.x + x, center.z + z);
            restored
                .world
                .insert(Chunk::generate(&restored.terrain, coord));
        }
    }
    let before = frame(game);
    let after = frame(&restored);
    let difference = renderer.compare_rebuilt_frames(&before, &after, |renderer| {
        renderer.clear_chunks();
        for z in -3..=3 {
            for x in -3..=3 {
                check_deadline(deadline)?;
                let coord = crate::world::ChunkCoord::new(center.x + x, center.z + z);
                if let Some(chunk) = restored.world.chunk(coord) {
                    renderer.upload_chunk(
                        coord,
                        &mesh_chunk_with_edits(chunk, &restored.terrain, restored.world.edits()),
                    );
                }
            }
        }
        Ok(())
    })?;
    unchanged_difference(difference)?;
    capture_scene(renderer, capture, "survival-resumed", &after)?;
    if native && !renderer.render(&after)? {
        return Err(io::Error::other("resumed survival frame was not presented").into());
    }
    verify_menus(renderer, &restored, capture, native, deadline)?;
    check_deadline(deadline)?;
    println!(
        "Persistence GPU verified: private-slot round trip, edited terrain, dropped block, same-pose pixels, Continue, and replace confirmation"
    );
    Ok(())
}

pub(super) fn unchanged_difference(difference: FrameDifference) -> io::Result<()> {
    if !difference.changed_fraction.is_finite()
        || !difference.mean_absolute_error.is_finite()
        || difference.changed_fraction > 0.000_01
        || difference.mean_absolute_error > 0.000_001
    {
        return Err(io::Error::other(
            "resumed progress changed same-pose GPU pixels",
        ));
    }
    Ok(())
}

fn frame(game: &Game) -> FrameState<'_> {
    FrameState {
        player: &game.player,
        world: &game.world,
        hud: &[],
        elapsed: 1.0,
        time_of_day: game.time().hour(),
        water_time: Some(1.0),
        render_distance: 3,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        headlamp: game.headlamp(),
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

fn verify_menus(
    renderer: &mut Renderer,
    game: &Game,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut preview = Player::new(glam::Vec3::from_array(game.terrain().spawn_position()));
    preview.position.y += 8.0;
    preview.yaw = -0.55;
    preview.pitch = -0.42;
    let (width, height) = renderer.size();
    for (screen, label, notice) in [
        (Screen::MainMenu, "menu-continue", SaveNotice::Saved),
        (
            Screen::ConfirmReplace(GameMode::Survival),
            "replace-world",
            SaveNotice::Saved,
        ),
        (Screen::Paused, "save-failed", SaveNotice::SaveFailed),
        (
            Screen::MainMenu,
            "save-incompatible",
            SaveNotice::Incompatible,
        ),
    ] {
        check_deadline(deadline)?;
        let hud = build_interface(
            width,
            height,
            &HudInfo {
                captured: false,
                flying: false,
                swimming: false,
                phasing: false,
                headlamp: game.headlamp(),
                underground: false,
                biome: "Plains",
                position: preview.position.to_array(),
                fps: 60.0,
                loaded_chunks: game.world().len(),
                pending_chunks: 0,
                speed: game.speed(),
                time: game.time(),
            },
            &UiState {
                screen,
                mode: game.mode(),
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: usize::from(matches!(screen, Screen::ConfirmReplace(_))),
                can_continue: true,
                save_notice: Some(notice),
            },
        );
        let overlay = FrameState {
            player: &preview,
            hud: &hud,
            survival: None,
            ..frame(game)
        };
        capture_scene(renderer, capture, label, &overlay)?;
        if native && !renderer.render(&overlay)? {
            return Err(io::Error::other("persistence menu was not presented").into());
        }
    }
    Ok(())
}

pub(super) struct ScratchDirectory(pub(super) PathBuf);

impl ScratchDirectory {
    pub(super) fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        loop {
            let path = std::env::temp_dir().join(format!(
                "rustcraft-save-smoke-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests;
