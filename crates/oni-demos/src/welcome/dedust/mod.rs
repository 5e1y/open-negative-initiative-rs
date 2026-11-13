//! The welcome's De-dust film: a dusty photograph's detail under the canvas's overlays beside the app's pane;
//! DETECT runs the app's reading, REMOVE writes a correction a speck and the app heals each, at their pace.

mod answers;
mod framing;
mod scan;

pub use answers::Seen;
pub use framing::{Structure, View, framing, within};
pub use scan::{Found, Scan, found, reduced};

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

use answers::*;
use framing::*;

/// The welcome's stage in points, `WelcomeStyle.stage-width` by `stage-height`.
pub const STAGE: (f32, f32) = (560.0, 502.0);
/// The variable naming the dusty photograph: its path is the owner's, never written in the repository.
pub const SOURCE: &str = "ONI_WELCOME_DUST";

/// The film's bounds on the app's two waits, as the plan's gates set them: shorter is shown at the
/// least, longer fails the film rather than show a pace the app does not keep.
const READING: (f32, f32) = (0.3, 3.0);
const HEALING: f32 = 4.0;

/// The specks a detail shows: one row of the pane's tiles in every language.
pub const SPECKS: usize = 4;
/// The zooms a detail is looked for at, closest first: the strip's pixels a frame pixel, as the canvas's
/// pinch leaves it between 200 % and 50 % on a Retina screen.
const ZOOMS: [f32; 6] = [2.0, 1.0, 0.875, 0.75, 0.625, 0.5];

pub fn source() -> Result<PathBuf, String> {
    let path = std::env::var_os(SOURCE).map(PathBuf::from).ok_or(format!("set {SOURCE} to the dusty photograph"))?;
    path.is_file().then_some(path).ok_or(format!("{SOURCE} names no file"))
}

/// What the film is asked: the Sensitivity the pane rests at and the zoom, else the least that frames
/// `SPECKS` specks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ask {
    pub sensitivity: Option<f32>,
    pub zoom: Option<f32>,
    /// A rehearsal on another roll: fewer specks than `SPECKS` are filmed, not refused.
    pub rehearsal: bool,
    /// The hand goes its way and never presses, as a check's twin asks.
    pub still_hand: bool,
    /// Every speck of the frame ringed and removed, beyond the strip: a check's twin.
    pub whole: bool,
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
    let (reading, took, forms) = scan.read()?;
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
            if let Some((view, n)) = framing(&all, scan.size, strip, z, least_ring, SPECKS, &forms)
                && best.as_ref().is_none_or(|b| n > b.2)
            {
                best = Some((s, view, n, all.clone()));
                if n == SPECKS {
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
    let (sensitivity, view, shown, whole) = best.or_else(middle).ok_or("the frame is smaller than the strip")?;
    if shown < SPECKS && !ask.rehearsal {
        return Err(format!("no detail shows {SPECKS} specks apart at any Sensitivity, {shown} at most"));
    }
    // The reading is the frame's; what the strip rings, counts and removes is what it found in view.
    let all = if ask.whole { whole.clone() } else { within(&whole, view, scan.size, least_ring) };
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
        "{} specks in the frame at Sensitivity {}, {} in the detail at {}×, {} px from the left and {} from the top; read in {:.2} s, healed in {:.2} s",
        whole.len(),
        Readout::Percent.format(sensitivity),
        all.len(),
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
    let (reading, took, forms) = scan.read()?;
    let mut lines = vec![format!("{:?}, read in {:.2} s, {} px past the window", scan.settings.mode, took.as_secs_f32(), reading.past)];
    for s in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
        let all = found(&reading, s);
        let at = |z: f32| framing(&all, scan.size, strip, z, least_ring, usize::MAX, &forms);
        let best: Vec<String> = ZOOMS.iter().map(|&z| at(z).map_or(format!("{z}×: none"), |(v, n)| format!("{z}×: {n} at ({}, {})", v.x, v.y))).collect();
        lines.push(format!("Sensitivity {s:.2}: {} specks; {}", all.len(), best.join(", ")));
    }
    // At rest and at 50 %, the detail at 100 % holding the most rings, as the canvas shows them.
    if let Some(out) = stills {
        let veil = veil_of(&stage)?;
        let stem = path.file_stem().map_or("frame".into(), |s| s.to_string_lossy().to_string());
        for (s, name) in [(0.0, "rest"), (0.5, "50")] {
            let all = found(&reading, s);
            let Some((view, n)) = framing(&all, scan.size, strip, 1.0, least_ring, usize::MAX, &forms).or_else(|| busiest(&all, scan.size, strip, 1.0)) else { continue };
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

/// `DustStyle.veil` as the kernel takes it, straight, 0 to 1.
fn veil_of(stage: &Stage) -> Result<[f32; 4], String> {
    match stage.instance().get_global_property("DustStyle", "veil").map_err(|e| format!("DustStyle.veil: {e}"))? {
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
    // Every changed pixel of the photograph written, so the faintest heal is drawn.
    encode::gif(&clip, Tuning { tolerance: 0, colours: 128 })
}
