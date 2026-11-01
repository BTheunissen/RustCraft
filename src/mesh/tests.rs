use std::cell::Cell;

use crate::world::{ChunkCoord, WORLD_HEIGHT};

use super::*;

fn mesh_blocks(size: [i32; 3], blocks: &[([i32; 3], Block)]) -> ChunkMesh {
    mesh_volume(
        [0; 3],
        size,
        |position| {
            blocks
                .iter()
                .find(|(local, _)| *local == position)
                .map_or(Block::Air, |(_, block)| *block)
        },
        |_| Block::Air,
    )
}

fn profile_mesh(
    origin: [i32; 3],
    size: [i32; 3],
    profiles: &[([i32; 2], WaterSample)],
) -> ChunkMesh {
    let profiles: HashMap<_, _> = profiles.iter().copied().collect();
    let block_at = |[x, y, z]: [i32; 3]| {
        if let Some(profile) = profiles.get(&[x, z]) {
            if y <= profile.bottom {
                Block::Clay
            } else if (y as f32) < profile.surface {
                Block::Water
            } else {
                Block::Air
            }
        } else {
            Block::Air
        }
    };
    let mut metadata = WaterMetadata::default();
    for (&position, &profile) in &profiles {
        let blocks: Vec<_> = (origin[1]..origin[1] + size[1])
            .map(|y| block_at([position[0], y, position[1]]))
            .collect();
        metadata.add_column(position, origin[1], &blocks, |_| Some(profile));
    }
    mesh_volume_with_water(
        origin,
        size,
        |local| block_at(std::array::from_fn(|axis| origin[axis] + local[axis])),
        block_at,
        Some(&mut metadata),
        None,
        None,
    )
}

fn water_profile(surface: f32, biome: WaterBiome) -> WaterSample {
    WaterSample {
        bottom: 0,
        surface,
        flow: [0.35, 0.0, -0.2],
        biome,
        salinity: if biome == WaterBiome::Ocean { 1.0 } else { 0.0 },
    }
}

fn face_count(mesh: &Mesh) -> usize {
    assert_eq!(mesh.vertices.len() % 4, 0);
    assert_eq!(mesh.indices.len(), mesh.vertices.len() / 4 * 6);
    mesh.vertices.len() / 4
}

fn face_center(vertices: &[Vertex; 4]) -> [f32; 3] {
    std::array::from_fn(|axis| vertices.iter().map(|v| v.position[axis]).sum::<f32>() / 4.0)
}

fn faces(mesh: &Mesh) -> &[[Vertex; 4]] {
    let (faces, remainder) = mesh.vertices.as_chunks::<4>();
    assert!(remainder.is_empty());
    faces
}

fn reference_daylight(y: i32, mut block_at: impl FnMut(i32) -> Block) -> f32 {
    let mut light = 1.0;
    for above in (y.max(MIN_WORLD_Y)..WORLD_HEIGHT).rev() {
        match block_at(above) {
            Block::Air => {}
            Block::Water => light *= 0.9,
            Block::Leaves => light *= 0.65,
            _ => return 0.0,
        }
    }
    light
}

fn water_face_visible(terrain: &Terrain, world: [i32; 3], face: &Face) -> bool {
    let neighbor = std::array::from_fn::<_, 3, _>(|axis| world[axis] + face.offset[axis]);
    let neighbor_block = terrain.block_at(neighbor[0], neighbor[1], neighbor[2]);
    if neighbor_block == Block::Air {
        return true;
    }
    let own = terrain.water_at(world[0], world[1], world[2]).unwrap();
    if face.offset[1] == 1 && neighbor_block.is_opaque() && own.surface < world[1] as f32 + 1.0 {
        return true;
    }
    if neighbor_block != Block::Water || face.offset[1] != 0 {
        return false;
    }
    let other = terrain
        .water_at(neighbor[0], neighbor[1], neighbor[2])
        .unwrap();
    other.surface < own.surface.min(world[1] as f32 + 1.0)
}

fn source_block(face: &[Vertex; 4]) -> [i32; 3] {
    std::array::from_fn(|axis| {
        let minimum = face
            .iter()
            .map(|vertex| vertex.position[axis])
            .fold(f32::MAX, f32::min);
        if face[0].normal[axis] > 0.5 {
            (minimum.ceil() as i32).saturating_sub(1)
        } else {
            minimum.floor() as i32
        }
    })
}

fn assert_valid_mesh(mesh: &Mesh) {
    face_count(mesh);
    assert!(
        mesh.indices
            .iter()
            .all(|index| (*index as usize) < mesh.vertices.len())
    );

    for (face_index, vertices) in faces(mesh).iter().enumerate() {
        let base = (face_index * 4) as u32;
        assert_eq!(
            &mesh.indices[face_index * 6..face_index * 6 + 6],
            &[base, base + 1, base + 2, base, base + 2, base + 3]
        );
        assert!(vertices.iter().all(|v| v.normal == vertices[0].normal));
        assert!(vertices.iter().all(|v| v.material == vertices[0].material));
        assert!(vertices.iter().all(|v| {
            v.light.is_finite() && (0.0..=1.0).contains(&v.light) && v.light == vertices[0].light
        }));
        assert!(vertices.iter().all(|v| {
            v.position
                .into_iter()
                .chain(v.normal)
                .chain(v.uv)
                .chain(v.water)
                .chain(v.water_extra)
                .all(f32::is_finite)
        }));
        if vertices[0].material != 9 {
            assert!(
                vertices
                    .iter()
                    .all(|v| v.water == [0.0; 4] && v.water_extra == [0.0; 4])
            );
        }
        assert!(
            vertices
                .iter()
                .all(|v| v.material < crate::atlas::TILE_COUNT)
        );
        for uv in [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]] {
            assert_eq!(vertices.iter().filter(|v| v.uv == uv).count(), 1);
        }
    }

    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = std::array::from_fn(|index| mesh.vertices[triangle[index] as usize]);
        let ab: [f32; 3] = std::array::from_fn(|axis| b.position[axis] - a.position[axis]);
        let ac: [f32; 3] = std::array::from_fn(|axis| c.position[axis] - a.position[axis]);
        let cross = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let area = cross
            .into_iter()
            .zip(a.normal)
            .map(|(cross, normal)| cross * normal)
            .sum::<f32>();
        assert!(area > 0.0);
        for (axis, cross) in cross.into_iter().enumerate() {
            assert!((cross - a.normal[axis] * area).abs() < 0.000001);
        }
        assert_eq!(a.normal.iter().map(|v| v * v).sum::<f32>(), 1.0);
    }
}

mod edits;
mod geometry;
mod lighting;
mod seams;
mod water;
