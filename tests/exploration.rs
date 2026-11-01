//! Renderer-independent checks across actual generated terrain and player simulation.

use glam::Vec3;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::mesh_chunk;
use rustcraft::player::{MovementInput, Player};
use rustcraft::world::{Chunk, ChunkCoord, Terrain};

#[test]
fn generated_spawn_supports_jumping_and_landing() {
    let terrain = Terrain::default();
    let spawn = Vec3::from_array(terrain.spawn_position());
    let mut player = Player::new(spawn);
    let solid = |x, y, z| terrain.block_at(x, y, z).is_solid();
    for _ in 0..10 {
        player.update(MovementInput::default(), 1.0 / 60.0, solid);
    }
    assert!(player.is_grounded());
    let ground = player.position.y;
    let mut peak = ground;
    for frame in 0..120 {
        player.update(
            MovementInput {
                jump: frame == 0,
                ..Default::default()
            },
            1.0 / 60.0,
            solid,
        );
        peak = peak.max(player.position.y);
    }
    assert!(
        peak > ground + 0.8,
        "jump only rose {} blocks",
        peak - ground
    );
    assert!(player.is_grounded());
    assert!((player.position.y - ground).abs() < 0.01);
}

#[test]
fn flight_explores_multiple_chunk_columns_and_respawn_restores_a_safe_pose() {
    let terrain = Terrain::new(42);
    let spawn = Vec3::from_array(terrain.spawn_position());
    let solid = |x, y, z| terrain.block_at(x, y, z).is_solid();
    let mut player = Player::new(spawn);
    let initial_chunk = ChunkCoord::from_world(spawn.x, spawn.z);
    player.toggle_flight(solid);
    for _ in 0..360 {
        player.update(
            MovementInput {
                forward: true,
                jump: true,
                speed: rustcraft::player::SpeedMultiplier::Double,
                ..Default::default()
            },
            1.0 / 60.0,
            solid,
        );
    }
    assert!(player.flying);
    assert!(player.position.y > spawn.y + 10.0);
    assert_ne!(
        ChunkCoord::from_world(player.position.x, player.position.z),
        initial_chunk
    );
    player.respawn(spawn);
    assert!((player.position - spawn).length() < 0.01);
    assert!(!player.flying);
    for _ in 0..5 {
        player.update(MovementInput::default(), 1.0 / 60.0, solid);
    }
    assert!(player.is_grounded());
}

#[test]
fn neighboring_generated_meshes_are_valid_in_negative_world_space() {
    let terrain = Terrain::default();
    for coord in [ChunkCoord::new(-1, -1), ChunkCoord::new(0, -1)] {
        let chunk = Chunk::generate(&terrain, coord);
        let mesh = mesh_chunk(&chunk, &terrain);
        assert!(!mesh.opaque.vertices.is_empty());
        for submesh in [&mesh.opaque, &mesh.water] {
            assert!(submesh.vertices.iter().all(|vertex| {
                vertex.position.iter().all(|value| value.is_finite())
                    && vertex.material < TILE_COUNT
            }));
            assert!(
                submesh
                    .indices
                    .iter()
                    .all(|index| (*index as usize) < submesh.vertices.len())
            );
            assert_eq!(submesh.indices.len() % 6, 0);
        }
    }
}
