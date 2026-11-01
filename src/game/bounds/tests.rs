use super::*;

#[test]
fn touching_faces_are_excluded_at_negative_and_large_coordinates() {
    for x in [-1024.5, -0.5, 0.5, 512.5] {
        let bounds = Bounds::cube(Vec3::new(x, 4.5, x), 0.5);
        let (min, max) = bounds.cells().unwrap();
        assert_eq!(min, [x.floor() as i32, 4, x.floor() as i32]);
        assert_eq!(min, max);
        assert!(!bounds.overlaps_block([min[0] + 1, 4, min[2]]));
        assert!(bounds.overlaps_block(min));
    }
}

#[test]
fn player_bounds_and_distance_use_unit_cubes_not_centers() {
    let position = Vec3::new(-0.5, 1.0, -0.5);
    let bounds = Bounds::player(position);
    assert!(bounds.overlaps_block([-1, 2, -1]));
    assert!(!bounds.overlaps_block([-1, 0, -1]));
    assert!(!bounds.overlaps_block([0, 1, -1]));
    assert_eq!(block_distance_squared(position, [-1, 1, -1]), 0.0);
    assert_eq!(block_distance_squared(position, [-1, 3, -1]), 4.0);
}

#[test]
fn nonfinite_or_unrepresentable_bounds_are_rejected() {
    for x in [f32::NAN, f32::INFINITY, f32::MAX] {
        assert!(
            Bounds::cube(Vec3::new(x, 0.0, 0.0), 0.125)
                .cells()
                .is_none()
        );
    }
    assert_eq!(checked_floor(-0.01), Some(-1));
    assert_eq!(checked_floor(f64::from(i32::MAX) + 1.0), None);
    assert_eq!(checked_floor(f64::from(i32::MIN) - 1.0), None);
}
