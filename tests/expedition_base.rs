//! Public survival loop: gather, craft, equip, store, light, evict and restart safely.

use glam::Vec3;

use rustcraft::game::{CraftError, Game, GameMode, Item, RecipeId, StorageSlot, Tool};
use rustcraft::mesh::mesh_chunk_with_edits;
use rustcraft::save::SaveSlot;
use rustcraft::world::{Block, Chunk, ChunkCoord, MIN_WORLD_Y, WORLD_HEIGHT, WorldView};

fn arena() -> Game {
    let mut game = Game::with_mode(42, GameMode::Survival);
    let terrain = game.terrain();
    game.world_mut()
        .insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    for x in 0..16 {
        for z in 0..16 {
            for y in MIN_WORLD_Y..WORLD_HEIGHT {
                game.world_mut()
                    .edit_block([x, y, z], if y <= 0 { Block::Stone } else { Block::Air });
            }
        }
    }
    let mut snapshot = game.snapshot();
    snapshot.player.position = Vec3::new(8.5, 1.0, 8.5);
    snapshot.player.yaw = 0.0;
    snapshot.player.pitch = 0.0;
    let mut game = Game::from_snapshot(snapshot).unwrap();
    game.world_mut()
        .insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    game
}

fn mine_and_collect(game: &mut Game, block: Block, count: usize) {
    for _ in 0..count {
        game.world_mut().edit_block([8, 2, 7], block);
        for _ in 0..60 {
            game.interact(true, false, 0.05);
            if game.world().block_at(8, 2, 7) == Some(Block::Air) {
                break;
            }
        }
        assert_eq!(game.world().block_at(8, 2, 7), Some(Block::Air));
        for _ in 0..15 {
            game.interact(false, false, 0.05);
        }
    }
}

fn select(game: &mut Game, item: Item) {
    let index = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == item))
        .unwrap();
    assert!(index < 9);
    game.inventory_mut().select(index);
}

#[test]
fn complete_base_progress_survives_crate_cursor_eviction_private_save_and_continue() {
    let mut game = arena();
    mine_and_collect(&mut game, Block::Wood, 5);
    mine_and_collect(&mut game, Block::Stone, 5);
    mine_and_collect(&mut game, Block::Crystal, 1);
    assert!(game.pickups().is_empty());
    for _ in 0..5 {
        game.inventory_mut()
            .craft(RecipeId::Planks.index())
            .unwrap();
    }
    for recipe in [
        RecipeId::StonePickaxe,
        RecipeId::StoneAxe,
        RecipeId::StorageCrate,
        RecipeId::Lantern,
    ] {
        game.inventory_mut().craft(recipe.index()).unwrap();
    }
    select(&mut game, Tool::StonePickaxe.into());
    game.world_mut().edit_block([8, 2, 7], Block::Stone);
    for _ in 0..4 {
        game.interact(true, false, 0.2);
    }
    assert_eq!(game.world().block_at(8, 2, 7), Some(Block::Stone));
    game.interact(true, false, 0.05);
    assert_eq!(game.world().block_at(8, 2, 7), Some(Block::Air));
    for _ in 0..12 {
        game.interact(false, false, 0.05);
    }
    select(&mut game, Block::StorageCrate.into());
    game.world_mut().edit_block([8, 2, 6], Block::Stone);
    assert!(game.place_selected());
    assert!(game.try_open_storage());
    let tool = game
        .inventory()
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|stack| stack.item == Tool::StoneAxe.into()))
        .unwrap();
    game.storage_click(StorageSlot::Player(tool), false);
    game.storage_click(StorageSlot::Crate(0), false);
    game.storage_click(StorageSlot::Crate(0), true);
    assert_eq!(
        game.snapshot().crates[0].slots[0].unwrap().item,
        Tool::StoneAxe.into()
    );
    game.close_storage();
    let mut pose = game.snapshot();
    pose.player.position.z = 10.5;
    let terrain = game.terrain();
    game = Game::from_snapshot(pose).unwrap();
    game.world_mut()
        .insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    select(&mut game, Block::Lantern.into());
    assert!(game.place_selected());
    let chunk = Chunk::generate(&terrain, ChunkCoord::new(0, 0));
    let before_mesh = mesh_chunk_with_edits(&chunk, &terrain, game.world().edits());
    assert!(
        before_mesh
            .opaque
            .vertices
            .iter()
            .any(|vertex| vertex.local_light[0] > 0.0)
    );
    let before = game.snapshot();
    assert_eq!(
        game.inventory_mut().craft(RecipeId::StonePickaxe.index()),
        Err(CraftError::MissingIngredients)
    );
    assert_eq!(game.snapshot(), before);
    game.world_mut().remove(ChunkCoord::new(0, 0));
    let directory = OwnedDirectory::new();
    let slot = SaveSlot::new(directory.0.join("world.save"));
    slot.write(&game.snapshot()).unwrap();
    let mut continued = Game::from_snapshot(slot.load().unwrap().unwrap()).unwrap();
    assert_eq!(continued.snapshot(), before);
    continued
        .world_mut()
        .insert(Chunk::generate(&terrain, ChunkCoord::new(0, 0)));
    let resumed = mesh_chunk_with_edits(&chunk, &terrain, continued.world().edits());
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&before_mesh.opaque.vertices),
        bytemuck::cast_slice::<_, u8>(&resumed.opaque.vertices)
    );
    assert!(continued.snapshot().validate().is_ok());
}

struct OwnedDirectory(std::path::PathBuf);

impl OwnedDirectory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        loop {
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rustcraft-base-tests-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("private base fixture: {error}"),
            }
        }
    }
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
