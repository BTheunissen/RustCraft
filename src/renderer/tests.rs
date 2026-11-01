use super::{
    CameraUniform, check_gpu_errors, gpu_wait_timeout, padded_row_bytes, phase_chunk_near,
    phase_material_table, scene_color, scene_difference, scene_stats, unpack_pixels,
    water_animation_time,
};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn camera_layout_matches_wgsl_uniform_alignment() {
    assert_eq!(size_of::<CameraUniform>(), 480);
    assert_eq!(std::mem::offset_of!(CameraUniform, eye), 128);
    assert_eq!(std::mem::offset_of!(CameraUniform, settings), 192);
    assert_eq!(std::mem::offset_of!(CameraUniform, look), 208);
    assert_eq!(std::mem::offset_of!(CameraUniform, environment), 224);
    assert_eq!(std::mem::offset_of!(CameraUniform, lamp), 240);
    assert_eq!(std::mem::offset_of!(CameraUniform, phase_origin), 256);
    assert_eq!(std::mem::offset_of!(CameraUniform, phase_settings), 272);
    assert_eq!(std::mem::offset_of!(CameraUniform, phase_materials), 288);
    assert_eq!(std::mem::offset_of!(CameraUniform, water_fog), 416);
    assert_eq!(std::mem::offset_of!(CameraUniform, water_optics), 432);
    assert_eq!(std::mem::offset_of!(CameraUniform, atmosphere), 448);
    assert_eq!(std::mem::offset_of!(CameraUniform, clouds), 464);
    assert_eq!(
        std::mem::offset_of!(CameraUniform, inverse_ray_projection),
        64
    );
}

#[test]
fn translated_cameras_keep_sky_and_air_rays_identical_and_match_surface_projection() {
    use super::FrameState;
    use crate::player::Player;
    use crate::world::ResidentWorld;
    use glam::{Mat4, Vec3, Vec4};
    let world = ResidentWorld::default();
    let mut player = Player::new(Vec3::new(203.5, 42.0, -44.5));
    player.yaw = -0.55;
    player.pitch = -0.18;
    let frame = FrameState {
        world: &world,
        player: &player,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: None,
        render_distance: 8,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        headlamp: false,
        in_lava: false,
    };
    let uniform = CameraUniform::from_frame(&frame, 1.5, [0; 3], false, true, |_, _, _| None);
    let inverse = Mat4::from_cols_array_2d(&uniform.inverse_ray_projection);
    for step in -16..=16 {
        let mut translated = player.clone();
        translated.position += Vec3::new(0.067, 0.003, -0.054) * step as f32;
        let shifted = FrameState {
            player: &translated,
            ..frame
        };
        let next = CameraUniform::from_frame(&shifted, 1.5, [0; 3], false, true, |_, _, _| None);
        assert_eq!(uniform.inverse_ray_projection, next.inverse_ray_projection);
        let projection = Mat4::from_cols_array_2d(&next.view_projection);
        for ndc in [[0.0, 0.0], [-0.75, 0.25], [0.4, -0.6]] {
            let direction = (inverse * Vec4::new(ndc[0], ndc[1], 1.0, 1.0))
                .truncate()
                .normalize();
            let projected = projection.project_point3(translated.eye_position() + direction * 20.0);
            assert!((projected.x - ndc[0]).abs() < 0.00001);
            assert!((projected.y - ndc[1]).abs() < 0.00001);
        }
    }
}

#[test]
fn air_depth_guide_is_unblended_even_when_water_color_is_transparent() {
    use super::pipelines::scene_targets;
    use super::postprocess::HDR_FORMAT;
    let water = scene_targets(HDR_FORMAT, Some(wgpu::BlendState::ALPHA_BLENDING), true);
    let color = water[0].as_ref().unwrap();
    let guide = water[1].as_ref().unwrap();
    assert_eq!(color.blend, Some(wgpu::BlendState::ALPHA_BLENDING));
    assert_eq!(guide.blend, None);
    assert_eq!(guide.format, HDR_FORMAT);
    assert_eq!(guide.write_mask, wgpu::ColorWrites::ALL);
    let overlay = scene_targets(HDR_FORMAT, Some(wgpu::BlendState::ALPHA_BLENDING), false);
    assert!(overlay[1].as_ref().unwrap().write_mask.is_empty());
}

#[test]
fn camera_air_parameters_preserve_render_distance_caves_and_other_media() {
    use super::FrameState;
    use crate::player::Player;
    use crate::world::ResidentWorld;
    let world = ResidentWorld::default();
    let player = Player::new(glam::Vec3::new(-17.5, -45.0, -0.5));
    let frame = FrameState {
        world: &world,
        player: &player,
        hud: &[],
        survival: None,
        elapsed: 1.0,
        time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        water_time: None,
        render_distance: 2,
        underwater: false,
        water: None,
        sky_exposure: 0.0,
        headlamp: false,
        in_lava: false,
    };
    for render_distance in [2, 8, 12] {
        let frame = FrameState {
            render_distance,
            ..frame
        };
        let uniform =
            CameraUniform::from_frame(&frame, 1.5, [-32, -64, -32], false, true, |_, _, _| None);
        assert_eq!(uniform.settings[2], render_distance as f32 * 16.0 - 12.0);
        assert_eq!(uniform.environment[0], 0.0);
        assert_eq!(uniform.atmosphere[3], 1.0);
    }
    for (underwater, in_lava, phasing, enabled) in [
        (true, false, false, true),
        (false, true, false, true),
        (false, false, true, true),
        (false, false, false, false),
    ] {
        let medium = FrameState {
            underwater,
            in_lava,
            ..frame
        };
        let uniform = CameraUniform::from_frame(
            &medium,
            1.5,
            [-32, -64, -32],
            phasing,
            enabled,
            |_, _, _| None,
        );
        assert_eq!(uniform.atmosphere[3], 0.0);
    }
}

#[test]
fn water_clock_can_be_isolated_without_passing_invalid_times_to_the_shader() {
    assert_eq!(water_animation_time(2.2, None), 2.2);
    assert_eq!(water_animation_time(2.2, Some(3.6)), 3.6);
    assert_eq!(water_animation_time(2.2, Some(f32::NAN)), 2.2);
    assert_eq!(water_animation_time(f32::INFINITY, None), 0.0);
    assert_eq!(water_animation_time(f32::NAN, Some(4.0)), 4.0);
}

#[test]
fn daylight_vertex_attribute_matches_the_extended_mesh_layout() {
    use super::{Vertex, WORLD_ATTRIBUTES};
    assert_eq!(size_of::<Vertex>(), 84);
    assert_eq!(std::mem::offset_of!(Vertex, light), 36);
    assert_eq!(WORLD_ATTRIBUTES[4].offset, 36);
    assert_eq!(WORLD_ATTRIBUTES[4].shader_location, 4);
    assert_eq!(std::mem::offset_of!(Vertex, water), 40);
    assert_eq!(std::mem::offset_of!(Vertex, water_extra), 56);
    assert_eq!(WORLD_ATTRIBUTES[5].offset, 40);
    assert_eq!(WORLD_ATTRIBUTES[6].offset, 56);
    assert_eq!(std::mem::offset_of!(Vertex, local_light), 72);
    assert_eq!(WORLD_ATTRIBUTES[7].offset, 72);
    assert_eq!(WORLD_ATTRIBUTES[7].shader_location, 7);
}

#[test]
fn cutaway_reverse_faces_are_bounded_near_the_camera() {
    let eye = glam::Vec3::new(-0.1, -45.0, -16.1);
    assert!(phase_chunk_near(crate::world::ChunkCoord::new(-1, -2), eye));
    assert!(phase_chunk_near(crate::world::ChunkCoord::new(0, -1), eye));
    assert!(!phase_chunk_near(crate::world::ChunkCoord::new(8, 8), eye));
}

#[test]
fn scene_statistics_measure_colored_texture_without_counting_alpha() {
    let mut pixels = [0; 8 * 8 * 4];
    for y in 2..6 {
        for x in 2..6 {
            let offset = (y * 8 + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(if x % 2 == 0 {
                &[120, 170, 210, 0]
            } else {
                &[35, 70, 90, 255]
            });
        }
    }
    let stats = scene_stats(&pixels, 8, 8);
    assert!(stats.mean_luminance > 0.35 && stats.mean_luminance < 0.55);
    assert_eq!(stats.visible_fraction, 1.0);
    assert!(stats.luminance_deviation > 0.1);
}

#[test]
fn temporal_readback_ignores_hud_margins_and_alpha_but_detects_moving_scene_pixels() {
    let before = vec![46; 8 * 8 * 4];
    let mut after = before.clone();
    after[..4].fill(255);
    for pixel in after.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    let unchanged = scene_difference(&before, &after, 8, 8);
    assert_eq!(unchanged.mean_absolute_error, 0.0);
    assert_eq!(unchanged.changed_fraction, 0.0);
    let center = (4 * 8 + 4) * 4;
    after[center..center + 3].fill(110);
    let changed = scene_difference(&before, &after, 8, 8);
    assert!((changed.mean_absolute_error - 64.0 / 255.0 / 16.0).abs() < 0.00001);
    assert_eq!(changed.changed_fraction, 1.0 / 16.0);
}

#[test]
fn full_frame_readback_counts_hand_and_hud_margins_without_counting_alpha() {
    use super::readback::full_frame_difference;
    let before = vec![46; 8 * 8 * 4];
    let mut after = before.clone();
    for pixel in after.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    assert_eq!(
        full_frame_difference(&before, &after, 8, 8).changed_fraction,
        0.0
    );
    for (x, y) in [(0, 0), (7, 7), (6, 5)] {
        let offset = (y * 8 + x) * 4;
        after[offset..offset + 3].fill(110);
    }
    assert_eq!(
        scene_difference(&before, &after, 8, 8).changed_fraction,
        0.0
    );
    let changed = full_frame_difference(&before, &after, 8, 8);
    assert!((changed.mean_absolute_error - 64.0 / 255.0 * 3.0 / 64.0).abs() < 0.00001);
    assert_eq!(changed.changed_fraction, 3.0 / 64.0);
}

#[test]
fn full_frame_readback_keeps_the_two_level_change_threshold_and_handles_empty_frames() {
    use super::readback::full_frame_difference;
    let before = [10, 20, 30, 0, 10, 20, 30, 0];
    let after = [12, 20, 30, 255, 13, 20, 30, 255];
    let changed = full_frame_difference(&before, &after, 2, 1);
    assert_eq!(changed.changed_fraction, 0.5);
    assert!((changed.mean_absolute_error - 5.0 / (6.0 * 255.0)).abs() < 0.00001);
    let empty = full_frame_difference(&[], &[], 0, 0);
    assert_eq!(empty.mean_absolute_error, 0.0);
    assert_eq!(empty.changed_fraction, 0.0);
}

#[test]
fn aquatic_color_statistics_exclude_overlay_edges_and_do_not_collapse_rgb_to_gray() {
    let mut pixels = vec![255; 8 * 8 * 4];
    for y in 2..6 {
        for x in 2..6 {
            let offset = (y * 8 + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(&[30, 90, 150, 0]);
        }
    }
    let color = scene_color(&pixels, 8, 8);
    for (actual, expected) in color.into_iter().zip([30.0, 90.0, 150.0]) {
        assert!((actual - expected / 255.0).abs() < 0.00001);
    }
}

#[test]
fn realistic_uniform_gray_frames_cannot_invent_cutaway_contrast() {
    for (width, height, gray) in [(1200, 760, 46), (2400, 1520, 128)] {
        let pixels = vec![gray; width as usize * height as usize * 4];
        let stats = scene_stats(&pixels, width, height);
        assert!(
            stats.luminance_deviation < 0.00001,
            "{width}x{height}: {stats:?}"
        );
        assert!((stats.mean_luminance - f32::from(gray) / 255.0).abs() < 0.00001);
    }
}

#[test]
fn phase_materials_share_the_regular_mesh_atlas_contract() {
    let table = phase_material_table();
    for block in crate::world::Block::ALL {
        let material = table[block as usize / 4][block as usize % 4];
        assert_eq!(material, crate::mesh::material_for_face(block, 0));
        assert!(material < crate::atlas::TILE_COUNT);
    }
    assert_eq!(
        table[crate::world::Block::Leaves as usize / 4][crate::world::Block::Leaves as usize % 4],
        8
    );
    assert_eq!(
        table[crate::world::Block::Calcite as usize / 4][crate::world::Block::Calcite as usize % 4],
        21
    );
    for block in crate::world::Block::ALL {
        for vertical in [-1, 1] {
            if !matches!(
                block,
                crate::world::Block::Grass | crate::world::Block::Wood
            ) {
                assert_eq!(
                    crate::mesh::material_for_face(block, vertical),
                    table[block as usize / 4][block as usize % 4]
                );
            }
        }
    }
}

#[test]
fn readback_rows_are_aligned_without_extra_padding_when_unnecessary() {
    assert_eq!(padded_row_bytes(1), 256);
    assert_eq!(padded_row_bytes(64), 256);
    assert_eq!(padded_row_bytes(65), 512);
    assert_eq!(padded_row_bytes(1280), 5120);
}

#[test]
fn readback_removes_padding_and_swizzles_bgra() {
    let data = [30, 20, 10, 255, 0, 0, 0, 0, 60, 50, 40, 128, 0, 0, 0, 0];
    assert_eq!(
        unpack_pixels(&data, 4, 8, true),
        [10, 20, 30, 255, 40, 50, 60, 128]
    );
    assert_eq!(
        unpack_pixels(&data, 4, 8, false),
        [30, 20, 10, 255, 60, 50, 40, 128]
    );
}

#[test]
fn gpu_callback_errors_are_reported_instead_of_counted_as_success() {
    let (send, errors) = mpsc::channel();
    assert!(check_gpu_errors(&errors).is_ok());
    send.send("GPU validation failed".into()).unwrap();
    let error = check_gpu_errors(&errors).unwrap_err();
    assert!(error.to_string().contains("GPU validation failed"));
    assert!(check_gpu_errors(&errors).is_ok());
}

#[test]
fn scene_luminance_ignores_hud_margins_and_distinguishes_dark_frames() {
    let mut pixels = vec![255; 8 * 8 * 4];
    for y in 2..6 {
        for x in 2..6 {
            let offset = (y * 8 + x) * 4;
            pixels[offset..offset + 3].fill(0);
        }
    }
    let center = (4 * 8 + 4) * 4;
    pixels[center..center + 3].fill(255);
    let dark = scene_stats(&pixels, 8, 8);
    assert!((dark.mean_luminance - 1.0 / 16.0).abs() < 0.0001);
    assert_eq!(dark.visible_fraction, 1.0 / 16.0);
    assert!(dark.luminance_deviation > 0.2);
    assert!((scene_stats(&[255; 8 * 8 * 4], 8, 8).mean_luminance - 1.0).abs() < 0.0001);
    assert!(scene_stats(&[255; 8 * 8 * 4], 8, 8).luminance_deviation < 0.001);
}

#[test]
fn gpu_waits_cannot_extend_the_absolute_diagnostic_deadline() {
    let now = Instant::now();
    assert_eq!(
        gpu_wait_timeout(None, now).unwrap(),
        Duration::from_secs(10)
    );
    assert_eq!(
        gpu_wait_timeout(Some(now + Duration::from_millis(100)), now).unwrap(),
        Duration::from_millis(100)
    );
    assert_eq!(
        gpu_wait_timeout(Some(now + Duration::from_secs(30)), now).unwrap(),
        Duration::from_secs(10)
    );
    assert!(gpu_wait_timeout(Some(now), now).is_err());
}
