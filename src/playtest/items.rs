//! Tagged-item GPU/save regression; no recipes exposed before their feature lands.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

use super::{capture_scene, check_deadline};
use crate::game::{Game, Tool};
use crate::hud::Screen;
use crate::renderer::{FrameState, Renderer, SurvivalFrame};
use crate::save::SaveSlot;
use crate::world::Block;

pub(super) fn verify(
    renderer: &mut Renderer,
    game: &mut Game,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    game.inventory_mut().close_cursor();
    verify_materials(renderer, game, capture, native, deadline)?;
    verify_crafting(renderer, game, capture, native, deadline)?;
    let mut baseline = None;
    for (tool, label) in [
        (Tool::StonePickaxe, "stone-pickaxe"),
        (Tool::StoneAxe, "stone-axe"),
    ] {
        check_deadline(deadline)?;
        if !game
            .inventory()
            .slots()
            .iter()
            .flatten()
            .any(|stack| stack.item == tool.into())
            && game.inventory_mut().insert(tool, 1) != 0
        {
            return Err(io::Error::other("item GPU fixture has no tool capacity").into());
        }
        let index = game
            .inventory()
            .slots()
            .iter()
            .position(|slot| slot.is_some_and(|stack| stack.item == tool.into()))
            .ok_or_else(|| io::Error::other("tool was not inserted"))?;
        if index >= 9 {
            return Err(io::Error::other("item fixture tool is outside hotbar").into());
        }
        game.inventory_mut().select(index);
        let frame = super::survival::frame(game, &[], true);
        let handless = FrameState {
            survival: frame.survival.map(|survival| SurvivalFrame {
                show_hand: false,
                ..survival
            }),
            ..frame
        };
        if renderer
            .compare_full_frames(&handless, &frame)?
            .changed_fraction
            < 0.001
        {
            return Err(io::Error::other("held tool did not change actual GPU pixels").into());
        }
        if let Some(previous) = baseline {
            let previous = FrameState {
                survival: frame.survival.map(|survival| SurvivalFrame {
                    held: Some(previous),
                    ..survival
                }),
                ..frame
            };
            if renderer
                .compare_full_frames(&previous, &frame)?
                .changed_fraction
                < 0.0001
            {
                return Err(io::Error::other("tool silhouettes are not visually distinct").into());
            }
        }
        baseline = Some(tool.into());
        capture_scene(renderer, capture, label, &frame)?;
        if native && !renderer.render(&frame)? {
            return Err(io::Error::other("tool completion frame not presented").into());
        }
    }
    game.inventory_mut().insert(Block::Wood, 7);
    let index = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Block::Wood.into()))
        .ok_or_else(|| io::Error::other("no split stack"))?;
    game.inventory_mut().right_click_slot(index);
    let before = game.snapshot();
    let directory = super::persistence::ScratchDirectory::new()?;
    let slot = SaveSlot::new(directory.0.join("world.save"));
    slot.write(&before)?;
    let restored = Game::from_snapshot(
        slot.load()?
            .ok_or_else(|| io::Error::other("item save disappeared"))?,
    )
    .map_err(io::Error::other)?;
    if restored.snapshot() != before {
        return Err(io::Error::other("split/tool save round trip changed items").into());
    }
    let hud = super::survival::interface(renderer, game, Screen::Inventory);
    capture_scene(
        renderer,
        capture,
        "item-split-inventory",
        &super::survival::frame(game, &hud, false),
    )?;
    game.inventory_mut().close_cursor();
    println!(
        "Item GPU verified: distinct tool icons/held silhouettes, half-stack cursor, tagged private-save round trip"
    );
    super::storage::verify(renderer, game, capture, native, deadline)?;
    Ok(())
}

fn verify_materials(
    renderer: &mut Renderer,
    game: &mut Game,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let feet = game.player.position.floor().as_ivec3().to_array();
    let target = [feet[0] + 1, feet[1], feet[2] - 2];
    let coord = crate::world::ChunkCoord::from_world(target[0] as f32, target[2] as f32);
    for (block, label) in [
        (Block::Planks, "placed-planks"),
        (Block::DressedLimestone, "placed-dressed-limestone"),
    ] {
        check_deadline(deadline)?;
        game.world.edit_block(target, block);
        let chunk = crate::world::Chunk::generate(&game.terrain, coord);
        renderer.upload_chunk(
            coord,
            &crate::mesh::mesh_chunk_with_edits(&chunk, &game.terrain, game.world.edits()),
        );
        super::aim_at(
            &mut game.player,
            glam::Vec3::from_array(target.map(|coordinate| coordinate as f32))
                + glam::Vec3::splat(0.5),
        );
        game.inventory_mut().insert(block, 2);
        let index = game
            .inventory()
            .slots()
            .iter()
            .position(|slot| slot.is_some_and(|stack| stack.item == block.into()))
            .ok_or_else(|| io::Error::other("processed item insertion failed"))?;
        if index < 9 {
            game.inventory_mut().select(index);
        }
        let frame = super::survival::frame(game, &[], true);
        capture_scene(renderer, capture, label, &frame)?;
        if native && !renderer.render(&frame)? {
            return Err(
                io::Error::other("processed material completion frame not presented").into(),
            );
        }
    }
    Ok(())
}

fn verify_crafting(
    renderer: &mut Renderer,
    game: &mut Game,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    check_deadline(deadline)?;
    game.inventory_mut().insert(Block::Wood, 2);
    game.inventory_mut().insert(Block::Limestone, 1);
    let hud = super::survival::interface(renderer, game, Screen::Crafting);
    capture_scene(
        renderer,
        capture,
        "crafting-available",
        &super::survival::frame(game, &hud, false),
    )?;
    for index in 0..2 {
        game.inventory_mut()
            .craft(index)
            .map_err(|error| io::Error::other(format!("craft failed: {error:?}")))?;
    }
    game.inventory_mut().insert(Block::Stone, 5);
    game.inventory_mut().insert(Block::Planks, 8);
    for index in 2..4 {
        game.inventory_mut()
            .craft(index)
            .map_err(|error| io::Error::other(format!("tool craft failed: {error:?}")))?;
    }
    let hud = super::survival::interface(renderer, game, Screen::Crafting);
    capture_scene(
        renderer,
        capture,
        "crafting-all-delivered",
        &super::survival::frame(game, &hud, false),
    )?;
    let before = game.snapshot();
    let scratch = super::persistence::ScratchDirectory::new()?;
    let slot = SaveSlot::new(scratch.0.join("world.save"));
    slot.write(&before)?;
    let restored = slot
        .load()?
        .ok_or_else(|| io::Error::other("crafted save missing"))?;
    if restored != before {
        return Err(io::Error::other("crafted inventory changed on save").into());
    }
    let hud = super::survival::interface(renderer, game, Screen::Crafting);
    let frame = super::survival::frame(game, &hud, false);
    capture_scene(renderer, capture, "crafting-after", &frame)?;
    if native && !renderer.render(&frame)? {
        return Err(io::Error::other("crafting completion frame not presented").into());
    }
    println!(
        "Crafting GPU verified: delivered material/tool recipes, paused controls, actual transactional outputs and private-save round trip"
    );
    Ok(())
}
