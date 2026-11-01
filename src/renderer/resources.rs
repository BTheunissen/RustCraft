//! Scene resource ownership and initialization.

use super::frame::CameraUniform;
use super::lighting::{LightingResources, lighting_entries, world_shader_source};
use super::pipelines::{
    hud_buffer, hud_pipeline, phase_back_pipeline, phase_scan_pipeline, sky_pipeline,
    world_pipeline,
};
use super::postprocess::HDR_FORMAT;
use super::survival::SurvivalResources;
use crate::atlas::{TILE_COUNT, TILE_SIZE, generate_atlas};
use crate::phase::PHASE_SIZE;
use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

pub(super) struct SceneResources {
    pub(super) camera_buffer: wgpu::Buffer,
    pub(super) camera_bind_group: wgpu::BindGroup,
    pub(super) sky_pipeline: wgpu::RenderPipeline,
    pub(super) opaque_pipeline: wgpu::RenderPipeline,
    pub(super) water_pipeline: wgpu::RenderPipeline,
    pub(super) phase_back_pipeline: wgpu::RenderPipeline,
    pub(super) phase_scan_pipeline: wgpu::RenderPipeline,
    pub(super) phase_texture: wgpu::Texture,
    pub(super) hud_pipeline: wgpu::RenderPipeline,
    pub(super) hud_buffer: wgpu::Buffer,
    pub(super) hud_capacity: u64,
    pub(super) survival: SurvivalResources,
    pub(super) lighting: LightingResources,
}

impl SceneResources {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform"),
            contents: bytemuck::bytes_of(&CameraUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let atlas = generate_atlas();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("isolated pixel-art tiles with linear-light mipmaps"),
            size: wgpu::Extent3d {
                width: TILE_SIZE,
                height: TILE_SIZE,
                depth_or_array_layers: TILE_COUNT,
            },
            mip_level_count: TILE_SIZE.ilog2() + 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, mip) in atlas.mipmaps().into_iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas_texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mip.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.size * 4),
                    rows_per_image: Some(mip.size),
                },
                wgpu::Extent3d {
                    width: mip.size,
                    height: mip.size,
                    depth_or_array_layers: TILE_COUNT,
                },
            );
        }
        let atlas_view = atlas_texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("anisotropic minification; magnified art uses exact texel loads"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let phase_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bounded creative cutaway voxels"),
            size: wgpu::Extent3d {
                width: PHASE_SIZE as u32,
                height: PHASE_SIZE as u32,
                depth_or_array_layers: PHASE_SIZE as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let phase_view = phase_texture.create_view(&Default::default());
        let lighting = LightingResources::new(device);
        let mut entries = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<CameraUniform>() as u64),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
        ];
        entries.extend(lighting_entries(false));
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera, atlas, and unified lighting layout"),
            entries: &entries,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera and atlas"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&phase_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: lighting.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&lighting.shadows.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&lighting.shadows.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&lighting.voxel_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: lighting.probes.as_entire_binding(),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world pipeline layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel and sky shader"),
            source: wgpu::ShaderSource::Wgsl(world_shader_source().into()),
        });
        let opaque_pipeline = world_pipeline(device, &shader, &layout, HDR_FORMAT, false);
        let water_pipeline = world_pipeline(device, &shader, &layout, HDR_FORMAT, true);
        let phase_back_pipeline = phase_back_pipeline(device, &shader, &layout, HDR_FORMAT);
        let phase_scan_pipeline = phase_scan_pipeline(device, &shader, &layout, HDR_FORMAT);
        let hud_pipeline = hud_pipeline(device, format);
        let hud_capacity = 4096;
        let hud_buffer = hud_buffer(device, hud_capacity);
        let sky_pipeline = sky_pipeline(device, &shader, &layout, HDR_FORMAT);
        let survival = SurvivalResources::new(device, &bind_layout, HDR_FORMAT);
        Self {
            camera_buffer,
            camera_bind_group,
            sky_pipeline,
            opaque_pipeline,
            water_pipeline,
            phase_back_pipeline,
            phase_scan_pipeline,
            phase_texture,
            hud_pipeline,
            hud_buffer,
            hud_capacity,
            survival,
            lighting,
        }
    }
}
