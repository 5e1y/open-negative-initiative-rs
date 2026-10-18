//! Decoding and rendering off the interface thread: a frame decoded once is kept on the GPU as its
//! preview, and a newer job replaces the one waiting in the render slot, so a drag never queues.

use crate::display::{self, Params, Screen};
use oni_graph::stages::Crop;
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::{CustomFlatField, Inputs, Pipeline, Request};
use oni_settings::frame::FlatFieldMask;
use oni_settings::id::Id;
use std::collections::HashMap;
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
    /// `Some(grey)` for a file naming no profile, which the hand may say how to read.
    pub undeclared: Option<bool>,
}

/// How a frame's sidecar asks a file outside the RAW engine to be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Its windows were measured on its channels converted to the working primaries.
    pub primaries_converted: bool,
    /// The encoding the hand chose for a file naming no profile.
    pub source_encoding: Option<oni_settings::SourceEncoding>,
    /// The three passes a lead names, decoded and laid as one frame.
    pub passes: Option<oni_settings::frame::RgbPasses>,
}

impl Reading {
    pub fn of(s: &oni_settings::PipelineSettings) -> Reading {
        Reading { primaries_converted: s.primaries_converted, source_encoding: s.source_encoding, passes: s.rgb_passes.clone() }
    }
}

/// `RawDecode.linear` on its preview: a lead's three passes composed, any other file alone.
pub fn decode(path: &Path, reading: Reading) -> Result<Decoded, String> {
    match reading.passes.clone() {
        Some(passes) => composed(path, &passes, |p| alone(p, reading.clone())),
        None => alone(path, reading),
    }
}

/// `RawDecode.alone` on its preview: the branch the bytes claim, the preview side asked of it.
fn alone(path: &Path, reading: Reading) -> Result<Decoded, String> {
    match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => {
            let d = oni_decode::decode(path, 1.0).map_err(|e| format!("LibRaw cannot decode it (code {})", e.0))?;
            let full_width = d.width as f64;
            Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width, undeclared: None })
        }
        oni_decode::Branch::LinearRgb => {
            let (full, _) = oni_decode::container::pixel_size(path).ok_or("its container is unreadable")?;
            linear_scan(path, Some(PREVIEW_SIDE as f64), reading, full)
        }
        oni_decode::Branch::Plain => plain(path, Some(PREVIEW_SIDE as f64), reading),
    }
}

/// The frame's own pixels, unreduced: what a zoom past the preview reads.
pub fn decode_full(path: &Path, reading: Reading) -> Result<Decoded, String> {
    let one = |p: &Path, r: Reading| match oni_decode::route(p) {
        oni_decode::Branch::LinearRgb => linear_scan(p, None, r, 0),
        oni_decode::Branch::Plain => plain(p, None, r),
        oni_decode::Branch::Mosaic => alone(p, r),
    };
    match reading.passes.clone() {
        Some(passes) => composed(path, &passes, |p| one(p, reading.clone())),
        None => one(path, reading),
    }
}

/// `RawDecode.alone(longestSide:)`: one file, never its passes, no longer than `side` (`None`: whole).
pub fn decode_alone(path: &Path, side: Option<f64>) -> Result<Decoded, String> {
    let reading = Reading::default();
    let mut d = match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => alone(path, reading)?,
        oni_decode::Branch::LinearRgb => {
            let (full, _) = oni_decode::container::pixel_size(path).ok_or("its container is unreadable")?;
            linear_scan(path, side, reading, full)?
        }
        oni_decode::Branch::Plain => plain(path, side, reading)?,
    };
    if let Some(side) = side {
        d.linear = shrunk(&d.linear, side);
    }
    Ok(d)
}

/// The image averaged down to `side` on its long side, each pixel the mean of the source pixels
/// whose centres it covers; a smaller one is kept.
pub fn shrunk(l: &Linear, side: f64) -> Linear {
    let long = l.width.max(l.height) as f64;
    if long <= side || l.width == 0 || l.height == 0 {
        return l.clone();
    }
    let scale = side / long;
    let (w, h) = (((l.width as f64 * scale).round() as usize).max(1), ((l.height as f64 * scale).round() as usize).max(1));
    let span = |i: usize, n: usize, of: usize| {
        let (a, b) = (i * of / n, ((i + 1) * of / n).max(i * of / n + 1));
        a..b.min(of)
    };
    let mut pixels = Vec::with_capacity(w * h);
    for row in 0..h {
        let rows = span(row, h, l.height);
        for col in 0..w {
            let cols = span(col, w, l.width);
            let mut sum = [0.0f64; 4];
            for y in rows.clone() {
                for x in cols.clone() {
                    let p = l.pixels[y * l.width + x];
                    for c in 0..4 {
                        sum[c] += f64::from(p[c]);
                    }
                }
            }
            let n = (rows.len() * cols.len()).max(1) as f64;
            pixels.push(sum.map(|v| (v / n) as f32));
        }
    }
    Linear { width: w, height: h, pixels }
}

/// `RGBPasses.composed`: red from the red pass, green from the green, blue from the blue, on the
/// green pass's extent, the red and blue laid by their registration; a missing pass fails the frame.
fn composed(lead: &Path, passes: &oni_settings::frame::RgbPasses, decode: impl Fn(&Path) -> Result<Decoded, String>) -> Result<Decoded, String> {
    let beside = |name: &str| lead.parent().map_or_else(|| PathBuf::from(name), |f| f.join(name));
    let green = decode(&beside(&passes.green)).map_err(|e| format!("its green pass: {e}"))?;
    let red = decode(&beside(&passes.red)).map_err(|e| format!("its red pass: {e}"))?;
    let blue = decode(&beside(&passes.blue)).map_err(|e| format!("its blue pass: {e}"))?;
    let fits = passes.registration.clone().unwrap_or_default();
    let (w, h) = (green.linear.width, green.linear.height);
    let r = laid(&red.linear, &fits.red, w, h, 0);
    let b = laid(&blue.linear, &fits.blue, w, h, 2);
    let pixels = green.linear.pixels.iter().enumerate().map(|(i, g)| [r[i], g[1], b[i], 1.0]).collect();
    Ok(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: green.full_width, undeclared: None })
}

/// One channel of a pass on a `w` × `h` frame through its fit, sampled bilinearly with the pass's
/// edge held past it, as `clampedToExtent().transformed(by:)` reads it; y up as Core Image counts.
fn laid(pass: &Linear, fit: &oni_settings::frame::Fit, w: usize, h: usize, channel: usize) -> Vec<f32> {
    let identity = *fit == oni_settings::frame::Fit::default();
    if identity && (pass.width, pass.height) == (w, h) {
        return pass.pixels.iter().map(|p| p[channel]).collect();
    }
    let (fw, fh) = (w as f64, h as f64);
    let (mx, my) = (fw / 2.0, fh / 2.0);
    let (cs, sn) = (fit.rotation.cos() * fit.scale, fit.rotation.sin() * fit.scale);
    let (a, b, c, d) = (cs, sn, -sn, cs);
    let tx = mx - (a * mx + c * my) + fit.dx * fw;
    let ty = my - (b * mx + d * my) + fit.dy * fw;
    let det = a * d - b * c;
    let texel = |x: i64, y_up: i64| -> f32 {
        let col = x.clamp(0, pass.width as i64 - 1) as usize;
        let row = (pass.height as i64 - 1 - y_up).clamp(0, pass.height as i64 - 1) as usize;
        pass.pixels[row * pass.width + col][channel]
    };
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        let oy = fh - row as f64 - 0.5;
        for col in 0..w {
            let (px, py) = (col as f64 + 0.5 - tx, oy - ty);
            // The fit's inverse: where in the pass this output point was taken from.
            let (ix, iy) = ((d * px - c * py) / det - 0.5, (-b * px + a * py) / det - 0.5);
            let (x0, y0) = (ix.floor(), iy.floor());
            let (fx, fy) = ((ix - x0) as f32, (iy - y0) as f32);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1, y0) * fx;
            let up = texel(x0, y0 + 1) * (1.0 - fx) + texel(x0 + 1, y0 + 1) * fx;
            out.push(top * (1.0 - fy) + up * fy);
        }
    }
    out
}

/// The container's own channels, or converted from its ColorMatrix1 when the windows were measured
/// so; a file naming no matrix is in the working space already. `full` 0 reads the decode's width.
fn linear_scan(path: &Path, side: Option<f64>, reading: Reading, full: usize) -> Result<Decoded, String> {
    let mut d = oni_decode::container::linear(path, side).ok_or("its samples are unreadable")?;
    if reading.primaries_converted {
        let (matrix, illuminant) = oni_decode::container::calibration(path).ok_or("its container is unreadable")?;
        if let Some(m) = oni_colour::source::calibrated_to_working(&matrix, illuminant) {
            for p in &mut d.pixels {
                let v = oni_colour::pcs::mul_f32(&m, [p[0], p[1], p[2]]);
                p[..3].copy_from_slice(&v);
            }
        }
    }
    let full_width = if full > 0 { full } else { d.width } as f64;
    Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width, undeclared: None })
}

/// `plainLinear`: the file's curve undone in its profile, else the chosen encoding, else sRGB.
fn plain(path: &Path, side: Option<f64>, reading: Reading) -> Result<Decoded, String> {
    let mut e = oni_decode::plain::read(path, side).ok_or("no image reader takes it")?;
    oni_colour::source::Source::of(e.icc.as_deref(), reading.source_encoding, e.grey).linearise(&mut e.pixels, !reading.primaries_converted);
    let undeclared = e.icc.is_none().then_some(e.grey);
    Ok(Decoded { linear: Linear { width: e.width, height: e.height, pixels: e.pixels }, full_width: e.full.0 as f64, undeclared })
}

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
fn patches_for<'a>(kept: &'a HashMap<Id, Option<Source>>, settings: &PipelineSettings) -> HashMap<Id, &'a Source> {
    settings.corrections.iter().filter_map(|c| Some((c.id, kept.get(&c.id)?.as_ref()?))).collect()
}

/// The custom mask read for the settings' own id, which the pipeline lays only when the ids match.
fn custom_for<'a>(kept: &'a HashMap<String, Option<CustomFlatField>>, settings: &PipelineSettings) -> Option<&'a CustomFlatField> {
    kept.get(settings.flat_field.custom.as_deref()?)?.as_ref()
}

/// What the canvas shows: the viewport over its drawable surface, and the frame's full-resolution
/// pixels once built, which a zoom past the preview's ratio reads instead of a stretched preview.
#[derive(Clone)]
pub struct View {
    pub viewport: crate::viewport::Viewport,
    /// The canvas, in drawable pixels.
    pub size: (f64, f64),
    pub full: Option<Source>,
    /// False while a navigation is in the hand: the preview serves it, as Swift's does.
    pub settled: bool,
}

/// A view's identity for a kept render: the viewport, the canvas, settled or not, and which full frame.
pub type ViewKey = (crate::viewport::Viewport, (f64, f64), bool, Option<u64>);

impl View {
    pub fn key(&self) -> ViewKey {
        (self.viewport, self.size, self.settled, self.full.as_ref().map(Source::id))
    }
}

/// Where a render sits on the canvas, in drawable pixels, and what it covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The displayed image's size in full-resolution pixels, which the viewport reasons on.
    pub full: (f64, f64),
    /// The render holds the whole image, so its histogram is the picture's.
    pub whole: bool,
    pub at_full_resolution: bool,
}

/// The conversion a canvas copy goes through: settings in, linear Rec. 2020 out.
pub trait Renderer<I>: Send {
    fn render(&mut self, negative: &I, settings: &PipelineSettings) -> Result<Linear, String>;

    /// The levels' and manual curves' input distributions; `None` where nothing measures.
    fn inputs(&mut self, _negative: &I, _settings: &PipelineSettings, _held: bool) -> Option<InputHistograms> {
        None
    }

    /// The same render kept on the GPU for the display kernel, the view's region when given; `None`
    /// where only pixels are made.
    fn resident(&mut self, _negative: &I, _settings: &PipelineSettings, _view: Option<&View>) -> Option<Result<(Source, Option<Placed>), String>> {
        None
    }
}

impl Renderer<Negative> for Shared {
    fn render(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<Linear, String> {
        self.lock()?.render(negative, settings, None)
    }

    fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Option<Result<(Source, Option<Placed>), String>> {
        Some(self.lock().and_then(|mut e| e.resident(negative, settings, view)))
    }

    fn inputs(&mut self, negative: &Negative, settings: &PipelineSettings, held: bool) -> Option<InputHistograms> {
        self.lock().ok()?.inputs(negative, settings, held)
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
    /// Forgets a frame's preview, which a new reading of its file makes stale.
    pub fn remove(&mut self, path: &str) {
        self.kept.retain(|(p, _)| p != path);
    }

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
    /// What the canvas shows; none renders the whole preview.
    pub view: Option<View>,
    /// Compare's before, the bar's reset frame with the dust kept, rendered on the same view.
    pub reference: Option<PipelineSettings>,
    /// A gesture holds the frame: the input histograms read the small measuring copy.
    pub held: bool,
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
    /// Where the canvas lays it; none fills the canvas.
    pub placed: Option<Placed>,
    pub before: Option<(Source, Option<Placed>)>,
    /// The levels' and manual curves' input distributions, read at most once an image interval.
    pub inputs: Option<InputHistograms>,
}

/// What the levels and the manual curves are drawn over: the distributions in front of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputHistograms {
    pub levels: oni_measure::histogram::Histogram,
    pub curves: oni_measure::histogram::Histogram,
}

impl InputHistograms {
    /// `HistogramTraces`: each channel's counts coarsened to the plot's columns, `HistogramScale`'s
    /// heights (the peak off the two bins at each end, then the 0.42 power), filled at 28 % under an
    /// 85 % edge, a channel out of focus at a fifth; premultiplied RGBA8 of `w` by `h`.
    pub fn raster(histogram: &oni_measure::histogram::Histogram, focus: Option<usize>, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![[0f32; 4]; w * h];
        let n = histogram.rgb.len();
        if n < 2 || w == 0 || h == 0 {
            return vec![0; w * h * 4];
        }
        for c in 0..3 {
            let counts: Vec<f32> = histogram.rgb.iter().map(|b| b[c].max(0.0)).collect();
            let interior = if n > 4 { &counts[2..n - 2] } else { &counts[..] };
            let peak = interior.iter().copied().fold(0.0, f32::max);
            if peak <= 0.0 {
                continue;
            }
            let column = |x: usize| {
                let (a, b) = (x * n / w, ((x + 1) * n / w).max(x * n / w + 1).min(n));
                let mean = counts[a..b].iter().sum::<f32>() / (b - a) as f32;
                (mean / peak).powf(0.42).min(1.0)
            };
            let strength = if focus.is_some_and(|f| f != c) { 0.2 } else { 1.0 };
            for x in 0..w {
                let top = h as f32 * (1.0 - column(x));
                for y in 0..h {
                    let edge = (y as f32 - top).abs() < 1.0;
                    let a = if edge { 0.85 } else if y as f32 >= top { 0.28 } else { continue } * strength;
                    let px = &mut out[y * w + x];
                    // Source over, premultiplied, in the channel's own primary.
                    for k in 0..3 {
                        px[k] = if k == c { a } else { 0.0 } + px[k] * (1.0 - a);
                    }
                    px[3] = a + px[3] * (1.0 - a);
                }
            }
        }
        out.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect()
    }
}

/// `Negative.measureSmall`'s scale of the measuring copy.
const MEASURE_SMALL: f64 = 0.35;

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
                // The before is kept while its settings and the view hold, so a notch never renders it again.
                let mut kept: Option<(PipelineSettings, Option<ViewKey>, Source, Option<Placed>)> = None;
                let mut last_inputs = Instant::now() - Duration::from_secs(1);
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
                    let resident = if gpu.load(Ordering::Relaxed) { renderer.resident(&job.negative, &job.settings, job.view.as_ref()) } else { None };
                    let done = match resident {
                        Some(made) => made.map(|(source, placed)| {
                            let e = source.extent();
                            let (width, height) = (e.w as usize, e.h as usize);
                            let picture = Picture::Resident(source);
                            Done { revision: job.revision, path: job.path.clone(), picture, width, height, histogram: None, kind: job.kind, asked: job.asked, pipeline: started.elapsed(), placed, before: None, inputs: None }
                        }),
                        None => renderer.render(&job.negative, &job.settings).map(|linear| {
                            let pipeline = started.elapsed();
                            let rgba8 = encoded(&linear, &Params::new(screen, linear.width as u32, linear.height as u32, seed), 1);
                            let histogram = Some(Histogram::of(&rgba8));
                            let (width, height) = (linear.width, linear.height);
                            let picture = Picture::Pixels { linear, rgba8 };
                            Done { revision: job.revision, path: job.path.clone(), picture, width, height, histogram, kind: job.kind, asked: job.asked, pipeline, placed: None, before: None, inputs: None }
                        }),
                    };
                    let before = match (&job.reference, gpu.load(Ordering::Relaxed)) {
                        (Some(reference), true) => {
                            let key = job.view.as_ref().map(View::key);
                            match &kept {
                                Some((s, k, source, placed)) if s == reference && *k == key => Some((source.clone(), *placed)),
                                _ => match renderer.resident(&job.negative, reference, job.view.as_ref()) {
                                    Some(Ok((source, placed))) => {
                                        kept = Some((reference.clone(), key, source.clone(), placed));
                                        Some((source, placed))
                                    }
                                    _ => None,
                                },
                            }
                        }
                        _ => None,
                    };
                    // At the image's own cadence, and always on the last job of a burst, so the hand's
                    // release lands the full copy's reading.
                    let quiet = shared.0.lock().map(|s| s.waiting.is_none()).unwrap_or(true);
                    let inputs = if done.is_ok() && (quiet || last_inputs.elapsed() >= crate::timing::IMAGE_INTERVAL) {
                        last_inputs = Instant::now();
                        renderer.inputs(&job.negative, &job.settings, job.held && !quiet)
                    } else {
                        None
                    };
                    let done = done.map(|d| Done { before, inputs, ..d });
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

    #[test]
    fn a_pass_shrunk_to_a_side_keeps_its_mean_and_a_smaller_one_is_kept() {
        let (w, h) = (9usize, 6usize);
        let l = Linear { width: w, height: h, pixels: (0..w * h).map(|i| [i as f32, 1.0, (i % w) as f32, 1.0]).collect() };
        let s = shrunk(&l, 3.0);
        assert_eq!((s.width, s.height), (3, 2));
        let mean = |l: &Linear, c: usize| l.pixels.iter().map(|p| f64::from(p[c])).sum::<f64>() / l.pixels.len() as f64;
        assert!((mean(&s, 0) - mean(&l, 0)).abs() < 1e-4 && (mean(&s, 2) - mean(&l, 2)).abs() < 1e-4);
        assert_eq!(s.pixels[0][2], 1.0, "the first column of three averages columns 0 to 2");
        // Twin: asked a side past its own, the image is its own.
        assert_eq!(shrunk(&l, 20.0).pixels, l.pixels);
    }

    #[test]
    fn three_passes_give_each_its_own_channel_and_a_registration_moves_its_pass() {
        let (w, h) = (8usize, 4usize);
        // Each pass lit only at one column, every channel alike, as a narrowband pass is not.
        let pass = |col: usize, v: f32| Linear { width: w, height: h, pixels: (0..w * h).map(|i| if i % w == col { [v, v, v, 1.0] } else { [0.0, 0.0, 0.0, 1.0] }).collect() };
        let fit = oni_settings::frame::Fit::default();
        let red = laid(&pass(3, 0.5), &fit, w, h, 0);
        assert_eq!(red[3], 0.5, "identity: the red pass's own column");
        // Two pixels right, as a fraction of the frame's width.
        let shifted = oni_settings::frame::Fit { dx: 2.0 / w as f64, ..fit };
        let moved = laid(&pass(3, 0.5), &shifted, w, h, 0);
        assert!((moved[5] - 0.5).abs() < 1e-6 && moved[3].abs() < 1e-6, "{moved:?}");
        // Twin: dy moves along the rows, y up: one pixel up is the row above.
        let up = oni_settings::frame::Fit { dy: 1.0 / w as f64, ..fit };
        let rows = laid(&Linear { width: w, height: h, pixels: (0..w * h).map(|i| if i / w == 2 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }).collect() }, &up, w, h, 0);
        assert!((rows[w] - 1.0).abs() < 1e-6 && rows[2 * w].abs() < 1e-6, "{rows:?}");
    }

    #[test]
    fn a_plain_image_opens_linearised_on_its_own_primaries_and_reduced_to_the_preview() {
        let dir = std::env::temp_dir().join(format!("oni-app-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scan.png");
        let (w, h) = (3000u32, 10u32);
        let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&path).unwrap()), w, h);
        e.set_color(png::ColorType::Rgb);
        let row: Vec<u8> = (0..w).flat_map(|_| [128u8, 128, 255]).collect();
        e.write_header().unwrap().write_image_data(&row.repeat(h as usize)).unwrap();
        let held = decode(&path, Reading::default()).unwrap();
        assert_eq!((held.linear.width, held.full_width), (PREVIEW_SIDE, 3000.0));
        let p = held.linear.pixels[held.linear.pixels.len() / 2];
        // Untagged, the file is sRGB: 128 is 0.216 in light, blue's 255 stays one on its own primaries.
        assert!((p[0] - 0.2158).abs() < 2e-3 && (p[2] - 1.0).abs() < 1e-4, "{p:?}");
        // Twin: measured on converted primaries, sRGB's blue mixes into the working red and green.
        let converted = decode(&path, Reading { primaries_converted: true, ..Reading::default() }).unwrap();
        let q = converted.linear.pixels[converted.linear.pixels.len() / 2];
        assert!((q[0] - p[0]).abs() > 1e-2, "{q:?} against {p:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
            worker.submit(Job { revision, path: PathBuf::new(), negative: negative.clone(), settings, kind, asked, view: None, reference: None, held: false });
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
            Arc::new(e.prepare(Decoded { linear, full_width: 8.0, undeclared: None }).unwrap())
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

    /// A zoomed view renders only what the canvas shows: at 100 % from the full frame, the canvas's
    /// own size, laid edge to edge; fitted, the whole image from the preview.
    #[test]
    fn a_view_renders_its_visible_region_from_the_source_its_scale_needs() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the views are not exercised");
            return;
        };
        let mut engine = Engine::on(&device);
        let (fw, fh) = (1600usize, 1000usize);
        let gradient = |w: usize, h: usize| (0..w * h).map(|i| [0.2 + 0.5 * (i % w) as f32 / w as f32, 0.3, 0.4, 1.0]).collect::<Vec<_>>();
        // A 400 px preview of a 1600 px frame: the preview's ratio is a quarter.
        let negative = engine.prepare(Decoded { linear: Linear { width: 400, height: 250, pixels: gradient(400, 250) }, full_width: fw as f64, undeclared: None }).unwrap();
        let full = engine.full(Decoded { linear: Linear { width: fw, height: fh, pixels: gradient(fw, fh) }, full_width: fw as f64, undeclared: None }).unwrap();
        let settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
        let size = (600.0, 400.0);
        let hundred = crate::viewport::Viewport { scale: Some(1.0), offset: (0.0, 0.0) };
        let view = View { viewport: hundred, size, full: Some(full.clone()), settled: true };
        let (source, placed) = engine.resident(&negative, &settings, Some(&view)).unwrap();
        let placed = placed.unwrap();
        assert!(placed.at_full_resolution && !placed.whole);
        let e = source.extent();
        assert!((e.w as f64 - size.0).abs() <= 2.0 && (e.h as f64 - size.1).abs() <= 2.0, "the canvas's pixels only: {e:?}");
        assert!(placed.x <= 0.0 && placed.y <= 0.0 && placed.x + placed.width >= size.0 && placed.y + placed.height >= size.1);
        // Twin: unsettled, the same view stays on the preview, a quarter of the pixels stretched four times.
        let moving = View { settled: false, ..view.clone() };
        let (stretched, placed) = engine.resident(&negative, &settings, Some(&moving)).unwrap();
        assert!(!placed.unwrap().at_full_resolution);
        assert!(stretched.extent().w * 3 < e.w, "{:?}", stretched.extent());
        // Fitted on a canvas smaller than the preview: the whole image, from the preview.
        let fitted = View { viewport: crate::viewport::Viewport::default(), size: (300.0, 200.0), full: Some(full), settled: true };
        let (_, placed) = engine.resident(&negative, &settings, Some(&fitted)).unwrap();
        let placed = placed.unwrap();
        assert!(placed.whole && !placed.at_full_resolution, "{placed:?}");
        assert!((placed.width - 300.0).abs() < 1.0 && (placed.height - 187.5).abs() < 1.0, "{placed:?}");
    }

    /// A frame naming a custom flat-field is divided by that mask, read from the support folder;
    /// one naming a mask that is gone renders as with none, as the Swift app leaves the stage out.
    #[test]
    fn a_custom_flat_field_reaches_the_render_by_its_id() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the custom flat-field is not exercised");
            return;
        };
        let support = std::env::temp_dir().join(format!("oni-render-custom-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
        std::fs::create_dir_all(crate::flat_fields::folder(&support)).unwrap();
        // A panel falling off to the right: the division lifts the right of the frame.
        let (mw, mh) = (8u32, 4u32);
        let mask: Vec<f32> = (0..mw * mh).flat_map(|i| { let v = 1.0 - 0.6 * (i % mw) as f32 / mw as f32; [v, v, v, 1.0] }).collect();
        let out = std::fs::File::create(crate::flat_fields::folder(&support).join("panel.tiff")).unwrap();
        tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(mw, mh, &mask).unwrap();
        let mut engine = Engine::on(&device);
        engine.support = Some(support.clone());
        let (w, h) = (16usize, 8usize);
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
        let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
        let plain = engine.render(&negative, &settings, None).unwrap();
        settings.flat_field = oni_settings::frame::FlatField { mask: FlatFieldMask::Custom, custom: Some("panel".into()), ..Default::default() };
        let divided = engine.render(&negative, &settings, None).unwrap();
        let at = |l: &Linear, x: usize| l.pixels[(h / 2) * w + x][1];
        assert!((at(&divided, w - 3) - at(&divided, 2)).abs() > 1e-3, "the mask's falloff shows: {} {}", at(&divided, 2), at(&divided, w - 3));
        assert!((at(&plain, w - 3) - at(&plain, 2)).abs() < 1e-5, "the flat frame without it stays flat");
        // Twin: a mask that is gone leaves the stage out, so the frame renders as with none.
        settings.flat_field.custom = Some("gone".into());
        let gone = engine.render(&negative, &settings, None).unwrap();
        assert!(gone.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a[1] - b[1]).abs() < 1e-6));
        // The same id loaded afterwards is read on the next render, the miss not kept.
        std::fs::copy(crate::flat_fields::folder(&support).join("panel.tiff"), crate::flat_fields::folder(&support).join("gone.tiff")).unwrap();
        let found = engine.render(&negative, &settings, None).unwrap();
        assert!((at(&found, w - 3) - at(&divided, w - 3)).abs() < 1e-6, "{} {}", at(&found, w - 3), at(&divided, w - 3));
        let _ = std::fs::remove_dir_all(&support);
    }

    /// A correction's patch from the cache is laid through its stroke; one not generated yet is
    /// left out, so the frame renders as with no correction.
    #[test]
    fn a_dust_patch_from_the_cache_heals_its_stroke() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the dust patches are not exercised");
            return;
        };
        let caches = std::env::temp_dir().join(format!("oni-render-patches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caches);
        let mut engine = Engine::on(&device);
        engine.caches = Some(caches.clone());
        let (w, h) = (64usize, 48usize);
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
        let mut settings = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
        let plain = engine.render(&negative, &settings, None).unwrap();
        let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
        let point = |x: f64, y: f64| oni_settings::cg::Point { x, y };
        settings.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![point(0.5, 0.5)], radius: 6.0 }];
        let missing = engine.render(&negative, &settings, None).unwrap();
        assert!(missing.pixels.iter().zip(&plain.pixels).all(|(a, b)| (a[1] - b[1]).abs() < 1e-6), "no patch, no change");
        // A bright patch the size of the tile: the frame is 64 × 48, so the tile is the whole frame.
        std::fs::create_dir_all(caches.join("Corrections")).unwrap();
        let out = std::fs::File::create(crate::patches::file(&caches, id)).unwrap();
        let bright: Vec<f32> = (0..w * h).flat_map(|_| [0.9f32, 0.9, 0.9, 1.0]).collect();
        tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w as u32, h as u32, &bright).unwrap();
        let healed = engine.render(&negative, &settings, None).unwrap();
        let at = |l: &Linear, x: usize, y: usize| l.pixels[y * w + x][1];
        assert!((at(&healed, w / 2, h / 2) - at(&plain, w / 2, h / 2)).abs() > 1e-2, "the stroke's centre takes the patch");
        // Twin: far from the stroke the frame is its own, so a patch laid everywhere would be seen.
        assert!((at(&healed, 2, 2) - at(&plain, 2, 2)).abs() < 1e-5);
        let _ = std::fs::remove_dir_all(&caches);
    }

    /// The real pipeline on a two-pixel frame, on the first adapter wgpu finds.
    #[test]
    fn the_input_plots_read_in_front_of_their_stage_and_a_curve_moves_only_what_follows() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the engine is not exercised");
            return;
        };
        let mut engine = Engine::on(&device);
        let (w, h) = (300usize, 200usize);
        let pixels = (0..w * h).map(|i| { let v = 0.02 + 0.6 * (i % w) as f32 / w as f32; [v, v * 0.7, v * 0.5, 1.0] }).collect();
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
        let rest = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        let before = engine.inputs(&negative, &rest, false).expect("readings");
        assert!(before.levels.rgb.len() > 2 && before.curves.rgb.len() > 2);
        // An adversarial manual curve: the output moves, the curves' own input does not.
        let mut curved = rest.clone();
        curved.manual_curves.linked.points = vec![oni_settings::cg::Point::new(0.0, 0.0), oni_settings::cg::Point::new(0.3, 0.8), oni_settings::cg::Point::new(1.0, 1.0)];
        let after = engine.inputs(&negative, &curved, false).expect("readings");
        assert_eq!(after.curves, before.curves, "the curves' input is read in front of them");
        let shown = |s: &PipelineSettings, e: &mut Engine| e.render(&negative, s, None).unwrap().pixels;
        let moved = shown(&rest, &mut engine).iter().zip(shown(&curved, &mut engine)).map(|(a, b)| (a[1] - b[1]).abs()).fold(0.0, f32::max);
        assert!(moved > 0.05, "the curve reaches the output: {moved}");
        // Twin: gains, in front of the levels, move the levels' own input.
        let mut gained = rest.clone();
        gained.gains.stops = [0.8, 0.0, -0.8];
        let gained = engine.inputs(&negative, &gained, false).expect("readings");
        assert_ne!(gained.levels, before.levels, "the levels' input reads the gains");
        // The small copy under the hand reads a third of the pixels.
        let small = engine.inputs(&negative, &rest, true).expect("readings");
        let count = |h: &oni_measure::histogram::Histogram| h.rgb.iter().map(|b| b[0]).sum::<f32>();
        assert!(count(&small.levels) < 0.2 * count(&before.levels), "{} of {}", count(&small.levels), count(&before.levels));
    }

    #[test]
    fn an_input_plot_draws_its_focus_channel_whole_and_the_others_at_a_fifth() {
        let rgb: Vec<[f32; 3]> = (0..64).map(|i| [i as f32, 32.0, (64 - i) as f32]).collect();
        let h = oni_measure::histogram::Histogram { rgb, luma: vec![0.0; 64], ends: None };
        let all = InputHistograms::raster(&h, None, 64, 20);
        let green = InputHistograms::raster(&h, Some(1), 64, 20);
        let at = |v: &[u8], x: usize, y: usize, c: usize| v[(y * 64 + x) * 4 + c];
        // Near the bottom every trace fills: red is dimmed once green holds the focus, green is not.
        assert!(at(&green, 40, 19, 0) < at(&all, 40, 19, 0) && at(&green, 40, 19, 1) >= at(&all, 40, 19, 1) / 2);
        // Twin: an empty reading draws nothing at all.
        let empty = oni_measure::histogram::Histogram::default();
        assert!(InputHistograms::raster(&empty, None, 8, 8).iter().all(|&b| b == 0));
    }

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
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
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
        let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: pixels.clone() }, full_width: w as f64, undeclared: None }).unwrap();
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
