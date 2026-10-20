//! Proto: ONI's own inversion, then the virtual Frontier's look fitted on the synthetic checker as a
//! table from ONI's display code to the virtual Frontier's.

use oni_render::ColorCube;
use std::sync::LazyLock;

pub static CUBE: LazyLock<ColorCube> =
    LazyLock::new(|| crate::frontier_table::read(include_str!("../resources/scanner/proto.cube")).expect("proto.cube"));

#[cfg(test)]
mod tests {
    use super::*;
    use oni_render::cpu;

    #[test]
    fn a_measured_roll_s_grey_enters_proto_where_the_chart_s_did() {
        use crate::{conversion, scanner};
        use oni_render::ScannerEncode;
        let entry = |proto: bool| {
            let mut s = scanner::check_roll();
            (s.face, s.scanner_machine, s.scanner_proto) = (oni_settings::Face::Scanner, oni_settings::ScannerMachine::Oni, proto);
            let k = conversion::stage47(&scanner::on_oni_machine(&s).unwrap());
            let t = scanner::CHECK_GREY.map(|d| 10f32.powf(-d));
            cpu::scanner_encode(cpu::stage47([t[0], t[1], t[2], 1.0], &k), &ScannerEncode { root: 1.0 })
        };
        let grey = entry(true);
        assert!((0..3).all(|c| (grey[c] - 0.633).abs() < 0.01), "the chart export's grey: {grey:?}");
        // Twin: without the anchor the grey enters a fifth of a density lower, deep in Proto's toe.
        assert!(entry(false)[1] < 0.55);
    }

    #[test]
    fn proto_darkens_onis_grey_onto_the_virtual_frontier_s_and_climbs_along_the_greys() {
        assert_eq!(CUBE.cube_dimension, 33);
        // ONI's 18 % grey in display code lands on the virtual Frontier's green-tinged grey.
        let grey = cpu::color_cube([0.633, 0.633, 0.633, 1.0], &CUBE);
        assert!((grey[0] - 0.39).abs() < 0.03 && (grey[1] - 0.44).abs() < 0.03 && grey[1] > grey[0], "{grey:?}");
        let axis: Vec<f32> = (0..=20).map(|i| cpu::color_cube([i as f32 / 20.0; 4], &CUBE)[1]).collect();
        assert!(axis.windows(2).all(|w| w[1] >= w[0]), "{axis:?}");
        // Twin: ONI's own look reads the same code elsewhere, so a frame on Proto is not ONI's render.
        let oni = cpu::color_cube([0.633f32.sqrt(); 4], &crate::oni_look::CUBE);
        assert!((0..3).any(|c| (oni[c] - grey[c]).abs() > 0.05), "{oni:?}");
    }
}
