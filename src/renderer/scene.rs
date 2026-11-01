//! Per-frame uploads and scene/HUD pass encoding.

use super::pipelines::hud_buffer;
use super::{CameraUniform, FrameState, Renderer, chunk_distance_squared, phase_chunk_near};
use crate::camera::{chunk_visible, view_projection};
use crate::phase::PHASE_SIZE;
use crate::phase::PhaseCache;
use crate::world::{CHUNK_SIZE, WorldView};

impl Renderer {
    pub(super) fn prepare_frame(&mut self, frame: &FrameState<'_>) {
        let exposure = self.resources.lighting.prepare(
            &self.queue,
            frame,
            self.diagnostic_deadline.is_some() && !self.progressive_lighting,
            self.lighting_features,
        );
        self.postprocess.prepare(
            &self.queue,
            exposure,
            self.lighting_features.bloom,
            self.config.format.is_srgb(),
        );
        let eye = frame.player.eye_position();
        let phasing = PhaseCache::phasing(frame.world, eye, frame.player.flying);
        if phasing && self.phase.update(frame.world, eye) {
            let volume = self.phase.volume();
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.resources.phase_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                volume.bytes(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(PHASE_SIZE as u32),
                    rows_per_image: Some(PHASE_SIZE as u32),
                },
                self.resources.phase_texture.size(),
            );
        }
        let mut uniform = CameraUniform::from_frame(
            frame,
            self.config.width as f32 / self.config.height as f32,
            self.phase.volume().origin(),
            phasing,
            self.lighting_features.volumetrics,
            |x, y, z| frame.world.block_at(x, y, z),
        );
        if uniform.atmosphere[3] > 0.5 && self.lighting_features.volumetric_reference {
            uniform.atmosphere[3] = 2.0;
        }
        uniform.environment[3] = self.lighting_features.texture_filtering as u8 as f32;
        if self.diagnostic_sample_offset != [0.0; 2] {
            let [x, y] = self.diagnostic_sample_offset;
            let shift = glam::Mat4::from_translation(glam::Vec3::new(
                x * 2.0 / self.config.width as f32,
                -y * 2.0 / self.config.height as f32,
                0.0,
            ));
            let matrix = shift * glam::Mat4::from_cols_array_2d(&uniform.view_projection);
            uniform.view_projection = matrix.to_cols_array_2d();
            uniform.inverse_ray_projection =
                (glam::Mat4::from_cols_array_2d(&uniform.inverse_ray_projection) * shift.inverse())
                    .to_cols_array_2d();
        }
        self.queue.write_buffer(
            &self.resources.camera_buffer,
            0,
            bytemuck::bytes_of(&uniform),
        );
        let hud_bytes = bytemuck::cast_slice(frame.hud);
        if hud_bytes.len() as u64 > self.resources.hud_capacity {
            self.resources.hud_capacity = (hud_bytes.len() as u64).next_power_of_two();
            self.resources.hud_buffer = hud_buffer(&self.device, self.resources.hud_capacity);
        }
        if !hud_bytes.is_empty() {
            self.queue
                .write_buffer(&self.resources.hud_buffer, 0, hud_bytes);
        }
        self.resources.survival.prepare(
            &self.queue,
            frame,
            self.config.width as f32 / self.config.height as f32,
        );
    }

    pub(super) fn encode_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        frame: &FrameState<'_>,
        draw_water: bool,
    ) {
        self.resources.lighting.shadows.encode(
            encoder,
            &self.chunks,
            self.resources.lighting.coverage,
        );
        self.resources.lighting.encode_probes(encoder);
        let eye = frame.player.eye_position();
        let phasing = PhaseCache::phasing(frame.world, eye, frame.player.flying);
        let matrix = view_projection(
            eye,
            frame.player.look_direction(),
            self.config.width as f32 / self.config.height as f32,
            frame.render_distance as f32 * CHUNK_SIZE as f32 * 2.0 + 64.0,
        );
        let mut visible: Vec<_> = self
            .chunks
            .iter()
            .filter(|(coord, _)| chunk_visible(matrix, **coord))
            .collect();
        visible.sort_by(|(a, _), (b, _)| {
            chunk_distance_squared(**a, eye)
                .total_cmp(&chunk_distance_squared(**b, eye))
                .then_with(|| a.cmp(b))
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sky and voxel world"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.postprocess.scene,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.volumetrics.guide,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.resources.camera_bind_group, &[]);
            pass.set_pipeline(&self.resources.sky_pipeline);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.resources.opaque_pipeline);
            for (_, chunk) in &visible {
                if let Some(mesh) = &chunk.opaque {
                    mesh.draw(&mut pass);
                }
            }
            if phasing {
                pass.set_pipeline(&self.resources.phase_back_pipeline);
                for (coord, chunk) in visible.iter().rev() {
                    if phase_chunk_near(**coord, eye)
                        && let Some(mesh) = &chunk.opaque
                    {
                        mesh.draw(&mut pass);
                    }
                }
            }
            self.resources.survival.draw_pickups(&mut pass);
            if draw_water {
                pass.set_pipeline(&self.resources.water_pipeline);
                for (_, chunk) in visible.iter().rev() {
                    if let Some(mesh) = &chunk.water {
                        mesh.draw(&mut pass);
                    }
                }
            }
            if phasing {
                pass.set_pipeline(&self.resources.phase_scan_pipeline);
                pass.draw(0..3, 0..1);
            }
            self.resources.survival.draw_target(&mut pass);
        }
        self.volumetrics.encode(encoder, &self.postprocess.scene);
        self.resources.survival.encode_hand(
            encoder,
            &self.postprocess.scene,
            &self.hand_depth,
            &self.resources.camera_bind_group,
        );
        self.postprocess.encode(encoder, color);
        if !frame.hud.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.resources.hud_pipeline);
            pass.set_vertex_buffer(0, self.resources.hud_buffer.slice(..));
            pass.draw(0..frame.hud.len() as u32, 0..1);
        }
    }
}
