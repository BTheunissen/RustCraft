//! Original bounded cuboid silhouettes; tools have no voxel/atlas identity.

use glam::{Mat4, Quat, Vec3};

use super::geometry::{BlockInstance, block_instance};
use crate::game::Tool;
use crate::world::Block;

pub(super) fn hand(tool: Tool, aspect: f32, rotation: Quat, offset: Vec3) -> [BlockInstance; 3] {
    let origin = Vec3::new(aspect * 0.40, -0.16, -1.12) + offset;
    let head = match tool {
        Tool::StonePickaxe => [
            (Vec3::new(0.0, 0.24, 0.0), Vec3::new(0.36, 0.07, 0.09)),
            (Vec3::new(0.14, 0.19, 0.0), Vec3::new(0.07, 0.10, 0.09)),
        ],
        Tool::StoneAxe => [
            (Vec3::new(0.11, 0.19, 0.0), Vec3::new(0.20, 0.20, 0.09)),
            (Vec3::new(0.0, 0.20, 0.0), Vec3::new(0.08, 0.12, 0.11)),
        ],
    };
    let mut parts = [block_instance(
        Block::Wood,
        Mat4::from_scale_rotation_translation(Vec3::new(0.055, 0.43, 0.06), rotation, origin),
        1.0,
    ); 3];
    for (index, (center, scale)) in head.into_iter().enumerate() {
        parts[index + 1] = block_instance(
            Block::Stone,
            Mat4::from_scale_rotation_translation(scale, rotation, origin + rotation * center),
            1.0,
        );
    }
    parts
}

pub(super) fn pickup(
    tool: Tool,
    position: Vec3,
    age: f32,
    elapsed: f32,
    light: f32,
) -> [BlockInstance; 3] {
    let time = super::geometry::animation_time(elapsed);
    let phase = super::geometry::animation_time(age) * 0.37;
    let rotation = Quat::from_rotation_y((time * 1.1 + phase).rem_euclid(std::f32::consts::TAU));
    let origin = position + Vec3::Y * (0.035 + (time * 2.3 + phase).sin() * 0.025);
    let head = match tool {
        Tool::StonePickaxe => [
            (Vec3::new(0.0, 0.09, 0.0), Vec3::new(0.22, 0.035, 0.04)),
            (Vec3::new(0.08, 0.055, 0.0), Vec3::new(0.04, 0.06, 0.04)),
        ],
        Tool::StoneAxe => [
            (Vec3::new(0.04, 0.055, 0.0), Vec3::new(0.10, 0.10, 0.04)),
            (Vec3::new(0.0, 0.06, 0.0), Vec3::new(0.04, 0.07, 0.05)),
        ],
    };
    let mut parts = [block_instance(
        Block::Wood,
        Mat4::from_scale_rotation_translation(Vec3::new(0.03, 0.20, 0.03), rotation, origin),
        light,
    ); 3];
    for (index, (offset, scale)) in head.into_iter().enumerate() {
        parts[index + 1] = block_instance(
            Block::Stone,
            Mat4::from_scale_rotation_translation(scale, rotation, origin + rotation * offset),
            light,
        );
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::survival::geometry::{cube_vertices, hand_instances};

    #[test]
    fn original_tools_have_distinct_finite_bounded_held_and_drop_silhouettes() {
        let (pick, count) = hand_instances(Some(Tool::StonePickaxe.into()), 1.6, 0.0, 0.0);
        let (axe, axe_count) = hand_instances(Some(Tool::StoneAxe.into()), 1.6, 0.0, 0.0);
        assert_eq!((count, axe_count), (5, 5));
        assert_ne!(pick[3].model, axe[3].model);
        for tool in Tool::ALL {
            for elapsed in [0.0, 1.0, f32::NAN] {
                let position = Vec3::new(-16.1, -30.0, -0.5);
                for instance in pickup(tool, position, 1.0, elapsed, 0.5) {
                    let model = Mat4::from_cols_array_2d(&instance.model);
                    assert!(model.is_finite() && model.determinant() > 0.0);
                    for vertex in cube_vertices() {
                        let offset =
                            model.transform_point3(Vec3::from_array(vertex.position)) - position;
                        assert!(offset.abs().max_element() < 0.19);
                    }
                }
            }
        }
    }
}
