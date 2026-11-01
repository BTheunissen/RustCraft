//! Half-resolution shadowed air and depth-guided HDR composition; no history or second fog.

mod resources;

use super::SceneResources;
use crate::lighting::atmosphere::half_resolution;

pub(super) struct Volumetrics {
    pub(super) guide: wgpu::TextureView,
    pub(super) guide_texture: wgpu::Texture,
    pub(super) scatter_texture: wgpu::Texture,
    scatter: wgpu::TextureView,
    scene_group: wgpu::BindGroup,
    scatter_group: wgpu::BindGroup,
    march: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
}

impl Volumetrics {
    pub(super) fn new(
        device: &wgpu::Device,
        scene: &SceneResources,
        width: u32,
        height: u32,
    ) -> Self {
        let guide_texture = resources::texture(
            device,
            "full-resolution air depth and fog guide",
            [width.max(1), height.max(1)],
        );
        let guide = guide_texture.create_view(&Default::default());
        let scatter_texture = resources::texture(
            device,
            "half-resolution shadowed sunlight scattering",
            half_resolution(width, height),
        );
        let scatter = scatter_texture.create_view(&Default::default());
        let (scene_layout, scene_group) = resources::scene_inputs(device, scene, &guide);
        let (scatter_layout, scatter_group) = resources::scatter_input(device, &scatter);
        let march_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadowed air march layout"),
            bind_group_layouts: &[Some(&scene_layout)],
            ..Default::default()
        });
        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("depth-guided air composite layout"),
            bind_group_layouts: &[Some(&scene_layout), Some(&scatter_layout)],
            ..Default::default()
        });
        let source = format!(
            "{}\n{}\n{}\n{}\n{}",
            crate::lighting::atmosphere::shader_constants(),
            include_str!("../shaders/camera.wgsl"),
            include_str!("../shaders/lighting.wgsl"),
            include_str!("../shaders/atmosphere.wgsl"),
            include_str!("../shaders/volumetrics.wgsl"),
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadowed volumetric sunlight and bilateral upscale"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        Self {
            guide,
            guide_texture,
            scatter_texture,
            scatter,
            scene_group,
            scatter_group,
            march: resources::pipeline(device, &shader, &march_layout, "fs_march_air", false),
            composite: resources::pipeline(
                device,
                &shader,
                &composite_layout,
                "fs_composite_air",
                true,
            ),
        }
    }

    pub(super) fn encode(&self, encoder: &mut wgpu::CommandEncoder, hdr: &wgpu::TextureView) {
        for (view, pipeline, composite) in [
            (&self.scatter, &self.march, false),
            (hdr, &self.composite, true),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if composite {
                    "depth-aware sunlight into HDR before bloom"
                } else {
                    "half-resolution shadowed sunlight march"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if composite {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.scene_group, &[]);
            if composite {
                pass.set_bind_group(1, &self.scatter_group, &[]);
            }
            pass.draw(0..3, 0..1);
        }
    }
}
