//! The texture kind the graph holds, and the pool that recycles it.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

/// The format every stage writes: images stay full floating point between stages.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
pub(crate) const BYTES_PER_PIXEL: u64 = 16;

/// How a source holds its pixels: full floats, or half floats for a decode and its reductions, which
/// keep 11 significant bits at half the memory; stages read both alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Full,
    Half,
}

impl Precision {
    pub(crate) fn format(self) -> wgpu::TextureFormat {
        match self {
            Precision::Full => FORMAT,
            Precision::Half => wgpu::TextureFormat::Rgba16Float,
        }
    }

    pub(crate) fn bytes(self) -> u64 {
        match self {
            Precision::Full => BYTES_PER_PIXEL,
            Precision::Half => BYTES_PER_PIXEL / 2,
        }
    }
}

/// The bytes the graph's textures hold, and the most they held at once.
#[derive(Default)]
pub(crate) struct Meter {
    pub live: AtomicI64,
    pub peak: AtomicI64,
}

impl Meter {
    fn add(&self, bytes: i64) {
        let now = self.live.fetch_add(bytes, Ordering::Relaxed) + bytes;
        self.peak.fetch_max(now, Ordering::Relaxed);
    }
}

/// The step pooled textures' sides are rounded up to.
pub const STEP: u32 = 128;

/// Owned outputs kept for reuse: the shown picture, the one in flight, Compare's before.
const OUTPUTS: usize = 4;

/// Renders an idle texture waits for reuse: a notch's own reads (histograms, loupe) sit between two of
/// its renders, and a 43 MB texture made again costs about 1.5 ms.
pub const IDLE_RENDERS: u64 = 8;
/// Bytes idle textures may hold, the newest kept: a zoom asks new sizes every step, which would
/// otherwise pile up gigabytes no render takes again.
const IDLE_BYTES: u64 = 512 << 20;

struct Inner {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// A source tile belongs to its source and never enters the pool.
    pooled: bool,
    precision: Precision,
    meter: Arc<Meter>,
}

impl Drop for Inner {
    /// Freed at once rather than when the driver gets to it; wgpu holds it until queued work using it completes.
    fn drop(&mut self) {
        self.texture.destroy();
        self.meter.live.fetch_sub(self.bytes(), Ordering::Relaxed);
    }
}

impl Inner {
    fn bytes(&self) -> i64 {
        (u64::from(self.width) * u64::from(self.height) * self.precision.bytes()) as i64
    }
}

/// A float texture. Only `Pool` creates one, so no other format can enter the graph.
#[derive(Clone)]
pub(crate) struct FloatTexture(Arc<Inner>);

impl FloatTexture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.0.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.0.view
    }

    pub fn size(&self) -> (u32, u32) {
        (self.0.width, self.0.height)
    }

    /// Owned textures are never written once made: a source may share one with another.
    pub fn owned(&self) -> bool {
        !self.0.pooled
    }

    pub fn precision(&self) -> Precision {
        self.0.precision
    }
}

struct Free {
    texture: FloatTexture,
    generation: u64,
}

pub(crate) struct Pool {
    device: wgpu::Device,
    free: Vec<Free>,
    /// The renders' owned outputs, taken again once nothing else holds them: a fresh texture written
    /// by a kernel is cleared first, a 43 MB pass at the canvas's size.
    outputs: Vec<FloatTexture>,
    generation: u64,
    pub idle_bytes: u64,
    pub created: u64,
    pub meter: Arc<Meter>,
    max_side: u32,
}

impl Pool {
    pub fn new(device: wgpu::Device) -> Self {
        let max_side = device.limits().max_texture_dimension_2d;
        Self { device, free: Vec::new(), outputs: Vec::new(), generation: 0, idle_bytes: IDLE_BYTES, created: 0, meter: Arc::default(), max_side }
    }

    /// A texture holding at least this size, recycled when one is free. Sides are rounded up to
    /// `STEP`, so a chain's stages, each a few pixels apart in reach, share their textures.
    pub fn acquire(&mut self, width: u32, height: u32) -> FloatTexture {
        let up = |v: u32| (v.div_ceil(STEP) * STEP).min(self.max_side).max(v);
        let size = (up(width), up(height));
        if let Some(i) = self.free.iter().position(|f| f.texture.size() == size) {
            return self.free.swap_remove(i).texture;
        }
        self.create(size.0, size.1, true)
    }

    /// A source tile: same usages, never recycled.
    pub fn create_owned(&mut self, width: u32, height: u32, precision: Precision) -> FloatTexture {
        self.created += 1;
        made(&self.device, &self.meter, width, height, false, precision)
    }

    /// A render's owned output: one no source holds any more when there is one of this size, else a
    /// new one; outputs nothing holds past the last `OUTPUTS` are let go.
    pub fn output(&mut self, width: u32, height: u32) -> FloatTexture {
        let idle = |t: &FloatTexture| Arc::strong_count(&t.0) == 1;
        if let Some(t) = self.outputs.iter().find(|t| idle(t) && t.size() == (width, height)) {
            return t.clone();
        }
        let t = self.create(width, height, false);
        self.outputs.push(t.clone());
        while self.outputs.len() > OUTPUTS {
            let Some(i) = self.outputs.iter().position(idle) else { break };
            self.outputs.remove(i);
        }
        t
    }

    fn create(&mut self, width: u32, height: u32, pooled: bool) -> FloatTexture {
        self.created += 1;
        made(&self.device, &self.meter, width, height, pooled, Precision::Full)
    }

    /// Hands a texture back; one still held elsewhere (a cache, a source) is only let go of here.
    pub fn release(&mut self, texture: FloatTexture) {
        if texture.0.pooled && Arc::strong_count(&texture.0) == 1 {
            self.free.push(Free { texture, generation: self.generation });
        }
    }

    /// Ends a render: what no render reused since `IDLE_RENDERS` renders is freed, so a small read
    /// between two renders of one chain never costs its textures a new allocation and clear.
    pub fn end_render(&mut self) {
        let generation = self.generation;
        self.free.retain(|f| f.generation + IDLE_RENDERS > generation);
        self.free.sort_by_key(|f| std::cmp::Reverse(f.generation));
        let mut held = 0;
        self.free.retain(|f| {
            let (w, h) = f.texture.size();
            held += u64::from(w) * u64::from(h) * BYTES_PER_PIXEL;
            held <= self.idle_bytes
        });
        self.generation += 1;
    }

    /// Frees every idle texture, for a system short of memory.
    pub fn purge(&mut self) {
        self.free.clear();
        self.outputs.retain(|t| Arc::strong_count(&t.0) > 1);
    }

    pub fn idle(&self) -> usize {
        self.free.len()
    }
}

/// A texture counted in `meter`; a pool's when `pooled`, else a source tile's.
pub(crate) fn made(device: &wgpu::Device, meter: &Arc<Meter>, width: u32, height: u32, pooled: bool, precision: Precision) -> FloatTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("oni-graph"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: precision.format(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let inner = Inner { texture, view, width, height, pooled, precision, meter: meter.clone() };
    meter.add(inner.bytes());
    FloatTexture(Arc::new(inner))
}
