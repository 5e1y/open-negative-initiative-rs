//! A slide projected: each layer's dye, read off the light its channel passes, filters a projector's lamp wavelength by
//! wavelength; the screen's light, seen by an eye adapted to the bare film's, in working Rec. 2020.

use super::*;

/// The film's dyes and the lamp as seen, beside the input as two rows of `bands`: each wavelength's cyan, magenta and
/// yellow at unit amount and the base's density, then the lamp's light times the observer's three functions.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    /// The lamp's XYZ, adapted to the bare film's light, to working Rec. 2020, by rows; the bare film reads (1, 1, 1).
    pub to_working: [V3; 3],
    /// The screen's veiling light, a share of its white laid over every tone.
    pub flare: f32,
    pub bands: usize,
}

impl Projection {
    pub fn head(&self) -> [u32; 4] {
        [self.bands as u32, 0, 0, 0]
    }

    pub fn rows(&self) -> Vec<V4> {
        let t = &self.to_working;
        vec![v3(t[0], self.flare), v3(t[1], 0.0), v3(t[2], 0.0)]
    }
}

/// The projected light of the layers' amounts the input's channels code as `10^-amount`, alpha kept.
pub fn projection(s: V4, k: &Projection, table: &[V4]) -> V4 {
    let amounts = map(rgb(s), |_, v| -v.max(1e-12).log10());
    let mut xyz = [0.0f32; 3];
    for i in 0..k.bands {
        let (dye, seen) = (table[i], table[k.bands + i]);
        let density = dye[3] + amounts[0] * dye[0] + amounts[1] * dye[1] + amounts[2] * dye[2];
        let light = 10f32.powf(-density);
        xyz = map(xyz, |c, v| v + light * seen[c]);
    }
    let t = &k.to_working;
    with_alpha([0, 1, 2].map(|r| dot(t[r], xyz) + k.flare), s[3])
}
