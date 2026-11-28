//! The paper: each paper printing each layer's density through pure dyes as a double reading of its curves; the print its
//! own; a frame on the one it names but a measure; every stock on every paper; the GPU on every branch. Where the window
//! lies on a paper is `every_paper`'s, a frame naming none `inversion`'s. Each has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::film::{self, bake};
use oni_pipeline::paper;
use oni_pipeline::{conversion, stages};
use oni_render::{Negative, PAPER_CURVE_NODES, Paper, Shown, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{LegacyPrint, PaperStock, PipelineSettings, VirtualStock};
use serde::Deserialize;

/// A paper's profile as its file holds it, read apart from the bake.
#[derive(Deserialize)]
struct Sheet {
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
    crosstalk: [[f64; 3]; 3],
}

fn sheet_of(paper: PaperStock) -> Sheet {
    #[derive(Deserialize)]
    struct File {
        data: Sheet,
    }
    serde_json::from_str::<File>(&paper::source(paper).expect("a profiled paper")).expect("it reads").data
}

/// The sheet's dye amounts over its D-min at each node, its grey unmixed through its crosstalk, held where they would fall.
fn sheet_amounts(sheet: &Sheet) -> Vec<[f64; 3]> {
    let floor = sheet.density_curves[0];
    let unmix = bake::inverse(&sheet.crosstalk);
    let mut nodes: Vec<[f64; 3]> = sheet.density_curves.iter().map(|d| bake::apply(&unmix, std::array::from_fn(|c| d[c] - floor[c]))).collect();
    for i in 1..nodes.len() {
        nodes[i] = std::array::from_fn(|c| nodes[i][c].max(nodes[i - 1][c]));
    }
    nodes
}

/// Layer `c`'s log exposure on the sheet where its amount reads `amount`, on its rising stretch.
fn exposure_of(sheet: &Sheet, nodes: &[[f64; 3]], c: usize, amount: f64) -> f64 {
    let k = nodes.partition_point(|n| n[c] <= amount).clamp(1, nodes.len() - 1) - 1;
    let t = ((amount - nodes[k][c]) / (nodes[k + 1][c] - nodes[k][c])).clamp(0.0, 1.0);
    sheet.log_exposure[k] + t * (sheet.log_exposure[k + 1] - sheet.log_exposure[k])
}

#[test]
fn every_data_sheets_look_is_its_sheets_curves_under_one_enlarger() {
    for paper in PaperStock::all().into_iter().filter(|p| !p.is_machine()) {
        let s = PipelineSettings { print_paper: Some(paper), ..film::on_reference(VirtualStock::of("kodak-portra-800"), &VirtualStock::of("kodak-portra-800").reference()) };
        let k = conversion::stage47(&s).paper().expect("a negative on its paper");
        let (sheet, cc) = (sheet_of(paper), oni_pipeline::paper::sheet(paper).unwrap().filtration.expect("a data sheet's filtration"));
        let nodes = sheet_amounts(&sheet);
        let top: [f64; 3] = std::array::from_fn(|c| nodes[nodes.len() - 1][c]);
        let rising = |c: usize, a: f32| f64::from(a) > 0.02 * top[c] && f64::from(a) < 0.98 * top[c];
        // Each grey's densities unmixed through the sheet's Status A crosstalk, read back to its log exposure on the sheet where
        // it rises: one enlarger lays every layer down by the latitude along the window, each apart from green's by its filtration.
        let unmix = bake::inverse(&sheet.crosstalk);
        let held = |t: f32| -> [f32; 3] {
            let d = k.dyed(k.amounts([t; 3]));
            bake::apply(&unmix, std::array::from_fn(|c| f64::from(d[c] - k.floor[c]))).map(|v| v as f32)
        };
        let off = |latitude: f32| -> f64 {
            let mut worst = 0.0f64;
            let (a0, ta) = (held(0.5), 0.5f32);
            for i in 0..=50 {
                let t = i as f32 / 50.0;
                let a = held(t);
                for c in 0..3 {
                    if !(rising(c, a[c]) && rising(c, a0[c])) {
                        continue;
                    }
                    let fall = exposure_of(&sheet, &nodes, c, f64::from(a[c])) - exposure_of(&sheet, &nodes, c, f64::from(a0[c]));
                    worst = worst.max((fall + f64::from(latitude) * f64::from(t - ta)).abs());
                    if c != 1 && rising(1, a[1]) {
                        let apart = exposure_of(&sheet, &nodes, c, f64::from(a[c])) - exposure_of(&sheet, &nodes, 1, f64::from(a[1]));
                        worst = worst.max((apart - f64::from(cc[1] - cc[c]) / 100.0).abs());
                    }
                }
            }
            worst
        };
        println!("{}: its look on its sheet's curves within {:.1e} of log exposure, latitude {:.3}, filtration {cc:?} CC", paper.name(), off(k.latitude), k.latitude);
        assert!(off(k.latitude) < 5e-3, "{paper:?}: {}", off(k.latitude));
        // Twin: the same amounts read under a window a fifth wider.
        assert!(off(1.2 * k.latitude) > 0.05, "{paper:?}");
    }
}

/// CIELAB chroma of a linear Rec. 2020 light on D65.
fn chroma(v: [f32; 3]) -> f64 {
    let xyz = bake::apply(&bake::rec2020_to_xyz(), v.map(f64::from));
    let white = bake::apply(&bake::rec2020_to_xyz(), [1.0; 3]);
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [x, y, z] = std::array::from_fn(|c| f(xyz[c] / white[c]));
    (500.0 * (x - y)).hypot(200.0 * (y - z))
}

#[test]
fn a_grey_keeps_its_densities_and_a_colour_loses_what_the_dyes_crosstalk_takes() {
    let s = film::on_reference(VirtualStock::of("kodak-portra-800"), &VirtualStock::of("kodak-portra-800").reference());
    for paper in PaperStock::all().into_iter().filter(|p| !p.is_machine()) {
        let k = conversion::stage47(&PipelineSettings { print_paper: Some(paper), ..s.clone() }).paper().expect("a negative on its paper");
        // The same greys through pure dyes: each channel's density along the window its own layer's.
        let grey = |t: f32| k.dyed(k.amounts([t; 3]));
        let densities: [[f32; 3]; PAPER_CURVE_NODES] = std::array::from_fn(|i| {
            let d = grey(k.curve_low + k.curve_step * i as f32);
            std::array::from_fn(|c| d[c] - k.floor[c])
        });
        let pure = Paper { curves: densities, crosstalk: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], ..k };
        let held = (0..=40).map(|i| -0.2 + 1.4 * i as f32 / 40.0).map(|t| (0..3).map(|c| (pure.dyed(pure.amounts([t; 3]))[c] - grey(t)[c]).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
        // A cyan: the red share two tenths over the others in the window's middle.
        let cyan = |p: &Paper| chroma(p.viewed(p.dyed(p.amounts([0.7, 0.5, 0.5]))));
        let kept = cyan(&k) / cyan(&pure);
        println!("{}: a grey within {held:.1e} D through pure dyes; a cyan keeps {kept:.3} of its pure dyes' chroma", paper.name());
        assert!(held < 1e-5 && kept < 0.95 && kept > 0.5, "{paper:?}: {held} {kept}");
        // Twin: the greys' densities read as amounts lay the crosstalk on a grey twice.
        let twice = Paper { curves: densities, ..k };
        assert!((0..3).any(|c| (twice.dyed(twice.amounts([0.5; 3]))[c] - grey(0.5)[c]).abs() > 0.05), "{paper:?}");
    }
}

/// The frames the rest is read on: a digital photo, a negative, and black and white.
fn rests() -> [(&'static str, PipelineSettings); 3] {
    [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono())]
}

#[test]
fn the_print_is_the_papers_own_its_colour_density_included() {
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let mut lit = prepared_negative();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows, lit.print.black_point) = (2.0, 0.2, 0.1, 0.03);
    for (label, s) in rests().into_iter().chain([("lit negative", lit)]) {
        let k = conversion::stage47(&s);
        let p = k.paper().expect(label);
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
fn a_frame_prints_on_the_paper_it_names_but_a_measure_and_a_legacy_print_naming_none_on_its_own() {
    let mut measured = prepared_negative();
    measured.print.bypassed = true;
    assert!(conversion::stage47(&measured).paper().is_none(), "a measure upstream of the print");
    // A frame on a paper and a positive read as its own negative print on the paper they name.
    let named = prepared_negative();
    let inverted = PipelineSettings { inverted_source: true, ..named.clone() };
    for (label, s) in [("a paper named", named.clone()), ("a positive read as its own negative", inverted)] {
        let k = conversion::stage47(&s);
        assert!(s.paper() == Some(SHEET_PAPER) && k.paper().is_some() && k.print_on == 0.0, "{label}");
    }
    // Twins: naming none, a frame shows its pure inversion and a legacy print keeps its own print curve.
    let unnamed = unpapered(named);
    assert!(unnamed.paper().is_none() && conversion::stage47(&unnamed).shown == Shown::Inversion);
    let legacy = PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..unnamed };
    let k = conversion::stage47(&legacy);
    assert!(k.paper().is_none() && k.shown == Shown::Curve && k.print_on == 1.0, "a legacy print's own curve");
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
    let frames = [
        ("digital", digital()),
        ("lit negative", lit),
        ("lit negative, its pick on its rest", anchored),
        ("lit negative, Brightness -4", darker),
        ("lit negative, Brightness +4", brighter),
        ("black and white", mono),
        ("Crystal Archive DPII", dpii),
        ("Frontier, its scanner's mix", frontier),
    ];
    for (label, s) in frames {
        let k = conversion::stage47(&s);
        let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        let twin = Stage47 { shown: k.paper().map_or(k.shown, |p| Shown::Paper(Paper { curve_low: p.curve_low + 1.1f32.log10() / p.latitude, ..p })), ..k };
        judge(&format!("paper, {label}"), &out, &reference, &mapped(&pixels, |p| cpu::stage47(p, &twin)));
    }
    // The paper's pass alone, on its own reading.
    let k = conversion::stage47(&digital());
    let p = k.paper().unwrap();
    let alone = whole(&mut g, &Image::new(&source).then(stages::paper(&p)));
    let brighter = Paper { brightness: 0.1, ..p };
    judge("paper alone", &alone, &mapped(&pixels, |q| cpu::paper(q, &p)), &mapped(&pixels, |q| cpu::paper(q, &brighter)));
}

/// Every stock a digital photo is shot on rests on every paper with its bare film on the paper's black and the sensor's
/// saturation entering green's window at its white, whatever the paper. Twin: a tenth more of the enlarger's time moves the print.
#[test]
fn every_stock_rests_its_ends_on_every_paper() {
    let mut missed = Vec::new();
    // A reversal film is projected, never printed.
    for stock in VirtualStock::sheets().into_iter().filter(|s| !s.entry().reversal) {
        let baked = film::stock(stock);
        for p in PaperStock::all() {
            let s = PipelineSettings { print_paper: Some(p), ..digital().on_stock(stock) };
            let k = conversion::stage47(&s);
            let q = k.paper().unwrap();
            let sat = film::saturation();
            let (bare, white) = (baked.shot([0.0, 0.0, 0.0, 1.0], 0.0), baked.shot([sat, sat, sat, 1.0], 0.0));
            let black = cpu::stage47(bare, &k);
            let share = cpu::windowed(white, &k)[1];
            let off = (0..3).map(|c| (black[c] - q.anchor[c]).abs()).fold((share - 1.0).abs(), f32::max);
            if off > 2e-4 || !cpu::stage47(white, &k).iter().chain(&black).all(|v| v.is_finite()) {
                missed.push(format!("{stock:?} on {p:?}: {off:.1e}"));
            }
        }
    }
    println!("{} stock and paper pairs off their ends: {missed:?}", missed.len());
    assert!(missed.is_empty(), "{missed:?}");
    let s = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..digital() };
    let k = conversion::stage47(&s);
    let rest = k.paper().unwrap();
    let longer = Stage47 { shown: Shown::Paper(Paper { curve_low: rest.curve_low + 1.1f32.log10() / rest.latitude, ..rest }), ..k };
    let white = film::stock(VirtualStock::of("kodak-portra-800")).shot([1.0; 4], 0.0);
    assert!((cpu::stage47(white, &longer)[1] - cpu::stage47(white, &k)[1]).abs() > 1e-3);
}
