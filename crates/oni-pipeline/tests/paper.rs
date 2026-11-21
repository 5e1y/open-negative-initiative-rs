//! The paper: spektrafilm's print of Portra 800's dyes on each paper; each frame's black, white and grey printed neutral
//! through ONI's window, Exposure and Contrast on ONI's curve inside it; slides never printed; the GPU on every branch.
//! Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::{conversion, scanner, stages};
use oni_render::{Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Face, PaperStock, PipelineSettings, VirtualStock};
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
    *PaperStock::ALL.iter().find(|p| p.profile() == name).expect("a shipped paper")
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

/// The paper's print without its window, as it printed before the window laid its anchors.
fn unwindowed(k: &Stage47) -> Stage47 {
    Stage47 { paper: k.paper.map(|p| Paper { window_black: [0.0; 3], window_white: [1.0; 3], window_luma: false, ..p }), ..*k }
}

#[test]
fn the_anchors_print_black_white_and_the_grey_neutral() {
    let dose_zero = PipelineSettings { separation: 0.0, ..prepared_negative() };
    for (label, s) in [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono()), ("no unmix", dose_zero)] {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        println!("{label}: window black {:?}, white {:?}", p.window_black, p.window_white);
        let [black, white, grey] = anchor_scans(&s).map(|q| cpu::stage47(q, &k));
        println!("{label}: black {black:?}, white {white:?}, grey {grey:?} (ONI's print {:.5})", *GREY_PRINT);
        assert!((0..3).all(|c| black[c].abs() < 1e-4), "{label}: the black prints black: {black:?}");
        assert!((0..3).all(|c| (white[c] - 1.0).abs() < 1e-4), "{label}: the white prints white: {white:?}");
        assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 2e-4), "{label}: the grey prints a plain RAW's: {grey:?}");
        // Twin: the paper without its window prints its own white and its Dmax.
        let [black, white, _] = anchor_scans(&s).map(|q| cpu::stage47(q, &unwindowed(&k)));
        println!("{label}: unwindowed, black {black:?}, white {white:?}");
        assert!(white[1] < 0.9 && black[1] > 2e-3, "{label}: {white:?} {black:?}");
        // Twin: the same paper with no filtration prints the grey off neutral.
        let bare = Stage47 { paper: k.paper.map(|p| Paper { filters: [1.0; 3], ..p }), ..k };
        let unfiltered = cpu::stage47(grey_scan(&s), &bare);
        if label != "black and white" {
            assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2, "{label}: {unfiltered:?}");
        }
    }
    // A digital photo's reference grey is its scene grey: the paper prints the scene's grey as ONI's print does.
    let s = digital();
    let stock = film::stock(VirtualStock::KodakPortra800);
    let scan = cpu::virtual_negative([GREY, GREY, GREY, 1.0], &stock.kernel, &stock.table);
    let printed = cpu::stage47(scan, &conversion::stage47(&s));
    assert!((0..3).all(|c| (printed[c] - *GREY_PRINT).abs() < 2e-3), "{printed:?}");
}

#[test]
fn the_enlarger_rests_where_the_window_lays_the_grey_on_a_plain_raws() {
    let dose_zero = PipelineSettings { separation: 0.0, ..prepared_negative() };
    for (label, s) in [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono()), ("no unmix", dose_zero)] {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let anchors = paper::anchors(&s, k.dyes.as_ref());
        let grey = p.windowed(p.printed(anchors[2]));
        let off = grey.iter().map(|g| (g - *GREY_PRINT).abs()).fold(0.0f32, f32::max);
        println!("{label}: the windowed grey {grey:?}, within {off:.1e} of {:.5}; filtration {:?}", *GREY_PRINT, conversion::printing(&s).unwrap().filtration);
        // Measured within 4e-7: the solve's tolerance in log light.
        assert!(off < 2e-6, "{label}: {grey:?}");
        // Twin: the enlarger set where the bare print's grey is a plain RAW's leaves the window's grey near 0.49.
        let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
        let (mut low, mut high) = (p.scale.ln() - 10.0, p.scale.ln() + 10.0);
        for _ in 0..80 {
            let mid = 0.5 * (low + high);
            if luma(Paper { scale: mid.exp(), ..p }.printed(anchors[2])) > *GREY_PRINT { low = mid } else { high = mid }
        }
        let bare = paper::anchored(Paper { scale: (0.5 * (low + high)).exp(), ..p }, &anchors, s.mode == ConversionMode::Monochrome);
        let lifted = bare.windowed(bare.printed(anchors[2]));
        println!("{label}: set on the bare print, the windowed grey {lifted:?}, {:+.3} stop of exposure apart", (bare.scale / p.scale).log2());
        assert!(lifted.iter().all(|g| *g > *GREY_PRINT + 0.05), "{label}: {lifted:?}");
    }
}

#[test]
fn exposure_brightens_the_grey_and_holds_the_black_and_the_white() {
    let s = prepared_negative();
    let [_, _, rest] = lit(&s, |_| {});
    for stops in [-1.0, 1.0] {
        let [black, white, grey] = lit(&s, |t| t.print.exposure += stops);
        println!("{stops:+} stop: black {black:?}, white {white:?}, grey {grey:?}");
        assert!((0..3).all(|c| black[c].abs() < 1e-4 && (white[c] - 1.0).abs() < 1e-4), "{stops}: {black:?} {white:?}");
        // As on ONI's print: a stop doubles the grey's odds against white, 0.398 to 0.570 up, 0.249 down.
        assert!((0..3).all(|c| if stops > 0.0 { grey[c] > rest[c] * 1.4 } else { grey[c] < rest[c] / 1.5 }), "{stops}: {grey:?} {rest:?}");
    }
    // Twin: a stop up as the enlarger's, a stop less light on the paper, lifts the black off black.
    let k = conversion::stage47(&s);
    let enlarger = Stage47 { paper: k.paper.map(|p| Paper { scale: p.scale * 0.5, ..p }), ..k };
    let black = cpu::stage47(anchor_scans(&s)[0], &enlarger);
    println!("the enlarger's stop lifts the black to {black:?}");
    assert!(black[1] > 1e-3, "{black:?}");
}

#[test]
fn contrast_bends_about_the_grey_and_holds_the_ends() {
    let s = prepared_negative();
    let k = conversion::stage47(&s);
    let darker = { let g = grey_scan(&s); [g[0] * 3.0, g[1] * 3.0, g[2] * 3.0, 1.0] };
    let flat = cpu::stage47(darker, &k);
    for travel in [-0.3, 0.3] {
        let [black, white, grey] = lit(&s, |t| t.print.contrast = travel);
        let mut t = s.clone();
        t.print.contrast = travel;
        let dark = cpu::stage47(darker, &conversion::stage47(&t));
        println!("contrast {travel:+}: black {black:?}, white {white:?}, grey {grey:?}, a darker grey {dark:?} from {flat:?}");
        assert!((0..3).all(|c| black[c].abs() < 1e-4 && (white[c] - 1.0).abs() < 1e-4), "{travel}: {black:?} {white:?}");
        assert!(if travel > 0.0 { dark[1] < flat[1] * 0.95 } else { dark[1] > flat[1] * 1.05 }, "{travel}: {dark:?} {flat:?}");
    }
    // Twin: a contrast laid as a straight slope about the grey, its window narrowed, pushes the white past white.
    let pivot = GREY_PRINT.powf(1.0 / Print::DISPLAY_GAMMA);
    let narrowed = [pivot - pivot / 1.3, pivot + (1.0 - pivot) / 1.3, 1.0];
    let straight = Stage47 { red: narrowed, green: narrowed, blue: narrowed, ..k };
    let white = cpu::stage47(anchor_scans(&s)[1], &straight);
    assert!(white[1] > 1.1, "{white:?}");
}

#[test]
fn at_rest_the_print_is_the_windowed_paper() {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let dose_zero = PipelineSettings { separation: 0.0, ..prepared_negative() };
    for (label, s) in [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono()), ("no unmix", dose_zero)] {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let paper_alone = |q: V4| {
            let read = k.dyes.as_ref().map_or(q, |d| cpu::dye_separation(q, &k, d));
            cpu::paper(read, &p)
        };
        let worst = |k: &Stage47| pixels.iter().map(|q| {
            let (a, b) = (cpu::stage47(*q, k), paper_alone(*q));
            (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0f32, f32::max)
        }).fold(0.0f32, f32::max);
        let at_rest = worst(&k);
        println!("{label}: at rest within {at_rest:.1e} of the windowed paper");
        assert!(at_rest < 1e-5, "{label}: {at_rest}");
        // Twin: ONI's print curve at its own rest laid on the paper, a tone curve stacked.
        let stacked = worst(&Stage47 { print_stops: Print::RESTING_EXPOSURE, ..k });
        assert!(stacked > 1e-2, "{label}: {stacked}");
    }
}

#[test]
fn the_scanner_face_prints_on_the_paper_at_its_rest_and_its_keys_move_the_enlarger() {
    let mut s = prepared_negative();
    s.face = Face::Scanner;
    let machine = scanner::on_oni_machine(&s).unwrap();
    assert_eq!(machine.print.exposure, Print::resting_exposure(s.mode));
    let grey = grey_scan(&s);
    let rest = cpu::stage47(grey, &conversion::stage47(&machine));
    assert!((0..3).all(|c| (rest[c] - *GREY_PRINT).abs() < 2e-4), "{rest:?}");
    // A cyan key prints cyaner, as on ONI's print: its red falls, its blue holds.
    s.scanner.c = 4;
    let keyed = cpu::stage47(grey, &conversion::stage47(&scanner::on_oni_machine(&s).unwrap()));
    assert!(keyed[0] < rest[0] * 0.9 && (keyed[2] - rest[2]).abs() < 0.01, "{keyed:?} {rest:?}");
    // Twin: on ONI's print the machine enters at its look's exposure.
    let onis = PipelineSettings { print_paper: None, ..s };
    assert_eq!(scanner::on_oni_machine(&onis).unwrap().print.exposure, scanner::look_entry_exposure(ConversionMode::Negative));
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
    lit.density = 0.3;
    let mono = PipelineSettings { print_paper: Some(PaperStock::KodakSupraEndura), ..mono() };
    let unmixed_none = PipelineSettings { separation: 0.0, print_paper: Some(PaperStock::FujifilmCrystalArchiveTypeII), ..prepared_negative() };
    for (label, s) in [("digital", digital()), ("lit negative", lit), ("black and white", mono), ("no unmix", unmixed_none)] {
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
    let wider = p.window_white.map(|w| w * 1.05);
    judge("paper alone", &alone, &mapped(&pixels, |q| cpu::paper(q, &p)), &mapped(&pixels, |q| cpu::paper(q, &Paper { window_white: wider, ..p })));
}
