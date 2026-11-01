//! GPU texture readback and scene-only acceptance statistics.

use super::pipelines::depth_view;
use super::{FrameDifference, FrameState, FrameStats, RenderResult, Renderer, check_gpu_errors};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::mpsc;

impl Renderer {
    /// Exercises the actual interactive fixed-budget schedule against a fully updated reference.
    pub(crate) fn compare_progressive_lighting(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<(FrameDifference, FrameDifference)> {
        let frame = FrameState {
            hud: &[],
            survival: None,
            ..*frame
        };
        let saved = self.progressive_lighting;
        let result = (|| {
            self.progressive_lighting = false;
            let (reference, _) = self.readback_frame(&frame)?;
            self.resources.lighting.reset_probes(&self.queue);
            self.progressive_lighting = true;
            let (initial, _) = self.readback_frame(&frame)?;
            for _ in 1..crate::lighting::volume::PROBE_COUNT
                / crate::lighting::volume::PROBES_PER_FRAME
                - 1
            {
                self.render_offscreen(&frame)?;
            }
            let (complete, _) = self.readback_frame(&frame)?;
            Ok((
                scene_difference(&initial, &reference, self.config.width, self.config.height),
                scene_difference(&complete, &reference, self.config.width, self.config.height),
            ))
        })();
        self.progressive_lighting = saved;
        result
    }

    /// Isolates one lighting component at a fixed pose/time, with overlays removed.
    pub(crate) fn compare_lighting(
        &mut self,
        frame: &FrameState<'_>,
        features: super::lighting::LightingFeatures,
    ) -> RenderResult<FrameDifference> {
        let frame = FrameState {
            hud: &[],
            survival: None,
            ..*frame
        };
        let saved = self.lighting_features;
        let result = (|| {
            let (before, _) = self.readback_frame(&frame)?;
            self.lighting_features = features;
            let (after, _) = self.readback_frame(&frame)?;
            Ok(scene_difference(
                &before,
                &after,
                self.config.width,
                self.config.height,
            ))
        })();
        self.lighting_features = saved;
        result
    }

    pub(crate) fn with_lighting_features<T>(
        &mut self,
        features: super::lighting::LightingFeatures,
        render: impl FnOnce(&mut Self) -> RenderResult<T>,
    ) -> RenderResult<T> {
        let saved = self.lighting_features;
        self.lighting_features = features;
        let result = render(self);
        self.lighting_features = saved;
        result
    }

    /// Renders the real scene to an offscreen texture and saves lossless GPU readback.
    /// This does not require desktop screen-recording permissions.
    pub fn save_screenshot(&mut self, path: &Path, frame: &FrameState<'_>) -> RenderResult<()> {
        self.capture_frame(path, frame).map(|_| ())
    }

    /// Saves a scene and returns readback luminance evidence for lighting checks.
    pub fn capture_frame(
        &mut self,
        path: &Path,
        frame: &FrameState<'_>,
    ) -> RenderResult<FrameStats> {
        let (pixels, stats) = self.readback_frame(frame)?;
        let writer = BufWriter::new(File::create(path)?);
        let mut encoder = png::Encoder::new(writer, self.config.width, self.config.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
        writer.finish()?;
        println!("Screenshot: {}", path.display());
        Ok(stats)
    }

    /// Reads scene-only statistics without creating any files.
    pub fn measure_frame(&mut self, frame: &FrameState<'_>) -> RenderResult<FrameStats> {
        self.readback_frame(frame).map(|(_, stats)| stats)
    }

    /// Compares actual shader output at two times or fluid states, excluding the HUD.
    pub fn compare_frames(
        &mut self,
        first: &FrameState<'_>,
        second: &FrameState<'_>,
    ) -> RenderResult<FrameDifference> {
        let first = FrameState { hud: &[], ..*first };
        let second = FrameState {
            hud: &[],
            ..*second
        };
        let (before, _) = self.readback_frame(&first)?;
        let (after, _) = self.readback_frame(&second)?;
        Ok(scene_difference(
            &before,
            &after,
            self.config.width,
            self.config.height,
        ))
    }

    /// Compares the entire shader output, including HUD and first-person overlays.
    /// Keep camera pose and animation time fixed to isolate hand, UI, or mining changes.
    pub fn compare_full_frames(
        &mut self,
        first: &FrameState<'_>,
        second: &FrameState<'_>,
    ) -> RenderResult<FrameDifference> {
        self.compare_rebuilt_frames(first, second, |_| Ok(()))
    }

    /// Captures the baseline before replacing GPU meshes, then compares the rebuilt
    /// frame. Otherwise both reads would silently use the already-restored geometry.
    pub(crate) fn compare_rebuilt_frames(
        &mut self,
        first: &FrameState<'_>,
        second: &FrameState<'_>,
        rebuild: impl FnOnce(&mut Self) -> RenderResult<()>,
    ) -> RenderResult<FrameDifference> {
        let (before, _) = self.readback_frame(first)?;
        rebuild(self)?;
        let (after, _) = self.readback_frame(second)?;
        Ok(full_frame_difference(
            &before,
            &after,
            self.config.width,
            self.config.height,
        ))
    }

    /// Isolates the real water draw pass at one pose/time, including reverse-facing surfaces.
    /// This diagnostic cannot pass merely because underwater fog or the sky is visible.
    pub fn measure_water_pass(&mut self, frame: &FrameState<'_>) -> RenderResult<FrameDifference> {
        let frame = FrameState { hud: &[], ..*frame };
        let (without_water, _) = self.readback_scene(&frame, false)?;
        let (with_water, _) = self.readback_scene(&frame, true)?;
        Ok(scene_difference(
            &without_water,
            &with_water,
            self.config.width,
            self.config.height,
        ))
    }

    /// Measures scene RGB for aquatic palette checks, excluding the HUD and outer margins.
    pub fn measure_scene_color(&mut self, frame: &FrameState<'_>) -> RenderResult<[f32; 3]> {
        let frame = FrameState { hud: &[], ..*frame };
        let (pixels, _) = self.readback_frame(&frame)?;
        Ok(scene_color(&pixels, self.config.width, self.config.height))
    }

    pub(super) fn readback_frame(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<(Vec<u8>, FrameStats)> {
        self.readback_scene(frame, true)
    }

    pub(super) fn read_scene_pixels(&mut self, frame: &FrameState<'_>) -> RenderResult<Vec<u8>> {
        self.readback_scene(frame, false).map(|(pixels, _)| pixels)
    }

    fn readback_scene(
        &mut self,
        frame: &FrameState<'_>,
        draw_water: bool,
    ) -> RenderResult<(Vec<u8>, FrameStats)> {
        check_gpu_errors(&self.gpu_errors)?;
        self.prepare_frame(frame);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verification frame"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let depth = depth_view(&self.device, self.config.width, self.config.height);
        let row_bytes = self.config.width * 4;
        let padded_row_bytes = padded_row_bytes(self.config.width);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot readback"),
            size: u64::from(padded_row_bytes) * u64::from(self.config.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode_scene(&mut encoder, &view, &depth, frame, draw_water);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(self.config.height),
                },
            },
            texture.size(),
        );
        self.queue.submit(Some(encoder.finish()));
        let (sent, received) = mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sent.send(result);
            });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(self.wait_timeout()?),
        })?;
        check_gpu_errors(&self.gpu_errors)?;
        received.recv_timeout(self.wait_timeout()?)??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let pixels = unpack_pixels(&mapped, row_bytes as usize, padded_row_bytes as usize, bgra);
        let stats = scene_stats(&pixels, self.config.width, self.config.height);
        drop(mapped);
        buffer.unmap();
        Ok((pixels, stats))
    }
}

pub(super) fn padded_row_bytes(width: u32) -> u32 {
    (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

pub(super) fn unpack_pixels(data: &[u8], row_bytes: usize, stride: usize, bgra: bool) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(data.len() / stride * row_bytes);
    for row in data.chunks_exact(stride) {
        pixels.extend_from_slice(&row[..row_bytes]);
    }
    if bgra {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    pixels
}

pub(super) fn scene_stats(pixels: &[u8], width: u32, height: u32) -> FrameStats {
    let mut total = 0.0;
    let mut total_squared = 0.0;
    let mut visible = 0;
    let mut count = 0;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let offset = ((y * width + x) * 4) as usize;
            let luminance = (f64::from(pixels[offset]) * 0.2126
                + f64::from(pixels[offset + 1]) * 0.7152
                + f64::from(pixels[offset + 2]) * 0.0722)
                / 255.0;
            total += luminance;
            total_squared += luminance * luminance;
            visible += usize::from(luminance > 0.07);
            count += 1;
        }
    }
    let mean = total / count.max(1) as f64;
    FrameStats {
        mean_luminance: mean as f32,
        visible_fraction: visible as f32 / count.max(1) as f32,
        luminance_deviation: (total_squared / count.max(1) as f64 - mean * mean)
            .max(0.0)
            .sqrt() as f32,
    }
}

pub(super) fn scene_difference(
    before: &[u8],
    after: &[u8],
    width: u32,
    height: u32,
) -> FrameDifference {
    rectangle_difference(
        before,
        after,
        width,
        width / 4..width * 3 / 4,
        height / 4..height * 3 / 4,
    )
}

pub(super) fn full_frame_difference(
    before: &[u8],
    after: &[u8],
    width: u32,
    height: u32,
) -> FrameDifference {
    rectangle_difference(before, after, width, 0..width, 0..height)
}

fn rectangle_difference(
    before: &[u8],
    after: &[u8],
    width: u32,
    columns: std::ops::Range<u32>,
    rows: std::ops::Range<u32>,
) -> FrameDifference {
    let mut total = 0.0_f64;
    let mut count = 0_usize;
    let mut changed = 0_usize;
    for y in rows {
        for x in columns.clone() {
            let offset = ((y * width + x) * 4) as usize;
            let differences: [u8; 3] = std::array::from_fn(|channel| {
                before[offset + channel].abs_diff(after[offset + channel])
            });
            total += differences.into_iter().map(f64::from).sum::<f64>() / (3.0 * 255.0);
            changed += usize::from(differences.into_iter().any(|difference| difference > 2));
            count += 1;
        }
    }
    FrameDifference {
        mean_absolute_error: (total / count.max(1) as f64) as f32,
        changed_fraction: changed as f32 / count.max(1) as f32,
    }
}

pub(super) fn scene_color(pixels: &[u8], width: u32, height: u32) -> [f32; 3] {
    let mut total = [0.0_f64; 3];
    let mut count = 0_usize;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let offset = ((y * width + x) * 4) as usize;
            for (channel, total) in total.iter_mut().enumerate() {
                *total += f64::from(pixels[offset + channel]) / 255.0;
            }
            count += 1;
        }
    }
    total.map(|total| (total / count.max(1) as f64) as f32)
}
