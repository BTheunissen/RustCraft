//! Independent subpixel renders measure minification against the original art's area average.

use super::super::hdr_readback::half_value;
use super::super::{FrameState, LightingFeatures, RenderResult, Renderer};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextureStability {
    pub(crate) pixels: usize,
    pub(crate) filtered_error: f32,
    pub(crate) nearest_error: f32,
    pub(crate) filtered_variation: f32,
    pub(crate) nearest_variation: f32,
}

impl Renderer {
    pub(crate) fn measure_texture_stability(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<TextureStability> {
        let features = self.lighting_features;
        let offset = self.diagnostic_sample_offset;
        let result = self.with_lighting_features(
            LightingFeatures {
                texture_filtering: true,
                ..features
            },
            |renderer| {
                renderer.diagnostic_sample_offset = [0.0; 2];
                let filtered = renderer.read_scene_pixels(frame)?;
                let (guide, stride) =
                    renderer.read_hdr_texture(&renderer.volumetrics.guide_texture)?;
                renderer.lighting_features.texture_filtering = false;
                let nearest = renderer.read_scene_pixels(frame)?;
                let count = (renderer.config.width * renderer.config.height) as usize;
                let mut original = Moments::new(count);
                let mut stable = Moments::new(count);
                for y in [-0.375, -0.125, 0.125, 0.375] {
                    for x in [-0.375, -0.125, 0.125, 0.375] {
                        renderer.diagnostic_sample_offset = [x, y];
                        renderer.lighting_features.texture_filtering = false;
                        original.add(&renderer.read_scene_pixels(frame)?);
                        renderer.lighting_features.texture_filtering = true;
                        stable.add(&renderer.read_scene_pixels(frame)?);
                    }
                }
                texture_error(
                    &filtered,
                    &nearest,
                    &original,
                    &stable,
                    &guide,
                    [renderer.config.width, renderer.config.height, stride],
                )
            },
        );
        self.diagnostic_sample_offset = offset;
        result
    }
}

struct Moments {
    mean: Vec<[f64; 3]>,
    squares: Vec<[f64; 3]>,
}

impl Moments {
    fn new(count: usize) -> Self {
        Self {
            mean: vec![[0.0; 3]; count],
            squares: vec![[0.0; 3]; count],
        }
    }

    fn add(&mut self, pixels: &[u8]) {
        for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            for (channel, value) in pixel[..3].iter().enumerate() {
                let value = f64::from(*value) / 255.0;
                self.mean[index][channel] += value / 16.0;
                self.squares[index][channel] += value * value / 16.0;
            }
        }
    }

    fn deviation(&self, index: usize, channel: usize) -> f64 {
        (self.squares[index][channel] - self.mean[index][channel].powi(2))
            .max(0.0)
            .sqrt()
    }
}

fn texture_error(
    filtered: &[u8],
    nearest: &[u8],
    reference: &Moments,
    stable: &Moments,
    guide: &[u8],
    [width, height, stride]: [u32; 3],
) -> RenderResult<TextureStability> {
    let mut result = TextureStability {
        pixels: 0,
        filtered_error: 0.0,
        nearest_error: 0.0,
        filtered_variation: 0.0,
        nearest_variation: 0.0,
    };
    let mut errors = [0.0_f64; 2];
    let mut variation = [0.0_f64; 2];
    for y in height / 5..height * 4 / 5 {
        for x in width / 5..width * 4 / 5 {
            let index = (y * width + x) as usize;
            let offset = (y * stride + x * 8) as usize;
            let depth = half_value(u16::from_le_bytes([guide[offset], guide[offset + 1]]));
            let color = &filtered[index * 4..index * 4 + 3];
            if !(20.0..80.0).contains(&depth)
                || f32::from(color[1]) < f32::from(color[0]) * 1.15
                || f32::from(color[1]) < f32::from(color[2]) * 1.15
            {
                continue;
            }
            for channel in 0..3 {
                let target = reference.mean[index][channel];
                if !target.is_finite() {
                    return Err(
                        std::io::Error::other("texture reference contains invalid color").into(),
                    );
                }
                errors[0] += (f64::from(filtered[index * 4 + channel]) / 255.0 - target).abs();
                errors[1] += (f64::from(nearest[index * 4 + channel]) / 255.0 - target).abs();
                variation[0] += stable.deviation(index, channel);
                variation[1] += reference.deviation(index, channel);
            }
            result.pixels += 1;
        }
    }
    let channels = result.pixels.max(1) as f64 * 3.0;
    result.filtered_error = (errors[0] / channels) as f32;
    result.nearest_error = (errors[1] / channels) as f32;
    result.filtered_variation = (variation[0] / channels) as f32;
    result.nearest_variation = (variation[1] / channels) as f32;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_error_counts_only_minified_colored_surfaces_and_excludes_alpha() {
        let mut filtered = vec![0; 8 * 8 * 4];
        let mut nearest = filtered.clone();
        let mut guide = vec![0; 8 * 256];
        let mut reference = Moments::new(64);
        let stable = Moments::new(64);
        for y in 1..6 {
            for x in 1..6 {
                let index = y * 8 + x;
                filtered[index * 4..index * 4 + 4].copy_from_slice(&[40, 140, 40, 1]);
                nearest[index * 4..index * 4 + 4].copy_from_slice(&[90, 200, 90, 255]);
                reference.mean[index] = [40.0 / 255.0, 140.0 / 255.0, 40.0 / 255.0];
                guide[y * 256 + x * 8..y * 256 + x * 8 + 2]
                    .copy_from_slice(&0x5000_u16.to_le_bytes());
            }
        }
        let stats = texture_error(
            &filtered,
            &nearest,
            &reference,
            &stable,
            &guide,
            [8, 8, 256],
        )
        .unwrap();
        assert_eq!(stats.pixels, 25);
        assert!(stats.filtered_error < 0.000001);
        assert!((stats.nearest_error - 160.0 / (3.0 * 255.0)).abs() < 0.000001);
        reference.mean[3 * 8 + 3][1] = f64::NAN;
        assert!(
            texture_error(
                &filtered,
                &nearest,
                &reference,
                &stable,
                &guide,
                [8, 8, 256]
            )
            .is_err()
        );
    }

    #[test]
    fn jitter_moments_distinguish_texture_crawling_from_a_stable_sample() {
        let mut stable = Moments::new(1);
        let mut crawling = Moments::new(1);
        for index in 0..16 {
            stable.add(&[128, 128, 128, 255]);
            crawling.add(&[if index < 8 { 0 } else { 255 }; 4]);
        }
        assert!(stable.deviation(0, 1) < 0.000001);
        assert_eq!(crawling.mean[0], [0.5; 3]);
        assert_eq!(crawling.deviation(0, 1), 0.5);
    }
}
