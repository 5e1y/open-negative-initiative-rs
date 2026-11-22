//! Chroma decompression: the roll's colour factor and the weights the fused kernel carries.

use super::*;

/// `RollReference.colourFactor`: the spans each channel's points reach over the roll's, on their
/// geometric mean; 1 on no roll.
pub fn colour_factor(s: &PipelineSettings) -> f32 {
    factor_reaching(s, s.print.channel_points(s.mode.is_monochrome()))
}

/// The colour factor with each channel's window reached by `points`, which it answers only under Lighting's colour
/// compensation.
fn factor_reaching(s: &PipelineSettings, points: [(f32, f32); 3]) -> f32 {
    let Some(reference) = s.roll_reference.as_ref() else { return 1.0 };
    let own = s.levels.windows().map(|w| (w.white - w.black).abs());
    let roll: [f32; 3] = std::array::from_fn(|c| {
        (graduation::value(reference.white[c], s.mode) - graduation::value(reference.black[c], s.mode)).abs()
    });
    let reached = |c: usize| if s.print.bypassed || !s.print.colour_compensation { 1.0 } else { (1.0 - points[c].0 - points[c].1) / (1.0 - Print::RESTING_BLACK_POINT) };
    let product: f32 = (0..3).map(|c| own[c].max(1e-4) / roll[c].max(1e-4) * reached(c).max(1e-4)).product();
    product.powf(1.0 / 3.0).clamp(oni_settings::frame::RollReference::FACTOR_RANGE.0, oni_settings::frame::RollReference::FACTOR_RANGE.1)
}

/// `ChromaDecompression.weights`: the windows' colour and the film's own the kernel carries; a colour
/// negative on no measured roll, a digital photo's included, takes its dose.
pub fn decompression_weights(s: &PipelineSettings) -> (f32, f32) {
    let (windows, film) = weighted(s, colour_factor(s));
    // Without Lighting's colour compensation the points stretch the film's share as they stretch the windows'.
    if s.print.colour_compensation || s.print.bypassed { (windows, film) } else { (windows, film / points_reach(s)) }
}

/// The points' reach over each channel's window, on their geometric mean: 1 while they rest.
fn points_reach(s: &PipelineSettings) -> f32 {
    let points = s.print.channel_points(s.mode.is_monochrome());
    let product: f32 = points.iter().map(|(black, white)| ((1.0 - black - white) / (1.0 - Print::RESTING_BLACK_POINT)).max(1e-4)).product();
    product.powf(1.0 / 3.0)
}

fn weighted(s: &PipelineSettings, factor: f32) -> (f32, f32) {
    if s.roll_reference.is_some() {
        return (factor, 0.0);
    }
    let colour_negative = s.mode.invert_flag() > 0.5 && !s.mode.is_monochrome();
    let dose = if colour_negative { s.chroma_decompression.clamp(0.0, 1.0) } else { 1.0 };
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
