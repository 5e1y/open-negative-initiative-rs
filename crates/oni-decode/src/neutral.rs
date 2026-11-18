//! The white a RAW's channels are balanced on: the Swift app's 5000 K for every film but a slide.

/// A slide's RAW is balanced at 5500 K, tint 0 (owner's call), so a positive decodes as a plain RAW.
pub const SLIDE_KELVIN: f64 = 5500.0;

/// Robertson's nodes either side of 5500 K, mired then (u, v); a temperature between them is read
/// on a straight line in mired, as DNG temperatures are.
const ROBERTSON: [(f64, f64, f64); 2] = [(175.0, 0.20525, 0.31647), (200.0, 0.21142, 0.32312)];

/// The white a RAW's channels are balanced on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Neutral {
    /// The Swift app's 5000 K, every negative's.
    #[default]
    Swift,
    /// `SLIDE_KELVIN`, a positive's.
    Slide,
}

impl Neutral {
    pub(crate) fn uv(self) -> (f64, f64) {
        match self {
            Neutral::Swift => crate::NEUTRAL_UV,
            Neutral::Slide => {
                let [(m0, u0, v0), (m1, u1, v1)] = ROBERTSON;
                let f = (1e6 / SLIDE_KELVIN - m0) / (m1 - m0);
                (u0 + f * (u1 - u0), v0 + f * (v1 - v0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::white::{self, White};

    #[test]
    fn the_settings_white_reads_the_decodes_two_neutrals() {
        assert_eq!(white::uv(White { kelvin: 5000.0, tint: 0.0 }), Neutral::Swift.uv());
        assert_eq!(white::MOSAIC_WHITE, Neutral::Swift.uv());
        let (slide, own) = (white::uv(White { kelvin: SLIDE_KELVIN as f32, tint: 0.0 }), Neutral::Slide.uv());
        assert!((slide.0 - own.0).abs() < 1e-15 && (slide.1 - own.1).abs() < 1e-15, "{slide:?} {own:?}");
        // Twin: 10 K away is another white.
        assert_ne!(white::uv(White { kelvin: 5010.0, tint: 0.0 }), Neutral::Swift.uv());
    }
}
