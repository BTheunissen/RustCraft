//! Bounded CPU-only hydrology diagnostics: `cargo run --locked --example water_probe`.

use glam::Vec3;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::{Mesh, mesh_chunk};
use rustcraft::world::{
    Block, CHUNK_SIZE, CaveBiome, Chunk, ChunkCoord, MIN_WORLD_Y, SEA_LEVEL, Terrain, WORLD_HEIGHT,
    WaterBiome, WaterLandmark, WaterSample,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

const SEARCH_RADIUS: i32 = 640;
const COARSE_STEP: i32 = 8;
const LOCAL_RADIUS: i32 = 16;
const SURFACE_BIOMES: [WaterBiome; 5] = [
    WaterBiome::Lake,
    WaterBiome::River,
    WaterBiome::Estuary,
    WaterBiome::Ocean,
    WaterBiome::Waterfall,
];
const WATER_BIOMES: [WaterBiome; 6] = [
    WaterBiome::Lake,
    WaterBiome::River,
    WaterBiome::Estuary,
    WaterBiome::Ocean,
    WaterBiome::Waterfall,
    WaterBiome::CavePool,
];

#[path = "water_probe/mesh.rs"]
mod mesh;
#[path = "water_probe/profiles.rs"]
mod profiles;
#[path = "water_probe/survey.rs"]
mod survey;

use mesh::mesh_materials;
#[cfg(test)]
use profiles::range;
use profiles::{Profiles, quantiles, validate_sample};
use survey::{RegionalCoverage, Survey, survey_coords, validate_landmark};

fn seed_argument(arguments: &[String]) -> Result<Option<u64>, String> {
    match arguments {
        [] => Ok(Some(Terrain::default().seed())),
        [flag] if flag == "--help" || flag == "-h" => Ok(None),
        [flag, seed] if flag == "--seed" => seed
            .parse()
            .map(Some)
            .map_err(|_| "--seed requires an unsigned integer".into()),
        _ => Err("usage: water_probe [--seed NUMBER]".into()),
    }
}

fn main() -> Result<(), String> {
    let Some(seed) = seed_argument(&std::env::args().skip(1).collect::<Vec<_>>())? else {
        println!("CPU-only water survey: water_probe [--seed NUMBER]");
        return Ok(());
    };
    run(seed)
}

fn run(seed: u64) -> Result<(), String> {
    let terrain = Terrain::new(seed);
    let started = Instant::now();
    let spawn = Vec3::from_array(terrain.spawn_position());
    let mut landmarks = Vec::new();
    for biome in SURFACE_BIOMES {
        let landmark = terrain
            .water_landmark(biome)
            .ok_or_else(|| format!("seed {seed} has no nearby actual {} landmark", biome.name()))?;
        let water = validate_landmark(&terrain, spawn, biome, landmark)?;
        landmarks.push((landmark, water));
    }
    let cave_feet = terrain
        .cave_position(CaveBiome::Lush)
        .map(Vec3::from_array)
        .ok_or_else(|| "no nearby lush cave pose for the retained pool survey".to_string())?;
    let lookup = started.elapsed();
    let positions: Vec<_> = landmarks.iter().map(|(landmark, _)| *landmark).collect();
    let coords = survey_coords(&positions, cave_feet, spawn);
    if !(45..=60).contains(&coords.len()) {
        return Err(format!(
            "water probe exceeded its 45..=60 chunk budget: {}",
            coords.len()
        ));
    }
    let (survey, generation) = Survey::generate(&terrain, coords);
    println!(
        "seed={seed} spawn feet={:?}; {} distinct CPU chunks; diagnostic lookup={:.3} ms (excluded from generation and meshing)",
        spawn.to_array(),
        survey.chunks.len(),
        lookup.as_secs_f64() * 1_000.0,
    );
    let started = Instant::now();
    let profiles = survey.inspect()?;
    let inspection = started.elapsed();
    for (biome, profile) in WATER_BIOMES.into_iter().zip(&profiles) {
        profile.report(biome.name());
    }
    if profiles[WaterBiome::CavePool as usize].columns == 0 {
        return Err("surveyed lush cave region has no retained supported freshwater pool".into());
    }
    let mut materials = [0; TILE_COUNT as usize];
    let mut geometry = [[0_usize; 2]; 2];
    let mut meshing = Duration::ZERO;
    for chunk in survey.chunks.values() {
        let started = Instant::now();
        let mesh = mesh_chunk(chunk, &terrain);
        meshing += started.elapsed();
        for (pass, (mesh, water)) in [(&mesh.opaque, false), (&mesh.water, true)]
            .into_iter()
            .enumerate()
        {
            let counts = mesh_materials(mesh, water)?;
            for (total, count) in materials.iter_mut().zip(counts) {
                *total += count;
            }
            geometry[pass][0] += mesh.vertices.len();
            geometry[pass][1] += mesh.indices.len() / 3;
        }
    }
    let count = survey.chunks.len() as f64;
    println!(
        "CPU generation={:.3} ms ({:.3}/chunk), meshing={:.3} ms ({:.3}/chunk), resident profile inspection={:.3} ms; opaque/water vertices-triangles={geometry:?}; sand/gravel/clay atlas vertices={}/{}/{}",
        generation.as_secs_f64() * 1_000.0,
        generation.as_secs_f64() * 1_000.0 / count,
        meshing.as_secs_f64() * 1_000.0,
        meshing.as_secs_f64() * 1_000.0 / count,
        inspection.as_secs_f64() * 1_000.0,
        materials[4],
        materials[22],
        materials[23],
    );
    for (landmark, water) in landmarks {
        println!(
            "{} diagnostic: feet={:?}, target={:?}, actual wet voxel={:?}; bed={}, mean surface={:.3}, flow={:?}, salinity={:.3}; inland level offset from sea={:.3}",
            landmark.biome.name(),
            landmark.position,
            landmark.target,
            landmark.water,
            water.bottom,
            water.surface,
            water.flow,
            water.salinity,
            water.surface - SEA_LEVEL as f32,
        );
        survey
            .local_profile(&terrain, landmark)?
            .report("dense 33x33 local footprint");
    }
    println!(
        "retained cave-pool survey centered at feet={:?}",
        cave_feet.to_array()
    );
    let started = Instant::now();
    let coarse =
        RegionalCoverage::inspect(&terrain, [spawn.x.floor() as i32, spawn.z.floor() as i32])?;
    coarse.report();
    println!(
        "coarse procedural lookup/coverage={:.3} ms (not part of resident generation or meshing)",
        started.elapsed().as_secs_f64() * 1_000.0
    );
    println!(
        "supported beds and CPU mesh metadata verified; visual water color/foam/depth, GPU waves, FPS, and physical input are not verified by this probe"
    );
    Ok(())
}

#[cfg(test)]
#[path = "water_probe/tests.rs"]
mod tests;
