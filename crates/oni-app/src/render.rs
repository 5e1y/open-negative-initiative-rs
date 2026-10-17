//! Decoding and rendering off the interface thread: a frame decoded once is kept on the GPU as its
//! preview, and a newer job replaces the one waiting in the render slot, so a drag never queues.

use crate::display::{self, Params, Screen};
use oni_graph::stages::Crop;
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::PipelineSettings;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// `Negative.previewSide`: the long side every editor render and thumbnail starts from.
pub const PREVIEW_SIDE: usize = 2000;

/// Scene-linear Rec. 2020, row-major, alpha 1.
#[derive(Clone)]
pub struct Linear {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

/// A file read on the CPU: the whole frame, or a linear scan already reduced while reading.
pub struct Decoded {
    pub linear: Linear,
    /// The frame's full-resolution width, which the dosed radii and the crop are measured on.
    pub full_width: f64,
}

/// `RawDecode.alone` on its preview: the branch the bytes claim, the preview side asked of it.
/// A linear scan measured on converted primaries and a plain image are refused, never approximated.
pub fn decode(path: &Path, primaries_converted: bool) -> Result<Decoded, String> {
    match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => {
            let d = oni_decode::decode(path, 1.0).map_err(|e| format!("LibRaw cannot decode it (code {})", e.0))?;
            let full_width = d.width as f64;
            Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width })
        }
        oni_decode::Branch::LinearRgb => {
            if primaries_converted {
                return Err("a linear scan measured on converted primaries is not read yet".into());
            }
            let (full, _) = oni_decode::container::pixel_size(path).ok_or("its container is unreadable")?;
            let d = oni_decode::container::linear(path, Some(PREVIEW_SIDE as f64)).ok_or("its samples are unreadable")?;
            Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width: full as f64 })
        }
        oni_decode::Branch::Plain => Err("a plain image (JPEG, PNG, RGB TIFF) is not read yet".into()),
    }
}

/// A frame's preview, resident on the GPU, and the full width it was reduced from.
pub struct Negative {
    pub preview: Source,
    pub full_width: f64,
}

impl Negative {
    pub fn size(&self) -> (u32, u32) {
        let e = self.preview.extent();
        (e.w, e.h)
    }
}

/// `RawDecode.rebuildingEdge`: a reduced decode's two outer rows and columns taken from the third,
/// because scaling darkens the border. Follows a crop to `inner`, and reads only inside it.
struct Edge {
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

fn rebuilt_edge(image: Image) -> Image {
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
    graph: Graph,
    pipeline: Pipeline,
    /// The open frame's preview reduced to the canvas: its source's id, its long side, its pixels.
    fitted: Option<(u64, usize, Source)>,
}

/// The engine every thread shares, behind a gate the interface thread closes while the window's
/// surface is reconfigured: wgpu refuses a reconfigure that another thread submits work during.
#[derive(Clone)]
pub struct Shared(Arc<Gated>);

struct Gated {
    engine: Mutex<Engine>,
    gate: Mutex<Gate>,
    turn: Condvar,
}

#[derive(Default)]
struct Gate {
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
        Engine { graph: Graph::from_device(d.device.clone(), d.queue.clone(), name, oni_graph::TEXTURE_SIDE), pipeline: Pipeline::new(), fitted: None }
    }

    pub fn shared(d: &Device) -> Shared {
        Shared(Arc::new(Gated { engine: Mutex::new(Engine::on(d)), gate: Mutex::new(Gate::default()), turn: Condvar::new() }))
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
        Ok(Negative { preview, full_width: decoded.full_width })
    }

    /// `Pipeline.apply` on the preview, reduced first to a long side of `side` when given.
    pub fn render(&mut self, negative: &Negative, settings: &PipelineSettings, side: Option<usize>) -> Result<Linear, String> {
        let (w, h) = negative.size();
        let long = w.max(h) as f64;
        let scale = side.map_or(1.0, |s| (s as f64 / long).min(1.0));
        let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
        let (region, pixels) = self
            .pipeline
            .render(&mut self.graph, &negative.preview, settings, &request, &Inputs::default(), None)
            .map_err(|e| e.to_string())?;
        Ok(Linear { width: region.w as usize, height: region.h as usize, pixels })
    }

    /// `Pipeline.apply` on the preview, left on the GPU in one texture for the display kernel to read.
    /// With `side`, on the preview reduced once to that long side, so a notch computes no hidden pixel.
    pub fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, side: Option<usize>) -> Result<Source, String> {
        let (w, h) = negative.size();
        let long = w.max(h) as usize;
        let source = match side.filter(|&s| s < long) {
            Some(side) => self.fitted(negative, side, long)?,
            None => negative.preview.clone(),
        };
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        let image = self.pipeline.image(&mut self.graph, &source, settings, &request, &Inputs::default()).map_err(|e| e.to_string())?;
        let source = self.graph.materialise(&image).map_err(|e| e.to_string())?;
        if source.view().is_none() {
            return Err("the render is wider than one texture".into());
        }
        Ok(source)
    }

    fn fitted(&mut self, negative: &Negative, side: usize, long: usize) -> Result<Source, String> {
        let id = negative.preview.id();
        if let Some((kept, s, source)) = &self.fitted
            && (*kept, *s) == (id, side)
        {
            return Ok(source.clone());
        }
        self.fitted = None;
        let reduced = Image::new(&negative.preview).lanczos(side as f64 / long as f64);
        let source = self.graph.materialise(&reduced).map_err(|e| e.to_string())?;
        self.fitted = Some((id, side, source.clone()));
        Ok(source)
    }
}

/// The conversion a canvas copy goes through: settings in, linear Rec. 2020 out.
pub trait Renderer<I>: Send {
    fn render(&mut self, negative: &I, settings: &PipelineSettings) -> Result<Linear, String>;

    /// The same render kept on the GPU for the display kernel, fitted to `side` when given; `None`
    /// where only pixels are made.
    fn resident(&mut self, _negative: &I, _settings: &PipelineSettings, _side: Option<usize>) -> Option<Result<Source, String>> {
        None
    }
}

impl Renderer<Negative> for Shared {
    fn render(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<Linear, String> {
        self.lock()?.render(negative, settings, None)
    }

    fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, side: Option<usize>) -> Option<Result<Source, String>> {
        Some(self.lock().and_then(|mut e| e.resident(negative, settings, side)))
    }
}

/// The last previews decoded, so reopening a frame or opening one the thumbnails just read skips
/// the decode; four previews are about 170 MB of GPU memory.
pub struct Previews {
    kept: VecDeque<(String, Arc<Negative>)>,
}

const KEPT_PREVIEWS: usize = 4;

impl Default for Previews {
    fn default() -> Self {
        Previews { kept: VecDeque::with_capacity(KEPT_PREVIEWS) }
    }
}

impl Previews {
    pub fn get(&mut self, path: &str) -> Option<Arc<Negative>> {
        let i = self.kept.iter().position(|(p, _)| p == path)?;
        let entry = self.kept.remove(i).expect("found above");
        let negative = entry.1.clone();
        self.kept.push_back(entry);
        Some(negative)
    }

    pub fn insert(&mut self, path: String, negative: Arc<Negative>) {
        self.kept.retain(|(p, _)| *p != path);
        if self.kept.len() >= KEPT_PREVIEWS {
            self.kept.pop_front();
        }
        self.kept.push_back((path, negative));
    }
}

pub struct Job<I> {
    pub revision: u64,
    pub path: PathBuf,
    pub negative: Arc<I>,
    pub settings: PipelineSettings,
    pub kind: crate::timing::Kind,
    /// When the store asked for it, or the frame was opened, which the end-to-end time starts from.
    pub asked: Instant,
    /// The long side, in device pixels, the picture is shown at; none renders the whole preview.
    pub side: Option<usize>,
}

/// The long side of a `size` picture fitted inside a `canvas` of device pixels; none while it has no area.
pub fn fitted_side(size: (u32, u32), canvas: (f32, f32)) -> Option<usize> {
    let (w, h) = (size.0 as f32, size.1 as f32);
    if canvas.0 < 1.0 || canvas.1 < 1.0 || w < 1.0 || h < 1.0 {
        return None;
    }
    Some(((canvas.0 / w).min(canvas.1 / h) * w.max(h)).ceil() as usize)
}

/// What a render hands the canvas: a texture the display kernel encodes, or for the software
/// renderer the linear image with its bytes encoded on the render thread.
pub enum Picture {
    Resident(Source),
    Pixels { linear: Linear, rgba8: Vec<u8> },
}

/// A finished render, with the histogram of what is shown when the render thread measured it.
pub struct Done {
    pub revision: u64,
    pub path: PathBuf,
    pub picture: Picture,
    pub width: usize,
    pub height: usize,
    pub histogram: Option<Histogram>,
    pub kind: crate::timing::Kind,
    pub asked: Instant,
    pub pipeline: Duration,
}

struct Slot<I> {
    waiting: Option<Job<I>>,
    closed: bool,
}

pub struct Worker<I> {
    slot: Arc<(Mutex<Slot<I>>, Condvar)>,
    /// Set while the display kernel draws the canvas, so a notch stays on the GPU end to end.
    pub on_gpu: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl<I: Send + Sync + 'static> Worker<I> {
    pub fn new(mut renderer: Box<dyn Renderer<I>>, screen: Screen, deliver: impl Fn(Result<Done, String>) + Send + 'static) -> Worker<I> {
        let slot = Arc::new((Mutex::new(Slot { waiting: None, closed: false }), Condvar::new()));
        let shared = slot.clone();
        let on_gpu = Arc::new(AtomicBool::new(false));
        let gpu = on_gpu.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                let mut seed = 0u32;
                loop {
                    let job = {
                        let (lock, wake) = &*shared;
                        let mut s = lock.lock().expect("render slot");
                        while s.waiting.is_none() && !s.closed {
                            s = wake.wait(s).expect("render slot");
                        }
                        if s.closed {
                            return;
                        }
                        s.waiting.take().expect("a job")
                    };
                    seed = seed.wrapping_add(1);
                    let started = Instant::now();
                    let resident = if gpu.load(Ordering::Relaxed) { renderer.resident(&job.negative, &job.settings, job.side) } else { None };
                    let done = match resident {
                        Some(made) => made.map(|source| {
                            let e = source.extent();
                            let (width, height) = (e.w as usize, e.h as usize);
                            let picture = Picture::Resident(source);
                            Done { revision: job.revision, path: job.path, picture, width, height, histogram: None, kind: job.kind, asked: job.asked, pipeline: started.elapsed() }
                        }),
                        None => renderer.render(&job.negative, &job.settings).map(|linear| {
                            let pipeline = started.elapsed();
                            let rgba8 = encoded(&linear, &Params::new(screen, linear.width as u32, linear.height as u32, seed), 1);
                            let histogram = Some(Histogram::of(&rgba8));
                            let (width, height) = (linear.width, linear.height);
                            let picture = Picture::Pixels { linear, rgba8 };
                            Done { revision: job.revision, path: job.path, picture, width, height, histogram, kind: job.kind, asked: job.asked, pipeline }
                        }),
                    };
                    deliver(done);
                }
            })
            .expect("render thread");
        Worker { slot, on_gpu, thread: Some(thread) }
    }

    /// Lays `job` in the slot, replacing a job still waiting there.
    pub fn submit(&self, job: Job<I>) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").waiting = Some(job);
        wake.notify_one();
    }
}

impl<I> Drop for Worker<I> {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slot;
        lock.lock().expect("render slot").closed = true;
        wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// `display::encode` on every `step`-th row and column, its rows shared among the cores: the shown
/// bytes at a step of 1, a histogram's sample past it. A 2000 px preview costs 150 ms on one core.
pub fn encoded(linear: &Linear, k: &Params, step: usize) -> Vec<u8> {
    let (w, h) = (linear.width, linear.height);
    let rows: Vec<usize> = (0..h).step_by(step).collect();
    let row_bytes = w.div_ceil(step) * 4;
    let mut out = vec![0u8; rows.len() * row_bytes];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = rows.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (band, chunk) in rows.chunks(per).zip(out.chunks_mut(per * row_bytes)) {
            scope.spawn(move || {
                for (&y, line) in band.iter().zip(chunk.chunks_mut(row_bytes)) {
                    for (i, x) in (0..w).step_by(step).enumerate() {
                        let p = display::encode_pixel(linear.pixels[y * w + x], x as u32, y as u32, k);
                        line[i * 4..i * 4 + 4].copy_from_slice(&p);
                    }
                }
            });
        }
    });
    out
}

/// The shown image's three channels in 256 bins each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    pub bins: Vec<[u32; 3]>,
}

impl Histogram {
    pub fn of(rgba8: &[u8]) -> Histogram {
        let mut bins = vec![[0u32; 3]; 256];
        for p in rgba8.chunks_exact(4) {
            for c in 0..3 {
                bins[p[c] as usize][c] += 1;
            }
        }
        Histogram { bins }
    }

    /// `DSHistogram`: each channel's bins as a filled curve, red, green and blue added where they
    /// overlap, the tallest bin at the top; premultiplied RGBA8 of `w` by `h`.
    pub fn raster(&self, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0u8; w * h * 4];
        if self.bins.is_empty() || w == 0 || h == 0 {
            return out;
        }
        // The two ends hold every clipped pixel; scaling by them would flatten the rest.
        let top = self.bins[1..255].iter().flat_map(|b| b.iter().copied()).max().unwrap_or(1).max(1) as f32;
        for x in 0..w {
            let bin = (x * 256 / w).min(255);
            for c in 0..3 {
                let tall = ((self.bins[bin][c] as f32 / top).min(1.0) * h as f32).round() as usize;
                for y in h - tall..h {
                    let px = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    px[c] = px[c].saturating_add(150);
                    px[3] = px[3].max(150);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Counts its renders and holds each one long enough for jobs to pile up behind it.
    struct Slow(Arc<Mutex<Vec<u64>>>);

    impl Renderer<Linear> for Slow {
        fn render(&mut self, negative: &Linear, settings: &PipelineSettings) -> Result<Linear, String> {
            std::thread::sleep(Duration::from_millis(60));
            self.0.lock().unwrap().push(settings.print.exposure.to_bits() as u64);
            Ok(negative.clone())
        }
    }

    #[test]
    fn a_burst_of_notches_renders_the_first_and_the_last_only() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        let worker = Worker::new(Box::new(Slow(seen.clone())), Screen::Srgb, move |d| tx.send(d.map(|d| d.revision)).unwrap());
        let negative = Arc::new(Linear { width: 2, height: 1, pixels: vec![[0.5; 4]; 2] });
        for revision in 1..=20u64 {
            let mut settings = PipelineSettings::default();
            settings.print.exposure = revision as f32;
            let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
            worker.submit(Job { revision, path: PathBuf::new(), negative: negative.clone(), settings, kind, asked, side: None });
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut revisions = Vec::new();
        while let Ok(r) = rx.recv_timeout(Duration::from_millis(500)) {
            revisions.push(r.unwrap());
        }
        assert_eq!(revisions.last(), Some(&20), "the latest settings are always rendered");
        assert!(revisions.len() <= 3, "{revisions:?}: the slot replaced the waiting jobs");
        // Twin: twenty submissions, so a queue would have rendered twenty.
        assert!(seen.lock().unwrap().len() < 20);
    }

    #[test]
    fn a_render_is_fitted_inside_the_canvas_on_its_long_side() {
        assert_eq!(fitted_side((2000, 1332), (808.0, 1052.0)), Some(808), "a wide frame meets the canvas's sides");
        assert_eq!(fitted_side((1332, 2000), (808.0, 1052.0)), Some(1052), "a tall one meets its top and bottom");
        assert_eq!(fitted_side((2000, 1332), (0.0, 600.0)), None, "a canvas not laid out yet asks for the whole preview");
        // Twin: fitting the short side would leave the wide frame overflowing the canvas.
        assert_ne!(fitted_side((2000, 1332), (808.0, 1052.0)), Some((808.0 * 2000.0 / 1332.0) as usize));
    }

    #[test]
    fn the_parallel_encode_writes_what_the_reference_writes() {
        let (w, h) = (37usize, 23usize);
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.3, 1.0]).collect();
        let linear = Linear { width: w, height: h, pixels };
        let k = Params::new(Screen::Srgb, w as u32, h as u32, 5);
        let reference = display::encode(&linear.pixels, &k);
        assert_eq!(encoded(&linear, &k, 1), reference);
        let sample = encoded(&linear, &k, 2);
        assert_eq!(sample.len(), 19 * 12 * 4);
        assert_eq!(sample[4..8], reference[8..12], "the second sample is the third pixel");
        // Twin: another seed moves the dither, so a band encoded with the wrong one is seen.
        assert_ne!(encoded(&linear, &Params::new(Screen::Srgb, w as u32, h as u32, 6), 1), reference);
    }

    #[test]
    fn the_histogram_counts_each_channel_on_its_own() {
        let h = Histogram::of(&[10, 20, 30, 255, 10, 10, 10, 255]);
        assert_eq!(h.bins[10], [2, 1, 1]);
        assert_eq!(h.bins[20], [0, 1, 0]);
        let raster = h.raster(256, 10);
        // Bin 10 is the tallest red: its column is red to the top, its neighbour empty.
        assert!(raster[10 * 4] > 0 && raster[11 * 4 + 3] == 0);
    }

    #[test]
    fn previews_keep_the_last_four_used() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the previews are not exercised");
            return;
        };
        let mut engine = Engine::on(&device);
        let one = |e: &mut Engine| {
            let linear = Linear { width: 8, height: 8, pixels: vec![[0.5; 4]; 64] };
            Arc::new(e.prepare(Decoded { linear, full_width: 8.0 }).unwrap())
        };
        let mut kept = Previews::default();
        for k in 0..4 {
            kept.insert(k.to_string(), one(&mut engine));
        }
        assert!(kept.get("0").is_some(), "a use moves the frame to the back");
        kept.insert("4".into(), one(&mut engine));
        assert!(kept.get("1").is_none(), "the least recently used frame left");
        // Twin: the frame used just before the insertion stayed.
        assert!(kept.get("0").is_some());
    }

    /// The real pipeline on a two-pixel frame, on the first adapter wgpu finds.
    #[test]
    fn the_engine_inverts_a_negative_and_keeps_a_positive_in_order() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the engine is not exercised");
            return;
        };
        let mut engine = Engine::on(&device);
        // Six columns, so the rebuilt edge leaves the two middle ones as they are.
        let (w, h) = (6usize, 6usize);
        let pixels = (0..w * h).map(|i| if (i % w) < 3 { [0.6, 0.4, 0.3, 1.0] } else { [0.05, 0.03, 0.02, 1.0] }).collect();
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64 }).unwrap();
        let luma = |p: [f32; 4]| p[0] + p[1] + p[2];
        let settings = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        let out = engine.render(&negative, &settings, None).unwrap();
        assert_eq!((out.width, out.height), (w, h));
        let (thin, dense) = (out.pixels[2 * w + 2], out.pixels[2 * w + 3]);
        assert!(luma(thin) < luma(dense), "light through a thin negative prints dark: {thin:?} {dense:?}");
        // Twin: the positive mode renders the same frame without inverting it.
        let mut positive = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
        positive.face = settings.face;
        let out = engine.render(&negative, &positive, None).unwrap();
        assert!(luma(out.pixels[2 * w + 2]) > luma(out.pixels[2 * w + 3]));
    }

    #[test]
    fn a_reduced_preview_rebuilds_its_two_outer_rows_from_the_third() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the edge is not exercised");
            return;
        };
        let mut engine = Engine::on(&device);
        let (w, h) = (8usize, 7usize);
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32, (i / w) as f32, 0.0, 1.0]).collect();
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: pixels.clone() }, full_width: w as f64 }).unwrap();
        let mut graph_out = engine.graph.render(&Image::new(&negative.preview), negative.preview.extent(), None).unwrap();
        assert_eq!(graph_out.len(), w * h);
        let at = |x: usize, y: usize| graph_out[y * w + x];
        assert_eq!(at(0, 0)[..2], [2.0, 2.0], "the corner takes the third row and column");
        assert_eq!(at(7, 3)[..2], [5.0, 3.0]);
        assert_eq!(at(4, 4)[..2], [4.0, 4.0], "the inside is left as it is");
        // Twin: the decode itself had other values on its border.
        graph_out = pixels;
        assert_ne!(graph_out[0][..2], [2.0, 2.0]);
    }
}
