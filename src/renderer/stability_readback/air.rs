//! Moving air is isolated in linear HDR by cancelling the same-pose surface rendering.

use super::super::hdr_readback::half_value;
use super::super::{FrameState, LightingFeatures, RenderResult, Renderer};

impl Renderer {
    pub(crate) fn measure_air_motion(
        &mut self,
        first: &FrameState<'_>,
        second: &FrameState<'_>,
    ) -> RenderResult<f32> {
        self.with_lighting_features(
            LightingFeatures {
                indirect: false,
                bloom: false,
                volumetrics: true,
                ..self.lighting_features
            },
            |renderer| {
                let first = FrameState {
                    hud: &[],
                    survival: None,
                    headlamp: false,
                    ..*first
                };
                let second = FrameState {
                    hud: &[],
                    survival: None,
                    headlamp: false,
                    ..*second
                };
                let before = isolated_air(renderer, &first)?;
                let after = isolated_air(renderer, &second)?;
                central_difference(
                    &before,
                    &after,
                    renderer.config.width,
                    renderer.config.height,
                )
            },
        )
    }
}

fn isolated_air(renderer: &mut Renderer, frame: &FrameState<'_>) -> RenderResult<Vec<[f32; 3]>> {
    renderer.render_offscreen(frame)?;
    let (shadowed, stride) = renderer.read_hdr_texture(&renderer.postprocess.scene_texture)?;
    renderer.lighting_features.volumetrics = false;
    let result = (|| {
        renderer.render_offscreen(frame)?;
        let (ambient, _) = renderer.read_hdr_texture(&renderer.postprocess.scene_texture)?;
        let mut pixels =
            Vec::with_capacity((renderer.config.width * renderer.config.height) as usize);
        for y in 0..renderer.config.height {
            for x in 0..renderer.config.width {
                let offset = (y * stride + x * 8) as usize;
                let channels = std::array::from_fn(|channel| {
                    let index = offset + channel * 2;
                    half_value(u16::from_le_bytes([shadowed[index], shadowed[index + 1]]))
                        - half_value(u16::from_le_bytes([ambient[index], ambient[index + 1]]))
                });
                if !channels.into_iter().all(f32::is_finite) {
                    return Err(
                        std::io::Error::other("moving air contains invalid HDR radiance").into(),
                    );
                }
                pixels.push(channels);
            }
        }
        Ok(pixels)
    })();
    renderer.lighting_features.volumetrics = true;
    result
}

fn central_difference(
    before: &[[f32; 3]],
    after: &[[f32; 3]],
    width: u32,
    height: u32,
) -> RenderResult<f32> {
    let mut total = 0.0_f64;
    let mut count = 0;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let index = (y * width + x) as usize;
            for channel in 0..3 {
                let delta = (before[index][channel] - after[index][channel]).abs();
                if !delta.is_finite() {
                    return Err(std::io::Error::other("moving air comparison is nonfinite").into());
                }
                total += f64::from(delta);
                count += 1;
            }
        }
    }
    Ok((total / count.max(1) as f64) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_air_evidence_uses_linear_channels_and_ignores_outer_frame() {
        let before = vec![[0.0; 3]; 64];
        let mut after = before.clone();
        after[..8].fill([5.0; 3]);
        assert_eq!(central_difference(&before, &after, 8, 8).unwrap(), 0.0);
        after[3 * 8 + 3] = [0.01, 0.02, 0.03];
        assert!(
            (central_difference(&before, &after, 8, 8).unwrap() - 0.02 / 16.0).abs() < 0.000001
        );
        after[3 * 8 + 3][0] = f32::NAN;
        assert!(central_difference(&before, &after, 8, 8).is_err());
    }
}
