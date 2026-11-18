//! A RAW's As Shot white, read from its metadata alone: the light the camera's own balance names,
//! never applied to a pixel, so every decode stays on its pinned multipliers.

use crate::m3::{apply, inverse};
use crate::source::{Route, Source};
use crate::{Branch, M3, Session, oni_open, route};
use oni_settings::white::{self, White};
use std::path::Path;

/// The As Shot white of a mosaic: its metadata's multipliers through its colour matrix. `None` for
/// any other file, a sensor of other than three colours, or a white no isotherm reads.
pub fn as_shot(path: &Path) -> Option<White> {
    if route(path) != Branch::Mosaic {
        return None;
    }
    let mut source = Source::open(path, Route::PLATFORM).ok()?;
    let mut s = Session(unsafe { std::mem::zeroed() });
    if unsafe { oni_open(source.name(), source.take(), 1.0, 0, &mut s.0) } != 0 || s.0.colors != 3 {
        return None;
    }
    let cam_xyz: M3 = [0, 1, 2].map(|r| s.0.cam_xyz[r].map(f64::from));
    lit_by(&cam_xyz, [0, 1, 2].map(|c| f64::from(s.0.cam_mul[c])))
}

/// The scene white a camera reads as `1 / mul` on its channels, `cam_xyz` mapping XYZ to them.
pub(crate) fn lit_by(cam_xyz: &M3, mul: [f64; 3]) -> Option<White> {
    if mul.iter().any(|m| !m.is_finite() || *m <= 0.0) {
        return None;
    }
    let xyz = apply(&inverse(cam_xyz)?, mul.map(|m| 1.0 / m));
    if xyz.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let sum: f64 = xyz.iter().sum();
    let w = white::of_uv(white::uv_of_xy(xyz[0] / sum, xyz[1] / sum));
    (w.kelvin.is_finite() && w.tint.is_finite()).then_some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Adobe's matrix for the Nikon Z f, as LibRaw holds it.
    const Z_F: M3 = [[1.1607, -0.4491, -0.0977], [-0.4522, 1.246, 0.2304], [-0.0458, 0.1519, 0.7616]];

    #[test]
    fn the_multipliers_of_a_white_read_back_as_that_white() {
        for (kelvin, tint) in [(5000.0, 0.0), (3200.0, 4.0), (6500.0, -8.0)] {
            let lit = White { kelvin, tint };
            let neutral = apply(&Z_F, white::xyz(white::uv(lit)));
            let back = lit_by(&Z_F, neutral.map(|n| 1.0 / n)).unwrap();
            assert!((back.kelvin - kelvin).abs() < 1.0 && (back.tint - tint).abs() < 0.1, "{back:?}");
            // Twin: the multipliers taken for the neutral itself land thousands of kelvin away.
            let wrong = lit_by(&Z_F, neutral).unwrap_or(White { kelvin: 0.0, tint: 0.0 });
            assert!((wrong.kelvin - kelvin).abs() > 1000.0 || (wrong.tint - tint).abs() > 30.0, "{wrong:?}");
        }
        assert_eq!(lit_by(&Z_F, [2.0, 0.0, 1.5]), None);
    }
}
