//! Supported formations, material clusters, and view candidates.

use super::*;

impl Survey {
    pub(super) fn pool_viewpoint(&self, material: Block) -> Option<FeatureView> {
        let mut pools = Vec::new();
        for (&coord, chunk) in &self.chunks {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y + 1..-2 {
                        if chunk.block(x, y, z) != material {
                            continue;
                        }
                        let center = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                        if broad_pool_surface(center, material, |point| self.block(point)) {
                            pools.push(center);
                        }
                    }
                }
            }
        }
        let spawn = Vec3::from_array(self.terrain.spawn_position());
        pools.sort_unstable_by(|&a, &b| {
            let distance =
                |[x, _, z]: [i32; 3]| (x as f32 - spawn.x).powi(2) + (z as f32 - spawn.z).powi(2);
            distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
        });
        for center in pools {
            let target = Vec3::new(
                center[0] as f32 + 0.5,
                center[1] as f32 + 1.02,
                center[2] as f32 + 0.5,
            );
            let mut candidates = Vec::new();
            for dz in -7_i32..=7 {
                for dx in -7_i32..=7 {
                    if !(3..=7).contains(&dx.abs().max(dz.abs())) {
                        continue;
                    }
                    for y in center[1] + 1..=center[1] + 4 {
                        let feet = Vec3::new(
                            (center[0] + dx) as f32 + 0.5,
                            y as f32,
                            (center[2] + dz) as f32 + 0.5,
                        );
                        let point = feet.to_array().map(|coordinate| coordinate.floor() as i32);
                        if !self.block([point[0], point[1] - 1, point[2]]).is_solid()
                            || self.block(point) != Block::Air
                            || self.block([point[0], point[1] + 1, point[2]]) != Block::Air
                        {
                            continue;
                        }
                        let eye = feet + Vec3::Y * EYE_HEIGHT;
                        if clear_ray_between(eye, target, |sample| self.block(sample)) {
                            candidates.push(feet);
                        }
                    }
                }
            }
            candidates.sort_unstable_by(|a, b| {
                a.distance_squared(target)
                    .total_cmp(&b.distance_squared(target))
                    .then_with(|| a.x.total_cmp(&b.x))
                    .then_with(|| a.z.total_cmp(&b.z))
            });
            if let Some(feet) = candidates
                .into_iter()
                .find(|&feet| self.validate_resting(feet).is_ok())
            {
                return Some(FeatureView {
                    block: center,
                    feet,
                    target,
                });
            }
        }
        None
    }

    pub(super) fn ore_viewpoint(&self, material: Block) -> Option<FeatureView> {
        let mut exposed = Vec::new();
        for (&coord, chunk) in &self.chunks {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in MIN_WORLD_Y + 1..-2 {
                        if chunk.block(x, y, z) != material {
                            continue;
                        }
                        let point = [coord.x * CHUNK_SIZE + x, y, coord.z * CHUNK_SIZE + z];
                        if NEIGHBORS.into_iter().any(|offset| {
                            self.block(std::array::from_fn(|axis| point[axis] + offset[axis]))
                                == Block::Air
                        }) {
                            exposed.push(point);
                        }
                    }
                }
            }
        }
        let spawn = Vec3::from_array(self.terrain.spawn_position());
        exposed.sort_unstable_by(|&a, &b| {
            let distance =
                |[x, _, z]: [i32; 3]| (x as f32 - spawn.x).powi(2) + (z as f32 - spawn.z).powi(2);
            distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
        });
        for point in exposed {
            let target = block_center(point);
            for distance in 2..=6 {
                for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    for y in point[1] - 3..=point[1] {
                        let feet = Vec3::new(
                            (point[0] + dx * distance) as f32 + 0.5,
                            y as f32,
                            (point[2] + dz * distance) as f32 + 0.5,
                        );
                        let candidate = feet.to_array().map(|coordinate| coordinate.floor() as i32);
                        if !self.block([candidate[0], y - 1, candidate[2]]).is_solid()
                            || self.block(candidate) != Block::Air
                            || self.block([candidate[0], y + 1, candidate[2]]) != Block::Air
                            || !clear_ray_to(feet + Vec3::Y * EYE_HEIGHT, point, |sample| {
                                self.block(sample)
                            })
                        {
                            continue;
                        }
                        if self.validate_resting(feet).is_ok() {
                            return Some(FeatureView {
                                block: point,
                                feet,
                                target,
                            });
                        }
                    }
                }
            }
        }
        None
    }
}

#[derive(Default)]
pub(super) struct Connectivity {
    pub(super) cells: usize,
    pub(super) negative_cells: usize,
    pub(super) min: [i32; 3],
    pub(super) max: [i32; 3],
    pub(super) sky_connected: bool,
    pub(super) clipped: bool,
    pub(super) exposed: HashSet<[i32; 3]>,
    pub(super) clear: HashSet<[i32; 3]>,
}

pub(super) struct FeatureView {
    pub(super) block: [i32; 3],
    pub(super) feet: Vec3,
    pub(super) target: Vec3,
}

#[derive(Debug, Default)]
pub(super) struct Features {
    pub(super) exposed: [usize; Block::COUNT],
    pub(super) upper_calcite: usize,
    pub(super) calcite_air_gaps: usize,
    pub(super) supported_calcite_tips: usize,
    pub(super) unsupported_calcite_tips: usize,
    pub(super) clustered_moss: usize,
    pub(super) clustered_crystal: usize,
    pub(super) attached_crystal: usize,
    pub(super) unsupported_crystal: usize,
    pub(super) broad_liquid_surfaces: [usize; 2],
    pub(super) calcite_attachments: BTreeMap<[i32; 3], bool>,
    pub(super) crystal_attachments: BTreeMap<[i32; 3], bool>,
}

impl Features {
    pub(super) fn record(
        &mut self,
        point: [i32; 3],
        material: Block,
        block: impl Fn([i32; 3]) -> Block,
    ) {
        self.exposed[material as usize] += 1;
        if material.is_liquid() {
            self.broad_liquid_surfaces[usize::from(material == Block::Lava)] +=
                usize::from(broad_pool_surface(point, material, &block));
        }
        let clustered = NEIGHBORS.into_iter().any(|offset| {
            block(std::array::from_fn(|axis| point[axis] + offset[axis])) == material
        });
        self.clustered_moss += usize::from(material == Block::Moss && clustered);
        self.clustered_crystal += usize::from(material == Block::Crystal && clustered);
        if material == Block::Crystal {
            let attached =
                material_is_attached(point, material, &block, &mut self.crystal_attachments);
            self.attached_crystal += usize::from(attached);
            self.unsupported_crystal += usize::from(!attached);
        }
        if material != Block::Calcite {
            return;
        }
        self.upper_calcite += usize::from(point[1] >= -20);
        self.calcite_air_gaps += usize::from(NEIGHBORS.into_iter().any(|offset| {
            let next = |distance| std::array::from_fn(|axis| point[axis] + offset[axis] * distance);
            let mut gap = 1;
            while gap <= 20 && block(next(gap)) == Block::Air {
                gap += 1;
            }
            (3..=20).contains(&gap) && block(next(gap)) == Block::Calcite
        }));
        let downward_tip = block([point[0], point[1] - 1, point[2]]) == Block::Air;
        let upward_tip = block([point[0], point[1] + 1, point[2]]) == Block::Air;
        if downward_tip || upward_tip {
            let supported =
                [(-1, upward_tip), (1, downward_tip)]
                    .into_iter()
                    .any(|(direction, enabled)| {
                        enabled && formation_is_supported(point, direction, &block)
                    })
                    || material_is_attached(point, material, &block, &mut self.calcite_attachments);
            self.supported_calcite_tips += usize::from(supported);
            self.unsupported_calcite_tips += usize::from(!supported);
        }
    }

    pub(super) fn report(&self, label: &str) {
        print!("  {label} exposed geology:");
        for material in GEOLOGY {
            print!(" {material:?}={}", self.exposed[material as usize]);
        }
        println!(
            "; supported/unsupported calcite tips={}/{}, upper calcite={}, calcite air-gap samples={}, adjacent moss/crystal cluster voxels={}/{}, attached/floating crystal voxels={}/{}, clear 3x3 water/lava surface centers={}/{}",
            self.supported_calcite_tips,
            self.unsupported_calcite_tips,
            self.upper_calcite,
            self.calcite_air_gaps,
            self.clustered_moss,
            self.clustered_crystal,
            self.attached_crystal,
            self.unsupported_crystal,
            self.broad_liquid_surfaces[0],
            self.broad_liquid_surfaces[1],
        );
    }
}

pub(super) fn formation_is_supported(
    point: [i32; 3],
    direction: i32,
    block: impl Fn([i32; 3]) -> Block,
) -> bool {
    for distance in 1..=WORLD_DEPTH {
        let next = [point[0], point[1] + direction * distance, point[2]];
        if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&next[1]) {
            return false;
        }
        match block(next) {
            Block::Calcite => {}
            material => return material.is_solid(),
        }
    }
    false
}

pub(super) fn material_is_attached(
    root: [i32; 3],
    material: Block,
    block: impl Fn([i32; 3]) -> Block,
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
            if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&next[1]) {
                continue;
            }
            let neighbor = block(next);
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

pub(super) fn broad_pool_surface(
    [x, y, z]: [i32; 3],
    material: Block,
    block: impl Fn([i32; 3]) -> Block,
) -> bool {
    (-1..=1).all(|dz| {
        (-1..=1).all(|dx| {
            block([x + dx, y, z + dz]) == material
                && block([x + dx, y + 1, z + dz]) == Block::Air
                && block([x + dx, y + 2, z + dz]) == Block::Air
        })
    })
}
