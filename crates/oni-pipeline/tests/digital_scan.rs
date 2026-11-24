//! A digital photo's negative scanned as a real one is: the GPU's film and scan against their scalar reference, the bare
//! film reading its orange base, a grey ramp lit at the film's white printing its ends neutral on ONI Standard, and the chart's chroma
//! against the film's reference where it is at hand (`ONI_CHART_DIR`). Each verdict has a twin.

mod common;
#[path = "../examples/chart/mod.rs"]
mod chart;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_pipeline::{Pipeline, film};
use oni_render::cpu;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

const PORTRA: VirtualStock = VirtualStock::of("kodak-portra-800");

/// Portra 800's read-out alone, each layer's amount on its own channel, on the reference that read-out derives.
fn read_out_alone() -> film::Stock {
    let entry = PORTRA.entry();
    let mut stock = film::bake_through(&entry, film::couplers(&entry)).unwrap();
    stock.scan = None;
    stock.reference = film::derived(&stock);
    stock
}

#[test]
fn the_gpu_scans_a_digital_negative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_scans_a_digital_negative_as_its_reference_does") else { return };
    let (w, h) = (29, 11);
    let pixels: Vec<V4> = noise(w, h, 3, 0.0, 1.5).iter().map(|q| q.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(PORTRA);
    let exposure = s.film().unwrap().exposure;
    let filmed = Pipeline::new().filmed(&mut g, oni_graph::Image::new(&source), &s).unwrap();
    let out = whole(&mut g, &filmed);
    let stock = film::stock(PORTRA);
    let reference = mapped(&pixels, |p| stock.shot(p, exposure));
    // Twin: the read-out a scan replaces, each layer's amount on its own channel.
    judge("a digital negative scanned", &out, &reference, &mapped(&pixels, |p| cpu::virtual_negative(p, &stock.kernel_at(exposure), &stock.table)));
}

#[test]
fn a_scanned_negative_reads_its_orange_base() {
    let stock = film::stock(PORTRA);
    let bare = stock.shot([0.0, 0.0, 0.0, 1.0], 0.0);
    println!("bare film scanned {bare:?}, its reference's stops {:?}", stock.reference.stops);
    // The base passes red the most and blue the least, as a masked negative's does, and the gains lay it on grey.
    assert!(bare[0] > 1.5 * bare[1] && bare[1] > 1.5 * bare[2], "{bare:?}");
    let grey: [f32; 3] = std::array::from_fn(|c| bare[c] * stock.reference.stops[c].exp2());
    assert!((0..3).all(|c| (grey[c] / grey[1] - 1.0).abs() < 1e-4), "{grey:?}");
    // Twin: the read-out alone holds no base, its bare film all but clear on every channel.
    let alone = cpu::virtual_negative([0.0, 0.0, 0.0, 1.0], &stock.kernel, &stock.table);
    assert!(alone[2] > 0.8 * alone[0], "{alone:?}");
}

/// A ramp of greys lit at the film's own white (Portra 800: D55), as a daylight scene reaches it, on ONI Standard: its
/// ends neutral, each channel's window from the bare film to a scene white, its mids the film's own crossover, nothing
/// solved on greys (owner's call; the mids' C* is printed for the preset's owner to judge).
const NEUTRAL_ENDS: f64 = 1.5;

#[test]
fn a_grey_ramp_lit_at_the_films_white_prints_its_ends_neutral_on_oni_standard() {
    let white = oni_pipeline::conversion::scene_white(&PipelineSettings::rest(ConversionMode::Digital, true).on_stock(PORTRA)).unwrap();
    let rows = [white.r_vector, white.g_vector, white.b_vector].map(|r| [f64::from(r[0]), f64::from(r[1]), f64::from(r[2])]);
    let back = oni_settings::white::inverse(&rows);
    let lit = |v: f64| oni_settings::white::apply(&back, [chart::dec(v); 3]);
    let ramp = |r: &chart::Render, film: bool| chart::RAMP.map(|v| chart::cielab(chart::oklab(if film { r.scene(lit(v)) } else { r.code([v; 3]) })));
    let chroma = |l: &chart::V3| l[1].hypot(l[2]);
    let ends = |ramp: &[chart::V3]| chroma(&ramp[0]).max(chroma(&ramp[ramp.len() - 1]));
    let render = chart::Render::new(PORTRA.key(), film::couplers(&PORTRA.entry()));
    let (shot, raw) = (ramp(&render, true), ramp(&render, false));
    println!("lit at D55: L* a* b* {:?}", shot.map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    println!("a RAW's neutral, lit at 5000 K: {:?}", raw.map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    println!("the mids' widest C* {:.1}", shot.iter().map(chroma).fold(0.0, f64::max));
    assert!(ends(&shot) < NEUTRAL_ENDS, "the ends' C* {}", ends(&shot));
    // Twins: a RAW's own neutral is a 5000 K grey, warmer than the film's light, and prints its white warm; the read-out
    // the scan replaced, read on its own reference, tints its mids further.
    assert!(ends(&raw) > NEUTRAL_ENDS, "twin: {}", ends(&raw));
    let alone = ramp(&chart::Render::of(PORTRA.key(), read_out_alone()), true);
    println!("twin, the read-out alone: {:?}", alone.map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    assert!(alone.iter().map(chroma).fold(0.0, f64::max) > shot.iter().map(chroma).fold(0.0, f64::max));
}

/// The chart's bounds on Portra 800 against its reference, grey matched: the ColorChecker's chroma within a tenth of the
/// reference's, and the mean ΔE00 the fitted couplers reach with a margin, the reference's tone apart from ONI's print.
const CHROMA_WITHIN: f64 = 0.1;
const MEAN_DE00: f64 = 8.8;

#[test]
fn portra_800s_chart_holds_its_looks_chroma() {
    let Some(cube) = std::env::var_os("ONI_CHART_DIR").map(|d| std::path::Path::new(&d).join("reference-kodak-portra-800")).filter(|p| p.exists()) else {
        return eprintln!("no ONI_CHART_DIR: Portra 800's chart is not compared with its reference");
    };
    let cube = chart::Cube::read(&cube.to_string_lossy());
    let colours = chart::colours();
    let targets: Vec<chart::V3> = colours.iter().map(|&c| chart::oklab(cube.at(c))).collect();
    let judged = |r: &chart::Render| chart::judged(&chart::printed(r, &colours, &cube), &targets);
    let couplers = film::couplers(&PORTRA.entry());
    let [de, _, chroma, ..] = judged(&chart::Render::new(PORTRA.key(), couplers));
    println!("scanned: mean dE00 {de:.2}, ColorChecker chroma {chroma:.3} of the reference's");
    assert!((chroma - 1.0).abs() < CHROMA_WITHIN && de < MEAN_DE00, "chroma {chroma}, dE00 {de}");
    // Twins: the read-out alone, and the scan with no cross term, fall outside.
    let [alone_de, _, alone, ..] = judged(&chart::Render::of(PORTRA.key(), read_out_alone()));
    let [_, _, uncoupled, ..] = judged(&chart::Render::new(PORTRA.key(), diagonal(couplers)));
    println!("read-out alone: dE00 {alone_de:.2}, chroma {alone:.3}; no cross term: chroma {uncoupled:.3}");
    assert!((alone - 1.0).abs() > CHROMA_WITHIN && (uncoupled - 1.0).abs() > CHROMA_WITHIN);
}

/// `couplers`' diagonal alone.
fn diagonal(couplers: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| if i == j { couplers[i][j] } else { 0.0 }))
}
