//! Water and sediment geometry validation.

use super::*;

pub(super) fn mesh_materials(
    mesh: &Mesh,
    water: bool,
) -> Result<[usize; TILE_COUNT as usize], String> {
    if !mesh.vertices.len().is_multiple_of(4) || mesh.indices.len() != mesh.vertices.len() / 4 * 6 {
        return Err("incomplete quad/triangle mesh".into());
    }
    let mut counts = [0; TILE_COUNT as usize];
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
            || !vertex
                .uv
                .into_iter()
                .all(|value| (0.0..=1.0).contains(&value))
        {
            return Err("non-finite or out-of-range vertex data".into());
        }
        if vertex.material >= TILE_COUNT || (vertex.material == 9) != water {
            return Err(format!("invalid atlas tile/draw pass: {}", vertex.material));
        }
        if water {
            let biome = vertex.water_extra[1];
            if vertex.water[2] <= 0.0
                || !(0.0..=1.0).contains(&vertex.water[3])
                || !(0.0..WATER_BIOMES.len() as f32).contains(&biome)
                || biome.fract() != 0.0
                || !(0.0..=1.0).contains(&vertex.water_extra[2])
                || !(0.0..=1.0).contains(&vertex.water_extra[3])
            {
                return Err("invalid water depth/salinity/biome/foam/wave metadata".into());
            }
        } else if vertex.water != [0.0; 4] || vertex.water_extra != [0.0; 4] {
            return Err("opaque vertices contain water metadata".into());
        }
        counts[vertex.material as usize] += 1;
    }
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [Some(a), Some(b), Some(c)] = triangle.map(|id| mesh.vertices.get(id as usize)) else {
            return Err("triangle index outside vertex buffer".into());
        };
        let ab = Vec3::from_array(b.position) - Vec3::from_array(a.position);
        let ac = Vec3::from_array(c.position) - Vec3::from_array(a.position);
        if ab.cross(ac).dot(Vec3::from_array(a.normal)) <= 0.0 {
            return Err("degenerate or inward triangle".into());
        }
    }
    Ok(counts)
}
