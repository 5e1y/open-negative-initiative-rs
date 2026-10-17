//! Paper Correction's colour side: where its gains are laid, and the automatic key that solves them
//! on a paper's profile. The setting itself is `oni_settings::blocks::PaperCorrection`.

pub use oni_settings::blocks::PaperCorrection;

use crate::legs::{self, Back, Legs};
use crate::profile::OutputProfile;
use crate::space::Space;

/// The gains on their way into `space`: inert on any space naming no paper.
pub fn laid(correction: &PaperCorrection, pixels: &mut [f32], space: &Space) {
    if space.paper().is_some() {
        correction.apply(pixels);
    }
}

/// The gains under which a grey proofs grey on this paper, the strongest held at 0 so nothing
/// clips. Reads the profile alone, never a picture; `None` for a profile naming no paper.
pub fn automatic(profile: &OutputProfile) -> Option<PaperCorrection> {
    let data = std::fs::read(&profile.path).ok()?;
    if !OutputProfile::is_printer(&data) {
        return None;
    }
    let legs = legs::legs(&data)?;
    let back = legs::legs_back(&data)?;
    let mut gain = [1.0f32; 3];
    for _ in 0..16 {
        let shown = proofed(gain.map(|g| PaperCorrection::PROBE * g), &legs, &back)?;
        // `all` and not a fold of `min`, which skips a NaN channel.
        if !shown.iter().all(|v| *v > 0.0) {
            return None;
        }
        let mean = shown.iter().sum::<f32>() / 3.0;
        for c in 0..3 {
            gain[c] *= mean / shown[c];
        }
        let strongest = gain.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        gain = gain.map(|g| g / strongest);
    }
    let (low, high) = PaperCorrection::RANGE;
    Some(PaperCorrection { stops: gain.map(|g| g.log2().clamp(low, high)) })
}

/// One colour through the export's legs and back, the paper simulated.
pub fn proofed(colour: [f32; 3], legs: &Legs, back: &Back) -> Option<[f32; 3]> {
    let device = legs::run(legs, &[colour[0], colour[1], colour[2], 1.0])?;
    let shown = legs::run_back(back, &device, None)?;
    Some([shown[0], shown[1], shown[2]])
}
