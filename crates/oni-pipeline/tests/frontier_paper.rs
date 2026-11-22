//! The Frontier SP-3000 as a paper: its print is the measured table, its rest neutral with no offset of the machine's,
//! Lighting on the negative as on every paper, the GPU its reference. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, error, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::look_lab::measure::{self, centile, constrained, median};
use oni_pipeline::paper::GREY_PRINT;
use oni_pipeline::{conversion, film, scanner, stages};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{LegacyMachine, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};

const FRONTIER: PaperStock = PaperStock::FujifilmFrontierSp3000;

fn on_frontier(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { print_paper: Some(FRONTIER), ..s }
}

/// The frames the print is read on: a negative, a digital photo, black and white.
fn frames() -> [(&'static str, PipelineSettings); 3] {
    [("negative", on_frontier(prepared_negative())), ("digital", on_frontier(digital())), ("black and white", on_frontier(mono()))]
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper.expect("a negative on the Frontier")
}

/// The print at unit time and no filtration: the table's own coordinates, amounts read as `x`.
fn bare(s: &PipelineSettings) -> Paper {
    Paper { scale: 1.0, filters: [1.0; 3], ..paper_of(s) }
}

/// Median and p90 ΔE (Oklab × 100, display-clipped) of `p` against the measured cube on its constrained nodes.
fn against_table(p: &Paper) -> (f64, f64) {
    let to709 = |v: [f32; 3]| -> [f64; 3] {
        let m = [[1.660_49, -0.587_641, -0.072_850], [-0.124_550, 1.132_9, -0.008_349], [-0.018_150, -0.100_579, 1.118_73]];
        std::array::from_fn(|r| (0..3).map(|c| m[r][c] * f64::from(v[c])).sum::<f64>().clamp(0.0, 1.0))
    };
    let e: Vec<f64> = constrained()
        .iter()
        .map(|(x, code)| {
            let (got, want) = (measure::oklab(to709(p.printed(x.map(|v| v as f32)))), measure::lab_of(*code));
            100.0 * (0..3).map(|c| (got[c] - want[c]).powi(2)).sum::<f64>().sqrt()
        })
        .collect();
    (median(&e), centile(&e, 0.9))
}

#[test]
fn the_print_reproduces_the_measured_table() {
    let p = bare(&frames()[0].1);
    let (med, p90) = against_table(&p);
    println!("the Frontier's print against its table on the constrained nodes: ΔE median {med:.3}, p90 {p90:.3}");
    assert!(med < 1.2 && p90 < 2.6, "{med} {p90}");
    // Twin: each layer's curve linearised between its ends misses the table by far.
    let straight: &'static [[f32; 3]; PAPER_CURVE_NODES] = Box::leak(Box::new(std::array::from_fn(|i| {
        let t = i as f32 / (PAPER_CURVE_NODES - 1) as f32;
        std::array::from_fn(|c| p.curves[0][c] + t * (p.curves[PAPER_CURVE_NODES - 1][c] - p.curves[0][c]))
    })));
    let (med, p90) = against_table(&Paper { curves: straight, ..p });
    println!("twin, the curves linearised: ΔE median {med:.3}, p90 {p90:.3}");
    assert!(med > 1.2 && p90 > 2.6, "{med} {p90}");
}

#[test]
fn the_rest_prints_the_reference_grey_neutral_with_no_offset_of_the_machines() {
    for (label, s) in frames() {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let grey = cpu::stage47(grey_scan(&s), &k);
        let filtration = conversion::printing(&s).unwrap().filtration;
        println!("{label}: grey {grey:?} (a plain RAW's {:.5}), filtration {filtration:?} CC, pick {:?}, white {:?}", *GREY_PRINT, p.pick, p.white);
        assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 2e-6), "{label}: {grey:?}");
        // Twin: the same enlarger through no filtration prints the grey off neutral.
        if label != "black and white" {
            let unfiltered = cpu::stage47(grey_scan(&s), &Stage47 { paper: Some(Paper { filters: [1.0; 3], ..p }), ..k });
            assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2 || (unfiltered[1] - unfiltered[0]).abs() > 1e-2, "{label}: {unfiltered:?}");
        }
    }
    // On a Frontier legacy print the paper takes the keys alone: the cube's resting offset is not laid.
    let mut s = on_frontier(prepared_negative());
    s.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, ..LegacyPrint::default() });
    let laid = scanner::on_legacy_print(&s).unwrap();
    assert_eq!(laid.gains.stops, s.gains.stops);
    // Twin: on the cube's path the rest rides in the gains.
    let cube = scanner::on_legacy_print(&PipelineSettings { print_paper: None, ..s.clone() }).unwrap();
    assert!((0..3).any(|c| (cube.gains.stops[c] - s.gains.stops[c]).abs() > 1e-3), "{:?}", cube.gains.stops);
}

#[test]
fn lighting_moves_the_negative_it_prints_and_the_enlarger_stays_at_rest() {
    let luma = |v: V4| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    for (label, s) in frames() {
        let rest = paper_of(&s);
        let was = cpu::stage47(grey_scan(&s), &conversion::stage47(&s));
        let moves: [(&str, fn(&mut Print)); 3] =
            [("Exposure +1", |p| p.exposure += 1.0), ("Contrast 0.3", |p| p.contrast = 0.3), ("warmer", |p| p.set_filters(Print::TEMPERATURE_AXIS))];
        for (name, f) in moves {
            let mut t = s.clone();
            f(&mut t.print);
            let k = conversion::stage47(&t);
            let p = k.paper.unwrap();
            // Black and white reads no filter.
            let filtered = name == "warmer" && label == "black and white";
            assert!(k.negative.is_some() != filtered, "{label}, {name}: Lighting lays its move on the negative");
            assert_eq!((p.scale, p.filters), (rest.scale, rest.filters), "{label}, {name}: the enlarger at its rest");
            let is = cpu::stage47(grey_scan(&s), &k);
            println!("{label}, {name}: the grey's luma {:.4} from {:.4}, red over blue {:.3} from {:.3}", luma(is), luma(was), is[0] / is[2], was[0] / was[2]);
            match name {
                "Exposure +1" => assert!(luma(is) > luma(was) * 1.1, "{label}"),
                "Contrast 0.3" => assert!((luma(is) / luma(was) - 1.0).abs() < 0.02, "{label}: the grey held"),
                _ if label != "black and white" => assert!((luma(is) / luma(was) - 1.0).abs() < 0.05 && is[0] / is[2] > was[0] / was[2] * 1.05, "{label}"),
                _ => assert!(is == was, "{label}: black and white reads no filter"),
            }
        }
        // Twin: at rest Lighting lays nothing on the negative.
        assert!(conversion::stage47(&s).negative.is_none());
    }
}

#[test]
fn the_gpu_prints_the_frontier_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_prints_the_frontier_as_its_reference_does") else { return };
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 17, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 5] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0], [1e-6, 0.4, 0.002, 0.5]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    let light = |mut s: PipelineSettings| {
        (s.print.exposure, s.print.contrast, s.print.shadows, s.print.highlights) = (s.print.exposure + 0.7, 0.2, 0.1, -0.1);
        s.print.set_filters([0.2, -0.1, 0.0]);
        (s.print.black_point, s.print.white_point, s.print.brightness, s.print.paper_blacks) = (0.03, 0.05, 0.5, false);
        s
    };
    for (label, s) in frames() {
        let lit = light(s);
        let k = conversion::stage47(&lit);
        let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        // Twin: half a stop more on the enlarger.
        let twin = conversion::stage47(&PipelineSettings { print: Print { exposure: lit.print.exposure + 0.5, ..lit.print }, ..lit.clone() });
        let twin = mapped(&pixels, |p| cpu::stage47(p, &twin));
        judge(&format!("Frontier, lit {label}, on pure black"), &out, &reference, &twin);
        let (e, t) = (error(&out, &reference), error(&out, &twin));
        assert!(e < 1.5e-5 && t > 4e-2, "{label}: {e:.2e}, twin {t:.2e}");
        // Paper blacks off lays the pick on pure black.
        let dark = conversion::stage47(&PipelineSettings { print: Print { paper_blacks: true, ..lit.print }, ..lit.clone() });
        let lifted = error(&reference, &mapped(&pixels, |p| cpu::stage47(p, &dark)));
        println!("{label}: pure black moves the print {lifted:.1e}, its pick printed at {:?}", dark.paper.unwrap().anchor);
        assert!(lifted > 0.0, "{label}");
    }
}
