//! The Frontier's look, baked once as ONI's is: the Look Lab's chain at the machine's own values, its paper
//! the ONI look's tone, read on the ONI machine's render in display code. Its rest rides in the gains.

use crate::look_lab::{Chain, FITTED, paper};
use oni_render::{ColorCube, cpu};
use std::sync::LazyLock;

/// The table's nodes a side.
const NODES: usize = 33;

/// The ONI look's grey for a grey of display code `u`: the brightness its table lends the ONI machine's
/// render, printed with no exposure, which the Frontier keeps.
pub fn oni_tone(u: f64) -> f64 {
    let v = u.clamp(0.0, 1.0).powf(1.0 / f64::from(crate::oni_look::ROOT)) as f32;
    f64::from(cpu::color_cube([v, v, v, 1.0], &crate::oni_look::CUBE)[1])
}

/// Each display code read as the table's density from its black to its white, through the chain.
pub static CUBE: LazyLock<ColorCube> = LazyLock::new(|| {
    let (black, white) = (f64::from(crate::scanner::ENDS.0), f64::from(crate::scanner::ENDS.1));
    let mut look = FITTED.frontier.clone();
    look.paper = std::array::from_fn(|i| oni_tone(((paper::density(i) - black) / (white - black)).clamp(0.0, 1.0)));
    let chain = Chain::new(&look);
    let cube_data = (0..NODES * NODES * NODES)
        .map(|i| {
            let x = [i % NODES, (i / NODES) % NODES, i / (NODES * NODES)].map(|k| black + (white - black) * k as f64 / (NODES - 1) as f64);
            let c = chain.code(x);
            [c[0] as f32, c[1] as f32, c[2] as f32, 1.0]
        })
        .collect();
    ColorCube { cube_dimension: NODES, cube_data }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baked_look_keeps_onis_tone_and_lends_the_frontier_its_colour() {
        let mid = [0.45f32, 0.45, 0.45, 1.0];
        let (frontier, oni) = (cpu::color_cube(mid, &CUBE), cpu::color_cube(mid.map(f32::sqrt), &crate::oni_look::CUBE));
        assert!((frontier[1] - oni[1]).abs() < 0.05, "ONI's tone on a grey: {frontier:?} against {oni:?}");
        // Twin: a coloured code turns, so the table is no copy of ONI's.
        let sky = [0.35f32, 0.5, 0.7, 1.0];
        let (f, o) = (cpu::color_cube(sky, &CUBE), cpu::color_cube(sky.map(f32::sqrt), &crate::oni_look::CUBE));
        assert!((0..3).any(|c| (f[c] - o[c]).abs() > 0.02), "{f:?} {o:?}");
    }
}
