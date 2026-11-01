//! Public-API checks across generated caves, meshing, and the walking collider.

use glam::Vec3;
use rustcraft::atlas::TILE_COUNT;
use rustcraft::mesh::{Mesh, mesh_chunk};
use rustcraft::player::{MovementInput, PLAYER_HALF_WIDTH, PLAYER_HEIGHT, Player};
use rustcraft::world::{
    Block, CHUNK_SIZE, CaveBiome, CaveLandform, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain,
    WORLD_HEIGHT,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

const DT: f32 = 1.0 / 120.0;
const POSITION_TOLERANCE: f32 = 0.002;
const CAVE_BIOMES: [CaveBiome; 3] = [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal];
const UNDERGROUND_MATERIALS: [(Block, usize); 11] = [
    (Block::CoalOre, 12),
    (Block::IronOre, 13),
    (Block::GoldOre, 14),
    (Block::Crystal, 15),
    (Block::Moss, 16),
    (Block::Water, 9),
    (Block::Lava, 17),
    (Block::Limestone, 18),
    (Block::Slate, 19),
    (Block::Basalt, 20),
    (Block::Calcite, 21),
];

struct GeneratedWorld {
    terrain: Terrain,
    chunks: RefCell<BTreeMap<ChunkCoord, Chunk>>,
}

impl GeneratedWorld {
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

    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.block([x, y, z]).is_solid()
    }

    fn poses(&self) -> Vec<(CaveBiome, Vec3)> {
        CAVE_BIOMES
            .into_iter()
            .map(|biome| {
                let feet = self.terrain.cave_position(biome).unwrap_or_else(|| {
                    panic!(
                        "seed {} has no nearby safe {} pose",
                        self.terrain.seed(),
                        biome.name()
                    )
                });
                (biome, Vec3::from_array(feet))
            })
            .collect()
    }

    fn feature_coords(&self) -> BTreeSet<ChunkCoord> {
        let sites: Vec<_> = self
            .poses()
            .into_iter()
            .map(|(_, feet)| feet)
            .chain(
                [
                    CaveLandform::Gallery,
                    CaveLandform::Rift,
                    CaveLandform::Cathedral,
                    CaveLandform::Labyrinth,
                ]
                .into_iter()
                .map(|kind| {
                    Vec3::from_array(
                        self.terrain
                            .cave_landmark(kind)
                            .unwrap_or_else(|| {
                                panic!(
                                    "seed {}, missing {} landmark",
                                    self.terrain.seed(),
                                    kind.name()
                                )
                            })
                            .position,
                    )
                }),
            )
            .collect();
        let mut coords = BTreeSet::new();
        for feet in &sites {
            let center = ChunkCoord::from_world(feet.x, feet.z);
            for z in center.z - 1..=center.z + 1 {
                for x in center.x - 1..=center.x + 1 {
                    coords.insert(ChunkCoord::new(x, z));
                }
            }
        }
        for position in
            std::iter::once(self.terrain.spawn_position()).chain(self.terrain.entrance_position())
        {
            coords.insert(ChunkCoord::from_world(position[0], position[2]));
        }
        if coords.len() < 45 {
            let mut extra = BTreeSet::new();
            for feet in sites {
                let center = ChunkCoord::from_world(feet.x, feet.z);
                for z in center.z - 3..=center.z + 3 {
                    for x in center.x - 3..=center.x + 3 {
                        extra.insert(ChunkCoord::new(x, z));
                    }
                }
            }
            for coord in extra {
                coords.insert(coord);
                if coords.len() >= 45 {
                    break;
                }
            }
        }
        assert!((45..=65).contains(&coords.len()));
        coords
    }
}

fn body_is_clear(feet: Vec3, block: impl Fn([i32; 3]) -> Block) -> bool {
    let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = feet + Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|coordinate| (f64::from(coordinate) + 0.000_01).floor() as i32);
    let last = max
        .to_array()
        .map(|coordinate| (f64::from(coordinate) - 0.000_01).floor() as i32);
    (first[0]..=last[0]).all(|x| {
        (first[1]..=last[1]).all(|y| (first[2]..=last[2]).all(|z| !block([x, y, z]).is_solid()))
    })
}

fn assert_safe(world: &GeneratedWorld, player: &Player) {
    assert!(player.position.is_finite());
    assert!(
        body_is_clear(player.position, |point| world.block(point)),
        "seed {}, collider embedded at {:?}",
        world.terrain.seed(),
        player.position
    );
}

fn settle(world: &GeneratedWorld, player: &mut Player) {
    for _ in 0..240 {
        player.update(MovementInput::default(), DT, |x, y, z| world.solid(x, y, z));
        assert_safe(world, player);
    }
    assert!(player.is_grounded(), "feet={:?}", player.position);
    assert!(!player.flying);
}

fn assert_position(actual: Vec3, expected: Vec3) {
    assert!(
        actual.distance(expected) < POSITION_TOLERANCE,
        "expected {expected:?}, got {actual:?}"
    );
}

fn safe_floor(world: &GeneratedWorld, feet: Vec3) -> Block {
    world.block([
        feet.x.floor() as i32,
        (feet.y - 0.001).floor() as i32,
        feet.z.floor() as i32,
    ])
}

fn nearby_flat_walk(world: &GeneratedWorld, feet: Vec3) -> Option<(Vec3, Vec3)> {
    let [x, y, z] = feet.to_array().map(|coordinate| coordinate.floor() as i32);
    for radius in 0_i32..=8 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                let start = Vec3::new((x + dx) as f32 + 0.5, y as f32, (z + dz) as f32 + 0.5);
                for direction in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                    if (0..=8).all(|step| {
                        let sample = start + direction * (step as f32 / 4.0);
                        safe_floor(world, sample).is_solid()
                            && body_is_clear(sample, |point| world.block(point))
                    }) {
                        return Some((start, direction));
                    }
                }
            }
        }
    }
    None
}

fn exposed_clusters(
    world: &GeneratedWorld,
    material: Block,
    mut points: BTreeSet<[i32; 3]>,
) -> Vec<usize> {
    let mut sizes = Vec::new();
    while let Some(root) = points.pop_first() {
        let mut queue = vec![root];
        let mut cells = 0;
        while let Some(point) = queue.pop() {
            cells += 1;
            for offset in [
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
            ] {
                let next = std::array::from_fn(|axis| point[axis] + offset[axis]);
                if points.remove(&next) {
                    assert_eq!(world.block(next), material);
                    queue.push(next);
                }
            }
        }
        sizes.push(cells);
    }
    sizes
}

fn underground_mesh_materials(mesh: &Mesh, water: bool) -> [usize; TILE_COUNT as usize] {
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
        assert!((0.0..=1.0).contains(&vertex.light));
        assert!(vertex.material < TILE_COUNT);
        assert_eq!(vertex.material == 9, water);
    }
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|index| mesh.vertices.get(index as usize).unwrap());
        let ab = Vec3::from_array(b.position) - Vec3::from_array(a.position);
        let ac = Vec3::from_array(c.position) - Vec3::from_array(a.position);
        assert!(ab.cross(ac).dot(Vec3::from_array(a.normal)) > 0.0);
    }
    let mut exposed = [0; TILE_COUNT as usize];
    for face in mesh.vertices.as_chunks::<4>().0 {
        let center_y = face.iter().map(|vertex| vertex.position[1]).sum::<f32>() / 4.0;
        if center_y >= 0.0 || (center_y == MIN_WORLD_Y as f32 && face[0].normal[1] == -1.0) {
            continue;
        }
        assert!(face.iter().all(|vertex| vertex.light == face[0].light));
        exposed[face[0].material as usize] += 1;
    }
    exposed
}

fn observed_seam_passage(world: &GeneratedWorld) -> Option<([i32; 3], [i32; 3])> {
    for (_, feet) in world.poses() {
        let center = ChunkCoord::from_world(feet.x, feet.z);
        for axis in [0, 2] {
            let chunk_axis = if axis == 0 { center.x } else { center.z };
            let other = if axis == 0 { center.z } else { center.x };
            for seam in [chunk_axis * CHUNK_SIZE, (chunk_axis + 1) * CHUNK_SIZE] {
                for horizontal in (other - 1) * CHUNK_SIZE..(other + 2) * CHUNK_SIZE {
                    for y in MIN_WORLD_Y + 4..-2 {
                        let a = if axis == 0 {
                            [seam - 1, y, horizontal]
                        } else {
                            [horizontal, y, seam - 1]
                        };
                        let b = if axis == 0 {
                            [seam, y, horizontal]
                        } else {
                            [horizontal, y, seam]
                        };
                        if [a, b].into_iter().all(|[x, y, z]| {
                            world.block([x, y - 1, z]).is_solid()
                                && world.block([x, y, z]) == Block::Air
                                && world.block([x, y + 1, z]) == Block::Air
                        }) {
                            return Some((a, b));
                        }
                    }
                }
            }
        }
    }
    None
}

#[path = "underground_exploration/acceptance.rs"]
mod acceptance;
