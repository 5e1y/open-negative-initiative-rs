//! The finishing stages' arguments: the curves' tables, the saturation curves (the hand's, the points',
//! Contrast's, Chrome's), the colour mixer's packed doses, the vignette, the dosed detail radii and Texture's band.

use oni_render::{Curves, Detail, LUT_SIZE, SaturationCurve as SaturationArgs, Spectrogram as SpectrogramArgs};
use oni_settings::blocks::{ChromaDenoise, Sharpen, Texture};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet, SaturationCurve};
use oni_settings::spectrogram::{Band, Spectrogram};
use oni_settings::{ConversionMode, PipelineSettings};
use oni_settings::tone::{Print, Tone, contrast};

/// `CurveSet.rgbLUT` and `lumaLUT` with the kernel's flags; `None` when all five curves rest.
pub fn curves(set: &CurveSet) -> Option<Curves> {
    if set.is_neutral() {
        return None;
    }
    let table = |c: &Curve| {
        let e = c.evaluated();
        (0..LUT_SIZE).map(|i| e.value(i as f64 / (LUT_SIZE - 1) as f64) as f32).collect::<Vec<f32>>()
    };
    let mut linked = table(&set.linked);
    // Contrast rides on the linked table, so one transfer function per channel reaches the GPU.
    if set.contrast != 0.0 {
        linked = linked.into_iter().map(|v| contrast::apply(v, set.contrast, 0.5)).collect();
    }
    let (red, green, blue, luma) = (table(&set.red), table(&set.green), table(&set.blue), table(&set.luma));
    let channels = !set.red.is_neutral() || !set.green.is_neutral() || !set.blue.is_neutral();
    Some(Curves {
        rgb_lut: (0..LUT_SIZE).map(|i| [linked[i], red[i], green[i], blue[i]]).collect(),
        luma_lut: luma.iter().map(|&v| [v, 0.0, 0.0, 1.0]).collect(),
        bent: [
            f32::from(u8::from(channels)),
            f32::from(u8::from(!set.linked.is_neutral() || set.contrast != 0.0)),
            f32::from(u8::from(!set.luma.is_neutral())),
        ],
    })
}

/// `SaturationCurve.arguments`; `None` when the curve rests.
pub fn saturation(curve: &SaturationCurve) -> Option<SaturationArgs> {
    if curve.is_neutral() || curve.points.len() != 5 {
        return None;
    }
    let p = &curve.points;
    let m = Curve { points: p.clone() }.second_derivatives();
    Some(SaturationArgs {
        knots: [p[1].x as f32, p[2].x as f32, p[3].x as f32],
        lows: [p[0].y as f32, p[1].y as f32, p[2].y as f32, p[3].y as f32],
        high: p[4].y as f32,
        bends: [m[1] as f32, m[2] as f32, m[3] as f32],
    })
}

const SAUCE_FULL: [(f64, f64); 5] = [(0.0, 0.0), (0.103, 0.284), (0.368, 0.537), (0.799, 0.296), (1.0, 0.0)];
const SAUCE_DOUBLE: [(f64, f64); 5] = [(0.0, 0.0), (0.056, 0.222), (0.284, 0.522), (0.833, 0.062), (1.0, 0.0)];
/// `SaturationCurve.sauceRange`'s top.
pub const SAUCE_TOP: f32 = 1.5;

/// `SaturationCurve.sauce`: the full curve rises from neutral up to 1, then travels toward the double.
pub fn sauce(amount: f32) -> SaturationCurve {
    let a = f64::from(amount.clamp(0.0, 2.0));
    let points = SAUCE_FULL
        .iter()
        .zip(SAUCE_DOUBLE)
        .map(|(&(fx, fy), (dx, dy))| {
            if a <= 1.0 {
                Point::new(fx, 0.5 + a * (fy - 0.5))
            } else {
                Point::new(fx + (a - 1.0) * (dx - fx), fy + (a - 1.0) * (dy - fy))
            }
        })
        .collect();
    SaturationCurve { points }
}

/// `Print.pointsCurve`: the black and white points' saturation, a line from the black's to the white's.
/// A positive's white point moves its print the other way, so its saturation is mirrored (owner's call); none without
/// Lighting's colour compensation.
pub fn points_curve(print: &Print, mode: ConversionMode) -> SaturationCurve {
    if print.bypassed || !print.colour_compensation || (print.black_point == 0.0 && print.white_point == 0.0) {
        return SaturationCurve::default();
    }
    let (black, white) = print.held_points();
    let sense = if mode.reads_positive() { -1.0 } else { 1.0 };
    let white = sense * white;
    let low = f64::from(((1.0 - Print::POINT_SATURATION_GAIN * black) / 2.0).clamp(0.0, 1.0));
    let high = f64::from(((1.0 + Print::POINT_SATURATION_GAIN * white) / 2.0).clamp(0.0, 1.0));
    let points = SaturationCurve::resting().into_iter().map(|p| Point::new(p.x, low + p.x * (high - low))).collect();
    SaturationCurve { points }
}

/// The saturation ONI's print lays by itself, the points' then Contrast's: the render's passes and the plot's guide; a
/// paper's print lays none, its colour its own.
pub fn print_saturation(s: &PipelineSettings) -> [SaturationCurve; 2] {
    if s.is_monochrome() || on_paper(s) {
        return [SaturationCurve::default(), SaturationCurve::default()];
    }
    [points_curve(&s.print, s.mode), contrast_curve(s)]
}

/// How far either side of a window value its slope is read, as a share of that value.
const CONTRAST_REACH: f32 = 0.01;

/// Contrast's saturation on ONI's print: at each tone it shows, the inverse of the colour Contrast's slope gives it there,
/// so every tone keeps the colour of the print without Contrast; drawn on the Saturation Curve's plot as the render lays
/// it; none on a paper or without Lighting's colour compensation.
pub fn contrast_curve(s: &PipelineSettings) -> SaturationCurve {
    let print = &s.print;
    if print.bypassed || !print.colour_compensation || print.contrast == 0.0 || s.is_monochrome() || on_paper(s) {
        return SaturationCurve::default();
    }
    let stops = print.on_curve(Print::resting_exposure(s.mode)).exposure;
    let bent = print.master();
    let points = SaturationCurve::resting()
        .into_iter()
        .map(|p| Point::new(p.x, f64::from((0.5 * held(&bent, stops, p.x as f32)).clamp(0.0, 1.0))))
        .collect();
    SaturationCurve { points }
}

fn on_paper(s: &PipelineSettings) -> bool {
    s.paper().is_some() && !s.print.bypassed
}

/// The colour kept at printed tone `t`: the slope without Contrast over the slope with it, on logarithmic light; whole
/// at the black, where every curve leaves straight.
fn held(bent: &Tone, stops: f32, t: f32) -> f32 {
    let n = unprinted(bent, stops, t);
    let (low, high) = (n * (1.0 - CONTRAST_REACH), (n * (1.0 + CONTRAST_REACH)).min(1.0));
    let slope = |tone: &Tone| (tone.shaped(high, stops) / tone.shaped(low, stops)).ln();
    let (with, without) = (slope(bent), slope(&Tone { contrast: 0.0, ..*bent }));
    if t <= 0.0 || !(with > 0.0 && without.is_finite()) { 1.0 } else { without / with }
}

/// The window value `tone` prints at `t`: the RGB curve read backwards, as the print plot's mirror draws it.
fn unprinted(tone: &Tone, stops: f32, t: f32) -> f32 {
    let (mut low, mut high) = (0.0f32, 1.0f32);
    for _ in 0..32 {
        let mid = 0.5 * (low + high);
        if tone.shaped(mid, stops) < t { low = mid } else { high = mid }
    }
    0.5 * (low + high)
}

fn separation(a: f32, b: f32) -> f32 {
    Spectrogram::separation(a, b)
}

/// `Spectrogram.rawWeight`: a band's weight at a hue before the eight are normalised.
fn raw_weight(band: Band, hue: f32) -> f32 {
    let i = band.index();
    let mut d = hue - band.centre();
    if d > 180.0 {
        d -= 360.0;
    }
    if d < -180.0 {
        d += 360.0;
    }
    let span = if d >= 0.0 {
        separation(band.centre(), Band::ALL[(i + 1) % 8].centre())
    } else {
        separation(Band::ALL[(i + 7) % 8].centre(), band.centre())
    } * Spectrogram::BAND_SPREAD;
    let raw = (1.0 - d.abs() / span).max(0.0);
    raw * raw * (3.0 - 2.0 * raw)
}

/// `Spectrogram.authority`: how much of itself a band owns at its centre once the eight are normalised.
pub fn authority(band: Band) -> f32 {
    let total: f32 = Band::ALL.iter().map(|b| raw_weight(*b, band.centre())).sum();
    let mine = raw_weight(band, band.centre());
    if total > 0.0 && mine > 0.0 { mine / total } else { 1.0 }
}

/// The mixer's doses as the kernel takes them, each divided by its band's authority.
pub fn spectrogram(mixer: &Spectrogram) -> Option<SpectrogramArgs> {
    if mixer.is_neutral() {
        return None;
    }
    let packed = |v: &[f32; 8], from: usize| -> [f32; 4] { std::array::from_fn(|k| v[from + k] / authority(Band::ALL[from + k])) };
    Some(SpectrogramArgs {
        hue_low: packed(&mixer.hue, 0),
        hue_high: packed(&mixer.hue, 4),
        sat_low: packed(&mixer.saturation, 0),
        sat_high: packed(&mixer.saturation, 4),
        luma_low: packed(&mixer.luma, 0),
        luma_high: packed(&mixer.luma, 4),
    })
}

/// `Pipeline.applyDetail`'s arguments at `scale`, the current width over the full-resolution one.
pub fn detail(chroma: Option<&ChromaDenoise>, sharpen: Option<&Sharpen>, scale: f64) -> Option<Detail> {
    let chroma = chroma.filter(|c| !c.is_neutral()).map(|c| (f64::from(c.radius) * scale, c.force));
    let sharpen = sharpen.filter(|s| !s.is_neutral()).map(|s| (f64::from(s.radius) * scale, s.amount, s.halo));
    Detail::dosed(chroma, sharpen)
}

/// `Texture.narrow`, `wide`, `peakGain`, `edgeContrast` and `minimumBase`.
pub mod texture {
    pub const NARROW: f64 = 0.0002;
    pub const WIDE: f64 = 0.0008;
    pub const PEAK_GAIN: f32 = 0.82;
    pub const EDGE_CONTRAST: f32 = 0.125;
    pub const MINIMUM_BASE: f64 = 0.6;
}

/// Texture's two radii on the reference side and its gain; `None` when it rests or cannot reach.
pub fn texture(t: &Texture, reference: f64) -> Option<(f64, f64, f32)> {
    if t.amount == 0.0 {
        return None;
    }
    let (narrow, wide) = (texture::NARROW * reference, texture::WIDE * reference);
    (wide > texture::MINIMUM_BASE).then_some((narrow, wide, t.amount / texture::PEAK_GAIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_rides_on_the_linked_table_and_a_resting_set_bakes_nothing() {
        assert!(curves(&CurveSet::default()).is_none());
        let set = CurveSet { contrast: 0.4, ..CurveSet::default() };
        let k = curves(&set).unwrap();
        assert_eq!(k.bent, [0.0, 1.0, 0.0]);
        let quarter = k.rgb_lut[LUT_SIZE / 4];
        assert!(quarter[0] < 0.24 && (quarter[1] - 0.25).abs() < 1e-3, "{quarter:?}");
        // Twin: the same set without contrast keeps the diagonal on the linked table.
        let flat = curves(&CurveSet { red: Curve { points: vec![Point::new(0.0, 0.1), Point::new(1.0, 1.0)] }, ..CurveSet::default() }).unwrap();
        assert!((flat.rgb_lut[LUT_SIZE / 4][0] - 0.25).abs() < 1e-3 && flat.bent == [1.0, 0.0, 0.0]);
    }

    #[test]
    fn the_sauce_is_neutral_at_zero_and_continuous_at_one() {
        assert!(sauce(0.0).is_neutral());
        let below = sauce(0.9999);
        let above = sauce(1.0001);
        for (a, b) in below.points.iter().zip(&above.points) {
            assert!((a.y - b.y).abs() < 1e-3 && (a.x - b.x).abs() < 1e-3);
        }
        // Twin: halfway to the double curve sits away from the full one.
        assert!((sauce(1.5).points[3].y - sauce(1.0).points[3].y).abs() > 0.05);
    }

    #[test]
    fn every_band_owns_part_of_its_centre_and_the_packing_divides_by_it() {
        for band in Band::ALL {
            let a = authority(band);
            assert!(a > 0.0 && a <= 1.0, "{band:?}: {a}");
        }
        let mut mixer = Spectrogram::default();
        mixer.saturation[5] = 0.5;
        let k = spectrogram(&mixer).unwrap();
        assert!((k.sat_high[1] * authority(Band::Blue) - 0.5).abs() < 1e-6);
        // Twin: the bare dose would not reach grey at full travel.
        assert!((k.sat_high[1] - 0.5).abs() > 0.01);
    }

    #[test]
    fn the_points_lay_a_line_of_saturation() {
        let neg = ConversionMode::Negative;
        assert!(points_curve(&Print::default(), neg).is_neutral());
        let pointed = Print { black_point: 0.1, white_point: 0.1, ..Print::default() };
        let p = points_curve(&pointed, neg);
        assert!(p.points[0].y < 0.5 && p.points[4].y > 0.5);
        let mid = (p.points[0].y + p.points[4].y) / 2.0;
        assert!((p.points[2].y - mid).abs() < 1e-9);
        // A slide's white point mirrors the negative's, its black point stays as it is.
        let slide = points_curve(&pointed, ConversionMode::Positive);
        assert!((slide.points[0].y - p.points[0].y).abs() < 1e-9 && (slide.points[4].y - (1.0 - p.points[4].y)).abs() < 1e-9);
    }
}
