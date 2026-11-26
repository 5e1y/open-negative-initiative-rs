//! `Palette`: the year's colours, each photo's own read off its thumbnail and the year's few gathered
//! from them in Oklab, where a distance is a difference the eye sees.

/// A colour in Oklab: lightness, green to red, blue to yellow.
pub type Lab = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swatch {
    pub rgb: [u8; 3],
    /// Its share of the photos read, 0 to 1.
    pub share: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Palette {
    /// The year's colours, the most shared first.
    pub swatches: Vec<Swatch>,
    /// The photos' mean on the blue to yellow axis: above zero the year leans warm.
    pub warmth: f32,
    /// Their mean chroma, near zero for a year in black and white.
    pub chroma: f32,
    pub sampled: usize,
    /// For each swatch, the place among the tones given of the photo nearest it.
    pub picks: Vec<usize>,
}

/// Under this mean chroma a picture has no colour of its own: a black and white one, or a grey day.
pub const NEUTRAL: f32 = 0.02;
/// A quarter of the photos having a colour makes a year in colour, its palette theirs alone.
pub const COLOURED: f32 = 0.25;
const HUES: usize = 12;
/// A thumbnail is read on about this many pixels, enough for its colour and a millisecond's work.
const SAMPLES: usize = 4096;
const ROUNDS: usize = 12;

fn linear(code: u8) -> f32 {
    let v = f32::from(code) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn code(linear: f32) -> u8 {
    let v = linear.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// sRGB codes to Oklab (Ottosson's matrices).
pub fn lab(rgb: [u8; 3]) -> Lab {
    let (r, g, b) = (linear(rgb[0]), linear(rgb[1]), linear(rgb[2]));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// Oklab back to sRGB codes, a colour outside the gamut clipped a channel at a time.
pub fn rgb(lab: Lab) -> [u8; 3] {
    let l = (lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2]).powi(3);
    let m = (lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2]).powi(3);
    let s = (lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2]).powi(3);
    [
        code(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        code(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        code(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

fn chroma(lab: Lab) -> f32 {
    lab[1].hypot(lab[2])
}

/// A picture's own colour: the mean of its heaviest hue, each pixel weighing its chroma so the greys
/// around a subject do not wash it out; its mean grey when it has no colour. `rgba` is 8-bit sRGB.
pub fn dominant(rgba: &[u8], width: usize, height: usize) -> Option<Lab> {
    let pixels = width.checked_mul(height).filter(|n| *n > 0 && rgba.len() >= n * 4)?;
    let stride = (pixels / SAMPLES).max(1);
    let mut bins = [[0.0f32; 4]; HUES];
    let (mut mean, mut count) = ([0.0f32; 3], 0.0f32);
    for at in (0..pixels).step_by(stride) {
        let p = lab([rgba[at * 4], rgba[at * 4 + 1], rgba[at * 4 + 2]]);
        let weight = chroma(p);
        let turn = p[2].atan2(p[1]) / std::f32::consts::TAU + 0.5;
        let bin = &mut bins[((turn * HUES as f32) as usize).min(HUES - 1)];
        for (sum, value) in bin.iter_mut().zip([p[0] * weight, p[1] * weight, p[2] * weight, weight]) {
            *sum += value;
        }
        for (sum, value) in mean.iter_mut().zip(p) {
            *sum += value;
        }
        count += 1.0;
    }
    let mean = mean.map(|v| v / count);
    let weight: f32 = bins.iter().map(|b| b[3]).sum();
    if weight / count < NEUTRAL {
        return Some([mean[0], 0.0, 0.0]);
    }
    let heaviest = bins.iter().fold(bins[0], |best, b| if b[3] > best[3] { *b } else { best });
    Some([heaviest[0] / heaviest[3], heaviest[1] / heaviest[3], heaviest[2] / heaviest[3]])
}

fn distance(a: Lab, b: Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn nearest(centres: &[Lab], tone: Lab) -> usize {
    centres.iter().enumerate().fold((0, f32::MAX), |best, (i, c)| if distance(*c, tone) < best.1 { (i, distance(*c, tone)) } else { best }).0
}

/// A tone as an ink that stands out on black: its hue kept, lit and saturated as far as sRGB holds; none
/// for a tone with no colour of its own.
pub fn vivid(tone: Lab) -> Option<[u8; 3]> {
    let c = chroma(tone);
    if c < NEUTRAL * 2.0 {
        return None;
    }
    let inside = |lab: Lab| {
        let l = (lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2]).powi(3);
        let m = (lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2]).powi(3);
        let s = (lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2]).powi(3);
        let linear = [4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s, -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s, -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s];
        linear.iter().all(|v| (-0.001..=1.001).contains(v))
    };
    // The most chroma this hue takes at this lightness, by halving steps down from a chroma no screen holds.
    let at = |k: f32| [VIVID_LIGHT, tone[1] / c * k, tone[2] / c * k];
    let most = (0..40).map(|i| VIVID_CHROMA * (1.0 - i as f32 / 40.0)).find(|k| inside(at(*k)))?;
    Some(rgb(at(most)))
}

/// An ink's lightness on black, and the chroma it is asked: a pastel reads as a tint, this as a colour.
pub const VIVID_LIGHT: f32 = 0.82;
pub const VIVID_CHROMA: f32 = 0.2;

impl Palette {
    /// The `count` colours `tones` gather around, each photo counting once: those of the photos that have
    /// a colour, the greys among them left out of a year in colour. The first centre is the tone nearest
    /// their mean and each next the furthest from those chosen, so the same photos give the same year.
    pub fn of(tones: &[Lab], count: usize) -> Palette {
        if tones.is_empty() || count == 0 {
            return Palette::default();
        }
        let all = tones.len() as f32;
        let (warmth, mean_chroma) = (tones.iter().map(|t| t[2]).sum::<f32>() / all, tones.iter().map(|t| chroma(*t)).sum::<f32>() / all);
        let coloured: Vec<usize> = (0..tones.len()).filter(|i| chroma(tones[*i]) >= NEUTRAL).collect();
        let sampled = tones.len();
        // The tones gathered, each with its place among those given.
        let places: Vec<usize> = if coloured.len() as f32 >= all * COLOURED { coloured } else { (0..tones.len()).collect() };
        let gathered: Vec<Lab> = places.iter().map(|i| tones[*i]).collect();
        let tones: &[Lab] = &gathered;
        let n = tones.len() as f32;
        let mean = tones.iter().fold([0.0; 3], |sum, t| [sum[0] + t[0] / n, sum[1] + t[1] / n, sum[2] + t[2] / n]);
        let mut centres = vec![tones[nearest(tones, mean)]];
        while centres.len() < count.min(tones.len()) {
            let far = tones.iter().fold((tones[0], -1.0), |best, t| {
                let d = distance(centres[nearest(&centres, *t)], *t);
                if d > best.1 { (*t, d) } else { best }
            });
            if far.1 <= 1e-6 {
                break;
            }
            centres.push(far.0);
        }
        let mut sizes = vec![0usize; centres.len()];
        for _ in 0..ROUNDS {
            let mut sums = vec![[0.0f32; 3]; centres.len()];
            sizes.iter_mut().for_each(|s| *s = 0);
            for tone in tones {
                let i = nearest(&centres, *tone);
                sizes[i] += 1;
                for (sum, value) in sums[i].iter_mut().zip(tone) {
                    *sum += value;
                }
            }
            for ((centre, sum), size) in centres.iter_mut().zip(sums).zip(&sizes) {
                if *size > 0 {
                    *centre = sum.map(|v| v / *size as f32);
                }
            }
        }
        let mut found: Vec<(Swatch, usize)> = centres
            .iter()
            .zip(&sizes)
            .filter(|(_, s)| **s > 0)
            .map(|(c, s)| (Swatch { rgb: rgb(*c), share: *s as f32 / n }, places[nearest(tones, *c)]))
            .collect();
        found.sort_by(|a, b| b.0.share.total_cmp(&a.0.share).then(a.0.rgb.cmp(&b.0.rgb)));
        let (swatches, picks) = found.into_iter().unzip();
        Palette { swatches, warmth, chroma: mean_chroma, sampled, picks }
    }

    /// A year of pictures with no colour of their own.
    pub fn is_monochrome(&self) -> bool {
        self.sampled > 0 && self.chroma < NEUTRAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(rgb: [u8; 3], pixels: usize) -> Vec<u8> {
        (0..pixels).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    #[test]
    fn oklab_goes_out_and_back_and_knows_white_grey_and_the_warm_side() {
        for colour in [[255, 255, 255], [0, 0, 0], [203, 163, 114], [12, 200, 90], [30, 60, 220]] {
            let back = rgb(lab(colour));
            assert!(colour.iter().zip(back).all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 1), "{colour:?} -> {back:?}");
        }
        let white = lab([255, 255, 255]);
        assert!((white[0] - 1.0).abs() < 1e-3 && chroma(white) < 1e-3);
        assert!(lab([230, 160, 60])[2] > 0.05 && lab([60, 120, 230])[2] < -0.05, "orange is warm, blue is not");
        // Twin: a grey has no chroma, a red has.
        assert!(chroma(lab([128, 128, 128])) < 1e-3 && chroma(lab([200, 30, 30])) > 0.1);
    }

    #[test]
    fn a_pictures_colour_is_its_subjects_not_the_grey_around_it() {
        // A fifth of red on four fifths of grey: the plain mean would be a dull pink.
        let mut picture = flat([128, 128, 128], 800);
        picture.extend(flat([210, 40, 40], 200));
        let tone = dominant(&picture, 100, 10).unwrap();
        let red = lab([210, 40, 40]);
        assert!(distance(tone, red) < 1e-4, "{tone:?} against {red:?}");
        // Twin: the plain mean is far from the red.
        let n = 1000.0;
        let mean = [(128.0 * 800.0 + 210.0 * 200.0) / n, (128.0 * 800.0 + 40.0 * 200.0) / n, (128.0 * 800.0 + 40.0 * 200.0) / n].map(|v: f32| v.round() as u8);
        assert!(distance(lab(mean), red) > 0.01);
        // A picture without colour gives its grey; one with no pixel, or too few bytes, gives nothing.
        let grey = dominant(&flat([90, 90, 90], 64), 8, 8).unwrap();
        assert!(chroma(grey) == 0.0 && (grey[0] - lab([90, 90, 90])[0]).abs() < 1e-4);
        assert_eq!((dominant(&[], 0, 0), dominant(&[0; 8], 4, 4)), (None, None));
    }

    #[test]
    fn a_photos_tone_becomes_an_ink_that_stands_out_on_black_in_its_own_hue() {
        let hue = |lab: Lab| lab[2].atan2(lab[1]);
        for dull in [[150, 110, 80], [60, 80, 120], [70, 110, 75], [140, 60, 70]] {
            let ink = vivid(lab(dull)).unwrap();
            let (from, to) = (lab(dull), lab(ink));
            assert!((hue(from) - hue(to)).abs() < 0.12, "{dull:?} -> {ink:?}: the hue is kept");
            // The ink is lit at VIVID_LIGHT with what chroma sRGB holds there: a red may hold less than the dull one.
            assert!(to[0] > 0.75 && to[0] > from[0] && chroma(to) > 0.07, "{dull:?} -> {ink:?}: lit and saturated");
        }
        // Twin: a grey has no ink of its own, and the dull tone itself is no ink for black.
        assert_eq!(vivid(lab([120, 120, 120])), None);
        assert!(lab([60, 80, 120])[0] < 0.5);
    }

    #[test]
    fn the_years_colours_gather_by_what_the_eye_sees_the_most_shared_first() {
        let (orange, blue, green) = (lab([230, 150, 60]), lab([50, 110, 220]), lab([60, 170, 80]));
        let near = |c: Lab, d: f32| [c[0] + d, c[1], c[2] - d];
        let mut tones = vec![orange; 5];
        tones.extend([near(orange, 0.01), near(orange, -0.01), blue, near(blue, 0.01), blue, green]);
        let palette = Palette::of(&tones, 3);
        assert_eq!(palette.swatches.len(), 3);
        let shares: Vec<f32> = palette.swatches.iter().map(|s| s.share).collect();
        assert!((shares[0] - 7.0 / 11.0).abs() < 1e-6 && (shares[1] - 3.0 / 11.0).abs() < 1e-6 && (shares[2] - 1.0 / 11.0).abs() < 1e-6, "{shares:?}");
        assert!(distance(lab(palette.swatches[0].rgb), orange) < 1e-3);
        assert!(distance(lab(palette.swatches[2].rgb), green) < 1e-3);
        assert!(palette.warmth > 0.0 && !palette.is_monochrome());
        assert_eq!(palette, Palette::of(&tones, 3), "the same photos give the same year");
        // Twin: asked one colour, the three are not told apart.
        assert_eq!(Palette::of(&tones, 1).swatches.len(), 1);
        // More colours asked than the photos hold: only theirs; a year of greys is monochrome.
        assert_eq!(Palette::of(&[orange, orange, blue], 5).swatches.len(), 2);
        assert!(Palette::of(&[[0.5, 0.0, 0.0], [0.7, 0.0, 0.0]], 3).is_monochrome());
        assert_eq!(Palette::of(&[], 5), Palette::default());
        // Each swatch names the photo nearest it, by its place among the tones given.
        assert_eq!(palette.picks.len(), 3);
        assert!(distance(tones[palette.picks[0]], orange) < 1e-3 && distance(tones[palette.picks[1]], blue) < 1e-3 && palette.picks[2] == 10);
        // A year in colour leaves its greys out of the palette, and still counts them read.
        let grey = [0.4, 0.0, 0.0];
        let mixed = Palette::of(&[orange, blue, grey, grey, grey], 3);
        assert_eq!((mixed.swatches.len(), mixed.sampled), (2, 5));
        assert!(mixed.swatches.iter().all(|s| chroma(lab(s.rgb)) > NEUTRAL));
        // Twin: one coloured photo in ten is a year in grey, the grey its first colour.
        let dull = Palette::of(&[orange, grey, grey, grey, grey, grey, grey, grey, grey, grey], 2);
        assert!(chroma(lab(dull.swatches[0].rgb)) < NEUTRAL && (dull.swatches[0].share - 0.9).abs() < 1e-6);
    }
}
