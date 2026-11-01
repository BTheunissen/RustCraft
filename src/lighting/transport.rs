//! Probe layout, bounded update schedule, and the conservative voxel-ray reference.

#[cfg(test)]
use super::volume::PROBE_GRID;
use super::volume::{PROBE_COUNT, PROBES_PER_FRAME};
use crate::world::Block;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct IrradianceProbe {
    pub(crate) position: [f32; 4],
    pub(crate) lobes: [[f32; 4]; 6],
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ProbeSchedule {
    cursor: u32,
}

impl ProbeSchedule {
    pub(crate) fn next(&mut self, invalidated: bool, deterministic: bool) -> [u32; 2] {
        self.next_with_budget(invalidated, deterministic, PROBES_PER_FRAME)
    }

    pub(crate) fn next_with_budget(
        &mut self,
        invalidated: bool,
        deterministic: bool,
        budget: u32,
    ) -> [u32; 2] {
        if invalidated {
            self.cursor = 0;
        }
        if invalidated || deterministic {
            return [0, PROBE_COUNT];
        }
        let start = self.cursor;
        let budget = budget
            .clamp(PROBES_PER_FRAME, PROBE_COUNT)
            .div_ceil(PROBES_PER_FRAME)
            * PROBES_PER_FRAME;
        self.cursor = (self.cursor + budget) % PROBE_COUNT;
        [start, budget]
    }
}

/// World-grid addresses remain identical when the camera's local volume scrolls.
#[cfg(test)]
pub(crate) fn probe_index(cell: [i32; 3]) -> u32 {
    let [x, y, z] = cell.map(|value| value.rem_euclid(PROBE_GRID as i32) as u32);
    (z * PROBE_GRID + y) * PROBE_GRID + x
}

#[cfg(test)]
fn update_cell(index: u32) -> [u32; 3] {
    let coarse = PROBE_GRID / 2;
    let batch = coarse.pow(3);
    let phase = index / batch;
    let block = index % batch;
    [
        (block % coarse) * 2 + (phase & 1),
        ((block / coarse) % coarse) * 2 + ((phase >> 1) & 1),
        (block / (coarse * coarse)) * 2 + ((phase >> 2) & 1),
    ]
}

/// CPU reference for probe rays. Unknown data stops transport rather than becoming sky.
/// All side cells touched at a tied crossing are checked to prevent diagonal light leaks.
pub(crate) fn trace_voxel_ray(
    origin: Vec3,
    direction: Vec3,
    limit: f32,
    mut sample: impl FnMut([i32; 3]) -> Option<Block>,
) -> Option<Block> {
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() < 0.001
        || !limit.is_finite()
        || limit <= 0.0
    {
        return None;
    }
    let ray = direction.normalize();
    let mut cell = origin.floor().as_ivec3();
    let step = ray.signum().as_ivec3();
    let delta = ray.abs().map(|value| {
        if value == 0.0 {
            f32::INFINITY
        } else {
            value.recip()
        }
    });
    let boundary = cell.as_vec3() + step.max(glam::IVec3::ZERO).as_vec3();
    let mut next = (boundary - origin) / ray;
    for axis in 0..3 {
        if ray[axis] == 0.0 {
            next[axis] = f32::INFINITY;
        }
    }
    let block = sample(cell.to_array())?;
    if block.is_opaque() {
        return Some(block);
    }
    for _ in 0..112 {
        let distance = next.min_element();
        if distance > limit.min(40.0) {
            return None;
        }
        let tied = std::array::from_fn::<_, 3, _>(|axis| next[axis] <= distance + 0.00001);
        for mask in 1..8 {
            let mut entered = cell;
            let mut valid = true;
            for axis in 0..3 {
                if mask & (1 << axis) != 0 {
                    if !tied[axis] {
                        valid = false;
                        break;
                    }
                    entered[axis] = entered[axis].checked_add(step[axis])?;
                }
            }
            if valid {
                let block = sample(entered.to_array())?;
                if block.is_opaque() {
                    return Some(block);
                }
            }
        }
        for axis in 0..3 {
            if tied[axis] {
                cell[axis] = cell[axis].checked_add(step[axis])?;
                next[axis] += delta[axis];
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
