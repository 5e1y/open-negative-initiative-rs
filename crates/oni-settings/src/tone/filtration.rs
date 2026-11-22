//! The print's filters read as an enlarger's C, M and Y, and written back as Temperature and Tint.

use super::{CC_PER_STOP, Print, V3};

impl Print {
    /// The filters as an enlarger's C, M and Y in CC, each its own band's, red's C: the least at none, the neutral
    /// density the print's held luma takes back, so they read Temperature and Tint alone.
    pub fn filtration(&self) -> V3 {
        let cc = self.filters().map(|f| CC_PER_STOP * f);
        let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
        cc.map(|v| v - least)
    }

    /// The moves of Temperature and Tint that bring `filtration` to `cc`: the filters' differences, which a neutral
    /// density leaves alone.
    pub fn filtration_moves(&self, cc: V3) -> (f32, f32) {
        let now = self.filtration();
        let d: V3 = std::array::from_fn(|c| cc[c] - now[c]);
        ((d[0] - d[2]) / CC_PER_STOP, ((d[0] + d[2]) / 2.0 - d[1]) / CC_PER_STOP)
    }
}
