//! The engine: the GPU device, the pipeline on it, its gate, and the frame kept as its preview.

use super::*;

/// A frame's preview, resident on the GPU, and the full width it was reduced from.
pub struct Negative {
    pub preview: Source,
    pub full_width: f64,
    pub undeclared: Option<bool>,
}

impl Negative {
    pub fn size(&self) -> (u32, u32) {
        let e = self.preview.extent();
        (e.w, e.h)
    }
}

/// `RawDecode.rebuildingEdge`: a reduced decode's two outer rows and columns taken from the third,
/// because scaling darkens the border. Follows a crop to `inner`, and reads only inside it.
pub(crate) struct Edge {
    extent: Rect,
    inner: Rect,
}

impl Stage for Edge {
    fn name(&self) -> &'static str {
        "edge"
    }

    fn shader(&self) -> &'static str {
        "fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p); }"
    }

    fn params(&self) -> Vec<u8> {
        let (e, i) = (self.extent, self.inner);
        bytemuck::bytes_of(&[e.x, e.y, e.w as i32, e.h as i32, i.x, i.y, i.w as i32, i.h as i32]).to_vec()
    }

    fn extent(&self, _input: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, output: Rect) -> Rect {
        let i = self.inner;
        let x0 = output.x.clamp(i.x, i.right() - 1);
        let y0 = output.y.clamp(i.y, i.bottom() - 1);
        Rect::from_corners(x0, y0, output.right().clamp(x0 + 1, i.right()), output.bottom().clamp(y0 + 1, i.bottom()))
    }
}

pub(crate) fn rebuilt_edge(image: Image) -> Image {
    let e = image.extent();
    if e.w <= 5 || e.h <= 5 {
        return image;
    }
    let inner = Rect::new(e.x + 2, e.y + 2, e.w - 4, e.h - 4);
    image.then(Crop(inner)).then(Edge { extent: e, inner })
}

/// A wgpu device the whole app shares: the render graph, the display kernel and the window's canvas.
pub struct Device {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Device {
    /// The high-performance adapter at its own limits: the defaults stop textures at 8 192 px.
    pub fn open() -> Result<Device, String> {
        let instance = wgpu::Instance::default();
        let options = wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() };
        let adapter = pollster::block_on(instance.request_adapter(&options)).map_err(|e| format!("no GPU adapter: {e}"))?;
        let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), ..Default::default() };
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).map_err(|e| format!("no GPU device: {e}"))?;
        // Logged, not raised: a surface reconfigure that loses a race keeps its old size until the next frame.
        device.on_uncaptured_error(Arc::new(|e: wgpu::Error| crate::timing::line(&format!("wgpu error: {e}"))));
        Ok(Device { instance, adapter, device, queue })
    }
}

/// The pipeline and the graph it records on, one per app, shared by the canvas and the thumbnails.
pub struct Engine {
    pub(super) graph: Graph,
    pipeline: Pipeline,
    /// The open frame's preview reduced to the canvas: its source's id, its long side, its pixels.
    fitted: Option<(u64, usize, Source)>,
    /// Laid after the pipeline on every render the editor shows, never on an export.
    pub proof: crate::proof::Proofing,
    /// Where the custom flat-field masks are kept, and those read so far, `None` once one is gone.
    pub support: Option<PathBuf>,
    custom: HashMap<String, Option<CustomFlatField>>,
    /// Where the dust patches are kept, and those read so far, `None` while one is not generated.
    pub caches: Option<PathBuf>,
    patches: HashMap<Id, Option<Source>>,
    /// `Negative.measure` and `measureSmall` of the preview whose id leads: what the input
    /// histograms read, the small one under the hand.
    measures: Option<(u64, Source, Source)>,
}

/// The engine every thread shares, behind a gate the interface thread closes while the window's
/// surface is reconfigured: wgpu refuses a reconfigure that another thread submits work during.
#[derive(Clone)]
pub struct Shared(Arc<Gated>);

pub(crate) struct Gated {
    engine: Mutex<Engine>,
    gate: Mutex<Gate>,
    turn: Condvar,
}

#[derive(Default)]
pub(crate) struct Gate {
    held: bool,
    inside: usize,
}

/// The engine, locked by a thread the gate let through.
pub struct EngineGuard<'a> {
    engine: Option<MutexGuard<'a, Engine>>,
    gated: &'a Gated,
}

impl Deref for EngineGuard<'_> {
    type Target = Engine;
    fn deref(&self) -> &Engine {
        self.engine.as_ref().expect("held until dropped")
    }
}

impl DerefMut for EngineGuard<'_> {
    fn deref_mut(&mut self) -> &mut Engine {
        self.engine.as_mut().expect("held until dropped")
    }
}

impl Drop for EngineGuard<'_> {
    fn drop(&mut self) {
        self.engine.take();
        self.gated.leave();
    }
}

impl Gated {
    fn leave(&self) {
        if let Ok(mut g) = self.gate.lock() {
            g.inside -= 1;
        }
        self.turn.notify_all();
    }
}

impl Shared {
    /// Waits while the gate is held, then for the engine.
    pub fn lock(&self) -> Result<EngineGuard<'_>, String> {
        let gated = &*self.0;
        {
            let mut g = gated.gate.lock().map_err(|_| "the render gate failed earlier")?;
            while g.held {
                g = gated.turn.wait(g).map_err(|_| "the render gate failed earlier")?;
            }
            g.inside += 1;
        }
        match gated.engine.lock() {
            Ok(engine) => Ok(EngineGuard { engine: Some(engine), gated }),
            Err(_) => {
                gated.leave();
                Err("the render engine failed earlier".into())
            }
        }
    }

    /// Closes the gate and waits for the threads inside to leave, so nothing submits until `release`.
    pub fn hold(&self) {
        let gated = &*self.0;
        let Ok(mut g) = gated.gate.lock() else { return };
        g.held = true;
        while g.inside > 0 {
            match gated.turn.wait(g) {
                Ok(next) => g = next,
                Err(_) => return,
            }
        }
    }

    pub fn release(&self) {
        if let Ok(mut g) = self.0.gate.lock() {
            g.held = false;
        }
        self.0.turn.notify_all();
    }
}

impl Engine {
    pub fn on(d: &Device) -> Engine {
        let info = d.adapter.get_info();
        let name = format!("{} ({:?})", info.name, info.backend);
        Engine { graph: Graph::from_device(d.device.clone(), d.queue.clone(), name, oni_graph::TEXTURE_SIDE), pipeline: Pipeline::new(), fitted: None, proof: Default::default(), support: None, custom: HashMap::new(), caches: None, patches: HashMap::new(), measures: None }
    }

    pub fn shared(d: &Device) -> Shared {
        Shared(Arc::new(Gated { engine: Mutex::new(Engine::on(d)), gate: Mutex::new(Gate::default()), turn: Condvar::new() }))
    }

    /// The graph and the pipeline, for a render the canvas never shows: an export reads them unproofed.
    /// `refreshLevelsInput` and `refreshManualCurvesInput`: the distributions in front of the levels
    /// and of the manual curves, read through the measuring windows, on the small copy while `held`.
    pub fn inputs(&mut self, negative: &Negative, settings: &PipelineSettings, held: bool) -> Option<InputHistograms> {
        use oni_measure::reader::{Copy, Reader};
        use oni_measure::truncate::Stage;
        let id = negative.preview.id();
        if self.measures.as_ref().is_none_or(|(kept, _, _)| *kept != id) {
            let mut r = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline };
            let measure = r.measuring(&negative.preview)?;
            // `measureSmall`: a third of the measuring copy's side, as `CILanczosScaleTransform` makes it.
            let small = self.graph.materialise(&Image::new(&measure).lanczos_anchored(MEASURE_SMALL)).ok()?;
            self.measures = Some((id, measure, small));
        }
        let (_, measure, small) = self.measures.as_ref()?;
        let source = if held { small.clone() } else { measure.clone() };
        let copy = || Copy { source: &source, full_width: Some(negative.full_width) };
        let mut r = Reader { graph: &mut self.graph, pipeline: &mut self.pipeline };
        let levels = r.reading(copy(), settings, Stage::Levels).histogram;
        let curves = r.reading(copy(), settings, Stage::ManualCurves).histogram;
        self.graph.purge();
        Some(InputHistograms { levels, curves })
    }

    pub fn parts(&mut self) -> (&mut Graph, &mut Pipeline) {
        (&mut self.graph, &mut self.pipeline)
    }

    pub fn adapter(&self) -> &str {
        &self.graph.adapter
    }

    /// The decode laid on the GPU, reduced by Lanczos to the preview side, its edge rebuilt and
    /// rendered once, so a notch starts from resident pixels.
    pub fn prepare(&mut self, decoded: Decoded) -> Result<Negative, String> {
        let l = decoded.linear;
        let source = self.graph.upload(l.width as u32, l.height as u32, &l.pixels).map_err(|e| e.to_string())?;
        drop(l.pixels);
        let mut image = Image::new(&source);
        let long = l.width.max(l.height);
        if long > PREVIEW_SIDE {
            image = image.lanczos(PREVIEW_SIDE as f64 / long as f64);
        }
        let preview = self.graph.materialise(&rebuilt_edge(image)).map_err(|e| e.to_string())?;
        drop(source);
        self.graph.purge();
        Ok(Negative { preview, full_width: decoded.full_width, undeclared: decoded.undeclared })
    }

    /// `RawDecode.linear` at the preview's side as pixels: the decode reduced and its edge rebuilt
    /// as a frame's preview is, read back whole.
    pub fn preview_pixels(&mut self, decoded: Decoded) -> Result<Linear, String> {
        let negative = self.prepare(decoded)?;
        let image = Image::new(&negative.preview);
        let region = image.extent();
        let pixels = self.graph.render(&image, region, None).map_err(|e| e.to_string())?;
        Ok(Linear { width: region.w as usize, height: region.h as usize, pixels })
    }

    /// `Pipeline.apply` on the preview, reduced first to a long side of `side` when given.
    pub fn render(&mut self, negative: &Negative, settings: &PipelineSettings, side: Option<usize>) -> Result<Linear, String> {
        let (w, h) = negative.size();
        let long = w.max(h) as f64;
        let scale = side.map_or(1.0, |s| (s as f64 / long).min(1.0));
        let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let region = image.extent();
        let pixels = self.graph.render(&image, region, None).map_err(|e| e.to_string())?;
        Ok(Linear { width: region.w as usize, height: region.h as usize, pixels })
    }

    /// A frame's full-resolution decode laid on the GPU as it is, for a zoom past the preview.
    pub fn full(&mut self, decoded: Decoded) -> Result<Source, String> {
        let l = decoded.linear;
        self.graph.upload(l.width as u32, l.height as u32, &l.pixels).map_err(|e| e.to_string())
    }

    /// `Pipeline.apply` left on the GPU in one texture for the display kernel to read. With a view,
    /// only its visible region, from the preview or past its ratio from the full frame, each reduced
    /// once to the scale shown, so a notch computes no hidden pixel.
    pub fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Source, Option<Placed>), String> {
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        let Some(view) = view else {
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
            let image = self.proof.laid(&mut self.graph, image, settings)?;
            return Ok((self.one_texture(&image, None)?, None));
        };
        let ratio = negative.size().0 as f64 / negative.full_width;
        let shown = {
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?.extent()
        };
        let full = (shown.w as f64 / ratio, shown.h as f64 / ratio);
        let vp = view.viewport;
        let scale = vp.effective_scale(full, view.size);
        let placement = vp.placement(full, view.size);
        let at_full = view.settled && vp.needs_full_resolution(ratio, full, view.size, crate::viewport::NOMINAL_BUDGET);
        let (base, base_ratio) = match (&view.full, at_full) {
            (Some(f), true) => (f.clone(), 1.0),
            _ => (negative.preview.clone(), ratio),
        };
        let e = base.extent();
        let long = e.w.max(e.h) as usize;
        let side = ((long as f64 * (scale / base_ratio).min(1.0)).round() as usize).max(1);
        let source = if side < long { self.fitted(&base, side, long)? } else { base };
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, &source, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let extent = image.extent();
        let stretch = scale * full.0 / extent.w as f64;
        let (vx, vy, vw, vh) = placement.visible;
        let corner = |v: f64, origin: i32, f: fn(f64) -> f64| origin + f(v / stretch) as i32;
        let region = Rect::from_corners(
            corner(vx, extent.x, f64::floor).max(extent.x),
            corner(vy, extent.y, f64::floor).max(extent.y),
            corner(vx + vw, extent.x, f64::ceil).min(extent.right()),
            corner(vy + vh, extent.y, f64::ceil).min(extent.bottom()),
        );
        if region.is_empty() {
            return Err("the image is off the canvas".into());
        }
        let placed = Placed {
            x: placement.image.0 + f64::from(region.x - extent.x) * stretch,
            y: placement.image.1 + f64::from(region.y - extent.y) * stretch,
            width: f64::from(region.w) * stretch,
            height: f64::from(region.h) * stretch,
            full,
            whole: region == extent,
            at_full_resolution: at_full && view.full.is_some(),
        };
        Ok((self.one_texture(&image, Some(region))?, Some(placed)))
    }

    /// `Pipeline.correctionSource` read back where a search for `c` looks: the frame at full
    /// resolution, framed with every other correction healed, and the framed extent's size.
    pub fn search_region(&mut self, full: &Source, settings: &PipelineSettings, c: &oni_settings::frame::Correction) -> Result<(crate::heal::Region, (f64, f64)), String> {
        let mut upstream = settings.clone();
        upstream.corrections.retain(|other| other.id != c.id);
        self.read_custom(&upstream);
        let inputs = Inputs { patches: patches_for(&self.patches, &upstream), custom_flat_field: custom_for(&self.custom, &upstream) };
        let framed = self.pipeline.framed(&mut self.graph, Image::new(full), &upstream, 1.0, &inputs).map_err(|e| e.to_string())?;
        let e = framed.extent();
        let size = (f64::from(e.w), f64::from(e.h));
        let win = crate::heal::search_window(c, size.0, size.1);
        let region = Rect::new(e.x + win[0] as i32, e.y + win[1] as i32, win[2] as u32, win[3] as u32);
        let pixels = self.graph.render(&framed, region, None).map_err(|e| e.to_string())?;
        self.graph.purge();
        let r = crate::heal::Region { x: win[0] as i64, y: win[1] as i64, width: region.w as usize, height: region.h as usize, pixels, frame_width: size.0, frame_height: size.1 };
        Ok((r, size))
    }

    /// `Loupe.recipe`: the pipeline on the full-resolution frame FIRST, the region second, since
    /// the neighbourhood radii index on the incoming extent; `None` on an unlaid band.
    pub fn loupe(&mut self, full: &Source, settings: &PipelineSettings, viewport: &crate::viewport::Viewport, drawable: (f64, f64), magnification: f64) -> Result<Option<(Linear, Rect)>, String> {
        self.read_custom(settings);
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, full, settings, &Request::default(), &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let Some(zone) = crate::loupe::region(image.extent(), viewport, drawable, magnification) else { return Ok(None) };
        let pixels = self.graph.render(&image, zone, None).map_err(|e| e.to_string())?;
        Ok(Some((Linear { width: zone.w as usize, height: zone.h as usize, pixels }, zone)))
    }

    /// Forgets a correction's patch, so the next render reads it again from the cache.
    pub fn forget_patch(&mut self, id: Id) {
        self.patches.remove(&id);
    }

    /// Each correction's patch from the cache, read and uploaded once; one not generated yet is left
    /// out, and asked for again on the next render.
    fn read_patches(&mut self, settings: &PipelineSettings) {
        let Some(caches) = self.caches.clone() else { return };
        for c in &settings.corrections {
            if self.patches.get(&c.id).is_some_and(Option::is_some) {
                continue;
            }
            let read = crate::patches::read(&caches, c.id).and_then(|(w, h, pixels)| self.graph.upload(w, h, &pixels).ok());
            self.patches.insert(c.id, read);
        }
    }

    /// `Pipeline.apply` unproofed, with the masks and patches the canvas reads: an export's render.
    pub fn unproofed(&mut self, source: &Source, settings: &PipelineSettings, request: &Request) -> Result<Image, String> {
        self.read_custom(settings);
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        self.pipeline.image(&mut self.graph, source, settings, request, &inputs).map_err(|e| e.to_string())
    }

    /// The custom mask the settings name, read and uploaded once per id.
    fn read_custom(&mut self, settings: &PipelineSettings) {
        self.read_patches(settings);
        let f = &settings.flat_field;
        let (Some(id), Some(support)) = (f.custom.as_ref().filter(|_| f.mask == FlatFieldMask::Custom), self.support.as_ref()) else { return };
        // A miss is asked again, as Swift keeps only hits: a mask loaded later reaches the frames naming it.
        if self.custom.get(id).is_some_and(Option::is_some) {
            return;
        }
        let read = crate::flat_fields::read(support, id).and_then(|m| {
            let mask = self.graph.upload(m.width, m.height, &m.pixels).ok()?;
            Some(CustomFlatField { id: id.clone(), mask, peaks: m.peaks, per_channel: m.per_channel })
        });
        self.custom.insert(id.clone(), read);
    }

    fn one_texture(&mut self, image: &Image, region: Option<Rect>) -> Result<Source, String> {
        let source = match region {
            Some(r) if r != image.extent() => self.graph.materialise_region(image, r),
            _ => self.graph.materialise(image),
        }
        .map_err(|e| e.to_string())?;
        if source.view().is_none() {
            return Err("the render is wider than one texture".into());
        }
        Ok(source)
    }

    fn fitted(&mut self, base: &Source, side: usize, long: usize) -> Result<Source, String> {
        let id = base.id();
        if let Some((kept, s, source)) = &self.fitted
            && (*kept, *s) == (id, side)
        {
            return Ok(source.clone());
        }
        self.fitted = None;
        let reduced = Image::new(base).lanczos(side as f64 / long as f64);
        let source = self.graph.materialise(&reduced).map_err(|e| e.to_string())?;
        self.fitted = Some((id, side, source.clone()));
        Ok(source)
    }
}

/// The patches of the settings' own corrections that have been generated.
pub(crate) fn patches_for<'a>(kept: &'a HashMap<Id, Option<Source>>, settings: &PipelineSettings) -> HashMap<Id, &'a Source> {
    settings.corrections.iter().filter_map(|c| Some((c.id, kept.get(&c.id)?.as_ref()?))).collect()
}

/// The custom mask read for the settings' own id, which the pipeline lays only when the ids match.
pub(crate) fn custom_for<'a>(kept: &'a HashMap<String, Option<CustomFlatField>>, settings: &PipelineSettings) -> Option<&'a CustomFlatField> {
    kept.get(settings.flat_field.custom.as_deref()?)?.as_ref()
}
