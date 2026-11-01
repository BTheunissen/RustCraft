use super::*;
use crate::world::{Biome, Block};

#[test]
fn acceptance_rejects_absent_flat_invalid_and_fog_bank_air() {
    let beam = AirStats {
        mean_radiance: 0.01,
        peak_radiance: 0.05,
        deviation: 0.005,
        lit_fraction: 0.4,
    };
    assert!(require_readable_air(beam, "beam").is_ok());
    for bad in [
        AirStats {
            mean_radiance: 0.0,
            ..beam
        },
        AirStats {
            mean_radiance: f32::NAN,
            ..beam
        },
        AirStats {
            peak_radiance: 0.6,
            ..beam
        },
        AirStats {
            peak_radiance: f32::INFINITY,
            ..beam
        },
        AirStats {
            deviation: 0.0,
            ..beam
        },
        AirStats {
            deviation: f32::NAN,
            ..beam
        },
        AirStats {
            lit_fraction: 0.0,
            ..beam
        },
        AirStats {
            lit_fraction: f32::NAN,
            ..beam
        },
    ] {
        assert!(require_readable_air(bad, "invalid").is_err());
    }
}

#[test]
fn upscale_acceptance_rejects_background_halos_and_nonfinite_pixels() {
    let clean = FrameDifference {
        mean_absolute_error: 0.0003,
        changed_fraction: 0.01,
    };
    assert!(require_upscale(clean).is_ok());
    for bad in [
        FrameDifference {
            mean_absolute_error: 0.01,
            ..clean
        },
        FrameDifference {
            mean_absolute_error: f32::NAN,
            ..clean
        },
        FrameDifference {
            changed_fraction: 0.2,
            ..clean
        },
        FrameDifference {
            changed_fraction: f32::NAN,
            ..clean
        },
    ] {
        assert!(require_upscale(bad).is_err());
    }
}

#[test]
fn generated_forest_acceptance_pose_starts_in_known_air_for_two_seeds() {
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let (position, target) = views::forest_view(terrain).unwrap();
        let eye = position + Vec3::Y * crate::player::EYE_HEIGHT;
        let [x, y, z] = eye.floor().as_ivec3().to_array();
        assert_eq!(terrain.column(x, z).biome, Biome::Forest);
        assert_eq!(terrain.block_at(x, y, z), Block::Air);
        assert!(position.y > crate::world::SEA_LEVEL as f32);
        let [tx, ty, tz] = target.floor().as_ivec3().to_array();
        assert!(matches!(
            terrain.block_at(tx, ty, tz),
            Block::Wood | Block::Leaves
        ));
        assert!(position.distance(target) < 12.0);
    }
}

#[test]
fn edge_acceptance_requires_real_silhouettes_and_bounded_linear_error() {
    let clean = AirEdgeStats {
        pixels: 150,
        mean_error: 0.0002,
        peak_error: 0.003,
    };
    assert!(require_edges(clean).is_ok());
    for bad in [
        AirEdgeStats { pixels: 0, ..clean },
        AirEdgeStats {
            mean_error: 0.005,
            ..clean
        },
        AirEdgeStats {
            mean_error: f32::NAN,
            ..clean
        },
        AirEdgeStats {
            peak_error: 0.15,
            ..clean
        },
        AirEdgeStats {
            peak_error: f32::INFINITY,
            ..clean
        },
    ] {
        assert!(require_edges(bad).is_err());
    }
}

#[test]
fn broad_entrance_has_roofed_air_beyond_the_outdoor_center_hint() {
    let terrain = Terrain::default();
    let position = Vec3::from_array(terrain.entrance_position().unwrap()) + Vec3::Y * 1.5;
    let mut player = Player::new(position);
    player.yaw = super::super::caves::entrance_yaw(terrain, player.eye_position());
    player.pitch = -0.45;
    let eye = player.eye_position();
    let matrix =
        crate::camera::view_projection(eye, player.look_direction(), 1200.0 / 760.0, 192.0);
    let point = matrix.inverse()
        * glam::Vec4::new(
            640.0 / 1200.0 * 2.0 - 1.0,
            1.0 - 320.0 / 760.0 * 2.0,
            1.0,
            1.0,
        );
    let ray = (point.truncate() / point.w - eye).normalize();
    let hint = crate::lighting::camera_cavity_clearance(eye, ray, |x, y, z| {
        Some(terrain.block_at(x, y, z))
    });
    assert_eq!(hint, 1000.0);
    let point = eye + ray * 28.0;
    let [x, y, z] = point.floor().as_ivec3().to_array();
    assert_eq!(terrain.block_at(x, y, z), Block::Air);
    assert_eq!(
        crate::lighting::sky_exposure(point, |x, y, z| terrain.block_at(x, y, z)),
        0.0
    );
    let sides = [[-1, 0], [1, 0], [0, -1], [0, 1]]
        .into_iter()
        .filter(|[dx, dz]| {
            (1..=8).any(|distance| {
                matches!(
                    terrain.block_at(x + dx * distance, y, z + dz * distance),
                    Block::Stone | Block::Limestone | Block::Dirt
                )
            })
        })
        .count();
    assert!(sides >= 2);
    assert_eq!(
        crate::lighting::camera_cavity_clearance(eye + Vec3::Y * 30.0, ray, |x, y, z| Some(
            terrain.block_at(x, y, z)
        )),
        1000.0
    );
}

#[test]
fn seed42_entrance_enclosure_near_local_volume_edge_requires_far_background_samples() {
    let terrain = Terrain::new(42);
    let position = Vec3::from_array(terrain.entrance_position().unwrap()) + Vec3::Y * 1.5;
    let mut player = Player::new(position);
    player.yaw = super::super::caves::entrance_yaw(terrain, player.eye_position());
    player.pitch = -0.45;
    let eye = player.eye_position();
    let matrix =
        crate::camera::view_projection(eye, player.look_direction(), 1200.0 / 760.0, 192.0);
    let point = matrix.inverse()
        * glam::Vec4::new(
            734.0 / 1200.0 * 2.0 - 1.0,
            1.0 - 390.0 / 760.0 * 2.0,
            1.0,
            1.0,
        );
    let ray = (point.truncate() / point.w - eye).normalize();
    let origin = crate::lighting::volume::volume_origin(eye).unwrap();
    let open = eye + ray * 32.0;
    assert_eq!(
        crate::lighting::sky_exposure(open, |x, y, z| terrain.block_at(x, y, z)),
        1.0
    );
    let point = eye + ray * 34.0;
    let [x, y, z] = point.floor().as_ivec3().to_array();
    assert_eq!(terrain.block_at(x, y, z), Block::Air);
    assert_eq!(
        crate::lighting::sky_exposure(point, |x, y, z| terrain.block_at(x, y, z)),
        0.0
    );
    assert!(x >= origin[0] && x < origin[0] + crate::lighting::volume::LIGHT_VOLUME_SIZE);
    assert!((1..=8).any(|step| terrain.block_at(x, y, z - step).is_opaque()));
    assert!((1..=8).any(|step| terrain.block_at(x, y, z + step).is_opaque()));
}

#[test]
fn inside_entrance_view_has_actual_roofed_air_and_a_clear_route_to_the_opening() {
    use crate::world::{Chunk, WorldView};
    for terrain in [Terrain::default(), Terrain::new(42)] {
        let position = Vec3::from_array(terrain.entrance_position().unwrap()) + Vec3::Y * 1.5;
        let mut player = Player::new(position);
        player.yaw = super::super::caves::entrance_yaw(terrain, player.eye_position());
        player.pitch = -0.45;
        let center = ChunkCoord::from_world(position.x, position.z);
        let mut world = ResidentWorld::default();
        for z in -4..=4 {
            for x in -4..=4 {
                world.insert(Chunk::generate(
                    &terrain,
                    ChunkCoord::new(center.x + x, center.z + z),
                ));
            }
        }
        let (eye, target) = views::entrance_view(&world, &player, 1200.0 / 760.0).unwrap();
        let [x, y, z] = eye.floor().as_ivec3().to_array();
        assert_eq!(world.block_at(x, y, z), Some(Block::Air));
        assert_eq!(
            crate::lighting::sky_exposure(eye, |x, y, z| world
                .block_at(x, y, z)
                .unwrap_or(Block::Stone)),
            0.0
        );
        for step in 0..=64 {
            let [x, y, z] = eye
                .lerp(target, step as f32 / 64.0)
                .floor()
                .as_ivec3()
                .to_array();
            assert_eq!(world.block_at(x, y, z), Some(Block::Air));
        }
    }
    assert!(
        views::entrance_view(&ResidentWorld::default(), &Player::new(Vec3::ZERO), 1.0).is_none()
    );
}
