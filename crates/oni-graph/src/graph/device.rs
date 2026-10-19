//! The graph on its GPU: opening, limits, statistics and purging.

use super::*;

impl Graph {
    /// The default adapter, tiles at `TEXTURE_SIDE` or the device's own limit if lower.
    pub fn new() -> Result<Self, Error> {
        Self::with_max_side(TEXTURE_SIDE)
    }

    /// Tiles and render pieces at most `max_side` pixels a side.
    pub fn with_max_side(max_side: u32) -> Result<Self, Error> {
        pollster::block_on(Self::open(max_side))
    }

    pub(super) async fn open(max_side: u32) -> Result<Self, Error> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| Error::NoAdapter(e.to_string()))?;
        let info = adapter.get_info();
        // The adapter's own limits: the defaults stop textures at 8 192 px, half a pixel-shift frame.
        let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), ..Default::default() };
        let (device, queue) = adapter.request_device(&descriptor).await.map_err(|e| Error::NoDevice(e.to_string()))?;
        Ok(Self::from_device(device, queue, format!("{} ({:?})", info.name, info.backend), max_side))
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
            adapter,
        }
    }

    pub fn max_side(&self) -> u32 {
        self.max_side
    }

    pub fn stats(&self) -> Stats {
        Stats {
            dispatches: self.dispatches,
            copies: self.copies,
            textures_created: self.pool.created,
            live_bytes: self.pool.live.load(Ordering::Relaxed),
            idle_textures: self.pool.idle(),
        }
    }

    /// Frees every idle texture; a caller short of memory clears its caches too.
    pub fn purge(&mut self) {
        self.pool.purge();
    }
}
