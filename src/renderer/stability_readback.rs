//! Movement diagnostics preserve the interactive cache around independent reference renders.

mod air;
mod textures;

use super::readback::scene_difference;
use super::{FrameDifference, FrameState, RenderResult, Renderer};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

impl Renderer {
    pub(crate) fn light_volume_origin(&self) -> [i32; 3] {
        self.resources.lighting.origin()
    }

    pub(crate) fn with_interactive_lighting<T>(
        &mut self,
        render: impl FnOnce(&mut Self) -> RenderResult<T>,
    ) -> RenderResult<T> {
        let saved = self.progressive_lighting;
        self.progressive_lighting = true;
        let result = render(self);
        self.progressive_lighting = saved;
        result
    }

    pub(crate) fn read_interactive_frame(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<Vec<u8>> {
        self.with_interactive_lighting(|renderer| {
            renderer.readback_frame(frame).map(|(pixels, _)| pixels)
        })
    }

    pub(crate) fn compare_interactive_frame(
        &mut self,
        frame: &FrameState<'_>,
    ) -> RenderResult<(Vec<u8>, FrameDifference)> {
        let pixels = self.read_interactive_frame(frame)?;
        let probes = &self.resources.lighting.probes;
        let backup = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("interactive probe cache isolated from movement reference"),
            size: probes.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(probes, 0, &backup, 0, probes.size());
        self.queue.submit(Some(encoder.finish()));
        let timing = self.resources.lighting.timing_state();
        let saved = self.progressive_lighting;
        self.progressive_lighting = false;
        let reference = self.readback_frame(frame);
        self.progressive_lighting = saved;
        self.resources.lighting.restore_timing(timing);
        let probes = &self.resources.lighting.probes;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&backup, 0, probes, 0, probes.size());
        self.queue.submit(Some(encoder.finish()));
        let (reference, _) = reference?;
        let difference =
            scene_difference(&pixels, &reference, self.config.width, self.config.height);
        Ok((pixels, difference))
    }

    pub(crate) fn save_motion_capture(
        &self,
        path: &Path,
        frames: &[Vec<u8>],
        frames_per_second: u16,
    ) -> RenderResult<()> {
        let writer = BufWriter::new(File::create(path)?);
        write_animation(
            writer,
            frames,
            self.config.width,
            self.config.height,
            frames_per_second,
        )?;
        println!(
            "Motion capture: {} ({} GPU frames)",
            path.display(),
            frames.len()
        );
        Ok(())
    }
}

fn write_animation(
    writer: impl std::io::Write,
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    frames_per_second: u16,
) -> RenderResult<()> {
    if frames.is_empty() || frames_per_second == 0 {
        return Err(
            std::io::Error::other("motion capture requires frames and a positive rate").into(),
        );
    }
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_animated(u32::try_from(frames.len())?, 0)?;
    encoder.set_frame_delay(1, frames_per_second)?;
    let mut writer = encoder.write_header()?;
    for pixels in frames {
        writer.write_image_data(pixels)?;
    }
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_capture_keeps_real_frames_and_their_timing() {
        let frames = vec![vec![15; 4 * 3 * 2], vec![180; 4 * 3 * 2]];
        let mut bytes = Vec::new();
        write_animation(&mut bytes, &frames, 3, 2, 30).unwrap();
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert_eq!(decoder.info().animation_control.unwrap().num_frames, 2);
        let mut actual = vec![0; decoder.output_buffer_size().unwrap()];
        for expected in frames {
            decoder.next_frame(&mut actual).unwrap();
            assert_eq!(actual, expected);
            let timing = decoder.info().frame_control.unwrap();
            assert_eq!((timing.delay_num, timing.delay_den), (1, 30));
        }
    }

    #[test]
    fn motion_capture_rejects_missing_frames_bad_sizes_and_zero_rate() {
        assert!(write_animation(Vec::new(), &[], 3, 2, 30).is_err());
        assert!(write_animation(Vec::new(), &[vec![0; 24]], 3, 2, 0).is_err());
        assert!(write_animation(Vec::new(), &[vec![0; 8]], 3, 2, 30).is_err());
    }
}
