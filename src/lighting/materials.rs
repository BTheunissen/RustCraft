//! Linear diffuse reflectance and radiance, indexed by voxel IDs, never atlas tiles.

use crate::world::Block;
use glam::Vec3;

pub(crate) const MATERIAL_SLOTS: usize = 32;

pub(crate) fn reflectance(block: Block) -> Vec3 {
    let color = match block {
        Block::Air | Block::Water => [0.0; 3],
        Block::Grass | Block::Leaves | Block::Moss => [0.10, 0.22, 0.055],
        Block::Dirt => [0.22, 0.095, 0.045],
        Block::Stone | Block::Gravel => [0.19, 0.20, 0.19],
        Block::Sand => [0.69, 0.55, 0.30],
        Block::Snow => [0.80, 0.87, 0.91],
        Block::Wood => [0.24, 0.12, 0.045],
        Block::Planks | Block::StorageCrate => [0.38, 0.22, 0.10],
        Block::Bedrock | Block::CoalOre => [0.035, 0.04, 0.055],
        Block::DeepStone | Block::Slate => [0.065, 0.085, 0.12],
        Block::IronOre => [0.19, 0.12, 0.085],
        Block::GoldOre => [0.30, 0.21, 0.06],
        Block::Crystal => [0.09, 0.22, 0.31],
        Block::Lava => [0.40, 0.07, 0.01],
        Block::Limestone | Block::Calcite | Block::DressedLimestone => [0.67, 0.59, 0.43],
        Block::Basalt => [0.09, 0.065, 0.045],
        Block::Clay => [0.20, 0.19, 0.18],
        Block::Lantern => [0.25, 0.17, 0.055],
    };
    Vec3::from_array(color)
}

pub(crate) fn emission(block: Block) -> Vec3 {
    match block {
        Block::Lava => Vec3::new(5.5, 1.35, 0.16),
        Block::Crystal => Vec3::new(0.35, 1.15, 2.4),
        _ => Vec3::ZERO,
    }
}

pub(crate) fn material_tables() -> ([[f32; 4]; MATERIAL_SLOTS], [[f32; 4]; MATERIAL_SLOTS]) {
    let mut albedo = [[0.0; 4]; MATERIAL_SLOTS];
    let mut emissive = [[0.0; 4]; MATERIAL_SLOTS];
    for block in Block::ALL {
        albedo[block as usize] = reflectance(block).extend(0.0).to_array();
        emissive[block as usize] = emission(block).extend(0.0).to_array();
    }
    (albedo, emissive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_transport_is_finite_energy_bounded_and_uses_voxel_ids() {
        let (albedo, emissive) = material_tables();
        for block in Block::ALL {
            let diffuse = reflectance(block);
            assert!(diffuse.is_finite() && diffuse.min_element() >= 0.0);
            assert!(diffuse.max_element() < 1.0);
            assert_eq!(albedo[block as usize], diffuse.extend(0.0).to_array());
            assert_eq!(
                emissive[block as usize],
                emission(block).extend(0.0).to_array()
            );
            assert_eq!(
                emission(block) != Vec3::ZERO,
                matches!(block, Block::Lava | Block::Crystal)
            );
        }
        assert!(emission(Block::Lava).x > emission(Block::Lava).z * 10.0);
        assert!(emission(Block::Crystal).z > emission(Block::Crystal).x * 4.0);
        assert_eq!(emissive[15], [0.0; 4]);
        assert_eq!(emissive[17], [0.0; 4]);
        assert!(
            albedo[Block::COUNT..]
                .iter()
                .all(|entry| *entry == [0.0; 4])
        );
    }
}
