//! What Auto White Balance's kept outcome is keyed by: every field its reading reads moves the
//! fingerprint, and every look the reading leaves at rest moves neither the fingerprint nor the
//! outcome; on roll #8, the editor's half decode kept on disk reads the balance LibRaw's own reads.

use oni_core::balance::{Measure, Outcome};
use oni_graph::Graph;
use oni_measure::key::{self, Decoded, Measurer};
use oni_pipeline::flat_field::Mask;
use oni_pipeline::resident::Files;
use oni_settings::blocks::SeparationFamily;
use oni_settings::cg::{Point, Rect, Size};
use oni_settings::frame::{ContactSheetGrid, Correction, FlatFieldMask, RgbPasses, RollReference};
use oni_settings::modes::{Face, SourceEncoding};
use oni_settings::ScannerMachine;
use oni_settings::tone::Tone;
use oni_settings::{ConversionMode, Id, PipelineSettings};
use std::path::{Path, PathBuf};
use std::sync::Arc;

type Change = (&'static str, fn(&mut PipelineSettings));

/// Every field the reading reads, each moved.
const INPUTS: [Change; 26] = [
    ("mode", |s| s.mode = ConversionMode::Positive),
    ("sheet", |s| s.sheet = Some(ContactSheetGrid { columns: 2, rows: 1, frames: 2, tile: Size { width: 36.0, height: 24.0 }, margin: 0.0, windows: None })),
    ("decoded_from_mosaic", |s| s.decoded_from_mosaic = !s.decoded_from_mosaic),
    ("capture_stops", |s| s.capture_stops = 0.6),
    ("lens_corrected", |s| s.lens_corrected = !s.lens_corrected),
    ("primaries_converted", |s| s.primaries_converted = !s.primaries_converted),
    ("source_encoding", |s| s.source_encoding = Some(SourceEncoding::Linear)),
    ("rgb_passes", |s| s.rgb_passes = Some(RgbPasses { red: "R.tif".into(), green: "G.tif".into(), blue: "B.tif".into(), registration: None })),
    ("pass_of", |s| s.pass_of = Some("lead.tif".into())),
    ("crop", |s| s.geometry.crop = Rect::new(0.0, 0.0, 0.5, 1.0)),
    ("measure_window", |s| s.geometry.measure_window = Some(Rect::new(0.55, 0.1, 0.4, 0.8))),
    ("angle", |s| s.geometry.angle = 8.0),
    ("quarter_turns", |s| s.geometry.quarter_turns = 1),
    ("gains", |s| s.gains.stops[0] += 0.4),
    ("density_ceiling", |s| s.density_ceiling += 0.3),
    ("separation", |s| s.separation = 0.9),
    ("separation_family", |s| s.separation_family = Some(SeparationFamily::FujiC41)),
    ("separation_by_hand", |s| s.separation_by_hand = !s.separation_by_hand),
    ("flat_field", |s| {
        s.flat_field.mask = FlatFieldMask::ValoiEasy35V1;
        s.flat_field.amount = 1.0;
    }),
    ("corrections", |s| s.corrections.push(Correction { id: Id::new(), stroke: vec![Point::new(0.2, 0.5), Point::new(0.45, 0.5)], radius: 6.0 })),
    ("levels", |s| s.levels.red.white -= 0.2),
    ("chroma_decompression", |s| s.chroma_decompression = 0.3),
    ("chroma_decompression_by_hand", |s| s.chroma_decompression_by_hand = !s.chroma_decompression_by_hand),
    ("roll_reference", |s| s.roll_reference = Some(RollReference { id: Id::new(), stops: [0.2, 0.0, -0.2], black: [0.0; 3], white: [1.0; 3], base: None, median: None })),
    ("density", |s| s.density = 0.4),
    ("geometry", |s| s.geometry.mirrored = !s.geometry.mirrored),
];

/// The inputs this check's scene shows moving the outcome; the others are read where it does not
/// reach: the decode, a sheet's cells, a roll of one, the rotation of a scene the means do not see.
const MOVING: [&str; 13] =
    ["mode", "capture_stops", "crop", "measure_window", "angle", "gains", "density_ceiling", "separation", "flat_field", "corrections", "levels", "roll_reference", "density"];

/// Every look the reading leaves at rest, each moved.
const LOOKS: [Change; 31] = [
    ("print.exposure", |s| s.print.exposure = 0.8),
    ("print.contrast", |s| s.print.contrast = 0.5),
    ("print.shadows", |s| s.print.shadows = -0.4),
    ("print.highlights", |s| s.print.highlights = 0.4),
    ("print.black_point", |s| s.print.black_point += 0.05),
    ("print.white_point", |s| s.print.white_point = 0.05),
    ("print.red", |s| s.print.red = Tone::exposure(0.3)),
    ("print.blue", |s| s.print.blue = Tone::exposure(-0.3)),
    ("print.hunt_base", |s| s.print.hunt_base = Some(Tone::exposure(0.2))),
    ("manual_curves", |s| s.manual_curves.contrast = 0.4),
    ("spectrogram", |s| s.spectrogram.hue[2] = 10.0),
    ("saturation", |s| s.saturation.points = vec![Point::new(0.0, 0.2), Point::new(1.0, 0.9)]),
    ("vignette", |s| s.vignette.stops = -0.8),
    ("secret_sauce", |s| s.secret_sauce = 0.5),
    ("texture", |s| s.texture.amount = 0.8),
    ("chroma", |s| {
        s.chroma.force = 0.7;
        s.chroma.radius = 4.0;
    }),
    ("sharpen", |s| {
        s.sharpen.amount = 1.0;
        s.sharpen.radius = 2.0;
        s.sharpen.halo = 0.4;
    }),
    ("face", |s| s.face = Face::Scanner),
    ("scanner", |s| s.scanner.d = 3),
    ("scanner_machine", |s| s.scanner_machine = ScannerMachine::Frontier),
    ("scanner_finish", |s| s.scanner_finish.contrast = 0.4),
    ("balance_filters", |s| s.balance_filters = Some([0.2, 0.0, -0.2])),
    ("balance_strength", |s| s.balance_strength = 0.3),
    ("balance_method", |s| s.balance_method = Some("auto".into())),
    ("paper_correction", |s| s.paper_correction.stops = [0.1, 0.0, -0.1]),
    ("manual_order", |s| s.manual_order = Some(3.0)),
    ("film", |s| s.film = serde_json::from_value(serde_json::json!({"id": Id::new(), "name": "Portra 400"})).ok()),
    ("roll_reference_declined", |s| s.roll_reference_declined = !s.roll_reference_declined),
    ("print.green", |s| s.print.green = Tone::exposure(0.1)),
    ("scanner.w", |s| s.scanner.w = -2),
    ("sharpen.halo", |s| s.sharpen.halo = 0.9),
];

fn reference() -> RollReference {
    RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None }
}

fn on_roll() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.roll_reference = Some(reference());
    s
}

#[test]
fn the_fingerprint_moves_with_every_input_and_with_no_look() {
    let base = on_roll();
    let print = key::balance_fingerprint(&base, "file 1@2");
    let mut failures = Vec::new();
    for (name, change) in INPUTS {
        let mut s = base.clone();
        change(&mut s);
        assert_ne!(s, base, "{name} moved nothing");
        if key::balance_fingerprint(&s, "file 1@2") == print {
            failures.push(format!("input {name} left the fingerprint"));
        }
    }
    // Twin: no look moves it, though each moves the settings.
    for (name, change) in LOOKS {
        let mut s = base.clone();
        change(&mut s);
        assert_ne!(s, base, "{name} moved nothing");
        if key::balance_fingerprint(&s, "file 1@2") != print {
            failures.push(format!("look {name} moved the fingerprint"));
        }
    }
    // The files the frame reads are its facts: another size or date is another fingerprint.
    assert_ne!(key::balance_fingerprint(&base, "file 1@3"), print);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const W: u32 = 120;
const H: u32 = 80;

/// A negative printing warm on the left and cool on the right, darker towards the bottom, so a
/// crop, a window or a rotation reads another light.
fn scene() -> Decoded {
    let mut pixels = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let shade = 0.5 + 0.5 * (1.0 - y as f32 / H as f32);
            let c = if x < W / 2 { [0.088, 0.11, 0.147] } else { [0.12, 0.10, 0.11] };
            let lit = if (40..60).contains(&x) && (20..40).contains(&y) { 3.0 } else { 1.0 };
            pixels.push([c[0] * shade * lit, c[1] * shade * lit, c[2] * shade * lit, 1.0]);
        }
    }
    Decoded { width: W, height: H, pixels, full_width: f64::from(W) }
}

/// Any correction's patch: a cyan block, so a dust painted over the frame moves its light.
struct Patches;

impl Files for Patches {
    fn mask(&self, _: &str) -> Option<Mask> {
        None
    }

    fn patch(&self, _: Id) -> Option<(u32, u32, Vec<[f32; 4]>)> {
        Some((W, H, vec![[0.02, 0.2, 0.3, 1.0]; (W * H) as usize]))
    }
}

fn measurer(check: &str, decode: Box<key::Decode>) -> Option<Measurer> {
    if !oni_fixtures::heavy(check) {
        return None;
    }
    let graph = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()?;
    Some(Measurer::new(graph, decode, Box::new(|_, _| String::new())).laying(Arc::new(Patches)))
}

/// Two outcomes alike bit for bit; a roll of one is measured under a new id each time.
fn alike(a: &Outcome, b: &Outcome) -> bool {
    let id = a.reference.as_ref().expect("a negative stands on a roll").id;
    let bare = |o: &Outcome| Outcome { reference: o.reference.clone().map(|r| RollReference { id, ..r }), filters: o.filters };
    bare(a) == bare(b)
}

fn gap(a: &Outcome, b: &Outcome) -> f32 {
    (0..3).map(|c| (a.filters[c] - b.filters[c]).abs()).fold(0.0, f32::max)
}

#[test]
fn every_input_the_scene_shows_moves_the_outcome_and_no_look_does() {
    let check = "balance_kept::outcome";
    let Some(m) = measurer(check, Box::new(|_: &Path, _: &PipelineSettings| Ok(scene()))) else { return };
    let mut failures = Vec::new();
    // On its roll, and alone, measured as a roll of one, where the print is read bypassed.
    for base in [on_roll(), PipelineSettings::rest(ConversionMode::Negative, false)] {
        let at_rest = m.balance("scene", &base).expect("the scene balances");
        // A roll of one lays its own windows and the resting separation over the frame's.
        let alone = base.roll_reference.is_none();
        for (name, change) in INPUTS.iter().filter(|(n, _)| MOVING.contains(n) && !(alone && ["levels", "separation"].contains(n))) {
            let mut s = base.clone();
            change(&mut s);
            let moved = m.balance("scene", &s);
            if moved.is_some_and(|o| alike(&o, &at_rest)) {
                failures.push(format!("input {name} left the outcome ({} roll)", if base.roll_reference.is_some() { "on its" } else { "no" }));
            }
        }
        // Twin: each look moves no bit of the outcome.
        for (name, change) in LOOKS {
            let mut s = base.clone();
            change(&mut s);
            match m.balance("scene", &s) {
                Some(o) if alike(&o, &at_rest) => {}
                other => failures.push(format!("look {name} moved the outcome by {:?}", other.map(|o| gap(&o, &at_rest)))),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// LibRaw's decode at the measuring copy's side, as the app reads a RAW for its keys.
fn libraw(path: &Path, _: &PipelineSettings) -> Result<Decoded, String> {
    let (d, factor) = oni_decode::decode_for(path, 1.0, 2000).map_err(|e| format!("{e:?}"))?;
    Ok(Decoded { width: d.width as u32, height: d.height as u32, pixels: d.pixels, full_width: (d.width * factor) as f64 })
}

#[test]
fn roll8_the_half_decode_kept_reads_the_balance_libraw_reads() {
    let check = "balance_kept::roll8";
    let Some(roll) = oni_fixtures::raw_dir(check) else { return };
    let decodes = std::env::temp_dir().join(format!("oni-measure-balance-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&decodes);
    oni_decode::cache::keep_in(decodes.clone(), 1 << 30);
    let Some(fresh) = measurer(check, Box::new(libraw)) else { return };
    let frames: Vec<(PathBuf, PipelineSettings)> = ["DSC_6432.NEF", "DSC_6437.NEF"]
        .iter()
        .map(|n| roll.join(n))
        .map(|p| {
            let s = oni_settings::sidecar::read(&p, true, true).expect("its sidecar").settings;
            (p, s)
        })
        .collect();
    // LibRaw's own decodes first, nothing kept; then the editor's half decodes, kept on disk.
    let read = |m: &Measurer, p: &Path, s: &PipelineSettings| m.balance(&p.to_string_lossy(), s).expect("the frame balances");
    let libraws: Vec<Outcome> = frames.iter().map(|(p, s)| read(&fresh, p, s)).collect();
    let kept = || std::fs::read_dir(&decodes).map_or(0, |d| d.flatten().count());
    assert_eq!(kept(), 0, "a measuring copy kept a decode");
    for (p, _) in &frames {
        oni_decode::decode_for(p, 1.0, oni_decode::HALF).expect("the editor's decode");
    }
    assert_eq!(kept(), frames.len(), "each half decode kept");
    // A measurer of its own, its copies made from the kept decodes.
    let Some(again) = measurer(check, Box::new(libraw)) else { return };
    for ((p, s), libraw) in frames.iter().zip(&libraws) {
        let served = read(&again, p, s);
        println!("{}: {:?} from LibRaw, {:?} from the kept half decode", p.display(), libraw.filters, served.filters);
        assert_eq!(&served, libraw, "{}: the kept decode reads another balance", p.display());
        // Twin: the window shifted reads another light.
        let mut shifted = s.clone();
        let w = shifted.geometry.measure_window.unwrap_or(shifted.geometry.crop);
        shifted.geometry.measure_window = Some(Rect::new(w.x, w.y, w.width * 0.4, w.height * 0.4));
        let elsewhere = read(&again, p, &shifted);
        assert!(gap(&elsewhere, libraw) > 1e-3, "{}: a shifted window reads the same balance", p.display());
    }
    let _ = std::fs::remove_dir_all(&decodes);
}
