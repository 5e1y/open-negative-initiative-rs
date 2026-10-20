//! Frontier 2's paper: each Crystal Archive layer's curve, cropped from its toe to its shoulder and laid
//! from the window's black to its white, so it prints the window ONI measured and nothing past it.

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

/// The paper's three curves as the Curves block draws and the curves stage applies them.
pub static CURVES: LazyLock<CurveSet> = LazyLock::new(|| {
    let paper: Paper = serde_json::from_slice(include_bytes!("../resources/scanner/paper.json")).expect("paper.json");
    let curve = |points: &[[f64; 2]]| Curve { points: points.iter().map(|p| Point::new(p[0], p[1])).collect() };
    CurveSet { red: curve(&paper.red), green: curve(&paper.green), blue: curve(&paper.blue), ..CurveSet::default() }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_runs_from_the_window_s_black_to_its_white_with_a_toe_and_a_shoulder() {
        for c in [&CURVES.red, &CURVES.green, &CURVES.blue] {
            let e = c.evaluated();
            assert!(e.value(0.0) < 1e-3 && (e.value(1.0) - 1.0).abs() < 1e-3);
            assert!(e.value(0.1) < 0.05 && e.value(0.9) > 0.9, "a toe and a shoulder: {} {}", e.value(0.1), e.value(0.9));
        }
        // Twin: the paper is no diagonal through its middle.
        assert!((CURVES.green.evaluated().value(0.3) - 0.3).abs() > 0.1);
    }
}
