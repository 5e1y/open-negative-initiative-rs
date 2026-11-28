//! Every paper as its file bakes it (`scripts/paper-looks.py`), nothing solved in the app: the print lays the bare film on
//! pure black and the window's white on pure white whatever the frame; a data sheet's look lays the window on its useful
//! range, the bare film at 0.9 Dmax of its neutral and the white at its D-min and 0.04; its filtration holds a neutral
//! ramp nearer neutral than none. Each verdict has a twin.

use oni_pipeline::conversion;
use oni_pipeline::paper;
use oni_render::{PAPER_CURVE_NODES, Paper, cpu};
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings};
use serde::Deserialize;

const ROYAL: PaperStock = PaperStock::of("kodak-royal-digital");

/// A negative at rest on `paper`, its ends pure.
fn on(paper: PaperStock) -> PipelineSettings {
    PipelineSettings { print_paper: Some(paper), ..PipelineSettings::rest(ConversionMode::Negative, true) }
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper().expect("a print on a paper")
}

#[test]
fn every_paper_lays_the_bare_film_on_black_and_the_windows_white_on_white_whatever_the_frame() {
    // Another frame of other windows and gains: the paper reads its shares alone.
    let mut wide = on(ROYAL);
    wide.gains.stops = [-0.8, 0.0, 1.2];
    for (w, (black, white)) in [&mut wide.levels.red, &mut wide.levels.green, &mut wide.levels.blue].into_iter().zip([(0.5, 1.9), (0.52, 2.1), (0.55, 2.3)]) {
        (w.black, w.white) = (black, white);
    }
    for p in PaperStock::all() {
        let (q, other) = (paper_of(&on(p)), paper_of(&PipelineSettings { print_paper: Some(p), ..wide.clone() }));
        assert_eq!(q, other, "{p:?}: the frame moves nothing on the paper");
        let (black, white) = (q.shown([0.0; 3]), q.shown([1.0; 3]));
        assert!(black.iter().all(|v| v.abs() < 1e-6) && white.iter().all(|v| (v - 1.0).abs() < 1e-5), "{p:?}: {black:?} {white:?}");
        // Twins: Print lift keeps the paper's own ends, and a legacy print laid takes no paper.
        let lifted = paper_of(&PipelineSettings { print: oni_settings::tone::Print { print_lift: true, ..on(p).print }, ..on(p) });
        assert!(lifted.anchor != [0.0; 3] && lifted.white_anchor != [1.0; 3], "{p:?}: Print lift");
        assert!(conversion::stage47(&PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..on(p) }).paper().is_none(), "{p:?}");
    }
}

#[derive(Deserialize)]
struct Profile {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    density_curves: Vec<[f32; 3]>,
}

/// A data sheet's neutral at the window's share `t`, the three channels' mean density.
fn neutral(q: &Paper, t: f32) -> f32 {
    q.dyed(q.amounts([t; 3])).iter().sum::<f32>() / 3.0
}

#[test]
fn a_data_sheets_look_lays_the_window_on_its_useful_range() {
    for p in PaperStock::all().into_iter().filter(|p| !p.is_machine()) {
        let sheet: Profile = serde_json::from_str(&paper::source(p).expect("a data sheet's paper")).unwrap();
        let mean = |d: &[f32; 3]| (d[0] + d[1] + d[2]) / 3.0;
        let (floor, top) = (mean(&sheet.data.density_curves[0]), sheet.data.density_curves.iter().map(mean).fold(0.0f32, f32::max));
        let q = paper_of(&on(p));
        let (at_black, at_white) = (neutral(&q, 0.0), neutral(&q, 1.0));
        println!("{}: the bare film at D {at_black:.3} (0.9 Dmax {:.3}), the white at D {at_white:.3} (D-min + 0.04 {:.3})", p.name(), 0.9 * top, floor + 0.04);
        assert!((at_black - 0.9 * top).abs() < 5e-3 && (at_white - floor - 0.04).abs() < 5e-3, "{p:?}: {at_black} {at_white}");
        // Twin: the window's middle lies at neither end.
        assert!((neutral(&q, 0.5) - 0.9 * top).abs() > 0.1 && (neutral(&q, 0.5) - floor - 0.04).abs() > 0.1, "{p:?}");
    }
}

/// How far from neutral the frame `s` prints the scans of one density on every channel across its window, the most of
/// a channel's cube root of its light off green's.
fn ramp_strays(s: &PipelineSettings) -> f32 {
    let k = conversion::stage47(s);
    let w = s.levels.windows();
    (0..=64)
        .map(|i| {
            let t = i as f32 / 64.0;
            let scan: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-(w[c].black + t * (w[c].white - w[c].black))) / k.gain[c] - k.pedestal);
            let v = cpu::stage47([scan[0], scan[1], scan[2], 1.0], &k).map(|v| v.max(0.0).cbrt());
            (v[0] - v[1]).abs().max((v[2] - v[1]).abs())
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_papers_baked_filtration_holds_a_neutral_ramp_nearer_neutral_than_none() {
    let windows = on(ROYAL).levels.windows();
    assert!(windows.iter().all(|w| *w == windows[1]), "one window on the three channels");
    println!("a neutral ramp strays {:.4} on Royal Digital", ramp_strays(&on(ROYAL)));
    // Its dyes' crosstalk and its layers' own curves tint it.
    assert!(ramp_strays(&on(ROYAL)) > 5e-3);
    // Royal's filtration, baked along the whole window, holds its ramp nearer neutral than its curves taken unfiltered:
    // each layer read where its filter's light, given back, would lay it.
    let q = paper_of(&on(ROYAL));
    let sheet = paper::sheet(ROYAL).expect("a shipped paper");
    let cc = sheet.filtration.expect("a data sheet's filtration");
    let unfiltered: [[f32; 3]; PAPER_CURVE_NODES] = std::array::from_fn(|i| {
        let t = q.curve_low + q.curve_step * i as f32;
        std::array::from_fn(|c| q.amounts([t - cc[c] / 100.0 / q.latitude; 3])[c])
    });
    let unfiltered = Paper { curves: unfiltered, ..q };
    let unfiltered = Paper { pick: unfiltered.printed([0.0; 3]), white: unfiltered.printed([1.0; 3]), ..unfiltered };
    let strays = |q: &Paper| (0..=32).map(|i| q.shown([i as f32 / 32.0; 3]).map(|v| v.max(0.0).cbrt())).map(|v| (v[0] - v[1]).abs().max((v[2] - v[1]).abs())).fold(0.0f32, f32::max);
    println!("Royal Digital: filtered {:.4}, unfiltered {:.4}, filtration {cc:?} CC", strays(&q), strays(&unfiltered));
    assert!(strays(&q) < strays(&unfiltered), "{} {}", strays(&q), strays(&unfiltered));
}
