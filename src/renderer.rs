//! Native GPU/surface adapter; simulation and generation stay outside.

mod air_readback;
mod frame;
mod gpu_mesh;
mod hdr_readback;
mod lighting;
mod pipelines;
mod postprocess;
mod readback;
mod resources;
mod scene;
mod shadows;
mod stability_readback;
mod survival;
mod volumetrics;

#[cfg(test)]
use crate::mesh::Vertex;
use crate::mesh::{ChunkMesh, material_for_face};
use crate::phase::{PHASE_RANGE, PhaseCache};
use crate::world::{Block, CHUNK_SIZE, ChunkCoord};
pub(crate) use air_readback::{AirEdgeStats, AirStats};
use frame::CameraUniform;
#[cfg(test)]
use frame::water_animation_time;
pub use frame::{FrameDifference, FrameState, FrameStats, SurvivalFrame};
use glam::Vec3;
use gpu_mesh::{GpuChunk, GpuMesh};
pub(crate) use lighting::LightingFeatures;
#[cfg(test)]
use pipelines::WORLD_ATTRIBUTES;
use pipelines::depth_view;
use postprocess::PostProcess;
#[cfg(test)]
use readback::{padded_row_bytes, scene_color, scene_difference, scene_stats, unpack_pixels};
use resources::SceneResources;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use volumetrics::Volumetrics;
use winit::dpi::PhysicalSize;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

type RenderResult<T> = Result<T, Box<dyn Error>>;

/// Owns the window surface and GPU resources; no world-generation work runs here.
pub struct Renderer {
    window: Arc<Window>,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    hand_depth: wgpu::TextureView,
    resources: SceneResources,
    postprocess: PostProcess,
    volumetrics: Volumetrics,
    lighting_features: LightingFeatures,
    progressive_lighting: bool,
    diagnostic_sample_offset: [f32; 2],
    phase: PhaseCache,
    chunks: HashMap<ChunkCoord, GpuChunk>,
    active: bool,
    offscreen_color: Option<wgpu::TextureView>,
    gpu_errors: mpsc::Receiver<String>,
    diagnostic_deadline: Option<Instant>,
}

impl Renderer {
    /// Initializes a compatible native GPU and a vsynced window surface.
    pub async fn new(window: Arc<Window>, display: OwnedDisplayHandle) -> RenderResult<Self> {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        let surface = instance.create_surface(Arc::clone(&window))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await?;
        let info = adapter.get_info();
        println!("GPU: {} ({:?})", info.name, info.backend);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RustCraft GPU"),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await?;
        let (errors, gpu_errors) = mpsc::channel();
        let uncaptured_errors = errors.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            let _ = uncaptured_errors.send(format!("GPU error: {error}"));
        }));
        device.set_device_lost_callback(move |reason, message| {
            let _ = errors.send(format!("GPU device lost ({reason:?}): {message}"));
        });

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| io::Error::other("GPU cannot present to this window"))?;
        if let Some(format) = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(wgpu::TextureFormat::is_srgb)
        {
            config.format = format;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        let depth = depth_view(&device, config.width, config.height);
        let hand_depth = depth_view(&device, config.width, config.height);

        let resources = SceneResources::new(&device, &queue, config.format);
        let postprocess = PostProcess::new(&device, config.format, config.width, config.height);
        let volumetrics = Volumetrics::new(&device, &resources, config.width, config.height);
        check_gpu_errors(&gpu_errors)?;
        Ok(Self {
            window,
            instance,
            surface,
            device,
            queue,
            config,
            depth,
            hand_depth,
            resources,
            postprocess,
            volumetrics,
            lighting_features: LightingFeatures::default(),
            progressive_lighting: false,
            diagnostic_sample_offset: [0.0; 2],
            phase: PhaseCache::default(),
            chunks: HashMap::new(),
            active: size.width > 0 && size.height > 0,
            offscreen_color: None,
            gpu_errors,
            diagnostic_deadline: None,
        })
    }

    /// Returns the current nonzero render resolution.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Defers rendering at zero size instead of configuring an invalid surface.
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.active = size.width > 0 && size.height > 0;
        if !self.active {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth = depth_view(&self.device, size.width, size.height);
        self.hand_depth = depth_view(&self.device, size.width, size.height);
        self.postprocess =
            PostProcess::new(&self.device, self.config.format, size.width, size.height);
        self.volumetrics = Volumetrics::new(&self.device, &self.resources, size.width, size.height);
        self.offscreen_color = None;
    }

    /// Uploads worker-prepared geometry; CPU residency remains owned by the game.
    pub fn upload_chunk(&mut self, coord: ChunkCoord, mesh: &ChunkMesh) {
        self.chunks.insert(
            coord,
            GpuChunk {
                opaque: GpuMesh::upload(&self.device, &mesh.opaque),
                water: GpuMesh::upload(&self.device, &mesh.water),
            },
        );
    }

    /// Frees chunks that leave the bounded resident area.
    pub fn remove_chunk(&mut self, coord: ChunkCoord) {
        self.chunks.remove(&coord);
    }

    /// Clears diagnostic scene residency without leaving extra chunks in memory.
    pub fn clear_chunks(&mut self) {
        self.chunks.clear();
    }

    /// Applies the same absolute smoke deadline to every diagnostic GPU wait.
    pub fn set_diagnostic_deadline(&mut self, deadline: Instant) {
        self.diagnostic_deadline = Some(deadline);
    }

    fn wait_timeout(&self) -> RenderResult<Duration> {
        gpu_wait_timeout(self.diagnostic_deadline, Instant::now())
    }

    /// Waits for queued GPU work and surfaces validation/device errors.
    pub fn verify_gpu(&self) -> RenderResult<()> {
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(self.wait_timeout()?),
        })?;
        check_gpu_errors(&self.gpu_errors)
    }

    /// Renders and presents one frame, recovering ordinary surface changes.
    /// Returns false when minimized, occluded, or temporarily unable to acquire a frame.
    pub fn render(&mut self, frame: &FrameState<'_>) -> RenderResult<bool> {
        check_gpu_errors(&self.gpu_errors)?;
        if !self.active {
            return Ok(false);
        }
        let (output, reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output) => (output, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(output) => (output, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(Arc::clone(&self.window))?;
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(io::Error::other("window surface validation failed").into());
            }
        };
        self.prepare_frame(frame);
        let view = output.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode_scene(&mut encoder, &view, &self.depth, frame, true);
        self.queue.submit(Some(encoder.finish()));
        check_gpu_errors(&self.gpu_errors)?;
        self.window.pre_present_notify();
        self.queue.present(output);
        if reconfigure {
            self.surface.configure(&self.device, &self.config);
        }
        Ok(true)
    }

    /// Draws the same shaders and meshes without window presentation.
    /// Used only by explicit offscreen verification, not as an interactive fallback.
    pub fn render_offscreen(&mut self, frame: &FrameState<'_>) -> RenderResult<()> {
        check_gpu_errors(&self.gpu_errors)?;
        self.prepare_frame(frame);
        let view = self
            .offscreen_color
            .get_or_insert_with(|| {
                self.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("offscreen smoke target"),
                        size: wgpu::Extent3d {
                            width: self.config.width,
                            height: self.config.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.config.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            })
            .clone();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode_scene(&mut encoder, &view, &self.depth, frame, true);
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(self.wait_timeout()?),
        })?;
        check_gpu_errors(&self.gpu_errors)?;
        Ok(())
    }
}

fn check_gpu_errors(errors: &mpsc::Receiver<String>) -> RenderResult<()> {
    if let Ok(error) = errors.try_recv() {
        return Err(io::Error::other(error).into());
    }
    Ok(())
}

fn gpu_wait_timeout(deadline: Option<Instant>, now: Instant) -> RenderResult<Duration> {
    let maximum = Duration::from_secs(10);
    match deadline {
        Some(deadline) if deadline <= now => {
            Err(io::Error::other("GPU diagnostic deadline expired").into())
        }
        Some(deadline) => Ok((deadline - now).min(maximum)),
        None => Ok(maximum),
    }
}

fn chunk_distance_squared(coord: ChunkCoord, eye: Vec3) -> f32 {
    let x = (i64::from(coord.x) * i64::from(CHUNK_SIZE)) as f32 + CHUNK_SIZE as f32 * 0.5 - eye.x;
    let z = (i64::from(coord.z) * i64::from(CHUNK_SIZE)) as f32 + CHUNK_SIZE as f32 * 0.5 - eye.z;
    x * x + z * z
}

fn phase_chunk_near(coord: ChunkCoord, eye: Vec3) -> bool {
    chunk_distance_squared(coord, eye) <= (PHASE_RANGE + CHUNK_SIZE as f32 * 1.5).powi(2)
}

fn phase_material_table() -> [[u32; 4]; 8] {
    let mut table = [[0; 4]; 8];
    for block in Block::ALL {
        table[block as usize / 4][block as usize % 4] = material_for_face(block, 0);
    }
    table
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod time_tests;
