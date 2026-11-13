//! The welcome's De-dust film: a dusty photograph's detail under the canvas's overlays beside the app's pane;
//! DETECT runs the app's reading, REMOVE writes a correction a speck and the app heals each, at their pace.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use oni_app::display::{Params, Screen};
use oni_app::fields::{Readout, Unit};
use oni_app::render::{self, Device, Engine, Negative, Reading, Shared};
use oni_graph::{Rect, Source};
use oni_i18n::{tr, trn};
use oni_measure::dust::{self, Limits, Speck};
use oni_pipeline::Request;
use oni_settings::PipelineSettings;
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::geometry::Placing;
use oni_settings::id::Id;
use slint::{Model, ModelRc, SharedString, VecModel};
use slint_interpreter::{Struct, Value};

use crate::cursor;
use crate::encode::{self, Clip, Region, Tuning};
use crate::film::FPS;
use crate::photo::{self, Rgba, magnified};
use crate::stage::{SCALE, Stage};
use crate::track::{Presses, Track};

/// The welcome's stage in points, `WelcomeStyle.stage-width` by `stage-height`.
pub const STAGE: (f32, f32) = (560.0, 502.0);
/// The variable naming the dusty photograph: its path is the owner's, never written in the repository.
pub const SOURCE: &str = "ONI_WELCOME_DUST";
/// The app's auto De-dust mask side (`MASK_SIDE` in its reading): the veil's texels, shown pixelated.
const MASK_SIDE: usize = 3072;
/// The film's bounds on the app's two waits, as the plan's gates set them: shorter is shown at the
/// least, longer fails the film rather than show a pace the app does not keep.
const READING: (f32, f32) = (0.3, 3.0);
const HEALING: f32 = 4.0;
/// A mark's number badge, in points, as `CorrectionMark` lays it off its box's corner, and the gap
/// kept between two specks' rings and marks so no number hides another.
const BADGE: (f32, f32) = (24.0, 20.0);
const GAP: f32 = 4.0;
/// How far inside the strip a speck's ring and mark stay, in points: none at an edge.
const MARGIN: f32 = 16.0;
/// The fewest specks a detail must show.
pub const FEWEST: usize = 6;
/// The zooms a detail is looked for at, closest first: the strip's pixels a frame pixel, as the canvas's
/// 200 %, 100 % and 50 % show it on a Retina screen.
const ZOOMS: [f32; 3] = [2.0, 1.0, 0.5];

pub fn source() -> Result<PathBuf, String> {
    let path = std::env::var_os(SOURCE).map(PathBuf::from).ok_or(format!("set {SOURCE} to the dusty photograph"))?;
    path.is_file().then_some(path).ok_or(format!("{SOURCE} names no file"))
}

/// The photograph opened as De-dust opens it: its sidecar's settings without a correction, its full
/// decode on the GPU, and its preview for the tiles' reminders. Nothing is written beside it.
pub struct Scan {
    engine: Shared,
    full: Source,
    preview: Negative,
    pub settings: PipelineSettings,
    /// The framed frame at full resolution, in pixels, and where its extent starts.
    pub size: (u32, u32),
    origin: (i32, i32),
    placing: Placing,
    caches: PathBuf,
    /// A finished photograph's own code values, shown as they are: a JPEG export is never converted.
    as_is: Option<Rgba>,
}

impl Scan {
    pub fn open(path: &Path) -> Result<Scan, String> {
        let plain = oni_decode::route(path) == oni_decode::Branch::Plain;
        let loaded = oni_settings::sidecar::read(path, oni_decode::decodes_via_mosaic(path)).map_err(|e| format!("its sidecar: {e:?}"))?;
        // An export with no edit is read as the positive it is, which the reading takes at its blacks.
        let mut settings = if plain && loaded.is_new { PipelineSettings::rest(oni_settings::ConversionMode::Positive, false) } else { loaded.settings };
        settings.corrections.clear();
        let device = Device::open()?;
        let engine = Engine::shared(&device);
        let caches = std::env::temp_dir().join(format!("oni-welcome-dedust-{}", std::process::id()));
        std::fs::create_dir_all(&caches).map_err(|e| e.to_string())?;
        engine.lock()?.caches = Some(caches.clone());
        let full = engine.full(render::decode_full(path, Reading::of(&settings))?)?;
        let preview = engine.prepare(render::decode_for(path, Reading::of(&settings), render::PREVIEW_SIDE)?)?;
        let e = engine.lock()?.unproofed(&full, &settings, &Request::default())?.extent();
        let scan = full.extent();
        let placing = settings.geometry.placing((f64::from(scan.w), f64::from(scan.h)));
        let as_is = if plain { Some(code_values(path)?) } else { None };
        if as_is.as_ref().is_some_and(|p| (p.width, p.height) != (e.w, e.h)) {
            return Err("the export's pixels and its render differ in size".into());
        }
        Ok(Scan { engine, full, preview, settings, size: (e.w, e.h), origin: (e.x, e.y), placing, caches, as_is })
    }

    /// The app's reading of the framed frame, and how long it took.
    pub fn read(&self) -> Result<(dust::Reading, Duration), String> {
        let started = Instant::now();
        let (width, pixels) = self.engine.lock()?.framed_whole(&self.full, &self.settings)?;
        let reading = dust::read(&pixels, width, &Limits::of(&self.settings), MASK_SIDE);
        if (reading.width, reading.height) != (self.size.0 as usize, self.size.1 as usize) {
            return Err(format!("the reading's frame is {}×{}, the render's {:?}", reading.width, reading.height, self.size));
        }
        Ok((reading, started.elapsed()))
    }

    /// The view of `s` at the strip's pixels, as the canvas zoomed past one to one shows it.
    pub fn detail(&self, s: &PipelineSettings, view: View) -> Result<Rgba, String> {
        let (w, h) = view.region();
        if let Some(whole) = &self.as_is {
            if s.corrections.is_empty() {
                return Ok(zoomed(&crop(whole, view.x, view.y, w, h), view));
            }
            return Ok(zoomed(&self.healed_as_is(s, view)?, view));
        }
        let mut e = self.engine.lock()?;
        let image = e.unproofed(&self.full, s, &Request::default())?;
        let (graph, _) = e.parts();
        let rect = Rect::new(self.origin.0 + view.x as i32, self.origin.1 + view.y as i32, w, h);
        let pixels = graph.render(&image, rect, None).map_err(|e| e.to_string())?;
        graph.purge();
        let picture = photo::encoded(&render::Linear { width: w as usize, height: h as usize, pixels });
        Ok(zoomed(&picture, view))
    }

    /// A finished photograph's view with its patches laid, still never converted: the decoded scan the
    /// patches lie on, framed as the reading reads it, brought back to the screen's encoding.
    fn healed_as_is(&self, s: &PipelineSettings, view: View) -> Result<Rgba, String> {
        let (w, h) = view.region();
        let (width, pixels) = self.engine.lock()?.framed_whole(&self.full, s)?;
        let rows = (view.y as usize..(view.y + h) as usize).flat_map(|y| pixels[y * width + view.x as usize..][..w as usize].iter().copied());
        let mut params = Params::new(Screen::Srgb, w, h, 0);
        params.dither = 0.0;
        // Primaries held, the scan is in the file's own, an sRGB export's: only its curve is laid back.
        if !s.primaries_converted {
            params.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
        }
        let linear = render::Linear { width: w as usize, height: h as usize, pixels: rows.collect() };
        Ok(Rgba { width: w, height: h, pixels: render::encoded(&linear, &params, 1) })
    }

    /// How far the healed path strays from the export's own values with nothing laid: 0 to 255.
    pub fn as_is_drift(&self, view: View) -> Result<u8, String> {
        let Some(whole) = &self.as_is else { return Ok(0) };
        let (w, h) = view.region();
        let (a, b) = (crop(whole, view.x, view.y, w, h), self.healed_as_is(&self.settings, view)?);
        Ok(a.pixels.iter().zip(&b.pixels).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0))
    }

    /// Each correction's reminder of its dust, as the tab's tiles show them, for an sRGB screen.
    pub fn originals(&self, s: &PipelineSettings, found: &[Found]) -> Result<Vec<Rgba>, String> {
        let strokes: Vec<(Vec<(f64, f64)>, f64)> = found.iter().map(|f| (f.stroke.clone(), f.radius)).collect();
        let linear = self.engine.lock()?.originals(&self.preview, s, &strokes)?;
        Ok(linear
            .iter()
            .map(|l| {
                let mut params = Params::new(Screen::Srgb, l.width as u32, l.height as u32, 0);
                params.dither = 0.0;
                Rgba { width: l.width as u32, height: l.height as u32, pixels: render::encoded(l, &params, 1) }
            })
            .collect())
    }

    /// The corrections `found` stands for, kept on the scan, as REMOVE writes them.
    pub fn corrections(&self, found: &[Found]) -> Vec<Correction> {
        found.iter().map(|f| Correction { id: Id::new(), stroke: f.stroke.iter().map(|&(x, y)| self.placing.on_scan(Point { x, y })).collect(), radius: f.radius }).collect()
    }

    /// The app's heal on `s`, each patch made in its order; `landed` hears each correction's index once
    /// its patch is written, and the heal's own time to it, `landed`'s own work left out.
    pub fn heal(&self, s: &PipelineSettings, mut landed: impl FnMut(usize, Duration) -> Result<(), String>) -> Result<(), String> {
        let mut skip = std::collections::HashSet::new();
        let (started, mut aside) = (Instant::now(), Duration::ZERO);
        let failed = std::cell::RefCell::new(None::<String>);
        oni_app::heal::ensure(&self.engine, &self.full, &self.caches, &mut skip, || failed.borrow().is_none().then(|| s.clone()), |made| {
            let at = started.elapsed() - aside;
            let result = made.map_err(|(_, e)| e).and_then(|(key, _, _)| {
                let k = s.corrections.iter().position(|c| c.id == key.id).ok_or("a patch for no correction")?;
                let own = Instant::now();
                landed(k, at)?;
                aside += own.elapsed();
                Ok(())
            });
            if let Err(e) = result {
                *failed.borrow_mut() = Some(e);
            }
        });
        failed.into_inner().map_or(Ok(()), Err)
    }
}

impl Drop for Scan {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.caches);
    }
}

/// A speck the reading found, on the framed frame: its box in pixels and the stroke REMOVE writes for it,
/// in fractions of the frame, with its radius in pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub bounds: [u32; 4],
    pub stroke: Vec<(f64, f64)>,
    pub radius: f64,
}

impl Found {
    pub fn of(s: &Speck, size: (u32, u32)) -> Found {
        let (stroke, radius) = dust::stroke(s, size.0 as usize, size.1 as usize);
        Found { bounds: s.bounds, stroke, radius }
    }

    /// The correction's mark box, `bounding` in the app: the stroke's extent padded by its radius, in pixels.
    fn mark(&self, size: (u32, u32)) -> [f64; 4] {
        let (w, h) = (f64::from(size.0), f64::from(size.1));
        let (x0, x1) = self.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0 * w), b.max(p.0 * w)));
        let (y0, y1) = self.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1 * h), b.max(p.1 * h)));
        [x0 - self.radius, y0 - self.radius, x1 - x0 + 2.0 * self.radius, y1 - y0 + 2.0 * self.radius]
    }
}

/// The dust the reading shows at `sensitivity`, as the canvas rings it.
pub fn found(reading: &dust::Reading, sensitivity: f32) -> Vec<Found> {
    let size = (reading.width as u32, reading.height as u32);
    reading.at(sensitivity).into_iter().map(|s| Found::of(s, size)).collect()
}

/// The part of the frame the strip shows: its corner on the framed frame in pixels, the zoom in the
/// strip's pixels a frame pixel, and the strip's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: u32,
    pub y: u32,
    pub zoom: f32,
    pub width: u32,
    pub height: u32,
}

impl View {
    /// The frame's pixels it covers.
    fn region(&self) -> (u32, u32) {
        ((self.width as f32 / self.zoom).ceil() as u32, (self.height as f32 / self.zoom).ceil() as u32)
    }

    /// The canvas's picture frame in the strip's points: where the whole framed frame would lie.
    fn frame(&self, size: (u32, u32)) -> (f64, f64, f64, f64) {
        let k = f64::from(self.zoom / SCALE);
        (-f64::from(self.x) * k, -f64::from(self.y) * k, f64::from(size.0) * k, f64::from(size.1) * k)
    }

    /// A box on the frame in pixels, in the strip's points.
    fn points(&self, b: [f64; 4], size: (u32, u32)) -> [f64; 4] {
        let f = self.frame(size);
        let (sx, sy) = (f.2 / f64::from(size.0), f.3 / f64::from(size.1));
        [f.0 + b[0] * sx, f.1 + b[1] * sy, b[2] * sx, b[3] * sy]
    }

    /// A speck's reading ring, as the canvas lays it: its box, the diameter at least `DustStyle.ring`.
    fn ring(&self, f: &Found, size: (u32, u32)) -> [f64; 4] {
        let b = f.bounds.map(f64::from);
        self.points([b[0], b[1], b[2] - b[0], b[3] - b[1]], size)
    }

    /// What a speck lays on the strip at any moment, its ring and its mark with the number: a box in points.
    fn footprint(&self, f: &Found, size: (u32, u32), least_ring: f64) -> [f64; 4] {
        let r = self.ring(f, size);
        let d = r[2].max(r[3]).max(least_ring);
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        let m = self.points(f.mark(size), size);
        let badge = [m[0] + m[2], m[1] - f64::from(BADGE.1), f64::from(BADGE.0), f64::from(BADGE.1)];
        let (x0, y0) = ((cx - d / 2.0).min(m[0]).min(badge[0]), (cy - d / 2.0).min(m[1]).min(badge[1]));
        let (x1, y1) = ((cx + d / 2.0).max(m[0] + m[2]).max(badge[0] + badge[2]), (cy + d / 2.0).max(m[1] + m[3]).max(badge[1] + badge[3]));
        [x0, y0, x1 - x0, y1 - y0]
    }
}

/// The view at `zoom` over the frame `size` that shows the most of `found` whole and apart: every speck it
/// touches lies `MARGIN` inside the strip, and no two footprints come within `GAP`; the middlemost on a tie.
pub fn framing(found: &[Found], size: (u32, u32), strip: (u32, u32), zoom: f32, least_ring: f64) -> Option<(View, usize)> {
    let blank = View { x: 0, y: 0, zoom, width: strip.0, height: strip.1 };
    let (rw, rh) = blank.region();
    if rw > size.0 || rh > size.1 {
        return None;
    }
    let (pw, ph) = (f64::from(strip.0) / f64::from(SCALE), f64::from(strip.1) / f64::from(SCALE));
    let step = 16;
    let mut best: Option<(View, usize, f64)> = None;
    for y in (0..=size.1 - rh).step_by(step) {
        for x in (0..=size.0 - rw).step_by(step) {
            let view = View { x, y, ..blank };
            let prints: Vec<[f64; 4]> = found.iter().map(|f| view.footprint(f, size, least_ring)).filter(|b| b[0] + b[2] > 0.0 && b[1] + b[3] > 0.0 && b[0] < pw && b[1] < ph).collect();
            let m = f64::from(MARGIN);
            if prints.is_empty() || prints.iter().any(|b| b[0] < m || b[1] < m || b[0] + b[2] > pw - m || b[1] + b[3] > ph - m) {
                continue;
            }
            let g = f64::from(GAP);
            let apart = prints.iter().enumerate().all(|(i, a)| prints[i + 1..].iter().all(|b| a[0] + a[2] + g <= b[0] || b[0] + b[2] + g <= a[0] || a[1] + a[3] + g <= b[1] || b[1] + b[3] + g <= a[1]));
            if !apart {
                continue;
            }
            let n = prints.len();
            let (cx, cy) = prints.iter().fold((0.0, 0.0), |(x, y), b| (x + b[0] + b[2] / 2.0, y + b[1] + b[3] / 2.0));
            let off = (cx / n as f64 - pw / 2.0).hypot(cy / n as f64 - ph / 2.0);
            if best.as_ref().is_none_or(|(_, bn, boff)| n > *bn || (n == *bn && off < *boff)) {
                best = Some((view, n, off));
            }
        }
    }
    best.map(|(v, n, _)| (v, n))
}

/// What the film is asked: the Sensitivity the pane rests at and the zoom, else the least that frames
/// `FEWEST` specks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ask {
    pub sensitivity: Option<f32>,
    pub zoom: Option<f32>,
    /// A rehearsal on another roll: fewer specks than `FEWEST` are filmed, not refused.
    pub rehearsal: bool,
    /// The hand goes its way and never presses, as a check's twin asks.
    pub still_hand: bool,
}

/// The film's shots in order, the frames each still is drawn from, and where the photograph lies.
pub struct Shot {
    pub frames: Vec<Rgba>,
    /// What each frame shows, read back from the pane's global and the scene.
    pub seen: Vec<Seen>,
    pub region: Region,
    pub view: View,
    /// Whether DETECT and REMOVE took the hand's clicks.
    pub detected: bool,
    pub removed: bool,
    /// The dusty detail, the rings and their count, the removal done with its tiles.
    pub stills: [usize; 3],
    pub report: String,
}

/// One frame as the pane and the canvas hold it: the pane's count, the rings' middles and the marks'
/// numbers in the strip's points, the tiles' numbers and how many patches are still awaited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub found: i32,
    pub rings: Vec<(f32, f32)>,
    pub marks: Vec<(i32, (f32, f32))>,
    pub rows: Vec<i32>,
    pub pending: usize,
    /// Where the pane's shown height ends on the stage, in points; none while it shows whole.
    pub foot: Option<f32>,
}

/// What the pane and the canvas show, as the app would set them, at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Showing {
    Dusty,
    Reading,
    Ringed,
    /// The corrections written, this many patches landed, the reminders landed or not.
    Healing(usize, bool),
}

/// The app's answers the film plays: the dust found and where the strip shows it, the reading's veil,
/// the landing order and times of the patches, and every picture shown.
struct Answers {
    found: Vec<Found>,
    view: View,
    size: (u32, u32),
    sensitivity: f32,
    mask: Rgba,
    /// Each patch's correction in the order they landed, with the heal's time to it.
    landed: Vec<(usize, f32)>,
    /// The detail after each landing, the first the dusty one.
    pictures: Vec<Rgba>,
    originals: Vec<Rgba>,
    originals_at: f32,
}

/// The De-dust film shot from the photograph at `path`, in the current language.
pub fn shoot(path: &Path, ask: Ask) -> Result<Shot, String> {
    let file = crate::film::scenes().join("welcome/dedust.slint");
    let mut stage = Stage::new(&file, "DedustShot", STAGE)?;
    stage.settle();
    let px = |name: &str| (stage.number(name) * SCALE).round() as u32;
    let strip = (px("photo-width"), px("photo-height"));
    let region = Region { x: px("photo-x"), y: px("photo-y"), width: strip.0, height: strip.1 };
    let least_ring = f64::from(stage.number("ring-least"));
    let veil = veil_of(&stage)?;

    let scan = Scan::open(path)?;
    // The detail rendered once before the reading, so its time holds no pipeline's first build.
    let warm = View { x: 0, y: 0, zoom: 1.0, width: 64, height: 64 };
    scan.detail(&scan.settings, warm)?;
    let (reading, took) = scan.read()?;
    let sensitivities: Vec<f32> = match ask.sensitivity {
        Some(s) => vec![s],
        None => (0..=20).map(|k| k as f32 / 20.0).collect(),
    };
    let zooms: Vec<f32> = ask.zoom.map_or(ZOOMS.to_vec(), |z| vec![z]);
    // The least Sensitivity and the closest zoom that frame enough specks; a rehearsal takes the most found.
    let mut best: Option<(f32, View, usize, Vec<Found>)> = None;
    'search: for &s in &sensitivities {
        let all = found(&reading, s);
        for &z in &zooms {
            if let Some((view, n)) = framing(&all, scan.size, strip, z, least_ring)
                && best.as_ref().is_none_or(|b| n > b.2)
            {
                best = Some((s, view, n, all.clone()));
                if n >= FEWEST {
                    break 'search;
                }
            }
        }
    }
    // A frame without dust is shown about its middle, at the first zoom its frame holds.
    let middle = || {
        let z = zooms.iter().copied().find(|&z| View { x: 0, y: 0, zoom: z, width: strip.0, height: strip.1 }.region().0 <= scan.size.0)?;
        let (w, h) = View { x: 0, y: 0, zoom: z, width: strip.0, height: strip.1 }.region();
        Some((sensitivities[0], View { x: (scan.size.0 - w) / 2, y: scan.size.1.saturating_sub(h) / 2, zoom: z, width: strip.0, height: strip.1 }, 0, Vec::new()))
    };
    let (sensitivity, view, shown, all) = best.or_else(middle).ok_or("the frame is smaller than the strip")?;
    if shown < FEWEST && !ask.rehearsal {
        return Err(format!("no detail shows {FEWEST} specks apart at any Sensitivity, {shown} at most"));
    }
    let reading_for = took.as_secs_f32();
    if reading_for > READING.1 {
        return Err(format!("the reading took {reading_for:.1} s, past the film's {} s", READING.1));
    }
    // An export shown as it is: its healed path must give back its own values where nothing is laid.
    let drift = scan.as_is_drift(view)?;
    if drift > 1 {
        return Err(format!("the healed export strays {drift} from its own values"));
    }
    let answers = answers_of(&scan, &reading, &all, view, sensitivity, veil)?;
    let healing_for = answers.landed.last().map_or(0.0, |l| l.1).max(answers.originals_at);
    if healing_for > HEALING {
        return Err(format!("the patches took {healing_for:.1} s, past the film's {HEALING} s"));
    }
    let report = format!(
        "{} specks at Sensitivity {}, {shown} in the detail at {}×, {} px from the left and {} from the top; read in {:.2} s, healed in {:.2} s",
        all.len(),
        Readout::Percent.format(sensitivity),
        view.zoom,
        view.x,
        view.y,
        reading_for,
        healing_for
    );

    // The hand: in from the right, DETECT, the reading, REMOVE, the patches, out to the right.
    let detect = centre_of(&stage, tr("DETECT"))?;
    let remove_key = centre_of(&stage, tr("REMOVE"))?;
    let enter = (STAGE.0 + 30.0, detect.1 + 60.0);
    let detect_down = 1.5;
    let detected = detect_down + 0.1;
    let ringed = detected + reading_for.max(READING.0);
    let remove_down = ringed + 1.2 + 0.6;
    let removed = remove_down + 0.1;
    let healed = removed + healing_for;
    let leave = healed + 0.3;
    let end = leave + 0.6 + 1.6;
    let path = Track::from(enter).hold(0.7).to(detect_down - 0.2, detect).hold(ringed + 1.2).to(remove_down - 0.2, remove_key).hold(leave).to(leave + 0.6, (STAGE.0 + 30.0, remove_key.1 + 80.0));
    let presses = Presses(if ask.still_hand { Vec::new() } else { vec![(detect_down, detected), (remove_down, removed)] });

    // The pane's keys call the app; here they mark when, and the timeline plays the app's answer.
    let asked = Rc::new(Cell::new(None::<f32>));
    let removing = Rc::new(Cell::new(None::<f32>));
    let now = Rc::new(Cell::new(0.0f32));
    for (callback, cell) in [("detect", asked.clone()), ("remove-found", removing.clone())] {
        let now = now.clone();
        stage.instance().set_global_callback("Dedust", callback, move |_| {
            if cell.get().is_none() {
                cell.set(Some(now.get()));
            }
            Value::Void
        }).map_err(|e| e.to_string())?;
    }
    let dedust = |name: &str, value: Value| stage.instance().set_global_property("Dedust", name, value).map_err(|e| format!("Dedust.{name}: {e}"));
    dedust("sensitivity", Value::Number(f64::from(sensitivity)))?;
    dedust("sensitivity-text", Value::String(Readout::Percent.format(sensitivity).into()))?;
    let radius = Correction::DEFAULT_RADIUS;
    let (low, high) = Correction::RADIUS_RANGE;
    dedust("fraction", Value::Number((radius - low) / (high - low)))?;
    let brush: SharedString = oni_app::fields::number(radius, 0, false, Unit::Pixels).into();
    dedust("readout", Value::String(brush.clone()))?;
    dedust("value-text", Value::String(brush))?;
    // What the pane is given before the first frame lands before it, its knobs at rest.
    stage.settle();

    let mut shown_as = None;
    let frames_count = (end * FPS as f32).round() as u32;
    let mut frames = Vec::with_capacity(frames_count as usize);
    let mut seen = Vec::with_capacity(frames_count as usize);
    for i in 0..frames_count {
        let t = i as f32 / FPS as f32;
        now.set(t);
        stage.pointer(path.at(t), presses.down(t));
        let showing = match (asked.get(), removing.get()) {
            (_, Some(r)) => {
                let since = t - r;
                Showing::Healing(answers.landed.iter().filter(|l| l.1 <= since).count(), answers.originals_at <= since)
            }
            (Some(a), None) if t - a >= reading_for.max(READING.0) => Showing::Ringed,
            (Some(_), None) => Showing::Reading,
            (None, None) => Showing::Dusty,
        };
        if shown_as != Some(showing) {
            show(&stage, showing, &answers)?;
            // The tiles laid once REMOVE writes them: the pane ends under their last whole row.
            if matches!(showing, Showing::Healing(..)) && stage.number("foot") == 0.0 {
                stage.set("foot", Value::Number(f64::from(foot(&stage))));
            }
            shown_as = Some(showing);
        }
        let mut shot = stage.shoot(Duration::from_millis(u64::from(i) * 1000 / u64::from(FPS)))?;
        cursor::draw(&mut shot, path.at(t), presses.down(t), SCALE);
        frames.push(shot);
        seen.push(seen_now(&stage));
    }
    let at = |t: f32| ((t * FPS as f32).round() as usize).min(frames.len() - 1);
    let stills = [at(0.5), at(ringed + 0.6), frames.len() - 1];
    Ok(Shot { frames, seen, region, view, detected: asked.get().is_some(), removed: removing.get().is_some(), stills, report })
}

/// What a photograph offers the film: its specks at a few Sensitivities, and how many each zoom's best
/// detail shows apart, for choosing the frame before it is filmed.
pub fn probe(path: &Path, stills: Option<&Path>) -> Result<String, String> {
    let mut stage = Stage::new(&crate::film::scenes().join("welcome/dedust.slint"), "DedustShot", STAGE)?;
    stage.settle();
    let strip = ((stage.number("photo-width") * SCALE).round() as u32, (stage.number("photo-height") * SCALE).round() as u32);
    let least_ring = f64::from(stage.number("ring-least"));
    let scan = Scan::open(path)?;
    let (reading, took) = scan.read()?;
    let mut lines = vec![format!("{:?}, read in {:.2} s, {} px past the window", scan.settings.mode, took.as_secs_f32(), reading.past)];
    for s in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
        let all = found(&reading, s);
        let best: Vec<String> = ZOOMS.iter().map(|&z| framing(&all, scan.size, strip, z, least_ring).map_or(format!("{z}×: none"), |(v, n)| format!("{z}×: {n} at ({}, {})", v.x, v.y))).collect();
        lines.push(format!("Sensitivity {s:.2}: {} specks; {}", all.len(), best.join(", ")));
    }
    // At rest and at 50 %, the detail at 100 % holding the most rings, as the canvas shows them.
    if let Some(out) = stills {
        let veil = veil_of(&stage)?;
        let stem = path.file_stem().map_or("frame".into(), |s| s.to_string_lossy().to_string());
        for (s, name) in [(0.0, "rest"), (0.5, "50")] {
            let all = found(&reading, s);
            let Some((view, n)) = framing(&all, scan.size, strip, 1.0, least_ring).or_else(|| busiest(&all, scan.size, strip, 1.0)) else { continue };
            let answers = answers_of(&scan, &reading, &[], view, s, veil)?;
            let dedust = |name: &str, value: Value| stage.instance().set_global_property("Dedust", name, value).map_err(|e| format!("Dedust.{name}: {e}"));
            dedust("sensitivity", Value::Number(f64::from(s)))?;
            dedust("sensitivity-text", Value::String(Readout::Percent.format(s).into()))?;
            stage.set_image("photo", &answers.pictures[0]);
            stage.set_image("veil-mask", &answers.mask);
            stage.set("veiled", Value::Bool(true));
            stage.set("rings", model(all.iter().map(|f| mark(view.ring(f, scan.size), 0)).collect()));
            lines.push(format!("the healed path strays {} from the export's values", scan.as_is_drift(view)?));
            let file = out.join(format!("probe-{stem}-{name}.png"));
            stage.shoot(Duration::ZERO)?.png(&file)?;
            lines.push(format!("{name}: {n} rings in the detail at ({}, {}), {}", view.x, view.y, file.display()));
        }
    }
    Ok(lines.join("\n"))
}

/// The found dust written, then healed by the app as REMOVE would have it, each landing's picture kept.
fn answers_of(scan: &Scan, reading: &dust::Reading, found: &[Found], view: View, sensitivity: f32, veil: [f32; 4]) -> Result<Answers, String> {
    let mut settings = scan.settings.clone();
    settings.corrections.extend(scan.corrections(found));
    let mut pictures = vec![scan.detail(&scan.settings, view)?];
    let mut landed = Vec::new();
    scan.heal(&settings, |k, at| {
        landed.push((k, at.as_secs_f32()));
        pictures.push(scan.detail(&settings, view)?);
        Ok(())
    })?;
    let started = Instant::now();
    let originals = scan.originals(&settings, found)?;
    let originals_at = started.elapsed().as_secs_f32();
    let mask = veiled(reading, view, scan.size, sensitivity, veil);
    Ok(Answers { found: found.to_vec(), view, size: scan.size, sensitivity, mask, landed, pictures, originals, originals_at })
}

/// The reading's veil over the view, sampled from its map as the canvas samples it, pixelated.
fn veiled(reading: &dust::Reading, view: View, size: (u32, u32), sensitivity: f32, veil: [f32; 4]) -> Rgba {
    let map = &reading.map;
    let (mw, mh) = (map.width as usize, map.height as usize);
    let mut texels = Vec::with_capacity((view.width * view.height) as usize);
    for v in 0..view.height {
        let fy = f64::from(view.y) + (f64::from(v) + 0.5) / f64::from(view.zoom);
        let ty = ((fy * mh as f64 / f64::from(size.1)) as usize).min(mh - 1);
        for u in 0..view.width {
            let fx = f64::from(view.x) + (f64::from(u) + 0.5) / f64::from(view.zoom);
            let tx = ((fx * mw as f64 / f64::from(size.0)) as usize).min(mw - 1);
            texels.push(map.texels[ty * mw + tx]);
        }
    }
    Rgba { width: view.width, height: view.height, pixels: oni_app::display::dust_mask(&texels, dust::notch(sensitivity), veil) }
}

/// What the app shows for `showing`: the pane's state through its global, the canvas's through the scene.
fn show(stage: &Stage, showing: Showing, answers: &Answers) -> Result<(), String> {
    let (view, size, sensitivity, count) = (answers.view, answers.size, answers.sensitivity, answers.found.len());
    let dedust = |name: &str, value: Value| stage.instance().set_global_property("Dedust", name, value).map_err(|e| format!("Dedust.{name}: {e}"));
    let (detecting, ringed) = (showing == Showing::Reading, showing == Showing::Ringed);
    dedust("detecting", Value::Bool(detecting))?;
    dedust("found-shown", Value::Bool(ringed))?;
    dedust("found", Value::Number(if ringed { count as f64 } else { 0.0 }))?;
    let note = match showing {
        Showing::Reading => tr("Reading the photo…").to_string(),
        Showing::Ringed => found_note(count, sensitivity),
        _ => String::new(),
    };
    dedust("found-note", Value::String(note.into()))?;
    stage.set_image("veil-mask", &answers.mask);
    stage.set("veiled", Value::Bool(ringed));
    let frame = view.frame(size);
    let rings: Vec<Value> = if ringed { answers.found.iter().map(|f| mark(view.ring(f, size), 0)).collect() } else { Vec::new() };
    stage.set("rings", model(rings));
    let (patched, reminded) = match showing {
        Showing::Healing(n, r) => (Some(n), r),
        _ => (None, false),
    };
    let landed: Vec<usize> = patched.map_or(Vec::new(), |n| answers.landed.iter().take(n).map(|l| l.0).collect());
    stage.set_image("photo", &answers.pictures[patched.unwrap_or(0)]);
    let marks: Vec<Value> = match patched {
        Some(_) => answers.found.iter().enumerate().map(|(k, f)| mark(view.points(f.mark(size), size), k + 1)).collect(),
        None => Vec::new(),
    };
    stage.set("marks", model(marks));
    let pending: Vec<Value> = match patched {
        Some(_) => answers
            .found
            .iter()
            .enumerate()
            .filter(|(k, _)| !landed.contains(k))
            .map(|(_, f)| {
                let commands = traced(&f.stroke, frame);
                let width = f.radius * 2.0 * frame.2 / f64::from(size.0);
                Value::Struct(Struct::from_iter([("commands".to_string(), Value::String(commands.into())), ("width".to_string(), Value::Number(width))]))
            })
            .collect(),
        None => Vec::new(),
    };
    stage.set("pending", model(pending));
    let rows: Vec<Value> = match patched {
        Some(_) => (0..answers.found.len())
            .map(|k| {
                let n = k + 1;
                let original = if reminded { image(&answers.originals[k]) } else { slint::Image::default() };
                Value::Struct(Struct::from_iter([
                    ("label".to_string(), Value::String(oni_i18n::tr!("Correction {n}", n = n).into())),
                    ("number".to_string(), Value::Number(n as f64)),
                    ("pending".to_string(), Value::Bool(!landed.contains(&k))),
                    ("fallback".to_string(), Value::Bool(false)),
                    ("original".to_string(), Value::Image(original)),
                ]))
            })
            .collect(),
        None => Vec::new(),
    };
    dedust("rows", model(rows))
}

/// The pane's note on a reading, in the app's words.
fn found_note(count: usize, sensitivity: f32) -> String {
    match count {
        0 if sensitivity >= 1.0 => tr("No dust found, even at full Sensitivity").to_string(),
        0 => tr("No dust found: raise Sensitivity to catch fainter specks").to_string(),
        n => trn("{n} speck of dust ringed", "{n} specks of dust ringed", n as u64),
    }
}

/// A stroke on the frame as the canvas traces it, `f` the frame's place in points.
fn traced(points: &[(f64, f64)], f: (f64, f64, f64, f64)) -> String {
    points.iter().enumerate().map(|(i, p)| format!("{} {:.2} {:.2}", if i == 0 { "M" } else { " L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3)).collect()
}

fn mark(b: [f64; 4], number: usize) -> Value {
    Value::Struct(Struct::from_iter([
        ("x".to_string(), Value::Number(b[0])),
        ("y".to_string(), Value::Number(b[1])),
        ("width".to_string(), Value::Number(b[2])),
        ("height".to_string(), Value::Number(b[3])),
        ("number".to_string(), Value::Number(number as f64)),
    ]))
}

fn model(values: Vec<Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(values)))
}

fn image(p: &Rgba) -> slint::Image {
    slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&p.pixels, p.width, p.height))
}

/// `DustStyle.veil` as the kernel takes it, straight, 0 to 1.
fn veil_of(stage: &Stage) -> Result<[f32; 4], String> {
    match stage.get("veil") {
        Value::Brush(b) => {
            let c = b.color();
            Ok([c.red(), c.green(), c.blue(), c.alpha()].map(|v| f32::from(v) / 255.0))
        }
        other => Err(format!("the scene's veil is no colour: {other:?}")),
    }
}

/// Where the pane's shown height ends, from its top: under the last row of tiles the stage holds whole,
/// each tile's foot its remove key's; with no tile, the whole pane.
fn foot(stage: &Stage) -> f32 {
    let top = stage.number("pane-y");
    ElementHandle::find_by_accessible_label(stage.instance(), tr("Remove This Correction"))
        .filter(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .map(|e| e.absolute_position().y + e.size().height)
        .filter(|&bottom| bottom <= STAGE.1)
        .fold(0.0, f32::max)
        .max(top)
        - top
}

/// The middle of the button labelled `label`, in points.
fn centre_of(stage: &Stage, label: &str) -> Result<(f32, f32), String> {
    let found = ElementHandle::find_by_accessible_label(stage.instance(), label)
        .filter(|e| e.accessible_role() == Some(AccessibleRole::Button) && e.size().width > 0.0)
        .map(|e| (e.absolute_position(), e.size()))
        .next()
        .ok_or(format!("no {label} key in the scene"))?;
    Ok((found.0.x + found.1.width / 2.0, found.0.y + found.1.height / 2.0))
}

/// The shot as a GIF, packed as the help demos are.
pub fn gif(shot: &Shot) -> Result<Vec<u8>, String> {
    let clip = Clip { frames: shot.frames.clone(), delay: (100 / FPS) as u16, photo: shot.region };
    encode::gif(&clip, Tuning { tolerance: 8, colours: 128 })
}

/// What the stage holds now: the pane's global and the scene's overlays, as the frame shows them.
fn seen_now(stage: &Stage) -> Seen {
    let global = |name: &str| stage.instance().get_global_property("Dedust", name).ok();
    let list = |v: Option<Value>| match v {
        Some(Value::Model(m)) => m.iter().collect::<Vec<Value>>(),
        _ => Vec::new(),
    };
    let field = |v: &Value, name: &str| match v {
        Value::Struct(s) => match s.get_field(name) {
            Some(Value::Number(n)) => *n as f32,
            _ => 0.0,
        },
        _ => 0.0,
    };
    let middle = |v: &Value| (field(v, "x") + field(v, "width") / 2.0, field(v, "y") + field(v, "height") / 2.0);
    Seen {
        found: match global("found") {
            Some(Value::Number(n)) => n as i32,
            _ => -1,
        },
        rings: list(Some(stage.get("rings"))).iter().map(middle).collect(),
        marks: list(Some(stage.get("marks"))).iter().map(|m| (field(m, "number") as i32, middle(m))).collect(),
        rows: list(global("rows")).iter().map(|r| field(r, "number") as i32).collect(),
        pending: list(Some(stage.get("pending"))).len(),
        foot: (stage.number("foot") > 0.0).then(|| stage.number("pane-y") + stage.number("foot")),
    }
}

/// `picture` reduced `by` times to `width` × `height` from its top left, each pixel the mean of the
/// block it stands for, as a canvas zoomed under one to one shows it.
pub fn reduced(picture: &Rgba, by: u32, width: u32, height: u32) -> Rgba {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (mut sum, mut n) = ([0u32; 4], 0u32);
            for (px, py) in (0..by * by).map(|k| (x * by + k % by, y * by + k / by)) {
                if px < picture.width && py < picture.height {
                    let at = ((py * picture.width + px) * 4) as usize;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += u32::from(picture.pixels[at + c]);
                    }
                    n += 1;
                }
            }
            pixels.extend(sum.map(|v| (v + n / 2).checked_div(n).unwrap_or(0) as u8));
        }
    }
    Rgba { width, height, pixels }
}

/// `picture`, a view's region, at the view's zoom.
fn zoomed(picture: &Rgba, view: View) -> Rgba {
    if view.zoom >= 1.0 { magnified(picture, view.zoom as u32, view.width, view.height) } else { reduced(picture, (1.0 / view.zoom).round() as u32, view.width, view.height) }
}

/// `w` × `h` of `p` from `(x, y)`.
fn crop(p: &Rgba, x: u32, y: u32, w: u32, h: u32) -> Rgba {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * p.width + x) * 4) as usize;
        pixels.extend_from_slice(&p.pixels[at..at + (w * 4) as usize]);
    }
    Rgba { width: w, height: h, pixels }
}

/// An export's own code values, read as the app reads a plain image: an sRGB file's are the screen's.
fn code_values(path: &Path) -> Result<Rgba, String> {
    let read = oni_decode::plain::read(path, None).ok_or("the export does not decode")?;
    let srgb = |icc: &Vec<u8>| icc.windows(4).any(|t| t == b"sRGB") || icc.windows(8).any(|t| t == b"\0s\0R\0G\0B");
    if read.icc.as_ref().is_some_and(|icc| !srgb(icc)) {
        return Err("the export's profile is not sRGB: its values are not the screen's".into());
    }
    let pixels = read.pixels.iter().flat_map(|p| [p[0], p[1], p[2]].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).into_iter().chain([255])).collect();
    Ok(Rgba { width: read.width as u32, height: read.height as u32, pixels })
}

/// The view at `zoom` holding the most specks, about the one with most neighbours, for a probe's look
/// when no detail holds them apart.
fn busiest(found: &[Found], size: (u32, u32), strip: (u32, u32), zoom: f32) -> Option<(View, usize)> {
    let blank = View { x: 0, y: 0, zoom, width: strip.0, height: strip.1 };
    let (rw, rh) = blank.region();
    if rw > size.0 || rh > size.1 {
        return None;
    }
    found
        .iter()
        .map(|f| {
            let (cx, cy) = ((f.bounds[0] + f.bounds[2]) / 2, (f.bounds[1] + f.bounds[3]) / 2);
            let view = View { x: cx.saturating_sub(rw / 2).min(size.0 - rw), y: cy.saturating_sub(rh / 2).min(size.1 - rh), ..blank };
            let inside = |g: &Found| g.bounds[0] >= view.x && g.bounds[2] <= view.x + rw && g.bounds[1] >= view.y && g.bounds[3] <= view.y + rh;
            (view, found.iter().filter(|g| inside(g)).count())
        })
        .max_by_key(|(_, n)| *n)
}
