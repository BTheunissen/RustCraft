//! Actual pre-tone-map floating-point GPU evidence, not an inference from final PNGs.

use super::{FrameState, RenderResult, Renderer, check_gpu_errors};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HdrStats {
    pub(crate) peak_channel: f32,
    pub(crate) highlight_fraction: f32,
}

impl Renderer {
    pub(crate) fn measure_hdr(&mut self, frame: &FrameState<'_>) -> RenderResult<HdrStats> {
        let frame = FrameState {
            hud: &[],
            survival: None,
            ..*frame
        };
        self.render_offscreen(&frame)?;
        let texture = &self.postprocess.scene_texture;
        let (bytes, stride) = self.read_hdr_texture(texture)?;
        hdr_stats(&bytes, texture.width(), texture.height(), stride)
    }

    pub(super) fn read_hdr_texture(&self, texture: &wgpu::Texture) -> RenderResult<(Vec<u8>, u32)> {
        let width = texture.width();
        let height = texture.height();
        let stride = (width * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("linear HDR verification readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        self.queue.submit(Some(encoder.finish()));
        let (send, receive) = mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(self.wait_timeout()?),
        })?;
        check_gpu_errors(&self.gpu_errors)?;
        receive.recv_timeout(self.wait_timeout()?)??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let bytes = mapped.to_vec();
        drop(mapped);
        buffer.unmap();
        Ok((bytes, stride))
    }
}

fn hdr_stats(bytes: &[u8], width: u32, height: u32, stride: u32) -> RenderResult<HdrStats> {
    let mut peak = 0.0_f32;
    let mut highlights = 0_u64;
    let mut count = 0_u64;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let start = (y * stride + x * 8) as usize;
            let mut highlight = false;
            for channel in 0..3 {
                let offset = start + channel * 2;
                let value = half_value(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
                if !value.is_finite() || value < 0.0 {
                    return Err(std::io::Error::other("HDR scene contains invalid radiance").into());
                }
                peak = peak.max(value);
                highlight |= value > 1.0;
            }
            highlights += u64::from(highlight);
            count += 1;
        }
    }
    Ok(HdrStats {
        peak_channel: peak,
        highlight_fraction: highlights as f32 / count.max(1) as f32,
    })
}

pub(super) fn half_value(value: u16) -> f32 {
    let sign = u32::from(value & 0x8000) << 16;
    let exponent = u32::from((value >> 10) & 31);
    let mantissa = u32::from(value & 1023);
    match exponent {
        0 => {
            let magnitude = mantissa as f32 * 2.0_f32.powi(-24);
            if sign == 0 { magnitude } else { -magnitude }
        }
        31 => f32::from_bits(sign | 0x7f80_0000 | mantissa << 13),
        _ => f32::from_bits(sign | ((exponent + 112) << 23) | mantissa << 13),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_decode_preserves_hdr_normal_subnormal_and_invalid_values() {
        for (bits, expected) in [
            (0, 0.0),
            (0x8000, -0.0),
            (0x3c00, 1.0),
            (0x4000, 2.0),
            (0x4500, 5.0),
            (0xc000, -2.0),
            (0x7bff, 65504.0),
            (1, 2.0_f32.powi(-24)),
        ] {
            assert_eq!(half_value(bits), expected);
        }
        assert_eq!(half_value(0x7c00), f32::INFINITY);
        assert!(half_value(0x7e00).is_nan());
    }

    #[test]
    fn hdr_evidence_ignores_alpha_and_padding_but_rejects_nonfinite_radiance() {
        let mut bytes = vec![0; 4 * 256];
        for y in 1..3 {
            for x in 1..3 {
                let offset = y * 256 + x * 8;
                bytes[offset..offset + 2].copy_from_slice(&0x4000_u16.to_le_bytes());
                bytes[offset + 6..offset + 8].copy_from_slice(&0x7e00_u16.to_le_bytes());
            }
        }
        let stats = hdr_stats(&bytes, 4, 4, 256).unwrap();
        assert_eq!(stats.peak_channel, 2.0);
        assert_eq!(stats.highlight_fraction, 1.0);
        bytes[256 + 8..256 + 10].copy_from_slice(&0x7c00_u16.to_le_bytes());
        assert!(hdr_stats(&bytes, 4, 4, 256).is_err());
        bytes[256 + 8..256 + 10].copy_from_slice(&0xbc00_u16.to_le_bytes());
        assert!(hdr_stats(&bytes, 4, 4, 256).is_err());
        let empty = hdr_stats(&[], 0, 0, 0).unwrap();
        assert_eq!(empty.peak_channel, 0.0);
        assert_eq!(empty.highlight_fraction, 0.0);
    }
}
