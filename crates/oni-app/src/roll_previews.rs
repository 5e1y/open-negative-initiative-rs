//! `RollPreviews` and the base's scans: each frame at rest on the roll's film and flat-field, framed
//! then inverted on screen values; the base's scan unturned and uninverted, its loupe full size.

use crate::display::{Params, Screen};
use crate::render::{self, Linear, Reading};
use oni_graph::{Graph, Image};
use oni_pipeline::{Inputs, Pipeline};
use oni_settings::frame::FlatField;
use oni_settings::{ConversionMode, PipelineSettings};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// `RollPreviews.tileSide`: a tile's long side, in pixels.
pub const TILE_SIDE: f64 = 360.0;
/// `RollPreviews.gridPause`: the rest before the tiles follow a new look, or the card a new frame.
pub const GRID_PAUSE: Duration = Duration::from_millis(250);
/// `RollBaseSurface`: the rest on one scan before its full resolution is decoded for the loupe.
pub const FULL_PAUSE: Duration = Duration::from_millis(300);

/// What the frames are shown on: the roll's film and its flat-field, none while neutral.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub film: ConversionMode,
    pub flat: Option<FlatField>,
}

/// An 8-bit sRGB picture, straight alpha, row-major.
#[derive(Clone, Debug)]
pub struct Bitmap {
    pub rgba: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

/// What the worker hands the window, each for the session it was asked in.
#[derive(Debug)]
pub enum Out {
    Tile(String, Bitmap),
    Card(String, Bitmap),
    Scan(String, Bitmap),
    Full(String, Bitmap),
}

/// `RollPreviews.rendered`: the frame carried onto the roll's film at rest, its flat-field the roll's
/// when one is set, per-pixel stages only.
pub fn rendered(base: &PipelineSettings, look: &Look) -> PipelineSettings {
    let mut s = oni_core::parameters::carried(base, look.film);
    if let Some(flat) = &look.flat {
        s.flat_field = flat.clone();
    }
    s.per_pixel_only()
}

/// `RollPreviews.inverted`: each channel's complement under its alpha, a monochrome film's on its
/// Rec. 709 luma, so a negative reads as its print.
pub fn inverted(rgba: &mut [u8], grey: bool) {
    for p in rgba.chunks_exact_mut(4) {
        let alpha = p[3];
        if grey {
            let luma = (0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])).round().min(f64::from(alpha)) as u8;
            let v = alpha - luma;
            p[0] = v;
            p[1] = v;
            p[2] = v;
        } else {
            for c in 0..3 {
                p[c] = alpha - p[c].min(alpha);
            }
        }
    }
}

struct Source {
    linear: Linear,
    full_width: f64,
}

#[derive(Default)]
struct Queue {
    session: u64,
    settings: HashMap<String, PipelineSettings>,
    sources: HashMap<String, Arc<Source>>,
    decode: VecDeque<String>,
    look: Option<Look>,
    retile: VecDeque<String>,
    retile_at: Option<Instant>,
    shown: Option<String>,
    card_now: bool,
    large: Option<(String, Arc<Source>)>,
    large_at: Option<(String, Instant)>,
    scan: Option<String>,
    full_at: Option<(String, Instant)>,
}

enum Work {
    Card(String, Arc<Source>, PipelineSettings, Look),
    Scan(String),
    Large(String),
    Decode(String),
    Tile(String, Arc<Source>, PipelineSettings, Look),
    Full(String),
}

impl Queue {
    /// The next job, the card first, then the base's scan, then the tiles; else how long to wait.
    fn next(&mut self, now: Instant) -> Result<Work, Option<Duration>> {
        if self.card_now {
            self.card_now = false;
            if let (Some(look), Some(path)) = (self.look.clone(), self.shown.clone())
                && let Some(base) = self.settings.get(&path).cloned()
            {
                let source = match &self.large {
                    Some((p, s)) if *p == path => Some(s.clone()),
                    _ => self.sources.get(&path).cloned(),
                };
                if let Some(source) = source {
                    return Ok(Work::Card(path, source, base, look));
                }
            }
        }
        if let Some(path) = self.scan.take() {
            return Ok(Work::Scan(path));
        }
        if let Some((path, at)) = self.large_at.clone()
            && at <= now
        {
            self.large_at = None;
            return Ok(Work::Large(path));
        }
        if let Some(path) = self.decode.pop_front() {
            return Ok(Work::Decode(path));
        }
        if self.retile_at.is_some_and(|at| at <= now) {
            self.retile_at = None;
            self.retile = self.sources.keys().cloned().collect();
        }
        while let Some(path) = self.retile.pop_front() {
            if let (Some(look), Some(source), Some(base)) = (self.look.clone(), self.sources.get(&path).cloned(), self.settings.get(&path).cloned()) {
                return Ok(Work::Tile(path, source, base, look));
            }
        }
        if let Some((path, at)) = self.full_at.clone()
            && at <= now
        {
            self.full_at = None;
            return Ok(Work::Full(path));
        }
        let waits = [self.large_at.as_ref().map(|l| l.1), self.retile_at, self.full_at.as_ref().map(|f| f.1)];
        Err(waits.into_iter().flatten().min().map(|at| at.saturating_duration_since(now)))
    }
}

type Post = Box<dyn Fn(u64, Out) + Send + Sync>;

/// The worker's handle; one thread with its own graph, so no preview waits on the canvas.
pub struct Previews {
    queue: Arc<(Mutex<Queue>, Condvar)>,
}

impl Previews {
    pub fn start(post: impl Fn(u64, Out) + Send + Sync + 'static) -> Previews {
        let queue: Arc<(Mutex<Queue>, Condvar)> = Arc::default();
        let held = queue.clone();
        let post: Post = Box::new(post);
        std::thread::Builder::new().name("roll previews".into()).spawn(move || run(held, post)).expect("roll previews thread");
        Previews { queue }
    }

    fn with(&self, change: impl FnOnce(&mut Queue)) {
        let (lock, wake) = &*self.queue;
        change(&mut lock.lock().expect("roll previews"));
        wake.notify_one();
    }

    /// `reset()`: everything dropped; what the worker still finishes lands on a session gone.
    pub fn reset(&self, session: u64) {
        self.with(|q| *q = Queue { session, ..Queue::default() });
    }

    /// `add(_:settings:)`: the frame's tile decoded after those asked before it.
    pub fn add(&self, path: &str, settings: &PipelineSettings) {
        self.with(|q| {
            q.settings.insert(path.to_string(), settings.clone());
            q.decode.push_back(path.to_string());
        });
    }

    /// `update(_:)`: a new look renders the card now and every tile after the grid's pause.
    pub fn update(&self, look: Look) {
        self.with(|q| {
            if q.look.as_ref() == Some(&look) {
                return;
            }
            q.look = Some(look);
            q.card_now = true;
            q.retile.clear();
            q.retile_at = Some(Instant::now() + GRID_PAUSE);
        });
    }

    /// `show(_:)`: the card from the tile's source now, from the measuring copy after the pause.
    pub fn show(&self, path: Option<&str>) {
        self.with(|q| {
            q.shown = path.map(str::to_string);
            q.card_now = true;
            match path {
                Some(p) if q.large.as_ref().is_none_or(|(l, _)| l != p) => q.large_at = Some((p.to_string(), Instant::now() + GRID_PAUSE)),
                _ => q.large_at = None,
            }
        });
    }

    /// The base step's scan of `path`, the last asked winning, then its full size once it rests.
    pub fn scan(&self, path: Option<&str>) {
        self.with(|q| {
            q.scan = path.map(str::to_string);
            q.full_at = None;
        });
    }
}

fn run(queue: Arc<(Mutex<Queue>, Condvar)>, post: Post) {
    let mut engine: Option<(Graph, Pipeline)> = None;
    let mut seed = 0u32;
    loop {
        let (work, session) = {
            let (lock, wake) = &*queue;
            let mut q = lock.lock().expect("roll previews");
            loop {
                match q.next(Instant::now()) {
                    Ok(work) => break (work, q.session),
                    Err(Some(wait)) => q = wake.wait_timeout(q, wait).expect("roll previews").0,
                    Err(None) => q = wake.wait(q).expect("roll previews"),
                }
            }
        };
        if engine.is_none() {
            engine = Graph::new().ok().map(|g| (g, Pipeline::new()));
        }
        let Some((graph, pipeline)) = engine.as_mut() else { continue };
        seed = seed.wrapping_add(1);
        let current = |q: &Queue| q.session == session;
        match work {
            Work::Card(path, source, base, look) => {
                if let Some(b) = picture(graph, pipeline, &source, &rendered(&base, &look), seed) {
                    post(session, Out::Card(path, b));
                }
            }
            Work::Scan(path) => {
                let scan = reduced(graph, &path, oni_measure::reader::MEASURE_SIDE).map(|s| encoded(&s.linear, seed));
                let (lock, _) = &*queue;
                let mut q = lock.lock().expect("roll previews");
                if current(&q) && q.scan.is_none() {
                    q.full_at = Some((path.clone(), Instant::now() + FULL_PAUSE));
                    drop(q);
                    if let Some(b) = scan {
                        post(session, Out::Scan(path, b));
                    }
                }
            }
            Work::Full(path) => {
                let full = render::decode_full(Path::new(&path), reading(&path)).ok().map(|d| encoded(&d.linear, seed));
                let (lock, _) = &*queue;
                let still = { let q = lock.lock().expect("roll previews"); current(&q) && q.scan.is_none() && q.full_at.is_none() };
                if still && let Some(b) = full {
                    post(session, Out::Full(path, b));
                }
            }
            Work::Large(path) => {
                let large = reduced(graph, &path, oni_measure::reader::MEASURE_SIDE).map(Arc::new);
                let (lock, wake) = &*queue;
                let mut q = lock.lock().expect("roll previews");
                if let Some(large) = large.filter(|_| current(&q) && q.shown.as_deref() == Some(path.as_str())) {
                    q.large = Some((path, large));
                    q.card_now = true;
                    wake.notify_one();
                }
            }
            Work::Decode(path) => {
                let source = reduced(graph, &path, TILE_SIDE).map(Arc::new);
                let (lock, _) = &*queue;
                let mut q = lock.lock().expect("roll previews");
                let (Some(source), true) = (source, current(&q) && q.settings.contains_key(&path)) else { continue };
                q.sources.insert(path.clone(), source.clone());
                if q.shown.as_deref() == Some(path.as_str()) && q.large.is_none() {
                    q.card_now = true;
                }
                if q.look.is_some() {
                    q.retile.push_front(path);
                }
            }
            Work::Tile(path, source, base, look) => {
                let b = picture(graph, pipeline, &source, &rendered(&base, &look), seed);
                let (lock, _) = &*queue;
                let fresh = { let q = lock.lock().expect("roll previews"); current(&q) && q.look.as_ref() == Some(&look) };
                if fresh && let Some(b) = b {
                    post(session, Out::Tile(path, b));
                }
            }
        }
        graph.purge();
    }
}

/// The frame's own reading off its sidecar, a lead decoding its passes laid, those a grouping holds first.
fn reading(path: &str) -> Reading {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let mut r = oni_settings::sidecar::read(Path::new(path), mosaic, true).map(|l| Reading::of(&l.settings)).unwrap_or_default();
    if let Some(held) = oni_library::rgb_passes::held(path) {
        r.passes = Some(held);
    }
    r
}

/// `RawDecode.linear(longestSide:)`, or the measuring copy: the decode reduced to `side`, its edge rebuilt.
fn reduced(graph: &mut Graph, path: &str, side: f64) -> Option<Source> {
    let decoded = render::decode(Path::new(path), reading(path)).ok()?;
    let l = decoded.linear;
    let uploaded = graph.upload(l.width as u32, l.height as u32, &l.pixels).ok()?;
    drop(l.pixels);
    let long = l.width.max(l.height) as f64;
    let mut image = Image::new(&uploaded);
    if long > side {
        image = oni_pipeline::stages::rebuilt_edge(image.lanczos(side / long));
    }
    let e = image.extent();
    let pixels = graph.render(&image, e, None).ok()?;
    Some(Source { linear: Linear { width: e.w as usize, height: e.h as usize, pixels }, full_width: decoded.full_width })
}

/// `Pipeline.framed` on a held source, then on screen values, inverted unless the film is positive.
fn picture(graph: &mut Graph, pipeline: &mut Pipeline, source: &Source, settings: &PipelineSettings, seed: u32) -> Option<Bitmap> {
    let l = &source.linear;
    let uploaded = graph.upload(l.width as u32, l.height as u32, &l.pixels).ok()?;
    let scale = if source.full_width > 0.0 { l.width as f64 / source.full_width } else { 1.0 };
    let framed = pipeline.framed(graph, Image::new(&uploaded), settings, scale, &Inputs::default()).ok()?;
    let e = framed.extent();
    if e.w == 0 || e.h == 0 {
        return None;
    }
    let pixels = graph.render(&framed, e, None).ok()?;
    let mut b = encoded(&Linear { width: e.w as usize, height: e.h as usize, pixels }, seed);
    if settings.mode != ConversionMode::Positive {
        inverted(&mut b.rgba, settings.mode.is_monochrome());
    }
    Some(b)
}

fn encoded(linear: &Linear, seed: u32) -> Bitmap {
    let (w, h) = (linear.width, linear.height);
    Bitmap { rgba: render::encoded(linear, &Params::new(Screen::Srgb, w as u32, h as u32, seed), 1), width: w, height: h }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `RollPreviews.selfCheck`'s bytes: a colour pixel's complement, a grey film's luma complement.
    #[test]
    fn a_negative_reads_as_its_complement_and_a_grey_film_on_its_luma() {
        let mut colour = vec![13, 201, 67, 255];
        inverted(&mut colour, false);
        assert_eq!(colour, [242, 54, 188, 255]);
        let mut grey = vec![13, 201, 67, 255];
        inverted(&mut grey, true);
        assert_eq!(grey, [104, 104, 104, 255]);
        // Twin: under a half alpha the complement is taken under the alpha, never under 255.
        let mut half = vec![13, 201, 67, 128];
        inverted(&mut half, false);
        assert_eq!(half, [115, 0, 61, 128]);
        assert_ne!(half[..3], [242, 54, 188]);
    }

    #[test]
    fn a_tile_shows_the_frame_at_rest_on_the_rolls_film_and_flat_field() {
        let mut base = PipelineSettings::rest(ConversionMode::Negative, true);
        base.levels.red.black = 0.3;
        let flat = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
        let s = rendered(&base, &Look { film: ConversionMode::Positive, flat: Some(flat.clone()) });
        assert_eq!((s.mode, s.flat_field.clone()), (ConversionMode::Positive, flat));
        // Twin: no flat-field on the look leaves the frame's own.
        let own = rendered(&base, &Look { film: ConversionMode::Negative, flat: None });
        assert_eq!(own.flat_field, base.flat_field);
    }

    #[test]
    fn the_card_comes_first_and_a_new_look_waits_for_the_grids_pause() {
        let mut q = Queue::default();
        let s = PipelineSettings::rest(ConversionMode::Negative, true);
        let source = Arc::new(Source { linear: Linear { width: 1, height: 1, pixels: vec![[0.5; 4]] }, full_width: 1.0 });
        q.settings.insert("a".into(), s.clone());
        q.sources.insert("a".into(), source);
        q.look = Some(Look { film: ConversionMode::Negative, flat: None });
        q.shown = Some("a".into());
        q.card_now = true;
        q.decode.push_back("b".into());
        let now = Instant::now();
        assert!(matches!(q.next(now), Ok(Work::Card(p, ..)) if p == "a"));
        assert!(matches!(q.next(now), Ok(Work::Decode(p)) if p == "b"));
        q.retile_at = Some(now + GRID_PAUSE);
        // Twin: before the pause no tile is rendered again; after it, every source is.
        assert!(matches!(q.next(now), Err(Some(_))));
        assert!(matches!(q.next(now + GRID_PAUSE), Ok(Work::Tile(p, ..)) if p == "a"));
    }
}
