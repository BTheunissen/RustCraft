//! Two stable depth-only sunlight passes; off-camera chunks can still be casters.

use super::gpu_mesh::GpuChunk;
use super::pipelines::{WORLD_ATTRIBUTES, depth_state};
use crate::camera::chunk_visible;
use crate::lighting::shadows::{SHADOW_CASCADES, SHADOW_RESOLUTION, ShadowCoverage};
use crate::mesh::Vertex;
use crate::world::ChunkCoord;
use std::collections::HashMap;

pub(super) struct ShadowResources {
    pub(super) view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    layers: [wgpu::TextureView; SHADOW_CASCADES],
    buffers: [wgpu::Buffer; SHADOW_CASCADES],
    groups: [wgpu::BindGroup; SHADOW_CASCADES],
    pipeline: wgpu::RenderPipeline,
}

impl ShadowResources {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cascaded sunlight depth"),
            size: wgpu::Extent3d {
                width: SHADOW_RESOLUTION,
                height: SHADOW_RESOLUTION,
                depth_or_array_layers: SHADOW_CASCADES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = std::array::from_fn(|index| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: index as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("filtered sunlight comparisons"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sunlight depth camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });
        let buffers = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sunlight cascade matrix"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let groups = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sunlight cascade"),
                layout: &bind_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers[index].as_entire_binding(),
                }],
            })
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sunlight depth layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sunlight depth shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow.wgsl").into()),
        });
        let mut depth = depth_state(true, wgpu::CompareFunction::LessEqual);
        depth.bias = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 2.0,
            clamp: 0.0,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opaque sunlight casters"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &WORLD_ATTRIBUTES,
                })],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            view,
            sampler,
            layers,
            buffers,
            groups,
            pipeline,
        }
    }

    pub(super) fn prepare(&self, queue: &wgpu::Queue, coverage: ShadowCoverage) {
        for (buffer, matrix) in self.buffers.iter().zip(coverage.matrices) {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&matrix.to_cols_array()));
        }
    }

    pub(super) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        chunks: &HashMap<ChunkCoord, GpuChunk>,
        coverage: ShadowCoverage,
    ) {
        for (index, matrix) in coverage.matrices.into_iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sunlight cascade depth"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.layers[index],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.groups[index], &[]);
            for (&coord, chunk) in chunks {
                if chunk_visible(matrix, coord)
                    && let Some(mesh) = &chunk.opaque
                {
                    mesh.draw(&mut pass);
                }
            }
        }
    }
}
