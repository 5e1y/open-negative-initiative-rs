//! A digital photo's negative scanned as a real one is: the GPU's film and scan against their scalar reference, the bare
//! film reading its orange base, the grey ramp held neutral on the paperless pipe, and the chart's chroma against the
//! film's reference where it is at hand (`ONI_CHART_DIR`). Each verdict has a twin.

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
    stock.reference = film::derived(entry.silver, entry.dyes, &stock);
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

/// The neutral axis's C* bound on the paperless pipe, as a digital photo renders it: the scan holds 6.5 at its most, a
/// shadow's warmth the sheet's three curves lay; the read-out it replaces reached 9.1.
const NEUTRAL_CHROMA: f64 = 7.0;

#[test]
fn a_grey_ramp_prints_neutral_on_the_paperless_pipe() {
    let ramp = |r: &chart::Render| chart::RAMP.map(|v| chart::cielab(chart::oklab(r.code([v; 3]))));
    let worst = |r: &chart::Render| ramp(r).iter().map(|l| l[1].hypot(l[2])).fold(0.0, f64::max);
    let (render, alone) = (chart::Render::new(PORTRA.key(), film::couplers(&PORTRA.entry())), chart::Render::of(PORTRA.key(), read_out_alone()));
    for (name, r) in [("scanned", &render), ("the read-out alone", &alone)] {
        println!("{name}: ramp L* a* b* {:?}", ramp(r).map(|l| l.map(|v| (v * 10.0).round() / 10.0)));
    }
    assert!(worst(&render) < NEUTRAL_CHROMA, "the ramp's C* reaches {}", worst(&render));
    // Twin: the read-out alone tints the ramp past the bound.
    assert!(worst(&alone) > NEUTRAL_CHROMA, "twin: {}", worst(&alone));
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
