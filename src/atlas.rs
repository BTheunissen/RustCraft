//! Original, deterministic pixel-art block textures generated entirely on the CPU.

/// Width and height of an individual square tile, in pixels.
pub const TILE_SIZE: u32 = 16;

/// Number of tiles in the single-row block atlas.
pub const TILE_COUNT: u32 = 28;

/// A single-row, tightly packed RGBA8 sRGB source image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atlas {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// Row-major RGBA bytes. Every pixel is opaque, including water and leaves.
    pub pixels: Vec<u8>,
}

/// Generates original block textures in renderer tile-index order.
///
/// Tiles are grass top, grass side, dirt, stone, sand, snow, wood side, wood end,
/// leaves, water, bedrock, deep stone, coal, iron, gold, crystal, moss, and lava
/// (0..=17), followed by limestone (18), slate (19), basalt (20), calcite (21),
/// gravel (22), clay (23), planks (24), dressed limestone (25), crate (26), lantern (27).
/// Atlas indices are distinct from world material IDs.
/// Row zero is the upper edge of a side texture. Colors are sRGB bytes; water
/// transparency and crystal/lava emission belong to the renderer, not the atlas.
/// Presentation keeps magnified texels crisp and filters tile-local mip levels when minified.
pub fn generate_atlas() -> Atlas {
    let width = TILE_SIZE * TILE_COUNT;
    let height = TILE_SIZE;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for tile in 0..TILE_COUNT {
            for x in 0..TILE_SIZE {
                let [r, g, b] = material_pixel(tile, x, y);
                pixels.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }

    Atlas {
        width,
        height,
        pixels,
    }
}

fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut value =
        x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b) ^ seed.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value
}

fn noise(x: u32, y: u32, seed: u32, amplitude: i32) -> i32 {
    (hash(x, y, seed) % (amplitude * 2 + 1) as u32) as i32 - amplitude
}

fn shade(base: [u8; 3], amount: i32) -> [u8; 3] {
    base.map(|channel| (i32::from(channel) + amount).clamp(0, 255) as u8)
}

fn dirt(x: u32, y: u32) -> [u8; 3] {
    let grain = noise(x, y, 11, 11) + noise(x / 3, y / 3, 12, 9);
    let base = if hash(x, y, 13) % 23 < 2 {
        [157, 125, 87]
    } else {
        [130, 86, 54]
    };
    shade(base, grain)
}

fn deep_stone(x: u32, y: u32) -> [u8; 3] {
    let strata = match (y + x / 5) % 6 {
        0 => -14,
        1 => 8,
        _ => 0,
    };
    shade(
        [65, 72, 89],
        strata + noise(x, y, 102, 5) + noise(x / 4, y / 3, 103, 6),
    )
}

fn ore_vein(x: u32, y: u32, seed: u32) -> bool {
    let seam = (x + y / 2 + hash(0, y / 4, seed) % 2) % 11;
    let branch = (y + x / 3 + hash(x / 4, 0, seed.wrapping_add(1)) % 2) % 13;
    seam < 2 || branch == 0
}

fn gravel(x: u32, y: u32) -> [u8; 3] {
    // Wrapped, hand-placed ellipses avoid both a mortar grid and isolated noisy flecks.
    const PEBBLES: [(i32, i32, i32, i32); 10] = [
        (2, 2, 2, 2),
        (8, 1, 3, 2),
        (14, 3, 2, 3),
        (5, 6, 3, 2),
        (10, 6, 1, 2),
        (0, 8, 2, 2),
        (12, 10, 3, 2),
        (6, 12, 2, 3),
        (2, 13, 2, 2),
        (12, 15, 2, 2),
    ];
    const PALETTE: [[u8; 3]; 5] = [
        [135, 132, 121],
        [157, 150, 136],
        [141, 137, 129],
        [164, 157, 144],
        [148, 142, 132],
    ];
    let size = TILE_SIZE as i32;
    for (index, (center_x, center_y, radius_x, radius_y)) in PEBBLES.into_iter().enumerate() {
        let dx = (x as i32 - center_x + size / 2).rem_euclid(size) - size / 2;
        let dy = (y as i32 - center_y + size / 2).rem_euclid(size) - size / 2;
        let radius = radius_x * radius_x * radius_y * radius_y;
        let distance = dx * dx * radius_y * radius_y + dy * dy * radius_x * radius_x;
        if distance <= radius {
            let rim = if distance * 4 >= radius * 3 { -8 } else { 0 };
            return shade(
                PALETTE[index % PALETTE.len()],
                -dx * 2 - dy * 3 + rim + noise(x / 2, y / 2, 211, 2),
            );
        }
    }
    shade([102, 100, 93], noise(x / 2, y / 2, 212, 2))
}

fn clay(x: u32, y: u32) -> [u8; 3] {
    let drift = match x / 4 {
        0 | 3 => 0,
        1 => 1,
        _ => 2,
    };
    let sediment = (y + drift) % 9;
    let seam = sediment == 0 && !hash(x / 3, y / 9, 221).is_multiple_of(5);
    let base = if seam {
        [115, 114, 113]
    } else {
        match sediment {
            1 => [137, 134, 131],
            2..=4 => [129, 127, 125],
            _ => [124, 123, 122],
        }
    };
    shade(base, noise(x, y, 222, 2) + noise(x / 4, y / 3, 223, 3))
}

fn material_pixel(tile: u32, x: u32, y: u32) -> [u8; 3] {
    match tile {
        0 => {
            let patch = noise(x / 3, y / 3, 1, 13);
            let grain = noise(x, y, 2, 8);
            let base = if hash(x / 2, y, 3) % 11 < 2 {
                [106, 155, 64]
            } else {
                [77, 132, 49]
            };
            shade(base, patch + grain)
        }
        1 => {
            let fringe = 3 + hash(x / 2, 0, 4) % 3;
            if y < fringe {
                let base = if y == 0 {
                    [109, 157, 66]
                } else if y + 1 == fringe {
                    [62, 106, 42]
                } else {
                    [78, 132, 48]
                };
                shade(base, noise(x, y, 5, 9))
            } else {
                dirt(x, y)
            }
        }
        2 => dirt(x, y),
        3 => {
            let grain = noise(x, y, 21, 10) + noise(x / 4, y / 3, 22, 13);
            let seam = (x + y / 2).is_multiple_of(9) && !hash(x / 3, y / 3, 23).is_multiple_of(3);
            shade([116, 122, 124], grain - if seam { 19 } else { 0 })
        }
        4 => {
            let base = match hash(x, y, 31) % 19 {
                0 | 1 => [188, 161, 110],
                2 | 3 => [238, 218, 167],
                _ => [219, 197, 144],
            };
            shade(base, noise(x / 2, y / 2, 32, 6))
        }
        5 => {
            let base = match hash(x, y, 41) % 23 {
                0 | 1 => [194, 215, 231],
                2..=5 => [247, 250, 248],
                _ => [229, 238, 243],
            };
            shade(base, noise(x / 3, y / 2, 42, 4))
        }
        6 => {
            let furrow = (x + hash(0, y / 6, 51) % 2) % 5;
            let base = match furrow {
                0 => [83, 54, 33],
                1 => [157, 107, 59],
                _ => [126, 82, 44],
            };
            let dx = x as i32 - 11;
            let dy = y as i32 - 8;
            let knot = dx * dx * 3 + dy * dy;
            let knot_shade = if (8..=15).contains(&knot) { -23 } else { 0 };
            shade(base, noise(x, y, 52, 7) + knot_shade)
        }
        7 => {
            if x == 0 || y == 0 || x + 1 == TILE_SIZE || y + 1 == TILE_SIZE {
                return shade([104, 70, 39], noise(x, y, 61, 8));
            }
            let dx = x as i32 - 7;
            let dy = y as i32 - 8;
            let radius_squared = dx * dx + dy * dy;
            let ring = matches!(radius_squared, 8..=13 | 29..=38 | 62..=75 | 110..=126);
            let split = x > 8 && y == 8 + (x - 8) / 5;
            let base = if split {
                [101, 67, 36]
            } else if ring {
                [137, 91, 48]
            } else {
                [183, 139, 82]
            };
            shade(base, noise(x, y, 62, 6))
        }
        8 => {
            let cluster = hash(x / 2, y / 2, 71) % 7;
            let base = match cluster {
                0 | 1 => [40, 77, 38],
                2 | 3 => [76, 129, 52],
                _ => [54, 105, 43],
            };
            let glint = if x.is_multiple_of(2) && y.is_multiple_of(2) {
                12
            } else {
                0
            };
            shade(base, noise(x, y, 72, 8) + glint)
        }
        9 => {
            let ripple_row = (y + (x / 4) % 2) % 8;
            let base = if ripple_row == 3 && !hash(x / 2, y / 8, 81).is_multiple_of(5) {
                [83, 159, 195]
            } else if ripple_row == 4 {
                [35, 99, 156]
            } else {
                [44, 120, 175]
            };
            shade(base, noise(x / 2, y / 2, 82, 5))
        }
        10 => {
            let fracture =
                (x + y / 2).is_multiple_of(7) || (y + hash(x / 4, 0, 91) % 3).is_multiple_of(8);
            let grain = noise(x / 3, y / 3, 92, 8) + noise(x, y, 93, 6);
            shade([43, 46, 53], grain - if fracture { 16 } else { 0 })
        }
        11 => deep_stone(x, y),
        12 => {
            if ore_vein(x, y, 111) {
                let base = if hash(x, y, 112).is_multiple_of(7) {
                    [31, 34, 42]
                } else {
                    [18, 20, 25]
                };
                shade(base, noise(x, y, 113, 4))
            } else {
                deep_stone(x, y)
            }
        }
        13 => {
            if ore_vein(x, y, 121) && !hash(x, y, 122).is_multiple_of(7) {
                let base = match hash(x, y, 123) % 4 {
                    0 => [218, 185, 147],
                    1 => [155, 106, 78],
                    _ => [184, 140, 103],
                };
                shade(base, noise(x, y, 124, 6))
            } else {
                deep_stone(x, y)
            }
        }
        14 => {
            if ore_vein(x, y, 131) && !hash(x, y, 132).is_multiple_of(7) {
                let base = match hash(x, y, 133) % 4 {
                    0 => [255, 220, 83],
                    1 => [174, 117, 22],
                    _ => [234, 177, 34],
                };
                shade(base, noise(x, y, 134, 5))
            } else {
                deep_stone(x, y)
            }
        }
        15 => {
            let dx = x as i32 - 5;
            let dy = y as i32 - 9;
            let facet = dx.abs() * 2 + dy.abs();
            let small_facet = (x as i32 - 12).abs() * 2 + (y as i32 - 4).abs();
            let base = if facet <= 7 {
                if dx == 0 && dy.abs() <= 4 {
                    [172, 240, 252]
                } else if facet >= 6 {
                    [55, 103, 157]
                } else if dx < 0 {
                    [67, 184, 213]
                } else {
                    [130, 91, 209]
                }
            } else if small_facet <= 4 {
                if x < 12 {
                    [99, 204, 232]
                } else {
                    [143, 91, 229]
                }
            } else {
                [35, 39, 59]
            };
            shade(base, noise(x, y, 141, 6))
        }
        16 => {
            let base = match hash(x / 2, y / 2, 151) % 9 {
                0 => [38, 56, 40],
                1 | 2 => [55, 76, 43],
                3 | 4 => [88, 116, 62],
                _ => [62, 98, 52],
            };
            shade(base, noise(x, y, 152, 7) + noise(x / 4, y / 3, 153, 5))
        }
        17 => {
            let crack = (x + y / 3 + hash(0, y / 4, 161) % 2) % 8;
            let branch = (y + x / 4) % 9;
            let base = if crack == 0 || branch == 0 {
                [255, 208, 55]
            } else if crack == 1 || branch == 1 {
                [244, 114, 24]
            } else if crack == 2 || branch == 2 {
                [143, 55, 23]
            } else {
                [85, 32, 24]
            };
            shade(base, noise(x, y, 162, 7))
        }
        18 => {
            let bedding = (y + x / 6 + hash(x / 4, 0, 171) % 2 + 3) % 7;
            let weathered_seam = bedding == 0 && !hash(x / 2, y / 7, 172).is_multiple_of(5);
            let base = if weathered_seam {
                [164, 151, 121]
            } else if hash(x, y, 173).is_multiple_of(31) {
                [186, 171, 135]
            } else {
                match bedding {
                    1 => [235, 226, 198],
                    2 | 3 => [225, 214, 186],
                    _ => [212, 201, 172],
                }
            };
            shade(base, noise(x, y, 174, 4) + noise(x / 3, y / 2, 175, 3))
        }
        19 => {
            let lamina = (y + x / 4 + hash(x / 5, 0, 181) % 2) % 5;
            let fracture = (x + y / 2) % 13 == 5 && !hash(x / 2, y / 3, 182).is_multiple_of(4);
            let base = if fracture {
                [24, 39, 49]
            } else {
                match lamina {
                    0 => [32, 48, 59],
                    1 => [84, 107, 122],
                    _ => [51, 71, 85],
                }
            };
            shade(base, noise(x, y, 183, 3) + noise(x / 3, y / 2, 184, 3))
        }
        20 => {
            let bend = match y % 12 {
                0..=2 | 10..=11 => 1,
                _ => 0,
            };
            let column_edge = (x + bend) % 6;
            // Sparse, staggered cross-joints keep cooling columns tall rather than brick-like.
            let cross_joint =
                (y + (x / 6 % 2) * 6).is_multiple_of(12) && (2..=4).contains(&column_edge);
            let base = if column_edge == 0 || cross_joint {
                [36, 32, 30]
            } else {
                match column_edge {
                    1 => [98, 86, 76],
                    5 => [64, 58, 50],
                    _ => [80, 71, 66],
                }
            };
            shade(base, noise(x, y, 191, 3) + noise(x / 3, y / 4, 192, 4))
        }
        21 => {
            let drift = match y / 4 {
                0 => 0,
                1 | 3 => 1,
                _ => 2,
            };
            let drape = (x + drift) % 8;
            let base = if hash(x, y, 201).is_multiple_of(23) {
                [209, 193, 166]
            } else {
                match drape {
                    0 => [179, 160, 131],
                    1 => [251, 244, 227],
                    2 | 3 => [241, 232, 211],
                    4..=6 => [231, 222, 202],
                    _ => [215, 203, 180],
                }
            };
            let terrace = if (y + x / 5).is_multiple_of(11) && drape > 3 {
                -8
            } else {
                0
            };
            shade(
                base,
                terrace + noise(x, y, 202, 3) + noise(x / 2, y / 3, 203, 2),
            )
        }
        22 => gravel(x, y),
        23 => clay(x, y),
        24 => building::planks(x, y),
        25 => building::dressed_limestone(x, y),
        26 => building::storage_crate(x, y),
        27 => building::lantern(x, y),
        _ => unreachable!("only valid atlas tiles are generated"),
    }
}

mod building;
mod mipmaps;
#[cfg(test)]
mod tests;
