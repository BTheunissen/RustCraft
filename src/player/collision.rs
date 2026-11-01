//! Swept voxel collision and half-open body bounds.

use super::{BODY_MAX, BODY_MIN, COLLISION_EPSILON, GROUND_PROBE, Player, Vec3};

impl Player {
    pub(super) fn move_axis(
        &mut self,
        axis: usize,
        distance: f32,
        is_solid: &impl Fn(i32, i32, i32) -> bool,
    ) -> bool {
        if distance == 0.0 {
            return false;
        }

        let mut destination = self.position;
        destination[axis] += distance;
        // Sweep the whole path, rather than only testing the destination body.
        let min = (self.position + BODY_MIN).min(destination + BODY_MIN);
        let max = (self.position + BODY_MAX).max(destination + BODY_MAX);
        let (min_voxel, max_voxel) = voxel_bounds(min, max);
        let requested = destination[axis];
        let mut allowed = requested;
        for x in min_voxel[0]..=max_voxel[0] {
            for y in min_voxel[1]..=max_voxel[1] {
                for z in min_voxel[2]..=max_voxel[2] {
                    if is_solid(x, y, z) {
                        let voxel = [x, y, z][axis] as f32;
                        if distance > 0.0 {
                            allowed = allowed.min(voxel - BODY_MAX[axis]);
                        } else {
                            allowed = allowed.max(voxel + 1.0 - BODY_MIN[axis]);
                        }
                    }
                }
            }
        }
        self.position[axis] = allowed;
        allowed != requested
    }
}

pub(super) fn supported(position: Vec3, is_solid: &impl Fn(i32, i32, i32) -> bool) -> bool {
    body_overlaps(position - Vec3::Y * GROUND_PROBE, is_solid)
}

pub(super) fn body_overlaps(position: Vec3, is_solid: &impl Fn(i32, i32, i32) -> bool) -> bool {
    let (min, max) = voxel_bounds(position + BODY_MIN, position + BODY_MAX);
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                if is_solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn voxel_bounds(min: Vec3, max: Vec3) -> ([i32; 3], [i32; 3]) {
    // Half-open faces must not include a touching voxel. Floor before casting
    // also matters for negative coordinates, where truncation picks the wrong cell.
    // Apply epsilon after widening: f32 can round the adjustment away even at
    // ordinary world coordinates such as 512, incorrectly including a wall.
    let epsilon = f64::from(COLLISION_EPSILON);
    let min = [min.x, min.y, min.z].map(|value| (f64::from(value) + epsilon).floor() as i32);
    let max = [max.x, max.y, max.z].map(|value| (f64::from(value) - epsilon).floor() as i32);
    (min, max)
}
