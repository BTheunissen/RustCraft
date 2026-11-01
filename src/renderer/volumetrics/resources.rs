//! Size-dependent air targets and explicit bindings to the existing sun/voxel resources.

use crate::renderer::SceneResources;
use crate::renderer::frame::CameraUniform;
use crate::renderer::lighting::lighting_entries;
use crate::renderer::postprocess::HDR_FORMAT;

pub(super) fn texture(
    device: &wgpu::Device,
    label: &str,
    [width, height]: [u32; 2],
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn sampled_texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

pub(super) fn scene_inputs(
    device: &wgpu::Device,
    scene: &SceneResources,
    guide: &wgpu::TextureView,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let mut entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size_of::<CameraUniform>() as u64),
            },
            count: None,
        },
        sampled_texture(1),
    ];
    entries.extend(lighting_entries(false));
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("air camera, fog guide, cascades, and conservative resident voxels"),
        entries: &entries,
    });
    let lighting = &scene.lighting;
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shared sunlight resources for air"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: scene.camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(guide),
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
    (layout, group)
}

pub(super) fn scatter_input(
    device: &wgpu::Device,
    scatter: &wgpu::TextureView,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("depth-aware air upscale layout"),
        entries: &[sampled_texture(0)],
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("half-resolution air radiance and ray depth"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(scatter),
        }],
    });
    (layout, group)
}

pub(super) fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &str,
    additive: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_air"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: additive.then_some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                }),
                write_mask: if additive {
                    wgpu::ColorWrites::RED | wgpu::ColorWrites::GREEN | wgpu::ColorWrites::BLUE
                } else {
                    wgpu::ColorWrites::ALL
                },
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
