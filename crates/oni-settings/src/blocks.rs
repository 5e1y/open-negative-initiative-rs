//! The small blocks of the chain: gains, chroma decompression's doses, the vignette, the grain, the halation, texture, sharpening, chroma denoise, the paper's gains.

use crate::axis::pipeline;
use serde::{Deserialize, Serialize};

mod grain;
mod grain_colour;

pub use grain::{Grain, GrainCurve, Strength};
pub use grain_colour::GrainColour;

/// Stage 4: per-channel gains, in stops, `gain = 2^s`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gains {
    pub stops: [f32; 3],
}

impl Gains {
    pub const BASE: f32 = 0.0;
    pub const RANGE: (f32, f32) = (-3.0, 3.0);
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
#[serde(default)]
pub struct Vignette {
    pub stops: f32,
}

impl Vignette {
    pub const RANGE: (f32, f32) = (-2.0, 2.0);
}

/// The light a film's base sends back into its layers, before their development: its dose, the share of a layer's
/// exposure sent back, and the glow's deviation on the film, in micrometres of a 135 frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Halation {
    pub amount: f32,
    pub size: f32,
}

impl Default for Halation {
    fn default() -> Self {
        Halation { amount: 0.0, size: Halation::SIZE_REST }
    }
}

impl Halation {
    /// To a quarter of a layer's exposure, well past a film with no anti-halation layer.
    pub const AMOUNT_RANGE: (f32, f32) = (0.0, 0.25);
    /// From a fifth of a 135 film's base to twice it.
    pub const SIZE_RANGE: (f32, f32) = (25.0, 250.0);
    /// A 135 film's triacetate base, 0.125 mm: the glow's deviation is about the thickness the light crosses twice.
    pub const SIZE_REST: f32 = 125.0;

    /// The glow's deviation in pixels of a scan whose long side is `long` pixels.
    pub fn pixels(&self, long: f64) -> f64 {
        Grain::pixels(self.size, long)
    }
}

/// A film's own blur, the light its emulsion spreads before its development: its deviation on the film, in micrometres
/// of a 135 frame, none at rest.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Bloom {
    pub size: f32,
}

impl Bloom {
    /// To six times the softest sheet's, a 20 cycles/mm film's.
    pub const RANGE: (f32, f32) = (0.0, 20.0);

    /// The blur's deviation in pixels of a scan whose long side is `long` pixels.
    pub fn pixels(&self, long: f64) -> f64 {
        Grain::pixels(self.size, long)
    }
}

/// Mid-frequency local contrast on the luma.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Texture {
    pub amount: f32,
}

impl Texture {
    pub const RANGE: (f32, f32) = (-1.0, 2.0);
}

/// Stage 10: unsharp mask on the luma. Its rest is on, not off: neutralise with `is_neutral`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
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
        self.stops.map(libm::exp2f)
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
    fn the_paper_stops_serialise_as_an_array() {
        let json = serde_json::to_value(PaperCorrection { stops: [0.25, 0.0, -0.5] }).unwrap();
        assert_eq!(json, serde_json::json!({ "stops": [0.25, 0.0, -0.5] }));
        let back: PaperCorrection = serde_json::from_str(r#"{"stops":[0.1,0.2,0.3]}"#).unwrap();
        assert_eq!(back.stops, [0.1, 0.2, 0.3]);
        // Twin: a renamed field is not the stops: they read their rest.
        let renamed: PaperCorrection = serde_json::from_str(r#"{"gains":[0.1,0.2,0.3]}"#).unwrap();
        assert_eq!(renamed, PaperCorrection::default());
    }
}
