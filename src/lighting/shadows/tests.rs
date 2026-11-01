use super::*;

fn sun_direction() -> Vec3 {
    crate::lighting::CelestialLight::default().sun_direction
}

#[test]
fn cascades_cover_receivers_and_the_roofs_above_negative_y_caves() {
    for (eye, distance) in [
        (Vec3::new(-16.1, -53.0, -0.1), 2),
        (Vec3::new(203.5, 43.6, -44.5), 8),
    ] {
        let coverage = shadow_coverage(eye, distance, sun_direction());
        assert!(coverage.radii[0] < coverage.radii[1]);
        for (matrix, radius) in coverage.matrices.into_iter().zip(coverage.radii) {
            for point in [
                eye,
                eye + sun_direction() * 150.0,
                eye + Vec3::X * radius * 0.7,
            ] {
                let clip = matrix.project_point3(point);
                assert!(clip.is_finite());
                assert!(clip.x.abs() < 1.0 && clip.y.abs() < 1.0);
                assert!((0.0..1.0).contains(&clip.z));
            }
            let near = matrix.project_point3(eye + sun_direction() * 100.0);
            let far = matrix.project_point3(eye - sun_direction() * 100.0);
            assert!(near.z < far.z);
        }
    }
}

#[test]
fn small_light_plane_motion_does_not_slide_the_shadow_texels() {
    let sun = sun_direction();
    let right = sun.cross(Vec3::Y).normalize();
    let base = shadow_coverage(Vec3::ZERO, 8, sun);
    let moved = shadow_coverage(right * 0.001, 8, sun);
    for (a, b) in base.matrices.into_iter().zip(moved.matrices) {
        let a = a.project_point3(Vec3::new(-10.0, 25.0, 7.0));
        let b = b.project_point3(Vec3::new(-10.0, 25.0, 7.0));
        assert!((a.x - b.x).abs() < 0.000001);
        assert!((a.y - b.y).abs() < 0.000001);
    }
}

#[test]
fn walking_along_sun_depth_keeps_the_light_basis_and_small_depth_steps_identical() {
    let sun = sun_direction();
    let eye = Vec3::new(203.5, 43.6, -44.5);
    let base = shadow_coverage(eye, 8, sun);
    let reference = Vec3::new(190.25, 35.0, -35.75);
    for step in 1..=32 {
        let moved = shadow_coverage(eye + sun * (step as f32 / 16.0), 8, sun);
        for (first, next) in base.matrices.into_iter().zip(moved.matrices) {
            let a = first.project_point3(reference);
            let b = next.project_point3(reference);
            assert!((a - b).abs().max_element() < 0.000002, "{a:?} != {b:?}");
            for axis in 0..3 {
                assert_eq!(first.col(axis), next.col(axis));
            }
        }
    }
}

#[test]
fn shadow_xy_snaps_move_by_integral_texels_without_changing_the_fixed_rotation() {
    let eye = Vec3::new(-38.5, 26.62, 12.5);
    let base = shadow_coverage(eye, 8, sun_direction());
    let sample = Vec3::new(-30.2, 21.0, -10.7);
    for step in -60..=60 {
        let moved = shadow_coverage(
            eye + Vec3::new(0.07, 0.0, -0.05) * step as f32,
            8,
            sun_direction(),
        );
        for (first, next) in base.matrices.into_iter().zip(moved.matrices) {
            for axis in 0..3 {
                assert_eq!(first.col(axis), next.col(axis));
            }
            let delta = (next.project_point3(sample) - first.project_point3(sample))
                * SHADOW_RESOLUTION as f32
                * 0.5;
            assert!((delta.x - delta.x.round()).abs() < 0.0004);
            assert!((delta.y - delta.y.round()).abs() < 0.0004);
        }
    }
}

#[test]
fn depth_anchor_recenter_moves_receivers_and_casters_by_the_same_depth_offset() {
    let sun = sun_direction();
    let rotation = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, -sun, Vec3::Y);
    let eye = rotation
        .inverse()
        .transform_point3(Vec3::new(10.0, 30.0, 63.99));
    let first = shadow_coverage(eye, 12, sun);
    let next = shadow_coverage(eye + sun * 0.03, 12, sun);
    for (first, next) in first.matrices.into_iter().zip(next.matrices) {
        let receiver = eye;
        let caster = eye + sun * 150.0;
        let first_receiver = first.project_point3(receiver);
        let next_receiver = next.project_point3(receiver);
        let first_caster = first.project_point3(caster);
        let next_caster = next.project_point3(caster);
        assert!((first_receiver.z - next_receiver.z).abs() > 0.1);
        assert!(
            ((first_receiver.z - first_caster.z) - (next_receiver.z - next_caster.z)).abs()
                < 0.000001
        );
        assert!((0.0..1.0).contains(&next_receiver.z));
        assert!((0.0..1.0).contains(&next_caster.z));
        assert!(
            (first_receiver.truncate() - next_receiver.truncate())
                .abs()
                .max_element()
                < 0.000002
        );
    }
}

#[test]
fn invalid_positions_and_residency_limits_have_finite_bounded_fallbacks() {
    let invalid = shadow_coverage(Vec3::NAN, -100, sun_direction());
    let minimum = shadow_coverage(Vec3::ZERO, 2, sun_direction());
    assert_eq!(invalid.radii, minimum.radii);
    assert_eq!(invalid.matrices, minimum.matrices);
    assert_eq!(
        shadow_coverage(Vec3::ZERO, 100, sun_direction()).radii,
        shadow_coverage(Vec3::ZERO, 12, sun_direction()).radii
    );
}

#[test]
fn moving_sun_keeps_cascades_finite_at_horizons_noon_and_night() {
    let eye = Vec3::new(-16.5, -30.0, 17.5);
    for step in 0..=288 {
        let sun = crate::lighting::CelestialLight::sample(f64::from(step) / 12.0).sun_direction;
        for distance in [2, 8, 12] {
            let coverage = shadow_coverage(eye, distance, sun);
            for matrix in coverage.matrices {
                assert!(matrix.is_finite());
                let receiver = matrix.project_point3(eye);
                let caster = matrix.project_point3(eye + sun * 150.0);
                assert!((0.0..1.0).contains(&receiver.z));
                assert!((0.0..1.0).contains(&caster.z));
                assert!(caster.z < receiver.z);
            }
        }
    }
    for direction in [
        Vec3::Y,
        -Vec3::Y,
        Vec3::ZERO,
        Vec3::NAN,
        Vec3::INFINITY,
        Vec3::splat(f32::MAX),
    ] {
        assert!(
            shadow_coverage(eye, 8, direction)
                .matrices
                .iter()
                .all(|matrix| matrix.is_finite())
        );
    }
    assert_ne!(
        shadow_coverage(
            eye,
            8,
            crate::lighting::CelestialLight::sample(7.0).sun_direction
        )
        .matrices,
        shadow_coverage(
            eye,
            8,
            crate::lighting::CelestialLight::sample(17.0).sun_direction
        )
        .matrices,
    );
}
