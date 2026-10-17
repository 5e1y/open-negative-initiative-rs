//! The one texture kind the graph holds, and the pool that recycles it.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

/// The only format any texture of the graph is created in: images stay floating point between stages.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
pub(crate) const BYTES_PER_PIXEL: u64 = 16;

struct Inner {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// A source tile belongs to its source and never enters the pool.
    pooled: bool,
    live: Arc<AtomicI64>,
}

impl Drop for Inner {
    /// Freed at once rather than when the driver gets to it; wgpu holds it until queued work using it completes.
    fn drop(&mut self) {
        self.texture.destroy();
        self.live.fetch_sub((self.width as u64 * self.height as u64 * BYTES_PER_PIXEL) as i64, Ordering::Relaxed);
    }
}

/// An rgba32float texture. Only `Pool` creates one, so no other format can enter the graph.
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
}

struct Free {
    texture: FloatTexture,
    generation: u64,
}

pub(crate) struct Pool {
    device: wgpu::Device,
    free: Vec<Free>,
    generation: u64,
    pub created: u64,
    pub live: Arc<AtomicI64>,
}

impl Pool {
    pub fn new(device: wgpu::Device) -> Self {
        Self { device, free: Vec::new(), generation: 0, created: 0, live: Arc::new(AtomicI64::new(0)) }
    }

    /// A texture of exactly this size, recycled when one is free.
    pub fn acquire(&mut self, width: u32, height: u32) -> FloatTexture {
        if let Some(i) = self.free.iter().position(|f| f.texture.size() == (width, height)) {
            return self.free.swap_remove(i).texture;
        }
        self.create(width, height, true)
    }

    /// A source tile: same format and usages, never recycled.
    pub fn create_owned(&mut self, width: u32, height: u32) -> FloatTexture {
        self.create(width, height, false)
    }

    fn create(&mut self, width: u32, height: u32, pooled: bool) -> FloatTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("oni-graph"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.created += 1;
        self.live.fetch_add((width as u64 * height as u64 * BYTES_PER_PIXEL) as i64, Ordering::Relaxed);
        FloatTexture(Arc::new(Inner { texture, view, width, height, pooled, live: self.live.clone() }))
    }

    /// Hands a texture back; one still held elsewhere (a cache, a source) is only let go of here.
    pub fn release(&mut self, texture: FloatTexture) {
        if texture.0.pooled && Arc::strong_count(&texture.0) == 1 {
            self.free.push(Free { texture, generation: self.generation });
        }
    }

    /// Ends a render: what was retired before this render and not reused by it is freed.
    pub fn end_render(&mut self) {
        let generation = self.generation;
        self.free.retain(|f| f.generation >= generation);
        self.generation += 1;
    }

    /// Frees every idle texture, for a system short of memory.
    pub fn purge(&mut self) {
        self.free.clear();
    }

    pub fn idle(&self) -> usize {
        self.free.len()
    }
}
