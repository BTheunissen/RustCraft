//! Frame staging, resident-only daylight, and three small survival draw calls.

use super::geometry::CUBE_VERTICES;
use super::resources::SurvivalResources;
use crate::renderer::FrameState;

impl SurvivalResources {
    pub(in crate::renderer) fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        frame: &FrameState<'_>,
        aspect: f32,
    ) {
        let uniform = self.geometry.stage(frame, aspect);
        if self.geometry.hand_count > 0 {
            queue.write_buffer(
                &self.hand_buffer,
                0,
                bytemuck::cast_slice(&self.geometry.hand[..self.geometry.hand_count as usize]),
            );
        }
        if !self.geometry.pickups.is_empty() {
            queue.write_buffer(
                &self.pickup_buffer,
                0,
                bytemuck::cast_slice(&self.geometry.pickups),
            );
        }
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub(in crate::renderer) fn draw_pickups(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.geometry.pickups.is_empty() {
            pass.set_pipeline(&self.pickup_pipeline);
            pass.set_bind_group(1, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.cube.slice(..));
            pass.set_vertex_buffer(1, self.pickup_buffer.slice(..));
            pass.draw(
                0..CUBE_VERTICES as u32,
                0..self.geometry.pickups.len() as u32,
            );
        }
    }

    pub(in crate::renderer) fn draw_target(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.geometry.has_target {
            pass.set_pipeline(&self.target_pipeline);
            pass.set_bind_group(1, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.cube.slice(..));
            pass.draw(0..CUBE_VERTICES as u32, 0..1);
        }
    }

    pub(in crate::renderer) fn encode_hand(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera: &wgpu::BindGroup,
    ) {
        if self.geometry.hand_count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("first-person overlay with independent cleared depth"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.hand_pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.cube.slice(..));
        pass.set_vertex_buffer(1, self.hand_buffer.slice(..));
        pass.draw(0..CUBE_VERTICES as u32, 0..self.geometry.hand_count);
    }
}
