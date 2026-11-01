//! Chunk sampling, traversability, and camera visibility.

use super::*;

pub(super) struct Survey {
    pub(super) terrain: Terrain,
    pub(super) chunks: BTreeMap<ChunkCoord, Chunk>,
    pub(super) boundary_columns: RefCell<BTreeMap<[i32; 2], [Block; WORLD_DEPTH as usize]>>,
}

pub(super) fn survey_coords(sites: &[Vec3], support_sites: &[Vec3]) -> BTreeSet<ChunkCoord> {
    let mut coords = BTreeSet::new();
    for site in sites {
        let center = ChunkCoord::from_world(site.x, site.z);
        for z in center.z - 1..=center.z + 1 {
            for x in center.x - 1..=center.x + 1 {
                coords.insert(ChunkCoord::new(x, z));
            }
        }
    }
    coords.extend(
        support_sites
            .iter()
            .map(|site| ChunkCoord::from_world(site.x, site.z)),
    );
    if sites.len() == BIOMES.len() + LANDFORMS.len() && coords.len() < 45 {
        let mut extra = BTreeSet::new();
        for site in sites {
            let center = ChunkCoord::from_world(site.x, site.z);
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
    coords
}

impl Survey {
    pub(super) fn generate(
        terrain: Terrain,
        sites: &[Vec3],
        support_sites: &[Vec3],
    ) -> (Self, Duration) {
        // Complete scenic surveys cover 45..=65 chunks even when landmarks overlap.
        let mut generation = Duration::ZERO;
        let chunks = survey_coords(sites, support_sites)
            .into_iter()
            .map(|coord| {
                let started = Instant::now();
                let chunk = Chunk::generate(&terrain, coord);
                generation += started.elapsed();
                (coord, chunk)
            })
            .collect();
        (
            Self {
                terrain,
                chunks,
                boundary_columns: RefCell::new(BTreeMap::new()),
            },
            generation,
        )
    }

    pub(super) fn block(&self, [x, y, z]: [i32; 3]) -> Block {
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&y) {
            return Block::Air;
        }
        let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        self.chunks.get(&coord).map_or_else(
            || {
                self.boundary_columns
                    .borrow_mut()
                    .entry([x, z])
                    .or_insert_with(|| self.terrain.column_blocks(x, z))
                    [(y - MIN_WORLD_Y) as usize]
            },
            |chunk| chunk.block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)),
        )
    }

    pub(super) fn observed_component(&self, feet: Vec3) -> Result<Connectivity, String> {
        let root = feet.to_array().map(|coordinate| coordinate.floor() as i32);
        self.component(
            feet,
            [
                root[0] - GEOMETRY_RADIUS,
                MIN_WORLD_Y + 1,
                root[2] - GEOMETRY_RADIUS,
            ],
            [
                root[0] + GEOMETRY_RADIUS,
                WORLD_HEIGHT - 2,
                root[2] + GEOMETRY_RADIUS,
            ],
        )
    }

    pub(super) fn underground_blocks(&self) -> [usize; Block::COUNT] {
        let mut counts = [0; Block::COUNT];
        for chunk in self.chunks.values() {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y..0 {
                        counts[chunk.block(x, y, z) as usize] += 1;
                    }
                }
            }
        }
        counts
    }

    pub(super) fn validate_resting(&self, feet: Vec3) -> Result<Block, String> {
        if !feet.is_finite() {
            return Err(format!("non-finite diagnostic feet: {feet:?}"));
        }
        let floor = self.block([
            feet.x.floor() as i32,
            (feet.y - 0.001).floor() as i32,
            feet.z.floor() as i32,
        ]);
        if !floor.is_solid() {
            return Err(format!("unsupported diagnostic feet {feet:?}: {floor:?}"));
        }
        let solid = |x, y, z| self.block([x, y, z]).is_solid();
        let mut player = Player::new(feet);
        player.toggle_flight(solid);
        player.toggle_flight(solid);
        for _ in 0..120 {
            player.update(MovementInput::default(), 1.0 / 60.0, solid);
        }
        if player.flying || !player.is_grounded() || player.position.distance(feet) > 0.002 {
            return Err(format!(
                "unsafe resting/flight-exit pose {feet:?}: {:?}",
                player.position
            ));
        }
        Ok(floor)
    }

    pub(super) fn validate_diagnostic(&self, feet: Vec3) -> Result<Block, String> {
        if !dry_headroom(feet, DIAGNOSTIC_HEADROOM, |point| self.block(point)) {
            return Err(format!(
                "diagnostic pose lacks {DIAGNOSTIC_HEADROOM}-block dry headroom: {feet:?}"
            ));
        }
        self.validate_resting(feet)
    }

    pub(super) fn validate_landmark(
        &self,
        landmark: CaveLandmark,
        requested: CaveLandform,
    ) -> Result<(), String> {
        let feet = Vec3::from_array(landmark.position);
        let target = Vec3::from_array(landmark.target);
        let spawn = Vec3::from_array(self.terrain.spawn_position());
        let eye = feet + Vec3::Y * EYE_HEIGHT;
        if landmark.landform != requested
            || !feet.is_finite()
            || !target.is_finite()
            || !(MIN_WORLD_Y as f32 + 1.0..0.0).contains(&feet.y)
            || (feet - spawn).with_y(0.0).length() > 250.0
            || eye.distance(target) <= 8.0
        {
            return Err(format!(
                "invalid {} landmark: {landmark:?}",
                requested.name()
            ));
        }
        self.validate_diagnostic(feet)?;
        if !clear_landmark_ray(eye, target, |point| self.block(point)) {
            return Err(format!(
                "occluded or liquid {} landmark target: {:?}",
                requested.name(),
                landmark.target
            ));
        }
        Ok(())
    }

    pub(super) fn connectivity(&self, feet: Vec3) -> Result<Connectivity, String> {
        let center = ChunkCoord::from_world(feet.x, feet.z);
        let min = [
            (center.x - 1) * CHUNK_SIZE,
            MIN_WORLD_Y + 1,
            (center.z - 1) * CHUNK_SIZE,
        ];
        let max = [
            (center.x + 2) * CHUNK_SIZE - 1,
            WORLD_HEIGHT - 2,
            (center.z + 2) * CHUNK_SIZE - 1,
        ];
        self.component(feet, min, max)
    }

    pub(super) fn component(
        &self,
        feet: Vec3,
        min: [i32; 3],
        max: [i32; 3],
    ) -> Result<Connectivity, String> {
        let root = feet.to_array().map(|coordinate| coordinate.floor() as i32);
        let clear = |[x, y, z]: [i32; 3]| {
            self.block([x, y, z]) == Block::Air && self.block([x, y + 1, z]) == Block::Air
        };
        if !clear(root) || !(min[1]..=max[1]).contains(&root[1]) {
            return Err(format!("no two-block-air clearance at {feet:?}"));
        }
        let mut queue = VecDeque::from([root]);
        let mut seen = HashSet::from([root]);
        let mut result = Connectivity {
            min: root,
            max: root,
            ..Connectivity::default()
        };
        while let Some(point) = queue.pop_front() {
            result.cells += 1;
            result.negative_cells += usize::from(point[1] < 0);
            result.sky_connected |= point[1] == max[1];
            result.clipped |= [0, 2]
                .into_iter()
                .any(|axis| point[axis] == min[axis] || point[axis] == max[axis]);
            for (axis, coordinate) in point.into_iter().enumerate() {
                result.min[axis] = result.min[axis].min(coordinate);
                result.max[axis] = result.max[axis].max(coordinate);
            }
            for headroom in 0..2 {
                for offset in NEIGHBORS {
                    let adjacent = [
                        point[0] + offset[0],
                        point[1] + headroom + offset[1],
                        point[2] + offset[2],
                    ];
                    if adjacent[1] < 0 && self.block(adjacent) != Block::Air {
                        result.exposed.insert(adjacent);
                    }
                }
            }
            for offset in NEIGHBORS {
                let adjacent = std::array::from_fn(|axis| point[axis] + offset[axis]);
                if (0..3).all(|axis| (min[axis]..=max[axis]).contains(&adjacent[axis]))
                    && !seen.contains(&adjacent)
                    && clear(adjacent)
                {
                    seen.insert(adjacent);
                    queue.push_back(adjacent);
                }
            }
        }
        result.clear = seen;
        Ok(result)
    }

    pub(super) fn visible_feature(
        &self,
        feet: Vec3,
        biome: CaveBiome,
        connectivity: &Connectivity,
    ) -> Option<(Block, [i32; 3])> {
        let preferred = match biome {
            CaveBiome::Rocky => [Block::IronOre, Block::CoalOre, Block::DeepStone],
            CaveBiome::Lush => [Block::Moss, Block::Water, Block::DeepStone],
            CaveBiome::Crystal => [Block::Crystal, Block::GoldOre, Block::DeepStone],
        };
        let eye = feet + Vec3::Y * EYE_HEIGHT;
        for material in preferred {
            let mut candidates: Vec<_> = connectivity
                .exposed
                .iter()
                .copied()
                .filter(|&point| {
                    self.block(point) == material
                        && block_center(point).y >= feet.y + 0.5
                        && eye.distance_squared(block_center(point)) <= 24.0_f32.powi(2)
                })
                .collect();
            candidates.sort_unstable_by(|&a, &b| {
                eye.distance_squared(block_center(a))
                    .total_cmp(&eye.distance_squared(block_center(b)))
                    .then_with(|| a.cmp(&b))
            });
            if let Some(point) = candidates
                .into_iter()
                .find(|&point| clear_ray_to(eye, point, |sample| self.block(sample)))
            {
                return Some((material, point));
            }
        }
        None
    }

    pub(super) fn exposed_material_features(&self) -> Features {
        let mut features = Features::default();
        for (&coord, chunk) in &self.chunks {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y + 1..0 {
                        let material = chunk.block(x, y, z);
                        if material == Block::Air {
                            continue;
                        }
                        let point = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                        if NEIGHBORS.into_iter().any(|offset| {
                            self.block(std::array::from_fn(|axis| point[axis] + offset[axis]))
                                == Block::Air
                        }) {
                            features.record(point, material, |point| self.block(point));
                        }
                    }
                }
            }
        }
        features
    }
}

pub(super) fn dry_headroom(feet: Vec3, height: f32, block: impl Fn([i32; 3]) -> Block) -> bool {
    if !feet.is_finite() || !height.is_finite() || height <= 0.0 {
        return false;
    }
    let min = feet - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
    let max = feet + Vec3::new(PLAYER_HALF_WIDTH, height, PLAYER_HALF_WIDTH);
    let first = min
        .to_array()
        .map(|coordinate| (f64::from(coordinate) + 0.000_01).floor() as i32);
    let last = max
        .to_array()
        .map(|coordinate| (f64::from(coordinate) - 0.000_01).floor() as i32);
    (first[0]..=last[0]).all(|x| {
        (first[1]..=last[1]).all(|y| (first[2]..=last[2]).all(|z| block([x, y, z]) == Block::Air))
    })
}

pub(super) fn block_center(point: [i32; 3]) -> Vec3 {
    Vec3::from_array(point.map(|coordinate| coordinate as f32 + 0.5))
}

pub(super) fn clear_ray_to(eye: Vec3, target: [i32; 3], block: impl Fn([i32; 3]) -> Block) -> bool {
    let delta = block_center(target) - eye;
    let samples = (delta.length() * 8.0).ceil().max(1.0) as usize;
    for sample in 0..=samples {
        let point = eye + delta * (sample as f32 / samples as f32);
        let voxel = point.to_array().map(|coordinate| coordinate.floor() as i32);
        if voxel == target {
            return true;
        }
        if block(voxel).is_opaque() {
            return false;
        }
    }
    false
}

pub(super) fn clear_ray_between(
    eye: Vec3,
    target: Vec3,
    block: impl Fn([i32; 3]) -> Block,
) -> bool {
    let delta = target - eye;
    let samples = (delta.length() * 8.0).ceil().max(1.0) as usize;
    (0..=samples).all(|sample| {
        let point = eye + delta * (sample as f32 / samples as f32);
        !block(point.to_array().map(|coordinate| coordinate.floor() as i32)).is_opaque()
    })
}

pub(super) fn clear_landmark_ray(
    eye: Vec3,
    target: Vec3,
    block: impl Fn([i32; 3]) -> Block,
) -> bool {
    if !eye.is_finite() || !target.is_finite() {
        return false;
    }
    let target_block = target
        .to_array()
        .map(|coordinate| coordinate.floor() as i32);
    if block(target_block).is_liquid() {
        return false;
    }
    let delta = target - eye;
    let samples = (delta.length() * 8.0).ceil().max(1.0) as usize;
    for sample in 0..=samples {
        let point = eye + delta * (sample as f32 / samples as f32);
        let voxel = point.to_array().map(|coordinate| coordinate.floor() as i32);
        if voxel == target_block {
            return true;
        }
        if block(voxel) != Block::Air {
            return false;
        }
    }
    false
}
