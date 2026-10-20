//! A frame's Look Lab edits on the shipped Frontier set, and the table they bake into on the measured
//! cube's own lattice, where the set at rest returns the measured table node for node.

use super::measure::SIDE;
use super::{FITTED, LabParams, bake};
use oni_render::ColorCube;
use oni_settings::frontier_lab::FrontierLab;

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
}
