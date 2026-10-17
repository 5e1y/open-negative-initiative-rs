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
        assert!(if up { step > 0.0 } else { step < 0.0 }, "[{}] the ceiling moves the render against its sense: {step:+.6}", mode.raw_value());
        measured.push((mode, step));
    }
    let of = |m| measured.iter().find(|(x, _)| *x == m).map(|(_, s)| *s).unwrap();
    // Twin: one sense for both modes would make the two measurements agree in sign.
    assert!(of(ConversionMode::Negative) * of(ConversionMode::Positive) < 0.0);

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
