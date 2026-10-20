//! Frontier 2's look in place of ONI's: the ONI look's tone, each channel moved by its Crystal Archive
//! layer's distance from the three layers' mean, so the paper lends its colour and not its contrast.

use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct Paper {
    red: Vec<[f64; 2]>,
    green: Vec<[f64; 2]>,
    blue: Vec<[f64; 2]>,
}

/// Points a channel's composed curve is drawn and applied through.
const POINTS: usize = 33;

/// The three channels' curves, as the Curves block draws and the curves stage applies them.
pub static CURVES: LazyLock<CurveSet> = LazyLock::new(|| {
    let paper: Paper = serde_json::from_slice(include_bytes!("../resources/scanner/paper.json")).expect("paper.json");
    let layer = |points: &[[f64; 2]]| Curve { points: points.iter().map(|p| Point::new(p[0], p[1])).collect() }.evaluated();
    let layers = [layer(&paper.red), layer(&paper.green), layer(&paper.blue)];
    let curve = |c: usize| {
        let points = (0..POINTS).map(|k| {
            let u = k as f64 / (POINTS - 1) as f64;
            let tone = crate::look_lab::edits::oni_tone(u);
            let mean = layers.iter().map(|l| l.value(tone)).sum::<f64>() / 3.0;
            Point::new(u, (tone + layers[c].value(tone) - mean).clamp(0.0, 1.0))
        });
        Curve { points: points.collect() }
    };
    CurveSet { red: curve(0), green: curve(1), blue: curve(2), ..CurveSet::default() }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_lends_its_colour_and_the_mean_keeps_onis_tone() {
        let [r, g, b] = [&CURVES.red, &CURVES.green, &CURVES.blue].map(|c| c.evaluated());
        for u in (1..20).map(|k| k as f64 / 20.0) {
            let mean = (r.value(u) + g.value(u) + b.value(u)) / 3.0;
            let tone = crate::look_lab::edits::oni_tone(u);
            assert!((mean - tone).abs() < 0.01, "no contrast of the paper's at {u}: {mean} against {tone}");
        }
        // Twin: the channels part somewhere, the paper's colour.
        let gap = (1..20).map(|k| k as f64 / 20.0).map(|u| (r.value(u) - b.value(u)).abs()).fold(0.0, f64::max);
        assert!(gap > 0.005, "{gap}");
    }
}
