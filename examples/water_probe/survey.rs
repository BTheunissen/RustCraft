//! Bounded resident/region water observations.

use super::*;

pub(super) fn validate_landmark(
    terrain: &Terrain,
    spawn: Vec3,
    requested: WaterBiome,
    landmark: WaterLandmark,
) -> Result<WaterSample, String> {
    let feet = Vec3::from_array(landmark.position);
    let target = Vec3::from_array(landmark.target);
    if landmark.biome != requested
        || !feet.is_finite()
        || !target.is_finite()
        || !(MIN_WORLD_Y as f32 + 1.0..WORLD_HEIGHT as f32).contains(&feet.y)
        || (feet - spawn).with_y(0.0).length() > SEARCH_RADIUS as f32
    {
        return Err(format!(
            "invalid {} diagnostic landmark: {landmark:?}",
            requested.name()
        ));
    }
    let [x, y, z] = landmark.water;
    let sample = terrain.water_at(x, y, z).ok_or_else(|| {
        format!(
            "{} landmark names a dry voxel: {:?}",
            requested.name(),
            landmark.water
        )
    })?;
    validate_sample(sample)?;
    if terrain.block_at(x, y, z) != Block::Water || sample.biome != requested {
        return Err(format!(
            "{} landmark does not describe actual {} water: {sample:?}",
            requested.name(),
            requested.name()
        ));
    }
    Ok(sample)
}

pub(super) fn survey_coords(
    landmarks: &[WaterLandmark],
    cave_feet: Vec3,
    spawn: Vec3,
) -> BTreeSet<ChunkCoord> {
    let centers: Vec<_> = landmarks
        .iter()
        .map(|landmark| {
            ChunkCoord::new(
                landmark.water[0].div_euclid(CHUNK_SIZE),
                landmark.water[2].div_euclid(CHUNK_SIZE),
            )
        })
        .chain(std::iter::once(ChunkCoord::from_world(
            cave_feet.x,
            cave_feet.z,
        )))
        .collect();
    let mut coords = BTreeSet::new();
    for center in &centers {
        for dz in -1..=1 {
            for dx in -1..=1 {
                coords.insert(ChunkCoord::new(center.x + dx, center.z + dz));
            }
        }
    }
    coords.insert(ChunkCoord::from_world(spawn.x, spawn.z));
    coords.extend(
        landmarks
            .iter()
            .map(|landmark| ChunkCoord::from_world(landmark.position[0], landmark.position[2])),
    );
    if landmarks.len() == SURFACE_BIOMES.len() && coords.len() < 45 {
        let mut candidates = Vec::new();
        for center in &centers {
            for dz in -4_i32..=4 {
                for dx in -4_i32..=4 {
                    candidates.push((
                        dx * dx + dz * dz,
                        ChunkCoord::new(center.x + dx, center.z + dz),
                    ));
                }
            }
        }
        candidates.sort_unstable();
        for (_, coord) in candidates {
            coords.insert(coord);
            if coords.len() >= 45 {
                break;
            }
        }
    }
    coords
}

pub(super) struct Survey {
    pub(super) chunks: BTreeMap<ChunkCoord, Chunk>,
}

impl Survey {
    pub(super) fn generate(terrain: &Terrain, coords: BTreeSet<ChunkCoord>) -> (Self, Duration) {
        let mut elapsed = Duration::ZERO;
        let chunks = coords
            .into_iter()
            .map(|coord| {
                let started = Instant::now();
                let chunk = Chunk::generate(terrain, coord);
                elapsed += started.elapsed();
                (coord, chunk)
            })
            .collect();
        (Self { chunks }, elapsed)
    }

    pub(super) fn water(&self, [x, y, z]: [i32; 3]) -> Option<WaterSample> {
        self.chunks
            .get(&ChunkCoord::new(
                x.div_euclid(CHUNK_SIZE),
                z.div_euclid(CHUNK_SIZE),
            ))?
            .water(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    }

    pub(super) fn block(&self, [x, y, z]: [i32; 3]) -> Option<Block> {
        self.chunks
            .get(&ChunkCoord::new(
                x.div_euclid(CHUNK_SIZE),
                z.div_euclid(CHUNK_SIZE),
            ))
            .map(|chunk| chunk.block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    pub(super) fn inspect(&self) -> Result<[Profiles; 6], String> {
        let mut profiles: [Profiles; 6] = std::array::from_fn(|_| Profiles::default());
        for (&coord, chunk) in &self.chunks {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let mut recorded = [false; 6];
                    for y in MIN_WORLD_Y..WORLD_HEIGHT {
                        let material = chunk.block(x, y, z);
                        let water = chunk.water(x, y, z);
                        if water.is_some() != (material == Block::Water) {
                            return Err(format!(
                                "resident wet metadata disagrees at chunk {coord:?}, ({x}, {y}, {z})"
                            ));
                        }
                        let Some(water) = water else { continue };
                        let index = water.biome as usize;
                        profiles[index].voxels += 1;
                        if recorded[index] {
                            continue;
                        }
                        recorded[index] = true;
                        profiles[index].record(water, Some(chunk.block(x, water.bottom, z)))?;
                        if water.biome != WaterBiome::CavePool {
                            for bed_y in water.bottom - 3..=water.bottom {
                                if !chunk.block(x, bed_y, z).is_solid() {
                                    return Err(format!(
                                        "surface water bed cap is carved at chunk {coord:?}, ({x}, {bed_y}, {z})"
                                    ));
                                }
                            }
                        } else if water.flow != [0.0; 3] || water.salinity != 0.0 {
                            return Err(format!("cave pool is not calm freshwater: {water:?}"));
                        }
                    }
                }
            }
        }
        Ok(profiles)
    }

    pub(super) fn local_profile(
        &self,
        terrain: &Terrain,
        landmark: WaterLandmark,
    ) -> Result<Profiles, String> {
        let mut result = Profiles::default();
        for dz in -LOCAL_RADIUS..=LOCAL_RADIUS {
            for dx in -LOCAL_RADIUS..=LOCAL_RADIUS {
                let x = landmark.water[0] + dx;
                let z = landmark.water[2] + dz;
                let Some(profile) = terrain.water_column(x, z) else {
                    continue;
                };
                if profile.biome != landmark.biome {
                    continue;
                }
                let y = profile.surface.ceil() as i32 - 1;
                let sample = self.water([x, y, z]).ok_or_else(|| {
                    format!("{} local footprint is dry or outside the bounded survey at ({x}, {y}, {z})", landmark.biome.name())
                })?;
                if sample.biome != profile.biome {
                    return Err(format!(
                        "local footprint and resident water differ at ({x}, {y}, {z})"
                    ));
                }
                result.record(sample, self.block([x, sample.bottom, z]))?;
            }
        }
        if result.columns == 0 {
            return Err(format!(
                "{} landmark lacks a dense local wet footprint",
                landmark.biome.name()
            ));
        }
        Ok(result)
    }
}

pub(super) struct RegionalCoverage {
    pub(super) samples: usize,
    pub(super) wet: BTreeMap<[i32; 2], WaterSample>,
    pub(super) profiles: [Profiles; 6],
    pub(super) marine_bounds: Option<[[i32; 2]; 2]>,
    pub(super) shore_gradients: Vec<f64>,
}

impl RegionalCoverage {
    pub(super) fn inspect(terrain: &Terrain, origin: [i32; 2]) -> Result<Self, String> {
        let mut result = Self {
            samples: 0,
            wet: BTreeMap::new(),
            profiles: std::array::from_fn(|_| Profiles::default()),
            marine_bounds: None,
            shore_gradients: Vec::new(),
        };
        for dz in (-SEARCH_RADIUS..=SEARCH_RADIUS).step_by(COARSE_STEP as usize) {
            for dx in (-SEARCH_RADIUS..=SEARCH_RADIUS).step_by(COARSE_STEP as usize) {
                if i64::from(dx).pow(2) + i64::from(dz).pow(2) > i64::from(SEARCH_RADIUS).pow(2) {
                    continue;
                }
                result.samples += 1;
                let [x, z] = [origin[0] + dx, origin[1] + dz];
                let Some(profile) = terrain.water_column(x, z) else {
                    continue;
                };
                let y = profile.surface.ceil() as i32 - 1;
                let water = terrain.water_at(x, y, z).ok_or_else(|| {
                    format!("coarse water footprint does not contain water at ({x}, {y}, {z})")
                })?;
                if terrain.block_at(x, y, z) != Block::Water || water.biome != profile.biome {
                    return Err(format!(
                        "coarse footprint names a dry or different water body at ({x}, {y}, {z})"
                    ));
                }
                result.profiles[water.biome as usize].record(water, None)?;
                result.wet.insert([x, z], water);
                if water.biome == WaterBiome::Ocean {
                    let bounds = result.marine_bounds.get_or_insert([[x, z]; 2]);
                    for (axis, value) in [x, z].into_iter().enumerate() {
                        bounds[0][axis] = bounds[0][axis].min(value);
                        bounds[1][axis] = bounds[1][axis].max(value);
                    }
                }
            }
        }
        for (&[x, z], water) in &result.wet {
            if !matches!(water.biome, WaterBiome::Ocean | WaterBiome::Estuary) {
                continue;
            }
            for [dx, dz] in [
                [COARSE_STEP, 0],
                [-COARSE_STEP, 0],
                [0, COARSE_STEP],
                [0, -COARSE_STEP],
            ] {
                let [next_x, next_z] = [x + dx, z + dz];
                let distance =
                    i64::from(next_x - origin[0]).pow(2) + i64::from(next_z - origin[1]).pow(2);
                if distance <= i64::from(SEARCH_RADIUS).pow(2)
                    && !result.wet.contains_key(&[next_x, next_z])
                {
                    result.shore_gradients.push(
                        f64::from((terrain.column(next_x, next_z).height - water.bottom).abs())
                            / f64::from(COARSE_STEP),
                    );
                }
            }
        }
        Ok(result)
    }

    pub(super) fn report(&self) {
        println!(
            "coarse actual-water survey: radius={SEARCH_RADIUS}, step={COARSE_STEP}, {} wet/{} horizontal samples ({:.2}%); grid-area estimate={} blocks², not an exact shoreline area",
            self.wet.len(),
            self.samples,
            self.wet.len() as f64 / self.samples.max(1) as f64 * 100.0,
            self.wet.len() * (COARSE_STEP * COARSE_STEP) as usize,
        );
        for (biome, profile) in WATER_BIOMES.into_iter().zip(&self.profiles) {
            profile.report(biome.name());
        }
        println!(
            "  observed ocean footprint bounds={:?}; marine/dry shore edges={}, bed gradient p10/p50/p90={:?} blocks/block",
            self.marine_bounds,
            self.shore_gradients.len(),
            quantiles(&self.shore_gradients),
        );
    }
}
