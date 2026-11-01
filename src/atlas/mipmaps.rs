//! Tile-local, linear-light mipmaps retain the art without distant texture crawling.

use super::{Atlas, TILE_COUNT, TILE_SIZE};

pub(crate) struct MipLevel {
    pub(crate) size: u32,
    pub(crate) pixels: Vec<u8>,
}

impl Atlas {
    pub(crate) fn mipmaps(&self) -> Vec<MipLevel> {
        let linear: [f64; 256] = std::array::from_fn(|value| {
            let srgb = value as f64 / 255.0;
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        });
        (0..=TILE_SIZE.ilog2())
            .map(|level| {
                let size = TILE_SIZE >> level;
                let footprint = TILE_SIZE / size;
                let mut pixels = Vec::with_capacity((size * size * TILE_COUNT * 4) as usize);
                for tile in 0..TILE_COUNT {
                    for y in 0..size {
                        for x in 0..size {
                            let mut sum = [0.0; 4];
                            for dy in 0..footprint {
                                for dx in 0..footprint {
                                    let offset = (((y * footprint + dy) * self.width
                                        + tile * TILE_SIZE
                                        + x * footprint
                                        + dx)
                                        * 4)
                                        as usize;
                                    for channel in 0..3 {
                                        sum[channel] +=
                                            linear[self.pixels[offset + channel] as usize];
                                    }
                                    sum[3] += f64::from(self.pixels[offset + 3]) / 255.0;
                                }
                            }
                            let count = f64::from(footprint * footprint);
                            for value in &sum[..3] {
                                pixels.push(encode_srgb(value / count));
                            }
                            pixels.push((sum[3] / count * 255.0).round() as u8);
                        }
                    }
                }
                MipLevel { size, pixels }
            })
            .collect()
    }
}

fn encode_srgb(linear: f64) -> u8 {
    let srgb = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::generate_atlas;

    #[test]
    fn array_layers_preserve_every_original_texel_without_neighboring_materials() {
        let atlas = generate_atlas();
        let levels = atlas.mipmaps();
        assert_eq!(
            levels.iter().map(|mip| mip.size).collect::<Vec<_>>(),
            [16, 8, 4, 2, 1]
        );
        for mip in &levels {
            assert_eq!(
                mip.pixels.len(),
                (mip.size * mip.size * TILE_COUNT * 4) as usize
            );
        }
        for tile in 0..TILE_COUNT {
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let source = ((y * atlas.width + tile * TILE_SIZE + x) * 4) as usize;
                    let layer = (((tile * TILE_SIZE + y) * TILE_SIZE + x) * 4) as usize;
                    assert_eq!(
                        levels[0].pixels[layer..layer + 4],
                        atlas.pixels[source..source + 4]
                    );
                }
            }
        }
    }

    #[test]
    fn minified_tiles_average_in_linear_light_and_never_mix_adjacent_layers() {
        let mut atlas = generate_atlas();
        for y in 0..TILE_SIZE {
            for tile in 0..TILE_COUNT {
                for x in 0..TILE_SIZE {
                    let offset = ((y * atlas.width + tile * TILE_SIZE + x) * 4) as usize;
                    let color = if tile == 0 {
                        [if (x + y).is_multiple_of(2) { 255 } else { 0 }, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    };
                    atlas.pixels[offset..offset + 4].copy_from_slice(&color);
                }
            }
        }
        for mip in atlas.mipmaps().into_iter().skip(1) {
            let layer_bytes = (mip.size * mip.size * 4) as usize;
            assert!(
                mip.pixels[..layer_bytes]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| *pixel == [188, 0, 0, 255])
            );
            assert!(
                mip.pixels[layer_bytes..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| *pixel == [0, 0, 255, 255])
            );
        }
    }

    #[test]
    fn mipmaps_keep_dark_colors_and_alpha_exact_on_uniform_tiles() {
        let mut atlas = generate_atlas();
        for pixel in atlas.pixels.as_chunks_mut::<4>().0 {
            *pixel = [4, 10, 31, 96];
        }
        for mip in atlas.mipmaps() {
            assert!(
                mip.pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| *pixel == [4, 10, 31, 96])
            );
        }
    }
}
