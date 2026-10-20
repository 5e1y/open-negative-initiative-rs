//! The automatic keys on the graph: Classic's two steps, a frame's roll, Auto White Balance and the
//! base pick, each read in front of the stage it writes, never off the screen.

use crate::auto_levels::{self, Balance};
use crate::filter_balance::{self, filtered, minkowski, unlit};
use crate::pipette;
use crate::reader::{Copy, Reader, on_graph};
use crate::roll::{self, Measured};
use crate::truncate::Stage;
use crate::window::sheet_reads;
use oni_core::balance::{self, Outcome, Suggestion};
use oni_core::parameters;
use oni_graph::{Graph, Image, Source};
use oni_pipeline::Pipeline;
use oni_pipeline::resident::{Files, Resident};
use oni_settings::blocks::Gains;
use oni_settings::cg::{self, Point};
use oni_settings::frame::{FlatField, Geometry};
use oni_settings::{ConversionMode, PipelineSettings};
use std::path::Path;
use std::sync::Mutex;

fn at_gains(s: &PipelineSettings, stops: [f32; 3]) -> PipelineSettings {
    let mut probe = s.clone();
    probe.gains.stops = stops;
    probe
}

/// `RollReference.measured`: the frame's Classic, read in front of the levels.
pub fn roll_measured(r: &mut Reader, copy: Copy, path: &str, s: &PipelineSettings) -> Option<Measured> {
    roll::measured(path, s, |stops| r.reading(copy, &at_gains(s, stops), Stage::Levels).histogram)
}

/// `AutoLevels.placed`: both steps on the frame, re-measured between them.
pub fn placed(r: &mut Reader, copy: Copy, s: &PipelineSettings) -> Option<Balance> {
    let first = r.reading(copy, s, Stage::Levels).histogram;
    if first.is_empty() {
        return None;
    }
    auto_levels::two_steps(&first, s.gains.stops, s.mode, 0.0, |stops| r.reading(copy, &at_gains(s, stops), Stage::Levels).histogram)
}

/// `AutoLevels.ends`: step two alone, the gains held.
pub fn ends(r: &mut Reader, copy: Copy, s: &PipelineSettings) -> Option<[Suggestion; 3]> {
    let h = r.reading(copy, s, Stage::Levels).histogram;
    if h.is_empty() { None } else { auto_levels::placement(&h, 0.0) }
}

/// `FilterBalance.shades`: each channel's sixth-power mean under given filters, over the kept pixels.
pub fn shades(r: &mut Reader, copy: Copy, s: &PipelineSettings, filters: [f32; 3], keeping: fn([f32; 3]) -> bool) -> Option<[f32; 3]> {
    let mut t = s.clone();
    filtered(&mut t, filters);
    let pixels: Vec<[f32; 3]> = r.balance_pixels(copy, &t)?.into_iter().filter(|p| keeping(*p)).collect();
    minkowski(&pixels)
}

/// `FilterBalance.filters`: Newton on the frame as it renders, Lighting at rest but for its filters.
pub fn filters(r: &mut Reader, copy: Copy, s: &PipelineSettings, keeping: fn([f32; 3]) -> bool) -> Option<[f32; 3]> {
    if s.mode.is_monochrome() {
        return Some([0.0; 3]);
    }
    let base = unlit(s);
    filter_balance::filters(false, |f| shades(r, copy, &base, f, keeping))
}

/// `FilterBalance.outcome`: a frame on no roll is first measured as a roll of one.
pub fn balance_outcome(r: &mut Reader, copy: Copy, path: &str, s: &PipelineSettings) -> Option<Outcome> {
    let (reference, referenced) = match &s.roll_reference {
        Some(own) => (own.clone(), s.clone()),
        None => {
            let frame = roll_measured(r, copy, path, s)?;
            let roll = roll::combined(&[frame], s.mode, None)?;
            let on = balance::written(&roll, s);
            (roll, on)
        }
    };
    let filters = filters(r, copy, &referenced, filter_balance::printable)?;
    Some(Outcome { reference, filters })
}

/// `AutomaticKey.outcome`: on a sheet standing on its roll, each frame read alone through its own
/// window and the mean laid, as a press over the whole roll averaged would.
pub fn outcome(r: &mut Reader, copy: Copy, path: &str, s: &PipelineSettings) -> Option<Outcome> {
    let (Some(grid), Some(_)) = (&s.sheet, &s.roll_reference) else {
        return balance_outcome(r, copy, path, s);
    };
    // Each read is already the frame's window: read whole, never inset a second time.
    let mut frame = s.clone();
    frame.sheet = None;
    frame.geometry = Geometry::default();
    frame.corrections = Vec::new();
    frame.geometry.measure_window = Some(frame.geometry.crop);
    let e = copy.source.extent();
    let extent = cg::Rect::new(0.0, 0.0, f64::from(e.w), f64::from(e.h));
    let mut found = Vec::new();
    for cell in sheet_reads(grid, &extent) {
        let Ok(cut) = r.graph.materialise_region(&Image::new(copy.source), on_graph(e, &cell)) else { continue };
        if let Some(o) = balance_outcome(r, Copy { source: &cut, full_width: None }, path, &frame) {
            found.push(o);
        }
    }
    balance::averaged(&found).into_iter().next()
}

/// `EditorScene.base`: the bare film under the pick, read before the levels on a frame stripped of
/// its framing, gains and dust, on the roll's film and mask.
#[allow(clippy::too_many_arguments)]
pub fn base(
    r: &mut Reader,
    copy: Copy,
    open: &PipelineSettings,
    mode: ConversionMode,
    mask: Option<&FlatField>,
    capture_stops: f32,
    unit: Point,
    radius: f64,
) -> Option<[f32; 3]> {
    let mut bare = parameters::imported(open);
    bare.mode = mode;
    bare.geometry = Geometry::default();
    bare.gains = Gains::default();
    bare.corrections = Vec::new();
    bare.capture_stops = capture_stops;
    if let Some(m) = mask {
        bare.flat_field = m.clone();
    }
    let graduated = r.plane(copy, &bare, Stage::Levels)?;
    pipette::read(&graduated, unit, mode, radius, pipette::RETAINED_SPACE, pipette::RETAINED_STATISTIC).map(|x| x.fractions)
}

/// A file decoded on the CPU: linear Rec. 2020 pixels, and the full width of the frame they came from.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[f32; 4]>,
    pub full_width: f64,
}

/// Decodes a file as its settings ask: primaries converted, the encoding of a file naming no profile.
pub type Decode = dyn Fn(&Path, &PipelineSettings) -> Result<Decoded, String> + Send + Sync;

/// `Negative.previewSide`, the copy the measuring one is reduced from.
pub const PREVIEW_SIDE: f64 = 2000.0;

/// What a decode depends on besides the path, so a measuring copy is reused only for the same read.
pub type Reading = dyn Fn(&PipelineSettings) -> String + Send + Sync;

/// Measuring copies kept: the open frame and the one before, 21 MB each at 1400 px.
const KEPT: usize = 2;

/// The key on files: each decoded as the app decodes it, reduced to its preview then to the
/// measuring side, then read on a graph of its own.
pub struct Measurer {
    engine: Mutex<Engine>,
    decode: Box<Decode>,
    reading: Box<Reading>,
}

struct Engine {
    graph: Graph,
    pipeline: Pipeline,
    /// The last copies made, keyed by path and reading, newest last.
    kept: Vec<(String, std::sync::Arc<Source>, f64)>,
    resident: Resident,
}

impl Measurer {
    pub fn new(graph: Graph, decode: Box<Decode>, reading: Box<Reading>) -> Measurer {
        Measurer { engine: Mutex::new(Engine { graph, pipeline: Pipeline::new(), kept: Vec::new(), resident: Resident::default() }), decode, reading }
    }

    /// Every reading laid with the custom mask and dust patches its frame names, read from `files`.
    pub fn laying(self, files: std::sync::Arc<dyn Files>) -> Measurer {
        if let Ok(mut e) = self.engine.lock() {
            e.resident = Resident::new(files);
        }
        self
    }

    /// `RawDecode.measuring(url)`: the preview, then the measuring copy, each edge rebuilt.
    fn copy(graph: &mut Graph, pipeline: &mut Pipeline, d: &Decoded) -> Option<Source> {
        let source = graph.upload(d.width, d.height, &d.pixels).ok()?;
        let long = f64::from(d.width.max(d.height));
        let mut image = Image::new(&source);
        if long > PREVIEW_SIDE {
            image = image.lanczos(PREVIEW_SIDE / long);
        }
        let preview = graph.materialise(&oni_pipeline::stages::rebuilt_edge(image)).ok()?;
        drop(source);
        let measuring = Reader { graph, pipeline, resident: None }.measuring(&preview);
        graph.purge();
        measuring
    }

    /// The frame's measuring copy, the kept one when the same file was read the same way.
    fn kept(&self, path: &str, s: &PipelineSettings) -> Option<(std::sync::Arc<Source>, f64)> {
        let key = format!("{path}\u{0}{}", (self.reading)(s));
        if let Some(found) = self.engine.lock().ok()?.kept.iter().find(|k| k.0 == key) {
            return Some((found.1.clone(), found.2));
        }
        let d = (self.decode)(Path::new(path), s).ok()?;
        let mut guard = self.engine.lock().ok()?;
        let e = &mut *guard;
        let source = std::sync::Arc::new(Self::copy(&mut e.graph, &mut e.pipeline, &d)?);
        e.kept.retain(|k| k.0 != key);
        if e.kept.len() >= KEPT {
            e.kept.remove(0);
        }
        e.kept.push((key, source.clone(), d.full_width));
        Some((source, d.full_width))
    }

    /// Makes the frame's measuring copy ahead of a press, so the press reads at once.
    pub fn prepare(&self, path: &str, s: &PipelineSettings) {
        self.kept(path, s);
    }

    /// Reads one frame with `read`, on its measuring copy; `None` where the file reads nothing.
    pub fn with<T>(&self, path: &str, s: &PipelineSettings, read: impl FnOnce(&mut Reader, Copy) -> Option<T>) -> Option<T> {
        let (source, full_width) = self.kept(path, s)?;
        let mut guard = self.engine.lock().ok()?;
        let Engine { graph, pipeline, resident, .. } = &mut *guard;
        resident.fresh();
        let out = read(&mut Reader { graph, pipeline, resident: Some(resident) }, Copy { source: &source, full_width: Some(full_width) });
        graph.purge();
        out
    }

    /// `RawDecode.linear(longestSide:)`: the file decoded, reduced to `side` with its edge rebuilt,
    /// then read with `read`.
    pub fn reduced<T>(&self, path: &str, s: &PipelineSettings, side: f64, read: impl FnOnce(&mut Reader, Copy) -> Option<T>) -> Option<T> {
        let d = (self.decode)(Path::new(path), s).ok()?;
        let mut guard = self.engine.lock().ok()?;
        let Engine { graph, pipeline, resident, .. } = &mut *guard;
        resident.fresh();
        let uploaded = graph.upload(d.width, d.height, &d.pixels).ok()?;
        let full_width = Some(d.full_width);
        let long = f64::from(d.width.max(d.height));
        drop(d);
        let source = if long > side {
            let reduced = graph.materialise(&oni_pipeline::stages::rebuilt_edge(Image::new(&uploaded).lanczos(side / long))).ok()?;
            drop(uploaded);
            reduced
        } else {
            uploaded
        };
        let out = read(&mut Reader { graph, pipeline, resident: Some(resident) }, Copy { source: &source, full_width });
        drop(source);
        graph.purge();
        out
    }
}

impl balance::Measure for Measurer {
    fn balance(&self, path: &str, settings: &PipelineSettings) -> Option<Outcome> {
        self.with(path, settings, |r, copy| outcome(r, copy, path, settings))
    }
}
