//! The engine: the GPU device, the pipeline on it, its gate, and the frame kept as its preview.

use super::*;

mod inputs;
pub(crate) mod prewarm;
mod originals;
mod tiles;
mod view;

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
        // Clearing a texture in place lets a sheet laid every notch reuse the last one's.
        let required_features = adapter.features() & wgpu::Features::CLEAR_TEXTURE;
        let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), required_features, ..Default::default() };
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
    /// The open frame reduced to the canvas, and the halvings it was reduced from.
    fitted: view::Fitting,
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
    /// The last levels and curves readings, kept while what enters each stage holds.
    read: [Option<inputs::Read>; 2],
    /// The canvas render's stage results the next notch starts from.
    cache: oni_graph::Cache,
}

/// The engine every thread shares, behind a gate the interface thread closes while the window's
/// surface is reconfigured: wgpu refuses a reconfigure that another thread submits work during.
#[derive(Clone)]
pub struct Shared(Arc<Gated>);

pub(crate) struct Gated {
    engine: Mutex<Engine>,
    door: Arc<Door>,
    uploader: oni_graph::Uploader,
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
        self.gated.door.leave();
    }
}

impl Shared {
    /// Waits while the gate is held, then for the engine.
    pub fn lock(&self) -> Result<EngineGuard<'_>, String> {
        let gated = &*self.0;
        gated.door.enter()?;
        match gated.engine.lock() {
            Ok(engine) => Ok(EngineGuard { engine: Some(engine), gated }),
            Err(_) => {
                gated.door.leave();
                Err("the render engine failed earlier".into())
            }
        }
    }

    /// The engine's device and door made the whole app's: every other graph records on that device.
    pub fn adopt(&self) -> Result<(), String> {
        let device = self.lock()?.device();
        door::adopt(device, self.0.door.clone());
        Ok(())
    }

    /// The decode laid on the GPU through the gate alone: a 24 MP upload holds the engine about
    /// 100 ms, which every render asked meanwhile would wait.
    pub fn upload(&self, l: &Linear) -> Result<Source, String> {
        self.0.door.enter()?;
        let laid = self.0.uploader.upload(l.width as u32, l.height as u32, &l.pixels).map_err(|e| e.to_string());
        self.0.door.leave();
        laid
    }

    /// `Engine::prepare` with the upload outside the engine.
    pub fn prepare(&self, decoded: Decoded) -> Result<Negative, String> {
        self.prepare_keeping(decoded).map(|(negative, _)| negative)
    }

    /// `prepare`, and the decode laid as it is when it was the frame's full resolution, so the full
    /// frame costs no second decode.
    pub fn prepare_keeping(&self, decoded: Decoded) -> Result<(Negative, Option<Source>), String> {
        let source = self.upload(&decoded.linear)?;
        let (w, h) = (decoded.linear.width, decoded.linear.height);
        drop(decoded.linear);
        let full = (w as f64 == decoded.full_width).then(|| source.clone());
        Ok((self.lock()?.reduce(source, (w, h), decoded.full_width, decoded.undeclared)?, full))
    }

    /// A frame's full-resolution decode laid on the GPU as it is, for a zoom past the preview.
    pub fn full(&self, decoded: Decoded) -> Result<Source, String> {
        self.upload(&decoded.linear)
    }

    /// Closes the gate and waits for the threads inside to leave, so nothing submits until `release`.
    pub fn hold(&self) {
        self.0.door.hold();
    }

    pub fn release(&self) {
        self.0.door.release();
    }
}

impl Engine {
    pub fn on(d: &Device) -> Engine {
        let info = d.adapter.get_info();
        let name = format!("{} ({:?})", info.name, info.backend);
        Engine { graph: Graph::from_device(d.device.clone(), d.queue.clone(), name, oni_graph::TEXTURE_SIDE), pipeline: Pipeline::new(), fitted: Default::default(), proof: Default::default(), support: None, custom: HashMap::new(), caches: None, patches: HashMap::new(), measures: None, read: [None, None], cache: oni_graph::Cache::frontier() }
    }

    pub fn shared(d: &Device) -> Shared {
        let engine = Engine::on(d);
        let uploader = engine.graph.uploader();
        Shared(Arc::new(Gated { engine: Mutex::new(engine), door: Arc::default(), uploader }))
    }

    /// What only the open frame's editor uses, dropped once it is left: the fitted copy, the
    /// measuring copies and the pool's idle textures, a few hundred MB on a 24 MP frame.
    pub fn trim(&mut self) {
        self.fitted = Default::default();
        self.measures = None;
        self.read = [None, None];
        self.cache.clear();
        self.graph.purge();
    }

    /// The graph and the pipeline, for a render the canvas never shows: an export reads them unproofed.
    pub fn parts(&mut self) -> (&mut Graph, &mut Pipeline) {
        (&mut self.graph, &mut self.pipeline)
    }

    pub fn device(&self) -> (wgpu::Device, wgpu::Queue) {
        self.graph.device()
    }

    pub fn stats(&self) -> oni_graph::Stats {
        self.graph.stats()
    }

    pub fn adapter(&self) -> &str {
        &self.graph.adapter
    }

    /// The decode laid on the GPU, reduced by Lanczos to the preview side, its edge rebuilt and
    /// rendered once, so a notch starts from resident pixels.
    pub fn prepare(&mut self, decoded: Decoded) -> Result<Negative, String> {
        let l = decoded.linear;
        let source = self.graph.upload_half(l.width as u32, l.height as u32, &l.pixels).map_err(|e| e.to_string())?;
        drop(l.pixels);
        self.reduce(source, (l.width, l.height), decoded.full_width, decoded.undeclared)
    }

    /// The laid decode of `size` reduced to the preview, its edge rebuilt and rendered once.
    pub fn reduce(&mut self, source: Source, size: (usize, usize), full_width: f64, undeclared: Option<bool>) -> Result<Negative, String> {
        let mut image = Image::new(&source);
        let long = size.0.max(size.1);
        if long > PREVIEW_SIDE {
            image = image.lanczos(PREVIEW_SIDE as f64 / long as f64);
        }
        let preview = self.graph.materialise_half(&rebuilt_edge(image)).map_err(|e| e.to_string())?;
        drop(source);
        self.graph.purge();
        Ok(Negative { preview, full_width, undeclared })
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

    /// A frame's full-resolution decode laid on the GPU as it is, for a zoom past the preview.
    pub fn full(&mut self, decoded: Decoded) -> Result<Source, String> {
        let l = decoded.linear;
        self.graph.upload_half(l.width as u32, l.height as u32, &l.pixels).map_err(|e| e.to_string())
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
        let r = crate::heal::Region { x: win[0] as i64, y: win[1] as i64, width: region.w as usize, height: region.h as usize, pixels, frame_width: size.0, frame_height: size.1 };
        Ok((r, size))
    }

    /// `Loupe.recipe`: the pipeline on the full-resolution frame FIRST, the region second, since
    /// the neighbourhood radii index on the incoming extent; kept on the GPU for the display kernel,
    /// `None` on an unlaid band.
    pub fn loupe(&mut self, full: &Source, settings: &PipelineSettings, viewport: &crate::viewport::Viewport, drawable: (f64, f64), magnification: f64) -> Result<Option<(Source, Rect)>, String> {
        self.read_custom(settings);
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, full, settings, &Request::default(), &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let Some(zone) = crate::loupe::region(image.extent(), viewport, drawable, magnification) else { return Ok(None) };
        let source = self.graph.materialise_region(&image, zone).map_err(|e| e.to_string())?;
        Ok(Some((source, zone)))
    }

    /// Frees the textures a search left idle.
    pub fn purge(&mut self) {
        self.graph.purge();
    }

    /// Forgets a correction's patch, so the next render and input reading read it again from the cache.
    pub fn forget_patch(&mut self, id: Id) {
        self.patches.remove(&id);
        self.read = [None, None];
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

}

/// The patches of the settings' own corrections that have been generated.
pub(crate) fn patches_for<'a>(kept: &'a HashMap<Id, Option<Source>>, settings: &PipelineSettings) -> HashMap<Id, &'a Source> {
    settings.corrections.iter().filter_map(|c| Some((c.id, kept.get(&c.id)?.as_ref()?))).collect()
}

/// The custom mask read for the settings' own id, which the pipeline lays only when the ids match.
pub(crate) fn custom_for<'a>(kept: &'a HashMap<String, Option<CustomFlatField>>, settings: &PipelineSettings) -> Option<&'a CustomFlatField> {
    kept.get(settings.flat_field.custom.as_deref()?)?.as_ref()
}
