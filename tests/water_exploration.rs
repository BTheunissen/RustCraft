//! CPU public-API checks across generated hydrology, resident metadata, meshes, and swimming.

use glam::Vec3;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::{Mesh, Vertex, mesh_chunk};
use rustcraft::phase::{PhaseWorld, UNKNOWN_VOXEL};
use rustcraft::player::{MovementInput, PLAYER_HALF_WIDTH, PLAYER_HEIGHT, Player};
use rustcraft::world::{
    Block, CHUNK_SIZE, CaveBiome, Chunk, ChunkCoord, MIN_WORLD_Y, SEA_LEVEL, Terrain, WORLD_HEIGHT,
    WaterBiome, WaterLandmark, WaterSample,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const DT: f32 = 1.0 / 120.0;
const METADATA_TOLERANCE: f32 = 0.0001;
const SURFACE_BIOMES: [WaterBiome; 5] = [
    WaterBiome::Lake,
    WaterBiome::River,
    WaterBiome::Estuary,
    WaterBiome::Ocean,
    WaterBiome::Waterfall,
];

fn seeds() -> [u64; 4] {
    [Terrain::default().seed(), 42, 1, u64::MAX]
}

struct ResidentWorld {
    terrain: Terrain,
    chunks: RefCell<BTreeMap<ChunkCoord, Chunk>>,
}

impl ResidentWorld {
    fn new(terrain: Terrain) -> Self {
        Self {
            terrain,
            chunks: RefCell::new(BTreeMap::new()),
        }
    }

    fn block(&self, [x, y, z]: [i32; 3]) -> Block {
        let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        self.chunks
            .borrow_mut()
            .entry(coord)
            .or_insert_with(|| Chunk::generate(&self.terrain, coord))
            .block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    }

    fn water(&self, x: i32, y: i32, z: i32) -> Option<WaterSample> {
        self.chunks
            .borrow()
            .get(&ChunkCoord::new(
                x.div_euclid(CHUNK_SIZE),
                z.div_euclid(CHUNK_SIZE),
            ))?
            .water(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    }

    fn load_neighborhood(&self, [x, z]: [i32; 2]) {
        let center = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let coord = ChunkCoord::new(center.x + dx, center.z + dz);
                self.chunks
                    .borrow_mut()
                    .entry(coord)
                    .or_insert_with(|| Chunk::generate(&self.terrain, coord));
            }
        }
    }

    fn advance(&self, player: &mut Player, input: MovementInput, steps: usize) {
        for _ in 0..steps {
            player.update_with_water(
                input,
                DT,
                |x, y, z| self.block([x, y, z]).is_solid(),
                |x, y, z| self.water(x, y, z),
            );
            assert!(player.position.is_finite());
            assert!(
                body_is_clear(player.position, |point| self.block(point)),
                "embedded feet={:?}",
                player.position
            );
        }
    }
}

fn landmark(terrain: &Terrain, biome: WaterBiome) -> WaterLandmark {
    let landmark = terrain.water_landmark(biome).unwrap_or_else(|| {
        panic!(
            "seed {}, missing nearby {} landmark",
            terrain.seed(),
            biome.name()
        )
    });
    assert_eq!(landmark.biome, biome);
    let feet = Vec3::from_array(landmark.position);
    let spawn = Vec3::from_array(terrain.spawn_position());
    assert!(feet.is_finite());
    assert!(Vec3::from_array(landmark.target).is_finite());
    assert!((feet - spawn).with_y(0.0).length() <= 640.0);
    let [x, y, z] = landmark.water;
    assert_eq!(terrain.block_at(x, y, z), Block::Water);
    assert_eq!(terrain.water_at(x, y, z).unwrap().biome, biome);
    landmark
}

fn assert_profile(water: WaterSample) {
    assert!(
        (MIN_WORLD_Y..WORLD_HEIGHT).contains(&water.bottom),
        "{water:?}"
    );
    assert!(water.surface.is_finite());
    assert!(water.surface > (water.bottom + 1) as f32);
    assert!(water.surface <= WORLD_HEIGHT as f32);
    assert!(water.flow.into_iter().all(f32::is_finite));
    assert!(water.salinity.is_finite() && (0.0..=1.0).contains(&water.salinity));
}

fn assert_water_eq(actual: Option<WaterSample>, expected: Option<WaterSample>) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) => {
            assert_eq!(actual.bottom, expected.bottom);
            assert_eq!(actual.biome, expected.biome);
            assert!(
                (actual.surface - expected.surface).abs() <= METADATA_TOLERANCE,
                "actual={actual:?}, expected={expected:?}"
            );
            assert!((actual.salinity - expected.salinity).abs() <= METADATA_TOLERANCE);
            for axis in 0..3 {
                assert!(
                    (actual.flow[axis] - expected.flow[axis]).abs() <= METADATA_TOLERANCE,
                    "actual={actual:?}, expected={expected:?}"
                );
            }
        }
        _ => panic!("water occupancy mismatch: actual={actual:?}, expected={expected:?}"),
    }
}

fn depth(water: WaterSample) -> f32 {
    water.surface - (water.bottom + 1) as f32
}

fn body_is_clear(feet: Vec3, block: impl Fn([i32; 3]) -> Block) -> bool {
    let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = feet + Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|coordinate| (f64::from(coordinate) + 0.00001).floor() as i32);
    let last = max
        .to_array()
        .map(|coordinate| (f64::from(coordinate) - 0.00001).floor() as i32);
    (first[0]..=last[0]).all(|x| {
        (first[2]..=last[2]).all(|z| (first[1]..=last[1]).all(|y| !block([x, y, z]).is_solid()))
    })
}

fn deepest_ocean(terrain: &Terrain, site: WaterLandmark) -> ([i32; 2], WaterSample) {
    let mut best = None;
    for dz in (-192..=192).step_by(4) {
        for dx in (-192..=192).step_by(4) {
            let point = [site.water[0] + dx, site.water[2] + dz];
            let Some(water) = terrain.water_column(point[0], point[1]) else {
                continue;
            };
            if water.biome == WaterBiome::Ocean
                && best.is_none_or(|(_, previous)| depth(water) > depth(previous))
            {
                best = Some((point, water));
            }
        }
    }
    best.unwrap_or_else(|| panic!("seed {}, no ocean near its diagnostic", terrain.seed()))
}

#[derive(Debug)]
struct RiverReach {
    visited: usize,
    estuary: bool,
    ocean: bool,
    min_surface: f32,
    max_surface: f32,
    mixed_salinity: usize,
}

fn connected_river(terrain: &Terrain, root: [i32; 2]) -> RiverReach {
    let root_water = terrain.water_column(root[0], root[1]).unwrap();
    let mut queue = VecDeque::from([root]);
    let mut seen = BTreeSet::from([root]);
    let mut result = RiverReach {
        visited: 0,
        estuary: false,
        ocean: false,
        min_surface: root_water.surface,
        max_surface: root_water.surface,
        mixed_salinity: 0,
    };
    while let Some([x, z]) = queue.pop_front() {
        let water = terrain.water_column(x, z).unwrap();
        result.visited += 1;
        result.estuary |= water.biome == WaterBiome::Estuary;
        result.ocean |= water.biome == WaterBiome::Ocean;
        result.min_surface = result.min_surface.min(water.surface);
        result.max_surface = result.max_surface.max(water.surface);
        result.mixed_salinity += usize::from(water.salinity > 0.0 && water.salinity < 1.0);
        if water.biome == WaterBiome::Ocean {
            continue;
        }
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [x + dx, z + dz];
            if (next[0] - root[0]).abs() > 640
                || (next[1] - root[1]).abs() > 640
                || seen.contains(&next)
            {
                continue;
            }
            let Some(next_water) = terrain.water_column(next[0], next[1]) else {
                continue;
            };
            if !matches!(
                next_water.biome,
                WaterBiome::River
                    | WaterBiome::Estuary
                    | WaterBiome::Ocean
                    | WaterBiome::Lake
                    | WaterBiome::Waterfall
            ) {
                continue;
            }
            let overlap = water.surface.min(next_water.surface)
                - (water.bottom.max(next_water.bottom) + 1) as f32;
            if overlap > 0.05 && seen.len() < 65_536 {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    result
}

fn waterfall_core(terrain: &Terrain, site: WaterLandmark) -> ([i32; 2], WaterSample) {
    let mut best = None;
    for dz in -12..=12 {
        for dx in -12..=12 {
            let point = [site.water[0] + dx, site.water[2] + dz];
            let Some(water) = terrain
                .water_column(point[0], point[1])
                .filter(|water| water.biome == WaterBiome::Waterfall)
            else {
                continue;
            };
            if best.is_none_or(|(_, previous)| depth(water) > depth(previous)) {
                best = Some((point, water));
            }
        }
    }
    best.unwrap_or_else(|| {
        panic!(
            "seed {}, waterfall diagnostic has no wet core",
            terrain.seed()
        )
    })
}

fn cave_survey_coords(terrain: &Terrain) -> BTreeSet<ChunkCoord> {
    let mut coords = BTreeSet::new();
    for biome in [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal] {
        let feet = terrain.cave_position(biome).unwrap();
        let center = ChunkCoord::from_world(feet[0], feet[2]);
        for dz in -1..=1 {
            for dx in -1..=1 {
                coords.insert(ChunkCoord::new(center.x + dx, center.z + dz));
            }
        }
    }
    assert!(coords.len() <= 27);
    coords
}

fn validate_geometry(mesh: &Mesh, water_pass: bool) {
    assert!(mesh.vertices.len().is_multiple_of(4));
    assert_eq!(mesh.indices.len(), mesh.vertices.len() / 4 * 6);
    for vertex in &mesh.vertices {
        assert!(
            vertex
                .position
                .iter()
                .chain(&vertex.normal)
                .chain(&vertex.uv)
                .chain(std::iter::once(&vertex.light))
                .chain(&vertex.water)
                .chain(&vertex.water_extra)
                .all(|value| value.is_finite())
        );
        assert!(
            vertex
                .uv
                .into_iter()
                .all(|value| (0.0..=1.0).contains(&value))
        );
        assert!((0.0..=1.0).contains(&vertex.light));
        assert!(vertex.material < TILE_COUNT);
        assert_eq!(vertex.material == 9, water_pass);
        if !water_pass {
            assert_eq!(vertex.water, [0.0; 4]);
            assert_eq!(vertex.water_extra, [0.0; 4]);
        }
    }
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|index| mesh.vertices.get(index as usize).unwrap());
        let ab = Vec3::from_array(b.position) - Vec3::from_array(a.position);
        let ac = Vec3::from_array(c.position) - Vec3::from_array(a.position);
        assert!(ab.cross(ac).dot(Vec3::from_array(a.normal)) > 0.0);
    }
}

fn source_voxel(face: &[Vertex; 4]) -> [i32; 3] {
    let center = face
        .iter()
        .map(|vertex| Vec3::from_array(vertex.position))
        .sum::<Vec3>()
        / 4.0;
    let sample = center - Vec3::from_array(face[0].normal) * 0.001;
    sample
        .to_array()
        .map(|coordinate| coordinate.floor() as i32)
}

fn expected_vertex_water(
    world: &ResidentWorld,
    actual: WaterSample,
    vertex: &Vertex,
) -> ([f32; 4], f32) {
    if vertex.position[1] != actual.surface {
        return (
            [
                actual.flow[0],
                actual.flow[2],
                depth(actual),
                actual.salinity,
            ],
            actual.flow[1],
        );
    }
    let [x, _, z] = vertex.position.map(|value| value.round() as i32);
    let mut count = 0;
    let mut data = [0.0; 4];
    let mut vertical = 0.0;
    for [x, z] in [[x - 1, z - 1], [x, z - 1], [x - 1, z], [x, z]] {
        let sample = ((actual.surface - 1.0).ceil() as i32 - 1
            ..=(actual.surface + 1.0).ceil() as i32 - 1)
            .find_map(|y| {
                if world.block([x, y, z]) != Block::Water
                    || world.block([x, y + 1, z]) != Block::Air
                {
                    return None;
                }
                world
                    .water(x, y, z)
                    .filter(|sample| (sample.surface - actual.surface).abs() <= 1.0)
            });
        let Some(sample) = sample else { continue };
        count += 1;
        for (sum, value) in data.iter_mut().zip([
            sample.flow[0],
            sample.flow[2],
            depth(sample),
            sample.salinity,
        ]) {
            *sum += value;
        }
        vertical += sample.flow[1];
    }
    assert!(
        count > 0,
        "wet vertex lacks actual exposed corner contributors: {vertex:?}"
    );
    (
        data.map(|value| value / count as f32),
        vertical / count as f32,
    )
}

#[path = "water_exploration/drainage.rs"]
mod drainage;
#[path = "water_exploration/generation.rs"]
mod generation;
#[path = "water_exploration/geometry.rs"]
mod geometry;
#[path = "water_exploration/movement.rs"]
mod movement;
