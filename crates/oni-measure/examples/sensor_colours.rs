//! `sensor_colours <roll> <out> <frame>…`: the roll measured as Prepare the Roll measures it in each of
//! the decode's colours, then the frames named rendered on the Pro face and the Frontier, as `.f32` dumps.

use oni_core::balance;
use oni_decode::Colours;
use oni_graph::Graph;
use oni_measure::key::{Decoded, Measurer, PREVIEW_SIDE};
use oni_measure::truncate::Stage;
use oni_measure::{auto_levels, key, pipette, roll};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::blocks::Gains;
use oni_settings::cg::{Point, Rect};
use oni_settings::frame::{Geometry, RollReference};
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::sidecar::{self, RollDraft};
use oni_settings::{ConversionMode, PipelineSettings};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

const COLOURS: [(&str, Colours); 3] = [("matrix", Colours::Matrix), ("sensor", Colours::Sensor), ("unbalanced", Colours::SensorUnbalanced)];

/// The colours the measurer's next decode reads in, an index into `COLOURS`.
static READING: AtomicU8 = AtomicU8::new(0);

/// The longest side of every render, the side Swift's `--render 1000` writes.
const SIDE: f64 = 1000.0;
/// The grid the base is looked for on, in steps across each side of every frame.
const BASE_GRID: usize = 96;
/// The band along each side of a frame the base is looked for in, a fraction of the side.
const EDGE: f64 = 0.07;
/// Squares the decodes are averaged on for the per-camera fit, as `apple-decode` writes them.
const BLOCK: usize = 8;

struct Frame {
    path: PathBuf,
    name: String,
    settings: PipelineSettings,
    /// The roll's film, its measuring window, and whether the frame rides without a vote: the draft's
    /// when one is left, else what the measure wrote on the frame.
    film: ConversionMode,
    window: Rect,
    flagged: bool,
}

fn frames(roll: &Path) -> Vec<Frame> {
    let mut out: Vec<Frame> = std::fs::read_dir(roll)
        .expect("the roll")
        .filter_map(|e| {
            let path = e.ok()?.path();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if !["nef", "rw2"].contains(&ext.as_str()) {
                return None;
            }
            let loaded = sidecar::read(&path, true, false).ok()?;
            loaded.settings.roll_reference.as_ref()?;
            let name = path.file_name()?.to_str()?.to_owned();
            let draft: Option<RollDraft> = loaded.roll_draft;
            let s = loaded.settings;
            let film = draft.as_ref().map_or(s.mode, |d| d.film);
            let window = draft.as_ref().and_then(|d| d.window).or(s.geometry.measure_window).expect("a measuring window");
            let flagged = draft.as_ref().is_some_and(|d| d.flagged || !d.ticked);
            Some(Frame { path, name, settings: s, film, window, flagged })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn measurer() -> Measurer {
    Measurer::new(
        Graph::new().expect("a GPU"),
        Box::new(|path: &Path, _: &PipelineSettings| {
            let colours = COLOURS[READING.load(Ordering::Relaxed) as usize].1;
            let (d, factor) = oni_decode::decode_in(path, 1.0, PREVIEW_SIDE as usize, colours).map_err(|e| format!("LibRaw {}", e.0))?;
            Ok(Decoded { width: d.width as u32, height: d.height as u32, full_width: (d.width * factor) as f64, pixels: d.pixels })
        }),
        Box::new(|_: &PipelineSettings| format!("{}", READING.load(Ordering::Relaxed))),
    )
}

/// `key::base`'s bare frame: framing, gains and dust stripped, on the roll's film.
fn bare(open: &PipelineSettings, film: ConversionMode) -> PipelineSettings {
    let mut bare = oni_core::parameters::imported(open);
    bare.mode = film;
    bare.geometry = Geometry::default();
    bare.gains = Gains::default();
    bare.corrections = Vec::new();
    bare
}

fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

/// Where on the roll the bare film reads what the Swift app recorded, the pick itself being unrecorded.
fn find_base(m: &Measurer, frames: &[Frame], film: ConversionMode, recorded: [f32; 3]) -> (usize, Point, f32) {
    let mut best = (0, Point::new(0.5, 0.5), f32::INFINITY);
    for (i, f) in frames.iter().enumerate() {
        let s = bare(&f.settings, film);
        let Some(plane) = m.with(&f.name_path(), &s, |r, copy| r.plane(copy, &s, Stage::Levels)) else { continue };
        for gy in 1..BASE_GRID {
            for gx in 1..BASE_GRID {
                let unit = Point::new(gx as f64 / BASE_GRID as f64, gy as f64 / BASE_GRID as f64);
                // Only the rebate is bare film; inside the picture a colour can match it on one decode only.
                if (EDGE..1.0 - EDGE).contains(&unit.x) && (EDGE..1.0 - EDGE).contains(&unit.y) {
                    continue;
                }
                let Some(read) = pipette::read(&plane, unit, film, roll::BASE_RADIUS, pipette::RETAINED_SPACE, pipette::RETAINED_STATISTIC) else { continue };
                let gap = worst(read.fractions, recorded);
                if gap < best.2 {
                    best = (i, unit, gap);
                }
            }
        }
    }
    best
}

impl Frame {
    fn name_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

fn json3(v: [f32; 3]) -> String {
    format!("[{:.6}, {:.6}, {:.6}]", v[0], v[1], v[2])
}

fn reference_json(r: &RollReference) -> String {
    format!(
        "{{\"stops\": {}, \"black\": {}, \"white\": {}, \"base\": {}, \"median\": {}}}",
        json3(r.stops),
        json3(r.black),
        json3(r.white),
        r.base.map_or("null".into(), json3),
        r.median.map_or("null".into(), json3)
    )
}

fn write(path: &Path, w: usize, h: usize, px: &[[f32; 4]]) {
    let mut bytes = Vec::with_capacity(8 + px.len() * 16);
    bytes.extend_from_slice(&(w as i32).to_le_bytes());
    bytes.extend_from_slice(&(h as i32).to_le_bytes());
    px.iter().flatten().for_each(|v| bytes.extend_from_slice(&v.to_le_bytes()));
    std::fs::write(path, bytes).expect("dump written");
}

/// The decode averaged on `BLOCK` squares from the top left, as `apple-decode` reduces Apple's.
fn blocks(d: &oni_decode::Linear) -> (usize, usize, Vec<[f32; 4]>) {
    let (bw, bh) = (d.width / BLOCK, d.height / BLOCK);
    let mut out = vec![[0f32, 0.0, 0.0, 1.0]; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let mut s = [0f64; 3];
            for yy in 0..BLOCK {
                for xx in 0..BLOCK {
                    let p = d.pixels[(y * BLOCK + yy) * d.width + x * BLOCK + xx];
                    (0..3).for_each(|c| s[c] += f64::from(p[c]));
                }
            }
            (0..3).for_each(|c| out[y * bw + x][c] = (s[c] / (BLOCK * BLOCK) as f64) as f32);
        }
    }
    (bw, bh, out)
}

/// The share of pixels with a channel under zero, the share with one over the white level, the
/// lowest first centile per channel and the largest per channel.
fn range(d: &oni_decode::Linear) -> String {
    let n = d.pixels.len() as f64;
    let under = d.pixels.iter().filter(|p| p[0] < 0.0 || p[1] < 0.0 || p[2] < 0.0).count() as f64 / n;
    let over = d.pixels.iter().filter(|p| p[0] > 1.0 || p[1] > 1.0 || p[2] > 1.0).count() as f64 / n;
    let low = [0, 1, 2].map(|c| {
        let mut v: Vec<f32> = d.pixels.iter().step_by(7).map(|p| p[c]).collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 1000]
    });
    let high = [0, 1, 2].map(|c| d.pixels.iter().map(|p| p[c]).fold(f32::MIN, f32::max));
    format!("{{\"under_zero\": {under:.6}, \"over_one\": {over:.6}, \"p01\": {}, \"max\": {}}}", json3(low), json3(high))
}

fn on_face(s: &PipelineSettings, face: &str) -> PipelineSettings {
    let mut s = s.per_pixel_only();
    if face == "pro" {
        s.face = Face::Pro;
    } else {
        s.face = Face::Scanner;
        s.scanner_machine = ScannerMachine::Frontier;
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (roll_dir, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let shown: Vec<&String> = args[2..].iter().collect();
    std::fs::create_dir_all(out).expect("the output folder");
    let frames = frames(roll_dir);
    let film = frames[0].film;
    let swift = frames[0].settings.roll_reference.clone().expect("a prepared roll");
    let recorded = swift.base.map(|b| [0, 1, 2].map(|c| b[c] - auto_levels::shift(swift.stops[c], film))).expect("a picked base");
    println!("{} frames on {:?}; Swift's base before its gains {recorded:?}", frames.len(), film);
    let m = measurer();
    let mut report = vec![format!("\"swift\": {}", reference_json(&swift))];

    READING.store(0, Ordering::Relaxed);
    let clock = std::time::Instant::now();
    let (at, unit, gap) = find_base(&m, &frames, film, recorded);
    println!("base found on {} at ({:.3}, {:.3}), {gap:.4} from Swift's, in {:.0} s", frames[at].name, unit.x, unit.y, clock.elapsed().as_secs_f64());
    report.push(format!("\"base_pick\": {{\"frame\": \"{}\", \"x\": {:.4}, \"y\": {:.4}, \"gap\": {gap:.5}}}", frames[at].name, unit.x, unit.y));

    let mut references = Vec::new();
    for (k, (label, _)) in COLOURS.iter().enumerate() {
        READING.store(k as u8, Ordering::Relaxed);
        let mut measured = Vec::new();
        for f in &frames {
            if f.flagged {
                continue;
            }
            let start = roll::start(&f.settings, film, f.window, None, f.settings.capture_stops);
            if let Some(read) = m.with(&f.name_path(), &start, |r, copy| key::roll_measured(r, copy, &f.name_path(), &start)) {
                measured.push(read);
            }
        }
        let pick = &frames[at];
        let open = bare(&pick.settings, film);
        let base = m.with(&pick.name_path(), &open, |r, copy| key::base(r, copy, &pick.settings, film, None, pick.settings.capture_stops, unit, roll::BASE_RADIUS));
        let reference = roll::combined(&measured, film, base).expect("the roll combines");
        println!("{label}: {} frames, stops {:?}, black {:?}, white {:?}", measured.len(), reference.stops, reference.black, reference.white);
        report.push(format!("\"{label}\": {}", reference_json(&reference)));
        references.push(reference);
    }

    let mut g = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    let mut ranges = Vec::new();
    for f in frames.iter().filter(|f| shown.iter().any(|s| **s == f.name)) {
        let start = roll::start(&f.settings, film, f.window, None, f.settings.capture_stops);
        let rest = PipelineSettings::rest(film, true);
        for (k, (label, colours)) in COLOURS.iter().enumerate() {
            let (d, _) = oni_decode::decode_in(&f.path, 1.0, 0, *colours).expect("LibRaw decodes the frame");
            ranges.push(format!("\"{}.{label}\": {}", f.name, range(&d)));
            if *colours != Colours::SensorUnbalanced {
                let (bw, bh, b) = blocks(&d);
                write(&out.join(format!("{}.{label}.blocks.f32", f.name)), bw, bh, &b);
            }
            let source = g.upload(d.width as u32, d.height as u32, &d.pixels).expect("upload");
            let request = Request { scale: SIDE / d.width.max(d.height) as f64, ..Request::default() };
            drop(d);
            let reference = &references[k];
            // The user's look on the roll measured again, the roll measured again at rest, the frame unprepared.
            let mut arms = vec![("look", balance::written(reference, &f.settings)), ("prepared", balance::written(reference, &start)), ("rest", rest.clone())];
            if *colours == Colours::Matrix {
                arms.push(("swiftsidecar", f.settings.clone()));
            }
            for (arm, s) in arms {
                for face in ["pro", "frontier"] {
                    let image = pipeline.image(&mut g, &source, &on_face(&s, face), &request, &Inputs::default()).expect("recipe");
                    let e = image.extent();
                    let px = g.render(&image, e, None).expect("render");
                    write(&out.join(format!("{}.{face}.{label}-{arm}.f32", f.name)), e.w as usize, e.h as usize, &px);
                }
            }
            drop(source);
            g.purge();
        }
        println!("{} rendered", f.name);
    }
    report.push(format!("\"ranges\": {{{}}}", ranges.join(", ")));
    std::fs::write(out.join("roll.json"), format!("{{{}}}\n", report.join(",\n"))).expect("roll.json");
}
