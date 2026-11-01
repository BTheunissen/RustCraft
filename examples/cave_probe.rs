//! CPU-only cave diagnostics: `cargo run --locked --example cave_probe -- --seed 42`.

use glam::Vec3;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::{Mesh, mesh_chunk};
use rustcraft::player::{EYE_HEIGHT, MovementInput, PLAYER_HALF_WIDTH, Player};
use rustcraft::world::{
    Block, CHUNK_SIZE, CaveBiome, CaveLandform, CaveLandmark, Chunk, ChunkCoord, MIN_WORLD_Y,
    Terrain, WORLD_DEPTH, WORLD_HEIGHT,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::time::{Duration, Instant};

const BIOMES: [CaveBiome; 3] = [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal];
const LANDFORMS: [CaveLandform; 4] = [
    CaveLandform::Gallery,
    CaveLandform::Rift,
    CaveLandform::Cathedral,
    CaveLandform::Labyrinth,
];
const GEOLOGY: [Block; 4] = [
    Block::Limestone,
    Block::Slate,
    Block::Basalt,
    Block::Calcite,
];
const DIAGNOSTIC_HEADROOM: f32 = 3.4;
const GEOMETRY_RADIUS: i32 = 20;
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
const NEIGHBORS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

#[path = "cave_probe/features.rs"]
mod features;
#[path = "cave_probe/mesh.rs"]
mod mesh;
#[path = "cave_probe/morphology.rs"]
mod morphology;
#[path = "cave_probe/report.rs"]
mod report;
#[path = "cave_probe/survey.rs"]
mod survey;

use features::{Connectivity, Features};
#[cfg(test)]
use features::{broad_pool_surface, formation_is_supported, material_is_attached};
use mesh::MeshSurvey;
use morphology::Morphology;
#[cfg(test)]
use morphology::{air_intervals, air_run, enclosed_rock_islands};
use report::run;
use survey::{Survey, block_center, clear_ray_between, clear_ray_to};
#[cfg(test)]
use survey::{clear_landmark_ray, dry_headroom, survey_coords};

fn seed_argument(arguments: &[String]) -> Result<Option<u64>, String> {
    match arguments {
        [] => Ok(Some(Terrain::default().seed())),
        [flag] if flag == "--help" || flag == "-h" => Ok(None),
        [flag, seed] if flag == "--seed" => seed
            .parse()
            .map(Some)
            .map_err(|_| "--seed requires an unsigned integer".into()),
        _ => Err("usage: cave_probe [--seed NUMBER]".into()),
    }
}

fn main() -> Result<(), String> {
    let Some(seed) = seed_argument(&std::env::args().skip(1).collect::<Vec<_>>())? else {
        println!("CPU-only cave survey: cave_probe [--seed NUMBER]");
        return Ok(());
    };
    run(seed)
}

#[cfg(test)]
#[path = "cave_probe/tests.rs"]
mod tests;
