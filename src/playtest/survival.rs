//! Real generated-world mining, pickup, inventory, placement, and UI readback checks.

use super::{aim_at, capture_scene, check_deadline, io, load_scene};
use crate::game::{Game, GameMode};
use crate::hud::{HudInfo, HudVertex, Screen, UiState, build_interface};
use crate::mesh::mesh_chunk_with_edits;
use crate::player::SpeedMultiplier;
use crate::renderer::{FrameDifference, FrameState, Renderer, SurvivalFrame};
use crate::world::{Block, Chunk, ChunkCoord, SampledWorld, Terrain, WorldView};
use glam::Vec3;
use std::error::Error;
use std::path::Path;
use std::time::Instant;

pub(crate) fn verify_survival(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    check_deadline(deadline)?;
    let mut game = Game::with_mode(terrain.seed(), GameMode::Survival);
    let origin = game.player.position;
    let center = ChunkCoord::from_world(origin.x, origin.z);
    game.world = load_scene(renderer, terrain, center, 3, deadline)?;
    let source =
        mining_site(&game).ok_or_else(|| io::Error::other("no reachable survival mining site"))?;
    let original = game
        .world
        .block_at(source[0], source[1], source[2])
        .ok_or_else(|| io::Error::other("missing survival source residency"))?;
    aim_at(&mut game.player, block_center(source));
    game.interact(false, false, 0.0);
    require(
        game.target()
            .is_some_and(|target| target.position == source),
        "survival raycast missed the mining fixture",
    )?;
    game.toggle_flight();
    game.cycle_speed();
    require(
        !game.player.flying && game.speed() == SpeedMultiplier::Normal,
        "survival accepted a creative movement action",
    )?;

    verify_menu(renderer, &game, capture, native)?;
    save_state(
        renderer,
        &game,
        Screen::Playing,
        "survival-start",
        capture,
        native,
    )?;

    while game.mining_progress() < 0.52 {
        check_deadline(deadline)?;
        game.interact(true, false, 0.05);
        require(
            game.world.block_at(source[0], source[1], source[2]) == Some(original),
            "mining completed before progressive feedback",
        )?;
    }
    let cracked = frame(&game, &[], true);
    let cracked = FrameState {
        survival: cracked.survival.map(|survival| SurvivalFrame {
            show_hand: false,
            ..survival
        }),
        ..cracked
    };
    let no_cracks = FrameState {
        survival: cracked.survival.map(|survival| SurvivalFrame {
            mining_progress: 0.0,
            ..survival
        }),
        ..cracked
    };
    visible_difference(
        renderer.compare_full_frames(&no_cracks, &cracked)?,
        "mining cracks",
    )?;
    save_state(
        renderer,
        &game,
        Screen::Playing,
        "survival-mining",
        capture,
        native,
    )?;

    for _ in 0..60 {
        check_deadline(deadline)?;
        game.interact(true, false, 0.05);
        if game.world.block_at(source[0], source[1], source[2]) == Some(Block::Air) {
            break;
        }
    }
    game.cancel_mining();
    require(
        game.world.block_at(source[0], source[1], source[2]) == Some(Block::Air),
        "hand mining did not remove the block",
    )?;
    require(
        game.pickups().len() == 1
            && game.pickups()[0].item == original.into()
            && game.pickups()[0].count == 1,
        "mining did not create exactly one original-material drop",
    )?;

    let source_chunk = ChunkCoord::from_world(source[0] as f32, source[2] as f32);
    game.world.remove(source_chunk);
    require(
        SampledWorld::new(&game.world, &game.terrain).block_at(source[0], source[1], source[2])
            == Some(Block::Air),
        "unloaded procedural fallback forgot a mined block",
    )?;
    game.world.insert(Chunk::generate(&terrain, source_chunk));
    require(
        game.world.block_at(source[0], source[1], source[2]) == Some(Block::Air),
        "chunk reload restored a mined block",
    )?;
    upload_edits(renderer, &game, source_chunk, deadline)?;

    let pickup_frame = frame(&game, &[], false);
    let empty_frame = FrameState {
        survival: pickup_frame.survival.map(|survival| SurvivalFrame {
            pickups: &[],
            target: None,
            ..survival
        }),
        ..pickup_frame
    };
    let pickup_frame = FrameState {
        survival: pickup_frame.survival.map(|survival| SurvivalFrame {
            target: None,
            ..survival
        }),
        ..pickup_frame
    };
    visible_difference(
        renderer.compare_full_frames(&empty_frame, &pickup_frame)?,
        "dropped block cube",
    )?;
    save_state(
        renderer,
        &game,
        Screen::Playing,
        "survival-pickup",
        capture,
        native,
    )?;
    super::persistence::verify_round_trip(renderer, &game, source, capture, native, deadline)?;

    let drop = game.pickups()[0].position;
    game.player.position = Vec3::new(drop.x, source[1] as f32, drop.z);
    for _ in 0..20 {
        check_deadline(deadline)?;
        game.interact(false, false, 0.05);
    }
    require(
        game.pickups().is_empty(),
        "nearby dropped block was not collected",
    )?;
    require(
        game.inventory()
            .selected_stack()
            .is_some_and(|stack| stack.item == original.into() && stack.count == 1),
        "pickup did not enter the hotbar",
    )?;
    game.player.position = origin;
    aim_at(
        &mut game.player,
        block_center([source[0], source[1] - 1, source[2]]),
    );
    game.interact(false, false, 0.0);

    let held = frame(&game, &[], true);
    let bare = FrameState {
        survival: held.survival.map(|survival| SurvivalFrame {
            held: None,
            ..survival
        }),
        ..held
    };
    visible_difference(
        renderer.compare_full_frames(&bare, &held)?,
        "held textured block",
    )?;
    save_state(
        renderer,
        &game,
        Screen::Playing,
        "survival-held",
        capture,
        native,
    )?;
    save_state(
        renderer,
        &game,
        Screen::Inventory,
        "survival-inventory",
        capture,
        native,
    )?;
    game.inventory_mut().click_slot(0);
    game.inventory_mut().click_slot(12);
    game.inventory_mut().close_cursor();
    require(
        game.inventory().slots()[12]
            .is_some_and(|stack| stack.item == original.into() && stack.count == 1),
        "inventory transfer did not preserve the mined block",
    )?;
    game.inventory_mut().click_slot(12);
    game.inventory_mut().click_slot(0);
    game.inventory_mut().close_cursor();

    require(
        game.target().is_some_and(|target| {
            std::array::from_fn::<_, 3, _>(|axis| target.position[axis] + target.normal[axis])
                == source
        }),
        "placement target does not lead back to the mined cell",
    )?;
    game.interact(false, true, 0.0);
    require(
        game.world.block_at(source[0], source[1], source[2]) == Some(original),
        "placement did not restore the held block",
    )?;
    require(
        game.inventory().selected_stack().is_none(),
        "successful placement did not consume one block",
    )?;
    upload_edits(renderer, &game, source_chunk, deadline)?;
    save_state(
        renderer,
        &game,
        Screen::Playing,
        "survival-placed",
        capture,
        native,
    )?;
    super::items::verify(renderer, &mut game, capture, native, deadline)?;
    renderer.verify_gpu()?;
    check_deadline(deadline)?;
    println!(
        "Survival GPU verified: live menu, timed cracks, one drop, pickup, hotbar, held block, inventory, placement, and edited chunk reload"
    );
    Ok(())
}

fn verify_menu(
    renderer: &mut Renderer,
    game: &Game,
    capture: Option<&Path>,
    native: bool,
) -> Result<(), Box<dyn Error>> {
    let mut preview = crate::player::Player::new(Vec3::from_array(game.terrain.spawn_position()));
    preview.yaw = -0.55;
    preview.position.y += 8.0;
    preview.pitch = -0.42;
    let hud = interface(renderer, game, Screen::MainMenu);
    let menu = FrameState {
        player: &preview,
        hud: &hud,
        survival: None,
        ..frame(game, &[], false)
    };
    let plain = FrameState { hud: &[], ..menu };
    let scene = renderer.measure_frame(&plain)?;
    require(
        scene.visible_fraction > 0.05 && scene.luminance_deviation > 0.01,
        "live menu background contains no visible terrain",
    )?;
    let mut rotated = preview.clone();
    rotated.yaw += 0.1;
    let later = FrameState {
        player: &rotated,
        ..plain
    };
    visible_difference(
        renderer.compare_frames(&plain, &later)?,
        "moving live menu terrain",
    )?;
    visible_difference(
        renderer.compare_full_frames(&plain, &menu)?,
        "main menu overlay",
    )?;
    capture_scene(renderer, capture, "menu", &menu)?;
    present(renderer, &menu, native)
}

fn save_state(
    renderer: &mut Renderer,
    game: &Game,
    screen: Screen,
    suffix: &str,
    capture: Option<&Path>,
    native: bool,
) -> Result<(), Box<dyn Error>> {
    let hud = interface(renderer, game, screen);
    let frame = frame(game, &hud, screen == Screen::Playing);
    capture_scene(renderer, capture, suffix, &frame)?;
    present(renderer, &frame, native)
}

fn present(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    native: bool,
) -> Result<(), Box<dyn Error>> {
    if native {
        require(
            renderer.render(frame)?,
            "survival completion frame was not presented",
        )?;
    }
    Ok(())
}

pub(super) fn interface(renderer: &Renderer, game: &Game, screen: Screen) -> Vec<HudVertex> {
    let (width, height) = renderer.size();
    build_interface(
        width,
        height,
        &HudInfo {
            captured: screen == Screen::Playing,
            flying: false,
            swimming: false,
            phasing: false,
            headlamp: game.headlamp(),
            underground: false,
            biome: "SURVIVAL BUILDING",
            position: game.player.position.to_array(),
            fps: 60.0,
            loaded_chunks: game.world.len(),
            pending_chunks: 0,
            speed: SpeedMultiplier::Normal,
            time: game.time(),
        },
        &UiState {
            screen,
            mode: GameMode::Survival,
            inventory: Some(game.inventory()),
            storage: game.storage_view(),
            interaction_notice: game.interaction_notice(),
            cursor: None,
            focused_button: 0,
            can_continue: false,
            save_notice: None,
        },
    )
}

pub(super) fn frame<'a>(game: &'a Game, hud: &'a [HudVertex], show_hand: bool) -> FrameState<'a> {
    FrameState {
        world: &game.world,
        player: &game.player,
        hud,
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
            held: game.inventory().selected_stack().map(|stack| stack.item),
            target: if show_hand { game.target() } else { None },
            mining_progress: game.mining_progress(),
            show_hand,
        }),
    }
}

fn mining_site(game: &Game) -> Option<[i32; 3]> {
    let feet = game.player.position.floor().as_ivec3().to_array();
    for offset in [[1, 0], [0, -1], [-1, 0], [0, 1]] {
        for y in (feet[1] - 2..=feet[1]).rev() {
            let source = [feet[0] + offset[0], y, feet[2] + offset[1]];
            let solid = game.world.block_at(source[0], y, source[2]);
            if solid.is_some_and(|block| block.is_solid() && block != Block::Bedrock)
                && game.world.block_at(source[0], y + 1, source[2]) == Some(Block::Air)
                && game
                    .world
                    .block_at(source[0], y - 1, source[2])
                    .is_some_and(Block::is_solid)
            {
                return Some(source);
            }
        }
    }
    None
}

fn block_center(position: [i32; 3]) -> Vec3 {
    Vec3::from_array(position.map(|axis| axis as f32 + 0.5))
}

fn upload_edits(
    renderer: &mut Renderer,
    game: &Game,
    center: ChunkCoord,
    deadline: Instant,
) -> Result<(), io::Error> {
    for z in -1..=1 {
        for x in -1..=1 {
            check_deadline(deadline)?;
            let coord = ChunkCoord::new(center.x + x, center.z + z);
            if let Some(chunk) = game.world.chunk(coord) {
                renderer.upload_chunk(
                    coord,
                    &mesh_chunk_with_edits(chunk, &game.terrain, game.world.edits()),
                );
            }
        }
    }
    Ok(())
}

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}

fn visible_difference(difference: FrameDifference, label: &str) -> Result<(), io::Error> {
    require(
        difference.changed_fraction.is_finite()
            && difference.mean_absolute_error.is_finite()
            && difference.changed_fraction > 0.00001
            && difference.mean_absolute_error > 0.000001,
        &format!("{label} did not change real GPU pixels"),
    )
}

#[cfg(test)]
mod tests;
