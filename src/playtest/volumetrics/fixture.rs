//! A private voxel room makes roof closure an air-only, same-pose occlusion test.

use super::{
    Error, FrameState, Instant, Path, Player, Renderer, Terrain, Vec3, aim_at, capture_scene,
    check_deadline, frame, io, load_scene, require_edges, require_upscale,
};
use crate::mesh::mesh_chunk_with_edits;
use crate::renderer::LightingFeatures;
use crate::world::{Block, ChunkCoord, ResidentWorld};

pub(super) fn verify(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut world = load_scene(renderer, terrain, ChunkCoord::new(0, 0), 2, deadline)?;
    for z in -15_i32..=15 {
        for x in -15_i32..=15 {
            for y in 44..=crate::world::WORLD_HEIGHT - 1 {
                let wall = y <= 60 && (x.abs() == 15 || z.abs() == 15 || y == 44 || y == 60);
                let aperture = y == 60 && (-8..=-5).contains(&x) && (-6..=-3).contains(&z);
                world.edit_block(
                    [x, y, z],
                    if wall && !aperture {
                        Block::Limestone
                    } else {
                        Block::Air
                    },
                );
            }
        }
    }
    let mut player = Player::new(Vec3::new(3.5, 47.0, 5.5));
    aim_at(&mut player, Vec3::new(-2.5, 53.0, -1.5));
    upload(renderer, &world, terrain, deadline)?;
    let open = frame(&world, &player, 2, 0.0);
    let beam = renderer.measure_air(&open)?;
    capture_scene(renderer, capture, "air-aperture", &open)?;
    if beam.peak_radiance < 0.001
        || beam.deviation < 0.0001
        || beam.lit_fraction < 0.01
        || beam.lit_fraction > 0.85
    {
        return Err(io::Error::other(format!(
            "roof aperture did not produce localized shafts with a roofed camera: {beam:?}"
        ))
        .into());
    }
    let reference = renderer.compare_lighting(
        &open,
        LightingFeatures {
            bloom: false,
            volumetric_reference: true,
            ..Default::default()
        },
    )?;
    require_upscale(reference)?;
    require_edges(renderer.measure_air_edges(&open)?)?;
    let (width, height) = renderer.size();
    renderer.resize(winit::dpi::PhysicalSize::new(641, 403));
    let odd_size = renderer.compare_lighting(
        &open,
        LightingFeatures {
            bloom: false,
            volumetric_reference: true,
            ..Default::default()
        },
    );
    renderer.resize(winit::dpi::PhysicalSize::new(width, height));
    require_upscale(odd_size?)?;
    for z in -6..=-3 {
        for x in -8..=-5 {
            world.edit_block([x, 60, z], Block::Limestone);
        }
    }
    upload(renderer, &world, terrain, deadline)?;
    let sealed = frame(&world, &player, 2, 0.0);
    let blocked = renderer.measure_air(&sealed)?;
    if blocked.peak_radiance > 0.000001 || blocked.lit_fraction != 0.0 {
        return Err(io::Error::other(format!(
            "closed overhang leaked sunlight into the air: {blocked:?}"
        ))
        .into());
    }
    let dark = capture_scene(renderer, capture, "air-sealed", &sealed)?;
    if dark.mean_luminance > 0.035 {
        return Err(io::Error::other(format!(
            "enclosed cave was brightened by scattering: {dark:?}"
        ))
        .into());
    }
    for medium in [
        FrameState {
            underwater: true,
            ..sealed
        },
        FrameState {
            in_lava: true,
            ..sealed
        },
    ] {
        let difference = renderer.compare_lighting(
            &medium,
            LightingFeatures {
                bloom: false,
                volumetrics: false,
                ..Default::default()
            },
        )?;
        if difference.mean_absolute_error != 0.0 || difference.changed_fraction != 0.0 {
            return Err(
                io::Error::other("air scattering changed water/lava medium rendering").into(),
            );
        }
    }
    if native && !renderer.render(&sealed)? {
        return Err(io::Error::other("volumetric acceptance frame was not presented").into());
    }
    println!(
        "Volumetric GPU verified: roof aperture lights {:.2}% of air (peak {:.5}); closing it gives zero air radiance, cave luminance {:.4}; water/lava unchanged; odd-size resize and full-resolution edge reference passed",
        beam.lit_fraction * 100.0,
        beam.peak_radiance,
        dark.mean_luminance
    );
    for z in -6..=-3 {
        for x in -8..=-5 {
            world.edit_block([x, 60, z], Block::Air);
        }
    }
    upload(renderer, &world, terrain, deadline)?;
    let restored = renderer.measure_air(&frame(&world, &player, 2, 0.0))?;
    if (restored.mean_radiance - beam.mean_radiance).abs() > 0.000001
        || (restored.lit_fraction - beam.lit_fraction).abs() > 0.000001
    {
        return Err(io::Error::other(
            "reopening the aperture did not restore the original air lighting",
        )
        .into());
    }
    println!("Volumetric GPU verified: reopening edited geometry restores identical beam energy");
    // Buried geology must stay dark even with the terrain cap above local Y coverage.
    for z in -14_i32..=14 {
        for x in -14_i32..=14 {
            for y in -51..=-39 {
                let wall = x.abs() == 14 || z.abs() == 14 || y == -51 || y == -39;
                world.edit_block([x, y, z], if wall { Block::Limestone } else { Block::Air });
            }
        }
    }
    upload(renderer, &world, terrain, deadline)?;
    let mut buried_player = Player::new(Vec3::new(3.5, -50.0, 5.5));
    aim_at(&mut buried_player, Vec3::new(-8.0, -45.0, -5.0));
    let buried = renderer.measure_air(&frame(&world, &buried_player, 2, 0.0))?;
    if buried.peak_radiance > 0.000001 || buried.lit_fraction != 0.0 {
        return Err(io::Error::other(format!(
            "buried negative-Y room leaked air light: {buried:?}"
        ))
        .into());
    }
    println!(
        "Volumetric GPU verified: sealed negative-Y cave at minimum residency has zero air radiance"
    );
    Ok(())
}

fn upload(
    renderer: &mut Renderer,
    world: &ResidentWorld,
    terrain: Terrain,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    for z in -1..=0 {
        for x in -1..=0 {
            check_deadline(deadline)?;
            let coord = ChunkCoord::new(x, z);
            let chunk = world
                .chunk(coord)
                .ok_or_else(|| io::Error::other("air fixture lost residency"))?;
            renderer.upload_chunk(
                coord,
                &mesh_chunk_with_edits(chunk, &terrain, world.edits()),
            );
        }
    }
    Ok(())
}
