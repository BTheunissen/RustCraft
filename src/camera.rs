//! Camera matrices and conservative chunk visibility, independent of the GPU.

use glam::{Mat4, Vec3, Vec4};

use crate::world::{CHUNK_SIZE, ChunkCoord, MIN_WORLD_Y, WORLD_DEPTH};

/// Builds a right-handed camera using WebGPU's zero-to-one depth range.
pub fn view_projection(eye: Vec3, direction: Vec3, aspect: f32, far: f32) -> Mat4 {
    let projection =
        glam::camera::rh::proj::directx::perspective(75.0_f32.to_radians(), aspect, 0.05, far);
    projection * glam::camera::rh::view::look_to_mat4(eye, direction, Vec3::Y)
}

/// Conservatively rejects chunk-column bounds outside any camera frustum plane.
pub fn chunk_visible(matrix: Mat4, coord: ChunkCoord) -> bool {
    let rows = matrix.transpose();
    let planes = [
        rows.w_axis + rows.x_axis,
        rows.w_axis - rows.x_axis,
        rows.w_axis + rows.y_axis,
        rows.w_axis - rows.y_axis,
        rows.z_axis,
        rows.w_axis - rows.z_axis,
    ];
    let min = Vec3::new(
        (i64::from(coord.x) * i64::from(CHUNK_SIZE)) as f32,
        MIN_WORLD_Y as f32,
        (i64::from(coord.z) * i64::from(CHUNK_SIZE)) as f32,
    );
    let max = min + Vec3::new(CHUNK_SIZE as f32, WORLD_DEPTH as f32, CHUNK_SIZE as f32);
    planes.into_iter().all(|plane| {
        let positive = Vec4::new(
            if plane.x >= 0.0 { max.x } else { min.x },
            if plane.y >= 0.0 { max.y } else { min.y },
            if plane.z >= 0.0 { max.z } else { min.z },
            1.0,
        );
        plane.dot(positive) >= 0.0
    })
}

#[cfg(test)]
mod tests;
