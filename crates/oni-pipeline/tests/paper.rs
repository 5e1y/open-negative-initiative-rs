//! The paper: spektrafilm's print of Portra 800's dyes on each paper; each frame's enlarger resting where its grey prints
//! neutral at a plain RAW's grey, its ends the paper's own; the print the paper's alone; slides never printed; the GPU on
//! every branch. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::{conversion, scanner, stages};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    amounts: Vec<[f32; 3]>,
    lamp: Vec<f64>,
    dichroics: Vec<[f64; 3]>,
    papers: std::collections::BTreeMap<String, Printed>,
}

#[derive(Deserialize)]
struct Printed {
    filters: [f32; 3],
    cmy_mid: [f32; 3],
    log_e_print: Vec<[f32; 3]>,
    cmy_print: Vec<[f32; 3]>,
    rgb_uncompressed: Vec<[f32; 3]>,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("paper/spektrafilm.json")).expect("the fixture reads")
}

fn stock(name: &str) -> PaperStock {
    *PaperStock::ALL.iter().find(|p| p.profile() == Some(name)).expect("a shipped paper")
}

/// spektrafilm's enlarger on Portra 800: its filters, its print exposure normalised on its mid grey's light.
fn spektrafilm_enlarger(paper: PaperStock, printed: &Printed) -> Paper {
    let k = conversion::stage47(&PipelineSettings { print_paper: Some(paper), ..film::on_reference(VirtualStock::KodakPortra800, &VirtualStock::KodakPortra800.reference()) });
    let base = Paper { filters: printed.filters.map(|cc| 10f32.powf(-cc / 100.0)), scale: 1.0, ..k.paper.expect("a negative on its paper") };
    let mid = base.exposed(printed.cmy_mid);
    Paper { scale: 10f32.powf(-(mid[0] + mid[1] + mid[2]) / 3.0), ..base }
}

#[test]
fn the_lamp_and_the_dichroic_filters_are_spektrafilms() {
    let f = fixture();
    let lamp = paper::lamp();
    let worst = (0..lamp.len()).map(|l| (lamp[l] - f.lamp[l]).abs() / f.lamp[l]).fold(0.0, f64::max);
    println!("the lamp within {worst:.1e} of spektrafilm's TH-KG3");
    assert!(worst < 1e-6, "{worst}");
    let filters = (0..lamp.len()).flat_map(|l| (0..3).map(move |c| (l, c))).map(|(l, c)| (paper::dichroic(l)[c] - f.dichroics[l][c]).abs()).fold(0.0, f64::max);
    assert!(filters < 1e-6, "{filters}");
    // Twin: the bare blackbody, no heat filter, is another lamp.
    let bare: Vec<f64> = (0..lamp.len()).map(|l| { let w = (380.0 + 5.0 * l as f64) * 1e-9; 1.0 / (w.powi(5) * ((0.014_388 / (w * 3400.0)).exp() - 1.0)) }).collect();
    let mean = bare.iter().sum::<f64>() / bare.len() as f64;
    assert!((0..lamp.len()).any(|l| (bare[l] / mean - f.lamp[l]).abs() / f.lamp[l] > 0.05));
}

#[test]
fn every_paper_prints_portra_800s_dye_amounts_as_spektrafilm_does() {
    let f = fixture();
    for (name, printed) in &f.papers {
        let k = spektrafilm_enlarger(stock(name), printed);
        let (mut log, mut density, mut light) = (0.0f32, 0.0f32, 0.0f32);
        for (i, a) in f.amounts.iter().enumerate() {
            let e = k.exposed(*a);
            let d = k.developed(e);
            let v = k.viewed(d);
            log = (0..3).map(|c| (e[c] - printed.log_e_print[i][c]).abs()).fold(log, f32::max);
            density = (0..3).map(|c| (d[c] - printed.cmy_print[i][c]).abs()).fold(density, f32::max);
            light = (0..3).map(|c| (v[c] - printed.rgb_uncompressed[i][c]).abs() / (0.01 + printed.rgb_uncompressed[i][c].abs())).fold(light, f32::max);
        }
        println!("{name}: log exposure within {log:.1e}, density {density:.1e}, light {light:.1e}");
        assert!(log < 2e-6 && density < 2e-4 && light < 2e-3, "{name}");
        // Twin: five CC more magenta print another colour.
        let swapped = Paper { filters: [k.filters[0], k.filters[1] * 10f32.powf(-0.05), k.filters[2]], ..k };
        let off = f.amounts.iter().enumerate().map(|(i, a)| (swapped.exposed(*a)[1] - printed.log_e_print[i][1]).abs()).fold(0.0, f32::max);
        assert!(off > 1e-2, "{name}: {off}");
    }
}

/// The frames the rest is read on: a digital photo, a negative, black and white, and a negative with no unmix.
fn rests() -> [(&'static str, PipelineSettings); 4] {
    [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono()), ("no unmix", PipelineSettings { separation: 0.0, ..prepared_negative() })]
}

#[test]
fn the_enlarger_rests_where_the_grey_prints_a_plain_raws_neutral_and_the_ends_are_the_papers() {
    for (label, s) in rests() {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let [black, white, grey] = anchor_scans(&s).map(|q| cpu::stage47(q, &k));
        let shares = paper::layer_shares(&p, [0.0; 3]);
        let dmax = p.viewed(p.curves[PAPER_CURVE_NODES - 1]);
        let dmax = if p.luma { [oni_render::LUMA.iter().zip(dmax).map(|(w, v)| w * v).sum(); 3] } else { dmax };
        println!("{label}: grey {grey:?} (a plain RAW's {:.5}), black {black:?} (Dmax {dmax:?}), white {white:?} (paper {:?}); the base at {shares:?} of each layer's range", *GREY_PRINT, p.white);
        // Measured within 3e-7: the solve's tolerance in log light.
        assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 2e-6), "{label}: {grey:?}");
        // The ends are the paper's: the base above its Dmax, the densest read at most its white, neither laid on 0 or 1.
        assert!((0..3).all(|c| black[c] >= dmax[c] - 1e-6 && black[c] > 1e-3 && white[c] <= p.white[c] + 1e-6 && white[c] < 0.9), "{label}");
        // Twin: the same enlarger through no filtration prints the grey off neutral.
        let bare = Stage47 { paper: Some(Paper { filters: [1.0; 3], ..p }), ..k };
        let unfiltered = cpu::stage47(grey_scan(&s), &bare);
        if label != "black and white" {
            assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2, "{label}: {unfiltered:?}");
        }
        // Twin: ONI's window laid on the print would put the base on black and the densest read on white.
        let windowed = |v: V4| -> [f32; 3] { std::array::from_fn(|c| (v[c] - black[c]) / (white[c] - black[c])) };
        assert!(windowed(black) == [0.0; 3] && windowed(grey).iter().all(|g| (g - *GREY_PRINT).abs() > 0.01), "{label}");
    }
    // A digital photo's reference grey is its scene grey: the paper prints the scene's grey as ONI's print does.
    let s = digital();
    let stock = film::stock(VirtualStock::KodakPortra800);
    let scan = cpu::virtual_negative([GREY, GREY, GREY, 1.0], &stock.kernel, &stock.table);
    let printed = cpu::stage47(scan, &conversion::stage47(&s));
    assert!((0..3).all(|c| (printed[c] - *GREY_PRINT).abs() < 2e-3), "{printed:?}");
}

#[test]
fn the_print_is_the_papers_own_its_colour_density_included() {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let mut lit = prepared_negative();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.black_point) = (2.0, 0.2, 0.1, 0.03);
    for (label, s) in rests().into_iter().chain([("lit negative", lit)]) {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        assert!(k.print_on == 0.0 && k.hunt == 0.0, "{label}: no print curve, no Hunt on the paper");
        let paper_alone = |q: V4| {
            let read = k.dyes.as_ref().map_or(q, |d| cpu::dye_separation(q, &k, d));
            cpu::paper(read, &p)
        };
        let worst = |k: &Stage47| pixels.iter().map(|q| {
            let (a, b) = (cpu::stage47(*q, k), paper_alone(*q));
            (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0f32, f32::max)
        }).fold(0.0f32, f32::max);
        let alone = worst(&k);
        println!("{label}: stage 4-7 within {alone:.1e} of the paper's print");
        assert!(alone == 0.0, "{label}: {alone}");
        // Twin: ONI's print curve at its own rest laid on the paper, a tone curve stacked.
        let stacked = worst(&Stage47 { print_on: 1.0, print_stops: Print::RESTING_EXPOSURE, ..k });
        assert!(stacked > 1e-2, "{label}: {stacked}");
    }
    // The colour density is the paper's own, laid on its print before its ends, as on ONI's print after its curve.
    let mut dense = prepared_negative();
    dense.density = 0.4;
    let k = conversion::stage47(&dense);
    let p = k.paper.unwrap();
    let (q, plain) = (pixels[7], Stage47 { paper: Some(Paper { density: 1.0, ..p }), ..k });
    let (v, w) = (cpu::stage47(q, &k), cpu::stage47(q, &plain));
    assert!(k.density == 1.0 && p.density > 1.0 && (v[0] / v[2]).ln().abs() > (w[0] / w[2]).ln().abs(), "{v:?} {w:?}");
}

#[test]
fn a_legacy_print_on_a_paper_rests_at_its_rest_and_its_keys_move_the_enlarger() {
    let mut s = prepared_negative();
    s.legacy_print = Some(LegacyPrint::default());
    let laid = scanner::on_legacy_print(&s).unwrap();
    let grey = grey_scan(&s);
    let rest = cpu::stage47(grey, &conversion::stage47(&laid));
    assert!((0..3).all(|c| (rest[c] - *GREY_PRINT).abs() < 2e-4), "{rest:?}");
    // A cyan key prints cyaner, as on ONI's print: its red falls, its blue holds.
    s.legacy_print = Some(LegacyPrint { c: 4, ..LegacyPrint::default() });
    let keyed = cpu::stage47(grey, &conversion::stage47(&scanner::on_legacy_print(&s).unwrap()));
    assert!(keyed[0] < rest[0] * 0.9 && (keyed[2] - rest[2]).abs() < 0.01, "{keyed:?} {rest:?}");
    // Twin: the keys at rest, no key moves the enlarger.
    s.legacy_print = Some(LegacyPrint::default());
    assert_eq!(cpu::stage47(grey, &conversion::stage47(&scanner::on_legacy_print(&s).unwrap())), rest);
}

#[test]
fn a_slide_is_never_printed_and_a_measure_reads_no_paper() {
    let slide = PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Positive, true) };
    assert!(slide.paper().is_none() && conversion::stage47(&slide).paper.is_none());
    let inverted = PipelineSettings { inverted_source: true, ..prepared_negative() };
    assert!(conversion::stage47(&inverted).paper.is_none(), "a positive read as its own negative");
    let mut measured = prepared_negative();
    measured.print.bypassed = true;
    assert!(conversion::stage47(&measured).paper.is_none(), "a measure upstream of the print");
    // Twin: the same negative printed.
    assert!(conversion::stage47(&prepared_negative()).paper.is_some());
    // Off, the negative renders as it did before the paper, to the bit.
    let off = PipelineSettings { print_paper: None, ..prepared_negative() };
    let k = conversion::stage47(&off);
    assert!(k.paper.is_none() && k.dyes.is_some_and(|d| d.tone_low != [-1.0; 3]), "ONI's print, its dyes toned");
}

#[test]
fn the_gpu_prints_on_the_paper_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_prints_on_the_paper_as_its_reference_does") else { return };
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 9, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 5] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0], [1e-6, 0.4, 0.002, 0.5]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    let mut lit = prepared_negative();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.highlights) = (2.0, 0.2, 0.1, -0.1);
    lit.print.set_filters([0.2, -0.1, 0.0]);
    (lit.print.black_point, lit.print.white_point, lit.print.channel_contrast, lit.print.channel_mid) = (0.03, 0.05, [0.1, 0.0, -0.1], [0.0, 0.1, 0.0]);
    (lit.print.channel_white, lit.print.channel_black, lit.print.brightness, lit.print.true_blacks) = ([0.02, 0.0, 0.0], [0.0, 0.0, 0.03], 0.5, true);
    lit.density = 0.3;
    let mut mono = PipelineSettings { print_paper: Some(PaperStock::KodakSupraEndura), ..mono() };
    (mono.print.contrast, mono.print.highlights, mono.print.white_point, mono.print.brightness, mono.print.true_blacks) = (0.3, -0.2, 0.04, -0.5, true);
    let unmixed_none = PipelineSettings { separation: 0.0, print_paper: Some(PaperStock::FujifilmCrystalArchiveTypeII), ..prepared_negative() };
    let mut anchored = PipelineSettings { print: Print { true_blacks: false, brightness: -0.7, ..lit.print }, ..lit.clone() };
    anchored.print.exposure -= 3.0;
    let darker = PipelineSettings { print: Print { brightness: -4.0, ..anchored.print }, ..anchored.clone() };
    let brighter = PipelineSettings { print: Print { brightness: 4.0, ..lit.print }, ..lit.clone() };
    let frames = [
        ("digital", digital()),
        ("lit negative", lit),
        ("lit negative, its pick on its rest", anchored),
        ("lit negative, Brightness -4", darker),
        ("lit negative, Brightness +4", brighter),
        ("black and white", mono),
        ("no unmix", unmixed_none),
    ];
    for (label, s) in frames {
        let k = conversion::stage47(&s);
        let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        let twin = Stage47 { paper: k.paper.map(|p| Paper { scale: p.scale * 1.1, ..p }), ..k };
        judge(&format!("paper, {label}"), &out, &reference, &mapped(&pixels, |p| cpu::stage47(p, &twin)));
    }
    // The paper's pass alone, on its own reading.
    let k = conversion::stage47(&digital());
    let p = k.paper.unwrap();
    let alone = whole(&mut g, &Image::new(&source).then(stages::paper(&p)));
    let brighter = Paper { brightness: 0.1, ..p };
    judge("paper alone", &alone, &mapped(&pixels, |q| cpu::paper(q, &p)), &mapped(&pixels, |q| cpu::paper(q, &brighter)));
}
