//! Format 64 keeps every Exposure: a sidecar of format 61 on every pipe, its Exposure at the former rest or off it, reads
//! Exposure as its departure from the pipe's rest and prints to the bit as format 64 holding that departure, stage 4-7
//! on the CPU and the whole chain on the GPU. A print on a paper lays Prepare's black and white on its own ends at any
//! Exposure. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::V4;
use frames::*;
use oni_graph::Graph;
use oni_pipeline::{Inputs, Pipeline, Request, conversion, film};
use oni_render::cpu;
use oni_settings::{ConversionMode, LegacyMachine, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};
use serde_json::Value;

/// Format 61's Exposure at rest: a negative's 1.5, a slide's 0.
fn former_rest(mode: ConversionMode) -> f32 {
    if mode == ConversionMode::Positive { 0.0 } else { 1.5 }
}

fn slide() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    s.gains.stops = [0.3, 0.0, -0.4];
    s
}

/// Every pipe Exposure reaches, as this build lays it before any Exposure.
fn pipes() -> Vec<(&'static str, PipelineSettings)> {
    let mut legacy = unpapered(prepared_negative());
    legacy.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 1, c: 2, m: -1, y: 0, w: 3, laid: true });
    let mut contrasted = onis(prepared_negative());
    contrasted.print.contrast = 0.2;
    vec![
        ("ONI Standard, negative", onis(prepared_negative())),
        ("ONI Standard, negative, Contrast", contrasted),
        ("legacy print, Frontier", legacy),
        ("ONI Standard, digital", onis(digital())),
        ("ONI Standard, black and white", onis(mono())),
        ("projector, slide", slide()),
        ("paper, negative", prepared_negative()),
        ("paper, digital", digital()),
        ("paper, black and white", mono()),
        ("paper, Frontier", PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..digital() }),
        ("no paper, negative", unpapered(prepared_negative())),
        ("no paper, digital", unpapered(digital())),
        ("no paper, black and white", unpapered(mono())),
    ]
}

/// Each pipe as a sidecar of format 61 held it, its Exposure `by` stops off that format's rest, each a sum f32 holds
/// exactly, beside the same document as format 64 holds it, its Exposure the departure `by`.
fn stored() -> Vec<(String, Value, Value)> {
    let mut out = Vec::new();
    for (label, s) in pipes() {
        for by in [0.0f32, 0.5, -0.75] {
            let mut value = serde_json::to_value(oni_settings::document::Document::of(s.clone())).unwrap();
            value["version"] = 61.into();
            value["settings"]["print"]["exposure"] = f64::from(former_rest(s.mode) + by).into();
            let mut departed = value.clone();
            departed["version"] = 64.into();
            departed["settings"]["print"]["exposure"] = f64::from(by).into();
            out.push((format!("{label}, {by:+}"), value, departed));
        }
    }
    out
}

/// The document read through this format's migration, then on the paper it names again: the migration takes a format's
/// default paper away.
fn read(value: &Value) -> PipelineSettings {
    let read = oni_settings::migrate::document(&serde_json::to_vec(value).unwrap()).unwrap().document.settings;
    let named = value["settings"].get("printPaper").map(|key| serde_json::from_value(key.clone()).unwrap());
    PipelineSettings { print_paper: named, ..read }
}

/// A scan noise and a virtual negative's scan.
fn pixels() -> Vec<V4> {
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let mut pixels: Vec<V4> = common::noise(17, 13, 7, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(common::noise(17, 13, 8, 0.0, 1.0));
    pixels
}

/// Each stored pipe's stage 4-7 print of `pixels` on the CPU, read through `read`, as its bits.
fn cpu_bits(read: impl Fn(&Value) -> PipelineSettings) -> Vec<Vec<u32>> {
    let pixels = pixels();
    stored().iter().map(|(_, value, _)| printed(&pixels, &read(value))).collect()
}

fn printed(pixels: &[V4], s: &PipelineSettings) -> Vec<u32> {
    let k = conversion::stage47(s);
    bits(&pixels.iter().map(|p| cpu::stage47(*p, &k)).collect::<Vec<_>>())
}

fn bits(px: &[V4]) -> Vec<u32> {
    px.iter().flatten().map(|v| v.to_bits()).collect()
}

/// Read as this format's, its Exposure taken whole: what the migration is there to avoid.
fn unmigrated(v: &Value) -> PipelineSettings {
    let mut v = v.clone();
    v["version"] = oni_settings::document::VERSION.into();
    read(&v)
}

#[test]
fn every_stored_exposure_reads_as_its_departure() {
    for (label, value, _) in stored() {
        let mode = read(&value).mode;
        let kept = value["settings"]["print"]["exposure"].as_f64().unwrap() as f32;
        assert_eq!(read(&value).print.exposure, kept - former_rest(mode), "{label}");
    }
    // The old negative's rest reads 0, a slide's 0 stays.
    let (negative, slide) = (&stored()[0].1, &stored()[15].1);
    assert_eq!((negative["settings"]["print"]["exposure"].as_f64(), read(negative).print.exposure), (Some(1.5), 0.0));
    assert_eq!((slide["settings"]["mode"].as_str(), read(slide).print.exposure), (Some("positive"), 0.0));
    // Twin: a document of this format keeps the value it holds.
    assert_eq!(unmigrated(negative).print.exposure, 1.5);
}

#[test]
fn every_pipe_prints_as_format_61_did_on_the_cpu() {
    // Each stored pipe prints to the bit as format 64 holding its departure does, every later format read alike.
    let pixels = pixels();
    let migrated = cpu_bits(read);
    for ((label, _, departed), got) in stored().iter().zip(&migrated) {
        assert!(*got == printed(&pixels, &read(departed)), "{label}: format 61 prints otherwise");
    }
    // Twin: the same documents read unmigrated move.
    assert_ne!(cpu_bits(unmigrated), migrated);
}

/// The whole chain on the GPU, bit for bit: each stored pipe as format 64 holding its departure renders.
#[test]
fn every_pipe_renders_whole_as_format_61_did_on_the_gpu() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED every_pipe_renders_whole_as_format_61_did_on_the_gpu: no GPU") };
    let pixels = pixels();
    let source = g.upload(17, 26, &pixels).unwrap();
    let mut render = |s: &PipelineSettings| {
        let image = Pipeline::new().image(&mut g, &source, s, &Request::default(), &Inputs::default()).unwrap();
        bits(&g.render(&image, image.extent(), None).unwrap())
    };
    let mut moved = 0;
    for (label, value, departed) in stored() {
        let migrated = render(&read(&value));
        assert!(migrated == render(&read(&departed)), "{label}: format 61 renders otherwise");
        // Twin: read unmigrated, a pipe off its former rest moves.
        moved += usize::from(render(&unmigrated(&value)) != migrated);
    }
    assert!(moved >= stored().len() / 2, "{moved} moved unmigrated");
}

/// The reference grey's print on `s`, its luma.
fn grey(s: &PipelineSettings) -> f32 {
    let p = cpu::stage47(grey_scan(s), &conversion::stage47(s));
    oni_render::LUMA.iter().zip(p).map(|(w, v)| w * v).sum()
}

#[test]
fn at_any_exposure_a_paper_lays_prepares_black_and_white_on_its_own_ends() {
    let mut seen = 0;
    // Contrast, as every other Lighting slider, moves the whites freely.
    for (label, s) in pipes().into_iter().filter(|(_, s)| s.paper().is_some() && s.legacy().is_none() && s.print.contrast == 0.0) {
        assert_eq!(s.print.exposure, 0.0, "{label}: Exposure rests at 0");
        for exposure in [0.0f32, -1.5, 1.0] {
            let t = PipelineSettings { print: oni_settings::tone::Print { exposure, ..s.print }, ..s.clone() };
            let k = conversion::stage47(&t);
            let p = k.paper().expect("a paper");
            let [black, white, _] = anchor_scans(&t).map(|q| cpu::stage47(q, &k));
            let off = |v: V4, a: [f32; 3]| (0..3).map(|c| (v[c] - a[c]).abs()).fold(0.0f32, f32::max);
            println!("{label}, Exposure {exposure:+}: black {black:?}, white {white:?}");
            assert!(off(black, p.anchor) < 1e-5 && off(white, p.white_anchor) < 1e-4, "{label}, {exposure}: {black:?} {white:?}");
        }
        // Twin: Exposure moves the tones between, the grey a stop and a half under printing darker by half a stop at least.
        let under = PipelineSettings { print: oni_settings::tone::Print { exposure: -1.5, ..s.print }, ..s.clone() };
        assert!((grey(&under) / grey(&s)).log2() < -0.5, "{label}: {} against {}", grey(&under), grey(&s));
        seen += 1;
    }
    assert!(seen >= 6, "{seen}");
}

