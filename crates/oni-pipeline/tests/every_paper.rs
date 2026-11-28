//! A print on a paper, Prepare's window laid on the paper's useful range (owner's calls): every paper lays the bare film on
//! its black end and the window's white on its white end whatever the frame, no grey placed; a paper's own filtration
//! holds a neutral ramp nearer neutral than none. Each verdict has a twin.

use oni_pipeline::conversion;
use oni_pipeline::paper::{self, placing};
use oni_render::{PAPER_CURVE_NODES, Paper, cpu};
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings};

const ROYAL: PaperStock = PaperStock::of("kodak-royal-digital");

/// A negative at rest on `paper`, its ends pure.
fn on(paper: PaperStock) -> PipelineSettings {
    PipelineSettings { print_paper: Some(paper), ..PipelineSettings::rest(ConversionMode::Negative, true) }
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper().expect("a print on a paper")
}

/// The paper's neutral, its three channels' mean density, with its layers at green's log exposure `x` and their offsets.
fn neutral(sheet: &paper::Sheet, x: f32) -> f32 {
    let amount = |c: usize, e: f32| {
        let f = ((e - sheet.curve_low) / sheet.curve_step).clamp(0.0, (PAPER_CURVE_NODES - 1) as f32);
        let i = (f.floor() as usize).min(PAPER_CURVE_NODES - 2);
        sheet.curves[i][c] + (f - i as f32) * (sheet.curves[i + 1][c] - sheet.curves[i][c])
    };
    let amounts: [f32; 3] = std::array::from_fn(|c| amount(c, x + sheet.placing.offsets[c]));
    (0..3).map(|c| sheet.floor[c] + (0..3).map(|k| sheet.crosstalk[c][k] * amounts[k]).sum::<f32>()).sum::<f32>() / 3.0
}

#[test]
fn every_paper_lays_prepares_black_on_its_black_and_its_white_on_its_white_whatever_the_frame() {
    // Two frames of other windows and gains: the paper reads their shares alone.
    let mut wide = on(ROYAL);
    wide.gains.stops = [-0.8, 0.0, 1.2];
    for (w, (black, white)) in [&mut wide.levels.red, &mut wide.levels.green, &mut wide.levels.blue].into_iter().zip([(0.5, 1.9), (0.52, 2.1), (0.55, 2.3)]) {
        (w.black, w.white) = (black, white);
    }
    for p in PaperStock::all() {
        let sheet = paper::sheet(p).expect("a shipped paper");
        let (q, other) = (paper_of(&on(p)), paper_of(&PipelineSettings { print_paper: Some(p), ..wide.clone() }));
        let green = |q: &Paper, t: f32| q.exposed(q.density([t; 3]))[1];
        let (black, white) = (green(&q, 0.0), green(&q, 1.0));
        let (top, floor) = (neutral(sheet, sheet.curve_low + sheet.curve_step * (PAPER_CURVE_NODES - 1) as f32), neutral(sheet, sheet.curve_low));
        let (at_black, at_white) = (neutral(sheet, black), neutral(sheet, white));
        println!("{}: the bare film at log H {black:.3}, D {at_black:.3} (Dmax {top:.3}); the white at {white:.3}, D {at_white:.3} (D-min {floor:.3})", p.name());
        assert!(green(&other, 0.0) == black && green(&other, 1.0) == white && other.scale == q.scale && other.filters == q.filters, "{p:?}: the frame moves no time");
        assert!((at_black - 0.9 * top).abs() < 2e-3 && (at_white - floor - 0.04).abs() < 2e-3, "{p:?}: {at_black} {at_white}");
        assert!(q.shown([0.0; 3]).iter().all(|v| v.abs() < 1e-6) && q.shown([1.0; 3]).iter().all(|v| (v - 1.0).abs() < 1e-5), "{p:?}");
        // Twin: a legacy print laid takes no paper, the one named included.
        assert!(conversion::stage47(&PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..on(p) }).paper().is_none(), "{p:?}");
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
fn a_papers_own_filtration_holds_a_neutral_ramp_nearer_neutral_than_none() {
    let windows = on(ROYAL).levels.windows();
    assert!(windows.iter().all(|w| *w == windows[1]), "one window on the three channels");
    println!("a neutral ramp strays {:.4} on Royal Digital", ramp_strays(&on(ROYAL)));
    // Its dyes' crosstalk and its layers' own curves tint it.
    assert!(ramp_strays(&on(ROYAL)) > 5e-3);
    // Royal's own filtration, solved along the whole window, holds its ramp nearer neutral than none.
    let q = paper_of(&on(ROYAL));
    let strays = |q: &Paper| (0..=32).map(|i| q.shown([i as f32 / 32.0; 3]).map(|v| v.max(0.0).cbrt())).map(|v| (v[0] - v[1]).abs().max((v[2] - v[1]).abs())).fold(0.0f32, f32::max);
    let unfiltered = Paper { filters: [1.0; 3], ..q };
    let unfiltered = Paper { pick: unfiltered.printed([0.0; 3]), white: unfiltered.printed([1.0; 3]), ..unfiltered };
    println!("Royal Digital: filtered {:.4}, unfiltered {:.4}, offsets {:?}", strays(&q), strays(&unfiltered), paper::sheet(ROYAL).expect("a shipped paper").placing.offsets);
    assert!(strays(&q) < strays(&unfiltered), "{} {}", strays(&q), strays(&unfiltered));
    // The solve's own ends: the useful range it lays on, recomputed, is the sheet's.
    let sheet = paper::sheet(ROYAL).expect("a shipped paper");
    let [white, black] = placing::useful_range(sheet, sheet.placing.offsets);
    assert!((black - sheet.placing.black).abs() < 1e-6 && (black - white - sheet.placing.range).abs() < 1e-6);
}
