//! Lazy bounded resident voxel survey with standing and swept-body checks.

use std::cell::RefCell;
use std::collections::BTreeMap;

use glam::Vec3;

use crate::player::{PLAYER_HALF_WIDTH, PLAYER_HEIGHT};
use crate::world::{
    Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, Terrain, WORLD_HEIGHT, WaterSample,
};

const MAX_CHUNKS: usize = 1_681;

pub(super) struct Survey {
    terrain: Terrain,
    min: [i32; 2],
    max: [i32; 2],
    chunks: RefCell<BTreeMap<ChunkCoord, Chunk>>,
    ground: RefCell<BTreeMap<[i32; 2], Option<i32>>>,
}

impl Survey {
    pub(super) fn new(terrain: Terrain, points: &[[f32; 3]]) -> Result<Self, String> {
        if points.is_empty() || points.iter().flatten().any(|value| !value.is_finite()) {
            return Err("coastal route requires finite endpoints".into());
        }
        let mut min = [i32::MAX; 2];
        let mut max = [i32::MIN; 2];
        for point in points {
            for (axis, value) in [point[0], point[2]].into_iter().enumerate() {
                let value = value.floor() as i32;
                min[axis] = min[axis].min(value.saturating_sub(48));
                max[axis] = max[axis].max(value.saturating_add(48));
            }
        }
        if (0..2).any(|axis| i64::from(max[axis]) - i64::from(min[axis]) >= 640) {
            return Err("coastal route exceeds its 640-block survey bounds".into());
        }
        Ok(Self {
            terrain,
            min,
            max,
            chunks: RefCell::default(),
            ground: RefCell::default(),
        })
    }

    pub(super) fn contains(&self, [x, z]: [i32; 2]) -> bool {
        (self.min[0]..=self.max[0]).contains(&x) && (self.min[1]..=self.max[1]).contains(&z)
    }

    pub(super) fn block(&self, [x, y, z]: [i32; 3]) -> Block {
        if !self.contains([x, z]) {
            return Block::Stone;
        }
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return Block::Air;
        }
        let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        let mut chunks = self.chunks.borrow_mut();
        if chunks.len() >= MAX_CHUNKS && !chunks.contains_key(&coord) {
            return Block::Stone;
        }
        chunks
            .entry(coord)
            .or_insert_with(|| Chunk::generate(&self.terrain, coord))
            .block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    }

    pub(super) fn water(&self, [x, y, z]: [i32; 3]) -> Option<WaterSample> {
        if self.block([x, y, z]) != Block::Water {
            return None;
        }
        let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        self.chunks.borrow().get(&coord)?.water(
            x.rem_euclid(CHUNK_SIZE),
            y,
            z.rem_euclid(CHUNK_SIZE),
        )
    }

    pub(super) fn feet(&self, [x, z]: [i32; 2]) -> Option<Vec3> {
        if !self.contains([x, z]) {
            return None;
        }
        let cached = self.ground.borrow().get(&[x, z]).copied();
        let y = if let Some(cached) = cached {
            cached
        } else {
            let mut y = self.terrain.column(x, z).height + 1;
            // The cached dense column is the collision authority, including
            // actual cave mouths and decorations above the cap.
            while y < WORLD_HEIGHT && self.block([x, y, z]).is_solid() {
                y += 1;
            }
            let feet = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
            let y = (self.block([x, y - 1, z]).is_solid()
                && self.clear(feet, PLAYER_HEIGHT)
                && self.block([x, y, z]) == Block::Air
                && self.block([x, y + 1, z]) == Block::Air)
                .then_some(y);
            self.ground.borrow_mut().insert([x, z], y);
            y
        }?;
        Some(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5))
    }

    pub(super) fn clear(&self, feet: Vec3, height: f32) -> bool {
        if !feet.is_finite() || !height.is_finite() || height <= 0.0 {
            return false;
        }
        let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
        let max = feet + Vec3::new(PLAYER_HALF_WIDTH, height, PLAYER_HALF_WIDTH);
        let first = min
            .to_array()
            .map(|value| (f64::from(value) + 0.00001).floor() as i32);
        let last = max
            .to_array()
            .map(|value| (f64::from(value) - 0.00001).floor() as i32);
        (first[0]..=last[0]).all(|x| {
            (first[1]..=last[1])
                .all(|y| (first[2]..=last[2]).all(|z| !self.block([x, y, z]).is_solid()))
        })
    }

    pub(super) fn edge(&self, first: Vec3, second: Vec3) -> bool {
        if (first.y - second.y).abs() > 1.0 {
            return false;
        }
        let jump = first.y != second.y;
        if jump && (!self.clear(first, 3.05) || !self.clear(second, 3.05)) {
            return false;
        }
        let y = first.y.max(second.y);
        (0..=8).all(|step| {
            let point = first.lerp(second, step as f32 / 8.0).with_y(y);
            self.clear(point, PLAYER_HEIGHT + if jump { 0.22 } else { 0.0 })
                && self.water(point.floor().as_ivec3().to_array()).is_none()
        })
    }

    pub(super) fn chunk_count(&self) -> usize {
        self.chunks.borrow().len()
    }
}
