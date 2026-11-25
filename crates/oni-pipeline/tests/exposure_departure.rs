//! Format 64 keeps every Exposure: a sidecar of format 61 on every pipe, its Exposure at the former rest or off it, reads
//! Exposure as its departure from the pipe's rest and prints to the bit as pinned, stage 4-7 on the CPU and the whole
//! chain on the GPU. A print on a paper lays Prepare's black and white on its own ends at any Exposure. Each verdict
//! has a twin.

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
/// exactly.
fn stored() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (label, s) in pipes() {
        for by in [0.0f32, 0.5, -0.75] {
            let mut value = serde_json::to_value(oni_settings::document::Document::of(s.clone())).unwrap();
            value["version"] = 61.into();
            value["settings"]["print"]["exposure"] = f64::from(former_rest(s.mode) + by).into();
            out.push((format!("{label}, {by:+}"), value));
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
    let mut pixels: Vec<V4> = common::noise(17, 13, 7, 0.0, 1.5).iter().map(|p| stock.scanned(cpu::virtual_negative(*p, &stock.kernel, &stock.table))).collect();
    pixels.extend(common::noise(17, 13, 8, 0.0, 1.0));
    pixels
}

fn fold(hash: &mut u64, px: &[V4]) {
    for b in px.iter().flatten().flat_map(|v| v.to_bits().to_le_bytes()) {
        *hash = (*hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
}

/// The bits of every stored pipe's stage 4-7 print of `pixels`, each read through `read`, folded by FNV-1a.
fn cpu_bits(read: impl Fn(&Value) -> PipelineSettings) -> u64 {
    let pixels = pixels();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (_, value) in stored() {
        let k = conversion::stage47(&read(&value));
        fold(&mut hash, &pixels.iter().map(|p| cpu::stage47(*p, &k)).collect::<Vec<_>>());
    }
    hash
}

/// Every pipe's print as format 61 stored it, folded on the CPU and on the GPU (Metal on Apple silicon): a legacy print on
/// its own curve, a paper's on its useful range, Exposure held at the window's ends; a slide on its projector; a frame
/// naming no paper on its pure inversion.
const BEFORE_CPU: u64 = 0xb83c_3386_8abf_021d;
const BEFORE_GPU: u64 = 0xf583_f9ae_8850_fb57;

#[test]
fn every_stored_exposure_reads_as_its_departure() {
    for (label, value) in stored() {
        let mode = read(&value).mode;
        let kept = value["settings"]["print"]["exposure"].as_f64().unwrap() as f32;
        assert_eq!(read(&value).print.exposure, kept - former_rest(mode), "{label}");
    }
    // The old negative's rest reads 0, a slide's 0 stays.
    let (negative, slide) = (&stored()[0].1, &stored()[15].1);
    assert_eq!((negative["settings"]["print"]["exposure"].as_f64(), read(negative).print.exposure), (Some(1.5), 0.0));
    assert_eq!((slide["settings"]["mode"].as_str(), read(slide).print.exposure), (Some("positive"), 0.0));
    // Twin: a document of this format keeps the value it holds.
    let mut current = negative.clone();
    current["version"] = oni_settings::document::VERSION.into();
    assert_eq!(read(&current).print.exposure, 1.5);
}

#[test]
fn every_pipe_prints_as_format_61_did_on_the_cpu() {
    let folded = cpu_bits(read);
    println!("every stored pipe folds to {folded:#018x} on the CPU");
    assert_eq!(folded, BEFORE_CPU);
    // Twin: the same documents read as this format's, their Exposure taken whole, move.
    let unmigrated = |v: &Value| {
        let mut v = v.clone();
        v["version"] = oni_settings::document::VERSION.into();
        read(&v)
    };
    assert_ne!(cpu_bits(unmigrated), BEFORE_CPU);
}

/// The whole chain on the GPU, bit for bit, where the fold was drawn: Metal on Apple silicon.
#[test]
fn every_pipe_renders_whole_as_format_61_did_on_the_gpu() {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return println!("SKIPPED every_pipe_renders_whole_as_format_61_did_on_the_gpu: its bits are Metal's on Apple silicon");
    }
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED every_pipe_renders_whole_as_format_61_did_on_the_gpu: no GPU") };
    let pixels = pixels();
    let source = g.upload(17, 26, &pixels).unwrap();
    let mut render = |read: &dyn Fn(&Value) -> PipelineSettings| {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (_, value) in stored() {
            let image = Pipeline::new().image(&mut g, &source, &read(&value), &Request::default(), &Inputs::default()).unwrap();
            fold(&mut hash, &g.render(&image, image.extent(), None).unwrap());
        }
        hash
    };
    let folded = render(&read);
    println!("every stored pipe folds to {folded:#018x} on the GPU");
    assert_eq!(folded, BEFORE_GPU);
    // Twin: a stop's tenth more on the first negative moves it.
    let nudged = |v: &Value| {
        let mut s = read(v);
        if v == &stored()[0].1 {
            s.print.exposure += 0.1;
        }
        s
    };
    assert_ne!(render(&nudged), BEFORE_GPU);
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
