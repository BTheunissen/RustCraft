//! Bounded probe transport and shared shadow/voxel resources, with CPU-only caching.

use super::FrameState;
use super::shadows::ShadowResources;
use crate::lighting::hdr::Exposure;
use crate::lighting::materials::material_tables;
use crate::lighting::shadows::{SHADOW_RESOLUTION, ShadowCoverage, shadow_coverage};
use crate::lighting::transport::{IrradianceProbe, ProbeSchedule};
use crate::lighting::volume::{
    LIGHT_VOLUME_SIZE, LightVolume, PROBE_COUNT, PROBE_GRID, PROBE_SPACING, VolumeUpdate,
};
use crate::lighting::{CelestialLight, refresh_probes, solar_probe_budget};
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LightingFeatures {
    pub(crate) shadows: bool,
    pub(crate) indirect: bool,
    pub(crate) emission: bool,
    pub(crate) bounce: bool,
    pub(crate) bloom: bool,
    pub(crate) volumetrics: bool,
    pub(crate) volumetric_reference: bool,
    pub(crate) texture_filtering: bool,
}

impl Default for LightingFeatures {
    fn default() -> Self {
        Self {
            shadows: true,
            indirect: true,
            emission: true,
            bounce: true,
            bloom: true,
            volumetrics: true,
            volumetric_reference: false,
            texture_filtering: true,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct LightingUniform {
    matrices: [[[f32; 4]; 4]; 2],
    eye: [f32; 4],
    shadow_settings: [f32; 4],
    origin: [i32; 4],
    previous_origin: [i32; 4],
    probe_settings: [f32; 4],
    controls: [u32; 4],
    features: [f32; 4],
    sun_direction: [f32; 4],
    sun_radiance: [f32; 4],
    sky_irradiance: [f32; 4],
    albedo: [[f32; 4]; 32],
    emission: [[f32; 4]; 32],
}

pub(super) struct LightingResources {
    pub(super) uniform: wgpu::Buffer,
    pub(super) shadows: ShadowResources,
    pub(super) voxels: wgpu::Texture,
    pub(super) voxel_view: wgpu::TextureView,
    pub(super) probes: wgpu::Buffer,
    pub(super) coverage: ShadowCoverage,
    volume: LightVolume,
    compute: wgpu::ComputePipeline,
    compute_group: wgpu::BindGroup,
    schedule: ProbeSchedule,
    reset_probes: Vec<u8>,
    dispatch_count: u32,
    exposure: Exposure,
    previous_hour: Option<f64>,
}

impl LightingResources {
    pub(super) fn timing_state(&self) -> (ProbeSchedule, Exposure, Option<f64>) {
        (self.schedule, self.exposure, self.previous_hour)
    }

    pub(super) fn restore_timing(
        &mut self,
        (schedule, exposure, hour): (ProbeSchedule, Exposure, Option<f64>),
    ) {
        self.schedule = schedule;
        self.exposure = exposure;
        self.previous_hour = hour;
    }

    pub(super) fn origin(&self) -> [i32; 3] {
        self.volume.origin()
    }

    pub(super) fn new(device: &wgpu::Device) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("unified lighting uniform"),
            size: size_of::<LightingUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadows = ShadowResources::new(device);
        let voxels = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("resident materials and vertical sky transmission"),
            size: wgpu::Extent3d {
                width: LIGHT_VOLUME_SIZE as u32,
                height: LIGHT_VOLUME_SIZE as u32,
                depth_or_array_layers: LIGHT_VOLUME_SIZE as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rg8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let voxel_view = voxels.create_view(&Default::default());
        let probes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visibility-aware directional irradiance probes"),
            size: u64::from(PROBE_COUNT) * size_of::<IrradianceProbe>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("probe compute resources"),
            entries: &lighting_entries(true),
        });
        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bounded probe transport"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&shadows.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&shadows.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&voxel_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: probes.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("probe compute layout"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let source = format!(
            "{}\n{}",
            include_str!("../shaders/lighting.wgsl").replace(
                "var<storage, read> irradiance",
                "var<storage, read_write> irradiance"
            ),
            include_str!("../shaders/probes.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("one-bounce voxel light transport"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fixed-budget irradiance update"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("update_probes"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            uniform,
            shadows,
            voxels,
            voxel_view,
            probes,
            coverage: shadow_coverage(glam::Vec3::ZERO, 8, CelestialLight::default().sun_direction),
            volume: LightVolume::default(),
            compute,
            compute_group,
            schedule: ProbeSchedule::default(),
            reset_probes: vec![0; PROBE_COUNT as usize * size_of::<IrradianceProbe>()],
            dispatch_count: 0,
            exposure: Exposure::default(),
            previous_hour: None,
        }
    }

    pub(super) fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        frame: &FrameState<'_>,
        deterministic: bool,
        features: LightingFeatures,
    ) -> f32 {
        let eye = frame.player.eye_position();
        let celestial = CelestialLight::sample(frame.time_of_day);
        let solar_refresh = refresh_probes(self.previous_hour, frame.time_of_day);
        let budget = solar_probe_budget(self.previous_hour, frame.time_of_day);
        self.previous_hour = Some(frame.time_of_day);
        let update = self.volume.update(frame.world, eye);
        if update != VolumeUpdate::Unchanged {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.voxels,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                self.volume.bytes(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(LIGHT_VOLUME_SIZE as u32 * 2),
                    rows_per_image: Some(LIGHT_VOLUME_SIZE as u32),
                },
                self.voxels.size(),
            );
        }
        self.coverage = shadow_coverage(eye, frame.render_distance, celestial.sun_direction);
        self.shadows.prepare(queue, self.coverage);
        let [x, y, z] = self.volume.origin();
        let invalidated = update == VolumeUpdate::Rebuild || solar_refresh;
        let [start, scheduled_count] = if budget == crate::lighting::volume::PROBES_PER_FRAME {
            self.schedule.next(invalidated, deterministic)
        } else {
            self.schedule
                .next_with_budget(invalidated, deterministic, budget)
        };
        let (previous_origin, count) = match update {
            VolumeUpdate::Scroll { previous_origin } if !deterministic && !solar_refresh => (
                [
                    previous_origin[0],
                    previous_origin[1],
                    previous_origin[2],
                    1,
                ],
                PROBE_COUNT,
            ),
            _ => ([0; 4], scheduled_count),
        };
        let (albedo, emission) = material_tables();
        let uniform = LightingUniform {
            matrices: self
                .coverage
                .matrices
                .map(|matrix| matrix.to_cols_array_2d()),
            eye: eye.extend(0.0).to_array(),
            shadow_settings: [
                self.coverage.radii[0],
                self.coverage.radii[1] - 20.0,
                self.coverage.radii[1] - 8.0,
                1.0 / SHADOW_RESOLUTION as f32,
            ],
            origin: [x, y, z, LIGHT_VOLUME_SIZE],
            previous_origin,
            probe_settings: [PROBE_SPACING as f32, PROBE_GRID as f32, 40.0, budget as f32],
            controls: [start, count, PROBE_GRID, PROBE_COUNT],
            features: [
                features.shadows as u8 as f32,
                features.indirect as u8 as f32,
                features.emission as u8 as f32,
                features.bounce as u8 as f32,
            ],
            sun_direction: celestial.sun_direction.extend(0.0).to_array(),
            sun_radiance: celestial.sun_radiance.extend(0.0).to_array(),
            sky_irradiance: celestial.sky_irradiance.extend(0.0).to_array(),
            albedo,
            emission,
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        self.dispatch_count = count;
        self.exposure.update(
            frame.sky_exposure * celestial.daylight,
            frame.elapsed,
            deterministic,
        )
    }

    pub(super) fn encode_probes(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("bounded single-bounce sky and emissive irradiance"),
            ..Default::default()
        });
        pass.set_pipeline(&self.compute);
        pass.set_bind_group(0, &self.compute_group, &[]);
        pass.dispatch_workgroups(self.dispatch_count.div_ceil(64), 1, 1);
    }

    pub(super) fn reset_probes(&mut self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.probes, 0, &self.reset_probes);
        self.schedule = ProbeSchedule::default();
    }
}

pub(super) fn lighting_entries(compute: bool) -> Vec<wgpu::BindGroupLayoutEntry> {
    let visibility = if compute {
        wgpu::ShaderStages::COMPUTE
    } else {
        wgpu::ShaderStages::FRAGMENT
    };
    [
        (
            4,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size_of::<LightingUniform>() as u64),
            },
        ),
        (
            5,
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
        ),
        (
            6,
            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
        ),
        (
            7,
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Uint,
                view_dimension: wgpu::TextureViewDimension::D3,
                multisampled: false,
            },
        ),
        (
            8,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: !compute,
                },
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    u64::from(PROBE_COUNT) * size_of::<IrradianceProbe>() as u64,
                ),
            },
        ),
    ]
    .into_iter()
    .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    })
    .collect()
}

pub(super) fn world_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        crate::lighting::atmosphere::shader_constants(),
        include_str!("../shaders/camera.wgsl"),
        include_str!("../shaders/lighting.wgsl"),
        include_str!("../shaders/atmosphere.wgsl"),
        include_str!("../shaders/world.wgsl")
    )
}

#[cfg(test)]
mod tests;
