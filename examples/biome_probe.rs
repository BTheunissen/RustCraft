//! CPU-only terrain diagnostics: `cargo run --locked --example biome_probe`.

use glam::Vec3;
use rustcraft::LaunchOptions;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::{Mesh, mesh_chunk};
use rustcraft::player::{MovementInput, PLAYER_HALF_WIDTH, PLAYER_HEIGHT, Player};
use rustcraft::world::{
    Biome, Block, CHUNK_SIZE, Chunk, ChunkCoord, Column, Terrain, WORLD_HEIGHT,
};
use std::time::Instant;

const SEARCH_RADIUS: i32 = 400;
const MATERIALS: [&str; TILE_COUNT as usize] = [
    "grass_top",
    "grass_side",
    "dirt",
    "stone",
    "sand",
    "snow",
    "wood_side",
    "wood_end",
    "leaves",
    "water",
    "bedrock",
    "deepstone",
    "coal_ore",
    "iron_ore",
    "gold_ore",
    "crystal",
    "moss",
    "lava",
    "limestone",
    "slate",
    "basalt",
    "calcite",
    "gravel",
    "clay",
    "planks",
    "dressed_limestone",
    "storage_crate",
    "lantern",
];
const BIOMES: [Biome; 4] = [Biome::Plains, Biome::Forest, Biome::Desert, Biome::Alpine];

#[derive(Debug)]
struct Coverage {
    columns: [usize; 4],
    trunks: [usize; 4],
    surfaces: [usize; Block::COUNT],
    min_height: i32,
    max_height: i32,
}

impl Default for Coverage {
    fn default() -> Self {
        Self {
            columns: [0; 4],
            trunks: [0; 4],
            surfaces: [0; Block::COUNT],
            min_height: WORLD_HEIGHT,
            max_height: 0,
        }
    }
}

impl Coverage {
    fn record(&mut self, column: Column, surface: Block, above_surface: Block) {
        let index = match column.biome {
            Biome::Plains => 0,
            Biome::Forest => 1,
            Biome::Desert => 2,
            Biome::Alpine => 3,
        };
        self.columns[index] += 1;
        self.trunks[index] += usize::from(above_surface == Block::Wood);
        self.surfaces[surface as usize] += 1;
        self.min_height = self.min_height.min(column.height);
        self.max_height = self.max_height.max(column.height);
    }
}

fn main() -> Result<(), String> {
    run(Terrain::default())
}

fn run(terrain: Terrain) -> Result<(), String> {
    let spawn = terrain.spawn_position();
    let origin = [spawn[0].floor() as i32, spawn[2].floor() as i32];
    let center = ChunkCoord::from_world(spawn[0], spawn[2]);
    let radius = LaunchOptions::default().render_distance;
    validate_landing(&terrain, Vec3::from_array(spawn))?;
    println!("seed={} spawn feet={spawn:?}", terrain.seed());

    let started = Instant::now();
    let coverage = inspect_region(&terrain, center, radius);
    let total: usize = coverage.columns.iter().sum();
    if coverage.columns.contains(&0)
        || [Block::Grass, Block::Sand, Block::Snow]
            .into_iter()
            .any(|block| coverage.surfaces[block as usize] == 0)
    {
        return Err(
            "initial resident region lacks an actual biome or expected surface material".into(),
        );
    }
    println!(
        "initial region: {} chunks, x={}..{}, z={}..{}, terrain heights={}..{} ({:.1} ms CPU survey)",
        (radius * 2 + 1).pow(2),
        (center.x - radius) * CHUNK_SIZE,
        (center.x + radius + 1) * CHUNK_SIZE - 1,
        (center.z - radius) * CHUNK_SIZE,
        (center.z + radius + 1) * CHUNK_SIZE - 1,
        coverage.min_height,
        coverage.max_height,
        started.elapsed().as_secs_f64() * 1_000.0,
    );
    for (index, biome) in BIOMES.iter().enumerate() {
        let columns = coverage.columns[index];
        let roots = coverage.trunks[index];
        println!(
            "  {}: {columns} columns ({:.1}%), {roots} tree roots ({:.2}/1024 columns)",
            biome.name(),
            columns as f64 / total as f64 * 100.0,
            roots as f64 / columns.max(1) as f64 * 1_024.0,
        );
    }
    print!("  terrain surface columns:");
    for (block, count) in Block::ALL.iter().zip(coverage.surfaces) {
        if count > 0 {
            print!(" {block:?}={count}");
        }
    }
    println!();

    for (biome, surface, required_materials) in [
        (Biome::Plains, Block::Grass, &[0][..]),
        (Biome::Forest, Block::Grass, &[0, 8][..]),
        (Biome::Desert, Block::Sand, &[4][..]),
        (Biome::Alpine, Block::Snow, &[5][..]),
    ] {
        let point = nearest_matching(origin, SEARCH_RADIUS, |point| {
            representative_site(&terrain, point, biome, surface)
        })
        .ok_or_else(|| {
            format!(
                "no clear {} site within {SEARCH_RADIUS} blocks",
                biome.name()
            )
        })?;
        report_site(&terrain, origin, point, biome.name(), required_materials)?;
    }

    let water = nearest_matching(origin, SEARCH_RADIUS, |[x, z]| {
        let column = terrain.column(x, z);
        terrain.water_column(x, z).is_some_and(|water| {
            terrain.block_at(x, water.surface.ceil() as i32 - 1, z) == Block::Water
                && terrain.block_at(x, column.height, z).is_solid()
        })
    })
    .ok_or_else(|| format!("no water within {SEARCH_RADIUS} blocks"))?;
    report_site(&terrain, origin, water, "Nearest water", &[9])?;
    if let Some(profile) = terrain.water_column(water[0], water[1]) {
        println!(
            "  {} mean water surface y={:.3}, salinity={:.3}; legacy solidity-only landing is on the bed, not a swimming test",
            profile.biome.name(),
            profile.surface,
            profile.salinity,
        );
    }
    println!("all sampled meshes valid; spawn and sample flight-to-walk landings grounded");
    Ok(())
}

fn inspect_region(terrain: &Terrain, center: ChunkCoord, radius: i32) -> Coverage {
    let mut coverage = Coverage::default();
    for chunk_z in center.z - radius..=center.z + radius {
        for chunk_x in center.x - radius..=center.x + radius {
            let chunk = Chunk::generate(terrain, ChunkCoord::new(chunk_x, chunk_z));
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let column = terrain.column(chunk_x * CHUNK_SIZE + x, chunk_z * CHUNK_SIZE + z);
                    coverage.record(
                        column,
                        chunk.block(x, column.height, z),
                        chunk.block(x, column.height + 1, z),
                    );
                }
            }
        }
    }
    coverage
}

fn nearest_matching(
    center: [i32; 2],
    radius: i32,
    mut accepts: impl FnMut([i32; 2]) -> bool,
) -> Option<[i32; 2]> {
    if radius < 0 {
        return None;
    }
    let mut best = None;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let distance = i64::from(dx).pow(2) + i64::from(dz).pow(2);
            if distance > i64::from(radius).pow(2)
                || best.is_some_and(|(_, best_distance)| distance >= best_distance)
            {
                continue;
            }
            let (Some(x), Some(z)) = (center[0].checked_add(dx), center[1].checked_add(dz)) else {
                continue;
            };
            if accepts([x, z]) {
                best = Some(([x, z], distance));
            }
        }
    }
    best.map(|(point, _)| point)
}

fn representative_site(terrain: &Terrain, [x, z]: [i32; 2], biome: Biome, surface: Block) -> bool {
    // Chunk-center sites with matching corners avoid photographing only a biome seam.
    let half = CHUNK_SIZE / 2;
    if x.rem_euclid(CHUNK_SIZE) != half || z.rem_euclid(CHUNK_SIZE) != half {
        return false;
    }
    let column = terrain.column(x, z);
    if column.biome != biome || terrain.block_at(x, column.height, z) != surface {
        return false;
    }
    for dz in [-half, half - 1] {
        for dx in [-half, half - 1] {
            if terrain.column(x + dx, z + dz).biome != biome {
                return false;
            }
        }
    }
    let feet = Vec3::new(x as f32 + 0.5, (column.height + 1) as f32, z as f32 + 0.5);
    if !landing_descent_clear(feet, |x, y, z| terrain.block_at(x, y, z)) {
        return false;
    }
    let mut player = Player::new(feet);
    let solid = |x, y, z| terrain.block_at(x, y, z).is_solid();
    player.toggle_flight(solid);
    player.toggle_flight(solid);
    !player.flying && player.position == feet && player.is_grounded()
}

fn validate_landing(terrain: &Terrain, feet: Vec3) -> Result<(), String> {
    let mut player = Player::new(feet + Vec3::Y * 4.0);
    let solid = |x, y, z| terrain.block_at(x, y, z).is_solid();
    player.toggle_flight(solid);
    player.toggle_flight(solid);
    for _ in 0..120 {
        player.update(MovementInput::default(), 1.0 / 60.0, solid);
    }
    if player.flying || !player.is_grounded() || player.position.distance(feet) > 0.001 {
        return Err(format!("unsafe landing at {feet:?}: {:?}", player.position));
    }
    Ok(())
}

fn landing_descent_clear(feet: Vec3, block: impl Fn(i32, i32, i32) -> Block) -> bool {
    if !feet.is_finite() {
        return false;
    }
    let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = feet + Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT + 4.0, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|coordinate| (f64::from(coordinate) + 0.000_01).floor() as i32);
    let last = max
        .to_array()
        .map(|coordinate| (f64::from(coordinate) - 0.000_01).floor() as i32);
    (first[0]..=last[0]).all(|x| {
        (first[2]..=last[2]).all(|z| (first[1]..=last[1]).all(|y| !block(x, y, z).is_solid()))
    })
}

fn material_counts(mesh: &Mesh, water: bool) -> Result<[usize; TILE_COUNT as usize], String> {
    if !mesh.indices.len().is_multiple_of(3) {
        return Err("incomplete triangle indices".into());
    }
    let mut counts = [0; TILE_COUNT as usize];
    for vertex in &mesh.vertices {
        if !vertex
            .position
            .iter()
            .chain(&vertex.normal)
            .chain(&vertex.uv)
            .chain(std::iter::once(&vertex.light))
            .chain(&vertex.water)
            .chain(&vertex.water_extra)
            .all(|value| value.is_finite())
        {
            return Err("non-finite vertex data".into());
        }
        if !(0.0..=1.0).contains(&vertex.light) {
            return Err("vertex light outside normalized range".into());
        }
        let count = counts
            .get_mut(vertex.material as usize)
            .ok_or_else(|| format!("invalid atlas material {}", vertex.material))?;
        if (vertex.material == 9) != water {
            return Err("material in the wrong opaque/water draw pass".into());
        }
        *count += 1;
    }
    for indices in mesh.indices.as_chunks::<3>().0 {
        let [Some(a), Some(b), Some(c)] = indices.map(|index| mesh.vertices.get(index as usize))
        else {
            return Err("triangle index outside vertex buffer".into());
        };
        let cross = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
            .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
        if cross.dot(Vec3::from_array(a.normal)) <= 0.0 {
            return Err("degenerate or inward triangle".into());
        }
    }
    Ok(counts)
}

fn report_site(
    terrain: &Terrain,
    origin: [i32; 2],
    [x, z]: [i32; 2],
    label: &str,
    required_materials: &[usize],
) -> Result<(), String> {
    let column = terrain.column(x, z);
    let feet = Vec3::new(x as f32 + 0.5, (column.height + 1) as f32, z as f32 + 0.5);
    validate_landing(terrain, feet)?;
    let coord = ChunkCoord::from_world(feet.x, feet.z);
    let started = Instant::now();
    let chunk = Chunk::generate(terrain, coord);
    let generation = started.elapsed();
    let started = Instant::now();
    let mesh = mesh_chunk(&chunk, terrain);
    let meshing = started.elapsed();
    let opaque = material_counts(&mesh.opaque, false)?;
    let water = material_counts(&mesh.water, true)?;
    for &material in required_materials {
        if opaque[material] + water[material] == 0 {
            return Err(format!("{label} mesh lacks {}", MATERIALS[material]));
        }
    }
    let distance = ((x - origin[0]).pow(2) + (z - origin[1]).pow(2)) as f64;
    println!(
        "{label}: block=({x}, {z}), feet={:?}, height={}, chunk=({}, {}), {:.1} blocks from spawn",
        feet.to_array(),
        column.height,
        coord.x,
        coord.z,
        distance.sqrt(),
    );
    println!(
        "  opaque={} vertices/{} triangles, water={} vertices/{} triangles; generation={:.3} ms, meshing={:.3} ms",
        mesh.opaque.vertices.len(),
        mesh.opaque.indices.len() / 3,
        mesh.water.vertices.len(),
        mesh.water.indices.len() / 3,
        generation.as_secs_f64() * 1_000.0,
        meshing.as_secs_f64() * 1_000.0,
    );
    print!("  material vertex counts:");
    for (id, name) in MATERIALS.iter().enumerate() {
        let count = opaque[id] + water[id];
        if count > 0 {
            print!(" {id}:{name}={count}");
        }
    }
    println!();
    Ok(())
}

#[cfg(test)]
#[path = "biome_probe/tests.rs"]
mod tests;
