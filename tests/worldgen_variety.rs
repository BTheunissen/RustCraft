//! Observed local cave geometry; no private generator shapes or global route search.

use glam::Vec3;
use rustcraft::player::{EYE_HEIGHT, PLAYER_HALF_WIDTH};
use rustcraft::world::{
    Block, CHUNK_SIZE, CaveBiome, CaveLandform, CaveLandmark, Chunk, ChunkCoord, MIN_WORLD_Y,
    Terrain, WORLD_HEIGHT,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

const RADIUS: i32 = 20;
const LANDFORMS: [CaveLandform; 4] = [
    CaveLandform::Gallery,
    CaveLandform::Rift,
    CaveLandform::Cathedral,
    CaveLandform::Labyrinth,
];
const NEIGHBORS: [[i32; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

struct World {
    terrain: Terrain,
    chunks: RefCell<BTreeMap<ChunkCoord, Chunk>>,
}

impl World {
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

    fn landmark(&self, kind: CaveLandform) -> CaveLandmark {
        self.terrain.cave_landmark(kind).unwrap_or_else(|| {
            panic!(
                "seed {} missing {} landmark",
                self.terrain.seed(),
                kind.name()
            )
        })
    }

    fn feature_coords(&self) -> BTreeSet<ChunkCoord> {
        let sites: Vec<_> = LANDFORMS
            .into_iter()
            .map(|kind| Vec3::from_array(self.landmark(kind).position))
            .chain(
                [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal]
                    .into_iter()
                    .map(|biome| {
                        Vec3::from_array(self.terrain.cave_position(biome).unwrap_or_else(|| {
                            panic!(
                                "seed {}, missing {} diagnostic",
                                self.terrain.seed(),
                                biome.name()
                            )
                        }))
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

struct LocalCave {
    min: [i32; 3],
    size: [i32; 3],
    blocks: Vec<Block>,
    connected: Vec<bool>,
    negative_cells: usize,
    span: [i32; 3],
}

impl LocalCave {
    fn sample(world: &World, feet: Vec3) -> Self {
        let root = feet.to_array().map(|coordinate| coordinate.floor() as i32);
        let mut cave = Self {
            min: [root[0] - RADIUS, MIN_WORLD_Y + 1, root[2] - RADIUS],
            size: [
                RADIUS * 2 + 1,
                WORLD_HEIGHT - MIN_WORLD_Y - 2,
                RADIUS * 2 + 1,
            ],
            blocks: Vec::new(),
            connected: Vec::new(),
            negative_cells: 0,
            span: [0; 3],
        };
        for z in cave.min[2]..cave.min[2] + cave.size[2] {
            for y in cave.min[1]..cave.min[1] + cave.size[1] {
                for x in cave.min[0]..cave.min[0] + cave.size[0] {
                    cave.blocks.push(world.block([x, y, z]));
                }
            }
        }
        cave.connected = vec![false; cave.blocks.len()];
        let clear = |point: [i32; 3]| {
            cave.block(point) == Block::Air
                && cave.block([point[0], point[1] + 1, point[2]]) == Block::Air
        };
        assert!(clear(root), "no standing air at {feet:?}");
        let mut seen = vec![false; cave.blocks.len()];
        seen[cave.index(root).unwrap()] = true;
        let mut queue = VecDeque::from([root]);
        let mut min = root;
        let mut max = root;
        let mut negative_cells = 0;
        while let Some(point) = queue.pop_front() {
            negative_cells += usize::from(point[1] < 0);
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
            for offset in NEIGHBORS {
                let next = std::array::from_fn(|axis| point[axis] + offset[axis]);
                if let Some(index) = cave.index(next)
                    && !seen[index]
                    && clear(next)
                {
                    seen[index] = true;
                    queue.push_back(next);
                }
            }
        }
        cave.connected = seen;
        cave.negative_cells = negative_cells;
        cave.span = std::array::from_fn(|axis| max[axis] - min[axis] + 1);
        cave
    }

    fn index(&self, point: [i32; 3]) -> Option<usize> {
        let local: [i32; 3] = std::array::from_fn(|axis| point[axis] - self.min[axis]);
        (0..3)
            .all(|axis| (0..self.size[axis]).contains(&local[axis]))
            .then_some(((local[2] * self.size[1] + local[1]) * self.size[0] + local[0]) as usize)
    }

    fn block(&self, point: [i32; 3]) -> Block {
        self.index(point)
            .map_or(Block::Stone, |index| self.blocks[index])
    }

    fn connected(&self, point: [i32; 3]) -> bool {
        self.index(point).is_some_and(|index| self.connected[index])
    }

    fn run(&self, point: [i32; 3], axis: usize) -> usize {
        if self.block(point) != Block::Air {
            return 0;
        }
        1 + [-1, 1]
            .into_iter()
            .map(|direction| {
                (1..=RADIUS)
                    .take_while(|&distance| {
                        let mut next = point;
                        next[axis] += direction * distance;
                        self.block(next) == Block::Air
                    })
                    .count()
            })
            .sum::<usize>()
    }
}

#[derive(Debug, Default)]
struct Sections {
    heights: Vec<usize>,
    widths: Vec<usize>,
    aspects: Vec<f32>,
    split_columns: usize,
    narrow: usize,
    wide: usize,
    junctions: usize,
    split_rows: usize,
    corridor_loops: usize,
}

impl Sections {
    fn observe(cave: &LocalCave, feet: Vec3) -> Self {
        let mut result = Self::default();
        for z in (cave.min[2]..cave.min[2] + cave.size[2]).step_by(2) {
            for x in (cave.min[0]..cave.min[0] + cave.size[0]).step_by(2) {
                let mut intervals = 0;
                let mut y = MIN_WORLD_Y + 1;
                while y < 0 {
                    if cave.block([x, y, z]) != Block::Air {
                        y += 1;
                        continue;
                    }
                    let first = y;
                    let mut connected = false;
                    while y < 0 && cave.block([x, y, z]) == Block::Air {
                        connected |= cave.connected([x, y, z]);
                        y += 1;
                    }
                    if connected && y - first >= 2 {
                        intervals += 1;
                        let point = [x, (first + y - 1) / 2, z];
                        let height = (y - first) as usize;
                        let width = cave.run(point, 0).min(cave.run(point, 2));
                        result.heights.push(height);
                        result.widths.push(width);
                        result.aspects.push(height as f32 / width.max(1) as f32);
                    }
                }
                result.split_columns += usize::from(intervals > 1);
            }
        }
        let y = feet.y.floor() as i32 + 1;
        let passable = |[x, z]: [i32; 2]| cave.connected([x, y, z]);
        let mut narrow = HashSet::new();
        for z in cave.min[2]..cave.min[2] + cave.size[2] {
            let mut runs = 0;
            let mut run = 0;
            for x in cave.min[0]..cave.min[0] + cave.size[0] {
                if !passable([x, z]) {
                    runs += usize::from(run >= 3);
                    run = 0;
                    continue;
                }
                run += 1;
                let point = [x, y, z];
                let width = cave.run(point, 0).min(cave.run(point, 2));
                result.narrow += usize::from(width <= 10);
                result.wide += usize::from(width >= 17);
                if width <= 10 {
                    narrow.insert([x, z]);
                }
                let exits = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                    .into_iter()
                    .filter(|[dx, dz]| (1..=6).all(|step| passable([x + dx * step, z + dz * step])))
                    .count();
                result.junctions += usize::from(width < 17 && exits >= 3);
            }
            runs += usize::from(run >= 3);
            result.split_rows += usize::from(runs >= 2);
        }
        result.corridor_loops = enclosed_corridor_islands(cave, passable, &narrow);
        result.heights.sort_unstable();
        result.widths.sort_unstable();
        result.aspects.sort_by(f32::total_cmp);
        result
    }

    fn height_quantile(&self, percent: usize) -> usize {
        self.heights[self.heights.len().saturating_sub(1) * percent / 100]
    }

    fn width_quantile(&self, percent: usize) -> usize {
        self.widths[self.widths.len().saturating_sub(1) * percent / 100]
    }

    fn aspect_quantile(&self, percent: usize) -> f32 {
        self.aspects[self.aspects.len().saturating_sub(1) * percent / 100]
    }

    fn summary(&self) -> String {
        format!(
            "intervals={}, p10/p50/p90 height={:?} width={:?} aspect={:?}; split columns/rows={}/{}, narrow/wide={}/{}, junctions={}, narrow loops={}",
            self.heights.len(),
            [10, 50, 90].map(|percent| self.height_quantile(percent)),
            [10, 50, 90].map(|percent| self.width_quantile(percent)),
            [10, 50, 90].map(|percent| self.aspect_quantile(percent)),
            self.split_columns,
            self.split_rows,
            self.narrow,
            self.wide,
            self.junctions,
            self.corridor_loops,
        )
    }
}

fn enclosed_corridor_islands(
    cave: &LocalCave,
    air: impl Fn([i32; 2]) -> bool,
    narrow: &HashSet<[i32; 2]>,
) -> usize {
    let min = [cave.min[0], cave.min[2]];
    let max = [min[0] + cave.size[0] - 1, min[1] + cave.size[2] - 1];
    let mut seen = HashSet::new();
    let mut islands = 0;
    for z in min[1]..=max[1] {
        for x in min[0]..=max[0] {
            if air([x, z]) || !seen.insert([x, z]) {
                continue;
            }
            let mut queue = VecDeque::from([[x, z]]);
            let mut cells = 0;
            let mut boundary = false;
            let mut rim = HashSet::new();
            while let Some(point) = queue.pop_front() {
                cells += 1;
                boundary |= (0..2).any(|axis| point[axis] == min[axis] || point[axis] == max[axis]);
                for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    let next = [point[0] + dx, point[1] + dz];
                    if !(0..2).all(|axis| (min[axis]..=max[axis]).contains(&next[axis])) {
                        continue;
                    }
                    if air(next) {
                        rim.insert(next);
                    } else if seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            islands += usize::from(
                !boundary
                    && cells >= 9
                    && rim.len() >= 12
                    && rim.iter().filter(|point| narrow.contains(*point)).count() * 2 >= rim.len(),
            );
        }
    }
    islands
}

fn material_component_attached(
    world: &World,
    material: Block,
    root: [i32; 3],
    cache: &mut BTreeMap<[i32; 3], bool>,
) -> bool {
    if let Some(&attached) = cache.get(&root) {
        return attached;
    }
    let mut seen = HashSet::from([root]);
    let mut queue = VecDeque::from([root]);
    let mut attached = false;
    while let Some(point) = queue.pop_front() {
        for offset in NEIGHBORS {
            let next = std::array::from_fn(|axis| point[axis] + offset[axis]);
            let neighbor = world.block(next);
            if neighbor == material {
                if seen.len() < 8_192 && seen.insert(next) {
                    queue.push_back(next);
                }
            } else if neighbor.is_solid() {
                attached = true;
            }
        }
        if attached {
            break;
        }
    }
    for point in seen {
        cache.insert(point, attached);
    }
    attached
}

fn assert_landmark_clear(world: &World, landmark: CaveLandmark, kind: CaveLandform) {
    let feet = Vec3::from_array(landmark.position);
    let target = Vec3::from_array(landmark.target);
    let spawn = Vec3::from_array(world.terrain.spawn_position());
    assert_eq!(landmark.landform, kind);
    assert!(feet.is_finite() && target.is_finite());
    assert!((MIN_WORLD_Y as f32 + 1.0..0.0).contains(&feet.y));
    assert!((feet - spawn).with_y(0.0).length() <= 250.0);
    let point = feet.to_array().map(|coordinate| coordinate.floor() as i32);
    assert!(world.block([point[0], point[1] - 1, point[2]]).is_solid());
    let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = feet + Vec3::new(PLAYER_HALF_WIDTH, 3.4, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|coordinate| (f64::from(coordinate) + 0.000_01).floor() as i32);
    let last = max
        .to_array()
        .map(|coordinate| (f64::from(coordinate) - 0.000_01).floor() as i32);
    for z in first[2]..=last[2] {
        for y in first[1]..=last[1] {
            for x in first[0]..=last[0] {
                assert_eq!(world.block([x, y, z]), Block::Air, "{landmark:?}");
            }
        }
    }
    let eye = feet + Vec3::Y * EYE_HEIGHT;
    let delta = target - eye;
    assert!(delta.length() > 8.0, "short view: {landmark:?}");
    let target_block = target
        .to_array()
        .map(|coordinate| coordinate.floor() as i32);
    assert!(!world.block(target_block).is_liquid());
    let samples = (delta.length() * 8.0).ceil() as usize;
    for step in 0..=samples {
        let point = eye + delta * (step as f32 / samples as f32);
        let voxel = point.to_array().map(|coordinate| coordinate.floor() as i32);
        if voxel == target_block {
            break;
        }
        assert_eq!(world.block(voxel), Block::Air, "occluded {landmark:?}");
    }
}

#[path = "worldgen_variety/acceptance.rs"]
mod acceptance;
