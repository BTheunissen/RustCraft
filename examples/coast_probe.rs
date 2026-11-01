//! Bounded CPU coastline survey; views are diagnostics, not gameplay navigation.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use rustcraft::mesh::mesh_chunk;
use rustcraft::world::{
    Block, CHUNK_SIZE, Chunk, ChunkCoord, CoastalLandmark, SEA_LEVEL, Terrain, WaterBiome,
};

fn seed_argument(arguments: &[String]) -> Result<Option<u64>, String> {
    match arguments {
        [] => Ok(Some(Terrain::default().seed())),
        [flag] if flag == "--help" || flag == "-h" => Ok(None),
        [flag, seed] if flag == "--seed" => seed
            .parse()
            .map(Some)
            .map_err(|_| "--seed requires an unsigned integer".into()),
        _ => Err("usage: coast_probe [--seed NUMBER]".into()),
    }
}

fn main() -> Result<(), String> {
    let Some(seed) = seed_argument(&std::env::args().skip(1).collect::<Vec<_>>())? else {
        println!("CPU-only coastline survey: coast_probe [--seed NUMBER]");
        return Ok(());
    };
    let terrain = Terrain::new(seed);
    let started = Instant::now();
    let site = terrain
        .coastal_landmark()
        .ok_or("no supported early coastal viewpoint/cove")?;
    validate_geometry(&terrain, &site)?;
    println!(
        "seed={seed}; spawn feet={:?}; coast bounds={:?}; viewpoint={:?}; cove={:?}; lookup/geometry={:.3} ms",
        terrain.spawn_position(),
        site.bounds,
        site.viewpoint,
        site.cove,
        started.elapsed().as_secs_f64() * 1_000.0
    );
    for (index, stack) in site.stacks.iter().enumerate() {
        println!(
            "stack {index}: center={:?}; actual top={}; height above mean marine water={}",
            stack.center,
            stack.top,
            stack.top + 1 - SEA_LEVEL
        );
    }
    println!(
        "arch: actual opening={:?}; roof={:?}; grounded supports={:?}",
        site.arch.opening, site.arch.roof, site.arch.supports
    );
    let started = Instant::now();
    let routes = rustcraft::game::survey_coastal_routes(terrain)?;
    println!(
        "walking routes controller-replayed forward/reverse: approach/cove/beach lengths={:.1}/{:.1}/{:.1}; rises={}/{}/{}; survey={} chunks, {} nodes, {} controller updates; route work={:.3} ms",
        routes.approach.length,
        routes.descent.length,
        routes.beach.length,
        routes.approach.rises,
        routes.descent.rises,
        routes.beach.rises,
        routes.surveyed_chunks,
        routes.expanded_nodes,
        routes.replay_steps,
        started.elapsed().as_secs_f64() * 1_000.0
    );
    let mut coords = BTreeSet::new();
    for [x, z] in site.stacks.iter().map(|stack| stack.center).chain(
        [site.viewpoint, site.cove].map(|feet| [feet[0].floor() as i32, feet[2].floor() as i32]),
    ) {
        let center = ChunkCoord::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        for dz in -1..=1 {
            for dx in -1..=1 {
                coords.insert(ChunkCoord::new(center.x + dx, center.z + dz));
            }
        }
    }
    if coords.len() > 72 {
        return Err("coast geometry survey exceeded 72-chunk budget".into());
    }
    let count = coords.len();
    let mut generation = Duration::ZERO;
    let mut meshing = Duration::ZERO;
    let mut vertices = [0; 2];
    let mut triangles = [0; 2];
    let mut wet = 0;
    for coord in coords {
        let started = Instant::now();
        let chunk = Chunk::generate(&terrain, coord);
        generation += started.elapsed();
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                if let Some(water) = chunk.water(x, SEA_LEVEL - 1, z) {
                    if water.biome != WaterBiome::Ocean
                        || water.salinity != 1.0
                        || !water.flow.iter().all(|value| value.is_finite())
                    {
                        return Err(format!(
                            "invalid marine occupancy at {coord:?} ({x},{z}): {water:?}"
                        ));
                    }
                    if !chunk.block(x, water.bottom, z).is_solid() {
                        return Err("unsupported marine column".into());
                    }
                    wet += 1;
                }
            }
        }
        let started = Instant::now();
        let mesh = mesh_chunk(&chunk, &terrain);
        meshing += started.elapsed();
        for (index, pass) in [&mesh.opaque, &mesh.water].into_iter().enumerate() {
            vertices[index] += pass.vertices.len();
            triangles[index] += pass.indices.len() / 3;
        }
    }
    println!(
        "{count} distinct geometry chunks; supported marine columns={wet}; generation={:.3} ms ({:.3}/chunk); meshing={:.3} ms ({:.3}/chunk); opaque/water vertices={vertices:?}, triangles={triangles:?}",
        generation.as_secs_f64() * 1_000.0,
        generation.as_secs_f64() * 1_000.0 / count as f64,
        meshing.as_secs_f64() * 1_000.0,
        meshing.as_secs_f64() * 1_000.0 / count as f64
    );
    println!(
        "CPU geometry/fluid/mesh survey only: physical walking, visual silhouettes, and owner acceptance are not certified by this probe."
    );
    Ok(())
}

fn validate_geometry(terrain: &Terrain, site: &CoastalLandmark) -> Result<(), String> {
    if !(3..=7).contains(&site.stacks.len()) {
        return Err("coast has an invalid main-stack count".into());
    }
    let mut heights = BTreeSet::new();
    for stack in &site.stacks {
        let [x, z] = stack.center;
        heights.insert(stack.top);
        if !(SEA_LEVEL + 16..=SEA_LEVEL + 36).contains(&stack.top)
            || !terrain.block_at(x, stack.top, z).is_solid()
            || terrain.block_at(x, stack.top + 1, z) != Block::Air
            || !(SEA_LEVEL - 5..=stack.top).all(|y| terrain.block_at(x, y, z).is_solid())
            || terrain.water_column(x, z).is_some()
        {
            return Err(format!("unsupported or invalid stack {stack:?}"));
        }
    }
    if heights.len() < 2 {
        return Err("coast has repeated stack heights".into());
    }
    for feet in [site.viewpoint, site.cove] {
        let [x, y, z] = feet.map(|value| value.floor() as i32);
        if !terrain.block_at(x, y - 1, z).is_solid()
            || !(0..3).all(|dy| terrain.block_at(x, y + dy, z) == Block::Air)
            || terrain.water_column(x, z).is_some()
        {
            return Err("coastal view/cove is not supported dry standing ground".into());
        }
    }
    let [x, y, z] = site.arch.opening.map(|value| value.floor() as i32);
    if !(y..site.arch.roof[0]).all(|y| terrain.block_at(x, y, z) == Block::Air)
        || !(site.arch.roof[0]..=site.arch.roof[1]).all(|y| terrain.block_at(x, y, z).is_solid())
        || !site
            .arch
            .supports
            .iter()
            .all(|&[x, y, z]| terrain.block_at(x, y, z).is_solid())
        || terrain
            .water_column(x, z)
            .is_none_or(|water| water.biome != WaterBiome::Ocean || water.salinity != 1.0)
    {
        return Err("arch has no real supported marine through-opening".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_reject_ambiguous_missing_and_negative_seeds() {
        assert_eq!(seed_argument(&[]).unwrap(), Some(Terrain::default().seed()));
        assert_eq!(seed_argument(&["--help".into()]).unwrap(), None);
        assert_eq!(
            seed_argument(&["--seed".into(), "42".into()]).unwrap(),
            Some(42)
        );
        for arguments in [
            vec!["--seed".into()],
            vec!["--seed".into(), "-1".into()],
            vec!["42".into()],
        ] {
            assert!(seed_argument(&arguments).is_err());
        }
    }

    #[test]
    fn generated_geometry_passes_but_missing_unsupported_and_repeated_stacks_fail() {
        let terrain = Terrain::default();
        let site = terrain.coastal_landmark().unwrap();
        validate_geometry(&terrain, &site).unwrap();
        let mut invalid = site.clone();
        invalid.stacks.clear();
        assert!(validate_geometry(&terrain, &invalid).is_err());
        let mut invalid = site.clone();
        invalid.stacks[0].top = 70;
        assert!(validate_geometry(&terrain, &invalid).is_err());
        let mut invalid = site.clone();
        invalid.stacks.fill(site.stacks[0]);
        assert!(validate_geometry(&terrain, &invalid).is_err());
        let mut invalid = site;
        invalid.cove[1] += 10.0;
        assert!(validate_geometry(&terrain, &invalid).is_err());
    }
}
