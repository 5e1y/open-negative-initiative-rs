//! A frame's Look Lab edits on the shipped Frontier set, printed on a straight paper the Lab's curve
//! bends, and the table they bake into on the measured cube's own lattice.

use super::measure::SIDE;
use super::{FITTED, LabParams, bake};
use oni_render::ColorCube;
use oni_settings::frontier_lab::FrontierLab;
use std::sync::LazyLock;

/// The paper without Fuji's: a straight line from the table's black to its white, ONI's Curves shaping
/// the tone after the look (the owner's call).
pub static STRAIGHT: LazyLock<[f64; super::paper::NODES]> = LazyLock::new(|| {
    let (black, white) = (f64::from(crate::scanner::ENDS.0), f64::from(crate::scanner::ENDS.1));
    std::array::from_fn(|i| ((super::paper::density(i) - black) / (white - black)).clamp(0.0, 1.0))
});

/// The shipped Frontier values moved by the frame's edits, printed on a straight paper the handles bend.
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
    p.paper = *STRAIGHT;
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

/// The Frontier's look on ONI's window: each channel's share of its window (0 black, 1 white), read as
/// the table's density across its black to its white.
pub fn table_on_code(lab: &FrontierLab) -> ColorCube {
    let chain = super::Chain::new(&edited(lab));
    let (black, white) = (f64::from(crate::scanner::ENDS.0), f64::from(crate::scanner::ENDS.1));
    const N: usize = 33;
    let cube_data = (0..N * N * N)
        .map(|i| {
            let x = [i % N, (i / N) % N, i / (N * N)].map(|k| black + (white - black) * k as f64 / (N - 1) as f64);
            let c = chain.code(x);
            [c[0] as f32, c[1] as f32, c[2] as f32, 1.0]
        })
        .collect();
    ColorCube { cube_dimension: N, cube_data }
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
    fn at_rest_the_lab_is_the_machine_on_a_straight_paper_and_an_edit_moves_its_colours_only() {
        let rest = table(&FrontierLab::default());
        let on_onis_paper = LabParams { paper: *STRAIGHT, ..FITTED.frontier.clone() };
        assert!(moved(&rest, &bake(&on_onis_paper, SIDE), |_| true) < 1e-5, "the sliders at zero: the machine on a straight paper");
        // Twin: the machine's own paper prints another table.
        assert!(moved(&rest, &MACHINE.render, |_| true) > 0.05);
        let mut lab = FrontierLab::default();
        (7..=9).for_each(|i| lab.set(LabValue::Hue(i), -10.0));
        let turned = table(&lab);
        let coloured = |x: [usize; 3]| x[0].abs_diff(x[2]) >= 6;
        assert!(moved(&turned, &rest, coloured) > 0.01, "a hue edit turns the coloured nodes");
        // Twin: the greys, without chroma to turn, stay within a hair of the machine.
        assert!(moved(&turned, &rest, |x| x[0] == x[1] && x[1] == x[2]) < 2e-3);
    }

    #[test]
    fn the_straight_paper_runs_from_the_table_s_black_to_its_white() {
        let (black, white) = (f64::from(crate::scanner::ENDS.0), f64::from(crate::scanner::ENDS.1));
        let curve = super::super::paper::Curve::new(&STRAIGHT);
        assert!((curve.at((black + white) / 2.0) - 0.5).abs() < 0.01 && curve.at(black - 0.1) == 0.0);
        // Twin: the machine's measured paper is no straight line through the same middle.
        assert!((super::super::paper::Curve::new(&FITTED.frontier.paper).at((black + white) / 2.0) - 0.5).abs() > 0.05);
    }

}
