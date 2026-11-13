//! What the app answers the film's clicks with, laid on the scene: the pane through its global, the
//! canvas's overlays through the scene's properties.

use super::*;

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
pub(super) enum Showing {
    Dusty,
    Reading,
    Ringed,
    /// The corrections written, this many patches landed, the reminders landed or not.
    Healing(usize, bool),
}

/// The app's answers the film plays: the dust found and where the strip shows it, the reading's veil,
/// the landing order and times of the patches, and every picture shown.
pub(super) struct Answers {
    pub(super) found: Vec<Found>,
    pub(super) view: View,
    pub(super) size: (u32, u32),
    pub(super) sensitivity: f32,
    pub(super) mask: Rgba,
    /// Each patch's correction in the order they landed, with the heal's time to it.
    pub(super) landed: Vec<(usize, f32)>,
    /// The detail after each landing, the first the dusty one.
    pub(super) pictures: Vec<Rgba>,
    pub(super) originals: Vec<Rgba>,
    pub(super) originals_at: f32,
}

/// The found dust written, then healed by the app as REMOVE would have it, each landing's picture kept.
pub(super) fn answers_of(scan: &Scan, reading: &dust::Reading, found: &[Found], view: View, sensitivity: f32, veil: [f32; 4]) -> Result<Answers, String> {
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
pub(super) fn veiled(reading: &dust::Reading, view: View, size: (u32, u32), sensitivity: f32, veil: [f32; 4]) -> Rgba {
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
pub(super) fn show(stage: &Stage, showing: Showing, answers: &Answers) -> Result<(), String> {
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
pub(super) fn found_note(count: usize, sensitivity: f32) -> String {
    match count {
        0 if sensitivity >= 1.0 => tr("No dust found, even at full Sensitivity").to_string(),
        0 => tr("No dust found: raise Sensitivity to catch fainter specks").to_string(),
        n => trn("{n} speck of dust ringed", "{n} specks of dust ringed", n as u64),
    }
}

/// A stroke on the frame as the canvas traces it, `f` the frame's place in points.
pub(super) fn traced(points: &[(f64, f64)], f: (f64, f64, f64, f64)) -> String {
    points.iter().enumerate().map(|(i, p)| format!("{} {:.2} {:.2}", if i == 0 { "M" } else { " L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3)).collect()
}

pub(super) fn mark(b: [f64; 4], number: usize) -> Value {
    Value::Struct(Struct::from_iter([
        ("x".to_string(), Value::Number(b[0])),
        ("y".to_string(), Value::Number(b[1])),
        ("width".to_string(), Value::Number(b[2])),
        ("height".to_string(), Value::Number(b[3])),
        ("number".to_string(), Value::Number(number as f64)),
    ]))
}

pub(super) fn model(values: Vec<Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(values)))
}

pub(super) fn image(p: &Rgba) -> slint::Image {
    slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&p.pixels, p.width, p.height))
}

/// What the stage holds now: the pane's global and the scene's overlays, as the frame shows them.
pub(super) fn seen_now(stage: &Stage) -> Seen {
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
