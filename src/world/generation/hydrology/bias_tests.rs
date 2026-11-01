use super::*;
use crate::world::{CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y};

#[test]
fn nearby_water_bias_keeps_spawn_dry_and_adds_substantial_water_within_the_starting_area() {
    for seed in [Terrain::default().seed(), 42, 0, 1, u64::MAX] {
        let terrain = Terrain::new(seed);
        let [spawn_x, spawn_y, spawn_z] = terrain.spawn_position();
        let [x, z] = [spawn_x.floor() as i32, spawn_z.floor() as i32];
        assert!(terrain.water_column(x, z).is_none());
        assert!(terrain.block_at(x, spawn_y as i32 - 1, z).is_solid());
        assert!((0..=2).all(|dy| terrain.block_at(x, spawn_y as i32 + dy, z) == Block::Air));
        let mut wet = 0;
        let mut nearest = f32::INFINITY;
        for dz in (-128..=128).step_by(4) {
            for dx in (-128..=128).step_by(4) {
                let distance = (dx as f32).hypot(dz as f32);
                if distance <= 128.0 && terrain.water_column(x + dx, z + dz).is_some() {
                    wet += 1;
                    nearest = nearest.min(distance);
                }
            }
        }
        assert!(wet >= 96, "seed {seed}: only {wet} nearby wet samples");
        assert!(nearest <= 100.0, "seed {seed}: nearest water {nearest}");
    }
}

#[test]
fn nearby_lakes_have_actual_sealed_water_and_consistent_chunk_metadata() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        for lake in catchment.nearby_lakes {
            let [x, z] = catchment
                .world(lake.center)
                .map(|value| value.floor() as i32);
            let water = terrain.water_column(x, z).unwrap();
            assert_eq!(water.biome, WaterBiome::Lake);
            assert_eq!(water.flow, [0.0; 3]);
            assert_eq!(water.salinity, 0.0);
            assert!(water.depth() >= 4.0);
            let coord = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
            let chunk = Chunk::generate(&terrain, coord);
            for y in water.bottom - 3..=water.bottom {
                assert!(
                    terrain.block_at(x, y, z).is_solid(),
                    "unsealed lake at ({x}, {y}, {z})"
                );
            }
            for y in MIN_WORLD_Y..WORLD_HEIGHT {
                assert_eq!(
                    chunk.block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)),
                    terrain.block_at(x, y, z)
                );
                assert_eq!(
                    chunk.water(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)),
                    terrain.water_at(x, y, z)
                );
            }
        }
    }
}

#[test]
fn nearby_lake_shores_are_contained_and_blending_halos_stay_dry() {
    for seed in [Terrain::default().seed(), 42, 0, 1, u64::MAX] {
        let terrain = Terrain::new(seed);
        let catchment = Catchment::new(seed, 0, 0);
        for lake in catchment.nearby_lakes {
            for angle in 0..32 {
                let angle = f64::from(angle) * std::f64::consts::TAU / 32.0;
                for radius in [0.92, 0.98, 1.04, 1.15, 1.5] {
                    let point = [
                        lake.center[0] + angle.cos() * lake.radius[0] * radius,
                        lake.center[1] + angle.sin() * lake.radius[1] * radius,
                    ];
                    let [x, z] = catchment.world(point).map(|value| value.floor() as i32);
                    let water = terrain.water_column(x, z);
                    if radius >= 1.15 {
                        assert!(water.is_none(), "wet lake halo ({x}, {z})");
                    }
                    if let Some(water) = water {
                        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            if terrain.water_column(x + dx, z + dz).is_none() {
                                let bank = terrain.column(x + dx, z + dz);
                                assert!(
                                    water.surface - (bank.height + 1) as f32 <= 1.0,
                                    "hanging lake shore ({x}, {z})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn connected_rivers_and_ocean_coasts_are_available_within_256_blocks_of_spawn() {
    for seed in [Terrain::default().seed(), 42, 0, 1, u64::MAX] {
        let terrain = Terrain::new(seed);
        let [spawn_x, _, spawn_z] = terrain.spawn_position();
        let [x, z] = [spawn_x.floor() as i32, spawn_z.floor() as i32];
        let mut rivers = 0;
        let mut marine = 0;
        for dz in (-256..=256).step_by(4) {
            for dx in (-256..=256).step_by(4) {
                if (dx as f32).hypot(dz as f32) > 256.0 {
                    continue;
                }
                if let Some(water) = terrain.water_column(x + dx, z + dz) {
                    rivers += usize::from(water.biome == WaterBiome::River);
                    marine += usize::from(water.biome == WaterBiome::Ocean);
                }
            }
        }
        assert!(
            rivers >= 24,
            "seed {seed}: only {rivers} river samples within 256"
        );
        assert!(
            marine >= 24,
            "seed {seed}: only {marine} ocean samples within 256"
        );
    }
}

#[test]
fn nearby_lakes_and_catchments_repeat_without_vertical_gaps_across_negative_cells() {
    for seed in [Terrain::default().seed(), 42] {
        let terrain = Terrain::new(seed);
        let copy = Terrain::new(seed);
        for (cell_x, cell_z) in [(0, 0), (-1, 0), (0, -1), (-1, -1)] {
            let catchment = Catchment::new(seed, cell_x, cell_z);
            for lake in catchment.nearby_lakes {
                let [x, z] = catchment
                    .world(lake.center)
                    .map(|value| value.floor() as i32);
                let water = terrain.water_column(x, z).unwrap();
                assert_eq!(Some(water), copy.water_column(x, z));
                assert_eq!(water.biome, WaterBiome::Lake);
                for y in water.bottom + 1..water.surface.ceil() as i32 {
                    assert_eq!(terrain.block_at(x, y, z), Block::Water);
                }
                assert!(terrain.block_at(x, water.bottom, z).is_solid());
            }
        }
    }
}
