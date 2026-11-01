//! Read-only camera candidates validated against generated chunks.

use super::{
    Block, Bounds, CELL_SIZE, CaveBiome, CaveLandform, CaveLandmark, Chunk, ChunkCoord, Climate,
    Node, Terrain, WORLD_HEIGHT, cave_biome, cave_landform, cells, entrance, node,
    split_block_coordinate,
};

/// Diagnostic sampling only: this does not change player spawning or controls.
pub(in crate::world::generation) fn cave_position(
    terrain: &Terrain,
    biome: CaveBiome,
) -> Option<[f32; 3]> {
    let mut chunks = std::collections::BTreeMap::new();
    for node in diagnostic_nodes(terrain) {
        for [x, z] in diagnostic_columns(node) {
            for y in node.floor as i32 - 1..=node.floor as i32 + 4 {
                if y >= 0
                    || cave_biome(terrain.seed(), x, y, z) != biome
                    || cave_biome(terrain.seed(), x, y + 2, z) != biome
                    || !diagnostic_climate_core(terrain.seed(), x, y + 1, z, biome)
                {
                    continue;
                }
                let clear = diagnostic_clear(terrain, &mut chunks, [x, y, z]);
                let scenic = clear
                    && (0..16).any(|angle| {
                        let angle = f64::from(angle) * std::f64::consts::TAU / 16.0;
                        (1..=18).all(|step| {
                            let ray_x =
                                (f64::from(x) + 0.5 + angle.cos() * f64::from(step)).floor() as i32;
                            let ray_z =
                                (f64::from(z) + 0.5 + angle.sin() * f64::from(step)).floor() as i32;
                            cached_block(terrain, &mut chunks, ray_x, y + 1, ray_z) == Block::Air
                        })
                    });
                if scenic && diagnostic_walls_match(terrain, &mut chunks, x, y, z, biome) {
                    return Some([x as f32 + 0.5, y as f32, z as f32 + 0.5]);
                }
            }
        }
    }
    None
}

fn diagnostic_nodes(terrain: &Terrain) -> Vec<Node> {
    let spawn = terrain.spawn_position();
    let center_x = spawn[0].floor() as i64;
    let center_z = spawn[2].floor() as i64;
    let mut sites = Vec::new();
    for cell_z in (center_z - 230).div_euclid(CELL_SIZE)..=(center_z + 230).div_euclid(CELL_SIZE) {
        for cell_x in
            (center_x - 230).div_euclid(CELL_SIZE)..=(center_x + 230).div_euclid(CELL_SIZE)
        {
            for deep in [false, true] {
                let node = node(terrain.seed(), cell_x, cell_z, deep);
                let distance = (node.center[0] - f64::from(spawn[0])).powi(2)
                    + (node.center[2] - f64::from(spawn[2])).powi(2);
                if distance <= 230.0_f64.powi(2) {
                    sites.push((distance, node));
                }
            }
        }
    }
    sites.sort_by(|left, right| left.0.total_cmp(&right.0));
    sites.into_iter().map(|(_, node)| node).collect()
}

fn diagnostic_columns(node: Node) -> Vec<[i32; 2]> {
    let mut columns = Vec::with_capacity(35);
    for along in [0.0, -0.6, 0.6, -0.35, 0.35, -0.75, 0.75] {
        for across in [0.0, -0.4, 0.4, -0.65, 0.65] {
            let point = node.point(along * node.radius[0], across * node.radius[2], node.floor);
            columns.push([point[0].floor() as i32, point[2].floor() as i32]);
        }
    }
    columns
}

pub(super) fn diagnostic_clear(
    terrain: &Terrain,
    chunks: &mut std::collections::BTreeMap<ChunkCoord, Chunk>,
    [x, y, z]: [i32; 3],
) -> bool {
    cached_block(terrain, chunks, x, y - 1, z).is_solid()
        && (-1..=1).all(|dz| {
            (-1..=1).all(|dx| {
                (0..=3)
                    .all(|dy| cached_block(terrain, chunks, x + dx, y + dy, z + dz) == Block::Air)
            })
        })
}

pub(in crate::world::generation) fn cave_landmark(
    terrain: &Terrain,
    landform: CaveLandform,
) -> Option<CaveLandmark> {
    let spawn = terrain.spawn_position();
    let mut chunks = std::collections::BTreeMap::new();
    for node in diagnostic_nodes(terrain)
        .into_iter()
        .filter(|node| node.landform == landform)
    {
        for [x, z] in diagnostic_columns(node) {
            if (x as f32 + 0.5 - spawn[0]).hypot(z as f32 + 0.5 - spawn[2]) > 250.0 {
                continue;
            }
            for y in node.floor as i32 - 1..=node.floor as i32 + 3 {
                if !diagnostic_clear(terrain, &mut chunks, [x, y, z])
                    || cave_landform(terrain.seed(), x, y, z) != landform
                {
                    continue;
                }
                for sign in [-1.0, 1.0] {
                    for distance in [22.0, 18.0, 26.0, 15.0] {
                        let rise = match landform {
                            CaveLandform::Rift | CaveLandform::Cathedral => 7.0,
                            CaveLandform::Gallery | CaveLandform::Labyrinth => 1.0,
                        };
                        let clear_view = (1..=distance as i32).all(|step| {
                            let fraction = f64::from(step) / distance;
                            cached_block(
                                terrain,
                                &mut chunks,
                                (f64::from(x) + 0.5 + node.axis[0] * sign * f64::from(step)).floor()
                                    as i32,
                                (f64::from(y) + 1.62 + rise * fraction).floor() as i32,
                                (f64::from(z) + 0.5 + node.axis[1] * sign * f64::from(step)).floor()
                                    as i32,
                            ) == Block::Air
                        });
                        if clear_view {
                            return Some(CaveLandmark {
                                landform,
                                position: [x as f32 + 0.5, y as f32, z as f32 + 0.5],
                                target: [
                                    (f64::from(x) + 0.5 + node.axis[0] * sign * distance) as f32,
                                    (f64::from(y) + 1.62 + rise) as f32,
                                    (f64::from(z) + 0.5 + node.axis[1] * sign * distance) as f32,
                                ],
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

pub(super) fn diagnostic_climate_core(seed: u64, x: i32, y: i32, z: i32, biome: CaveBiome) -> bool {
    let climate = (Climate::new(seed, x, z).phase + f64::from(y) / 96.0).sin();
    match biome {
        CaveBiome::Rocky => climate.abs() < 0.17,
        CaveBiome::Lush => climate < -0.7,
        CaveBiome::Crystal => climate > 0.7,
    }
}

pub(super) fn diagnostic_walls_match(
    terrain: &Terrain,
    chunks: &mut std::collections::BTreeMap<ChunkCoord, Chunk>,
    x: i32,
    y: i32,
    z: i32,
    biome: CaveBiome,
) -> bool {
    let mut hits = 0;
    let mut representative = 0;
    for pitch in [-0.4_f64, 0.0, 0.4] {
        for angle in 0..16 {
            let angle = f64::from(angle) * std::f64::consts::TAU / 16.0;
            for step in 1..=32 {
                let step = f64::from(step);
                let ray_x = (f64::from(x) + 0.5 + angle.cos() * step).floor() as i32;
                let ray_y = (f64::from(y) + 1.62 + pitch * step).floor() as i32;
                let ray_z = (f64::from(z) + 0.5 + angle.sin() * step).floor() as i32;
                let block = cached_block(terrain, chunks, ray_x, ray_y, ray_z);
                if block.is_opaque() {
                    hits += 1;
                    representative += usize::from(match biome {
                        CaveBiome::Rocky => matches!(
                            block,
                            Block::Stone
                                | Block::DeepStone
                                | Block::CoalOre
                                | Block::IronOre
                                | Block::GoldOre
                                | Block::Limestone
                                | Block::Slate
                                | Block::Basalt
                                | Block::Calcite
                        ),
                        CaveBiome::Lush => block == Block::Moss,
                        CaveBiome::Crystal => block == Block::Crystal,
                    });
                    break;
                }
            }
        }
    }
    hits >= 12
        && representative * 100
            >= hits
                * match biome {
                    CaveBiome::Rocky => 70,
                    CaveBiome::Lush => 18,
                    CaveBiome::Crystal => 6,
                }
}

pub(in crate::world::generation) fn entrance_position(terrain: &Terrain) -> Option<[f32; 3]> {
    let spawn = terrain.spawn_position();
    let center_x = spawn[0].floor() as i64;
    let center_z = spawn[2].floor() as i64;
    let query = Bounds {
        min_x: spawn[0] as f64 - 240.0,
        max_x: spawn[0] as f64 + 240.0,
        min_z: spawn[2] as f64 - 240.0,
        max_z: spawn[2] as f64 + 240.0,
        min_y: -25.0,
        max_y: f64::from(WORLD_HEIGHT),
    };
    let mut entrances = Vec::new();
    for cell_z in cells(query.min_z - 205.0, query.max_z + 115.0, CELL_SIZE) {
        for cell_x in cells(query.min_x - 205.0, query.max_x + 115.0, CELL_SIZE) {
            if let Some(entrance) = entrance(terrain, cell_x, cell_z, query) {
                let distance = (entrance.mouth[0] - center_x as f64).powi(2)
                    + (entrance.mouth[2] - center_z as f64).powi(2);
                if distance <= 230.0_f64.powi(2) {
                    entrances.push((distance, entrance));
                }
            }
        }
    }
    entrances.sort_by(|left, right| left.0.total_cmp(&right.0));
    for (_, entrance) in entrances {
        for forward in [7.0, 8.0, 6.0, 9.0] {
            for side in [0.0, -1.0, 1.0, -2.0, 2.0] {
                let x = (entrance.mouth[0] + entrance.direction[0] * forward
                    - entrance.direction[1] * side)
                    .floor() as i32;
                let z = (entrance.mouth[2]
                    + entrance.direction[1] * forward
                    + entrance.direction[0] * side)
                    .floor() as i32;
                let height = terrain.column(x, z).height;
                if terrain.block_at(x, height, z).is_solid()
                    && (1..=3).all(|dy| terrain.block_at(x, height + dy, z) == Block::Air)
                {
                    return Some([x as f32 + 0.5, (height + 1) as f32, z as f32 + 0.5]);
                }
            }
        }
    }
    None
}

pub(super) fn cached_block(
    terrain: &Terrain,
    chunks: &mut std::collections::BTreeMap<ChunkCoord, Chunk>,
    x: i32,
    y: i32,
    z: i32,
) -> Block {
    let (chunk_x, local_x) = split_block_coordinate(x);
    let (chunk_z, local_z) = split_block_coordinate(z);
    let coord = ChunkCoord::new(chunk_x, chunk_z);
    chunks
        .entry(coord)
        .or_insert_with(|| Chunk::generate(terrain, coord))
        .block(local_x, y, local_z)
}
