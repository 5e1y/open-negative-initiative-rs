//! A render: the plan evaluated piece by piece, each node dispatched once over what it must cover.

use super::*;

/// Waits for `slice` to map for reading; a failure comes back as an error, never a panic in whichever
/// thread polls the shared device.
pub(super) fn mapped(device: &wgpu::Device, slice: &wgpu::BufferSlice<'_>) -> Result<(), Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Error::Lost(e.to_string()))?;
    rx.recv().map_err(|e| Error::Lost(e.to_string()))?.map_err(|e| Error::Lost(e.to_string()))
}

/// A readback row, padded to the 256 bytes a texture-to-buffer copy requires.
pub(super) fn padded_row(width: u32) -> u64 {
    (width as u64 * BYTES_PER_PIXEL).div_ceil(256) * 256
}

impl Graph {
    /// Evaluates `region` of `image` and reads it back, row-major. With a cache, each stage's result
    /// is kept and the deepest one still current is the starting point.
    pub fn render(&mut self, image: &Image, region: Rect, mut cache: Option<&mut Cache>) -> Result<Vec<[f32; 4]>, Error> {
        let image = &image.readable();
        let extent = image.extent();
        if region.is_empty() || !extent.contains(&region) {
            return Err(Error::RegionOutside { region, extent });
        }
        if let Some(c) = cache.as_deref_mut() {
            let mut nodes = Vec::new();
            image.flatten(&mut nodes, &mut HashMap::new());
            c.begin(&nodes);
        }
        let mut out = vec![[0.0f32; 4]; region.area() as usize];
        let result = self.render_piece(image, region, region, &mut out, cache.as_deref_mut());
        self.settle(cache);
        self.ended();
        result.map(|()| out)
    }

    /// The whole image evaluated once into a new source: a preview held as pixels, not as a recipe.
    pub fn materialise(&mut self, image: &Image) -> Result<Source, Error> {
        let extent = image.extent();
        self.materialise_region(image, extent)
    }

    /// `region` of `image` evaluated once into a new source covering that region, kept on the GPU.
    pub fn materialise_region(&mut self, image: &Image, region: Rect) -> Result<Source, Error> {
        self.materialise_cached(image, region, None)
    }

    /// `materialise_region` starting from what `cache` holds, and leaving in it what it keeps.
    pub fn materialise_cached(&mut self, image: &Image, region: Rect, mut cache: Option<&mut Cache>) -> Result<Source, Error> {
        let image = &image.readable();
        let extent = image.extent();
        if region.is_empty() || !extent.contains(&region) {
            return Err(Error::RegionOutside { region, extent });
        }
        let plan = Plan::new(image, region, cache.as_deref())?;
        if !self.fits(&plan) {
            drop(plan);
            let pixels = self.render(image, region, cache)?;
            return self.upload_at(region, &pixels);
        }
        if let Some(c) = cache.as_deref_mut() {
            c.begin(&plan.nodes);
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (texture, rect) = self.evaluate(&mut encoder, plan, cache.as_deref_mut(), true);
        // The last stage wrote straight into a texture of its own; a pooled one is copied out.
        let owned = if texture.owned() && rect == region {
            texture
        } else {
            let owned = self.pool.output(region.w, region.h);
            self.copy(&mut encoder, &texture, rect, &owned, region, region);
            self.pool.release(texture);
            owned
        };
        self.submit(encoder);
        self.settle(cache);
        self.ended();
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: region, tiles: vec![(region, owned)] })))
    }

    /// Ends a render: the pool frees what no render took again, and the device lets go of what the GPU
    /// has finished with, which it otherwise keeps (textures destroyed, their zero fills) until polled.
    pub(super) fn ended(&mut self) {
        self.pool.end_render();
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Hands the pool what the cache let go of after a render.
    fn settle(&mut self, cache: Option<&mut Cache>) {
        for texture in cache.map(Cache::settle).unwrap_or_default() {
            self.pool.release(texture);
        }
    }

    pub(super) fn fits(&self, plan: &Plan) -> bool {
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
    pub(super) fn render_piece(
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
        let (texture, rect) = self.evaluate(&mut encoder, plan, cache, false);
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
        self.submit(encoder);
        self.pool.release(texture);
        let slice = readback.slice(..);
        mapped(&self.device, &slice)?;
        {
            let mapped = slice.get_mapped_range().map_err(|e| Error::Lost(e.to_string()))?;
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
    /// once its last reader has run; returns the last node's result and the rectangle it covers, in a
    /// texture of its own when `owned_last` asks it.
    pub(super) fn evaluate(&mut self, encoder: &mut wgpu::CommandEncoder, plan: Plan, cache: Option<&mut Cache>, owned_last: bool) -> (FloatTexture, Rect) {
        let ready = self.lay(encoder, plan);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("stages"), timestamp_writes: None });
        let last = if owned_last { Last::Owned } else { Last::Pooled };
        let result = self.record(&mut pass, ready, cache, last);
        drop(pass);
        result.expect("the last node is evaluated")
    }

    /// The plan with its sources laid, their copies outside the pass: every stage then runs in one
    /// compute pass, as each pass begun costs Metal a command buffer and its driver memory.
    pub(super) fn lay<'a>(&mut self, encoder: &mut wgpu::CommandEncoder, plan: Plan<'a>) -> Ready<'a> {
        let Plan { nodes, last, rects, hits, runs, shown } = plan;
        let n = nodes.len();
        let mut pending = vec![0usize; n];
        for i in 0..n {
            if let (Some(_), None) = (rects[i], &hits[i]) {
                for j in read_by(&nodes[i].kind, runs[i]) {
                    pending[j] += 1;
                }
            }
        }
        let mut results = hits;
        for i in 0..n {
            if let (Some(need), None, Kind::Source(source)) = (rects[i], &results[i], &nodes[i].kind) {
                results[i] = Some(self.assemble(encoder, source, need));
            }
        }
        Ready { nodes, last, rects, results, pending, runs, shown }
    }

    /// Dispatches every stage of `ready` into `pass`; the last node's result, unless it went into a sheet.
    pub(super) fn record(&mut self, pass: &mut wgpu::ComputePass<'_>, ready: Ready, mut cache: Option<&mut Cache>, last_into: Last) -> Option<(FloatTexture, Rect)> {
        let Ready { nodes, last, rects, mut results, mut pending, runs, shown } = ready;
        for i in 0..nodes.len() {
            let Some(need) = rects[i] else { continue };
            if results[i].is_some() {
                continue;
            }
            let (input, aux): (usize, &[usize]) = match &nodes[i].kind {
                Kind::Stage { input, aux, .. } => (*input, aux),
                Kind::Laid { input, .. } => (*input, &[]),
                Kind::Source(_) => unreachable!("every source is laid before the pass"),
            };
            let extent = nodes[input].extent;
            let (out, rect) = match (runs[i], &nodes[i].kind) {
                (Run::Whole, Kind::Stage { stage, .. }) => {
                    let (out, at) = match (&last_into, i == last) {
                        (Last::Sheet(sheet, at), true) => ((*sheet).clone(), *at),
                        (Last::Owned, true) => (self.pool.output(need.w, need.h), [0; 2]),
                        _ => (self.pool.acquire(need.w, need.h), [0; 2]),
                    };
                    let main = results[input].as_ref().expect("the input is evaluated first");
                    let extras = auxiliaries(&nodes, &results, aux);
                    self.dispatch(pass, *stage, (&main.0, main.1, extent), &extras, (&out, at), need);
                    (out, need)
                }
                (Run::Through, _) => results[input].clone().expect("the input is evaluated first"),
                (Run::Within(hit), Kind::Stage { stage, .. }) => {
                    let (onto, rect) = self.onto(pass, &mut results, &pending, input, need, extent);
                    let part = self.pool.acquire(hit.w, hit.h);
                    let extras = auxiliaries(&nodes, &results, aux);
                    self.dispatch(pass, *stage, (&onto, rect, extent), &extras, (&part, [0; 2]), hit);
                    self.dispatch(pass, &Put, (&part, hit, hit), &[], (&onto, [hit.x - rect.x, hit.y - rect.y]), hit);
                    self.pool.release(part);
                    (onto, rect)
                }
                (Run::Laid, Kind::Laid { laid, .. }) => {
                    let (onto, rect) = self.onto(pass, &mut results, &pending, input, need, extent);
                    self.lay_on(pass, laid, &shown[&i], (&onto, rect), extent);
                    (onto, rect)
                }
                _ => unreachable!("a run is planned for its node's kind"),
            };
            // A sheet takes a laid image as the last stage of any other writes it: at its place.
            if let (Last::Sheet(sheet, at), true, Kind::Laid { .. }) = (&last_into, i == last, &nodes[i].kind) {
                self.dispatch(pass, &Put, (&out, rect, nodes[i].extent), &[], (sheet, *at), need);
            }
            // A result lent by its input is kept only where its texture starts at the node's own rectangle.
            if let Some(c) = cache.as_deref_mut().filter(|_| rect == need)
                && let Some(old) = c.store(nodes[i].key, need, &out)
            {
                self.pool.release(old);
            }
            match (&last_into, i == last) {
                // Written on the sheet: what a laid image was built on goes back to the pool.
                (Last::Sheet(..), true) => self.pool.release(out),
                _ => results[i] = Some((out, rect)),
            }
            for j in read_by(&nodes[i].kind, runs[i]) {
                pending[j] -= 1;
                if pending[j] == 0 && j != last
                    && let Some((texture, _)) = results[j].take()
                {
                    self.pool.release(texture);
                }
            }
        }
        results[last].take()
    }

    /// The texture a confined node writes onto, and the rectangle it holds: its input's own when this
    /// node reads it last and nothing else holds it, else a copy of `need`.
    fn onto(&mut self, pass: &mut wgpu::ComputePass<'_>, results: &mut [Option<(FloatTexture, Rect)>], pending: &[usize], input: usize, need: Rect, extent: Rect) -> (FloatTexture, Rect) {
        if pending[input] == 1 && results[input].as_ref().is_some_and(|(t, _)| t.alone()) {
            return results[input].take().expect("the input is evaluated first");
        }
        let copy = self.pool.acquire(need.w, need.h);
        let (t, r) = results[input].as_ref().expect("the input is evaluated first");
        self.dispatch(pass, &Put, (t, *r, extent), &[], (&copy, [0; 2]), need);
        (copy, need)
    }

    /// The source pixels of `needed`: a tile itself when one holds them all, else a copy across tiles.
    pub(super) fn assemble(&mut self, encoder: &mut wgpu::CommandEncoder, source: &Source, needed: Rect) -> (FloatTexture, Rect) {
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
    pub(super) fn copy(&mut self, encoder: &mut wgpu::CommandEncoder, from: &FloatTexture, from_rect: Rect, to: &FloatTexture, to_rect: Rect, part: Rect) {
        let at = |r: Rect| wgpu::Origin3d { x: (part.x - r.x) as u32, y: (part.y - r.y) as u32, z: 0 };
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo { texture: from.texture(), mip_level: 0, origin: at(from_rect), aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyTextureInfo { texture: to.texture(), mip_level: 0, origin: at(to_rect), aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: part.w, height: part.h, depth_or_array_layers: 1 },
        );
        self.copies += 1;
    }
}

/// A plan whose sources are laid, ready to record.
pub(super) struct Ready<'a> {
    nodes: Vec<Flat<'a>>,
    pub(super) last: usize,
    rects: Vec<Option<Rect>>,
    pub(super) results: Vec<Option<(FloatTexture, Rect)>>,
    pending: Vec<usize>,
    runs: Vec<Run>,
    shown: HashMap<usize, Vec<(usize, Rect)>>,
}

/// The nodes a node reads as it runs: its input, and its auxiliary inputs unless it runs through.
fn read_by<'k>(kind: &'k Kind, run: Run) -> impl Iterator<Item = usize> + 'k {
    kind.reads().enumerate().filter(move |(place, _)| *place == 0 || run != Run::Through).map(|(_, node)| node)
}

/// Each auxiliary input's result: its texture, the rectangle it covers and its extent.
fn auxiliaries<'r>(nodes: &[Flat], results: &'r [Option<(FloatTexture, Rect)>], aux: &[usize]) -> Vec<(&'r FloatTexture, Rect, Rect)> {
    aux.iter()
        .map(|&a| {
            let (t, r) = results[a].as_ref().expect("an auxiliary input is evaluated first");
            (t, *r, nodes[a].extent)
        })
        .collect()
}

/// Its input where it is dispatched: a confined stage's part laid onto its input's pixels, or those
/// pixels copied where another reader still needs them as they are.
pub(super) struct Put;

impl Stage for Put {
    fn name(&self) -> &'static str {
        "put"
    }

    fn shader(&self) -> &'static str {
        "fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p); }"
    }
}

/// Where the last node of a plan is written.
pub(super) enum Last<'a> {
    Pooled,
    Owned,
    /// At this corner of a sheet the caller holds.
    Sheet(&'a FloatTexture, [i32; 2]),
}
