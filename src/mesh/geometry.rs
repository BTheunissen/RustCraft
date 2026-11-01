//! Outward cube faces and opaque vertex emission.

use super::{Mesh, Vertex};

pub(super) struct Face {
    pub(super) offset: [i32; 3],
    pub(super) corners: [[f32; 3]; 4],
}

pub(super) const FACES: [Face; 6] = [
    Face {
        offset: [1, 0, 0],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
    },
    Face {
        offset: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
    },
    Face {
        offset: [0, 1, 0],
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    },
    Face {
        offset: [0, -1, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    },
    Face {
        offset: [0, 0, 1],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
    },
    Face {
        offset: [0, 0, -1],
        corners: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
    },
];

pub(super) const UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

impl Mesh {
    pub(super) fn push_face(&mut self, world: [i32; 3], face: &Face, material: u32, light: f32) {
        let base = self.vertices.len() as u32;
        let normal = face.offset.map(|axis| axis as f32);
        for (corner, uv) in face.corners.into_iter().zip(UVS) {
            self.vertices.push(Vertex {
                position: std::array::from_fn(|axis| world[axis] as f32 + corner[axis]),
                normal,
                uv,
                material,
                light,
                water: [0.0; 4],
                water_extra: [0.0; 4],
                local_light: [0.0; 3],
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
