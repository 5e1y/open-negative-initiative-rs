//! Chroma decompression: the roll's colour factor and the weights the fused kernel carries.

use super::*;

/// `RollReference.colourFactor`: the spans the points reach over the roll's, on their geometric mean.
pub fn colour_factor(s: &PipelineSettings) -> f32 {
    let Some(reference) = &s.roll_reference else { return 1.0 };
    let own = s.levels.windows().map(|w| (w.white - w.black).abs());
    let roll: [f32; 3] = std::array::from_fn(|c| {
        (graduation::value(reference.white[c], s.mode) - graduation::value(reference.black[c], s.mode)).abs()
    });
    let (black, white) = s.print.held_points();
    let reached = if s.print.bypassed { 1.0 } else { (1.0 - black - white) / (1.0 - Print::RESTING_BLACK_POINT) };
    let product: f32 = (0..3).map(|c| own[c].max(1e-4) / roll[c].max(1e-4) * reached.max(1e-4)).product();
    product.powf(1.0 / 3.0).clamp(oni_settings::frame::RollReference::FACTOR_RANGE.0, oni_settings::frame::RollReference::FACTOR_RANGE.1)
}

/// `ChromaDecompression.weights`: the windows' colour and the film's own the kernel carries.
pub fn decompression_weights(s: &PipelineSettings) -> (f32, f32) {
    let factor = colour_factor(s);
    if s.roll_reference.is_some() {
        return (factor, 0.0);
    }
    let dose = if s.mode == ConversionMode::Negative { s.chroma_decompression.clamp(0.0, 1.0) } else { 1.0 };
    (dose * factor, 1.0 - dose)
}

/// `ChromaDecompression.signedSpan`.
pub fn signed_span(w: &Levels) -> f32 {
    let span = w.white - w.black;
    if span.abs() < 1e-4 { if span < 0.0 { -1e-4 } else { 1e-4 } } else { span }
}
