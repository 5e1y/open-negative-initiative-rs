//! `PrintCurve`: what Lighting prints, drawn over the output histogram on the Color tab: each
//! filtered channel's curve, the RGB curve, and its mirror, the saturation compensation's inverse.

use super::geometry::{Drawing, Ink, Line, Size, dashed, ds, polyline};
use super::curve::tint;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;

pub const CHANNEL_STEPS: usize = 96;
pub const LINKED_STEPS: usize = 160;

fn at(n: f32, y: f32, size: Size) -> (f64, f64) {
    let x = f64::from(n.powf(Print::DISPLAY_GAMMA));
    (x * size.w, (1.0 - f64::from(y)) * size.h)
}

/// The channels drawn are those whose filter moved; a monochrome print draws none.
pub fn drawing(print: &Print, monochrome: bool, size: Size) -> Drawing {
    let mut lines = Vec::new();
    let channels: &[LevelsChannel] = if monochrome { &[] } else { &LevelsChannel::PER_CHANNEL };
    for &channel in channels {
        if print.channel(channel).is_neutral() {
            continue;
        }
        let trace: Vec<(f64, f64)> = (0..=CHANNEL_STEPS)
            .map(|i| {
                let n = i as f32 / CHANNEL_STEPS as f32;
                at(n, print.output(n, channel, true).clamp(0.0, 1.0), size)
            })
            .collect();
        lines.push(Line::new(polyline(&trace), tint(channel), ds::STROKE_LINE));
    }
    let (linked, mirror) = traces(print, size);
    lines.push(Line::new(polyline(&linked), Ink::Guide, ds::STROKE_LINE));
    lines.push(Line::new(dashed(&mirror, ds::DASH_LENGTH), Ink::Guide, ds::STROKE_LINE));
    Drawing { lines, dots: Vec::new() }
}

/// The RGB curve and the same points with their axes swapped: its exact inverse.
pub fn traces(print: &Print, size: Size) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    (0..=LINKED_STEPS)
        .map(|i| {
            let n = i as f32 / LINKED_STEPS as f32;
            let x = f64::from(n.powf(Print::DISPLAY_GAMMA));
            let y = f64::from(print.output(n, LevelsChannel::Linked, true).clamp(0.0, 1.0));
            ((x * size.w, (1.0 - y) * size.h), (y * size.w, (1.0 - x) * size.h))
        })
        .unzip()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAND: Size = Size::new(340.0, 150.0);

    #[test]
    fn the_print_draws_its_moved_channels_its_curve_and_the_mirror() {
        let rest = Print::default();
        let d = drawing(&rest, false, BAND);
        assert_eq!(d.lines.len(), 2, "no channel moved: the RGB curve and its mirror");
        assert!(d.lines[1].commands.matches('M').count() > 10, "the mirror is dashed");
        let mut warm = Print::default();
        warm.red.exposure = 0.5;
        let d = drawing(&warm, false, BAND);
        assert_eq!(d.lines.len(), 3);
        assert_eq!(d.lines[0].ink, Ink::Red);
        // Twin: a monochrome print draws no channel, whatever its filters.
        assert_eq!(drawing(&warm, true, BAND).lines.len(), 2);
    }

    #[test]
    fn the_mirror_is_the_curve_with_its_axes_swapped() {
        let mut p = Print::default();
        p.exposure = 0.7;
        let (linked, mirror) = traces(&p, BAND);
        for (a, b) in linked.iter().zip(&mirror) {
            let (x, y) = (a.0 / BAND.w, 1.0 - a.1 / BAND.h);
            assert!((b.0 - y * BAND.w).abs() < 1e-9 && (b.1 - (1.0 - x) * BAND.h).abs() < 1e-9);
        }
        // Twin: a lifted exposure bends the curve off the diagonal, so the two are not one line.
        assert!(linked.iter().zip(&mirror).any(|(a, b)| (a.0 - b.0).abs() > 3.0));
    }
}
