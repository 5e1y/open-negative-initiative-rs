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
}

impl Graph {
    /// Lays `pixels` (row-major, `width` per row) on the GPU, in tiles past the texture side.
    pub fn upload(&mut self, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        self.upload_at(Rect::new(0, 0, width, height), pixels)
    }

    /// As `upload`, the pixels covering `extent` wherever it lies.
    pub fn upload_at(&mut self, extent: Rect, pixels: &[[f32; 4]]) -> Result<Source, Error> {
        if pixels.len() as u64 != extent.area() {
            return Err(Error::Size { expected: extent.area(), got: pixels.len() });
        }
        if extent.is_empty() {
            return Err(Error::RegionOutside { region: extent, extent });
        }
        let bytes: &[u8] = bytemuck::cast_slice(pixels);
        let row = extent.w as u64 * BYTES_PER_PIXEL;
        let mut tiles = Vec::new();
        for ty in (0..extent.h).step_by(self.max_side as usize) {
            for tx in (0..extent.w).step_by(self.max_side as usize) {
                let (w, h) = (self.max_side.min(extent.w - tx), self.max_side.min(extent.h - ty));
                let texture = self.pool.create_owned(w, h);
                let band = (UPLOAD_BAND_BYTES / (w as u64 * BYTES_PER_PIXEL)).max(1) as u32;
                for by in (0..h).step_by(band as usize) {
                    let rows = band.min(h - by);
                    let start = (ty + by) as u64 * row + tx as u64 * BYTES_PER_PIXEL;
                    let end = start + (rows as u64 - 1) * row + w as u64 * BYTES_PER_PIXEL;
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: texture.texture(),
                            mip_level: 0,
                            origin: wgpu::Origin3d { x: 0, y: by, z: 0 },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &bytes[start as usize..end as usize],
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: None },
                        wgpu::Extent3d { width: w, height: rows, depth_or_array_layers: 1 },
                    );
                }
                tiles.push((Rect::new(extent.x + tx as i32, extent.y + ty as i32, w, h), texture));
            }
        }
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent, tiles })))
    }
}
