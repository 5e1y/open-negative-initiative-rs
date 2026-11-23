//! The paper: each paper printing each layer's density through pure dyes as a double reading of its curves; the enlarger
//! resting on the paper's own grey, neutral on its white, its ends the paper's; the print its own; a measure on none; the
//! GPU on every branch. Each has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::film;
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::{conversion, scanner, stages};
use oni_render::{Negative, PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};
use serde::Deserialize;

fn luma(v: [f32; 3]) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

/// A paper's profile as its file holds it, read apart from the bake.
#[derive(Deserialize)]
struct Sheet {
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
}

fn sheet_of(paper: PaperStock) -> Sheet {
    #[derive(Deserialize)]
    struct File {
        data: Sheet,
    }
    serde_json::from_str::<File>(&paper::source(paper).expect("a profiled paper")).expect("it reads").data
}

/// `np.interp` of the curve of channel `c` at `e`, held at either end.
fn interp(sheet: &Sheet, c: usize, e: f64) -> f64 {
    let le = &sheet.log_exposure;
    let k = le.partition_point(|v| *v <= e).clamp(1, le.len() - 1) - 1;
    let t = ((e - le[k]) / (le[k + 1] - le[k])).clamp(0.0, 1.0);
    sheet.density_curves[k][c] + t * (sheet.density_curves[k + 1][c] - sheet.density_curves[k][c])
}

/// The print of a negative holding `density` over its bare film on each channel, in doubles off the profile: each
/// layer exposed by its own channel through the time and its filter, developed on the sheet's curves, each read
/// through a pure dye on its own channel.
fn read_off(sheet: &Sheet, filters: [f64; 3], scale: f64, density: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let log: [f64; 3] = std::array::from_fn(|c| (scale * filters[c]).log10() - density[c]);
    let developed: [f64; 3] = std::array::from_fn(|c| interp(sheet, c, log[c]));
    (log, developed, developed.map(|d| 10f64.powf(-d)))
}

/// Densities from a bare film to past a negative's densest, and every mix of the three.
fn densities() -> Vec<[f32; 3]> {
    let mut out = vec![[0.0; 3], [1.0; 3], [2.5, 0.2, 0.1], [0.1, 2.4, 0.3], [0.2, 0.3, 2.6]];
    out.extend((0..15).map(|i| [0.13 + 0.17 * i as f32, 0.07 + 0.29 * ((i * 7) % 9) as f32, 0.21 + 0.23 * ((i * 4) % 11) as f32]));
    out
}

#[test]
fn every_paper_prints_each_channels_density_on_its_own_layer_as_its_profile_reads() {
    for paper in PaperStock::all().into_iter().filter(|p| !p.is_machine()) {
        let s = PipelineSettings { print_paper: Some(paper), ..film::on_reference(VirtualStock::of("kodak-portra-800"), &VirtualStock::of("kodak-portra-800").reference()) };
        let k = conversion::stage47(&s).paper.expect("a negative on its paper");
        let sheet = sheet_of(paper);
        let (filters, scale) = (k.filters.map(f64::from), f64::from(k.scale));
        let (mut log, mut density, mut light) = (0.0f64, 0.0f64, 0.0f64);
        for d in densities() {
            let (e, dev, v) = (k.exposed(d), k.developed(k.exposed(d)), k.viewed(k.developed(k.exposed(d))));
            let (want_e, want_d, want_v) = read_off(&sheet, filters, scale, d.map(f64::from));
            log = (0..3).map(|c| (f64::from(e[c]) - want_e[c]).abs()).fold(log, f64::max);
            density = (0..3).map(|c| (f64::from(dev[c]) - want_d[c]).abs()).fold(density, f64::max);
            light = (0..3).map(|c| (f64::from(v[c]) - want_v[c]).abs() / (0.01 + want_v[c].abs())).fold(light, f64::max);
        }
        println!("{}: log exposure within {log:.1e}, density {density:.1e}, light {light:.1e}", paper.name());
        assert!(log < 2e-5 && density < 2e-4 && light < 2e-3, "{paper:?}");
        // Twin: five CC more magenta expose the green-sensitive layer otherwise.
        let magenta = [filters[0], filters[1] * 10f64.powf(-0.05), filters[2]];
        let off = densities().into_iter().map(|d| (f64::from(k.exposed(d)[1]) - read_off(&sheet, magenta, scale, d.map(f64::from)).0[1]).abs()).fold(0.0, f64::max);
        assert!(off > 1e-2, "{paper:?}: {off}");
        // Twin: each dye absorbing a twentieth of its neighbour's density reads another light.
        let crossed = densities().into_iter().map(|d| {
            let (v, want) = (k.viewed(k.developed(k.exposed(d))), read_off(&sheet, filters, scale, d.map(f64::from)).1);
            (0..3).map(|c| (f64::from(v[c]) - 10f64.powf(-(want[c] + 0.05 * want[(c + 1) % 3]))).abs() / (0.01 + f64::from(v[c]))).fold(0.0, f64::max)
        }).fold(0.0, f64::max);
        assert!(crossed > 2e-2, "{paper:?}: {crossed}");
    }
}

/// The frames the rest is read on: a digital photo, a negative, and black and white.
fn rests() -> [(&'static str, PipelineSettings); 3] {
    [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono())]
}

#[test]
fn the_enlarger_rests_where_the_grey_prints_a_plain_raws_neutral_on_the_papers_white_and_the_ends_are_the_papers() {
    for (label, s) in rests() {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let [black, white, grey] = anchor_scans(&s).map(|q| cpu::stage47(q, &k));
        let shares = paper::layer_shares(&p, paper::shares_of(&s, &k, [0.0; 3]));
        let dmax = p.viewed(p.curves[PAPER_CURVE_NODES - 1]);
        let dmax = if p.luma { [oni_render::LUMA.iter().zip(dmax).map(|(w, v)| w * v).sum(); 3] } else { dmax };
        let on_white: [f32; 3] = std::array::from_fn(|c| grey[c] / p.white[c] * luma(p.white));
        println!("{label}: grey {grey:?}, on the paper's white {on_white:?} (a plain RAW's {:.5})", *GREY_PRINT);
        println!("  black {black:?} (Dmax {dmax:?}), white {white:?} (paper {:?}); the base at {shares:?} of each layer's range", p.white);
        // With Print lift a plain RAW's grey's light, neutral on the paper's own white, whatever the frame.
        assert!((0..3).all(|c| (on_white[c] - *GREY_PRINT).abs() < 2e-4), "{label}: {on_white:?}");
        // The ends are the paper's: the base above its Dmax, the densest read at most its white, neither laid on 0 or 1.
        assert!((0..3).all(|c| black[c] >= dmax[c] - 1e-6 && black[c] > 1e-3 && white[c] <= p.white[c] + 1e-6 && white[c] < 0.9), "{label}");
        // Twin: the same time through ten CC more yellow prints the grey off neutral.
        let bare = Stage47 { paper: Some(Paper { filters: [p.filters[0], p.filters[1], p.filters[2] * 10f32.powf(-0.1)], ..p }), ..k };
        let unfiltered = cpu::stage47(grey_scan(&s), &bare);
        if label != "black and white" {
            assert!((unfiltered[0] / p.white[0] - unfiltered[2] / p.white[2]).abs() > 1e-2, "{label}: {unfiltered:?}");
        }
        // Twin: ONI's window laid on the print would put the base on black and the densest read on white.
        let windowed = |v: V4| -> [f32; 3] { std::array::from_fn(|c| (v[c] - black[c]) / (white[c] - black[c])) };
        assert!(windowed(black) == [0.0; 3] && windowed(grey).iter().all(|g| (g - *GREY_PRINT).abs() > 0.01), "{label}");
    }
    // A digital photo's reference grey is its scene grey: the paper prints the scene's grey at its own grey.
    let s = digital();
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let scan = cpu::virtual_negative([film::GREY, film::GREY, film::GREY, 1.0], &stock.kernel, &stock.table);
    let k = conversion::stage47(&s);
    let printed = cpu::stage47(scan, &k);
    let white = k.paper.unwrap().white;
    assert!((0..3).all(|c| (printed[c] / white[c] * luma(white) - *GREY_PRINT).abs() < 2e-3), "{printed:?}");
}

#[test]
fn the_print_is_the_papers_own_its_colour_density_included() {
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let mut lit = prepared_negative();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.black_point) = (2.0, 0.2, 0.1, 0.03);
    for (label, s) in rests().into_iter().chain([("lit negative", lit)]) {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        assert!(k.print_on == 0.0 && k.hunt == 0.0, "{label}: no print curve, no Hunt on the paper");
        let paper_alone = |q: V4| {
            let read = k.negative.as_ref().map_or(q, |n| cpu::negative(q, n));
            let shares = cpu::windowed(read, &k.after_negative());
            cpu::paper([shares[0], shares[1], shares[2], read[3]], &p)
        };
        let worst = |k: &Stage47| pixels.iter().map(|q| {
            let (a, b) = (cpu::stage47(*q, k), paper_alone(*q));
            (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0f32, f32::max)
        }).fold(0.0f32, f32::max);
        let alone = worst(&k);
        println!("{label}: stage 4-7 within {alone:.1e} of the paper's print");
        assert!(alone == 0.0, "{label}: {alone}");
        // Twin: the paper reading the scan's light itself, no window between.
        let unwindowed = pixels.iter().map(|q| {
            let (a, b) = (cpu::stage47(*q, &k), cpu::paper(*q, &p));
            (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0f32, f32::max)
        }).fold(0.0f32, f32::max);
        assert!(unwindowed > 1e-2, "{label}: {unwindowed}");
    }
    // Saturation lays on the negative, the paper printing what it spreads.
    let mut dense = prepared_negative();
    dense.density = 0.4;
    let k = conversion::stage47(&dense);
    let n = k.negative.unwrap();
    let (q, plain) = (pixels[7], Stage47 { negative: Some(Negative { spread: 1.0, ..n }), ..k });
    let (v, w) = (cpu::stage47(q, &k), cpu::stage47(q, &plain));
    assert!(n.spread > 1.0 && (v[0] / v[2]).ln().abs() > (w[0] / w[2]).ln().abs(), "{v:?} {w:?}");
}

#[test]
fn a_legacy_print_on_a_paper_rests_at_its_rest_and_its_keys_move_the_enlarger() {
    let mut s = prepared_negative();
    s.legacy_print = Some(LegacyPrint::default());
    let laid = scanner::on_legacy_print(&s).unwrap();
    let grey = grey_scan(&s);
    let k = conversion::stage47(&laid);
    let rest = cpu::stage47(grey, &k);
    let white = k.paper.unwrap().white;
    assert!((0..3).all(|c| (rest[c] / white[c] * luma(white) - *GREY_PRINT).abs() < 2e-4), "{rest:?}");
    // A cyan key prints cyaner, as on ONI's print: its red falls, its blue holds.
    s.legacy_print = Some(LegacyPrint { c: 4, ..LegacyPrint::default() });
    let keyed = cpu::stage47(grey, &conversion::stage47(&scanner::on_legacy_print(&s).unwrap()));
    assert!(keyed[0] < rest[0] * 0.9 && (keyed[2] - rest[2]).abs() < 0.01, "{keyed:?} {rest:?}");
    // Twin: the keys at rest, no key moves the enlarger.
    s.legacy_print = Some(LegacyPrint::default());
    assert_eq!(cpu::stage47(grey, &conversion::stage47(&scanner::on_legacy_print(&s).unwrap())), rest);
}

#[test]
fn a_measure_and_a_positive_read_as_its_own_negative_read_no_paper() {
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
    assert!(k.paper.is_none() && k.print_on == 1.0, "ONI's print");
}

#[test]
fn the_gpu_prints_on_the_paper_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_prints_on_the_paper_as_its_reference_does") else { return };
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 9, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 5] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0], [1e-6, 0.4, 0.002, 0.5]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    let mut lit = prepared_negative();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.highlights) = (2.0, 0.2, 0.1, -0.1);
    lit.print.set_filters([0.2, -0.1, 0.0]);
    (lit.print.black_point, lit.print.white_point, lit.print.channel_contrast, lit.print.channel_mid) = (0.03, 0.05, [0.1, 0.0, -0.1], [0.0, 0.1, 0.0]);
    (lit.print.channel_white, lit.print.channel_black, lit.print.brightness, lit.print.print_lift) = ([0.02, 0.0, 0.0], [0.0, 0.0, 0.03], 0.5, false);
    lit.density = 0.3;
    let mut mono = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..mono() };
    (mono.print.contrast, mono.print.highlights, mono.print.white_point, mono.print.brightness, mono.print.print_lift) = (0.3, -0.2, 0.04, -0.5, false);
    let dpii = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-crystal-archive-digital-dpii")), ..prepared_negative() };
    let frontier = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..prepared_negative() };
    let mut anchored = PipelineSettings { print: Print { print_lift: true, brightness: -0.7, ..lit.print }, ..lit.clone() };
    anchored.print.exposure -= 3.0;
    let darker = PipelineSettings { print: Print { brightness: -4.0, ..anchored.print }, ..anchored.clone() };
    let brighter = PipelineSettings { print: Print { brightness: 4.0, ..lit.print }, ..lit.clone() };
    let mut slide = PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Positive, true) };
    slide.gains.stops = [0.3, 0.0, -0.4];
    let mut lit_slide = slide.clone();
    (lit_slide.print.exposure, lit_slide.print.contrast, lit_slide.print.shadows, lit_slide.print.highlights) = (0.5, 0.3, 0.2, -0.1);
    (lit_slide.print.black_point, lit_slide.print.white_point, lit_slide.print.channel_mid, lit_slide.print.brightness) = (0.04, 0.03, [0.1, 0.0, -0.1], 0.4);
    lit_slide.print.set_filters([0.1, 0.0, -0.2]);
    let darker_slide = PipelineSettings { print: Print { brightness: -3.0, ..lit_slide.print }, ..lit_slide.clone() };
    let brighter_slide = PipelineSettings { print: Print { brightness: 3.0, ..lit_slide.print }, ..lit_slide.clone() };
    let frames = [
        ("digital", digital()),
        ("lit negative", lit),
        ("lit negative, its pick on its rest", anchored),
        ("lit negative, Brightness -4", darker),
        ("lit negative, Brightness +4", brighter),
        ("black and white", mono),
        ("Crystal Archive DPII", dpii),
        ("Frontier, its scanner's mix", frontier),
        ("slide", slide),
        ("lit slide", lit_slide),
        ("lit slide, Brightness -3", darker_slide),
        ("lit slide, Brightness +3", brighter_slide),
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

/// Every stock a digital photo is shot on rests on every paper with its grey printed at a plain RAW's share of the
/// paper's white, neutral, the Frontier's steep curves included, but Fujiclear, a display material whose layers part past
/// what the filtration reaches. Twin: at the rest's time on another filtration the Frontier prints its grey off neutral.
#[test]
fn every_stock_rests_its_grey_neutral_on_every_paper() {
    let mut missed = Vec::new();
    // A reversal film's projection reaches a paper through the internegative, whose own checks hold it.
    for stock in VirtualStock::all().into_iter().filter(|s| !s.entry().reversal) {
        for p in PaperStock::all().into_iter().filter(|p| *p != PaperStock::of("fujifilm-fujiclear")) {
            let s = PipelineSettings { print_paper: Some(p), ..digital().on_stock(stock) };
            let k = conversion::stage47(&s);
            let grey = cpu::stage47(grey_scan(&s), &k);
            let white = k.paper.unwrap().white;
            let off = (0..3).map(|c| (grey[c] / white[c] * luma(white) - *GREY_PRINT).abs()).fold(0.0f32, f32::max);
            if off > 2e-4 {
                missed.push(format!("{stock:?} on {p:?}: {off:.1e}"));
            }
        }
    }
    println!("{} stock and paper pairs off their grey: {missed:?}", missed.len());
    // The window lays every stock's grey on one density, so the paper's own filtration serves them all.
    assert!(missed.is_empty(), "{missed:?}");
    let s = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..digital() };
    let k = conversion::stage47(&s);
    let rest = conversion::printing(&s).unwrap().paper;
    let start = Paper { filters: [1.0, 10f32.powf(-0.5), 10f32.powf(-0.5)], ..rest };
    let printed = cpu::stage47(grey_scan(&s), &Stage47 { paper: Some(start), ..k });
    assert!((printed[0] - printed[1]).abs() > 0.02 || (printed[2] - printed[1]).abs() > 0.02, "{printed:?}");
}
