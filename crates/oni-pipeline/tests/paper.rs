//! The paper: spektrafilm's print of Portra 800's dye amounts reproduced on each paper, its lamp and filters; each
//! frame's reference grey printed neutral where ONI's print puts it; Lighting moving the enlarger; slides never
//! printed; the GPU running its reference on every branch. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::film::{self, GREY};
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::{conversion, scanner, stages};
use oni_render::{Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, DyeSet, Face, PaperStock, PipelineSettings, VirtualStock};
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
    let base = Paper { filters: printed.filters.map(|cc| 10f32.powf(-cc / 100.0)), scale: 1.0, pivot: [0.0; 3], contrast: 1.0, ..k.paper.expect("a negative on its paper") };
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

fn digital() -> PipelineSettings {
    PipelineSettings::rest(ConversionMode::Digital, true)
}

fn prepared_negative() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-0.8, 0.0, 1.2];
    let windows = [(0.55, 1.6, 0.45), (0.54, 1.75, 0.5), (0.52, 1.9, 0.58)];
    for (w, (black, white, mid)) in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue].into_iter().zip(windows) {
        (w.black, w.white, w.mid) = (black, white, mid);
    }
    s.dye_set = DyeSet::KodakPortra400;
    s.print_paper = Some(PaperStock::default());
    s
}

/// The scan pixel holding the frame's reference grey, through its gains and pedestal.
fn grey_scan(s: &PipelineSettings) -> V4 {
    let k = conversion::stage47(s);
    let d = paper::reference_grey(s);
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / k.gain[c] - k.pedestal);
    [light[0], light[1], light[2], 1.0]
}

#[test]
fn the_reference_grey_prints_neutral_where_onis_print_puts_it() {
    let mono = PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Monochrome, true) };
    let dose_zero = PipelineSettings { separation: 0.0, ..prepared_negative() };
    for (label, s) in [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono), ("no unmix", dose_zero)] {
        let k = conversion::stage47(&s);
        assert!(k.paper.is_some(), "{label}");
        let printed = cpu::stage47(grey_scan(&s), &k);
        println!("{label}: the grey prints {:.5} {:.5} {:.5} (ONI's print {:.5})", printed[0], printed[1], printed[2], *GREY_PRINT);
        assert!((0..3).all(|c| (printed[c] - *GREY_PRINT).abs() < 2e-4), "{label}: {printed:?}");
        // Twin: the same paper with no filtration prints the grey off neutral.
        let bare = Stage47 { paper: k.paper.map(|p| Paper { filters: [1.0; 3], ..p }), ..k };
        let unfiltered = cpu::stage47(grey_scan(&s), &bare);
        assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2, "{label}: {unfiltered:?}");
    }
    // A digital photo's reference grey is its scene grey: the paper prints the scene's grey as ONI's print does.
    let s = digital();
    let stock = film::stock(VirtualStock::KodakPortra800);
    let scan = cpu::virtual_negative([GREY, GREY, GREY, 1.0], &stock.kernel, &stock.table);
    let printed = cpu::stage47(scan, &conversion::stage47(&s));
    assert!((0..3).all(|c| (printed[c] - *GREY_PRINT).abs() < 2e-3), "{printed:?}");
}

#[test]
fn lighting_moves_the_enlarger_and_the_contrast_turns_about_the_grey() {
    let s = prepared_negative();
    let grey = grey_scan(&s);
    let at = |f: &dyn Fn(&mut PipelineSettings)| {
        let mut t = s.clone();
        f(&mut t);
        cpu::stage47(grey, &conversion::stage47(&t))
    };
    let rest = at(&|_| {});
    let up = at(&|t| t.print.exposure += 1.0);
    assert!(up[1] > rest[1] * 1.5, "a stop up brightens the print: {up:?} {rest:?}");
    // Warmer: Temperature's axis brightens red, darkens blue, through the cyan and yellow filters, the luma held.
    let warm = at(&|t| t.print.set_filters([0.25, 0.0, -0.25]));
    assert!(warm[0] > rest[0] * 1.05 && warm[2] < rest[2] * 0.95, "{warm:?}");
    let luma = |v: V4| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    assert!((luma(warm) - luma(rest)).abs() < 1e-3, "{} {}", luma(warm), luma(rest));
    let filtration = |t: &PipelineSettings| conversion::printing(t).unwrap().filtration;
    let mut warmed = s.clone();
    warmed.print.set_filters([0.25, 0.0, -0.25]);
    let (was, now) = (filtration(&s), filtration(&warmed));
    println!("filtration at rest {was:?}, warmer {now:?}");
    // More cyan against magenta, less yellow: the least filter stays at none.
    assert!(was[0] == 0.0 && now[1] - now[0] < was[1] - was[0] - 5.0 && now[2] - now[1] < was[2] - was[1] - 5.0, "{was:?} {now:?}");
    // Contrast steepens the paper about the grey: the grey stays, a darker grey darkens further.
    let steep = at(&|t| t.print.contrast = 0.3);
    assert!((0..3).all(|c| (steep[c] - rest[c]).abs() < 2e-4), "{steep:?} {rest:?}");
    let dark = [grey[0] * 3.0, grey[1] * 3.0, grey[2] * 3.0, 1.0];
    let (flat, steeper) = (cpu::stage47(dark, &conversion::stage47(&s)), {
        let mut t = s.clone();
        t.print.contrast = 0.3;
        cpu::stage47(dark, &conversion::stage47(&t))
    });
    assert!(steeper[1] < flat[1] * 0.95, "{steeper:?} {flat:?}");
    // Twin: the plain contrast leaves that darker grey where it was.
    let again = at(&|t| t.print.contrast = 0.0);
    assert_eq!(again, rest);
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
    let mono = PipelineSettings { print_paper: Some(PaperStock::KodakSupraEndura), ..PipelineSettings::rest(ConversionMode::Monochrome, true) };
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
    judge("paper alone", &alone, &mapped(&pixels, |q| cpu::paper(q, &p)), &mapped(&pixels, |q| cpu::paper(q, &Paper { contrast: 1.3, ..p })));
}
