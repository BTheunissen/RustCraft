//! Crafted cubes retain ordinary survival, save, collision, and mesh contracts.

use rustcraft::game::{Game, GameMode, SoundAction, SoundSurface};
use rustcraft::mesh::mesh_chunk_with_edits;
use rustcraft::world::{Block, Chunk, ChunkCoord, WorldView};

#[test]
fn processed_blocks_mine_collect_place_save_and_reload_as_their_own_material() {
    for block in [Block::Planks, Block::DressedLimestone] {
        let mut game = Game::with_mode(42, GameMode::Survival);
        let terrain = game.terrain();
        let feet = game.player().position;
        let coord = ChunkCoord::from_world(feet.x, feet.z);
        game.world_mut().insert(Chunk::generate(&terrain, coord));
        let source = [
            feet.x.floor() as i32,
            feet.y.floor() as i32 - 1,
            feet.z.floor() as i32,
        ];
        assert!(game.world_mut().edit_block(source, block));
        game.look(0.0, 1000.0);
        for _ in 0..10 {
            game.interact(true, false, 0.25);
            if game.world().block_at(source[0], source[1], source[2]) == Some(Block::Air) {
                break;
            }
        }
        assert_eq!(
            game.world().block_at(source[0], source[1], source[2]),
            Some(Block::Air)
        );
        assert_eq!(game.pickups().len(), 1);
        assert_eq!(game.pickups()[0].item, block.into());
        let sound = game.take_sound_events();
        assert_eq!(
            sound
                .iter()
                .flatten()
                .filter(|event| matches!(event.action, SoundAction::Break(_)))
                .count(),
            1
        );
        let family = if block == Block::Planks {
            SoundSurface::Wood
        } else {
            SoundSurface::Stone
        };
        assert!(
            sound
                .iter()
                .flatten()
                .any(|event| event.action == SoundAction::Break(family))
        );
        game.inventory_mut().insert(block, 2);
        game.respawn();
        let placed = (0..8).any(|index| {
            let yaw = index as f32 * std::f32::consts::TAU / 8.0;
            game.look(
                f64::from(yaw - game.player().yaw) / 0.0025,
                -f64::from(-0.5 - game.player().pitch) / 0.0025,
            );
            game.place_selected()
        });
        assert!(placed);
        let snapshot = game.snapshot();
        let mut restored = Game::from_snapshot(snapshot.clone()).unwrap();
        restored
            .world_mut()
            .insert(Chunk::generate(&terrain, coord));
        assert_eq!(restored.snapshot(), snapshot);
        for edit in snapshot.edits {
            assert_eq!(
                restored
                    .world()
                    .block_at(edit.position[0], edit.position[1], edit.position[2]),
                Some(edit.block)
            );
        }
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk_with_edits(&chunk, &terrain, restored.world().edits());
        let tile = if block == Block::Planks { 24 } else { 25 };
        assert!(
            mesh.opaque
                .vertices
                .iter()
                .any(|vertex| vertex.material == tile)
        );
        assert!(block.is_solid() && block.is_opaque());
    }
}
