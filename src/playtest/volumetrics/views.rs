//! Scenic canopy and entrance selection checks real voxels and clear lines of sight.

use crate::player::Player;
use crate::world::{Biome, Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain};
use crate::world::{ResidentWorld, WorldView};
use glam::Vec3;

pub(super) fn entrance_view(
    world: &ResidentWorld,
    player: &Player,
    aspect: f32,
) -> Option<(Vec3, Vec3)> {
    let eye = player.eye_position();
    let inverse =
        crate::camera::view_projection(eye, player.look_direction(), aspect, 192.0).inverse();
    for ndc_y in [0.16, 0.0, -0.12] {
        for ndc_x in [0.0, 0.22, -0.22, 0.4, -0.4] {
            let point = inverse * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
            let direction = (point.truncate() / point.w - eye).normalize();
            for distance in 12..=48 {
                let point = eye + direction * distance as f32;
                let [x, y, z] = point.floor().as_ivec3().to_array();
                if world.block_at(x, y, z) != Some(Block::Air)
                    || crate::lighting::sky_exposure(point, |x, y, z| {
                        world.block_at(x, y, z).unwrap_or(Block::Stone)
                    }) >= 0.05
                {
                    continue;
                }
                let clear = (12..distance * 2).all(|step| {
                    let [x, y, z] = (eye + direction * (step as f32 * 0.5))
                        .floor()
                        .as_ivec3()
                        .to_array();
                    world.block_at(x, y, z) == Some(Block::Air)
                });
                if clear {
                    return Some((point, eye + direction * 6.0));
                }
            }
        }
    }
    None
}

pub(in crate::playtest) fn forest_view(terrain: Terrain) -> Option<(Vec3, Vec3)> {
    let spawn = terrain.spawn_position();
    let center = ChunkCoord::from_world(spawn[0], spawn[2]);
    let mut offsets: Vec<_> = (-6..=6)
        .flat_map(|z| (-6..=6).map(move |x| (x, z)))
        .collect();
    offsets.sort_by_key(|(x, z)| x * x + z * z);
    for (dx, dz) in offsets {
        let coord = ChunkCoord::new(center.x + dx, center.z + dz);
        if terrain
            .column(coord.x * CHUNK_SIZE + 8, coord.z * CHUNK_SIZE + 8)
            .biome
            != Biome::Forest
        {
            continue;
        }
        let chunk = Chunk::generate(&terrain, coord);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let wx = coord.x * CHUNK_SIZE + x;
                let wz = coord.z * CHUNK_SIZE + z;
                let base = terrain.column(wx, wz).height + 1;
                if chunk.block(x, base, z) != Block::Wood {
                    continue;
                }
                let target = Vec3::new(wx as f32 + 0.5, base as f32 + 3.0, wz as f32 + 0.5);
                for (ox, oz) in [(6, 6), (5, 7), (7, 4), (4, 6)] {
                    let (cx, cz) = (wx + ox, wz + oz);
                    let ground = terrain.column(cx, cz).height + 1;
                    if (ground - base).abs() > 2 {
                        continue;
                    }
                    let blocks = terrain.column_blocks(cx, cz);
                    if (ground..ground + 3)
                        .any(|y| blocks[(y - MIN_WORLD_Y) as usize] != Block::Air)
                    {
                        continue;
                    }
                    let position = Vec3::new(cx as f32 + 0.5, ground as f32, cz as f32 + 0.5);
                    let eye = position + Vec3::Y * crate::player::EYE_HEIGHT;
                    if clear_view(terrain, eye, target) {
                        return Some((position, target));
                    }
                }
            }
        }
    }
    None
}

fn clear_view(terrain: Terrain, eye: Vec3, target: Vec3) -> bool {
    let mut columns = std::collections::HashMap::new();
    let delta = target - eye;
    (1..32).all(|step| {
        let [x, y, z] = (eye + delta * (step as f32 / 32.0))
            .floor()
            .as_ivec3()
            .to_array();
        let column = columns
            .entry((x, z))
            .or_insert_with(|| terrain.column_blocks(x, z));
        column[(y - MIN_WORLD_Y) as usize] == Block::Air
            || (target - Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5)).length() < 1.0
    })
}
