use super::caves::{
    entrance_yaw, liquid_view_position, nearby_visible_growth, scenic_yaw, verify_cave_pose,
    verify_lamp_readback,
};
use super::cutaway::{
    embedded_cave_eye, solid_phase_eye, verify_phase_readback, verify_restored_readback,
};
use super::water::{
    submerged_water_pose, verify_surfaced_readback, verify_water_animation, verify_water_palette,
    verify_water_pass, verify_water_scene,
};
use super::{aim_at, check_deadline};
use crate::renderer::{FrameDifference, FrameStats};
use crate::world::{Block, CaveBiome, Terrain, WaterBiome, WaterSample};
use glam::Vec3;
use std::time::{Duration, Instant};

#[test]
fn generated_cave_camera_finds_a_finite_open_view() {
    let terrain = Terrain::default();
    let position = Vec3::from_array(terrain.cave_position(CaveBiome::Rocky).unwrap());
    let eye = position + Vec3::Y * 1.62;
    let yaw = scenic_yaw(&terrain, eye);
    assert!(yaw.is_finite());
    let ahead = eye + Vec3::new(yaw.sin(), 0.0, -yaw.cos()) * 2.0;
    assert!(
        !terrain
            .block_at(
                ahead.x.floor() as i32,
                ahead.y.floor() as i32,
                ahead.z.floor() as i32
            )
            .is_solid()
    );
}

#[test]
fn expired_diagnostic_deadline_cannot_report_success() {
    assert!(check_deadline(Instant::now() - Duration::from_secs(1)).is_err());
    assert!(check_deadline(Instant::now() + Duration::from_secs(1)).is_ok());
}

#[test]
fn aquatic_readback_rejects_flat_fog_and_static_animation() {
    let scene = FrameStats {
        mean_luminance: 0.25,
        visible_fraction: 0.8,
        luminance_deviation: 0.08,
    };
    assert!(verify_water_scene(scene, "ocean").is_ok());
    assert!(
        verify_water_scene(
            FrameStats {
                luminance_deviation: 0.0,
                ..scene
            },
            "flat"
        )
        .is_err()
    );
    assert!(
        verify_water_scene(
            FrameStats {
                visible_fraction: 0.0,
                ..scene
            },
            "void"
        )
        .is_err()
    );
    let moving = FrameDifference {
        mean_absolute_error: 0.015,
        changed_fraction: 0.25,
    };
    assert!(verify_water_animation(moving, "river").is_ok());
    assert!(verify_water_palette([0.18, 0.40, 0.29], [0.11, 0.30, 0.48]).is_ok());
    assert!(verify_water_palette([0.3; 3], [0.3; 3]).is_err());
    assert!(verify_water_palette([f32::NAN; 3], [0.3; 3]).is_err());
    assert!(
        verify_water_scene(
            FrameStats {
                luminance_deviation: f32::NAN,
                ..scene
            },
            "invalid"
        )
        .is_err()
    );
    assert!(
        verify_water_animation(
            FrameDifference {
                changed_fraction: f32::NAN,
                ..moving
            },
            "invalid"
        )
        .is_err()
    );
    assert!(
        verify_water_animation(
            FrameDifference {
                changed_fraction: 0.0,
                ..moving
            },
            "static"
        )
        .is_err()
    );
    assert!(
        verify_water_animation(
            FrameDifference {
                mean_absolute_error: 0.0,
                ..moving
            },
            "static"
        )
        .is_err()
    );
}

#[test]
fn submerged_camera_keeps_body_above_bed_and_eye_below_actual_waterline() {
    let water = WaterSample {
        bottom: -8,
        surface: 18.0,
        flow: [0.0; 3],
        biome: WaterBiome::Ocean,
        salinity: 1.0,
    };
    let feet = submerged_water_pose(water, [-17, 17, -1]).unwrap();
    assert!(feet.y > (water.bottom + 1) as f32);
    assert!(feet.y + crate::player::EYE_HEIGHT < water.surface);
    assert_eq!((feet.x, feet.z), (-16.5, -0.5));
    assert!(
        submerged_water_pose(
            WaterSample {
                surface: f32::NAN,
                ..water
            },
            [0, 17, 0]
        )
        .is_err()
    );
    assert!(
        submerged_water_pose(
            WaterSample {
                bottom: 15,
                ..water
            },
            [0, 17, 0]
        )
        .is_err()
    );
}

#[test]
fn water_surface_acceptance_requires_a_real_draw_pass_and_restored_air_optics() {
    let surface = FrameDifference {
        mean_absolute_error: 0.04,
        changed_fraction: 0.8,
    };
    let empty = FrameDifference {
        mean_absolute_error: 0.0,
        changed_fraction: 0.0,
    };
    assert!(verify_water_pass(surface, "ocean underside").is_ok());
    assert!(verify_water_pass(empty, "fog alone").is_err());
    assert!(
        verify_water_pass(
            FrameDifference {
                changed_fraction: 0.0,
                ..surface
            },
            "hidden surface"
        )
        .is_err()
    );
    assert!(
        verify_water_pass(
            FrameDifference {
                mean_absolute_error: 0.0,
                ..surface
            },
            "unchanged pixels"
        )
        .is_err()
    );
    assert!(
        verify_water_pass(
            FrameDifference {
                mean_absolute_error: f32::NAN,
                ..surface
            },
            "invalid"
        )
        .is_err()
    );
    assert!(verify_surfaced_readback(empty).is_ok());
    assert!(verify_surfaced_readback(surface).is_err());
    assert!(
        verify_surfaced_readback(FrameDifference {
            changed_fraction: f32::NAN,
            ..empty
        })
        .is_err()
    );
}

#[test]
fn cutaway_diagnostic_starts_in_real_rock_and_looks_toward_a_cave() {
    let terrain = Terrain::default();
    let cave = Vec3::from_array(terrain.cave_position(CaveBiome::Rocky).unwrap());
    let eye = cave + Vec3::Y * crate::player::EYE_HEIGHT;
    let embedded = embedded_cave_eye(terrain, eye).unwrap();
    assert!(
        terrain
            .block_at(
                embedded.x.floor() as i32,
                embedded.y.floor() as i32,
                embedded.z.floor() as i32
            )
            .is_solid()
    );
    let mut player = crate::player::Player::new(embedded - Vec3::Y * crate::player::EYE_HEIGHT);
    aim_at(&mut player, eye);
    assert!(player.look_direction().dot((eye - embedded).normalize()) > 0.999);
}

#[test]
fn crystal_capture_targets_a_real_nearby_mineral_not_only_its_biome_label() {
    let terrain = Terrain::default();
    let position = Vec3::from_array(terrain.cave_position(CaveBiome::Crystal).unwrap());
    let eye = position + Vec3::Y * crate::player::EYE_HEIGHT;
    let target = nearby_visible_growth(terrain, eye, Block::Crystal).unwrap();
    assert!(eye.distance(target) <= 22.0);
    assert_eq!(
        terrain.block_at(
            target.x.floor() as i32,
            target.y.floor() as i32,
            target.z.floor() as i32
        ),
        Block::Crystal
    );
}

#[test]
fn phase_readback_rejects_a_void_or_unchanged_render() {
    let reference = FrameStats {
        mean_luminance: 0.02,
        visible_fraction: 0.01,
        luminance_deviation: 0.0,
    };
    let visible = FrameStats {
        mean_luminance: 0.18,
        visible_fraction: 0.75,
        luminance_deviation: 0.05,
    };
    assert!(verify_phase_readback(reference, visible).is_ok());
    assert!(verify_phase_readback(reference, reference).is_err());
    assert!(
        verify_phase_readback(
            reference,
            FrameStats {
                visible_fraction: 0.0,
                ..visible
            }
        )
        .is_err()
    );
    assert!(verify_phase_readback(visible, visible).is_err());
    assert!(verify_restored_readback(visible, visible).is_ok());
    assert!(verify_restored_readback(visible, reference).is_err());
    assert!(
        verify_phase_readback(
            reference,
            FrameStats {
                luminance_deviation: 0.0,
                ..visible
            }
        )
        .is_err()
    );
}

#[test]
fn deep_phase_diagnostic_is_surrounded_by_known_solid_blocks() {
    let terrain = Terrain::default();
    let eye = solid_phase_eye(terrain, Vec3::new(8.5, -52.5, 8.5)).unwrap();
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for distance in [-2.0, 0.0, 2.0] {
            let point = eye + axis * distance;
            assert!(
                terrain
                    .block_at(
                        point.x.floor() as i32,
                        point.y.floor() as i32,
                        point.z.floor() as i32
                    )
                    .is_solid()
            );
        }
    }
}

#[test]
fn natural_entrance_camera_prefers_an_open_downward_view() {
    let terrain = Terrain::default();
    let position = Vec3::from_array(terrain.entrance_position().unwrap());
    let eye = position + Vec3::Y * 3.12;
    let yaw = entrance_yaw(terrain, eye);
    assert!(yaw.is_finite());
    let point = eye + Vec3::new(yaw.sin(), -0.5, -yaw.cos()) * 2.0;
    assert!(
        !terrain
            .block_at(
                point.x.floor() as i32,
                point.y.floor() as i32,
                point.z.floor() as i32
            )
            .is_solid()
    );
}

#[test]
fn readback_requires_visible_geometry_and_real_headlamp_darkening() {
    let lit = FrameStats {
        mean_luminance: 0.2,
        visible_fraction: 0.5,
        luminance_deviation: 0.1,
    };
    let dark = FrameStats {
        mean_luminance: 0.02,
        visible_fraction: 0.01,
        luminance_deviation: 0.01,
    };
    assert!(verify_lamp_readback(lit, dark, "rocky").is_ok());
    assert!(
        verify_lamp_readback(
            FrameStats {
                visible_fraction: 0.0,
                ..lit
            },
            dark,
            "black"
        )
        .is_err()
    );
    assert!(verify_lamp_readback(lit, lit, "unchanged").is_err());
    assert!(verify_lamp_readback(dark, lit, "reversed").is_err());
}

#[test]
fn liquid_diagnostics_find_exposed_pools_and_obey_expired_deadlines() {
    let terrain = Terrain::default();
    let deadline = Instant::now() + Duration::from_secs(10);
    for liquid in [Block::Water, Block::Lava] {
        let position = liquid_view_position(terrain, liquid, deadline)
            .unwrap()
            .unwrap();
        assert!(position.y < 0.0);
        assert_eq!(
            terrain.block_at(
                position.x.floor() as i32,
                position.y.floor() as i32 - 3,
                position.z.floor() as i32
            ),
            liquid
        );
        assert_eq!(
            terrain.block_at(
                position.x.floor() as i32,
                position.y.floor() as i32 + 1,
                position.z.floor() as i32
            ),
            Block::Air
        );
    }
    assert!(
        liquid_view_position(
            terrain,
            Block::Water,
            Instant::now() - Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
fn diagnostic_poses_support_cave_movement_and_reject_unsupported_air() {
    let terrain = Terrain::default();
    for biome in [CaveBiome::Rocky, CaveBiome::Lush, CaveBiome::Crystal] {
        let position = Vec3::from_array(terrain.cave_position(biome).unwrap());
        assert!(verify_cave_pose(terrain, position, biome.name()).is_ok());
    }
    let spawn = Vec3::from_array(terrain.spawn_position());
    assert!(verify_cave_pose(terrain, spawn + Vec3::Y * 12.0, "unsupported").is_err());
}
