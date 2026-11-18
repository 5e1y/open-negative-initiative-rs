//! Chroma decompression: the roll's colour factor and the weights the fused kernel carries.

use super::*;

/// `RollReference.colourFactor`: the spans each channel's points reach over the roll's, on their
/// geometric mean; a digital photo stands on no roll's windows.
pub fn colour_factor(s: &PipelineSettings) -> f32 {
    let Some(reference) = s.roll_reference.as_ref().filter(|_| s.mode.is_film()) else { return 1.0 };
    let own = s.levels.windows().map(|w| (w.white - w.black).abs());
    let roll: [f32; 3] = std::array::from_fn(|c| {
        (graduation::value(reference.white[c], s.mode) - graduation::value(reference.black[c], s.mode)).abs()
    });
    let points = s.print.channel_points(s.mode.is_monochrome());
    let reached = |c: usize| if s.print.bypassed { 1.0 } else { (1.0 - points[c].0 - points[c].1) / (1.0 - Print::RESTING_BLACK_POINT) };
    let product: f32 = (0..3).map(|c| own[c].max(1e-4) / roll[c].max(1e-4) * reached(c).max(1e-4)).product();
    product.powf(1.0 / 3.0).clamp(oni_settings::frame::RollReference::FACTOR_RANGE.0, oni_settings::frame::RollReference::FACTOR_RANGE.1)
}

/// `ChromaDecompression.weights`: the windows' colour and the film's own the kernel carries.
pub fn decompression_weights(s: &PipelineSettings) -> (f32, f32) {
    let factor = colour_factor(s);
    if s.roll_reference.is_some() || !s.mode.is_film() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::frame::RollReference;

    fn on_roll() -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.roll_reference = Some(RollReference { id: oni_settings::Id::new(), stops: [0.0; 3], black: [0.1; 3], white: [0.9; 3], base: None, median: None });
        s.print.black_point = 0.05;
        s.print.white_point = 0.1;
        s
    }

    #[test]
    fn the_colour_factor_reads_each_channels_own_reach() {
        let s = on_roll();
        let own = s.levels.windows().map(|w| (w.white - w.black).abs());
        let reference = s.roll_reference.as_ref().unwrap();
        let roll: [f32; 3] = std::array::from_fn(|c| (graduation::value(reference.white[c], s.mode) - graduation::value(reference.black[c], s.mode)).abs());
        // The former factor, one reach for the three channels: equal to the bit with the channels at rest.
        let (black, white) = s.print.held_points();
        let global = (1.0 - black - white) / (1.0 - Print::RESTING_BLACK_POINT);
        let former = |own: [f32; 3]| {
            let product: f32 = (0..3).map(|c| own[c].max(1e-4) / roll[c].max(1e-4) * global.max(1e-4)).product();
            product.powf(1.0 / 3.0).clamp(RollReference::FACTOR_RANGE.0, RollReference::FACTOR_RANGE.1)
        };
        assert_eq!(colour_factor(&s), former(own));
        // Red's own white point narrows red's reach, and the factor with it.
        let mut red = s.clone();
        red.print.channel_white = [0.2, 0.0, 0.0];
        assert!(colour_factor(&red) < colour_factor(&s) - 1e-3, "{} {}", colour_factor(&red), colour_factor(&s));
        // Twin: the global reach alone misses red's move.
        assert_eq!(former(red.levels.windows().map(|w| (w.white - w.black).abs())), colour_factor(&s));
        // Black and white reads no channel's own.
        let (mut mono, mut mono_red) = (s.clone(), red.clone());
        mono.mode = ConversionMode::Monochrome;
        mono_red.mode = ConversionMode::Monochrome;
        assert_eq!(colour_factor(&mono_red), colour_factor(&mono));
    }
}
