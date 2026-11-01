//! Actual placement/transfer/eviction/private-save crate acceptance with GPU readback.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

use super::{capture_scene, check_deadline};
use crate::game::{Game, StorageSlot};
use crate::hud::Screen;
use crate::renderer::Renderer;
use crate::save::SaveSlot;
use crate::world::{Block, Chunk, ChunkCoord};

pub(super) fn verify(
    renderer: &mut Renderer,
    game: &mut Game,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    check_deadline(deadline)?;
    game.inventory_mut().close_cursor();
    game.inventory_mut().insert(Block::Wood, 3);
    for _ in 0..3 {
        game.inventory_mut()
            .craft(crate::game::RecipeId::Planks.index())
            .map_err(|error| io::Error::other(format!("base plank craft: {error:?}")))?;
    }
    game.inventory_mut()
        .craft(crate::game::RecipeId::StorageCrate.index())
        .map_err(|error| io::Error::other(format!("base crate craft: {error:?}")))?;
    game.inventory_mut().insert(Block::Crystal, 1);
    game.inventory_mut()
        .craft(crate::game::RecipeId::Lantern.index())
        .map_err(|error| io::Error::other(format!("base lantern craft: {error:?}")))?;
    let index = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Block::StorageCrate.into()))
        .ok_or_else(|| io::Error::other("crate insertion failed"))?;
    if index >= 9 {
        return Err(io::Error::other("crate fixture needs hotbar capacity").into());
    }
    game.inventory_mut().select(index);
    if !game.place_selected() {
        return Err(io::Error::other("ordinary crate placement failed").into());
    }
    if !game.try_open_storage() {
        return Err(io::Error::other("resident crate did not open").into());
    }
    game.inventory_mut().insert(Block::Crystal, 3);
    let crystal = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Block::Crystal.into()))
        .ok_or_else(|| io::Error::other("crystal insertion failed"))?;
    game.storage_click(StorageSlot::Player(crystal), false);
    game.storage_click(StorageSlot::Crate(0), false);
    game.storage_click(StorageSlot::Crate(0), true);
    let position = game
        .storage_view()
        .ok_or_else(|| io::Error::other("crate view missing"))?
        .position;
    let coord = ChunkCoord::from_world(position[0] as f32, position[2] as f32);
    let chunk = Chunk::generate(&game.terrain, coord);
    renderer.upload_chunk(
        coord,
        &crate::mesh::mesh_chunk_with_edits(&chunk, &game.terrain, game.world.edits()),
    );
    let hud = super::survival::interface(renderer, game, Screen::Storage);
    capture_scene(
        renderer,
        capture,
        "crate-transfer",
        &super::survival::frame(game, &hud, false),
    )?;
    let before = game.snapshot();
    let scratch = super::persistence::ScratchDirectory::new()?;
    let slot = SaveSlot::new(scratch.0.join("world.save"));
    slot.write(&before)?;
    game.close_storage();
    game.world.remove(coord);
    let mut restored = Game::from_snapshot(
        slot.load()?
            .ok_or_else(|| io::Error::other("crate save missing"))?,
    )
    .map_err(io::Error::other)?;
    restored.world.insert(Chunk::generate(&game.terrain, coord));
    if restored.snapshot() != before || !restored.try_open_storage() {
        return Err(io::Error::other(
            "crate contents/association changed across eviction and save",
        )
        .into());
    }
    let hud = super::survival::interface(renderer, &restored, Screen::Storage);
    let frame = super::survival::frame(&restored, &hud, false);
    capture_scene(renderer, capture, "crate-resumed", &frame)?;
    if native && !renderer.render(&frame)? {
        return Err(io::Error::other("crate completion frame not presented").into());
    }
    game.world.insert(Chunk::generate(&game.terrain, coord));
    let mut farther = game.player.position;
    farther.z += 2.0;
    game.player.respawn(farther);
    let lantern = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Block::Lantern.into()))
        .ok_or_else(|| io::Error::other("crafted lantern is missing"))?;
    if lantern >= 9 {
        return Err(io::Error::other("base fixture lantern must be in hotbar").into());
    }
    game.inventory_mut().select(lantern);
    super::aim_at(
        &mut game.player,
        glam::Vec3::from_array(position.map(|coordinate| coordinate as f32))
            + glam::Vec3::splat(0.5),
    );
    if !game.place_selected() {
        return Err(io::Error::other("ordinary base lantern placement failed").into());
    }
    let edited = Chunk::generate(&game.terrain, coord);
    let mesh = crate::mesh::mesh_chunk_with_edits(&edited, &game.terrain, game.world.edits());
    if !mesh
        .opaque
        .vertices
        .iter()
        .any(|vertex| vertex.local_light[0] > 0.0)
    {
        return Err(io::Error::other("crafted lantern has no local surface field").into());
    }
    renderer.upload_chunk(coord, &mesh);
    let hud = super::survival::interface(renderer, game, Screen::Playing);
    capture_scene(
        renderer,
        capture,
        "base-crafted-lantern",
        &super::survival::frame(game, &hud, true),
    )?;
    println!(
        "Storage/base GPU verified: crafted crate/light, ordinary placement, half/whole transfer, cursor-normalized private save, eviction and resumed contents"
    );
    Ok(())
}
