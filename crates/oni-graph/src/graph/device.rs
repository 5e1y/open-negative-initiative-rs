//! The graph on its GPU: opening, limits, statistics and purging.

use super::*;
use std::sync::OnceLock;

/// The process's GPU: the high-performance adapter, and one device on it at the adapter's own limits.
#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// The one device every graph and window of a process records on, opened at the first call: devices
/// opened side by side crashed Windows' software adapter (WARP) in 5 checks' runs of 30, one at a time in none.
pub fn gpu() -> Result<Gpu, Error> {
    static GPU: OnceLock<Result<Gpu, Error>> = OnceLock::new();
    GPU.get_or_init(|| pollster::block_on(open_gpu())).clone()
}

async fn open_gpu() -> Result<Gpu, Error> {
    let instance = wgpu::Instance::default();
    let options = wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() };
    let adapter = instance.request_adapter(&options).await.map_err(|e| Error::NoAdapter(e.to_string()))?;
    // The adapter's own limits, as the defaults stop textures at 8 192 px; clearing a texture in place
    // lets a sheet laid every notch reuse the last one's.
    let required_features = adapter.features() & wgpu::Features::CLEAR_TEXTURE;
    let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), required_features, ..Default::default() };
    let (device, queue) = adapter.request_device(&descriptor).await.map_err(|e| Error::NoDevice(e.to_string()))?;
    Ok(Gpu { instance, adapter, device, queue })
}

impl Graph {
    /// The default adapter, tiles at `TEXTURE_SIDE` or the device's own limit if lower.
    pub fn new() -> Result<Self, Error> {
        Self::with_max_side(TEXTURE_SIDE)
    }

    /// Tiles and render pieces at most `max_side` pixels a side.
    pub fn with_max_side(max_side: u32) -> Result<Self, Error> {
        let gpu = gpu()?;
        let info = gpu.adapter.get_info();
        Ok(Self::from_device(gpu.device, gpu.queue, format!("{} ({:?})", info.name, info.backend), max_side))
    }

    /// A graph on a device another crate already opened, so one GPU serves the whole app. Tiles
    /// follow the device's own texture limit, which the caller chose when it requested the device.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, adapter: String, max_side: u32) -> Self {
        let limits = device.limits();
        let uniforms = Uniforms {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uniforms"),
                size: UNIFORM_RING,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            at: 0,
            align: u64::from(limits.min_uniform_buffer_offset_alignment).max(16),
            staged: Vec::new(),
            staged_from: 0,
        };
        Self {
            pool: Pool::new(device.clone()),
            device,
            queue,
            layouts: HashMap::new(),
            pipelines: HashMap::new(),
            max_side: max_side.min(limits.max_texture_dimension_2d).max(1),
            max_buffer: limits.max_buffer_size,
            dispatches: 0,
            copies: 0,
            uniforms,
            packer: None,
            door: None,
            adapter,
        }
    }

    /// The device and queue the graph records on, for a kernel beside it on the same GPU.
    pub fn device(&self) -> (wgpu::Device, wgpu::Queue) {
        (self.device.clone(), self.queue.clone())
    }

    /// Every later submission and upload passes `door`.
    pub fn through(&mut self, door: Arc<dyn Door>) {
        self.door = Some(door);
    }

    pub fn max_side(&self) -> u32 {
        self.max_side
    }

    pub fn stats(&self) -> Stats {
        Stats {
            dispatches: self.dispatches,
            copies: self.copies,
            textures_created: self.pool.created,
            live_bytes: self.pool.meter.live.load(Ordering::Relaxed),
            peak_bytes: self.pool.meter.peak.load(Ordering::Relaxed),
            idle_textures: self.pool.idle(),
            pipelines: self.pipelines.len(),
        }
    }

    /// Frees every idle texture; a caller short of memory clears its caches too.
    pub fn purge(&mut self) {
        self.pool.purge();
    }

    /// The bytes idle textures may hold between renders.
    pub fn idle_budget(&mut self, bytes: u64) {
        self.pool.idle_bytes = bytes;
    }
}
