//! CPU layouts and transforms shared by the survival passes.

use crate::game::{BlockTarget, INTERACTION_REACH, Item};
use crate::mesh::material_for_face;
use crate::world::Block;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

pub(super) const MAX_PICKUPS: usize = 512;
pub(super) const HAND_INSTANCES: usize = 5;
pub(super) const CUBE_VERTICES: usize = 36;
const DROP_SIZE: f32 = 0.25;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct CubeVertex {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) uv: [f32; 2],
    pub(super) face: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct BlockInstance {
    pub(super) model: [[f32; 4]; 4],
    /// Side, top, bottom atlas tiles, then block/skin/sleeve appearance.
    pub(super) materials: [u32; 4],
    pub(super) light: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct SurvivalUniform {
    pub(super) hand_projection: [[f32; 4]; 4],
    pub(super) target_model: [[f32; 4]; 4],
    /// Progress, fracture width in face UVs, opacity, and reserved padding.
    pub(super) mining: [f32; 4],
}

struct Face {
    normal: [f32; 3],
    corners: [[f32; 3]; 4],
    material_face: u32,
}

const FACES: [Face; 6] = [
    Face {
        normal: [1.0, 0.0, 0.0],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
        material_face: 0,
    },
    Face {
        normal: [-1.0, 0.0, 0.0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
        material_face: 0,
    },
    Face {
        normal: [0.0, 1.0, 0.0],
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        material_face: 1,
    },
    Face {
        normal: [0.0, -1.0, 0.0],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        material_face: 2,
    },
    Face {
        normal: [0.0, 0.0, 1.0],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        material_face: 0,
    },
    Face {
        normal: [0.0, 0.0, -1.0],
        corners: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        material_face: 0,
    },
];

pub(super) fn cube_vertices() -> [CubeVertex; CUBE_VERTICES] {
    const INDICES: [usize; 6] = [0, 1, 2, 0, 2, 3];
    const UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    std::array::from_fn(|index| {
        let face = &FACES[index / 6];
        let corner = INDICES[index % 6];
        CubeVertex {
            position: face.corners[corner].map(|axis| axis - 0.5),
            normal: face.normal,
            uv: UVS[corner],
            face: face.material_face,
        }
    })
}

pub(super) fn block_instance(block: Block, model: Mat4, light: f32) -> BlockInstance {
    BlockInstance {
        model: model.to_cols_array_2d(),
        materials: [
            material_for_face(block, 0),
            material_for_face(block, 1),
            material_for_face(block, -1),
            0,
        ],
        light: light.clamp(0.0, 1.0),
    }
}

pub(super) fn animation_time(elapsed: f32) -> f32 {
    if elapsed.is_finite() { elapsed } else { 0.0 }
}

pub(super) fn pickup_instance(
    position: Vec3,
    block: Block,
    age: f32,
    elapsed: f32,
    light: f32,
) -> BlockInstance {
    let time = animation_time(elapsed);
    let phase = animation_time(age) * 0.37;
    let bob = 0.035 + (time * 2.3 + phase).sin() * 0.025;
    let rotation = Quat::from_rotation_y((time * 1.1 + phase).rem_euclid(std::f32::consts::TAU));
    block_instance(
        block,
        Mat4::from_scale_rotation_translation(
            Vec3::splat(DROP_SIZE),
            rotation,
            position + Vec3::Y * bob,
        ),
        light,
    )
}

pub(super) fn mining_parameters(progress: f32) -> [f32; 4] {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    [
        progress,
        0.004 + progress * 0.008,
        0.35 + progress * 0.6,
        0.0,
    ]
}

pub(super) fn target_is_current(
    target: BlockTarget,
    block_at: impl FnOnce(i32, i32, i32) -> Option<Block>,
) -> bool {
    let [x, y, z] = target.position;
    target.block.is_solid()
        && target.distance.is_finite()
        && (0.0..=INTERACTION_REACH).contains(&target.distance)
        && (matches!(
            target.normal,
            [1, 0, 0] | [-1, 0, 0] | [0, 1, 0] | [0, -1, 0] | [0, 0, 1] | [0, 0, -1]
        ) || (target.normal == [0; 3] && target.distance == 0.0))
        && block_at(x, y, z) == Some(target.block)
}

pub(super) fn target_model(position: [i32; 3]) -> Mat4 {
    // A small expansion avoids coplanar depth fighting without showing buried faces.
    Mat4::from_scale_rotation_translation(
        Vec3::splat(1.003),
        Quat::IDENTITY,
        Vec3::from_array(position.map(|axis| axis as f32)) + Vec3::splat(0.5),
    )
}

pub(super) fn hand_instances(
    held: Option<Item>,
    aspect: f32,
    elapsed: f32,
    progress: f32,
) -> ([BlockInstance; HAND_INSTANCES], u32) {
    let held = held.filter(|item| item.stack_limit() > 0);
    let time = animation_time(elapsed);
    let mining = mining_parameters(progress)[0];
    let swing = if mining > 0.0 {
        (time * 9.0).sin() * if held.is_some() { 0.025 } else { 0.055 }
    } else {
        0.0
    };
    let offset = Vec3::new(-swing * 0.5, (time * 1.8).sin() * 0.007 + swing, 0.0);
    let rotation = Quat::from_rotation_z(0.25 + swing)
        * Quat::from_rotation_y(-0.35)
        * Quat::from_rotation_x(-0.20);
    let hand_lift = if held.is_some() { 0.12 } else { 0.0 };
    let mut instances = [BlockInstance::zeroed(); HAND_INSTANCES];
    for (index, center, scale, kind) in [
        (
            0,
            Vec3::new(aspect * 0.46, -0.78 + hand_lift, -0.83),
            Vec3::new(0.20, 0.49, 0.23),
            2,
        ),
        (
            1,
            Vec3::new(aspect * 0.39, -0.53 + hand_lift, -0.91),
            Vec3::new(0.20, 0.28, 0.22),
            1,
        ),
    ] {
        instances[index] = block_instance(
            Block::Stone,
            Mat4::from_scale_rotation_translation(scale, rotation, center + offset),
            1.0,
        );
        instances[index].materials[3] = kind;
    }
    if let Some(Item::Tool(tool)) = held {
        let parts = super::tools::hand(tool, aspect, rotation, offset);
        instances[2..].copy_from_slice(&parts);
        (instances, HAND_INSTANCES as u32)
    } else if let Some(Item::Block(block)) = held {
        instances[2] = block_instance(
            block,
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.28),
                rotation * Quat::from_rotation_y(-0.30),
                Vec3::new(aspect * 0.37, -0.15, -0.95) + offset,
            ),
            1.0,
        );
        (instances, 3)
    } else {
        (instances, 2)
    }
}
