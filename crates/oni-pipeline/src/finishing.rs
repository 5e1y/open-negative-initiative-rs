//! The finishing stages' arguments: the curves' tables, the saturation curves (the hand's, the points',
//! Chrome's), the colour mixer's packed doses, the vignette, the dosed detail radii and Texture's band.

use oni_render::{Curves, Detail, LUT_SIZE, SaturationCurve as SaturationArgs, Spectrogram as SpectrogramArgs};
use oni_settings::blocks::{ChromaDenoise, Sharpen, Texture};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet, SaturationCurve};
use oni_settings::levels::{Levels, PictureLevels};
use oni_settings::spectrogram::{Band, Spectrogram};
use oni_settings::ConversionMode;
use oni_settings::tone::{Print, contrast};

/// `CurveSet.rgbLUT` and `lumaLUT` with the kernel's flags, a digital photo's `levels` laid in front of
/// the curves: each channel's table reads its own window after RGB's, and the luma table Luma's, the
/// kernel's last step, so it acts after the RGB curves. `None` when the five curves and the levels rest.
pub fn curves(set: &CurveSet, levels: Option<&PictureLevels>) -> Option<Curves> {
    let levels = levels.filter(|l| !l.is_neutral());
    if set.is_neutral() && levels.is_none() {
        return None;
    }
    let rest = PictureLevels::default();
    let l = levels.unwrap_or(&rest);
    // A table read past its own 1 goes on at slope 1, as the kernel reads it.
    let table = |c: &Curve, level: &dyn Fn(f64) -> f64| {
        let e = c.evaluated();
        let at_one = e.value(1.0);
        let read = |x: f64| if x > 1.0 { at_one + x - 1.0 } else { e.value(x) };
        (0..LUT_SIZE).map(|i| read(level(i as f64 / (LUT_SIZE - 1) as f64)) as f32).collect::<Vec<f32>>()
    };
    let bare = |x: f64| x;
    let mut linked = table(&set.linked, &bare);
    // Contrast rides on the linked table, so one transfer function per channel reaches the GPU.
    if set.contrast != 0.0 {
        linked = linked.into_iter().map(|v| contrast::apply(v, set.contrast, 0.5)).collect();
    }
    let channel = |c: &Curve, own: &Levels| table(c, &|x| l.linked.transfer(own.transfer(x)));
    let (red, green, blue) = (channel(&set.red, &l.red), channel(&set.green, &l.green), channel(&set.blue, &l.blue));
    let luma = table(&set.luma, &|x| l.luma.transfer(x));
    let windowed = [l.linked, l.red, l.green, l.blue].iter().any(|w| *w != Levels::default());
    let channels = windowed || !set.red.is_neutral() || !set.green.is_neutral() || !set.blue.is_neutral();
    let end = |own: &Levels| (l.linked.end_slope() * own.end_slope()) as f32;
    Some(Curves {
        rgb_lut: (0..LUT_SIZE).map(|i| [linked[i], red[i], green[i], blue[i]]).collect(),
        luma_lut: luma.iter().map(|&v| [v, 0.0, 0.0, 1.0]).collect(),
        bent: [
            f32::from(u8::from(channels)),
            f32::from(u8::from(!set.linked.is_neutral() || set.contrast != 0.0)),
            f32::from(u8::from(!set.luma.is_neutral() || l.luma != Levels::default())),
        ],
        ends: [1.0, end(&l.red), end(&l.green), end(&l.blue)],
        luma_end: l.luma.end_slope() as f32,
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
/// A positive's white point moves its print the other way, so its saturation is mirrored (owner's call).
pub fn points_curve(print: &Print, mode: ConversionMode) -> SaturationCurve {
    if print.bypassed || (print.black_point == 0.0 && print.white_point == 0.0) {
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
        assert!(curves(&CurveSet::default(), None).is_none());
        let set = CurveSet { contrast: 0.4, ..CurveSet::default() };
        let k = curves(&set, None).unwrap();
        assert_eq!(k.bent, [0.0, 1.0, 0.0]);
        let quarter = k.rgb_lut[LUT_SIZE / 4];
        assert!(quarter[0] < 0.24 && (quarter[1] - 0.25).abs() < 1e-3, "{quarter:?}");
        // Twin: the same set without contrast keeps the diagonal on the linked table.
        let flat = curves(&CurveSet { red: Curve { points: vec![Point::new(0.0, 0.1), Point::new(1.0, 1.0)] }, ..CurveSet::default() }, None).unwrap();
        assert!((flat.rgb_lut[LUT_SIZE / 4][0] - 0.25).abs() < 1e-3 && flat.bent == [1.0, 0.0, 0.0]);
    }

    fn bent() -> CurveSet {
        let red = Curve { points: vec![Point::new(0.0, 0.1), Point::new(1.0, 1.0)] };
        let luma = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.4, 0.55), Point::new(1.0, 1.0)] };
        CurveSet { contrast: 0.3, red, luma, ..CurveSet::default() }
    }

    fn bits(k: &Option<Curves>) -> Option<(Vec<[u32; 4]>, Vec<[u32; 4]>, [u32; 3], [u32; 4], u32)> {
        let b = |v: &[f32; 4]| v.map(f32::to_bits);
        k.as_ref().map(|k| (k.rgb_lut.iter().map(b).collect(), k.luma_lut.iter().map(b).collect(), k.bent.map(f32::to_bits), k.ends.map(f32::to_bits), k.luma_end.to_bits()))
    }

    #[test]
    fn resting_levels_bake_the_very_tables_the_curves_alone_do() {
        let rest = PictureLevels::default();
        assert!(curves(&CurveSet::default(), Some(&rest)).is_none(), "nothing to bake");
        for set in [CurveSet { contrast: 0.4, ..CurveSet::default() }, bent()] {
            assert_eq!(bits(&curves(&set, Some(&rest))), bits(&curves(&set, None)), "bit for bit");
        }
        // Twin: one handle moved bakes another RGB table.
        let mut moved = rest;
        moved.red.set_white(0.8);
        assert_ne!(curves(&bent(), Some(&moved)).unwrap().rgb_lut, curves(&bent(), None).unwrap().rgb_lut);
        assert_eq!(curves(&CurveSet::default(), Some(&moved)).unwrap().bent, [1.0, 0.0, 0.0], "the levels alone bend the channels");
    }

    /// One pixel through the CPU kernel on these levels and no curve.
    fn through(levels: &PictureLevels, p: [f32; 3]) -> [f32; 3] {
        let k = curves(&CurveSet::default(), Some(levels)).unwrap();
        let out = oni_render::cpu::curves([p[0], p[1], p[2], 1.0], &k);
        [out[0], out[1], out[2]]
    }

    #[test]
    fn each_mode_moves_what_it_names() {
        let p = [0.5, 0.4, 0.2];
        let window = Levels { white: 0.8, mid: 0.3, ..Levels::default() };
        // R moves red alone.
        let red = through(&PictureLevels { red: window, ..PictureLevels::default() }, p);
        assert!(red[0] > 0.6 && (red[1] - p[1]).abs() < 1e-6 && (red[2] - p[2]).abs() < 1e-6, "{red:?}");
        // RGB moves the three by one window, as R does red.
        let linked = through(&PictureLevels { linked: window, ..PictureLevels::default() }, p);
        assert!((linked[0] - red[0]).abs() < 1e-6 && linked[1] > p[1] + 0.1 && linked[2] > p[2] + 0.05, "{linked:?}");
        // Luma lifts the light and keeps the hue: one gain on all three.
        let luma = through(&PictureLevels { luma: window, ..PictureLevels::default() }, p);
        let gain = luma[0] / p[0];
        assert!(gain > 1.1 && (luma[1] / p[1] - gain).abs() < 1e-5 && (luma[2] / p[2] - gain).abs() < 1e-5, "{luma:?}");
        // Twin: the same window on RGB turns the hue, a bent window on each channel apart.
        assert!((linked[1] / p[1] - linked[0] / p[0]).abs() > 0.05, "{linked:?}");
    }

    #[test]
    fn past_one_a_table_goes_on_at_the_slope_its_levels_reached() {
        let levels = PictureLevels { linked: Levels::with_ends(0.1, 0.9), red: Levels { white: 0.8, highlights: 0.6, ..Levels::default() }, ..PictureLevels::default() };
        let k = curves(&CurveSet::default(), Some(&levels)).unwrap();
        let composite = |x: f64| levels.linked.transfer(levels.red.transfer(x));
        let slope = (composite(1.5) - composite(1.25)) / 0.25;
        assert!((f64::from(k.ends[1]) - slope).abs() < 1e-5, "{} {slope}", k.ends[1]);
        assert_eq!((k.ends[0], k.luma_end), (1.0, 1.0));
        let at = |x: f32| oni_render::cpu::curves([x, 0.0, 0.0, 1.0], &k)[0];
        assert!((f64::from(at(1.5)) - composite(1.5)).abs() < 1e-4, "{} {}", at(1.5), composite(1.5));
        // Twin: a slope of 1 past the table, the curves' own, falls short of the window's.
        let one = Curves { ends: [1.0; 4], ..k.clone() };
        assert!((f64::from(oni_render::cpu::curves([1.5, 0.0, 0.0, 1.0], &one)[0]) - composite(1.5)).abs() > 0.1);
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
