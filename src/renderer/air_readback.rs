//! Linear half-resolution air evidence, independent of surfaces, bloom, lamps, and HUD.

use super::hdr_readback::half_value;
use super::lighting::LightingFeatures;
use super::{FrameState, RenderResult, Renderer};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AirEdgeStats {
    pub(crate) pixels: u32,
    pub(crate) mean_error: f32,
    pub(crate) peak_error: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AirStats {
    pub(crate) mean_radiance: f32,
    pub(crate) peak_radiance: f32,
    pub(crate) deviation: f32,
    pub(crate) lit_fraction: f32,
}

impl Renderer {
    pub(crate) fn measure_air_edges(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<AirEdgeStats> {
        let frame = FrameState {
            hud: &[],
            survival: None,
            ..*frame
        };
        self.with_lighting_features(
            LightingFeatures {
                volumetric_reference: false,
                ..self.lighting_features
            },
            |renderer| {
                renderer.render_offscreen(&frame)?;
                let (half, stride) =
                    renderer.read_hdr_texture(&renderer.postprocess.scene_texture)?;
                let (guide, _) = renderer.read_hdr_texture(&renderer.volumetrics.guide_texture)?;
                renderer.lighting_features.volumetric_reference = true;
                renderer.render_offscreen(&frame)?;
                let (reference, _) =
                    renderer.read_hdr_texture(&renderer.postprocess.scene_texture)?;
                edge_stats(
                    &half,
                    &reference,
                    &guide,
                    renderer.config.width,
                    renderer.config.height,
                    stride,
                )
            },
        )
    }

    pub(crate) fn measure_air(&mut self, frame: &FrameState<'_>) -> RenderResult<AirStats> {
        let frame = FrameState {
            hud: &[],
            survival: None,
            ..*frame
        };
        self.render_offscreen(&frame)?;
        let texture = &self.volumetrics.scatter_texture;
        let (bytes, stride) = self.read_hdr_texture(texture)?;
        air_stats(&bytes, texture.width(), texture.height(), stride)
    }

    /// A center-mouth sample detects sky-colored unloaded cave background, not merely darkness elsewhere.
    pub(crate) fn measure_entrance_background(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<f32> {
        self.render_offscreen(&FrameState {
            hud: &[],
            survival: None,
            ..*frame
        })?;
        let (bytes, stride) = self.read_hdr_texture(&self.postprocess.scene_texture)?;
        let mut peak = 0.0_f32;
        for y in self.config.height * 40 / 100..self.config.height * 44 / 100 {
            for x in self.config.width * 50 / 100..self.config.width * 54 / 100 {
                let start = (y * stride + x * 8) as usize;
                for channel in 0..3 {
                    let radiance = half_channel(&bytes, start + channel * 2);
                    if !radiance.is_finite() || radiance < 0.0 {
                        return Err(std::io::Error::other(
                            "entrance background contains invalid HDR radiance",
                        )
                        .into());
                    }
                    peak = peak.max(radiance);
                }
            }
        }
        Ok(peak)
    }
}

fn half_channel(bytes: &[u8], offset: usize) -> f32 {
    half_value(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}

fn edge_stats(
    half: &[u8],
    reference: &[u8],
    guide: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> RenderResult<AirEdgeStats> {
    let mut total = 0.0_f64;
    let mut peak = 0.0_f32;
    let mut pixels = 0;
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let offset = (y * stride + x * 8) as usize;
            let depth = half_channel(guide, offset);
            let tolerance = (depth * 0.02).max(0.10);
            let edge = [
                offset - 8,
                offset + 8,
                offset - stride as usize,
                offset + stride as usize,
            ]
            .into_iter()
            .any(|neighbor| (half_channel(guide, neighbor) - depth).abs() > tolerance * 2.0);
            if !edge {
                continue;
            }
            let mut error = 0.0_f32;
            for channel in 0..3 {
                let difference = (half_channel(half, offset + channel * 2)
                    - half_channel(reference, offset + channel * 2))
                .abs();
                if !difference.is_finite() {
                    return Err(std::io::Error::other(
                        "air edge comparison contains invalid radiance",
                    )
                    .into());
                }
                error = error.max(difference);
            }
            total += f64::from(error);
            peak = peak.max(error);
            pixels += 1;
        }
    }
    Ok(AirEdgeStats {
        pixels,
        mean_error: (total / f64::from(pixels.max(1))) as f32,
        peak_error: peak,
    })
}

fn air_stats(bytes: &[u8], width: u32, height: u32, stride: u32) -> RenderResult<AirStats> {
    let mut total = 0.0_f64;
    let mut squares = 0.0_f64;
    let mut peak = 0.0_f32;
    let mut lit = 0_u64;
    let mut count = 0_u64;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let start = (y * stride + x * 8) as usize;
            let mut luminance = 0.0_f64;
            for (channel, weight) in [0.2126, 0.7152, 0.0722].into_iter().enumerate() {
                let offset = start + channel * 2;
                let value = half_value(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
                if !value.is_finite() || value < 0.0 {
                    return Err(
                        std::io::Error::other("air buffer contains invalid radiance").into(),
                    );
                }
                peak = peak.max(value);
                luminance += f64::from(value) * weight;
            }
            total += luminance;
            squares += luminance * luminance;
            lit += u64::from(luminance > 0.0001);
            count += 1;
        }
    }
    let mean = total / count.max(1) as f64;
    Ok(AirStats {
        mean_radiance: mean as f32,
        peak_radiance: peak,
        deviation: (squares / count.max(1) as f64 - mean * mean)
            .max(0.0)
            .sqrt() as f32,
        lit_fraction: lit as f32 / count.max(1) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_evidence_excludes_distance_alpha_hud_margins_and_padding() {
        let mut bytes = vec![0; 8 * 256];
        for y in 2..6 {
            for x in 2..6 {
                let start = y * 256 + x * 8;
                for channel in 0..3 {
                    bytes[start + channel * 2..start + channel * 2 + 2]
                        .copy_from_slice(&0x3000_u16.to_le_bytes());
                }
                bytes[start + 6..start + 8].copy_from_slice(&0x7e00_u16.to_le_bytes());
            }
        }
        let stats = air_stats(&bytes, 8, 8, 256).unwrap();
        assert_eq!(stats.mean_radiance, 0.125);
        assert_eq!(stats.peak_radiance, 0.125);
        assert_eq!(stats.lit_fraction, 1.0);
        assert!(stats.deviation < 0.000001);
        bytes[..64].fill(255);
        bytes[64..256].fill(255);
        assert_eq!(air_stats(&bytes, 8, 8, 256).unwrap().mean_radiance, 0.125);
    }

    #[test]
    fn air_evidence_distinguishes_beams_from_flat_fog_and_rejects_bad_energy() {
        let mut bytes = vec![0; 8 * 256];
        let offset = 3 * 256 + 3 * 8;
        bytes[offset..offset + 2].copy_from_slice(&0x2c00_u16.to_le_bytes());
        let beam = air_stats(&bytes, 8, 8, 256).unwrap();
        assert!(beam.deviation > 0.003);
        assert_eq!(beam.lit_fraction, 1.0 / 16.0);
        for invalid in [0xbc00_u16, 0x7c00, 0x7e00] {
            bytes[offset..offset + 2].copy_from_slice(&invalid.to_le_bytes());
            assert!(air_stats(&bytes, 8, 8, 256).is_err());
        }
        let empty = air_stats(&[], 0, 0, 0).unwrap();
        assert_eq!(empty.mean_radiance, 0.0);
        assert_eq!(empty.peak_radiance, 0.0);
        assert_eq!(empty.deviation, 0.0);
        assert_eq!(empty.lit_fraction, 0.0);
    }

    #[test]
    fn edge_evidence_measures_only_depth_discontinuities_and_rejects_invalid_energy() {
        let stride = 256;
        let mut guide = vec![0; 8 * stride];
        let half = vec![0; 8 * stride];
        let mut reference = half.clone();
        for y in 0..8 {
            for x in 0..8 {
                let depth = if x < 4 { 0x4900_u16 } else { 0x5640_u16 };
                guide[y * stride + x * 8..y * stride + x * 8 + 2]
                    .copy_from_slice(&depth.to_le_bytes());
            }
        }
        reference[3 * stride + 2 * 8..3 * stride + 2 * 8 + 2]
            .copy_from_slice(&0x3c00_u16.to_le_bytes());
        assert_eq!(
            edge_stats(&half, &reference, &guide, 8, 8, 256)
                .unwrap()
                .peak_error,
            0.0
        );
        let offset = 3 * stride + 3 * 8;
        reference[offset..offset + 2].copy_from_slice(&0x2000_u16.to_le_bytes());
        let stats = edge_stats(&half, &reference, &guide, 8, 8, 256).unwrap();
        assert_eq!(stats.pixels, 12);
        assert_eq!(stats.peak_error, 0.0078125);
        assert!((stats.mean_error - 0.0078125 / 12.0).abs() < 0.000001);
        reference[offset..offset + 2].copy_from_slice(&0x7e00_u16.to_le_bytes());
        assert!(edge_stats(&half, &reference, &guide, 8, 8, 256).is_err());
        assert_eq!(edge_stats(&[], &[], &[], 0, 0, 0).unwrap().pixels, 0);
    }
}
