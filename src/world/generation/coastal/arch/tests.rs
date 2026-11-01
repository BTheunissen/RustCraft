use super::*;
use crate::world::{Block, CHUNK_SIZE, Chunk, ChunkCoord, MIN_WORLD_Y, WORLD_HEIGHT, WaterBiome};
use std::collections::{BTreeSet, VecDeque};

const SEEDS: [u64; 6] = [0x5255_5354_4352_4146, 42, 0, 1, 7, 2026];

#[test]
fn aperture_has_real_through_air_marine_water_and_two_grounded_connected_supports() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let site = terrain.coastal_landmark().unwrap();
        let arch = site.arch;
        let [cx, cy, cz] = arch.opening.map(|value| value.floor() as i32);
        assert!(arch.roof[0] - cy >= 3);
        assert!(arch.roof[0] > SEA_LEVEL);
        assert_eq!(terrain.block_at(cx, cy, cz), Block::Air);
        assert!(terrain.column(cx, cz).height < SEA_LEVEL);
        let water = terrain.water_at(cx, SEA_LEVEL - 1, cz).unwrap();
        assert_eq!(water.biome, WaterBiome::Ocean);
        assert_eq!(water.salinity, 1.0);
        assert!(water.flow.iter().all(|value| value.is_finite()));
        for y in water.bottom - 3..=water.bottom {
            assert!(terrain.block_at(cx, y, cz).is_solid());
        }
        for y in SEA_LEVEL..arch.roof[0] {
            assert_eq!(terrain.block_at(cx, y, cz), Block::Air);
            assert!(terrain.water_at(cx, y, cz).is_none());
        }
        for y in arch.roof[0]..=arch.roof[1] {
            assert!(terrain.block_at(cx, y, cz).is_solid());
            assert!(terrain.water_at(cx, y, cz).is_none());
        }
        for [x, _, z] in arch.supports {
            let top = terrain.column(x, z).height;
            assert!(terrain.water_column(x, z).is_none());
            for y in SEA_LEVEL - 4..=top {
                assert!(
                    terrain.block_at(x, y, z).is_solid(),
                    "seed {seed}: support gap ({x},{y},{z})"
                );
                assert!(terrain.water_at(x, y, z).is_none());
            }
        }
        let first = arch.supports[0];
        let mut seen = BTreeSet::from([first]);
        let mut queue = VecDeque::from([first]);
        while let Some(point) = queue.pop_front() {
            for offset in [
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
            ] {
                let next: [i32; 3] = std::array::from_fn(|axis| point[axis] + offset[axis]);
                if (next[0] - cx).abs() <= 22
                    && (next[2] - cz).abs() <= 22
                    && (SEA_LEVEL + 1..=arch.roof[1]).contains(&next[1])
                    && terrain.block_at(next[0], next[1], next[2]).is_solid()
                    && seen.insert(next)
                {
                    queue.push_back(next);
                }
            }
        }
        assert!(
            seen.contains(&arch.supports[1]),
            "seed {seed}: disconnected crown/piers"
        );
        assert!(seen.contains(&[cx, arch.roof[0], cz]));
    }
}

#[test]
fn all_sampling_paths_and_resident_metadata_agree_at_aperture_roof_and_seams() {
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let site = terrain.coastal_landmark().unwrap();
        let [cx, _, cz] = site.arch.opening.map(|value| value.floor() as i32);
        let coords = [[cx, cz], [cx - 8, cz - 8], [cx + 8, cz + 8]]
            .map(|[x, z]| ChunkCoord::new(x.div_euclid(16), z.div_euclid(16)));
        for coord in coords {
            let chunk = Chunk::generate(&terrain, coord);
            let positions: Vec<_> = (0..16)
                .flat_map(|z| (0..16).map(move |x| [coord.x * 16 + x, coord.z * 16 + z]))
                .collect();
            for (index, (position, blocks)) in positions
                .iter()
                .zip(terrain.column_block_batch(&positions))
                .enumerate()
            {
                let [x, z] = *position;
                let lx = index as i32 % CHUNK_SIZE;
                let lz = index as i32 / CHUNK_SIZE;
                assert_eq!(chunk.coastal_roof(lx, lz), terrain.coastal_roof(x, z));
                assert_eq!(blocks, terrain.column_blocks(x, z));
                for y in MIN_WORLD_Y..WORLD_HEIGHT {
                    let block = blocks[(y - MIN_WORLD_Y) as usize];
                    assert_eq!(chunk.block(lx, y, lz), block);
                    if y >= SEA_LEVEL - 4 {
                        assert_eq!(terrain.block_at(x, y, z), block);
                    }
                    let resident = chunk.water(lx, y, lz);
                    assert_eq!(resident.is_some(), block == Block::Water);
                    if resident.is_some() {
                        assert_eq!(resident, terrain.water_at(x, y, z));
                    }
                }
            }
        }
    }
}

#[test]
fn arch_projected_width_and_height_exceed_standing_aperture_minimums() {
    use super::super::super::hydrology::diagnostic_coast;
    for seed in SEEDS {
        let terrain = Terrain::new(seed);
        let coast = &diagnostic_coast(seed);
        let center = coast.arch.center(coast);
        let mut open_columns = 0;
        for dv in -3..=3 {
            for du in -7..=7 {
                let [x, z] = coast
                    .world([center[0] + f64::from(du), center[1] + f64::from(dv)])
                    .map(|value| value.floor() as i32);
                assert!(
                    (SEA_LEVEL + 1..=SEA_LEVEL + 4)
                        .all(|y| terrain.block_at(x, y, z) == Block::Air),
                    "seed {seed}: aperture obstruction ({x},{z})"
                );
                open_columns += 1;
            }
        }
        assert!(open_columns >= 7 * 15);
    }
}
