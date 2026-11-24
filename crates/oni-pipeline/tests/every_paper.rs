//! A print on a paper, Prepare's window laid on the paper's useful range (owner's calls): ONI Standard prints the
//! data sheets' mean logistic between pure black and pure white; every paper lays the bare film on its black end and the
//! window's white on its white end whatever the frame, no grey placed; a neutral ramp prints neutral on ONI Standard. Each
//! verdict has a twin; a legacy print's own time is `legacy_print`'s.

use oni_pipeline::conversion;
use oni_pipeline::paper::{self, placing};
use oni_render::{PAPER_CURVE_NODES, Paper, cpu};
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings};

const ROYAL: PaperStock = PaperStock::of("kodak-royal-digital");

/// ONI Standard's curve over the window share `s`, as light between its ends: the logistic in density the six data-sheet
/// papers' mean fits.
fn logistic(s: f32) -> f32 {
    let d = |s: f32| 2.484 / (1.0 + (5.587 * (s - 0.354)).exp());
    let light = |s: f32| 10f32.powf(-d(s));
    (light(s) - light(0.0)) / (light(1.0) - light(0.0))
}

/// A negative at rest on `paper`, its ends pure.
fn on(paper: PaperStock) -> PipelineSettings {
    PipelineSettings { print_paper: Some(paper), ..PipelineSettings::rest(ConversionMode::Negative, true) }
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper().expect("a print on a paper")
}

#[test]
fn oni_standard_prints_the_data_sheets_mean_logistic_from_pure_black_to_pure_white() {
    assert_eq!(PaperStock::default(), PaperStock::of("oni-standard"));
    let off = |p: &Paper| (0..=64).map(|i| i as f32 / 64.0).map(|t| (p.shown([t; 3])[1] - logistic(t)).abs()).fold(0.0f32, f32::max);
    let standard = paper_of(&on(PaperStock::default()));
    let (black, white) = (standard.shown([0.0; 3]), standard.shown([1.0; 3]));
    println!("ONI Standard: black {black:?}, white {white:?}, off the logistic by {:.1e}", off(&standard));
    assert!(black.iter().all(|v| v.abs() < 1e-6) && white.iter().all(|v| (v - 1.0).abs() < 1e-6), "{black:?} {white:?}");
    assert!(off(&standard) < 5e-4, "{}", off(&standard));
    // Twin: Royal Digital, a paper of its own curves, strays from the mean.
    let royal = paper_of(&on(ROYAL));
    println!("twin, Royal Digital off the logistic by {:.3}", off(&royal));
    assert!(off(&royal) > 0.01, "{}", off(&royal));
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
    let mut wide = on(PaperStock::default());
    wide.gains.stops = [-0.8, 0.0, 1.2];
    for (w, (black, white)) in [&mut wide.levels.red, &mut wide.levels.green, &mut wide.levels.blue].into_iter().zip([(0.5, 1.9), (0.52, 2.1), (0.55, 2.3)]) {
        (w.black, w.white) = (black, white);
    }
    for p in PaperStock::all() {
        let sheet = paper::sheet(p);
        let (q, other) = (paper_of(&on(p)), paper_of(&PipelineSettings { print_paper: Some(p), ..wide.clone() }));
        let green = |q: &Paper, t: f32| q.exposed(q.density([t; 3]))[1];
        let (black, white) = (green(&q, 0.0), green(&q, 1.0));
        let (top, floor) = (neutral(sheet, sheet.curve_low + sheet.curve_step * (PAPER_CURVE_NODES - 1) as f32), neutral(sheet, sheet.curve_low));
        let (at_black, at_white) = (neutral(sheet, black), neutral(sheet, white));
        println!("{}: the bare film at log H {black:.3}, D {at_black:.3} (Dmax {top:.3}); the white at {white:.3}, D {at_white:.3} (D-min {floor:.3})", p.name());
        assert!(green(&other, 0.0) == black && green(&other, 1.0) == white && other.scale == q.scale && other.filters == q.filters, "{p:?}: the frame moves no time");
        if p == PaperStock::default() {
            // ONI Standard's file states its range: its curve's own share axis.
            assert!((black - 1.0).abs() < 1e-5 && white.abs() < 1e-5, "{p:?}: {black} {white}");
        } else {
            assert!((at_black - 0.9 * top).abs() < 2e-3 && (at_white - floor - 0.04).abs() < 2e-3, "{p:?}: {at_black} {at_white}");
        }
        assert!(q.shown([0.0; 3]).iter().all(|v| v.abs() < 1e-6) && q.shown([1.0; 3]).iter().all(|v| (v - 1.0).abs() < 1e-5), "{p:?}");
        // Twin: a legacy print's time, solved to show the reference grey at a plain RAW's grey, lays the bare
        // film elsewhere.
        let legacy = paper_of(&PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..on(p) });
        println!("  twin, the grey's time: the bare film at log H {:.3}", green(&legacy, 0.0));
        assert!((green(&legacy, 0.0) - black).abs() > 0.05, "{p:?}");
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
fn a_neutral_negative_ramp_prints_neutral_on_oni_standard() {
    let standard = on(PaperStock::default());
    let windows = standard.levels.windows();
    assert!(windows.iter().all(|w| *w == windows[1]), "one window on the three channels");
    println!("a neutral ramp strays {:.1e} on ONI Standard, {:.4} on Royal Digital", ramp_strays(&standard), ramp_strays(&on(ROYAL)));
    assert!(ramp_strays(&standard) < 1e-5, "{}", ramp_strays(&standard));
    // Twin: Royal Digital, its dyes' crosstalk and its layers' own curves, tints it.
    assert!(ramp_strays(&on(ROYAL)) > 5e-3);
    // Royal's own filtration, solved along the whole window, holds its ramp nearer neutral than none.
    let q = paper_of(&on(ROYAL));
    let strays = |q: &Paper| (0..=32).map(|i| q.shown([i as f32 / 32.0; 3]).map(|v| v.max(0.0).cbrt())).map(|v| (v[0] - v[1]).abs().max((v[2] - v[1]).abs())).fold(0.0f32, f32::max);
    let unfiltered = Paper { filters: [1.0; 3], ..q };
    let unfiltered = Paper { pick: unfiltered.printed([0.0; 3]), white: unfiltered.printed([1.0; 3]), ..unfiltered };
    println!("Royal Digital: filtered {:.4}, unfiltered {:.4}, offsets {:?}", strays(&q), strays(&unfiltered), paper::sheet(ROYAL).placing.offsets);
    assert!(strays(&q) < strays(&unfiltered), "{} {}", strays(&q), strays(&unfiltered));
    // The solve's own ends: the useful range it lays on, recomputed, is the sheet's.
    let sheet = paper::sheet(ROYAL);
    let [white, black] = placing::useful_range(sheet, sheet.placing.offsets);
    assert!((black - sheet.placing.black).abs() < 1e-6 && (black - white - sheet.placing.range).abs() < 1e-6);
}
