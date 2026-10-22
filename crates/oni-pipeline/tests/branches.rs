//! The kernel branches `stages.rs` leaves unreached, on the render graph against oni-render's scalar
//! reference: each verdict has a twin, a reference deliberately wrong, which the check must refuse.

mod common;

use common::{V4, error, graph, judge, mapped, noise, whole};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::stages::{self, DetailColumns, DetailRows};
use oni_pipeline::scanner;
use oni_render::cpu;
use oni_render::{Curves, Detail, FlatField, LUT_SIZE, SaturationCurve, ScannerDensity, ScannerOutput, Spectrogram, Stage47, Texture};
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use oni_settings::{ConversionMode, PipelineSettings};

/// Every mix of the values a kernel bends at: below zero, zero, a denormal's neighbour, one, past one.
fn edges() -> (u32, u32, Vec<V4>) {
    let edges = [-0.25, 0.0, 1e-7, 1.0, 1.5, 3.0];
    let pixels = (0..216).map(|i| [edges[i % 6], edges[i / 6 % 6], edges[i / 36], 1.0]).collect();
    (18, 12, pixels)
}

/// Transmittances a negative scan holds, from the densest highlight to just past the light panel.
fn transmittances(n: usize) -> Vec<V4> {
    let mut state = 0x2545_f491_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    (0..n)
        .map(|_| {
            let base = 10f32.powf(-2.6 * next());
            [base * (0.6 + 0.6 * next()), base * (0.5 + 0.5 * next()), base * (0.3 + 0.5 * next()), 1.0]
        })
        .collect()
}

/// Stages 4 to 7 on settings that reach every branch of the kernel, one family at a time.
fn stage47_cases() -> Vec<(&'static str, Stage47)> {
    let rest = Stage47::rest();
    let gains = Stage47 {
        gain: [2.1, 1.3, 0.9],
        red: [0.2, 1.6, 1.0],
        green: [0.25, 1.7, 0.95],
        blue: [0.3, 1.9, 1.05],
        pedestal: 0.001,
        log_guard: 1e-5,
        ..rest
    };
    let windows = Stage47 { low_points: [0.3, 0.22, 0.27], high_points: [0.7, 0.8, 0.74], ..gains };
    let decompressed =
        Stage47 { decompression: 0.7, film_share: 0.1, balance_low: [0.2, 0.25, 0.3], balance_span: [1.4, 1.45, 1.6], ..windows };
    let separated = Stage47 { sep_r: [1.2, -0.15, -0.05], sep_g: [-0.1, 1.25, -0.15], sep_b: [-0.05, -0.2, 1.25], ..decompressed };
    let printed = Stage47 {
        print_on: 1.0,
        print_stops: 1.5,
        print_contrast: 0.3,
        print_zones: [0.2, -0.4],
        own_stops: [0.1, -0.05, -0.08],
        hunt: 0.4,
        base_stops: 1.5,
        follow: 0.5,
        ..separated
    };
    let dense = Stage47 { density: 1.3, ..printed };
    let positive = Stage47 { invert: 0.0, window: 3.0, ..dense };
    let flattened = Stage47 { print_contrast: -0.5, print_zones: [0.6, -0.7], ..dense };
    vec![
        ("rest", rest),
        ("gains and levels", gains),
        ("window handles", windows),
        ("chroma decompression", decompressed),
        ("dye separation", separated),
        ("print, filters, Hunt, follow", printed),
        ("colour density", dense),
        ("positive mode", positive),
        ("negative contrast, double quarters", flattened),
    ]
}

#[test]
fn stage47_reaches_every_branch() {
    let Some(mut g) = graph("stage47_reaches_every_branch") else { return };
    let pixels = transmittances(256 * 256);
    let film = g.upload(256, 256, &pixels).unwrap();
    for (name, k) in stage47_cases() {
        let out = whole(&mut g, &Image::new(&film).then(stages::stage47(&k)));
        // A 1 % green gain the reference does not see must break the tolerance.
        let shifted = Stage47 { gain: [k.gain[0], k.gain[1] * 1.01, k.gain[2]], ..k };
        judge(&format!("stage47, {name}"), &out, &mapped(&pixels, |p| cpu::stage47(p, &k)), &mapped(&pixels, |p| cpu::stage47(p, &shifted)));
    }
}

#[test]
fn the_output_clip_holds_nan_and_infinities_as_metal_does() {
    let Some(mut g) = graph("the_output_clip_holds_nan_and_infinities_as_metal_does") else { return };
    // Metal's clamp is fmin(fmax(x, 0), 1): a NaN leaves as the low bound, an infinity as its end.
    let specials = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30, -0.0, 0.5];
    let pixels: Vec<V4> = (0..343).map(|i| [specials[i % 7], specials[i / 7 % 7], specials[i / 49], 1.0]).collect();
    let input = g.upload(49, 7, &pixels).unwrap();
    let out = whole(&mut g, &Image::new(&input).then(stages::output_clip()));
    let twin = mapped(&pixels, |p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), p[3]]);
    judge("output clip, NaN and infinities", &out, &mapped(&pixels, cpu::output_clip), &twin);
}

/// A two-input stage on `a` reading `b` at the same pixel, against `f` and refusing `twin`.
fn two(g: &mut Graph, name: &str, (a, pa): (&Source, &[V4]), (b, pb): (&Source, &[V4]), stage: stages::PerPixel, f: impl Fn(V4, V4) -> V4, twin: impl Fn(V4, V4) -> V4) {
    let out = whole(g, &Image::new(a).then_reading(stage, &[Image::new(b)]));
    let each = |f: &dyn Fn(V4, V4) -> V4| pa.iter().zip(pb).map(|(&p, &q)| f(p, q)).collect::<Vec<_>>();
    judge(name, &out, &each(&f), &each(&twin));
}

#[test]
fn flat_field_per_channel_at_rest_and_under_its_floor() {
    let Some(mut g) = graph("flat_field_per_channel_at_rest_and_under_its_floor") else { return };
    let (w, h, film) = edges();
    let (mask, floored) = (noise(w, h, 21, 0.5, 1.0), noise(w, h, 22, -0.2, 0.05));
    let (a, m, f) = (g.upload(w, h, &film).unwrap(), g.upload(w, h, &mask).unwrap(), g.upload(w, h, &floored).unwrap());
    let cases = [
        ("passes, tinted", FlatField { amount: 0.7, exponent: [1.1, 1.0, 0.9], per_channel: 1.0 }, (&m, &mask)),
        ("identity at amount 0", FlatField { amount: 0.0, exponent: [1.2, 1.0, 0.8], per_channel: 1.0 }, (&m, &mask)),
        ("mask under the floor", FlatField { amount: 1.0, exponent: [1.0; 3], per_channel: 1.0 }, (&f, &floored)),
    ];
    for (name, k, (source, pixels)) in cases {
        let wrong = FlatField { amount: k.amount + 0.02, exponent: [k.exponent[0], k.exponent[1] * 1.05, k.exponent[2]], ..k };
        two(&mut g, &format!("flat-field, {name}"), (&a, &film), (source, pixels), stages::flat_field(&k), |s, m| cpu::flat_field(s, m, &k), |s, m| cpu::flat_field(s, m, &wrong));
    }
}

/// Tables as `CurveSet.rgbLUT` packs them: linked, red, green, blue; luma apart.
fn tables(offset: f32) -> (Vec<V4>, Vec<V4>) {
    let n = LUT_SIZE;
    let at = |i: usize| i as f32 / (n - 1) as f32;
    let rgb = (0..n)
        .map(|i| {
            let x = at(i);
            let s = x * x * (3.0 - 2.0 * x);
            [0.3 * x + 0.7 * s + offset, x.powf(0.9) + offset, x.powf(1.05) + offset, x.powf(1.15) + offset]
        })
        .collect();
    let luma = (0..n).map(|i| [at(i).powf(0.8) + offset, 0.0, 0.0, 1.0]).collect();
    (rgb, luma)
}

#[test]
fn curves_bend_each_family_on_values_past_their_ends() {
    let Some(mut g) = graph("curves_bend_each_family_on_values_past_their_ends") else { return };
    let (w, h, pixels) = edges();
    let input = g.upload(w, h, &pixels).unwrap();
    let ((rgb, luma), (rgb_off, luma_off)) = (tables(0.0), tables(0.002));
    let n = LUT_SIZE as u32;
    let (rgb_table, luma_table) = (g.upload(n, 1, &rgb).unwrap(), g.upload(n, 1, &luma).unwrap());
    for bent in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0]] {
        let k = Curves { rgb_lut: rgb.clone(), luma_lut: luma.clone(), bent };
        let wrong = Curves { rgb_lut: rgb_off.clone(), luma_lut: luma_off.clone(), bent };
        let image = Image::new(&input).then_reading(stages::curves(&k), &[Image::new(&rgb_table), Image::new(&luma_table)]);
        judge(&format!("curves {bent:?}"), &whole(&mut g, &image), &mapped(&pixels, |p| cpu::curves(p, &k)), &mapped(&pixels, |p| cpu::curves(p, &wrong)));
    }
}

#[test]
fn saturation_texture_and_scanner_reach_their_other_branches() {
    let Some(mut g) = graph("saturation_texture_and_scanner_reach_their_other_branches") else { return };
    let (w, h, pixels) = edges();
    let input = g.upload(w, h, &pixels).unwrap();

    let k = SaturationCurve { knots: [0.2, 0.5, 0.8], lows: [0.6, 0.7, 0.5, 0.45], high: 0.5, bends: [0.0; 3] };
    let wrong = SaturationCurve { lows: k.lows.map(|v| v + 0.01), high: k.high + 0.01, ..k };
    let out = whole(&mut g, &Image::new(&input).then(stages::saturation(&k)));
    judge("saturation, lifted shadows", &out, &mapped(&pixels, |p| cpu::saturation_curve(p, &k)), &mapped(&pixels, |p| cpu::saturation_curve(p, &wrong)));

    let (narrow, coeff) = (noise(w, h, 23, 0.0, 1.2), noise(w, h, 24, -0.3, 1.1));
    let (b, c) = (g.upload(w, h, &narrow).unwrap(), g.upload(w, h, &coeff).unwrap());
    let gain = -0.8 / 0.82;
    let out = whole(&mut g, &Image::new(&input).then_reading(stages::texture(gain), &[Image::new(&b), Image::new(&c)]));
    let three = |k: Texture| (0..pixels.len()).map(|i| cpu::texture(pixels[i], narrow[i], coeff[i], &k)).collect::<Vec<_>>();
    judge("texture, negative gain", &out, &three(Texture { gain }), &three(Texture { gain: gain + 0.05 }));

    let mut frontier = PipelineSettings::rest(ConversionMode::Negative, true);
    frontier.face = Face::Scanner;
    frontier.scanner_machine = ScannerMachine::Frontier;
    frontier.scanner = ScannerClicks::new(1, -2, 3, 0, 0);
    let slide = ScannerDensity { invert: 0.0, ..scanner::fitted_density(&frontier) };
    let film = transmittances((w * h) as usize);
    let source = g.upload(w, h, &film).unwrap();
    let out = whole(&mut g, &Image::new(&source).then(stages::scanner_density(&slide)));
    let wrong = ScannerDensity { shift: slide.shift.map(|v| v + 0.01), ..slide };
    judge("scanner density, a slide", &out, &mapped(&film, |p| cpu::scanner_density(p, &slide)), &mapped(&film, |p| cpu::scanner_density(p, &wrong)));

    let warmth = noise(w, h, 25, 0.0, 1.0);
    let warm = g.upload(w, h, &warmth).unwrap();
    for (name, k) in [
        ("no warmth, no Contrast", ScannerOutput { warmth_share: 0.0, lift: scanner::LIFT, contrast: 0.0 }),
        ("negative Contrast", ScannerOutput { warmth_share: 0.1, lift: scanner::LIFT, contrast: -0.4 }),
    ] {
        let wrong = ScannerOutput { contrast: k.contrast + 0.02, warmth_share: k.warmth_share + 0.02, ..k };
        let stage = stages::scanner_output(k.warmth_share, k.lift, k.contrast);
        two(&mut g, &format!("scanner output, {name}"), (&input, &pixels), (&warm, &warmth), stage, |c, m| cpu::scanner_output(c, m, &k), |c, m| cpu::scanner_output(c, m, &wrong));
    }
}

/// Saturated colours all around the hue circle, at three levels.
fn hue_wheel() -> Vec<V4> {
    (0..3)
        .flat_map(|y| (0..360).map(move |x| (x, y)))
        .map(|(x, y)| {
            let a = (x as f32).to_radians();
            let level = [0.05, 0.3, 0.8][y];
            [level * (1.0 + 0.8 * a.cos()), level * (1.0 + 0.8 * (a - 2.094).cos()), level * (1.0 + 0.8 * (a + 2.094).cos()), 1.0]
        })
        .collect()
}

#[test]
fn the_spectrogram_turns_every_band_around_the_hue_circle() {
    let Some(mut g) = graph("the_spectrogram_turns_every_band_around_the_hue_circle") else { return };
    let pixels = hue_wheel();
    let input = g.upload(360, 3, &pixels).unwrap();
    let scaled = |f: f32| Spectrogram {
        hue_low: [12.0, -8.0, 0.0, 20.0].map(|v| v * f),
        hue_high: [-15.0, 5.0, 0.0, -30.0].map(|v| v * f),
        sat_low: [0.3, -0.4, 0.0, 0.5].map(|v| v * f),
        sat_high: [-0.6, 0.2, 1.0, 0.0].map(|v| v * f),
        luma_low: [0.2, 0.0, -0.3, 0.1].map(|v| v * f),
        luma_high: [0.0, -0.5, 0.4, 0.0].map(|v| v * f),
    };
    let (k, wrong) = (scaled(1.0), scaled(1.05));
    let out = whole(&mut g, &Image::new(&input).then(stages::spectrogram(&k)));
    judge("spectrogram, hue wheel", &out, &mapped(&pixels, |p| cpu::spectrogram(p, &k)), &mapped(&pixels, |p| cpu::spectrogram(p, &wrong)));
}

/// Detail through its two stages, as the pipeline lays them.
fn detailed(g: &mut Graph, source: &Source, k: Detail) -> Vec<V4> {
    let image = Image::new(source);
    let rows = image.clone().then(DetailRows(k));
    whole(g, &image.then_reading(DetailColumns(k), &[rows]))
}

#[test]
fn detail_leaves_a_flat_grey_and_clamps_every_tap_under_its_reach() {
    let Some(mut g) = graph("detail_leaves_a_flat_grey_and_clamps_every_tap_under_its_reach") else { return };
    // The taps sum to one, so a flat grey leaves untouched: no chroma to denoise, no edge to sharpen.
    let k = Detail::dosed(Some((4.0, 0.6)), Some((2.092, 1.0, 0.5))).unwrap();
    let flat = vec![[0.18, 0.18, 0.18, 1.0]; 64 * 48];
    let source = g.upload(64, 48, &flat).unwrap();
    let out = detailed(&mut g, &source, k);
    let brighter = mapped(&flat, |p| [p[0] * 1.0001, p[1] * 1.0001, p[2] * 1.0001, 1.0]);
    let (e, t) = (error(&out, &flat), error(&out, &brighter));
    println!("detail, flat grey: error {e:.2e}, twin {t:.2e}");
    assert!(e < 2e-6 && t > 2e-6, "detail moves a flat grey by {e:.2e}, or its twin passes at {t:.2e}");

    // Narrower than its reach: every tap clamps, on both sides and both passes.
    let (_, _, edges) = edges();
    let tiny: Vec<V4> = edges.into_iter().take(15).collect();
    let k = Detail::dosed(Some((4.0, 0.6)), Some((2.5, 1.0, 0.7))).unwrap();
    let source = g.upload(5, 3, &tiny).unwrap();
    let out = detailed(&mut g, &source, k);
    let picture = oni_render::Image::new(5, 3, tiny);
    judge("detail, 5x3 under a reach of 14", &out, &cpu::detail(&picture, &k).pixels, &cpu::detail(&picture, &Detail { force: 0.7, ..k }).pixels);
}
