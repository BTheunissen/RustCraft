//! Survival-specific vertex layouts and depth/blend state.

use super::geometry::{BlockInstance, CubeVertex};
use crate::renderer::pipelines::{depth_state, scene_targets};

pub(super) const CUBE_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Uint32];
pub(super) const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Uint32x4, 9 => Float32];

#[derive(Clone, Copy)]
pub(super) enum PassKind {
    Pickup,
    Target,
    Hand,
}

impl PassKind {
    pub(super) fn depth(self) -> wgpu::DepthStencilState {
        depth_state(
            !matches!(self, Self::Target),
            wgpu::CompareFunction::LessEqual,
        )
    }

    pub(super) fn blend(self) -> Option<wgpu::BlendState> {
        matches!(self, Self::Target).then_some(wgpu::BlendState::ALPHA_BLENDING)
    }
}

pub(super) fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    kind: PassKind,
) -> wgpu::RenderPipeline {
    let (label, vertex, fragment) = match kind {
        PassKind::Pickup => ("textured world pickups", "vs_pickup", "fs_world"),
        PassKind::Target => (
            "target outline and mining fractures",
            "vs_target",
            "fs_target",
        ),
        PassKind::Hand => ("first-person hand and held block", "vs_hand", "fs_hand"),
    };
    let cube = wgpu::VertexBufferLayout {
        array_stride: size_of::<CubeVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &CUBE_ATTRIBUTES,
    };
    let instances = wgpu::VertexBufferLayout {
        array_stride: size_of::<BlockInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRIBUTES,
    };
    let buffers = match kind {
        PassKind::Target => vec![Some(cube)],
        _ => vec![Some(cube), Some(instances)],
    };
    let targets = match kind {
        PassKind::Hand => vec![Some(format.into())],
        PassKind::Pickup => scene_targets(format, None, true).to_vec(),
        PassKind::Target => scene_targets(format, kind.blend(), false).to_vec(),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(kind.depth()),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
