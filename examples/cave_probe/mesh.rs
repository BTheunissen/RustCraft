//! Geometry/atlas validation for the CPU cave survey.

use super::*;

pub(super) struct MeshSurvey {
    pub(super) vertices: [usize; 2],
    pub(super) triangles: [usize; 2],
    pub(super) underground_faces: [usize; TILE_COUNT as usize],
    pub(super) exposed_liquids: [Option<[i32; 3]>; 2],
    pub(super) light_min: f32,
    pub(super) light_max: f32,
}

impl Default for MeshSurvey {
    fn default() -> Self {
        Self {
            vertices: [0; 2],
            triangles: [0; 2],
            underground_faces: [0; TILE_COUNT as usize],
            exposed_liquids: [None; 2],
            light_min: 1.0,
            light_max: 0.0,
        }
    }
}

impl MeshSurvey {
    pub(super) fn record(&mut self, mesh: &Mesh, water: bool) -> Result<(), String> {
        if !mesh.indices.len().is_multiple_of(3) || !mesh.vertices.len().is_multiple_of(4) {
            return Err("incomplete face or triangle data".into());
        }
        for vertex in &mesh.vertices {
            if !vertex
                .position
                .iter()
                .chain(&vertex.normal)
                .chain(&vertex.uv)
                .chain(std::iter::once(&vertex.light))
                .chain(&vertex.water)
                .chain(&vertex.water_extra)
                .all(|value| value.is_finite())
                || !(0.0..=1.0).contains(&vertex.light)
            {
                return Err("non-finite vertex data or light outside 0..=1".into());
            }
            if vertex.material >= TILE_COUNT || (vertex.material == 9) != water {
                return Err(format!("invalid material/draw pass: {}", vertex.material));
            }
        }
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [Some(a), Some(b), Some(c)] = triangle.map(|id| mesh.vertices.get(id as usize))
            else {
                return Err("triangle index outside vertex buffer".into());
            };
            let ab = Vec3::from_array(b.position) - Vec3::from_array(a.position);
            let ac = Vec3::from_array(c.position) - Vec3::from_array(a.position);
            if ab.cross(ac).dot(Vec3::from_array(a.normal)) <= 0.0 {
                return Err("degenerate or inward triangle".into());
            }
        }
        let pass = usize::from(water);
        self.vertices[pass] += mesh.vertices.len();
        self.triangles[pass] += mesh.indices.len() / 3;
        for face in mesh.vertices.as_chunks::<4>().0 {
            let center = face
                .iter()
                .map(|vertex| Vec3::from_array(vertex.position))
                .sum::<Vec3>()
                / 4.0;
            // The world's outside-bottom shell is not an exposed cave wall.
            if center.y >= 0.0 || (center.y == MIN_WORLD_Y as f32 && face[0].normal[1] == -1.0) {
                continue;
            }
            let material = face[0].material as usize;
            self.underground_faces[material] += 1;
            for vertex in face {
                self.light_min = self.light_min.min(vertex.light);
                self.light_max = self.light_max.max(vertex.light);
            }
            let liquid = match material {
                9 => Some(0),
                17 => Some(1),
                _ => None,
            };
            if let Some(liquid) = liquid {
                let block = center - Vec3::from_array(face[0].normal) * 0.5;
                self.exposed_liquids[liquid].get_or_insert_with(|| {
                    block.to_array().map(|coordinate| coordinate.floor() as i32)
                });
            }
        }
        Ok(())
    }
}
