//! The small blocks of the chain: gains, the dye separation's family and doses, chroma
//! decompression's doses, the vignette, texture, sharpening, chroma denoise, the paper's gains.

use crate::axis::pipeline;
use serde::{Deserialize, Serialize};

/// Stage 4: per-channel gains, in stops, `gain = 2^s`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Gains {
    pub stops: [f32; 3],
}

impl Gains {
    pub const BASE: f32 = 0.0;
    pub const RANGE: (f32, f32) = (-3.0, 3.0);
}

/// The dye separation's doses; the dose itself is a field of the settings.
pub mod separation {
    pub const RANGE: (f32, f32) = (0.0, 2.0);
    /// The literature's dose, the one the measured scanner separates at.
    pub const BASE: f32 = 0.6;
    /// The rest a frame written before the literature's dose carries.
    pub const PREVIOUS_BASE: f32 = 0.18;
    /// The rest a frame written before the dose was the hand's keeps until a key balances it.
    pub const FORMER_REST: f32 = 0.67;
    /// The rest before `FORMER_REST`, recognised on the way in.
    pub const FORMER_BASE: f32 = 1.0;
}

/// A dye set: which colorant spills into which channel. The process names it, never the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeparationFamily {
    #[serde(rename = "kodakC41")]
    KodakC41,
    #[serde(rename = "fujiC41")]
    FujiC41,
    #[serde(rename = "genericC41")]
    GenericC41,
    #[serde(rename = "ecn2")]
    Ecn2,
    #[serde(rename = "e6")]
    E6,
}

impl SeparationFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::KodakC41 => "Kodak",
            Self::FujiC41 => "Fuji",
            Self::GenericC41 => "Generic",
            Self::Ecn2 => "ECN-2",
            Self::E6 => "E-6",
        }
    }

    /// The family a mode rests on when the hand named none; monochrome has nothing to unmix.
    pub fn resting(mode: crate::modes::ConversionMode) -> Option<Self> {
        if mode.is_monochrome() {
            return None;
        }
        Some(if mode.invert_flag() > 0.5 { Self::GenericC41 } else { Self::E6 })
    }
}

/// How much colour the windows decompress with the tones.
pub mod chroma_decompression {
    /// Halfway: every balance key writes it unless the hand dialled another.
    pub const BASE: f32 = 0.5;
    /// The key's former dose; a frame still on it, not the hand's, moves to `BASE`.
    pub const FORMER_KEY_REST: f32 = 0.0;
    /// The dose a frame written before the rest moved renders at until a key balances it.
    pub const FORMER_REST: f32 = 1.0;
}

/// The automatic balance's resting share of the full correction.
pub mod filter_balance {
    pub const RESTING_STRENGTH: f32 = 0.67;
    pub const STRENGTH_RANGE: (f32, f32) = (0.0, 1.0);
}

/// A radial darkening of the print, in stops; 0 is the identity.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vignette {
    pub stops: f32,
}

impl Vignette {
    pub const RANGE: (f32, f32) = (-2.0, 2.0);
}

/// Mid-frequency local contrast on the luma.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Texture {
    pub amount: f32,
}

impl Texture {
    pub const RANGE: (f32, f32) = (-1.0, 2.0);
}

/// Stage 10: unsharp mask on the luma. Its rest is on, not off: neutralise with `is_neutral`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sharpen {
    /// Blur radius in full-resolution pixels.
    pub radius: f32,
    pub amount: f32,
    pub halo: f32,
}

impl Default for Sharpen {
    fn default() -> Self {
        Sharpen { radius: Self::RADIUS_ANCHOR, amount: 1.0, halo: 0.5 }
    }
}

impl Sharpen {
    pub const RADIUS_ANCHOR: f32 = 2.092;
    pub const AMOUNT_RANGE: (f32, f32) = (0.0, 3.0);
    pub const HALO_RANGE: (f32, f32) = (0.0, 1.0);
    /// The fraction of the largest side an untouched block carried before radii were pixels.
    pub const FORMER_REST: f32 = 0.0001;
    /// The amount the stage rested at before the decoder's own sharpening was pinned off.
    pub const FORMER_AMOUNT: f32 = 0.67;

    /// From the floor a fitted preview leaves inert to past the widest dose the films pay for.
    pub fn radius_range() -> (f32, f32) {
        (pipeline::preview_visible_radius() as f32, 8.0)
    }

    pub fn is_neutral(&self) -> bool {
        self.amount <= 0.0 || self.radius <= 0.0
    }
}

/// Chroma-only denoise after the inversion. Its rest is on; `OFF` is off.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChromaDenoise {
    pub radius: f32,
    pub force: f32,
}

impl Default for ChromaDenoise {
    fn default() -> Self {
        ChromaDenoise { radius: 4.0, force: 0.2 }
    }
}

impl ChromaDenoise {
    pub const OFF: ChromaDenoise = ChromaDenoise { radius: 0.0, force: 0.0 };
    pub const RADIUS_RANGE: (f32, f32) = (0.0, 6.0);
    /// The pairs an untouched block carried, in the fraction of the largest side they were written in.
    pub const FORMER_RESTS: [(f32, f32); 2] = [(0.0001, 0.33), (0.00133, 0.33)];

    pub fn is_neutral(&self) -> bool {
        self.force <= 0.0 || self.radius <= 0.0
    }

    pub fn retired(radius: f32, force: f32) -> bool {
        Self::FORMER_RESTS.iter().any(|&(r, f)| r == radius && f == force)
    }
}

/// Gains answering the paper the print is proofed on; inert on any other working space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PaperCorrection {
    pub stops: [f32; 3],
}

impl PaperCorrection {
    /// A sixth of the roll's gains' track: a paper's tint is a fraction of a stop.
    pub const RANGE: (f32, f32) = (-0.5, 0.5);

    /// The grey the gains are solved on; a paper's tint scales every level alike.
    pub const PROBE: f32 = 0.18;

    pub fn is_neutral(&self) -> bool {
        self.stops == [0.0; 3]
    }

    pub fn gains(&self) -> [f32; 3] {
        self.stops.map(f32::exp2)
    }

    /// The gains on interleaved RGBA pixels, alpha untouched.
    pub fn apply(&self, pixels: &mut [f32]) {
        if self.is_neutral() {
            return;
        }
        let gain = self.gains();
        for p in pixels.chunks_exact_mut(4) {
            for c in 0..3 {
                p[c] *= gain[c];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_gains_are_powers_of_two_and_neutral_moves_nothing() {
        let mut px = vec![0.2, 0.2, 0.2, 1.0];
        PaperCorrection { stops: [0.0, 0.0, -0.5] }.apply(&mut px);
        assert!((px[2] - 0.2 * 0.5f32.sqrt()).abs() < 1e-6 && px[0] == 0.2);
        let mut rest = vec![0.2, 0.2, 0.2, 1.0];
        PaperCorrection::default().apply(&mut rest);
        assert_eq!(rest, vec![0.2, 0.2, 0.2, 1.0]);
        // Twin: a gain read as a linear factor would halve the blue.
        assert!((px[2] - 0.2 * 0.5).abs() > 0.03);
    }

    #[test]
    fn the_paper_stops_serialise_as_the_swift_array() {
        let json = serde_json::to_value(PaperCorrection { stops: [0.25, 0.0, -0.5] }).unwrap();
        assert_eq!(json, serde_json::json!({ "stops": [0.25, 0.0, -0.5] }));
        let back: PaperCorrection = serde_json::from_str(r#"{"stops":[0.1,0.2,0.3]}"#).unwrap();
        assert_eq!(back.stops, [0.1, 0.2, 0.3]);
        // Twin: a renamed field is refused.
        assert!(serde_json::from_str::<PaperCorrection>(r#"{"gains":[0.1,0.2,0.3]}"#).is_err());
    }
}
