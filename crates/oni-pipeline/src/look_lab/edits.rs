//! A frame's Look Lab edits on the shipped Frontier set, and the table they bake into on the measured
//! cube's own lattice, where the set at rest returns the measured table node for node.

use super::measure::SIDE;
use super::{FITTED, LabParams, bake};
use oni_render::ColorCube;
use oni_settings::frontier_lab::{FrontierLab, PAPER};

/// The paper handles' densities: blacks, shadows, mid-tones (the machine's neutral), highlights, whites.
pub const HANDLES: [f64; PAPER] = [0.0, 0.4, 0.75, 1.15, 1.6];

/// The handles' distances spread over density, smooth through each handle and held past the ends,
/// so the blacks' handle moves the whole toe and the whites' the whole shoulder.
pub fn paper_shift(handles: &[f32; PAPER], d: f64) -> f64 {
    let v = handles.map(f64::from);
    if d <= HANDLES[0] {
        return v[0];
    }
    if d >= HANDLES[PAPER - 1] {
        return v[PAPER - 1];
    }
    let k = (0..PAPER - 1).find(|&k| d <= HANDLES[k + 1]).unwrap_or(PAPER - 2);
    let slope = |i: usize| {
        let (a, b) = (i.saturating_sub(1), (i + 1).min(PAPER - 1));
        if a == b { 0.0 } else { (v[b] - v[a]) / (HANDLES[b] - HANDLES[a]) }
    };
    let h = HANDLES[k + 1] - HANDLES[k];
    let t = (d - HANDLES[k]) / h;
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * v[k] + (t3 - 2.0 * t2 + t) * h * slope(k) + (-2.0 * t3 + 3.0 * t2) * v[k + 1] + (t3 - t2) * h * slope(k + 1)
}

/// The shipped Frontier values moved by the frame's edits.
pub fn edited(lab: &FrontierLab) -> LabParams {
    let mut p = FITTED.frontier.clone();
    let add = |values: &mut [f64], by: &[f32]| values.iter_mut().zip(by).for_each(|(v, d)| *v += f64::from(*d));
    add(&mut p.hue, &lab.hue);
    add(&mut p.twist, &lab.twist);
    add(&mut p.chroma, &lab.chroma);
    add(&mut p.matrix, &lab.matrix);
    add(&mut p.paper_gain, &lab.paper_gain);
    add(&mut p.paper_offset, &lab.paper_offset);
    add(&mut p.grey_a, &lab.grey_a);
    add(&mut p.grey_b, &lab.grey_b);
    p.paper.iter_mut().enumerate().for_each(|(i, v)| *v += paper_shift(&lab.paper, super::paper::density(i)));
    // A curve held inside the code and climbing, as its spline needs.
    let mut floor = 0.0f64;
    p.paper.iter_mut().for_each(|v| {
        *v = v.clamp(floor, 1.0);
        floor = *v;
    });
    p.saturation_by_l += f64::from(lab.saturation_by_l);
    p.character += f64::from(lab.character);
    p
}

/// The edited table at the measured cube's 27 nodes a side, so the character stays exact.
pub fn table(lab: &FrontierLab) -> ColorCube {
    bake(&edited(lab), SIDE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::MACHINE;
    use oni_settings::frontier_lab::LabValue;

    /// The worst channel gap over the nodes whose density `pick` keeps.
    fn moved(a: &ColorCube, b: &ColorCube, pick: impl Fn([usize; 3]) -> bool) -> f32 {
        let n = a.cube_dimension;
        (0..n * n * n)
            .filter(|i| pick([i % n, (i / n) % n, i / (n * n)]))
            .flat_map(|i| (0..3).map(move |c| (a.cube_data[i][c] - b.cube_data[i][c]).abs()))
            .fold(0.0, f32::max)
    }

    #[test]
    fn at_rest_the_lab_is_the_measured_table_and_an_edit_moves_its_colours_only() {
        let rest = table(&FrontierLab::default());
        assert_eq!(rest.cube_dimension, MACHINE.render.cube_dimension);
        assert!(moved(&rest, &MACHINE.render, |_| true) < 1e-5, "the sliders at zero render the machine");
        let mut lab = FrontierLab::default();
        (7..=9).for_each(|i| lab.set(LabValue::Hue(i), -10.0));
        let turned = table(&lab);
        let coloured = |x: [usize; 3]| x[0].abs_diff(x[2]) >= 6;
        assert!(moved(&turned, &rest, coloured) > 0.01, "a hue edit turns the coloured nodes");
        // Twin: the greys, without chroma to turn, stay within a hair of the machine.
        assert!(moved(&turned, &rest, |x| x[0] == x[1] && x[1] == x[2]) < 2e-3);
    }

    #[test]
    fn a_paper_node_raised_lifts_the_blacks_and_the_curve_stays_climbing() {
        let mut lab = FrontierLab::default();
        lab.set(LabValue::Paper(0), 0.08);
        let p = edited(&lab);
        assert!((p.paper[0] - FITTED.frontier.paper[0] - 0.08).abs() < 1e-6 && (p.paper[1] - FITTED.frontier.paper[1] - 0.08).abs() < 1e-6 && p.paper[2] > FITTED.frontier.paper[2], "the whole toe lifts: {:?}", &p.paper[..5]);
        assert!((p.paper[14] - FITTED.frontier.paper[14]).abs() < 1e-9, "the whites stay");
        assert!(p.paper.windows(2).all(|w| w[1] >= w[0]), "{:?}", p.paper);
        // Twin: the whites pulled far down are held level with the highlights, never under them.
        let mut dip = FrontierLab::default();
        dip.set(LabValue::Paper(4), -0.9);
        let q = edited(&dip);
        assert!(q.paper.windows(2).all(|w| w[1] >= w[0]) && q.paper[14] < 0.9 && q.paper[14] == q.paper[13], "{:?}", q.paper);
    }
}
