//! Fixed-capacity GPU allocations and reused CPU staging for survival presentation.

use super::geometry::{BlockInstance, HAND_INSTANCES, MAX_PICKUPS, SurvivalUniform, cube_vertices};
use super::pipelines::{PassKind, pipeline};
use super::shader_source;
use super::staging::SurvivalGeometry;
use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

pub(in crate::renderer) struct SurvivalResources {
    pub(super) cube: wgpu::Buffer,
    pub(super) pickup_buffer: wgpu::Buffer,
    pub(super) hand_buffer: wgpu::Buffer,
    pub(super) uniform_buffer: wgpu::Buffer,
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) pickup_pipeline: wgpu::RenderPipeline,
    pub(super) target_pipeline: wgpu::RenderPipeline,
    pub(super) hand_pipeline: wgpu::RenderPipeline,
    pub(super) geometry: SurvivalGeometry,
}

impl SurvivalResources {
    pub(in crate::renderer) fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        let cube = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shared survival cube"),
            contents: bytemuck::cast_slice(&cube_vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let pickup_buffer = instance_buffer(device, "bounded pickup instances", MAX_PICKUPS);
        let hand_buffer = instance_buffer(device, "first-person instances", HAND_INSTANCES);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("target and first-person uniform"),
            contents: bytemuck::bytes_of(&SurvivalUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("survival presentation layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<SurvivalUniform>() as u64),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("survival presentation"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("survival pipelines"),
            bind_group_layouts: &[Some(camera_layout), Some(&bind_layout)],
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("survival shader sharing world lighting and atlas"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        Self {
            cube,
            pickup_buffer,
            hand_buffer,
            uniform_buffer,
            bind_group,
            pickup_pipeline: pipeline(device, &shader, &layout, format, PassKind::Pickup),
            target_pipeline: pipeline(device, &shader, &layout, format, PassKind::Target),
            hand_pipeline: pipeline(device, &shader, &layout, format, PassKind::Hand),
            geometry: SurvivalGeometry::default(),
        }
    }
}

fn instance_buffer(device: &wgpu::Device, label: &str, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count * size_of::<BlockInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
