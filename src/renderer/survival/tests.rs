use super::cracks::SEGMENTS;
use super::geometry::{
    BlockInstance, CubeVertex, SurvivalUniform, animation_time, block_instance, cube_vertices,
    hand_instances, mining_parameters, pickup_instance, target_is_current, target_model,
};
use super::pipelines::{CUBE_ATTRIBUTES, INSTANCE_ATTRIBUTES, PassKind};
use crate::camera::view_projection;
use crate::game::BlockTarget;
use crate::mesh::material_for_face;
use crate::world::Block;
use glam::{Mat4, Vec2, Vec3};

#[test]
fn shared_cube_has_outward_ccw_faces_and_the_correct_material_axis() {
    let vertices = cube_vertices();
    for triangle in vertices.as_chunks::<3>().0 {
        let [a, b, c] = std::array::from_fn(|index| Vec3::from_array(triangle[index].position));
        let normal = Vec3::from_array(triangle[0].normal);
        assert!((b - a).cross(c - a).dot(normal) > 0.99);
        assert!(a.dot(normal) > 0.49);
        let expected_face = if normal.y > 0.5 {
            1
        } else if normal.y < -0.5 {
            2
        } else {
            0
        };
        for vertex in triangle {
            assert_eq!(vertex.face, expected_face);
            assert_eq!(vertex.normal, normal.to_array());
            assert!(vertex.position.into_iter().all(|axis| axis.abs() == 0.5));
            assert!(
                vertex
                    .uv
                    .into_iter()
                    .all(|axis| (0.0..=1.0).contains(&axis))
            );
        }
    }
}

#[test]
fn survival_vertex_instance_and_uniform_layouts_match_wgsl() {
    assert_eq!(size_of::<CubeVertex>(), 36);
    assert_eq!(size_of::<BlockInstance>(), 84);
    assert_eq!(size_of::<SurvivalUniform>(), 144);
    assert_eq!(std::mem::offset_of!(SurvivalUniform, target_model), 64);
    assert_eq!(std::mem::offset_of!(SurvivalUniform, mining), 128);
    for (attribute, offset) in CUBE_ATTRIBUTES.iter().zip([0, 12, 24, 32]) {
        assert_eq!(attribute.offset, offset);
    }
    for (attribute, offset) in INSTANCE_ATTRIBUTES.iter().zip([0, 16, 32, 48, 64, 80]) {
        assert_eq!(attribute.offset, offset);
    }
    assert_eq!(std::mem::offset_of!(BlockInstance, materials), 64);
    assert_eq!(std::mem::offset_of!(BlockInstance, light), 80);
    assert_eq!(INSTANCE_ATTRIBUTES[4].format, wgpu::VertexFormat::Uint32x4);
    assert_eq!(INSTANCE_ATTRIBUTES[5].shader_location, 9);
}

#[test]
fn all_drop_and_held_tiles_share_the_chunk_face_mapping() {
    for block in Block::ALL {
        let instance = block_instance(block, Mat4::IDENTITY, 0.5);
        assert_eq!(instance.materials[0], material_for_face(block, 0));
        assert_eq!(instance.materials[1], material_for_face(block, 1));
        assert_eq!(instance.materials[2], material_for_face(block, -1));
        assert!(
            instance.materials[..3]
                .iter()
                .all(|tile| *tile < crate::atlas::TILE_COUNT)
        );
    }
    assert_eq!(
        block_instance(Block::Grass, Mat4::IDENTITY, 1.0).materials,
        [1, 0, 2, 0]
    );
    assert_eq!(
        block_instance(Block::Wood, Mat4::IDENTITY, 1.0).materials,
        [6, 7, 7, 0]
    );
    assert_eq!(
        block_instance(Block::Stone, Mat4::IDENTITY, -1.0).light,
        0.0
    );
    assert_eq!(block_instance(Block::Stone, Mat4::IDENTITY, 2.0).light, 1.0);
}

#[test]
fn pickup_animation_stays_small_upright_and_above_its_cpu_position() {
    let position = Vec3::new(-16.1, -44.0, -0.2);
    let mut previous_rotation = None;
    for time in [0.0, 0.5, 2.0, 15.0, 40.0] {
        let instance = pickup_instance(position, Block::Grass, 3.0, time, 0.65);
        let model = Mat4::from_cols_array_2d(&instance.model);
        let (scale, rotation, center) = model.to_scale_rotation_translation();
        assert!((scale - Vec3::splat(0.25)).length() < 0.00001);
        assert!((rotation * Vec3::Y - Vec3::Y).length() < 0.00001);
        assert!((center.x - position.x).abs() < 0.00001);
        assert!((center.z - position.z).abs() < 0.00001);
        assert!((0.0099..=0.0601).contains(&(center.y - position.y)));
        assert!(model.determinant() > 0.0);
        assert_eq!(instance.materials, [1, 0, 2, 0]);
        if let Some(previous) = previous_rotation {
            assert_ne!(rotation, previous);
        }
        previous_rotation = Some(rotation);
    }
    let invalid_time = pickup_instance(position, Block::Stone, f32::NAN, f32::INFINITY, 0.0);
    assert!(invalid_time.model.into_iter().flatten().all(f32::is_finite));
    assert_eq!(animation_time(f32::NAN), 0.0);
}

#[test]
fn bare_and_held_hand_geometry_remains_in_the_lower_right_with_webgpu_depth() {
    for aspect in [1.0, 1.6, 2.4] {
        let projection = view_projection(Vec3::ZERO, Vec3::NEG_Z, aspect, 8.0);
        let (bare, bare_count) = hand_instances(None, aspect, 0.0, 0.0);
        assert_eq!(bare_count, 2);
        assert_eq!(bare[0].materials[3], 2);
        assert_eq!(bare[1].materials[3], 1);
        let (_, invalid_count) = hand_instances(Some(Block::Water.into()), aspect, 0.0, 0.0);
        assert_eq!(invalid_count, 2);
        for held in [None, Some(Block::Grass), Some(Block::Wood)] {
            let (instances, count) = hand_instances(held.map(Into::into), aspect, 0.0, 0.0);
            assert_eq!(count, if held.is_some() { 3 } else { 2 });
            for instance in &instances[..count as usize] {
                let model = Mat4::from_cols_array_2d(&instance.model);
                assert!(model.determinant() > 0.0);
                let clip = projection * model.w_axis;
                let ndc = clip.truncate() / clip.w;
                assert!((0.2..0.95).contains(&ndc.x), "{aspect}: {ndc:?}");
                assert!(ndc.y < -0.2, "{aspect}: {ndc:?}");
                assert!((0.0..1.0).contains(&ndc.z));
                assert!(cube_vertices().iter().any(|vertex| {
                    let clip = projection * model * Vec3::from_array(vertex.position).extend(1.0);
                    let ndc = clip.truncate() / clip.w;
                    ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0 && (0.0..1.0).contains(&ndc.z)
                }));
            }
            if let Some(block) = held {
                assert_eq!(instances[2].materials[3], 0);
                assert_eq!(instances[2].materials[1], material_for_face(block, 1));
            }
        }
    }
}

#[test]
fn held_block_stays_above_the_hotbar_and_away_from_the_target_through_mining_sway() {
    for aspect in [1.0, 4.0 / 3.0, 1.6, 16.0 / 9.0, 2.4] {
        let projection = view_projection(Vec3::ZERO, Vec3::NEG_Z, aspect, 8.0);
        for progress in [0.0, 0.75] {
            for tick in 0..=360 {
                let elapsed = tick as f32 / 360.0 * std::f32::consts::TAU / 1.8;
                let (instances, count) =
                    hand_instances(Some(Block::Grass.into()), aspect, elapsed, progress);
                assert_eq!(count, 3);
                let model = Mat4::from_cols_array_2d(&instances[2].model);
                let clip = projection * model.w_axis;
                let center = clip.truncate() / clip.w;
                assert!(center.x > 0.4 && center.y < -0.1, "{aspect}: {center:?}");
                for vertex in cube_vertices() {
                    let clip = projection * model * Vec3::from_array(vertex.position).extend(1.0);
                    let ndc = clip.truncate() / clip.w;
                    assert!(
                        ndc.y > -0.52,
                        "hotbar overlap at {elapsed}, {progress}: {ndc:?}"
                    );
                    assert!(
                        (0.15..0.95).contains(&ndc.x),
                        "target/edge overlap at {aspect}: {ndc:?}"
                    );
                    assert!(ndc.y < 0.15 && (0.0..1.0).contains(&ndc.z));
                }
            }
        }
    }
}

#[test]
fn hand_animation_changes_pose_without_changing_materials_or_winding() {
    let (idle, count) = hand_instances(Some(Block::Wood.into()), 1.6, 0.4, 0.0);
    let (mining, mining_count) = hand_instances(Some(Block::Wood.into()), 1.6, 0.4, 0.4);
    let (later, _) = hand_instances(Some(Block::Wood.into()), 1.6, 0.8, 0.4);
    let (invalid, _) = hand_instances(None, 1.6, f32::NAN, f32::INFINITY);
    assert_ne!(idle[2].model, mining[2].model);
    assert_ne!(mining[2].model, later[2].model);
    assert_eq!(count, mining_count);
    for (idle, mining) in idle[..count as usize]
        .iter()
        .zip(&mining[..mining_count as usize])
    {
        assert_eq!(idle.materials, mining.materials);
        assert!(Mat4::from_cols_array_2d(&mining.model).determinant() > 0.0);
    }
    assert!(
        invalid[..2]
            .iter()
            .all(|instance| { instance.model.into_iter().flatten().all(f32::is_finite) })
    );
}

fn target() -> BlockTarget {
    BlockTarget {
        position: [-17, -42, 15],
        normal: [0, 1, 0],
        block: Block::Stone,
        distance: 2.0,
    }
}

#[test]
fn target_overlay_rejects_missing_stale_liquid_and_invalid_hits() {
    assert!(target_is_current(target(), |x, y, z| {
        assert_eq!([x, y, z], [-17, -42, 15]);
        Some(Block::Stone)
    }));
    for resident in [None, Some(Block::Air), Some(Block::Dirt)] {
        assert!(!target_is_current(target(), |_, _, _| resident));
    }
    for block in [Block::Air, Block::Water, Block::Lava] {
        assert!(!target_is_current(
            BlockTarget { block, ..target() },
            |_, _, _| Some(block)
        ));
    }
    for distance in [
        -0.1,
        f32::NAN,
        f32::INFINITY,
        crate::game::INTERACTION_REACH + 0.01,
    ] {
        assert!(!target_is_current(
            BlockTarget {
                distance,
                ..target()
            },
            |_, _, _| Some(Block::Stone)
        ));
    }
    assert!(target_is_current(
        BlockTarget {
            normal: [0; 3],
            distance: 0.0,
            ..target()
        },
        |_, _, _| Some(Block::Stone)
    ));
    for normal in [[0, 0, 0], [1, 1, 0], [2, 0, 0]] {
        assert!(!target_is_current(
            BlockTarget { normal, ..target() },
            |_, _, _| Some(Block::Stone)
        ));
    }
    for normal in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        assert!(target_is_current(
            BlockTarget { normal, ..target() },
            |_, _, _| Some(Block::Stone)
        ));
    }
}

#[test]
fn target_cube_tracks_negative_coordinates_without_changing_for_mining_progress() {
    let model = target_model(target().position);
    let (scale, rotation, center) = model.to_scale_rotation_translation();
    assert!((scale - Vec3::splat(1.003)).length() < 0.00001);
    assert_eq!(center, Vec3::new(-16.5, -41.5, 15.5));
    assert!((rotation * Vec3::Y - Vec3::Y).length() < 0.00001);
    for vertex in cube_vertices() {
        let point = model.transform_point3(Vec3::from_array(vertex.position));
        let delta = point - center;
        assert!(
            delta
                .to_array()
                .into_iter()
                .all(|axis| (axis.abs() - 0.5015).abs() < 0.00001)
        );
    }
}

#[test]
fn mining_uniform_progress_is_finite_clamped_and_progressively_stronger() {
    let idle = mining_parameters(0.0);
    for progress in [f32::NAN, f32::INFINITY, -0.5] {
        assert_eq!(mining_parameters(progress), idle);
    }
    assert_eq!(mining_parameters(2.0), mining_parameters(1.0));
    let mut previous = idle;
    for progress in [0.01, 0.2, 0.5, 0.8, 1.0] {
        let next = mining_parameters(progress);
        assert!(next[0] > previous[0] && next[1] > previous[1] && next[2] > previous[2]);
        assert!(next[2] < 1.0);
        previous = next;
    }
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn crack_alpha(uv: Vec2, progress: f32) -> f32 {
    let [progress, width, opacity, _] = mining_parameters(progress);
    SEGMENTS.iter().fold(0.0_f32, |mask, segment| {
        let start = Vec2::from_array(segment.start);
        let span = Vec2::from_array(segment.end) - start;
        let along = ((uv - start).dot(span) / span.length_squared()).clamp(0.0, 1.0);
        let distance = (uv - start - span * along).length();
        let active = smoothstep(segment.onset, segment.onset + 0.16, progress);
        mask.max((1.0 - smoothstep(width, width + 0.0008, distance)) * active * opacity)
    })
}

#[test]
fn mining_fractures_branch_monotonically_and_leave_most_of_the_texture_visible() {
    let samples: Vec<_> = (0..100)
        .flat_map(|y| {
            (0..100).map(move |x| Vec2::new((x as f32 + 0.5) / 100.0, (y as f32 + 0.5) / 100.0))
        })
        .collect();
    let mut previous = vec![0.0; samples.len()];
    let mut previous_coverage = 0;
    for progress in [0.0, 0.15, 0.4, 0.7, 1.0] {
        let next: Vec<_> = samples
            .iter()
            .map(|&uv| crack_alpha(uv, progress))
            .collect();
        assert!(
            next.iter()
                .zip(&previous)
                .all(|(next, previous)| next >= previous)
        );
        let coverage = next.iter().filter(|alpha| **alpha > 0.05).count();
        if progress > 0.0 {
            assert!(coverage > previous_coverage);
        } else {
            assert_eq!(coverage, 0);
        }
        assert!(
            coverage < samples.len() / 5,
            "opaque fracture coverage: {coverage}"
        );
        previous = next;
        previous_coverage = coverage;
    }
    assert!(crack_alpha(Vec2::new(0.49, 0.505), 1.0) > 0.9);
    assert_eq!(crack_alpha(Vec2::new(0.02, 0.98), 1.0), 0.0);
}

#[test]
fn survival_pipeline_depth_and_blending_keep_world_occlusion_and_hand_self_occlusion() {
    for kind in [PassKind::Pickup, PassKind::Hand] {
        let depth = kind.depth();
        assert_eq!(depth.depth_write_enabled, Some(true));
        assert_eq!(depth.depth_compare, Some(wgpu::CompareFunction::LessEqual));
        assert_eq!(kind.blend(), None);
    }
    assert_eq!(PassKind::Target.depth().depth_write_enabled, Some(false));
    assert_eq!(
        PassKind::Target.blend(),
        Some(wgpu::BlendState::ALPHA_BLENDING)
    );
}

#[test]
fn survival_shader_validates_and_retains_the_original_world_entry_points() {
    let source = super::shader_source();
    let module = wgpu::naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    for name in [
        "vs_world",
        "vs_water",
        "fs_world",
        "fs_water",
        "fs_phase_scan",
        "vs_pickup",
        "vs_target",
        "fs_target",
        "vs_hand",
        "fs_hand",
    ] {
        assert!(module.entry_points.iter().any(|entry| entry.name == name));
    }
}
