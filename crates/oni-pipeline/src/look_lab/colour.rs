//! The colour stage, in Oklab on the paper's code: hue and chroma by sector, chroma by lightness, then
//! a cast on the greys by lightness.

use super::measure::{encoded, lch, linear, linear_rgb, oklab};
use super::params::{GREYS, LabParams, SECTORS};

/// Chroma past which a sector's hue offset applies whole, and the unit of its twist.
pub const FULL_OFFSET: f64 = 0.03;
pub const TWIST_UNIT: f64 = 0.1;

/// A sector value at hue `h`, interpolated between the two centres around it, all the way round.
pub fn sector(values: &[f64; SECTORS], h: f64) -> f64 {
    let at = h.rem_euclid(360.0) / (360.0 / SECTORS as f64);
    let i = (at as usize).min(SECTORS - 1);
    let t = at - i as f64;
    (1.0 - t) * values[i] + t * values[(i + 1) % SECTORS]
}

/// A grey-cast value at lightness `l`, linear between 0.1 … 0.9 and held past both ends.
pub fn grey(values: &[f64; GREYS], l: f64) -> f64 {
    let at = ((l - 0.1) / 0.2).clamp(0.0, (GREYS - 1) as f64);
    let i = (at as usize).min(GREYS - 2);
    let t = at - i as f64;
    (1.0 - t) * values[i] + t * values[i + 1]
}

/// Display code in, display code out; outside the display's range the encoding keeps its sign.
pub fn colour(p: &LabParams, code: [f64; 3]) -> [f64; 3] {
    let [l, c, h] = lch(oklab(code.map(linear)));
    let turned = h + sector(&p.hue, h) * (c / FULL_OFFSET).min(1.0) + sector(&p.twist, h) * c / TWIST_UNIT;
    let chroma = c * sector(&p.chroma, h) * (1.0 + p.saturation_by_l * (l - 0.5));
    let radians = turned.to_radians();
    let a = chroma * radians.cos() + grey(&p.grey_a, l);
    let b = chroma * radians.sin() + grey(&p.grey_b, l);
    linear_rgb([l, a, b]).map(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look_lab::params::SEED_NEUTRAL;

    #[test]
    fn at_rest_the_stage_returns_its_input() {
        let rest = SEED_NEUTRAL.paper_only();
        let code = [0.62, 0.41, 0.23];
        let out = colour(&rest, code);
        assert!((0..3).all(|k| (out[k] - code[k]).abs() < 1e-6), "{out:?}");
        // Twin: a 10° offset on every sector moves a coloured code.
        let turned = colour(&LabParams { hue: [10.0; SECTORS], ..rest }, code);
        assert!((0..3).any(|k| (turned[k] - code[k]).abs() > 0.01), "{turned:?}");
    }

    #[test]
    fn sectors_wrap_and_greys_hold_their_ends() {
        let mut v = [0.0; SECTORS];
        v[0] = 12.0;
        assert_eq!(sector(&v, 345.0), 6.0, "half way from 330° back to 0°");
        assert_eq!(sector(&v, 15.0), 6.0);
        let g = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!((grey(&g, 0.0), grey(&g, 0.4), grey(&g, 1.0)), (1.0, 2.5, 5.0));
        // Twin: read without the wrap, 345° would find no sector past 330°.
        assert_ne!(v[((345.0f64 / 30.0) as usize).min(SECTORS - 1)], 6.0);
    }
}
