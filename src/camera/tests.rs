use super::{chunk_visible, view_projection};
use crate::world::ChunkCoord;
use glam::{Vec3, Vec4};

#[test]
fn forward_points_project_to_the_center_with_zero_to_one_depth() {
    let eye = Vec3::new(0.0, 30.0, 0.0);
    let matrix = view_projection(eye, Vec3::NEG_Z, 16.0 / 9.0, 200.0);
    for distance in [0.05, 1.0, 100.0, 200.0] {
        let clip = matrix * (eye + Vec3::NEG_Z * distance).extend(1.0);
        let ndc = clip / clip.w;
        assert!(ndc.x.abs() < 0.0001);
        assert!(ndc.y.abs() < 0.0001);
        assert!((-0.0001..=1.0001).contains(&ndc.z));
    }
}

#[test]
fn inverse_projection_reconstructs_world_points() {
    let matrix = view_projection(
        Vec3::new(-10.0, 42.0, 5.0),
        Vec3::new(0.4, -0.2, -1.0).normalize(),
        1.5,
        160.0,
    );
    let point = Vec4::new(7.0, 23.0, -30.0, 1.0);
    let restored = matrix.inverse() * (matrix * point);
    let error = (restored / restored.w - point).abs().max_element();
    assert!(
        error < 0.01,
        "reconstruction error: {error}, result: {restored:?}"
    );
}

#[test]
fn rejects_chunks_behind_beyond_and_beside_the_camera() {
    let matrix = view_projection(Vec3::new(8.0, 30.0, 8.0), Vec3::NEG_Z, 1.0, 100.0);
    assert!(chunk_visible(matrix, ChunkCoord::new(0, -2)));
    assert!(chunk_visible(matrix, ChunkCoord::new(0, 0)));
    assert!(!chunk_visible(matrix, ChunkCoord::new(0, 2)));
    assert!(!chunk_visible(matrix, ChunkCoord::new(0, -10)));
    assert!(!chunk_visible(matrix, ChunkCoord::new(10, -1)));
}

#[test]
fn yaw_and_aspect_change_visible_chunks() {
    let eye = Vec3::new(8.0, 30.0, 8.0);
    let east = view_projection(eye, Vec3::X, 1.0, 150.0);
    assert!(chunk_visible(east, ChunkCoord::new(3, 0)));
    assert!(!chunk_visible(east, ChunkCoord::new(-3, 0)));
    let narrow = view_projection(eye, Vec3::NEG_Z, 0.4, 150.0);
    let wide = view_projection(eye, Vec3::NEG_Z, 2.0, 150.0);
    let side = ChunkCoord::new(3, -3);
    assert!(!chunk_visible(narrow, side));
    assert!(chunk_visible(wide, side));
}

#[test]
fn extreme_chunk_bounds_are_rejected_without_integer_overflow() {
    let matrix = view_projection(Vec3::new(8.0, 30.0, 8.0), Vec3::NEG_Z, 1.0, 100.0);
    for coord in [
        ChunkCoord::new(i32::MAX, i32::MAX),
        ChunkCoord::new(i32::MIN, i32::MIN),
    ] {
        assert!(!chunk_visible(matrix, coord));
    }
}

#[test]
fn underground_camera_retains_negative_y_chunk_geometry() {
    let eye = Vec3::new(8.0, -45.0, 8.0);
    let direction = Vec3::new(0.0, -0.45, -1.0).normalize();
    let matrix = view_projection(eye, direction, 1.5, 100.0);
    assert!(chunk_visible(matrix, ChunkCoord::new(0, -2)));
    assert!(!chunk_visible(matrix, ChunkCoord::new(0, 3)));
}
