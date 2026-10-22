//! The flat-field division (`pipeline.metal`).

use super::*;

/// `flatField(s, f, amount, exponent, perChannel)`: the mask `f` is laid on the frame's own extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatField {
    pub amount: f32,
    pub exponent: V3,
    pub per_channel: f32,
}

impl FlatField {
    pub fn rows(&self) -> Vec<V4> {
        vec![[self.amount, self.per_channel, 0.0, 0.0], v3(self.exponent, 0.0)]
    }
}

pub fn flat_field(s: V4, f: V4, k: &FlatField) -> V4 {
    let m = if k.per_channel > 0.5 { map(rgb(f), |_, x| x.max(0.05)) } else { [f[1].max(0.05); 3] };
    let d = map(m, |c, x| mix(1.0, x, k.amount).powf(k.exponent[c]));
    with_alpha(map(rgb(s), |c, x| x / d[c]), s[3])
}
