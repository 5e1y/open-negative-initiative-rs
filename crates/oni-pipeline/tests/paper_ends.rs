//! The paper's print between its own Dmax and white: True blacks lays the Dmax on pure black, the white kept, continuous
//! and monotone; Brightness an odds gain at twice its stops, both ends held. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use frames::*;
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::conversion;
use oni_render::{PAPER_CURVE_NODES, Paper};
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;

fn frames() -> [(&'static str, PipelineSettings); 3] {
    [("negative", prepared_negative()), ("digital", digital()), ("black and white", mono())]
}

fn paper_of(s: &PipelineSettings, f: impl Fn(&mut Print)) -> Paper {
    let mut t = s.clone();
    f(&mut t.print);
    conversion::stage47(&t).paper.expect("a paper")
}

#[test]
fn true_blacks_lay_the_papers_dmax_on_black_the_white_kept() {
    for (label, s) in frames() {
        let off = paper_of(&s, |_| {});
        let on = paper_of(&s, |p| p.true_blacks = true);
        // The paper's own ends, as its print shows its densest and its bare layers.
        let (dmax, white) = (on.dmax, on.white);
        assert_eq!((dmax, white), (off.dmax, off.white), "{label}: the same paper");
        let black = on.ended(dmax);
        let kept = on.ended(white);
        println!("{label}: Dmax {dmax:?} to {black:?}, white {white:?} to {kept:?}");
        assert!(black.iter().all(|v| v.abs() < 1e-7) && (0..3).all(|c| (kept[c] - white[c]).abs() < 1e-6), "{label}");
        // Along the negative's range, past both its ends, the print rises with the density and without a jump: its
        // largest step halves as the sampling doubles (a straight line in light from the Dmax to the white, C0 as the
        // paper's curves are at their nodes).
        let print = |u: f32| on.ended(on.printed(on.span.map(|a| a * u)));
        let (coarse, fine) = (steps(print, 2000), steps(print, 4000));
        println!("{label}: the largest step {coarse:.2e} over 2000 tones, {fine:.2e} over 4000");
        assert!(fine < 0.6 * coarse, "{label}");
        // Twin: a jump of a hundredth past the middle keeps its step whatever the sampling, so the check refuses it.
        let jump = |u: f32| print(u).map(|v| if u > 0.5 { v + 0.01 } else { v });
        assert!(steps(jump, 4000) >= 0.6 * steps(jump, 2000), "{label}");
        // The base and the grey rise less than the paper's span: the stretch is the Dmax's alone.
        let base = on.printed([0.0; 3]);
        let (stretched, plain) = (on.ended(base), off.ended(base));
        assert!((0..3).all(|c| stretched[c] < plain[c]), "{label}: {stretched:?} {plain:?}");
        // Exposure brightening still lifts the base off pure black.
        let up = paper_of(&s, |p| (p.true_blacks, p.exposure) = (true, p.exposure + 1.0));
        assert!(up.ended(up.printed([0.0; 3])).iter().all(|v| *v > 1e-3), "{label}");
        // Twin: off, the paper's Dmax stays its own light.
        assert!(off.ended(dmax).iter().all(|v| *v > 2e-3), "{label}");
    }
}

#[test]
fn brightness_is_an_odds_gain_at_twice_its_stops_between_the_papers_ends() {
    for (label, s) in frames() {
        let rest = paper_of(&s, |_| {});
        let grey = rest.printed(paper::anchors(&s, conversion::stage47(&s).dyes.as_ref())[2]);
        assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 2e-6), "{label}: {grey:?}");
        for (brightness, true_blacks) in [(-1.0f32, false), (0.5, false), (1.0, false), (1.0, true)] {
            let p = paper_of(&s, |q| (q.brightness, q.true_blacks) = (brightness, true_blacks));
            let floor = if true_blacks { [0.0; 3] } else { p.dmax };
            let (dmax, white) = (p.ended(p.dmax), p.ended(p.white));
            assert!((0..3).all(|c| (dmax[c] - floor[c]).abs() < 1e-7 && (white[c] - p.white[c]).abs() < 1e-6), "{label}, {brightness}: ends held");
            let law = |gain: f32| -> [f32; 3] {
                std::array::from_fn(|c| {
                    let u = (grey[c] - p.dmax[c]) / (p.white[c] - p.dmax[c]);
                    floor[c] + (p.white[c] - floor[c]) * (gain * u / (1.0 + (gain - 1.0) * u))
                })
            };
            let moved = p.ended(grey);
            let geared = law((Print::EXPOSURE_GEAR * brightness).exp2());
            let off = (0..3).map(|c| (moved[c] - geared[c]).abs()).fold(0.0f32, f32::max);
            let (tag, gear, stops) = (if true_blacks { ", true blacks" } else { "" }, Print::EXPOSURE_GEAR, (moved[1] / grey[1]).log2());
            println!("{label}, Brightness {brightness:+}{tag}: the grey {:.4} to {:.4} ({stops:+.3} stops), within {off:.1e} of the odds gain at {gear}x its stops", grey[1], moved[1]);
            assert!(off < 2e-6, "{label}, {brightness}");
            // Twin: the gain at the slider's own stops, ungeared, misses the grey.
            let ungeared = law(brightness.exp2());
            assert!((0..3).any(|c| (moved[c] - ungeared[c]).abs() > 1e-2), "{label}, {brightness}");
        }
    }
    // The ends are the paper's curves' own: its densest node and its bare one.
    let p = paper_of(&prepared_negative(), |_| {});
    assert_eq!(p.dmax, p.viewed(p.curves[PAPER_CURVE_NODES - 1]));
}

/// The largest rise of `print` between two neighbours of `n` tones across the negative's range and past both its ends,
/// every tone at least its predecessor and at or above pure black.
fn steps(print: impl Fn(f32) -> [f32; 3], n: usize) -> f32 {
    let mut last = print(-1.0);
    let mut worst = 0.0f32;
    for i in 1..=n {
        let v = print(-1.0 + 3.0 * i as f32 / n as f32);
        assert!((0..3).all(|c| v[c] >= last[c] - 1e-7 && v[c] >= 0.0), "monotone and at or above black: {v:?}");
        worst = (0..3).map(|c| v[c] - last[c]).fold(worst, f32::max);
        last = v;
    }
    worst
}
