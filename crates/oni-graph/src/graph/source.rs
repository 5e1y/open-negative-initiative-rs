//! Sources: pixels uploaded once in tiles of at most `TEXTURE_SIDE`, then read by every chain.

use super::*;

impl Source {
    pub fn extent(&self) -> Rect {
        self.0.extent
    }

    pub fn tiles(&self) -> usize {
        self.0.tiles.len()
    }

    /// Unique for the run: two sources with one id hold the same pixels.
    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// The one texture covering the whole extent, its texel (0, 0) at the extent's corner; none when tiled.
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        match self.0.tiles.as_slice() {
            [(_, texture)] => Some(texture.view()),
            _ => None,
        }
    }

    pub fn precision(&self) -> Precision {
        self.0.tiles.first().map_or(Precision::Full, |(_, t)| t.precision())
    }
}

impl Graph {
    /// Lays `pixels` (row-major, `width` per row) on the GPU, in tiles past the texture side.
    pub fn upload(&mut self, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        self.upload_at(Rect::new(0, 0, width, height), pixels)
    }

    /// As `upload`, the pixels covering `extent` wherever it lies.
    pub fn upload_at(&mut self, extent: Rect, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        let (queue, pool) = (&self.queue, &mut self.pool);
        laid(queue, self.max_side, extent, pixels, Precision::Full, |w, h, p| pool.create_owned(w, h, p))
    }

    /// As `upload`, held in half floats when one texture takes it whole.
    pub fn upload_half(&mut self, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        let (queue, pool) = (&self.queue, &mut self.pool);
        laid(queue, self.max_side, Rect::new(0, 0, width, height), pixels, Precision::Half, |w, h, p| pool.create_owned(w, h, p))
    }

    /// What lays pixels beside the graph, from any thread.
    pub fn uploader(&self) -> Uploader {
        Uploader { device: self.device.clone(), queue: self.queue.clone(), max_side: self.max_side, meter: self.pool.meter.clone() }
    }
}

/// Lays pixels on the GPU without the graph, so a decode's upload of a few hundred MB holds no lock
/// a render waits on; its textures are a source's own, never pooled.
#[derive(Clone)]
pub struct Uploader {
    device: wgpu::Device,
    queue: wgpu::Queue,
    max_side: u32,
    meter: Arc<Meter>,
}

impl Uploader {
    /// As `Graph::upload_half`: a decode is held in half floats.
    pub fn upload(&self, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        let make = |w, h, p| crate::texture::made(&self.device, &self.meter, w, h, false, p);
        laid(&self.queue, self.max_side, Rect::new(0, 0, width, height), pixels, Precision::Half, make)
    }
}

/// `pixels` written over `extent` in tiles of at most `max_side`, each a texture `make` gives; in half
/// floats when asked and one tile holds them, as only like formats copy across tiles.
fn laid(queue: &wgpu::Queue, max_side: u32, extent: Rect, pixels: &[[f32; 4]], asked: Precision, mut make: impl FnMut(u32, u32, Precision) -> FloatTexture) -> Result<Source, Error> {
    if pixels.len() as u64 != extent.area() {
        return Err(Error::Size { expected: extent.area(), got: pixels.len() });
    }
    if extent.is_empty() {
        return Err(Error::RegionOutside { region: extent, extent });
    }
    let precision = if extent.w <= max_side && extent.h <= max_side { asked } else { Precision::Full };
    let bytes: &[u8] = bytemuck::cast_slice(pixels);
    let row = extent.w as u64 * BYTES_PER_PIXEL;
    let mut halves: Vec<u16> = Vec::new();
    let mut tiles = Vec::new();
    for ty in (0..extent.h).step_by(max_side as usize) {
        for tx in (0..extent.w).step_by(max_side as usize) {
            let (w, h) = (max_side.min(extent.w - tx), max_side.min(extent.h - ty));
            let texture = make(w, h, precision);
            let band = (UPLOAD_BAND_BYTES / (w as u64 * BYTES_PER_PIXEL)).max(1) as u32;
            for by in (0..h).step_by(band as usize) {
                let rows = band.min(h - by);
                let start = (ty + by) as u64 * row + tx as u64 * BYTES_PER_PIXEL;
                let end = start + (rows as u64 - 1) * row + w as u64 * BYTES_PER_PIXEL;
                let at = wgpu::TexelCopyTextureInfo { texture: texture.texture(), mip_level: 0, origin: wgpu::Origin3d { x: 0, y: by, z: 0 }, aspect: wgpu::TextureAspect::All };
                let size = wgpu::Extent3d { width: w, height: rows, depth_or_array_layers: 1 };
                if precision == Precision::Half {
                    // One tile: the band's rows are whole and contiguous.
                    let floats: &[f32] = bytemuck::cast_slice(&bytes[start as usize..end as usize]);
                    halves.clear();
                    halves.extend(floats.iter().map(|&v| ::half::f16::from_f32(v).to_bits()));
                    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 8), rows_per_image: None };
                    queue.write_texture(at, bytemuck::cast_slice(&halves), layout, size);
                    continue;
                }
                queue.write_texture(at, &bytes[start as usize..end as usize], wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: None }, size);
            }
            tiles.push((Rect::new(extent.x + tx as i32, extent.y + ty as i32, w, h), texture));
        }
    }
    let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
    Ok(Source(Arc::new(SourceInner { id, extent, tiles })))
}

impl Graph {
    /// Each source read back whole, row-major, every copy in one submission and one wait: many small
    /// pictures pay one round trip to the GPU, not one each.
    pub fn read_back(&mut self, sources: &[&Source]) -> Vec<Vec<[f32; 4]>> {
        let mut parts = Vec::new();
        let mut size = 0u64;
        for (i, s) in sources.iter().enumerate() {
            for (rect, texture) in &s.0.tiles {
                parts.push((i, *rect, texture, size));
                size += rendering::padded_row(rect.w) * rect.h as u64;
            }
        }
        let mut out: Vec<Vec<[f32; 4]>> = sources.iter().map(|s| vec![[0.0; 4]; s.extent().area() as usize]).collect();
        if size == 0 {
            return out;
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for (_, rect, texture, offset) in &parts {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: texture.texture(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout { offset: *offset, bytes_per_row: Some(rendering::padded_row(rect.w) as u32), rows_per_image: None },
                },
                wgpu::Extent3d { width: rect.w, height: rect.h, depth_or_array_layers: 1 },
            );
        }
        self.flush_uniforms();
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        {
            let mapped = slice.get_mapped_range().expect("mapped range");
            for (i, rect, _, offset) in &parts {
                let (extent, row) = (sources[*i].extent(), rendering::padded_row(rect.w) as usize);
                let width = rect.w as usize * BYTES_PER_PIXEL as usize;
                for y in 0..rect.h as usize {
                    let first = ((rect.y - extent.y) as usize + y) * extent.w as usize + (rect.x - extent.x) as usize;
                    let target: &mut [u8] = bytemuck::cast_slice_mut(&mut out[*i][first..first + rect.w as usize]);
                    let at = *offset as usize + y * row;
                    target.copy_from_slice(&mapped[at..at + width]);
                }
            }
        }
        readback.unmap();
        out
    }
}
