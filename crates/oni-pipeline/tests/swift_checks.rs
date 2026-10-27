//! The Swift app's self-checks that render, each on its own adversarial values and with its twin,
//! a variant built to fail when the property it guards is broken.

use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request, conversion, finishing};
use oni_render::cpu;
use oni_settings::axis::{density_axis, pipeline};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PipelineSettings};

type V3 = [f32; 3];

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

/// `Pipeline.apply` on a flat `side`-square frame of `rgb`, read at its centre.
fn rendered(g: &mut Graph, rgb: V3, side: u32, s: &PipelineSettings) -> V3 {
    let pixels = vec![[rgb[0], rgb[1], rgb[2], 1.0]; (side * side) as usize];
    let source = g.upload(side, side, &pixels).expect("the frame uploads");
    let at = Rect { x: (side / 2) as i32, y: (side / 2) as i32, w: 1, h: 1 };
    let request = Request { region: Some(at), ..Request::default() };
    let (_, px) = Pipeline::new().render(g, &source, s, &request, &Inputs::default(), None).expect("the frame renders");
    [px[0][0], px[0][1], px[0][2]]
}

/// Stages 4 to 7 alone, the fused kernel's scalar reference.
fn stage47(rgb: V3, s: &PipelineSettings) -> V3 {
    let out = cpu::stage47([rgb[0], rgb[1], rgb[2], 1.0], &conversion::stage47(s));
    [out[0], out[1], out[2]]
}

fn spread(v: V3) -> f32 {
    v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min)
}

fn worst(a: V3, b: V3) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

fn luma(v: V3) -> f32 {
    pipeline::luma(v)
}

// MARK: - DensityAxis.selfCheck

/// `DensityAxis.rendered`: on the bare axis, decompression whole, no unmix, the RGB black at zero.
fn on_axis(rgb: V3, mode: ConversionMode, ceiling: f32) -> V3 {
    let mut s = PipelineSettings { mode, ..PipelineSettings::default() };
    s.chroma_decompression = 1.0;
    s.levels = LevelsSet { red: Levels::on_axis(), green: Levels::on_axis(), blue: Levels::on_axis() };
    s.density_ceiling = ceiling;
    s.separation = 0.0;
    s.print.black_point = 0.0;
    stage47(rgb, &s)
}

/// The signed step that disproves the mode's own direction first, over sixteen steps of the range.
fn slope(rgb: V3, mode: ConversionMode) -> f32 {
    let range = density_axis::range();
    let (low, high) = (*range.start(), *range.end());
    let up = density_axis::brightens(mode);
    let mut worst = if up { f32::MAX } else { f32::MIN };
    let mut previous = on_axis(rgb, mode, low);
    for step in 1..=16 {
        let next = on_axis(rgb, mode, low + (high - low) * step as f32 / 16.0);
        for c in 0..3 {
            let delta = next[c] - previous[c];
            worst = if up { worst.min(delta) } else { worst.max(delta) };
        }
        previous = next;
    }
    worst
}

const DECADE_PAIR: V3 = [0.20, 0.01, 0.001];
/// The widest pedestal is a sixtieth of 0.5, so no ceiling of the range opens this pair by half.
const BRIGHT_PAIR: V3 = [0.20, 0.50, 0.20];

#[test]
fn density_axis_sense_is_measured_in_every_mode() {
    let mut measured = Vec::new();
    for mode in ConversionMode::ALL {
        let step = slope(DECADE_PAIR, mode);
        let up = density_axis::brightens(mode);
        println!("[{}] worst signed step {step:+.6}", mode.raw_value());
        // A slide renders on the axis' base whatever its ceiling (owner's call).
        let held = mode == ConversionMode::Positive;
        assert!(if held { step == 0.0 } else if up { step > 0.0 } else { step < 0.0 }, "[{}] the ceiling moves the render against its sense: {step:+.6}", mode.raw_value());
        measured.push((mode, step));
    }
    let of = |m| measured.iter().find(|(x, _)| *x == m).map(|(_, s)| *s).unwrap();
    // Twin: the same sweep moves a negative and a black and white frame, a slide's alone stays.
    assert!(of(ConversionMode::Negative) != 0.0 && of(ConversionMode::Monochrome) != 0.0 && of(ConversionMode::Positive) == 0.0);

    let opening = |rgb: V3| {
        let range = density_axis::range();
        let at = |c: f32| {
            let px = on_axis(rgb, ConversionMode::Negative, c);
            (px[1] - px[2]).abs()
        };
        (at(*range.start()), at(*range.end()))
    };
    let (rest, top) = opening(DECADE_PAIR);
    assert!(top > 1.5 * rest, "the range opens a burnt decade {:.2}x, not past 1.5x", top / rest);
    // Twin: a pair far above the pedestal, where no ceiling has purchase.
    let (rest, top) = opening(BRIGHT_PAIR);
    assert!(top <= 1.5 * rest, "a pair the pedestal barely reaches opens {:.2}x", top / rest);
}

// MARK: - ConversionMode.selfCheck

#[test]
fn a_monochrome_render_lands_grey_and_a_negative_keeps_its_colour() {
    let Some(mut g) = graph("a_monochrome_render_lands_grey_and_a_negative_keeps_its_colour") else { return };
    let target = [0.40, 0.12, 0.05];
    let at = |g: &mut Graph, mode| rendered(g, target, 8, &PipelineSettings { mode, ..PipelineSettings::default() });
    let grey = spread(at(&mut g, ConversionMode::Monochrome));
    assert!(grey < 1e-6, "a monochrome render keeps {grey:.2e} between its channels");
    // Twin: the same frame as a negative keeps its spread.
    let coloured = spread(at(&mut g, ConversionMode::Negative));
    assert!(coloured > 0.05, "a negative keeps only {coloured:.4} of spread");
}

// MARK: - CurveSet.colourFilterChecks

#[test]
fn the_colour_filters_move_the_render_and_reach_no_gain() {
    let Some(mut g) = graph("the_colour_filters_move_the_render_and_reach_no_gain") else { return };
    let render = |g: &mut Graph, s: &PipelineSettings| {
        let mut m = s.per_pixel_only();
        m.chroma_decompression = 1.0;
        m.roll_reference = None;
        m.print.bypassed = false;
        rendered(g, [0.30, 0.12, 0.05], 2, &m)
    };
    let mut converted = PipelineSettings::default();
    converted.gains.stops = [0.83, 0.0, -1.42];
    let written = converted.gains;
    // Temperature then Tint at the top of their tracks, each moved along its own axis from rest.
    let mut filtered = converted.clone();
    let lay = |f: &mut Print, by: f32, axis: V3| {
        let now = f.filters();
        f.set_filters(std::array::from_fn(|c| now[c] + by * axis[c]));
    };
    lay(&mut filtered.print, Print::TEMPERATURE_RANGE.1, Print::TEMPERATURE_AXIS);
    lay(&mut filtered.print, Print::TINT_RANGE.1, Print::TINT_AXIS);
    let (tinted, bare) = (render(&mut g, &filtered), render(&mut g, &converted));
    let moved = worst(tinted, bare);
    assert!(filtered.gains == written && moved > 0.01, "the filters move the render by {moved:.4} and leave the gains");
    // Twin: wired onto the conversion, the same gesture would move a gain.
    let mut recoupled = converted.clone();
    recoupled.gains.stops[1] = 1.0;
    assert_ne!(recoupled.gains, written);
    // A sidecar carrying the three filters reads them back unchanged.
    let text = oni_settings::codec::encode_settings(&filtered).unwrap();
    let read = oni_settings::codec::decode(text.as_bytes()).unwrap();
    assert_eq!(read.settings.print, filtered.print);
}

// MARK: - ChromaDecompression.selfCheck

/// `ChromaDecompression.window`: one window's ends, gamma and two added points on a density.
fn window(e: f32, l: &Levels) -> f32 {
    let t = ((e - l.black) / (l.white - l.black).max(1e-4)).max(0.0).powf(l.gamma());
    let (c1, c3) = (l.shadow_ordinate(), l.highlight_ordinate());
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.min(1.0);
    let v = 1.0 - u;
    let curve = c1 * 4.0 * u * v * v * v + 0.5 * 6.0 * u * u * v * v + c3 * 4.0 * u * u * u * v + u * u * u * u;
    curve + (t - u)
}

/// `ChromaDecompression.applied`, the replica the kernel is judged by.
fn applied(d: V3, set: &LevelsSet, dose: f32) -> V3 {
    let windows = set.windows();
    let full: V3 = std::array::from_fn(|c| window(d[c], &windows[c]));
    let film = 1.0 - dose;
    if dose == 1.0 && film == 0.0 {
        return full;
    }
    let low: V3 = std::array::from_fn(|c| windows[c].black);
    let span: V3 = std::array::from_fn(|c| conversion::signed_span(&windows[c]));
    let place: V3 = std::array::from_fn(|c| (d[c] - low[c]) / span[c]);
    let mean = (place[0] + place[1] + place[2]) / 3.0;
    let neutral: V3 = std::array::from_fn(|c| low[c] + mean * span[c]);
    let toned: V3 = std::array::from_fn(|c| window(neutral[c], &windows[c]));
    let held: V3 = std::array::from_fn(|c| toned[c] + dose * (full[c] - toned[c]) + film * (d[c] - neutral[c]));
    let lift = luma(full) - luma(held);
    held.map(|x| x + lift)
}

/// A thin negative on bent curves, every handle off a round value, on a mosaic's wide axis.
fn thin() -> PipelineSettings {
    let mut s = PipelineSettings { separation: 0.0, density_ceiling: density_axis::MOSAIC_REST, ..PipelineSettings::default() };
    s.levels.red = Levels { black: 0.31, white: 0.78, mid: 0.46, shadows: 0.29, highlights: 0.71 };
    s.levels.green = Levels { black: 0.52, white: 0.99, mid: 0.53, shadows: 0.22, highlights: 0.77 };
    s.levels.blue = Levels { black: 0.83, white: 1.30, mid: 0.49, shadows: 0.27, highlights: 0.74 };
    s
}

/// The transmittance whose density, once the pedestal lifts it, is `d`.
fn source(d: V3, s: &PipelineSettings) -> V3 {
    let pedestal = density_axis::pedestal(s.density_ceiling);
    d.map(|x| 10f32.powf(-x) - pedestal)
}

fn decompressed(g: &mut Graph, d: V3, s: &PipelineSettings, dose: f32, lit: bool) -> V3 {
    let mut m = s.per_pixel_only();
    m.print.bypassed = !lit;
    m.chroma_decompression = dose;
    rendered(g, source(d, s), 2, &m)
}

#[test]
fn the_colour_follows_the_windows_as_far_as_its_dose_says() {
    let Some(mut g) = graph("the_colour_follows_the_windows_as_far_as_its_dose_says") else { return };
    let thin = thin();
    let colour = [0.40, 0.70, 0.93];
    for dose in [0.0, 0.37, 1.0] {
        let (got, expected) = (decompressed(&mut g, colour, &thin, dose, false), applied(colour, &thin.levels, dose));
        assert!(worst(got, expected) < 2e-4, "at {dose} the kernel lands {:.1e} off its replica", worst(got, expected));
    }
    let (full, film) = (decompressed(&mut g, colour, &thin, 1.0, false), decompressed(&mut g, colour, &thin, 0.0, false));
    assert!((luma(film) - luma(full)).abs() < 2e-4, "the light is the windows' at every dose");
    assert!(spread(film) < spread(full) * 0.8, "the film's colour {:.3} is not narrower than the stretch's {:.3}", spread(film), spread(full));
    let grey = [0.31 + 0.41 * 0.47, 0.52 + 0.41 * 0.47, 0.83 + 0.41 * 0.47];
    assert!(worst(decompressed(&mut g, grey, &thin, 0.0, false), decompressed(&mut g, grey, &thin, 1.0, false)) < 2e-4, "a grey is the fixed point");

    // One curve shape on the three windows, then all three narrowed together.
    let mut shaped = thin.clone();
    for w in [&mut shaped.levels.red, &mut shaped.levels.green, &mut shaped.levels.blue] {
        (w.mid, w.shadows, w.highlights) = (0.46, 0.29, 0.71);
    }
    let mut thinner = shaped.clone();
    for w in [&mut thinner.levels.red, &mut thinner.levels.green, &mut thinner.levels.blue] {
        w.white = w.black + 0.6 * (w.white - w.black);
    }
    let gaps = |v: V3| [v[0] - v[1], v[2] - v[1]];
    let moved = |g: &mut Graph, dose| {
        let (a, b) = (gaps(decompressed(g, colour, &thinner, dose, false)), gaps(decompressed(g, colour, &shaped, dose, false)));
        (a[0] - b[0]).abs().max((a[1] - b[1]).abs())
    };
    let (film_moved, stretched_moved) = (moved(&mut g, 0.0), moved(&mut g, 1.0));
    assert!(film_moved < 2e-4, "at 0 the channels' gaps move {film_moved:.1e}");
    // Twin: at a whole dose the same narrowing widens them.
    assert!(stretched_moved > 0.03, "at 1 the channels' gaps move only {stretched_moved:.3}");

    let slide = PipelineSettings { mode: ConversionMode::Positive, ..thin.clone() };
    assert_eq!(decompressed(&mut g, colour, &slide, 0.0, false), decompressed(&mut g, colour, &slide, 1.0, false));

    // A red filter's tint on a grey stands the same at 0 and at 1.
    let mut tinted = thin.clone();
    tinted.print.red.exposure = 0.3;
    let (tint_film, tint_full) = (decompressed(&mut g, grey, &tinted, 0.0, true), decompressed(&mut g, grey, &tinted, 1.0, true));
    assert!(worst(tint_film, tint_full) < 2e-4, "a filter's tint is decompressed: {:.1e}", worst(tint_film, tint_full));
    assert!((tint_full[0] - tint_full[1]).abs() > 0.01, "the twin: the filter lays a tint of {:.3}", tint_full[0] - tint_full[1]);
}

/// `colourFilterChecks`' reading: the windows' own arithmetic on a 2-pixel patch, the print off.
fn windows_only(g: &mut Graph, s: &PipelineSettings, measuring: bool) -> V3 {
    let mut m = s.per_pixel_only();
    m.print.bypassed = true;
    m.chroma_decompression = 1.0;
    m.roll_reference = None;
    let pixels = vec![[0.30, 0.12, 0.05, 1.0]; 4];
    let source = g.upload(2, 2, &pixels).expect("the patch uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: 1, h: 1 }), measuring, ..Request::default() };
    let (_, px) = Pipeline::new().render(g, &source, &m, &request, &Inputs::default(), None).expect("the patch renders");
    [px[0][0], px[0][1], px[0][2]]
}

fn curve(points: &[(f64, f64)]) -> Curve {
    Curve { points: points.iter().map(|&(x, y)| Point::new(x, y)).collect() }
}

#[test]
fn a_retired_colour_curve_renders_as_its_two_passes_did() {
    let Some(mut g) = graph("a_retired_colour_curve_renders_as_its_two_passes_did") else { return };
    let colour = CurveSet {
        linked: curve(&[(0.0, 0.0), (0.3, 0.42), (1.0, 1.0)]),
        luma: curve(&[(0.0, 0.0), (0.5, 0.44), (1.0, 1.0)]),
        ..CurveSet::default()
    };
    let mut hand = PipelineSettings::default();
    hand.manual_curves.linked = curve(&[(0.0, 0.0), (0.2, 0.26), (1.0, 1.0)]);
    hand.manual_curves.contrast = 0.2;
    let mut object: serde_json::Value = serde_json::from_str(&oni_settings::codec::encode_settings(&hand).unwrap()).unwrap();
    object["settings"]["curves"] = serde_json::to_value(&colour).unwrap();
    let back = oni_settings::codec::decode(serde_json::to_string(&object).unwrap().as_bytes()).unwrap().settings;
    let display = |v: V3| v.map(|x| x.max(0.0).powf(1.0 / 2.2));
    let measured = windows_only(&mut g, &hand, true);
    let k = finishing::curves(&colour).expect("the colour curves bake a table");
    let after = cpu::curves([measured[0], measured[1], measured[2], 1.0], &k);
    let former = display([after[0], after[1], after[2]]);
    let now = display(windows_only(&mut g, &back, true));
    let gap = worst(now, former);
    // Twin: dropping the retired curves parts from the two passes.
    let twin = worst(display(measured), former);
    assert!(gap < 4e-3 && twin > 4.0 * gap, "folded {gap:.4} from the two passes, dropped {twin:.4}");
}

#[test]
fn every_curve_moves_the_render_alone_and_a_written_one_renders_as_one_built() {
    let Some(mut g) = graph("every_curve_moves_the_render_alone_and_a_written_one_renders_as_one_built") else { return };
    let bend = curve(&[(0.0, 0.0), (0.3, 0.45), (1.0, 1.0)]);
    let unbent = windows_only(&mut g, &PipelineSettings::default(), false);
    let mut alone = Vec::new();
    for channel in [LevelsChannel::Red, LevelsChannel::Linked, LevelsChannel::Luma] {
        let mut one = PipelineSettings::default();
        *one.manual_curves.get_mut(channel) = bend.clone();
        alone.push((channel.label(), one));
    }
    let mut contrasted = PipelineSettings::default();
    contrasted.manual_curves.contrast = 0.4;
    alone.push(("contrast", contrasted));
    let still: Vec<&str> = alone.iter().filter(|(_, s)| worst(windows_only(&mut g, s, false), unbent) < 1e-3).map(|(n, _)| *n).collect();
    assert!(still.is_empty(), "these curves leave the render where it was: {still:?}");

    let mid = [0.7, 0.35, 0.55];
    let literal = format!(
        r#"{{"version": {}, "settings": {{"manualCurves": {{"red": {{"points": [[0, 0], [0.5, {}], [1, 1]]}}, "green": {{"points": [[0, 0], [0.5, {}], [1, 1]]}}, "blue": {{"points": [[0, 0], [0.5, {}], [1, 1]]}}}}}}}}"#,
        oni_settings::codec::VERSION,
        mid[0],
        mid[1],
        mid[2]
    );
    let mut built = PipelineSettings::default();
    for (channel, y) in [LevelsChannel::Red, LevelsChannel::Green, LevelsChannel::Blue].into_iter().zip(mid) {
        built.manual_curves.get_mut(channel).points = vec![Point::new(0.0, 0.0), Point::new(0.5, y), Point::new(1.0, 1.0)];
    }
    let typed = oni_settings::codec::decode(literal.as_bytes()).unwrap().settings;
    let after = windows_only(&mut g, &typed, false);
    assert_eq!(after, windows_only(&mut g, &built, false), "a hand-written sidecar renders to the bits of one built");
    // Twin: the three keys taken out neutralise the pass.
    let mut object: serde_json::Value = serde_json::from_str(&literal).unwrap();
    for key in ["red", "green", "blue"] {
        object["settings"]["manualCurves"].as_object_mut().unwrap().remove(key);
    }
    let plain = oni_settings::codec::decode(serde_json::to_string(&object).unwrap().as_bytes()).unwrap().settings;
    let gap = worst(after, windows_only(&mut g, &plain, false));
    assert!(plain.manual_curves.touched().is_empty() && gap > 0.01, "dropping the keys moves the pixel by only {gap:.4}");
}

// MARK: - DensityMigration.selfCheck(source:)

/// A literal, so the oracle cannot follow a moving resting ceiling.
const FROZEN_PEDESTAL: f32 = 0.008;
/// Twenty times under half an 8-bit step: a drift this far shows long before a cached thumbnail would.
const FRAME_AGREEMENT: f32 = 1e-4;

/// The frozen kernel: transmittance clamped into `[Tmin, 1]`, density divided by `DMAX`, three levels passes.
fn frozen(source: V3, stops: V3, levels: &LevelsSet) -> V3 {
    let lv = [&levels.red, &levels.green, &levels.blue];
    std::array::from_fn(|i| {
        let d = -((source[i] + FROZEN_PEDESTAL) * stops[i].exp2()).clamp(FROZEN_PEDESTAL / 8.0, 1.0).log10();
        oni_settings::density_migration::apply_levels(d / pipeline::DMAX, lv[i])
    })
}

struct Arm {
    name: &'static str,
    frozen: LevelsSet,
    stops: V3,
    migrating: bool,
}

fn arms() -> [Arm; 3] {
    let rest = LevelsSet { red: Levels::default(), green: Levels::default(), blue: Levels::default() };
    let at = |black, white, mid| Levels { black, white, mid, ..Levels::default() };
    let graded = LevelsSet { red: at(0.0731, 0.8137, 0.4411), green: at(0.1279, 0.7411, 0.5237), blue: at(0.2113, 0.9337, 0.6113) };
    let stops = [0.7331, 0.0, -0.4111];
    [
        Arm { name: "at rest", frozen: rest, stops: [0.0; 3], migrating: true },
        Arm { name: "graded", frozen: graded, stops, migrating: true },
        Arm { name: "the twin, left unmigrated", frozen: graded, stops, migrating: false },
    ]
}

/// `truncated(before: .curves)` with the print bypassed and the decompression whole, as the Swift arm sets it.
fn live(arm: &Arm) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.gains.stops = arm.stops;
    s.density_ceiling = -FROZEN_PEDESTAL.log10();
    s.separation = 0.0;
    s.levels = if arm.migrating { oni_settings::density_migration::migrate(&arm.frozen) } else { arm.frozen };
    s.vignette = Default::default();
    s.chroma = oni_settings::blocks::ChromaDenoise::OFF;
    s.spectrogram = Default::default();
    s.saturation = Default::default();
    s.secret_sauce = 0.0;
    s.texture = Default::default();
    s.sharpen.amount = 0.0;
    s.print.bypassed = true;
    s.chroma_decompression = 1.0;
    s
}

/// The decode at most `side` long, each output pixel the mean of its whole step.
fn reduced(image: &oni_decode::Linear, side: usize) -> (u32, u32, Vec<[f32; 4]>) {
    let step = image.width.max(image.height).div_ceil(side).max(1);
    let (w, h) = (image.width / step, image.height / step);
    let mut out = vec![[0.0f32; 4]; w * h];
    for oy in 0..h {
        for ox in 0..w {
            let mut s = [0.0f64; 3];
            for y in oy * step..(oy + 1) * step {
                for p in &image.pixels[y * image.width + ox * step..y * image.width + (ox + 1) * step] {
                    (0..3).for_each(|c| s[c] += p[c] as f64);
                }
            }
            let n = (step * step) as f64;
            out[oy * w + ox] = [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32, 1.0];
        }
    }
    (w as u32, h as u32, out)
}

/// The worst post-clip gap between the frozen kernel and `rendered`, and between the closed new form and it.
fn gaps(source: &[[f32; 4]], rendered: &[[f32; 4]], arm: &Arm) -> (f32, f32) {
    let migrated = oni_settings::density_migration::migrate(&arm.frozen);
    let ceiling = -FROZEN_PEDESTAL.log10();
    let (mut against, mut residue) = (0.0f32, 0.0f32);
    for (p, out) in source.iter().zip(rendered) {
        let s = [p[0], p[1], p[2]];
        let old = frozen(s, arm.stops, &arm.frozen);
        let new = oni_settings::density_migration::new(
            s,
            arm.stops,
            &migrated,
            ConversionMode::Negative,
            density_axis::pedestal(ceiling),
            density_axis::log_guard(ceiling),
        );
        for c in 0..3 {
            against = against.max((pipeline::clip(old[c]) - pipeline::clip(out[c])).abs());
            residue = residue.max((pipeline::clip(new[c]) - pipeline::clip(out[c])).abs());
        }
    }
    (against, residue)
}

#[test]
fn the_migrated_windows_render_the_frozen_kernel_on_a_real_frame() {
    let check = "the_migrated_windows_render_the_frozen_kernel_on_a_real_frame";
    let mut files: Vec<std::path::PathBuf> = oni_fixtures::raw_dir(check)
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect())
        .unwrap_or_default();
    files.sort();
    files.truncate(1);
    files.extend(oni_fixtures::lumix_dir(check).map(|d| d.join("P1072392.RW2")).filter(|p| p.exists()));
    let Some(mut g) = graph(check) else { return };
    for file in &files {
        let decoded = oni_decode::decode(file, 1.0).expect("decode");
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        // The thumbnail through the live chain on the GPU, the full frame through its scalar reference.
        let (w, h, small) = reduced(&decoded, 320);
        let source = g.upload(w, h, &small).expect("the thumbnail uploads");
        for arm in arms() {
            let (_, out) = Pipeline::new().render(&mut g, &source, &live(&arm), &Request::default(), &Inputs::default(), None).unwrap();
            let (against, residue) = gaps(&small, &out, &arm);
            println!("{name} thumbnail {w} x {h}, {}: frozen {against:.3e}, closed form {residue:.3e}", arm.name);
            if arm.migrating {
                assert!(against <= FRAME_AGREEMENT, "{name} thumbnail, {}: {against:.3e}", arm.name);
                assert!(residue <= FRAME_AGREEMENT, "{name} thumbnail, {}: the kernel's own share {residue:.3e}", arm.name);
            } else {
                // Twin: windows left three times too narrow are a full-scale wrong render.
                assert!(against > 0.5, "{name}: the unmigrated twin refuses only at {against:.4}");
            }
        }
        for arm in arms().iter().filter(|a| a.migrating) {
            let k = conversion::stage47(&live(arm));
            let out: Vec<[f32; 4]> = decoded.pixels.iter().map(|&p| cpu::stage47(p, &k)).collect();
            let (against, _) = gaps(&decoded.pixels, &out, arm);
            println!("{name} full {} x {}, {}: frozen {against:.3e}", decoded.width, decoded.height, arm.name);
            assert!(against <= FRAME_AGREEMENT, "{name} full, {}: {against:.3e}", arm.name);
        }
    }
}

// MARK: - NeighbourhoodProbe

mod probe {
    use oni_settings::axis::pipeline::LUMA_WEIGHTS;

    /// Large enough to carry sky, grain and edges, small enough that sixteen radii stay quick.
    pub const SIDE: usize = 1024;

    /// A centred square of the full-resolution decode: centred, since picking the busiest tile would grade the film.
    pub fn region(full: &oni_decode::Linear) -> (Vec<[f32; 4]>, usize, f64) {
        let cut = SIDE.min(full.width.min(full.height));
        let (x0, y0) = ((full.width - cut) / 2, (full.height - cut) / 2);
        let px = (0..cut).flat_map(|y| full.pixels[(y0 + y) * full.width + x0..(y0 + y) * full.width + x0 + cut].to_vec()).collect();
        (px, cut, full.width.max(full.height) as f64)
    }

    /// Luma per pixel and the three departures from grey, as both stages split them.
    pub fn planes(px: &[[f32; 4]]) -> (Vec<f32>, Vec<f32>) {
        let w = LUMA_WEIGHTS;
        let luma: Vec<f32> = px.iter().map(|p| p[0] * w[0] + p[1] * w[1] + p[2] * w[2]).collect();
        let chroma = px.iter().zip(&luma).flat_map(|(p, &y)| [p[0] - y, p[1] - y, p[2] - y]).collect();
        (luma, chroma)
    }

    /// Flat and edge pixels, cut on the luma gradient's median and top decile, the border in neither.
    pub fn split(luma: &[f32], side: usize) -> (Vec<bool>, Vec<bool>) {
        let mut gradient = vec![0.0f32; side * side];
        for y in 1..side - 1 {
            for x in 1..side - 1 {
                let i = y * side + x;
                gradient[i] = (luma[i + 1] - luma[i - 1]).abs() + (luma[i + side] - luma[i - side]).abs();
            }
        }
        let mut sorted = gradient.clone();
        sorted.sort_by(f32::total_cmp);
        let (median, decile) = (sorted[sorted.len() / 2], sorted[sorted.len() * 9 / 10]);
        let (mut flat, mut edges) = (vec![false; side * side], vec![false; side * side]);
        for y in 1..side - 1 {
            for x in 1..side - 1 {
                let i = y * side + x;
                flat[i] = gradient[i] <= median;
                edges[i] = gradient[i] >= decile;
            }
        }
        (flat, edges)
    }

    /// Mean chroma step between horizontal neighbours; on the flat half it is the noise the stage removes.
    pub fn neighbour_spread(chroma: &[f32], side: usize, mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for y in 0..side {
            for x in 0..side - 1 {
                let i = y * side + x;
                if mask[i] && mask[i + 1] {
                    (0..3).for_each(|c| total += f64::from((chroma[(i + 1) * 3 + c] - chroma[i * 3 + c]).abs()));
                    counted += 3;
                }
            }
        }
        if counted == 0 { 0.0 } else { (total / counted as f64) as f32 }
    }

    /// RMS move of the masked pixels; on edges it is colour carried across a boundary.
    pub fn displacement(before: &[f32], after: &[f32], mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for (i, _) in mask.iter().enumerate().filter(|m| *m.1) {
            (0..3).for_each(|c| total += f64::from(after[i * 3 + c] - before[i * 3 + c]).powi(2));
            counted += 3;
        }
        if counted == 0 { 0.0 } else { (total / counted as f64).sqrt() as f32 }
    }

    /// Mean luma step between horizontal neighbours; on edges it is the contour sharpening buys.
    pub fn step(luma: &[f32], side: usize, mask: &[bool]) -> f32 {
        let (mut total, mut counted) = (0.0f64, 0usize);
        for y in 0..side {
            for x in 0..side - 1 {
                let i = y * side + x;
                if mask[i] && mask[i + 1] {
                    total += f64::from((luma[i + 1] - luma[i]).abs());
                    counted += 1;
                }
            }
        }
        if counted == 0 { 0.0 } else { (total / counted as f64) as f32 }
    }

    /// The band the source holds in a 3 px square window, its edge clamped.
    pub fn morphology(luma: &[f32], side: usize, maximum: bool) -> Vec<f32> {
        let at = |x: i64, y: i64| luma[(y.clamp(0, side as i64 - 1) as usize) * side + x.clamp(0, side as i64 - 1) as usize];
        (0..side * side)
            .map(|i| {
                let (x, y) = ((i % side) as i64, (i / side) as i64);
                let window = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| at(x + dx, y + dy));
                if maximum { window.fold(f32::MIN, f32::max) } else { window.fold(f32::MAX, f32::min) }
            })
            .collect()
    }

    /// The share of a curve's rise past a radius.
    pub fn beyond(sweep: &[f64], curve: &[f32], past: f64) -> f64 {
        let Some(&total) = curve.last().filter(|&&t| t > 0.0 && sweep.len() == curve.len()) else { return 0.0 };
        let here = sweep.iter().position(|&r| r >= past).map_or(curve[curve.len() - 1], |i| curve[i]);
        f64::from(total - here) / f64::from(total)
    }

    /// Geometric from a quarter pixel, sixteen points, sampling the bottom of the range finely.
    pub fn sweep(top: f64) -> Vec<f64> {
        let step = (top / 0.25).powf(1.0 / 15.0);
        (0..16).map(|k| 0.25 * step.powi(k)).collect()
    }

    /// Where a curve's rise per pixel falls to a tenth of its best, read after the peak, and that best.
    pub fn exhaustion(sweep: &[f64], curve: &[f32]) -> (f64, f64) {
        let (mut mids, mut rate) = (Vec::new(), Vec::new());
        for i in 0..sweep.len() - 1 {
            let span = sweep[i + 1] - sweep[i];
            if span > 0.0 {
                mids.push((sweep[i] + sweep[i + 1]) / 2.0);
                rate.push(f64::from(curve[i + 1] - curve[i]) / span);
            }
        }
        let peak = rate.iter().copied().fold(f64::MIN, f64::max);
        if peak <= 0.0 {
            return (0.0, 0.0);
        }
        let wanted = 0.1 * peak;
        let crest = rate.iter().position(|&r| r == peak).unwrap_or(0);
        for i in crest + 1..rate.len() {
            if rate[i] <= wanted {
                let (high, low) = (rate[i - 1], rate[i]);
                let share = if high > low { (high - wanted) / (high - low) } else { 0.0 };
                return (mids[i - 1] + share * (mids[i] - mids[i - 1]), peak);
            }
        }
        (mids[mids.len() - 1], peak)
    }
}

/// `Pipeline.applyDetail` alone at full resolution: the rows pass, then the columns beside the source.
fn detailed(g: &mut Graph, source: &oni_graph::Source, k: Option<oni_render::Detail>) -> Vec<[f32; 4]> {
    let image = oni_graph::Image::new(source);
    let extent = image.extent();
    let Some(k) = k else { return g.render(&image, extent, None).unwrap() };
    let rows = image.clone().then(oni_pipeline::stages::DetailRows(k));
    g.render(&image.then_reading(oni_pipeline::stages::DetailColumns(k), &[rows]), extent, None).unwrap()
}

/// The first frame of the reference roll, decoded whole, or `None` after a skip line.
fn reference_frame(check: &str) -> Option<(String, oni_decode::Linear)> {
    if !oni_fixtures::heavy(check) {
        return None;
    }
    let dir = oni_fixtures::raw_dir(check)?;
    let mut files: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect();
    files.sort();
    let file = files.first()?;
    Some((file.file_name()?.to_string_lossy().into_owned(), oni_decode::decode(file, 1.0).ok()?))
}

// MARK: - ChromaDenoise.selfCheck(source:)

#[test]
fn the_chroma_track_reaches_the_films_dose_and_stops_short_of_smearing() {
    let check = "the_chroma_track_reaches_the_films_dose_and_stops_short_of_smearing";
    let Some((name, full)) = reference_frame(check) else { return };
    let Some(mut g) = graph(check) else { return };
    let (px, side, long_side) = probe::region(&full);
    let source = g.upload(side as u32, side as u32, &px).unwrap();
    let (luma, before) = probe::planes(&px);
    let (flat, edges) = probe::split(&luma, side);
    let mut measure = |radius: f32| {
        let dose = oni_settings::blocks::ChromaDenoise { radius, force: 1.0 };
        let (_, after) = probe::planes(&detailed(&mut g, &source, finishing::detail(Some(&dose), None, 1.0)));
        (probe::neighbour_spread(&after, side, &flat), probe::displacement(&before, &after, &edges))
    };
    let sweep = probe::sweep(48.0);
    let rest = measure(0.0);
    let (mut noise, mut bleed) = (Vec::new(), Vec::new());
    for &radius in &sweep {
        let point = measure(radius as f32);
        noise.push(rest.0 - point.0);
        bleed.push(point.1);
        println!("{name}: radius {radius:8.2} px, noise removed {:.6}, colour displaced at edges {:.6}", rest.0 - point.0, point.1);
    }
    let (reach, peak) = probe::exhaustion(&sweep, &noise);
    assert!(reach > 0.0, "denoising is never spent ({peak:.2e} per px at its peak)");
    let top = f64::from(oni_settings::blocks::ChromaDenoise::RADIUS_RANGE.1);
    assert!(top >= reach, "the track runs to {top} px and the film's dose sits at {reach:.2}");
    let noise_left = probe::beyond(&sweep, &noise, top);
    let smear_left = probe::beyond(&sweep, &bleed, top);
    assert!(smear_left >= 2.0 * noise_left, "past {top} px, {:.1} % noise left against {:.1} % smear", noise_left * 100.0, smear_left * 100.0);
    // Twin: a stop at the floor a preview leaves inert would leave most of the noise unreachable.
    let at_floor = probe::beyond(&sweep, &noise, pipeline::preview_visible_radius());
    assert!(at_floor > noise_left * 2.0, "a stop at the floor leaves {:.1} % against {:.1} %", at_floor * 100.0, noise_left * 100.0);
    // Twin: a radius of 0.01 of the long side, fixed to the frame, holds that dose inside its first tenth.
    assert!(reach / (0.01 * long_side) < 0.1);
}

// MARK: - Sharpen.selfCheck(source:)

#[test]
fn the_sharpening_track_reaches_the_films_edge_and_stops_short_of_halo() {
    let check = "the_sharpening_track_reaches_the_films_edge_and_stops_short_of_halo";
    let Some((name, full)) = reference_frame(check) else { return };
    let Some(mut g) = graph(check) else { return };
    let (px, side, long_side) = probe::region(&full);
    let source = g.upload(side as u32, side as u32, &px).unwrap();
    let (luma, _) = probe::planes(&px);
    let (_, edges) = probe::split(&luma, side);
    let (ceiling, base) = (probe::morphology(&luma, side, true), probe::morphology(&luma, side, false));
    let mut measure = |radius: f64| {
        let dose = oni_settings::blocks::Sharpen { radius: radius as f32, amount: 1.0, ..Default::default() };
        let (after, _) = probe::planes(&detailed(&mut g, &source, finishing::detail(None, Some(&dose), 1.0)));
        let (mut rim, mut counted) = (0.0f64, 0usize);
        for i in (0..side * side).filter(|&i| edges[i]) {
            rim += f64::from((after[i] - ceiling[i]).max(0.0) + (base[i] - after[i]).max(0.0));
            counted += 1;
        }
        (probe::step(&after, side, &edges), if counted == 0 { 0.0 } else { (rim / counted as f64) as f32 })
    };
    let sweep = probe::sweep(32.0);
    let rest = probe::step(&luma, side, &edges);
    let (mut contour, mut rim) = (Vec::new(), Vec::new());
    for &radius in &sweep {
        let point = measure(radius);
        contour.push(point.0 - rest);
        rim.push(point.1);
        println!("{name}: radius {radius:8.2} px, edge lifted {:.6}, rim outside the band {:.6}", point.0 - rest, point.1);
    }
    let (reach, peak) = probe::exhaustion(&sweep, &contour);
    assert!(reach > 0.0, "the edge is never spent ({peak:.2e} per px at its peak)");
    let (floor, top) = oni_settings::blocks::Sharpen::radius_range();
    let top = f64::from(top);
    assert!(top >= reach, "the track runs to {top} px and the film's edge is spent at {reach:.2}");
    let edge_left = probe::beyond(&sweep, &contour, top);
    let halo_left = probe::beyond(&sweep, &rim, top);
    assert!(edge_left < 0.2 && halo_left >= edge_left, "past {top} px, {:.1} % edge left against {:.1} % rim", edge_left * 100.0, halo_left * 100.0);
    // Twin: a stop at the bottom of the track would leave most of the edge unlifted.
    let at_floor = probe::beyond(&sweep, &contour, f64::from(floor));
    assert!(at_floor > edge_left * 2.0, "a stop at {floor} px leaves {:.1} % against {:.1} %", at_floor * 100.0, edge_left * 100.0);
    // Twin: a track of 0.002 of the long side, fixed to the frame, runs far past the edge's own answer.
    assert!(top / (0.002 * long_side) < 0.75);
}
