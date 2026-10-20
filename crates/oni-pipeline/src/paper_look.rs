//! Frontier 2's look in place of ONI's: the ONI look's tone (its exposure), then each Crystal Archive
//! layer's curve, cropped from its toe to its shoulder and laid from black to white, as one curve a channel.

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

/// The paper's three curves, each after the ONI look's tone, as the Curves block draws and the curves
/// stage applies them.
pub static CURVES: LazyLock<CurveSet> = LazyLock::new(|| {
    let paper: Paper = serde_json::from_slice(include_bytes!("../resources/scanner/paper.json")).expect("paper.json");
    let curve = |points: &[[f64; 2]]| {
        let layer = Curve { points: points.iter().map(|p| Point::new(p[0], p[1])).collect() }.evaluated();
        let composed = (0..POINTS).map(|k| {
            let u = k as f64 / (POINTS - 1) as f64;
            Point::new(u, layer.value(crate::look_lab::edits::oni_tone(u)).clamp(0.0, 1.0))
        });
        Curve { points: composed.collect() }
    };
    CurveSet { red: curve(&paper.red), green: curve(&paper.green), blue: curve(&paper.blue), ..CurveSet::default() }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_prints_onis_tone_from_black_to_white_with_a_toe_and_a_shoulder() {
        for c in [&CURVES.red, &CURVES.green, &CURVES.blue] {
            let e = c.evaluated();
            assert!(e.value(0.0) < 0.02 && e.value(1.0) > 0.95, "{} {}", e.value(0.0), e.value(1.0));
            assert!(e.value(0.05) < 0.05 && e.value(0.95) > 0.9, "a toe and a shoulder: {} {}", e.value(0.05), e.value(0.95));
        }
        // Twin: the paper is not the ONI look's tone alone.
        let green = CURVES.green.evaluated();
        let gap = (0..=20).map(|k| k as f64 / 20.0).map(|u| (green.value(u) - crate::look_lab::edits::oni_tone(u)).abs()).fold(0.0, f64::max);
        assert!(gap > 0.03, "{gap}");
    }
}
