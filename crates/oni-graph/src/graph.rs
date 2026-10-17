//! Sources, chains, and their evaluation by region on the GPU.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::util::DeviceExt;

use crate::stage::{AuxFrame, Frame, MAX_AUX, PRELUDE, aux_prelude};
use crate::stages::{Axis, Lanczos};
use crate::texture::{BYTES_PER_PIXEL, FloatTexture, Pool};
use crate::{Rect, Stage};

/// Metal's and Direct3D 12's largest texture side: a pixel-shift frame of 16 736 px goes in tiles.
pub const TEXTURE_SIDE: u32 = 16_384;

/// Largest upload written at once; a whole 16 384 px tile is 4 GiB, past what one staging buffer may hold.
const UPLOAD_BAND_BYTES: u64 = 64 << 20;

#[derive(Debug)]
pub enum Error {
    NoAdapter(String),
    NoDevice(String),
    /// The request leaves the image's extent, or is empty.
    RegionOutside { region: Rect, extent: Rect },
    /// Stage `stage` reads nothing of its input's extent for this request.
    EmptyInput { stage: usize },
    /// Not even one pixel of output fits the GPU's texture side.
    TooLarge,
    /// The pixel count does not match the size given.
    Size { expected: u64, got: usize },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoAdapter(e) => write!(f, "no GPU adapter: {e}"),
            Error::NoDevice(e) => write!(f, "no GPU device: {e}"),
            Error::RegionOutside { region, extent } => write!(f, "{region:?} is not inside {extent:?}"),
            Error::EmptyInput { stage } => write!(f, "stage {stage} reads nothing of its input"),
            Error::TooLarge => write!(f, "one output pixel needs more than the GPU's texture side"),
            Error::Size { expected, got } => write!(f, "{got} pixels given for {expected}"),
        }
    }
}

impl std::error::Error for Error {}

/// Counters a check reads: what the GPU was asked to do, and what the pool holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub dispatches: u64,
    pub copies: u64,
    pub textures_created: u64,
    pub live_bytes: i64,
    pub idle_textures: usize,
}

struct SourceInner {
    id: u64,
    extent: Rect,
    tiles: Vec<(Rect, FloatTexture)>,
}

/// Pixels resident on the GPU, in tiles no side of which passes the graph's texture side.
#[derive(Clone)]
pub struct Source(Arc<SourceInner>);

impl Source {
    pub fn extent(&self) -> Rect {
        self.0.extent
    }

    pub fn tiles(&self) -> usize {
        self.0.tiles.len()
    }

    /// The one texture covering the whole extent, its texel (0, 0) at the extent's corner; none when tiled.
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        match self.0.tiles.as_slice() {
            [(_, texture)] => Some(texture.view()),
            _ => None,
        }
    }
}

/// A stage and the further images it reads beside its input.
#[derive(Clone)]
struct Node {
    stage: Arc<dyn Stage>,
    aux: Vec<Image>,
}

/// A source and the stages that follow it: a recipe, evaluated only when a region is asked for.
#[derive(Clone)]
pub struct Image {
    source: Source,
    stages: Vec<Node>,
}

impl Image {
    pub fn new(source: &Source) -> Self {
        Self { source: source.clone(), stages: Vec::new() }
    }

    pub fn then(mut self, stage: impl Stage + 'static) -> Self {
        self.stages.push(Node { stage: Arc::new(stage), aux: Vec::new() });
        self
    }

    /// `stage` reading `aux` beside its input, through `load1(q)`, `load2(q)`, ... in each one's own
    /// coordinates. A node `aux` shares with this chain is evaluated once per render. Panics past `MAX_AUX`.
    pub fn then_reading(mut self, stage: impl Stage + 'static, aux: &[Image]) -> Self {
        assert!(aux.len() <= MAX_AUX, "{} auxiliary inputs, at most {MAX_AUX}", aux.len());
        self.stages.push(Node { stage: Arc::new(stage), aux: aux.to_vec() });
        self
    }

    /// Lanczos-3 by `scale` on both axes, each side rounded to a whole pixel; the result starts at 0.
    /// Panics on a scale that is not finite and positive, which has no picture to give.
    pub fn lanczos(self, scale: f64) -> Self {
        assert!(scale.is_finite() && scale > 0.0, "Lanczos scale {scale}");
        let e = self.extent();
        let side = |s: u32| ((s as f64 * scale).round() as u32).max(1);
        self.then(Lanczos { axis: Axis::X, origin: e.x, from: e.w, to: side(e.w) })
            .then(Lanczos { axis: Axis::Y, origin: e.y, from: e.h, to: side(e.h) })
    }

    pub fn extent(&self) -> Rect {
        self.stages.iter().fold(self.source.extent(), |e, node| node.stage.extent(e))
    }

    /// The stages of the chain itself, auxiliary images apart.
    pub fn stages(&self) -> usize {
        self.stages.len()
    }

    /// Every node this image reads, inputs before the stages reading them, each once; returns the last.
    fn flatten<'a>(&'a self, nodes: &mut Vec<Flat<'a>>, seen: &mut HashMap<u64, usize>) -> usize {
        let mut hasher = DefaultHasher::new();
        ("source", self.source.0.id).hash(&mut hasher);
        let key = hasher.finish();
        let mut at = intern(nodes, seen, Flat { key, extent: self.source.extent(), kind: Kind::Source(&self.source) });
        for node in &self.stages {
            let aux: Vec<usize> = node.aux.iter().map(|a| a.flatten(nodes, seen)).collect();
            let stage = node.stage.as_ref();
            let mut hasher = DefaultHasher::new();
            nodes[at].key.hash(&mut hasher);
            stage.name().hash(&mut hasher);
            stage.shader().hash(&mut hasher);
            stage.params().hash(&mut hasher);
            for &a in &aux {
                nodes[a].key.hash(&mut hasher);
            }
            let key = hasher.finish();
            let extent = stage.extent(nodes[at].extent);
            at = intern(nodes, seen, Flat { key, extent, kind: Kind::Stage { stage, input: at, aux } });
        }
        at
    }
}

enum Kind<'a> {
    Source(&'a Source),
    Stage { stage: &'a dyn Stage, input: usize, aux: Vec<usize> },
}

/// One node of a render: its key folds in every node upstream of it, so equal keys are one node.
struct Flat<'a> {
    key: u64,
    extent: Rect,
    kind: Kind<'a>,
}

fn intern<'a>(nodes: &mut Vec<Flat<'a>>, seen: &mut HashMap<u64, usize>, flat: Flat<'a>) -> usize {
    if let Some(&i) = seen.get(&flat.key) {
        return i;
    }
    seen.insert(flat.key, nodes.len());
    nodes.push(flat);
    nodes.len() - 1
}

/// What one evaluation needs: each node's rectangle, `None` where nothing reads it, and the cache's hits.
struct Plan<'a> {
    nodes: Vec<Flat<'a>>,
    last: usize,
    rects: Vec<Option<Rect>>,
    hits: Vec<Option<(FloatTexture, Rect)>>,
}

impl<'a> Plan<'a> {
    /// Consumers before producers, each rectangle the union of what its readers need; a node the cache
    /// holds whole asks nothing of its inputs.
    fn new(image: &'a Image, request: Rect, cache: Option<&Cache>) -> Result<Self, Error> {
        let mut nodes = Vec::new();
        let last = image.flatten(&mut nodes, &mut HashMap::new());
        let n = nodes.len();
        let mut rects: Vec<Option<Rect>> = vec![None; n];
        let mut hits: Vec<Option<(FloatTexture, Rect)>> = vec![None; n];
        rects[last] = Some(request);
        for i in (0..n).rev() {
            let Some(need) = rects[i] else { continue };
            let Kind::Stage { stage, input, aux } = &nodes[i].kind else { continue };
            if let Some(hit) = cache.and_then(|c| c.hit(i, nodes[i].key, need)) {
                hits[i] = Some(hit);
                continue;
            }
            let mut ask = |j: usize, region: Rect| -> Result<(), Error> {
                let region = region.intersect(&nodes[j].extent);
                if region.is_empty() {
                    return Err(Error::EmptyInput { stage: i.saturating_sub(1) });
                }
                rects[j] = Some(rects[j].map_or(region, |r| r.union(&region)));
                Ok(())
            };
            ask(*input, stage.input_region(need))?;
            for (k, &a) in aux.iter().enumerate() {
                ask(a, stage.aux_region(k, need))?;
            }
        }
        Ok(Self { nodes, last, rects, hits })
    }

    fn output(&self) -> Rect {
        self.rects[self.last].expect("the request")
    }
}

struct Entry {
    key: u64,
    rect: Rect,
    texture: FloatTexture,
}

/// Each stage's last result, held for the next render: a change on a late stage reruns only what follows it.
/// One cache serves one view of one graph; dropping it frees its textures.
#[derive(Default)]
pub struct Cache {
    entries: Vec<Option<Entry>>,
}

impl Cache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Node `index`'s result when it is current and covers `need`.
    fn hit(&self, index: usize, key: u64, need: Rect) -> Option<(FloatTexture, Rect)> {
        match self.entries.get(index) {
            Some(Some(e)) if e.key == key && e.rect.contains(&need) => Some((e.texture.clone(), e.rect)),
            _ => None,
        }
    }

    /// Stores node `index`'s result, handing back the one it replaces.
    fn store(&mut self, index: usize, key: u64, rect: Rect, texture: FloatTexture) -> Option<FloatTexture> {
        if self.entries.len() <= index {
            self.entries.resize_with(index + 1, || None);
        }
        self.entries[index].replace(Entry { key, rect, texture }).map(|e| e.texture)
    }

    /// Drops results past the graph's last node, left by a larger one.
    fn truncate(&mut self, nodes: usize) -> Vec<FloatTexture> {
        if self.entries.len() <= nodes {
            return Vec::new();
        }
        self.entries.drain(nodes..).flatten().map(|e| e.texture).collect()
    }
}

/// A layout for each count of auxiliary inputs.
struct Layout {
    group: wgpu::BindGroupLayout,
    pipeline: wgpu::PipelineLayout,
}

/// The GPU, the compiled stages and the texture pool.
pub struct Graph {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layouts: HashMap<usize, Layout>,
    pipelines: HashMap<u64, wgpu::ComputePipeline>,
    pool: Pool,
    max_side: u32,
    max_buffer: u64,
    dispatches: u64,
    copies: u64,
    pub adapter: String,
}

static SOURCE_IDS: AtomicU64 = AtomicU64::new(1);

impl Graph {
    /// The default adapter, tiles at `TEXTURE_SIDE` or the device's own limit if lower.
    pub fn new() -> Result<Self, Error> {
        Self::with_max_side(TEXTURE_SIDE)
    }

    /// Tiles and render pieces at most `max_side` pixels a side.
    pub fn with_max_side(max_side: u32) -> Result<Self, Error> {
        pollster::block_on(Self::open(max_side))
    }

    async fn open(max_side: u32) -> Result<Self, Error> {
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
            adapter,
        }
    }

    pub fn max_side(&self) -> u32 {
        self.max_side
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The largest parameter block one stage may bind.
    pub fn max_params(&self) -> u64 {
        self.device.limits().max_uniform_buffer_binding_size
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

    /// Evaluates `region` of `image` and reads it back, row-major. With a cache, each stage's result
    /// is kept and the deepest one still current is the starting point.
    pub fn render(&mut self, image: &Image, region: Rect, mut cache: Option<&mut Cache>) -> Result<Vec<[f32; 4]>, Error> {
        let extent = image.extent();
        if region.is_empty() || !extent.contains(&region) {
            return Err(Error::RegionOutside { region, extent });
        }
        if let Some(c) = cache.as_deref_mut() {
            let mut nodes = Vec::new();
            let count = {
                image.flatten(&mut nodes, &mut HashMap::new());
                nodes.len()
            };
            for texture in c.truncate(count) {
                self.pool.release(texture);
            }
        }
        let mut out = vec![[0.0f32; 4]; region.area() as usize];
        let result = self.render_piece(image, region, region, &mut out, cache);
        self.pool.end_render();
        result.map(|()| out)
    }

    /// The whole image evaluated once into a new source: a preview held as pixels, not as a recipe.
    pub fn materialise(&mut self, image: &Image) -> Result<Source, Error> {
        let extent = image.extent();
        self.materialise_region(image, extent)
    }

    /// `region` of `image` evaluated once into a new source covering that region, kept on the GPU.
    pub fn materialise_region(&mut self, image: &Image, region: Rect) -> Result<Source, Error> {
        let extent = image.extent();
        if region.is_empty() || !extent.contains(&region) {
            return Err(Error::RegionOutside { region, extent });
        }
        let plan = Plan::new(image, region, None)?;
        if !self.fits(&plan) {
            let pixels = self.render(image, region, None)?;
            return self.upload_at(region, &pixels);
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (texture, rect) = self.evaluate(&mut encoder, plan, None);
        let owned = self.pool.create_owned(region.w, region.h);
        self.copy(&mut encoder, &texture, rect, &owned, region, region);
        self.queue.submit([encoder.finish()]);
        self.pool.release(texture);
        self.pool.end_render();
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: region, tiles: vec![(region, owned)] })))
    }

    fn fits(&self, plan: &Plan) -> bool {
        let out = plan.output();
        let readback = padded_row(out.w) * out.h as u64;
        let sides = plan
            .rects
            .iter()
            .zip(&plan.hits)
            .filter_map(|(r, hit)| if hit.is_none() { *r } else { None })
            .all(|r| r.w <= self.max_side && r.h <= self.max_side);
        sides && readback <= self.max_buffer
    }

    /// Renders `piece` of `whole` into `out`, halving the piece until every node fits one texture.
    fn render_piece(
        &mut self,
        image: &Image,
        whole: Rect,
        piece: Rect,
        out: &mut [[f32; 4]],
        mut cache: Option<&mut Cache>,
    ) -> Result<(), Error> {
        let plan = Plan::new(image, piece, cache.as_deref())?;
        if !self.fits(&plan) {
            drop(plan);
            let (a, b) = if piece.w >= piece.h && piece.w > 1 {
                let half = piece.w / 2;
                (Rect::new(piece.x, piece.y, half, piece.h), Rect::new(piece.x + half as i32, piece.y, piece.w - half, piece.h))
            } else if piece.h > 1 {
                let half = piece.h / 2;
                (Rect::new(piece.x, piece.y, piece.w, half), Rect::new(piece.x, piece.y + half as i32, piece.w, piece.h - half))
            } else {
                return Err(Error::TooLarge);
            };
            self.render_piece(image, whole, a, out, cache.as_deref_mut())?;
            return self.render_piece(image, whole, b, out, cache);
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (texture, rect) = self.evaluate(&mut encoder, plan, cache);
        let row = padded_row(piece.w);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: row * piece.h as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d { x: (piece.x - rect.x) as u32, y: (piece.y - rect.y) as u32, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: None },
            },
            wgpu::Extent3d { width: piece.w, height: piece.h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        self.pool.release(texture);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        {
            let mapped = slice.get_mapped_range().expect("mapped range");
            let width = piece.w as usize * BYTES_PER_PIXEL as usize;
            for y in 0..piece.h as usize {
                let at = (piece.y - whole.y) as usize + y;
                let first = at * whole.w as usize + (piece.x - whole.x) as usize;
                let target: &mut [u8] = bytemuck::cast_slice_mut(&mut out[first..first + piece.w as usize]);
                target.copy_from_slice(&mapped[y * row as usize..y * row as usize + width]);
            }
        }
        readback.unmap();
        Ok(())
    }

    /// Records every node the cache cannot skip, producers first, each texture handed back to the pool
    /// once its last reader has run; returns the last node's result and the rectangle it covers.
    fn evaluate(&mut self, encoder: &mut wgpu::CommandEncoder, plan: Plan, mut cache: Option<&mut Cache>) -> (FloatTexture, Rect) {
        let Plan { nodes, last, rects, hits } = plan;
        let n = nodes.len();
        let mut pending = vec![0usize; n];
        for i in 0..n {
            if let (Some(_), None, Kind::Stage { input, aux, .. }) = (rects[i], &hits[i], &nodes[i].kind) {
                pending[*input] += 1;
                for &a in aux {
                    pending[a] += 1;
                }
            }
        }
        let mut results = hits;
        for i in 0..n {
            let Some(need) = rects[i] else { continue };
            if results[i].is_some() {
                continue;
            }
            match &nodes[i].kind {
                Kind::Source(source) => results[i] = Some(self.assemble(encoder, source, need)),
                Kind::Stage { stage, input, aux } => {
                    let out = self.pool.acquire(need.w, need.h);
                    {
                        let main = results[*input].as_ref().expect("the input is evaluated first");
                        let extras: Vec<(&FloatTexture, Rect, Rect)> = aux
                            .iter()
                            .map(|&a| {
                                let (t, r) = results[a].as_ref().expect("an auxiliary input is evaluated first");
                                (t, *r, nodes[a].extent)
                            })
                            .collect();
                        self.dispatch(encoder, *stage, (&main.0, main.1, nodes[*input].extent), &extras, &out, need);
                    }
                    if let Some(c) = cache.as_deref_mut()
                        && let Some(old) = c.store(i, nodes[i].key, need, out.clone())
                    {
                        self.pool.release(old);
                    }
                    results[i] = Some((out, need));
                    for j in std::iter::once(*input).chain(aux.iter().copied()) {
                        pending[j] -= 1;
                        if pending[j] == 0 && j != last
                            && let Some((texture, _)) = results[j].take()
                        {
                            self.pool.release(texture);
                        }
                    }
                }
            }
        }
        results[last].take().expect("the last node is evaluated")
    }

    /// The source pixels of `needed`: a tile itself when one holds them all, else a copy across tiles.
    fn assemble(&mut self, encoder: &mut wgpu::CommandEncoder, source: &Source, needed: Rect) -> (FloatTexture, Rect) {
        if let Some((rect, tile)) = source.0.tiles.iter().find(|(rect, _)| rect.contains(&needed)) {
            return (tile.clone(), *rect);
        }
        let texture = self.pool.acquire(needed.w, needed.h);
        for (rect, tile) in &source.0.tiles {
            let part = rect.intersect(&needed);
            if !part.is_empty() {
                self.copy(encoder, tile, *rect, &texture, needed, part);
            }
        }
        (texture, needed)
    }

    /// Copies `part`, in image coordinates, between textures covering `from_rect` and `to_rect`.
    fn copy(&mut self, encoder: &mut wgpu::CommandEncoder, from: &FloatTexture, from_rect: Rect, to: &FloatTexture, to_rect: Rect, part: Rect) {
        let at = |r: Rect| wgpu::Origin3d { x: (part.x - r.x) as u32, y: (part.y - r.y) as u32, z: 0 };
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo { texture: from.texture(), mip_level: 0, origin: at(from_rect), aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyTextureInfo { texture: to.texture(), mip_level: 0, origin: at(to_rect), aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: part.w, height: part.h, depth_or_array_layers: 1 },
        );
        self.copies += 1;
    }

    /// One stage over `output_rect`; `input` and each of `aux` is (texture, rectangle it covers, extent).
    fn dispatch(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        stage: &dyn Stage,
        input: (&FloatTexture, Rect, Rect),
        aux: &[(&FloatTexture, Rect, Rect)],
        output: &FloatTexture,
        output_rect: Rect,
    ) {
        let (texture, input_rect, input_extent) = input;
        let frame = Frame {
            out_origin: [output_rect.x, output_rect.y],
            in_origin: [input_rect.x, input_rect.y],
            in_min: [input_extent.x, input_extent.y],
            in_max: [input_extent.right(), input_extent.bottom()],
            out_size: [output_rect.w, output_rect.h],
            pad: [0; 2],
        };
        let mut params = stage.params();
        // A uniform binding holds at least 16 bytes, in 16-byte steps.
        params.resize(params.len().div_ceil(16).max(1) * 16, 0);
        let frame = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame"),
            contents: bytemuck::bytes_of(&frame),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let params = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mut frames = [AuxFrame::default(); MAX_AUX];
        for (f, (_, rect, extent)) in frames.iter_mut().zip(aux) {
            *f = AuxFrame { in_origin: [rect.x, rect.y], in_min: [extent.x, extent.y], in_max: [extent.right(), extent.bottom()], pad: [0; 2] };
        }
        let frames = (!aux.is_empty()).then(|| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("aux frames"),
                contents: bytemuck::bytes_of(&frames),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(texture.view()) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(output.view()) },
            wgpu::BindGroupEntry { binding: 3, resource: params.as_entire_binding() },
        ];
        if let Some(frames) = &frames {
            entries.push(wgpu::BindGroupEntry { binding: 4, resource: frames.as_entire_binding() });
            for (k, (t, _, _)) in aux.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry { binding: 5 + k as u32, resource: wgpu::BindingResource::TextureView(t.view()) });
            }
        }
        let pipeline = self.pipeline(stage, aux.len());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(stage.name()),
            layout: &self.layouts[&aux.len()].group,
            entries: &entries,
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(stage.name()), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(output_rect.w.div_ceil(8), output_rect.h.div_ceil(8), 1);
        self.dispatches += 1;
    }

    fn layout(&mut self, arity: usize) -> &Layout {
        let device = &self.device;
        self.layouts.entry(arity).or_insert_with(|| {
            let mut entries = vec![
                uniform_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: crate::texture::FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                uniform_entry(3),
            ];
            if arity > 0 {
                entries.push(uniform_entry(4));
                entries.extend((0..arity).map(|k| texture_entry(5 + k as u32)));
            }
            let group = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("stage"), entries: &entries });
            let pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("stage"),
                bind_group_layouts: &[Some(&group)],
                immediate_size: 0,
            });
            Layout { group, pipeline }
        })
    }

    fn pipeline(&mut self, stage: &dyn Stage, arity: usize) -> wgpu::ComputePipeline {
        let mut hasher = DefaultHasher::new();
        (stage.shader(), arity).hash(&mut hasher);
        let key = hasher.finish();
        if let Some(p) = self.pipelines.get(&key) {
            return p.clone();
        }
        let source = format!("{PRELUDE}\n{}\n{}", aux_prelude(arity), stage.shader());
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(stage.name()),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = self.layout(arity).pipeline.clone();
        let layout = &layout;
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(stage.name()),
            layout: Some(layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        self.pipelines.insert(key, pipeline.clone());
        pipeline
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A readback row, padded to the 256 bytes a texture-to-buffer copy requires.
fn padded_row(width: u32) -> u64 {
    (width as u64 * BYTES_PER_PIXEL).div_ceil(256) * 256
}
