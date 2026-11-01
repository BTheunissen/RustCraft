//! Seeded, world-coordinate noise shared by generation stages.

pub(super) fn smoothstep(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

pub(in crate::world::generation) fn value_noise(seed: u64, x: f64, z: f64) -> f64 {
    let floor_x = x.floor();
    let floor_z = z.floor();
    let fraction_x = smoothstep(x - floor_x);
    let fraction_z = smoothstep(z - floor_z);
    let x = floor_x as i64;
    let z = floor_z as i64;
    let sample = |x, z| (hash_2d(seed, x, z) >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0;
    let low = sample(x, z) * (1.0 - fraction_x) + sample(x + 1, z) * fraction_x;
    let high = sample(x, z + 1) * (1.0 - fraction_x) + sample(x + 1, z + 1) * fraction_x;
    low * (1.0 - fraction_z) + high * fraction_z
}

pub(in crate::world::generation) fn hash_2d(seed: u64, x: i64, z: i64) -> u64 {
    let mut value = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
