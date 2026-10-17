//! Every ported kernel on the GPU against its scalar reference, on roll #8 and on adversarial values;
//! each verdict has a twin, a reference deliberately wrong, which the check must refuse.

mod common;

use common::{adversarial_image, each, each_parallel, gpu, num, pair, roll, Judge, TOLERANCE};
use oni_render::*;

fn crop(image: &Image, x: usize, y: usize, w: usize, h: usize) -> Image {
    let pixels = (y..y + h).flat_map(|r| (x..x + w).map(move |c| (c, r))).map(|(c, r)| image.at(c, r)).collect();
    Image::new(w, h, pixels)
}

fn luma_only() -> ColorMatrix {
    let l = [0.2627, 0.6780, 0.0593, 0.0];
    ColorMatrix { r_vector: l, g_vector: l, b_vector: l, ..ColorMatrix::default() }
}

/// A 5 by 5 box blur, clamped at the edges: a stand-in for the unported Gaussian, smooth like it.
fn boxed(image: &Image) -> Image {
    let (w, h) = (image.width as i64, image.height as i64);
    let mut out = image.clone();
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 4];
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let p = image.at((x + dx).clamp(0, w - 1) as usize, (y + dy).clamp(0, h - 1) as usize);
                    for c in 0..4 {
                        s[c] += p[c] / 25.0;
                    }
                }
            }
            out.pixels[(y * w + x) as usize] = s;
        }
    }
    out
}

/// The real positive and the adversarial ring of the same shape, each kernel's two inputs.
fn inputs() -> Vec<(&'static str, Image)> {
    let r = roll();
    let real = if r.real { "roll #8" } else { "stand-in negative" };
    vec![(real, r.positive.clone()), ("adversarial", adversarial_image(r.positive.width, r.positive.height))]
}

#[test]
fn stage47_on_roll8() {
    let r = roll();
    let Some(g) = gpu("stage47_on_roll8") else { return };
    let mut judge = Judge::default();
    let lifted = ExposureAdjust { ev: -num(&r.settings, "captureStops") };
    let out = g.stage47(&g.exposure_adjust(&r.negative, &lifted).pixels, &r.stage47);
    let shifted = Stage47 { gain: [r.stage47.gain[0], r.stage47.gain[1] * 1.01, r.stage47.gain[2]], ..r.stage47 };
    let twin = each(&r.negative, |p| cpu::stage47(cpu::exposure_adjust(p, &lifted), &shifted));
    judge.check("stage47, roll #8 at its sidecar", &out, &r.positive.pixels, &twin.pixels, TOLERANCE);
    judge.done();
}

/// The whole frame through the head and the tail of the chain: past 4.2 M pixels a dispatch folds
/// onto its second dimension, which no crop reaches.
#[test]
fn whole_frame_on_roll8() {
    let r = roll();
    if !r.real || !oni_fixtures::heavy("whole_frame_on_roll8") {
        println!("SKIPPED whole_frame_on_roll8: it needs the whole 24 MP frame");
        return;
    }
    let Some(g) = gpu("whole_frame_on_roll8") else { return };
    let mut judge = Judge::default();
    let frame = &r.frame;
    println!("whole frame {}x{}, {} pixels", frame.width, frame.height, frame.pixels.len());
    let lifted = ExposureAdjust { ev: -num(&r.settings, "captureStops") };
    let out = g.output_clip(&frame.with_pixels(g.stage47(&g.exposure_adjust(frame, &lifted).pixels, &r.stage47)));
    let reference = each_parallel(frame, |p| cpu::output_clip(cpu::stage47(cpu::exposure_adjust(p, &lifted), &r.stage47)));
    let shifted = Stage47 { gain: [r.stage47.gain[0], r.stage47.gain[1] * 1.01, r.stage47.gain[2]], ..r.stage47 };
    let twin = each_parallel(frame, |p| cpu::output_clip(cpu::stage47(cpu::exposure_adjust(p, &lifted), &shifted)));
    judge.image("exposure, stage47, outputClip, whole frame", &out, &reference, &twin);
    // The twin a missed fold leaves: the last pixels never written, so zero.
    let mut unwritten = reference.clone();
    let tail = unwritten.pixels.len() - 1000;
    unwritten.pixels[tail..].iter_mut().for_each(|p| *p = [0.0; 4]);
    judge.image("the same, its last thousand pixels unwritten", &out, &reference, &unwritten);
    // Bands of rows each carry their own origin: a vignette off by one band would sit off centre.
    let k = Vignette::on_extent(frame.origin, frame.width, frame.height, 1.2, 0.0);
    let wrong = Vignette { centre: [k.centre[0], k.centre[1] + 1.0], ..k };
    judge.image("vignette, whole frame in bands", &g.vignette(&reference, &k), &cpu::vignette(&reference, &k), &cpu::vignette(&reference, &wrong));
    judge.done();
}

#[test]
fn output_clip() {
    let Some(g) = gpu("output_clip") else { return };
    let mut judge = Judge::default();
    for (name, image) in inputs() {
        let out = g.output_clip(&image);
        let reference = each(&image, cpu::output_clip);
        // The twin bounds below only, the clip a shoulder-free chain must never lose.
        let twin = each(&image, |p| [p[0].max(0.0), p[1].max(0.0), p[2].max(0.0), p[3]]);
        judge.image(&format!("outputClip, {name}"), &out, &reference, &twin);
    }
    // Metal's clamp is fmin(fmax(x, 0), 1): a NaN leaves as the low bound, an infinity as its end.
    let specials = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30, -0.0, 0.5];
    let pixels: Vec<[f32; 4]> = (0..specials.len().pow(3))
        .map(|i| [specials[i % 7], specials[i / 7 % 7], specials[i / 49], 1.0])
        .collect();
    let image = Image::new(pixels.len(), 1, pixels);
    let twin = each(&image, |p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), p[3]]);
    judge.image("outputClip, NaN and infinities", &g.output_clip(&image), &each(&image, cpu::output_clip), &twin);
    let k = ColorClamp::default();
    let twin = each(&image, |p| std::array::from_fn(|c| p[c].clamp(k.min_components[c], k.max_components[c])));
    judge.image("CIColorClamp, NaN and infinities", &g.color_clamp(&image, &k), &each(&image, |p| cpu::color_clamp(p, &k)), &twin);
    judge.done();
}

#[test]
fn flat_field() {
    let r = roll();
    let Some(g) = gpu("flat_field") else { return };
    let mut judge = Judge::default();
    let (w, h) = (r.negative.width, r.negative.height);
    let falloff = RadialGradient {
        center: [w as f32 * 0.45, h as f32 * 0.55],
        radius0: 50.0,
        radius1: w as f32 * 0.7,
        color0: [1.0, 0.98, 0.97, 1.0],
        color1: [0.55, 0.62, 0.7, 1.0],
    };
    let mask = cpu::generated([0.0; 2], w, h, |at| cpu::radial_gradient(at, &falloff));
    let floored = adversarial_image(w, h);
    let cases = [
        ("green mask, full", FlatField { amount: 1.0, exponent: [1.0; 3], per_channel: 0.0 }, &mask),
        ("passes, tinted", FlatField { amount: 0.7, exponent: [1.1, 1.0, 0.9], per_channel: 1.0 }, &mask),
        ("identity at amount 0", FlatField { amount: 0.0, exponent: [1.2, 1.0, 0.8], per_channel: 1.0 }, &mask),
        ("mask under the floor", FlatField { amount: 1.0, exponent: [1.0; 3], per_channel: 1.0 }, &floored),
    ];
    for (name, k, m) in cases {
        let out = g.flat_field(&r.negative, m, &k);
        let reference = pair(&r.negative, m, |s, f| cpu::flat_field(s, f, &k));
        let wrong = FlatField { amount: k.amount + 0.02, exponent: [k.exponent[0], k.exponent[1] * 1.05, k.exponent[2]], ..k };
        let twin = pair(&r.negative, m, |s, f| cpu::flat_field(s, f, &wrong));
        judge.image(&format!("flatField, {name}"), &out, &reference, &twin);
    }
    judge.done();
}

/// Tables as `CurveSet.rgbLUT` packs them: linked, red, green, blue; luma apart.
fn tables(offset: f32) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
    let n = LUT_SIZE;
    let rgb = (0..n)
        .map(|i| {
            let x = i as f32 / (n - 1) as f32;
            let s = x * x * (3.0 - 2.0 * x);
            [0.3 * x + 0.7 * s + offset, x.powf(0.9) + offset, x.powf(1.05) + offset, x.powf(1.15) + offset]
        })
        .collect();
    let luma = (0..n).map(|i| [(i as f32 / (n - 1) as f32).powf(0.8) + offset, 0.0, 0.0, 1.0]).collect();
    (rgb, luma)
}

#[test]
fn curves() {
    let Some(g) = gpu("curves") else { return };
    let mut judge = Judge::default();
    let (rgb, luma) = tables(0.0);
    let (rgb_off, luma_off) = tables(0.002);
    for bent in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0]] {
        let k = Curves { rgb_lut: rgb.clone(), luma_lut: luma.clone(), bent };
        let wrong = Curves { rgb_lut: rgb_off.clone(), luma_lut: luma_off.clone(), bent };
        for (name, image) in inputs() {
            let out = g.curves(&image, &k);
            judge.image(&format!("curves {bent:?}, {name}"), &out, &each(&image, |p| cpu::curves(p, &k)), &each(&image, |p| cpu::curves(p, &wrong)));
        }
    }
    judge.done();
}

#[test]
fn saturation_curve() {
    let Some(g) = gpu("saturation_curve") else { return };
    let mut judge = Judge::default();
    let sauce = SaturationCurve { knots: [0.103, 0.368, 0.799], lows: [0.0, 0.284, 0.537, 0.296], high: 0.0, bends: [0.8, -1.2, 0.5] };
    let lifted = SaturationCurve { knots: [0.2, 0.5, 0.8], lows: [0.6, 0.7, 0.5, 0.45], high: 0.5, bends: [0.0; 3] };
    for (curve, k) in [("sauce", sauce), ("lifted shadows", lifted)] {
        let wrong = SaturationCurve { lows: k.lows.map(|v| v + 0.01), high: k.high + 0.01, ..k };
        for (name, image) in inputs() {
            let out = g.saturation_curve(&image, &k);
            judge.image(&format!("saturationCurve {curve}, {name}"), &out, &each(&image, |p| cpu::saturation_curve(p, &k)), &each(&image, |p| cpu::saturation_curve(p, &wrong)));
        }
    }
    judge.done();
}

#[test]
fn texture() {
    let r = roll();
    let Some(g) = gpu("texture") else { return };
    let mut judge = Judge::default();
    let luma = each(&r.positive, |p| cpu::color_matrix(p, &luma_only()));
    let narrow = boxed(&luma);

    let moments = g.texture_moments(&narrow);
    let reference = each(&narrow, cpu::texture_moments);
    judge.image("textureMoments, roll #8", &moments, &reference, &each(&narrow, |p| cpu::texture_moments(p.map(|v| v * 1.01))));

    let wide = boxed(&reference);
    let k = TextureCoeff { relative: 0.125 };
    let coeff = g.texture_coeff(&wide, &k);
    let reference = each(&wide, |m| cpu::texture_coeff(m, &k));
    let twin = each(&wide, |m| cpu::texture_coeff(m, &TextureCoeff { relative: 0.25 }));
    judge.image("textureCoeff, roll #8", &coeff, &reference, &twin);

    let blurred = boxed(&reference);
    for gain in [1.0 / 0.82, -0.8 / 0.82] {
        let k = Texture { gain };
        let out = g.texture(&r.positive, &narrow, &blurred, &k);
        let three = |k: Texture| {
            let pixels = (0..r.positive.pixels.len()).map(|i| cpu::texture(r.positive.pixels[i], narrow.pixels[i], blurred.pixels[i], &k)).collect();
            r.positive.with_pixels(pixels)
        };
        judge.image(&format!("texture gain {gain:.2}, roll #8"), &out, &three(k), &three(Texture { gain: gain + 0.05 }));
    }
    judge.done();
}

#[test]
fn detail() {
    let r = roll();
    let Some(g) = gpu("detail") else { return };
    let mut judge = Judge::default();
    let chroma = (num(&r.settings, "chroma.radius") as f64, num(&r.settings, "chroma.force"));
    let sharpen = (num(&r.settings, "sharpen.radius") as f64, num(&r.settings, "sharpen.amount"), num(&r.settings, "sharpen.halo"));
    let sidecar = Detail::dosed(Some(chroma), Some(sharpen)).expect("roll #8 doses its detail");
    // The arguments `applyDetail` builds for 4 px and 2.092 px: reaches of ceil(3.5 sigma), a disk of 2.
    if sidecar.reach != [14.0, 8.0] || sidecar.disk != 2.0 || sidecar.margin() != 14 {
        judge.fail(format!("Detail::dosed gave {sidecar:?}"));
    }
    if Detail::dosed(Some((0.1614, 1.0)), Some((0.1, 1.0, 0.5))).is_some() {
        judge.fail("a radius at the blur floor still doses".into());
    }
    let cases = [
        ("roll #8 sidecar, denoise and sharpening", sidecar),
        ("denoise alone", Detail::dosed(Some(chroma), None).unwrap()),
        ("sharpening without halo", Detail::dosed(None, Some((sharpen.0, 1.5, 0.0))).unwrap()),
        ("widest sharpening, halo held", Detail::dosed(None, Some((6.53, 2.0, 1.0))).unwrap()),
        ("both, strong", Detail::dosed(Some((8.0, 1.0)), Some((3.0, 0.8, 0.3))).unwrap()),
    ];
    for (name, k) in cases {
        let wrong = Detail { sigma: k.sigma.map(|s| s * 1.05), ..k };
        for (input, image) in [("roll #8", r.positive.clone()), ("adversarial", adversarial_image(256, 192))] {
            let out = g.detail(&image, &k);
            judge.image(&format!("detail {name}, {input}"), &out, &cpu::detail(&image, &k), &cpu::detail(&image, &wrong));
        }
    }
    // The taps sum to one, so a flat grey leaves untouched: no chroma to denoise, no edge to sharpen.
    let flat = Image::new(64, 48, vec![[0.18, 0.18, 0.18, 1.0]; 64 * 48]);
    let brighter = each(&flat, |p| [p[0] * 1.0001, p[1] * 1.0001, p[2] * 1.0001, 1.0]);
    judge.check("detail, flat grey is a fixed point", &g.detail(&flat, &sidecar).pixels, &flat.pixels, &brighter.pixels, 2e-6);
    // Narrower than its reach: every tap clamps, on both sides and both passes.
    let tiny = adversarial_image(5, 3);
    let k = Detail::dosed(Some((4.0, 0.6)), Some((2.5, 1.0, 0.7))).unwrap();
    let wrong = Detail { force: 0.7, ..k };
    judge.image("detail, 5x3 under a reach of 14", &g.detail(&tiny, &k), &cpu::detail(&tiny, &k), &cpu::detail(&tiny, &wrong));
    judge.done();
}

/// Saturated colours all around the hue circle, at three levels, then the roll.
fn hue_wheel() -> Image {
    let (w, h) = (360, 3);
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let a = (x as f32).to_radians();
            let level = [0.05, 0.3, 0.8][y];
            [level * (1.0 + 0.8 * a.cos()), level * (1.0 + 0.8 * (a - 2.094).cos()), level * (1.0 + 0.8 * (a + 2.094).cos()), 1.0]
        })
        .collect();
    Image::new(w, h, pixels)
}

#[test]
fn spectrogram() {
    let Some(g) = gpu("spectrogram") else { return };
    let mut judge = Judge::default();
    let k = Spectrogram {
        hue_low: [12.0, -8.0, 0.0, 20.0],
        hue_high: [-15.0, 5.0, 0.0, -30.0],
        sat_low: [0.3, -0.4, 0.0, 0.5],
        sat_high: [-0.6, 0.2, 1.0, 0.0],
        luma_low: [0.2, 0.0, -0.3, 0.1],
        luma_high: [0.0, -0.5, 0.4, 0.0],
    };
    let scaled = |f: f32| Spectrogram {
        hue_low: k.hue_low.map(|v| v * f),
        hue_high: k.hue_high.map(|v| v * f),
        sat_low: k.sat_low.map(|v| v * f),
        sat_high: k.sat_high.map(|v| v * f),
        luma_low: k.luma_low.map(|v| v * f),
        luma_high: k.luma_high.map(|v| v * f),
    };
    let wrong = scaled(1.05);
    let mut images = inputs();
    images.push(("hue wheel", hue_wheel()));
    for (name, image) in images {
        let out = g.spectrogram(&image, &k);
        judge.image(&format!("spectrogram, {name}"), &out, &each(&image, |p| cpu::spectrogram(p, &k)), &each(&image, |p| cpu::spectrogram(p, &wrong)));
    }
    judge.done();
}

#[test]
fn vignette() {
    let Some(g) = gpu("vignette") else { return };
    let mut judge = Judge::default();
    for (name, image) in inputs() {
        let placed = Image { origin: [37.0, -12.0], ..image };
        for (stops, hunt) in [(1.2, 0.19), (-0.8, 0.19), (2.0, 0.0)] {
            let k = Vignette::on_extent(placed.origin, placed.width, placed.height, stops, hunt);
            let wrong = Vignette { centre: [k.centre[0] + 5.0, k.centre[1]], ..k };
            let out = g.vignette(&placed, &k);
            // Hunt floors a negative channel wherever its power is not exactly 1, a step that one ulp moves
            // just past the midpoint: pixels whose factor lies within 1e-5 of 1 there are left out.
            let kept: Vec<usize> = (0..placed.pixels.len())
                .filter(|&i| {
                    let at = placed.ci_coord(i % placed.width, i / placed.width);
                    let d = [(at[0] - k.centre[0]) / k.half_side[0], (at[1] - k.centre[1]) / k.half_side[1]];
                    let past = (d[0] * d[0] + d[1] * d[1]).sqrt() * std::f32::consts::FRAC_1_SQRT_2 - k.dose[1];
                    let t = (past / (1.0 - k.dose[1])).max(0.0);
                    past < -1e-5 || (1.0 - (-k.dose[0] * t * t).exp2()).abs() > 1e-5 || k.hunt == 0.0
                })
                .collect();
            let pick = |img: &Image| kept.iter().map(|&i| img.pixels[i]).collect::<Vec<_>>();
            let (reference, twin) = (cpu::vignette(&placed, &k), cpu::vignette(&placed, &wrong));
            let label = format!("vignette {stops} stops, hunt {hunt}, {name}, {} on the step", placed.pixels.len() - kept.len());
            judge.check(&label, &pick(&out), &pick(&reference), &pick(&twin), TOLERANCE);
        }
    }
    judge.done();
}

#[test]
fn screen_gamut_zebra() {
    let r = roll();
    let Some(g) = gpu("screen_gamut_zebra") else { return };
    let mut judge = Judge::default();
    let picture = Image { origin: [3.0, 5.0], ..r.positive.clone() };
    // The picture in a narrower display's encoding, pushed so its saturated half leaves the cube.
    let narrow = ColorMatrix {
        r_vector: [1.660_49 * 1.4, -0.587_641 * 1.4, -0.072_850 * 1.4, 0.0],
        g_vector: [-0.124_550 * 1.4, 1.132_9 * 1.4, -0.008_349 * 1.4, 0.0],
        b_vector: [-0.018_150 * 1.4, -0.100_579 * 1.4, 1.118_73 * 1.4, 0.0],
        ..ColorMatrix::default()
    };
    let shown = each(&picture, |p| cpu::color_matrix(p, &narrow));
    let k = ScreenGamutZebra { tint: [0xC8 as f32 / 255.0, 0xF0 as f32 / 255.0, 0x3C as f32 / 255.0, 1.0], period: 9.0, duty: 0.34, phase: 0.37, tolerance: 0.01 };
    let out = g.screen_gamut_zebra(&picture, &shown, &k);
    let reference = cpu::screen_gamut_zebra(&picture, &shown, &k);
    let striped = reference.pixels.iter().filter(|p| **p == k.tint).count();
    println!("zebra: {striped} of {} pixels striped", reference.pixels.len());
    if striped == 0 || striped == reference.pixels.len() {
        judge.fail(format!("zebra exercised neither branch: {striped} striped"));
    }
    let twin = cpu::screen_gamut_zebra(&picture, &shown, &ScreenGamutZebra { phase: k.phase + 1.0, ..k });
    judge.image("screenGamutZebra, roll #8", &out, &reference, &twin);
    judge.done();
}

#[test]
fn rgb_passes() {
    let r = roll();
    let Some(g) = gpu("rgb_passes") else { return };
    let mut judge = Judge::default();
    let (w, h) = (r.negative.width, r.negative.height);
    let (red, green, blue) = (r.negative.clone(), r.positive.clone(), adversarial_image(w, h));
    let out = g.rgb_passes(&red, &green, &blue);
    let reference: Vec<_> = (0..w * h).map(|i| cpu::rgb_passes(red.pixels[i], green.pixels[i], blue.pixels[i])).collect();
    let twin: Vec<_> = (0..w * h).map(|i| cpu::rgb_passes(blue.pixels[i], green.pixels[i], red.pixels[i])).collect();
    judge.check("rgbPasses, three passes", &out.pixels, &reference, &twin, TOLERANCE);
    judge.done();
}

#[test]
fn scanner() {
    let r = roll();
    let Some(g) = gpu("scanner") else { return };
    let Some(f) = common::frontier("scanner") else { return };
    let mut judge = Judge::default();
    let s = &r.settings;
    let click = |k: &str| num(s, &format!("scanner.{k}")) as i64;
    let offset = f.offset([("D", click("d")), ("C", click("c")), ("M", click("m")), ("Y", click("y"))]);
    let neutral = common::Frontier::v3(&f.json["neutral"]["density"]);
    let range: Vec<f32> = (0..2).map(|i| f.json["render"]["densityRange"][i].as_f64().unwrap() as f32).collect();
    let (black, white) = (num(s, "levels.green.black"), num(s, "levels.green.white"));
    let ceiling = num(s, "densityCeiling");
    let k = ScannerDensity {
        gain: r.stage47.gain,
        pedestal: 10f32.powf(-ceiling),
        log_guard: 10f32.powf(-ceiling) / 8.0,
        median: [black + 0.6 * (white - black); 3],
        sep_r: [1.18, -0.12, -0.06],
        sep_g: [-0.09, 1.21, -0.12],
        sep_b: [-0.04, -0.17, 1.21],
        scale: 1.3 / (white - black).max(0.02),
        shift: std::array::from_fn(|c| neutral[c] + offset[c]),
        reached_low: common::Frontier::v3(&f.json["render"]["reached"][0]),
        reached_high: common::Frontier::v3(&f.json["render"]["reached"][1]),
        low: range[0],
        span: range[1] - range[0],
        invert: 1.0,
        dmax: 3.0,
    };
    let lifted = ExposureAdjust { ev: -num(s, "captureStops") };
    let negative = each(&r.negative, |p| cpu::exposure_adjust(p, &lifted));
    for (name, k) in [("negative", k), ("slide", ScannerDensity { invert: 0.0, ..k })] {
        let out = g.scanner_density(&negative, &k);
        let wrong = ScannerDensity { shift: k.shift.map(|v| v + 0.01), ..k };
        judge.image(&format!("scannerDensity, roll #8 as {name}"), &out, &each(&negative, |p| cpu::scanner_density(p, &k)), &each(&negative, |p| cpu::scanner_density(p, &wrong)));
    }

    let density = each(&negative, |p| cpu::scanner_density(p, &k));
    let mut nudged = f.render.clone();
    for v in &mut nudged.cube_data {
        v[1] += 0.002;
    }
    for (name, image) in [("roll #8 density", density.clone()), ("adversarial", adversarial_image(512, 384))] {
        let out = g.color_cube(&image, &f.render);
        judge.image(&format!("CIColorCube Frontier 27, {name}"), &out, &each(&image, |p| cpu::color_cube(p, &f.render)), &each(&image, |p| cpu::color_cube(p, &nudged)));
    }

    let code = each(&density, |p| cpu::color_cube(p, &f.render));
    let warmth = each(&code, |p| cpu::color_cube(p, &f.warmth_plus));
    let per_count = f.json["looks"]["warmthPlus"]["perCount"].as_f64().unwrap() as f32;
    let contrast = num(s, "scannerFinish.contrast");
    for (name, k) in [
        ("TEMP +3, the frame's Contrast", ScannerOutput { warmth_share: 3.0 / per_count, lift: SCANNER_LIFT, contrast }),
        ("no warmth, no Contrast", ScannerOutput { warmth_share: 0.0, lift: SCANNER_LIFT, contrast: 0.0 }),
        ("negative Contrast", ScannerOutput { warmth_share: 0.1, lift: SCANNER_LIFT, contrast: -0.4 }),
    ] {
        let out = g.scanner_output(&code, &warmth, &k);
        let wrong = ScannerOutput { contrast: k.contrast + 0.02, warmth_share: k.warmth_share + 0.02, ..k };
        judge.image(&format!("scannerOutput {name}"), &out, &pair(&code, &warmth, |c, w| cpu::scanner_output(c, w, &k)), &pair(&code, &warmth, |c, w| cpu::scanner_output(c, w, &wrong)));
    }

    let k = ScannerEncode { root: 2.0 };
    for (name, image) in inputs() {
        let out = g.scanner_encode(&image, &k);
        judge.image(&format!("scannerEncode root 2, {name}"), &out, &each(&image, |p| cpu::scanner_encode(p, &k)), &each(&image, |p| cpu::scanner_encode(p, &ScannerEncode { root: 2.02 })));
    }
    judge.done();
}

#[test]
fn correction_search() {
    let r = roll();
    let Some(g) = gpu("correction_search") else { return };
    let mut judge = Judge::default();
    // A ring planted 96 px right and 64 px down of the hole's: the search must find that offset.
    let mut source = r.positive.clone();
    let (cx, cy) = (500usize, 380usize);
    for y in cy - 60..cy + 60 {
        for x in cx - 60..cx + 60 {
            source.pixels[y * source.width + x] = source.at(x + 96, y + 64);
        }
    }
    let height = source.height as f32;
    let centre = [cx as f32, height - cy as f32];
    let k = CorrectionCost {
        dest_centre: centre,
        half_size: [36.0, 36.0],
        window_origin: [-160.0, -160.0],
        step: 8.0,
        grid_n: 8.0,
        min_radius: 43.2,
        extent_min: [0.0, 0.0],
        extent_max: [source.width as f32, height],
        hole: [centre[0] - 12.0, centre[1] - 12.0, centre[0] + 12.0, centre[1] + 12.0],
    };
    let grid = 41;
    for (name, origin) in [("at the origin", [0.0, 0.0]), ("displaced extent", [10.0, 20.0])] {
        let placed = Image { origin, ..source.clone() };
        let shift = |p: [f32; 2]| [p[0] + origin[0], p[1] + origin[1]];
        let k = CorrectionCost {
            dest_centre: shift(k.dest_centre),
            extent_min: shift(k.extent_min),
            extent_max: shift(k.extent_max),
            hole: [k.hole[0] + origin[0], k.hole[1] + origin[1], k.hole[2] + origin[0], k.hole[3] + origin[1]],
            ..k
        };
        let out = g.correction_cost(&placed, &k, grid);
        let reference = cpu::correction_cost(&placed, &k, grid);
        let wrong = CorrectionCost { half_size: k.half_size.map(|v| v * 1.02), ..k };
        judge.image(&format!("correctionCost coarse, {name}"), &out, &reference, &cpu::correction_cost(&placed, &wrong, grid));
        let best = out.pixels.iter().filter(|p| p[0].is_finite()).min_by(|a, b| a[0].total_cmp(&b[0])).copied();
        println!("best coarse candidate {best:?}");
        match best {
            Some(b) if (b[1] - 96.0).abs() <= 8.0 && (b[2] + 64.0).abs() <= 8.0 => {}
            other => judge.fail(format!("{name}: the planted copy at (96, -64) was not found, best {other:?}")),
        }
        let excluded = out.pixels.iter().filter(|p| p[0].is_infinite()).count();
        if excluded == 0 || excluded == out.pixels.len() {
            judge.fail(format!("{name}: {excluded} candidates excluded, one branch unexercised"));
        }
    }

    let fine = CorrectionCost { half_size: [30.0, 30.0], window_origin: [96.0 - 16.0, -64.0 - 16.0], step: 1.0, grid_n: 30.0, min_radius: 36.0, ..k };
    let out = g.correction_cost(&source, &fine, 33);
    let reference = cpu::correction_cost(&source, &fine, 33);
    let wrong = CorrectionCost { half_size: [30.6, 30.6], ..fine };
    judge.image("correctionCost fine, step 1", &out, &reference, &cpu::correction_cost(&source, &wrong, 33));
    // Offsets sit at texel centres, so the exact copy falls half a step from the nearest candidate.
    let best = out.pixels.iter().min_by(|a, b| a[0].total_cmp(&b[0])).unwrap();
    println!("best fine candidate {best:?}");
    if (best[1] - 96.0).abs() != 0.5 || (best[2] + 64.0).abs() != 0.5 {
        judge.fail(format!("the fine pass missed the planted copy at (96, -64): {best:?}"));
    }

    let patch = Image { origin: [300.0, 200.0], ..crop(&r.positive, 300, 300, 200, 150) };
    let k = CorrectionBiasField { base: [0.01, -0.02, 0.005], grad_x: [1e-4, -2e-4, 5e-5], grad_y: [-1e-4, 1e-4, 2e-4], origin: [400.0, 275.0] };
    let wrong = CorrectionBiasField { grad_x: k.grad_x.map(|v| v * 1.5), ..k };
    judge.image("correctionBiasField, roll #8 patch", &g.correction_bias_field(&patch, &k), &cpu::correction_bias_field(&patch, &k), &cpu::correction_bias_field(&patch, &wrong));
    judge.done();
}

#[test]
fn per_pixel_builtins() {
    let Some(g) = gpu("per_pixel_builtins") else { return };
    let mut judge = Judge::default();
    for (name, image) in inputs() {
        let k = ColorMatrix { bias_vector: [0.01, -0.02, 0.0, 0.0], ..luma_only() };
        let wrong = ColorMatrix { r_vector: [0.2126, 0.7152, 0.0722, 0.0], ..k };
        judge.image(&format!("CIColorMatrix grey trace, {name}"), &g.color_matrix(&image, &k), &each(&image, |p| cpu::color_matrix(p, &k)), &each(&image, |p| cpu::color_matrix(p, &wrong)));

        let k = ColorClamp::default();
        let wrong = ColorClamp { max_components: [1.0, 1.0, 1.01, 1.0], min_components: [0.0, -0.01, 0.0, 0.0] };
        judge.image(&format!("CIColorClamp, {name}"), &g.color_clamp(&image, &k), &each(&image, |p| cpu::color_clamp(p, &k)), &each(&image, |p| cpu::color_clamp(p, &wrong)));

        let k = ExposureAdjust { ev: 0.321_928 };
        judge.image(&format!("CIExposureAdjust, {name}"), &g.exposure_adjust(&image, &k), &each(&image, |p| cpu::exposure_adjust(p, &k)), &each(&image, |p| cpu::exposure_adjust(p, &ExposureAdjust { ev: 0.33 })));
    }

    let r = roll();
    let (w, h) = (r.positive.width, r.positive.height);
    let soft = RadialGradient { center: [400.0, 300.0], radius0: 40.0, radius1: 140.0, color0: [1.0; 4], color1: [0.0, 0.0, 0.0, 0.0] };
    let mask = cpu::generated([0.0; 2], w, h, |at| cpu::radial_gradient(at, &soft));
    let background = adversarial_image(w, h);
    let out = g.blend_with_mask(&r.positive, &background, &mask);
    let blend = |m: &Image| {
        let pixels = (0..w * h).map(|i| cpu::blend_with_mask(r.positive.pixels[i], background.pixels[i], m.pixels[i])).collect();
        r.positive.with_pixels(pixels)
    };
    let shifted = cpu::generated([3.0, 0.0], w, h, |at| cpu::radial_gradient(at, &soft));
    judge.image("CIBlendWithMask, roll #8 over the ring", &out, &blend(&mask), &blend(&shifted));

    let veiled = pair(&r.positive, &mask, |p, m| p.map(|v| v * m[3]));
    let out = g.source_over(&veiled, &background);
    let reference = pair(&veiled, &background, cpu::source_over);
    let twin = pair(&veiled, &background, |a, b| std::array::from_fn(|c| a[c] + b[c] * (1.0 - a[1])));
    judge.image("composited(over:), premultiplied veil", &out, &reference, &twin);

    for origin in [[0.0, 0.0], [-20.0, 33.0]] {
        let k = LinearGradient { point0: [10.0, 20.0], point1: [300.0, 150.0], color0: [0.1, 0.2, 0.3, 1.0], color1: [0.9, 0.5, 0.0, 1.0] };
        let wrong = LinearGradient { point1: [301.0, 150.0], ..k };
        judge.image(&format!("CILinearGradient at {origin:?}"), &g.linear_gradient(origin, 320, 200, &k), &cpu::generated(origin, 320, 200, |at| cpu::linear_gradient(at, &k)), &cpu::generated(origin, 320, 200, |at| cpu::linear_gradient(at, &wrong)));
        let k = RadialGradient { center: [150.0, 90.0], radius0: 20.0, radius1: 120.0, color0: [1.0, 1.0, 1.0, 1.0], color1: [0.0, 0.1, 0.2, 1.0] };
        let wrong = RadialGradient { radius1: 121.0, ..k };
        judge.image(&format!("CIRadialGradient at {origin:?}"), &g.radial_gradient(origin, 320, 200, &k), &cpu::generated(origin, 320, 200, |at| cpu::radial_gradient(at, &k)), &cpu::generated(origin, 320, 200, |at| cpu::radial_gradient(at, &wrong)));
    }
    judge.done();
}

#[test]
fn area_reductions() {
    let r = roll();
    let Some(g) = gpu("area_reductions") else { return };
    let mut judge = Judge::default();
    let rects = [
        Rect { x: 100, y: 200, width: 64, height: 48 },
        Rect { x: 0, y: 500, width: 1024, height: 3 },
        Rect { x: 0, y: 0, width: 1024, height: 768 },
    ];
    for rect in rects {
        // A bright last row and a unique peak on the last pixel: a rectangle one short of its edge misses both.
        let mut image = r.positive.clone();
        let last = rect.y + rect.height - 1;
        for x in rect.x..rect.x + rect.width {
            image.pixels[last * image.width + x] = [2.0, 2.0, 2.0, 1.0];
        }
        image.pixels[last * image.width + rect.x + rect.width - 1] = [2.5, 3.5, 4.5, 1.0];
        let (mean, highest) = g.area_average_and_maximum(&image, rect);
        let short = Rect { height: rect.height - 1, width: rect.width - 1, ..rect };
        judge.check(&format!("CIAreaAverage {rect:?}"), &[mean], &[cpu::area_average(&image, rect)], &[cpu::area_average(&image, short)], 1e-5);
        judge.check(&format!("CIAreaMaximum {rect:?}"), &[highest], &[cpu::area_maximum(&image, rect)], &[cpu::area_maximum(&image, short)], 0.0);
    }
    judge.done();
}

#[test]
fn area_histogram() {
    let r = roll();
    let Some(g) = gpu("area_histogram") else { return };
    let mut judge = Judge::default();
    let bins = 1024;
    let k = AreaHistogram { count: bins, scale: 1.0 };
    // `histogramEdgeLaw`, measured on Core Image: under 0 and past 1 land in the end bins, 0.5 in the middle.
    for (v, bin) in [(-0.5, 0), (1.0, bins - 1), (1.5, bins - 1), (0.5, bins / 2)] {
        let flat = Image::new(8, 8, vec![[v, v, v, 1.0]; 64]);
        let shares = g.area_histogram(&flat, Rect { x: 0, y: 0, width: 8, height: 8 }, &k);
        if shares[bin][0] != 1.0 || shares[bin][1] != 1.0 {
            judge.fail(format!("a flat {v} did not land whole in bin {bin}: {:?}", shares[bin]));
        }
    }
    // A NaN takes bin 0, where the reference's fmax sends it; Core Image's own law for it is unmeasured.
    let whole = Rect { x: 0, y: 0, width: 8, height: 8 };
    let nan = Image::new(8, 8, vec![[f32::NAN, 0.5, f32::NAN, 1.0]; 64]);
    let reference = cpu::area_histogram(&nan, whole, &k);
    let mut twin = reference.clone();
    for c in [0, 2] {
        twin[0][c] = 0.0;
        twin[bins - 1][c] = 1.0;
    }
    judge.check("CIAreaHistogram, NaN", &g.area_histogram(&nan, whole, &k), &reference, &twin, 0.0);
    let images = [("roll #8 positive", r.positive.clone()), ("roll #8 negative", r.negative.clone()), ("adversarial", adversarial_image(1024, 768))];
    for (name, image) in images {
        for rect in [Rect { x: 0, y: 0, width: image.width, height: image.height }, Rect { x: 101, y: 57, width: 333, height: 211 }] {
            let n = rect.width * rect.height;
            let shares = g.area_histogram(&image, rect, &k);
            let counts: Vec<[u64; 4]> = shares.iter().map(|s| s.map(|v| (v as f64 * n as f64).round() as u64)).collect();
            let reference = cpu::histogram_counts(&image, rect, bins);
            // The twin rounds to the nearer bin edge instead of flooring.
            let nudged = Image { pixels: image.pixels.iter().map(|p| p.map(|v| v + 0.5 / bins as f32)).collect(), ..image.clone() };
            let twin = cpu::histogram_counts(&nudged, rect, bins);
            let wrong = counts.iter().zip(&reference).filter(|(a, b)| a != b).count();
            let twin_wrong = counts.iter().zip(&twin).filter(|(a, b)| a != b).count();
            let total: u64 = counts.iter().map(|c| c[1]).sum();
            println!("CIAreaHistogram {name} {rect:?}: {wrong} bins differ, twin {twin_wrong}, {total} of {n} counted");
            if wrong != 0 || total != n as u64 {
                judge.fail(format!("{name} {rect:?}: {wrong} bins differ from the reference, {total} of {n} counted"));
            }
            if twin_wrong == 0 {
                judge.fail(format!("{name} {rect:?}: the rounding twin matches, the check cannot see a bin edge"));
            }
            let share_ref = cpu::area_histogram(&image, rect, &k);
            let (err, _) = common::error(&shares, &share_ref);
            if err > 1e-6 {
                judge.fail(format!("{name}: shares off by {err:e}"));
            }
        }
    }
    judge.done();
}
