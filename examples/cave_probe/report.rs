//! Bounded survey output.

use super::*;

pub(super) fn run(seed: u64) -> Result<(), String> {
    let terrain = Terrain::new(seed);
    let diagnostic_started = Instant::now();
    let spawn = Vec3::from_array(terrain.spawn_position());
    let mut caves = Vec::new();
    for biome in BIOMES {
        let feet = terrain
            .cave_position(biome)
            .map(Vec3::from_array)
            .ok_or_else(|| format!("no nearby safe {} cave diagnostic pose", biome.name()))?;
        let point = feet.to_array().map(|coordinate| coordinate.floor() as i32);
        if !feet.is_finite()
            || feet.y >= 0.0
            || Vec3::new(feet.x - spawn.x, 0.0, feet.z - spawn.z).length() > 250.0
            || terrain.cave_biome(point[0], point[1], point[2]) != biome
        {
            return Err(format!("invalid {} cave pose: {feet:?}", biome.name()));
        }
        caves.push((biome, feet));
    }
    let mut landmarks = Vec::new();
    for landform in LANDFORMS {
        let landmark = terrain
            .cave_landmark(landform)
            .ok_or_else(|| format!("no nearby safe {} cave landmark", landform.name()))?;
        landmarks.push((landform, landmark));
    }
    let entrance = terrain.entrance_position().map(Vec3::from_array);
    let sites: Vec<_> = caves
        .iter()
        .map(|(_, feet)| *feet)
        .chain(
            landmarks
                .iter()
                .map(|(_, landmark)| Vec3::from_array(landmark.position)),
        )
        .collect();
    let support_sites: Vec<_> = std::iter::once(spawn).chain(entrance).collect();
    let diagnostic_lookup = diagnostic_started.elapsed();
    let (survey, generation) = Survey::generate(terrain, &sites, &support_sites);
    let spawn_floor = survey.validate_diagnostic(spawn)?;
    println!(
        "seed={seed} spawn feet={:?} support={spawn_floor:?}; diagnostic lookup={:.3} ms (excluded from chunk generation/mesh timings)",
        spawn.to_array(),
        diagnostic_lookup.as_secs_f64() * 1_000.0,
    );
    let counts = survey.underground_blocks();
    print!(
        "survey: {} distinct chunks; negative-Y block counts:",
        survey.chunks.len()
    );
    for (block, count) in Block::ALL.iter().zip(counts) {
        if count > 0 {
            print!(" {block:?}={count}");
        }
    }
    println!();
    let mut mesh_survey = MeshSurvey::default();
    let mut meshing = Duration::ZERO;
    for chunk in survey.chunks.values() {
        let started = Instant::now();
        let mesh = mesh_chunk(chunk, &terrain);
        meshing += started.elapsed();
        mesh_survey.record(&mesh.opaque, false)?;
        mesh_survey.record(&mesh.water, true)?;
    }
    println!(
        "generation={:.3} ms ({:.3}/chunk), meshing={:.3} ms ({:.3}/chunk); opaque={}/{} vertices/triangles, water={}/{}",
        generation.as_secs_f64() * 1_000.0,
        generation.as_secs_f64() * 1_000.0 / survey.chunks.len() as f64,
        meshing.as_secs_f64() * 1_000.0,
        meshing.as_secs_f64() * 1_000.0 / survey.chunks.len() as f64,
        mesh_survey.vertices[0],
        mesh_survey.triangles[0],
        mesh_survey.vertices[1],
        mesh_survey.triangles[1],
    );
    print!("  exposed cave faces:");
    for (name, count) in MATERIALS.iter().zip(mesh_survey.underground_faces) {
        if count > 0 {
            print!(" {name}={count}");
        }
    }
    println!(
        "; normalized light={:.3}..{:.3}",
        mesh_survey.light_min, mesh_survey.light_max
    );
    survey.exposed_material_features().report("survey");
    for (label, point) in ["water", "lava"]
        .into_iter()
        .zip(mesh_survey.exposed_liquids)
    {
        println!(
            "  exposed underground {label} block={point:?} (non-collidable voxels; resting checks use the legacy solidity-only controller)"
        );
    }
    for material in [Block::Water, Block::Lava] {
        if let Some(view) = survey.pool_viewpoint(material) {
            let direction = (view.target - view.feet - Vec3::Y * EYE_HEIGHT).normalize();
            println!(
                "  GPU pool candidate: {material:?} 3x3 open surface block={:?}, feet={:?}, target={:?}, yaw={:.3}, pitch={:.3} radians",
                view.block,
                view.feet.to_array(),
                view.target.to_array(),
                direction.x.atan2(-direction.z),
                direction.y.asin(),
            );
        } else {
            println!(
                "  no supported line-of-sight {material:?} 3x3 pool viewpoint in surveyed chunks"
            );
        }
    }
    for material in [Block::CoalOre, Block::IronOre, Block::GoldOre] {
        if let Some(view) = survey.ore_viewpoint(material) {
            let direction = (view.target - view.feet - Vec3::Y * EYE_HEIGHT).normalize();
            println!(
                "  GPU ore candidate: {material:?} block={:?}, feet={:?}, yaw={:.3}, pitch={:.3} radians",
                view.block,
                view.feet.to_array(),
                direction.x.atan2(-direction.z),
                direction.y.asin(),
            );
        } else {
            println!("  no supported line-of-sight {material:?} viewpoint in surveyed chunks");
        }
    }
    for (landform, landmark) in landmarks {
        survey.validate_landmark(landmark, landform)?;
        let feet = Vec3::from_array(landmark.position);
        let target = Vec3::from_array(landmark.target);
        let direction = (target - feet - Vec3::Y * EYE_HEIGHT).normalize();
        let connectivity = survey.observed_component(feet)?;
        println!(
            "{} landmark: feet={:?}, target={:?}, yaw={:.3}, pitch={:.3} radians; two-block-air component={} negative cells, bounds={:?}..{:?}, clipped={}",
            landform.name(),
            landmark.position,
            landmark.target,
            direction.x.atan2(-direction.z),
            direction.y.asin(),
            connectivity.negative_cells,
            connectivity.min,
            connectivity.max,
            connectivity.clipped,
        );
        Morphology::inspect(feet, &connectivity.clear, |point| survey.block(point)).report();
    }
    for (biome, feet) in caves {
        let floor = survey.validate_diagnostic(feet)?;
        let connectivity = survey.connectivity(feet)?;
        println!(
            "{}: feet={:?}, support={floor:?}; two-block-air component={} cells ({} negative), bounds={:?}..{:?}, sky-connected={}, horizontally clipped={}",
            biome.name(),
            feet.to_array(),
            connectivity.cells,
            connectivity.negative_cells,
            connectivity.min,
            connectivity.max,
            connectivity.sky_connected,
            connectivity.clipped,
        );
        let mut exposed = [0; Block::COUNT];
        for &point in &connectivity.exposed {
            exposed[survey.block(point) as usize] += 1;
        }
        print!("  component-adjacent underground blocks:");
        for (block, count) in Block::ALL.iter().zip(exposed) {
            if count > 0 {
                print!(" {block:?}={count}");
            }
        }
        println!();
        if let Some((material, point)) = survey.visible_feature(feet, biome, &connectivity) {
            let direction = (block_center(point) - feet - Vec3::Y * EYE_HEIGHT).normalize();
            println!(
                "  GPU candidate: visible {material:?} block={point:?}, yaw={:.3}, pitch={:.3} radians",
                direction.x.atan2(-direction.z),
                direction.y.asin(),
            );
        } else {
            println!("  no sampled line-of-sight feature at this diagnostic pose");
        }
    }
    if let Some(feet) = entrance {
        let floor = survey.validate_resting(feet)?;
        let connectivity = survey.connectivity(feet)?;
        println!(
            "entrance: feet={:?}, support={floor:?}; connected negative-Y clear cells={} (local 3x3 window)",
            feet.to_array(),
            connectivity.negative_cells,
        );
    } else {
        println!("entrance: no diagnostic pose supplied");
    }
    println!(
        "connectivity measures clear-air/flight space, not a proven walking route; GPU visibility is unverified"
    );
    Ok(())
}
