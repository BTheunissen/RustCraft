//! CPU-side exposed-face meshing in world coordinates, with separate water geometry.

use crate::lighting::daylight_through;
use crate::water::biome_wave_weight;
use crate::world::{
    Block, CHUNK_SIZE, Chunk, MIN_WORLD_Y, Terrain, WORLD_DEPTH, WaterBiome, WaterSample,
    WorldEdits,
};
use std::collections::HashMap;

/// A world-space vertex with face-local UVs, an atlas tile, and occluded daylight.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub material: u32,
    /// Per-face vertical skylight in `0..=1`, separate from local warm transport.
    pub light: f32,
    /// Horizontal current, water-column depth, and salinity; zero for opaque faces.
    pub water: [f32; 4],
    /// Water: vertical current, biome, foam, wave weight. Opaque X marks exposed
    /// coastal overhang atmosphere independently of its shaded surface light.
    pub water_extra: [f32; 4],
    /// Local warm lantern transport, never camera/sky exposure or water metadata.
    pub local_light: [f32; 3],
}

/// Indexed triangles with outward counterclockwise winding.
#[derive(Default, Debug)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Opaque and water geometry that can be uploaded and drawn separately.
#[derive(Default, Debug)]
pub struct ChunkMesh {
    pub opaque: Mesh,
    pub water: Mesh,
}

mod coastal_lighting;
mod geometry;
mod lanterns;
mod materials;
mod sampling;
mod water;

use geometry::{FACES, Face, UVS};
pub(crate) use materials::material_for_face;
use sampling::{BoundarySamples, SkylightColumn};
use water::WaterMetadata;

/// Meshes a chunk without an edit overlay, using batched procedural halo columns.
/// Opaque faces border non-opaque blocks; water also exposes fractional strips.
/// Cached vertical daylight is occluded by roofs regardless of their elevation.
pub fn mesh_chunk(chunk: &Chunk, terrain: &Terrain) -> ChunkMesh {
    mesh_chunk_impl(chunk, terrain, None)
}

/// Meshes edited local voxels and neighboring daylight/water-corner columns.
/// The input may already contain edits; reapplying the sparse overlay is harmless.
/// Missing neighbors use one bounded generation batch, then the same snapshot.
pub fn mesh_chunk_with_edits(chunk: &Chunk, terrain: &Terrain, edits: &WorldEdits) -> ChunkMesh {
    mesh_chunk_impl(chunk, terrain, Some(edits))
}

fn mesh_chunk_impl(chunk: &Chunk, terrain: &Terrain, edits: Option<&WorldEdits>) -> ChunkMesh {
    let coord = chunk.coord();
    let (Some(x), Some(z)) = (
        coord.x.checked_mul(CHUNK_SIZE),
        coord.z.checked_mul(CHUNK_SIZE),
    ) else {
        return ChunkMesh::default();
    };
    let columns: Option<Vec<_>> = edits
        .filter(|edits| edits.has_chunk_edits(coord))
        .map(|edits| {
            (0..CHUNK_SIZE)
                .flat_map(|local_z| (0..CHUNK_SIZE).map(move |local_x| [local_x, local_z]))
                .map(|[local_x, local_z]| {
                    let mut blocks = std::array::from_fn(|index| {
                        chunk.block(local_x, MIN_WORLD_Y + index as i32, local_z)
                    });
                    edits.apply_to_column([x + local_x, z + local_z], &mut blocks);
                    blocks
                })
                .collect()
        });
    let local_block = |[x, y, z]: [i32; 3]| {
        columns.as_ref().map_or_else(
            || chunk.block(x, y, z),
            |columns| columns[(z * CHUNK_SIZE + x) as usize][(y - MIN_WORLD_Y) as usize],
        )
    };
    let mut coastal =
        coastal_lighting::CoastalLight::prepare(chunk, terrain, edits, [x, z], local_block);
    let mut positions = occupied_halo_columns_from([x, z], local_block);
    for (inside, outside) in [
        ([0, 0], [-1, -1]),
        ([CHUNK_SIZE - 1, 0], [CHUNK_SIZE, -1]),
        ([0, CHUNK_SIZE - 1], [-1, CHUNK_SIZE]),
        ([CHUNK_SIZE - 1, CHUNK_SIZE - 1], [CHUNK_SIZE, CHUNK_SIZE]),
    ] {
        if (MIN_WORLD_Y..MIN_WORLD_Y + WORLD_DEPTH)
            .any(|y| local_block([inside[0], y, inside[1]]) == Block::Water)
            && let (Some(x), Some(z)) = (x.checked_add(outside[0]), z.checked_add(outside[1]))
        {
            positions.push([x, z]);
        }
    }
    let halo: HashMap<_, _> = positions
        .iter()
        .zip(terrain.column_block_batch(&positions))
        .map(|(&[x, z], mut blocks)| {
            if let Some(edits) = edits {
                edits.apply_to_column([x, z], &mut blocks);
            }
            ((x, z), blocks)
        })
        .collect();
    let mut water = WaterMetadata::default();
    let lanterns = lanterns::prepare([x, z], terrain, edits);
    for local_z in 0..CHUNK_SIZE {
        for local_x in 0..CHUNK_SIZE {
            let blocks: [Block; WORLD_DEPTH as usize] = std::array::from_fn(|index| {
                local_block([local_x, MIN_WORLD_Y + index as i32, local_z])
            });
            water.add_column([x + local_x, z + local_z], MIN_WORLD_Y, &blocks, |last| {
                chunk.water(local_x, last, local_z)
            });
        }
    }
    for (&(x, z), blocks) in &halo {
        if blocks.contains(&Block::Water) {
            let profile = terrain.water_column(x, z);
            water.add_column([x, z], MIN_WORLD_Y, blocks, |_| profile);
        }
    }
    mesh_volume_with_water(
        [x, MIN_WORLD_Y, z],
        [CHUNK_SIZE, WORLD_DEPTH, CHUNK_SIZE],
        |[x, y, z]| local_block([x, y + MIN_WORLD_Y, z]),
        |[x, y, z]| {
            if !(MIN_WORLD_Y..MIN_WORLD_Y + WORLD_DEPTH).contains(&y) {
                return Block::Air;
            }
            halo.get(&(x, z))
                .map_or(Block::Air, |blocks| blocks[(y - MIN_WORLD_Y) as usize])
        },
        Some(&mut water),
        Some(&mut coastal),
        lanterns.as_ref(),
    )
}

#[cfg(test)]
fn occupied_halo_columns(chunk: &Chunk, origin: [i32; 2]) -> Vec<[i32; 2]> {
    occupied_halo_columns_from(origin, |[x, y, z]| chunk.block(x, y, z))
}

fn occupied_halo_columns_from(
    origin: [i32; 2],
    block_at: impl Fn([i32; 3]) -> Block,
) -> Vec<[i32; 2]> {
    let mut columns = Vec::with_capacity((CHUNK_SIZE * 4) as usize);
    for along in 0..CHUNK_SIZE {
        for (inside, outside) in [
            ([0, along], [-1, along]),
            ([CHUNK_SIZE - 1, along], [CHUNK_SIZE, along]),
            ([along, 0], [along, -1]),
            ([along, CHUNK_SIZE - 1], [along, CHUNK_SIZE]),
        ] {
            if (MIN_WORLD_Y..MIN_WORLD_Y + WORLD_DEPTH).any(|y| {
                let block = block_at([inside[0], y, inside[1]]);
                block.is_opaque() || block == Block::Water
            }) && let (Some(x), Some(z)) = (
                origin[0].checked_add(outside[0]),
                origin[1].checked_add(outside[1]),
            ) {
                columns.push([x, z]);
            }
        }
    }
    columns
}

#[cfg(test)]
fn mesh_volume(
    origin: [i32; 3],
    size: [i32; 3],
    local_block: impl FnMut([i32; 3]) -> Block,
    outside_block: impl FnMut([i32; 3]) -> Block,
) -> ChunkMesh {
    mesh_volume_with_water(origin, size, local_block, outside_block, None, None, None)
}

fn mesh_volume_with_water(
    origin: [i32; 3],
    size: [i32; 3],
    mut local_block: impl FnMut([i32; 3]) -> Block,
    mut outside_block: impl FnMut([i32; 3]) -> Block,
    mut water: Option<&mut WaterMetadata>,
    mut coastal: Option<&mut coastal_lighting::CoastalLight>,
    lanterns: Option<&crate::lighting::lantern::LanternField>,
) -> ChunkMesh {
    let mut result = ChunkMesh::default();
    if size.into_iter().any(|length| length <= 0) {
        return result;
    }
    let columns: Vec<_> = (0..size[2])
        .flat_map(|z| (0..size[0]).map(move |x| [x, z]))
        .map(|[x, z]| SkylightColumn::sample(size[1], |y| local_block([x, y, z])))
        .collect();
    let mut boundary = BoundarySamples::new(origin, size);

    for z in 0..size[2] {
        for x in 0..size[0] {
            let column = &columns[(z * size[0] + x) as usize];
            for y in 0..size[1] {
                let local = [x, y, z];
                let block = column.blocks[y as usize];
                let is_water = block == Block::Water;
                if !is_water && !block.is_opaque() {
                    continue;
                }

                let world = std::array::from_fn(|axis| origin[axis] + local[axis]);
                let mesh = if is_water {
                    &mut result.water
                } else {
                    &mut result.opaque
                };

                for face in &FACES {
                    let neighbor = std::array::from_fn(|axis| local[axis] + face.offset[axis]);
                    let (neighbor_block, light) = if (0..size[0]).contains(&neighbor[0])
                        && (0..size[1]).contains(&neighbor[1])
                        && (0..size[2]).contains(&neighbor[2])
                    {
                        columns[(neighbor[2] * size[0] + neighbor[0]) as usize].at(neighbor[1])
                    } else {
                        boundary.sample(neighbor, column.light[0], &mut outside_block)
                    };
                    let light = coastal.as_deref_mut().map_or(light, |coastal| {
                        let sample =
                            std::array::from_fn(|axis| origin[axis].checked_add(neighbor[axis]));
                        if let [Some(x), Some(y), Some(z)] = sample {
                            coastal.at([x, y, z], light)
                        } else {
                            light
                        }
                    });

                    if is_water
                        && let Some(water) = water.as_deref_mut()
                        && let Some(run) = water.at(world)
                    {
                        let lower = world[1] as f32;
                        let upper = (lower + 1.0).min(run.sample.surface);
                        let mut visible_lower = lower;
                        // A fractional cap is separated from the ceiling voxel
                        // even though voxel adjacency alone reports solid contact.
                        let mut visible = neighbor_block == Block::Air
                            || (face.offset[1] == 1
                                && neighbor_block.is_opaque()
                                && upper < lower + 1.0);
                        if neighbor_block == Block::Water && face.offset[1] == 0 {
                            let neighbor_world = std::array::from_fn(|axis| {
                                world[axis].checked_add(face.offset[axis])
                            });
                            if let [Some(x), Some(y), Some(z)] = neighbor_world
                                && let Some(neighbor_run) = water.at([x, y, z])
                                && neighbor_run.sample.surface < upper
                            {
                                visible = true;
                                visible_lower = lower.max(neighbor_run.sample.surface);
                            }
                        }
                        if visible && upper > visible_lower {
                            let first = mesh.vertices.len();
                            mesh.push_water_face(
                                world,
                                face,
                                light,
                                run,
                                [visible_lower, upper],
                                water,
                            );
                            if let Some(field) = lanterns {
                                let local_light = field.sample(world);
                                for vertex in &mut mesh.vertices[first..] {
                                    vertex.local_light = local_light;
                                }
                            }
                        }
                        continue;
                    }

                    let visible = if is_water {
                        neighbor_block == Block::Air
                    } else {
                        !neighbor_block.is_opaque()
                    };
                    if visible {
                        let outdoor_overhang = coastal.as_ref().is_some_and(|coastal| {
                            let air = std::array::from_fn(|axis| world[axis].checked_add(face.offset[axis]));
                            coastal.outdoor(world, light) || matches!(air,[Some(x),Some(y),Some(z)] if coastal.outdoor([x,y,z],light))
                        });
                        let first = mesh.vertices.len();
                        mesh.push_face(
                            world,
                            face,
                            material_for_face(block, face.offset[1]),
                            light,
                        );
                        if outdoor_overhang {
                            for vertex in &mut mesh.vertices[first..] {
                                vertex.water_extra[0] = 1.0;
                            }
                        }
                        if let Some(field) = lanterns {
                            let air = std::array::from_fn(|axis| {
                                world[axis].checked_add(face.offset[axis])
                            });
                            if let [Some(x), Some(y), Some(z)] = air {
                                let light = field.sample([x, y, z]);
                                for vertex in &mut mesh.vertices[first..] {
                                    vertex.local_light = light;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests;
