//! The Frontier's look, baked once as ONI's is: the Look Lab's chain at the machine's own values on the
//! ONI look's tone, read on the ONI machine's render in display code. Its rest rides in the gains.

use crate::look_lab::edits;
use oni_render::ColorCube;
use oni_settings::frontier_lab::FrontierLab;
use std::sync::LazyLock;

pub static CUBE: LazyLock<ColorCube> = LazyLock::new(|| edits::table_on_code(&FrontierLab::default()));

#[cfg(test)]
mod tests {
    use super::*;
    use oni_render::cpu;

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
