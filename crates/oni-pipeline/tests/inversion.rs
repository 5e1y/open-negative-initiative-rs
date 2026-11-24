//! The pure inversion, the render of a frame naming no paper (owner's call): each channel's density, read between
//! Prepare's black and white, shown as the sRGB code of its light with no curve and no bound but the output clip's, on
//! the scalar reference and through the graph, against the owner's reference formula written here. Each has a twin.

mod common;

use common::{V4, graph, judge, mapped, whole};
use oni_graph::Image;
use oni_pipeline::{conversion, stages};
use oni_render::{Shown, cpu};
use oni_settings::levels::Levels;
use oni_settings::{ConversionMode, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};

const STANDARD: PaperStock = PaperStock::of("oni-standard");

/// A prepared colour negative on a camera scan: each channel's own gain and window, in density, no paper named.
fn prepared() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-0.9, 0.0, 0.7];
    (s.levels.red, s.levels.green, s.levels.blue) = (Levels::with_ends(0.31, 1.62), Levels::with_ends(0.28, 1.71), Levels::with_ends(0.35, 1.48));
    s
}

fn ends(s: &PipelineSettings) -> [(f64, f64); 3] {
    s.levels.windows().map(|w| (f64::from(w.black), f64::from(w.white)))
}

fn decoded(code: f64) -> f64 {
    if code <= 0.04045 { code / 12.92 } else { ((code + 0.055) / 1.055).powf(2.4) }
}

/// The reference the owner approved on the reference roll, as its screen shows it: the scan's density through its gain,
/// between the window's ends freed of the density guard's pedestal, held within 0 and 1 and read through `curve`.
fn reference(scan: V4, s: &PipelineSettings, curve: impl Fn(f64) -> f64) -> V4 {
    let pedestal = 10f64.powf(-f64::from(s.density_ceiling));
    let out: [f64; 3] = std::array::from_fn(|c| {
        let gain = 2f64.powf(f64::from(s.gains.stops[c]));
        let freed = |level: f64| -(10f64.powf(-level) - pedestal * gain).log10();
        let (black, white) = (freed(ends(s)[c].0), freed(ends(s)[c].1));
        let density = -(f64::from(scan[c]).max(1e-6) * gain).log10();
        curve(((density - black) / (white - black)).clamp(0.0, 1.0))
    });
    [out[0] as f32, out[1] as f32, out[2] as f32, scan[3]]
}

/// The scan holding the `share` of each channel's window in density, 0 its bare film and 1 its densest, past either too.
fn scanned(s: &PipelineSettings, share: [f64; 3]) -> V4 {
    let pedestal = 10f64.powf(-f64::from(s.density_ceiling));
    let light: [f64; 3] = std::array::from_fn(|c| {
        let (black, white) = ends(s)[c];
        10f64.powf(-(black + share[c] * (white - black))) / 2f64.powf(f64::from(s.gains.stops[c])) - pedestal
    });
    [light[0] as f32, light[1] as f32, light[2] as f32, 1.0]
}

/// Scans along and across the windows: neutral shares from under the bare film to past the densest, and coloured ones.
fn scans(s: &PipelineSettings) -> Vec<V4> {
    let neutral = (0..=140).map(|i| [-0.2 + 0.01 * f64::from(i); 3]);
    let coloured = (0..=20).flat_map(|i| {
        let t = 0.05 * f64::from(i);
        [[t, 1.0 - t, 0.5], [0.3, t, 1.0 - t], [1.0 - t, 0.6, t]]
    });
    neutral.chain(coloured).map(|share| scanned(s, share)).collect()
}

fn worst(a: &[V4], b: &[V4]) -> f32 {
    a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
}

#[test]
fn a_frame_naming_no_paper_shows_each_windows_share_as_the_srgb_code_of_its_light() {
    let s = prepared();
    let k = conversion::stage47(&s);
    assert!(s.print_paper.is_none() && s.shows_inversion() && k.shown == Shown::Inversion && conversion::printing(&s).is_none());
    let scans = scans(&s);
    // The reference is the picture on its screen: the stage's own, past the output clip.
    let shown = mapped(&scans, |p| cpu::output_clip(cpu::stage47(p, &k)));
    let approved = mapped(&scans, |p| reference(p, &s, decoded));
    // The app reads its density over the guard's pedestal, the reference past it: on these gains and windows, 2.5e-3 of
    // light at most at a ceiling of 3.5.
    let off = worst(&shown, &approved);
    println!("the pure inversion off the owner's reference by {off:.2e} of light");
    assert!(off < 3e-3, "{off}");
    // Prepare's black lies on pure black and its white on pure white, each channel's own.
    let rgb = |p: V4| [p[0], p[1], p[2]];
    let at = |share: f64| rgb(cpu::stage47(scanned(&s, [share; 3]), &k));
    assert!(at(0.0).iter().all(|v| v.abs() < 1e-5) && at(1.0).iter().all(|v| (v - 1.0).abs() < 1e-4), "{:?} {:?}", at(0.0), at(1.0));
    // Past either end the stage bounds nothing, so a later stage reads what the film holds there; the output clip, the
    // chain's only bound, lays it on black or on white.
    let (under, over) = (at(-0.2), at(1.3));
    assert!(under.iter().all(|v| *v < -0.01) && over.iter().all(|v| *v > 1.5), "{under:?} {over:?}");
    let clipped = |share: f64| rgb(cpu::output_clip(cpu::stage47(scanned(&s, [share; 3]), &k)));
    assert_eq!((clipped(-0.2), clipped(1.3)), ([0.0; 3], [1.0; 3]));
    // A neutral share shows neutral, whatever the channels' gains and windows.
    let grey = at(0.5);
    assert!((grey[0] - grey[1]).abs() < 1e-4 && (grey[2] - grey[1]).abs() < 1e-4 && (grey[1] - 0.214).abs() < 1e-3, "{grey:?}");
    // Twins: a display gamma's power or the share taken as light is another render, and so is a paper's or a legacy
    // print's curve on the same windows.
    let through = |curve: fn(f64) -> f64| worst(&shown, &mapped(&scans, |p| reference(p, &s, curve)));
    let (gamma, linear) = (through(|t| t.powf(2.2)), through(|t| t));
    assert!(gamma > 2.0 * off && linear > 0.2, "{gamma} {linear}");
    let other = |s: &PipelineSettings| worst(&shown, &mapped(&scans, |p| cpu::output_clip(cpu::stage47(p, &conversion::stage47(s)))));
    let on_paper = other(&PipelineSettings { print_paper: Some(STANDARD), ..s.clone() });
    let on_curve = other(&PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..s.clone() });
    println!("twins: gamma 2.2 {gamma:.2e}, linear {linear:.2}, ONI Standard {on_paper:.2}, a legacy print's curve {on_curve:.2}");
    assert!(on_paper > 0.05 && on_curve > 0.05, "{on_paper} {on_curve}");
}

#[test]
fn the_graph_shows_the_pure_inversion_as_its_reference_does() {
    let Some(mut g) = graph("the_graph_shows_the_pure_inversion_as_its_reference_does") else { return };
    let s = prepared();
    let k = conversion::stage47(&s);
    let scans = scans(&s);
    let (w, h) = (scans.len() as u32 / 3, 3);
    let pixels = &scans[..(w * h) as usize];
    let source = g.upload(w, h, pixels).unwrap();
    let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
    let on_paper = conversion::stage47(&PipelineSettings { print_paper: Some(STANDARD), ..s.clone() });
    judge("the pure inversion", &out, &mapped(pixels, |p| cpu::stage47(p, &k)), &mapped(pixels, |p| cpu::stage47(p, &on_paper)));
    // Its kernel alone, on shares past both ends of the window; twin, a display gamma's power between them.
    let shares = common::noise(w, h, 7, -0.3, 1.4);
    let input = g.upload(w, h, &shares).unwrap();
    let out = whole(&mut g, &Image::new(&input).then(stages::inversion()));
    let gamma = |p: V4| [p[0], p[1], p[2]].map(|v| v.clamp(0.0, 1.0).powf(2.2));
    judge("the inversion's kernel", &out, &mapped(&shares, cpu::inversion), &mapped(&shares, |p| [gamma(p)[0], gamma(p)[1], gamma(p)[2], p[3]]));
    // Lighting moved, its pass and the inversion's are laid with the windows' as the reference runs them.
    let mut lit = s.clone();
    (lit.print.exposure, lit.print.contrast) = (0.4, 0.2);
    let moved = conversion::stage47(&lit);
    assert!(moved.negative.is_some() && moved.shown == Shown::Inversion);
    let out = whole(&mut g, &stages::converted(Image::new(&source), &moved));
    judge("the pure inversion under Lighting", &out, &mapped(pixels, |p| cpu::stage47(p, &moved)), &mapped(pixels, |p| cpu::stage47(p, &k)));
}

#[test]
fn every_frame_a_paper_may_print_shows_its_inversion_until_it_names_one() {
    let shown = |s: &PipelineSettings| conversion::stage47(s).shown;
    for mode in [ConversionMode::Negative, ConversionMode::Monochrome, ConversionMode::Digital] {
        let rest = PipelineSettings::rest(mode, true);
        assert!(rest.print_paper.is_none() && shown(&rest) == Shown::Inversion, "{mode:?}");
        // A paper named prints, and a legacy print keeps its own curve.
        let on_paper = PipelineSettings { print_paper: Some(STANDARD), ..rest.clone() };
        assert!(matches!(shown(&on_paper), Shown::Paper(_)) && conversion::printing(&on_paper).is_some(), "{mode:?}");
        let legacy = PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..rest.clone() };
        assert!(shown(&legacy) == Shown::Curve && conversion::stage47(&legacy).print_on == 1.0, "{mode:?}");
        // A measure upstream of the print reads the windows' shares as they are.
        let mut measured = rest.clone();
        measured.print.bypassed = true;
        assert!(shown(&measured) == Shown::Curve && conversion::stage47(&measured).print_on == 0.0, "{mode:?}");
        // A paper's own settings move nothing a frame on no paper shows; on a paper they do.
        let mut ended = rest.clone();
        (ended.print.brightness, ended.print.print_lift) = (1.0, true);
        assert_eq!(conversion::stage47(&ended), conversion::stage47(&rest), "{mode:?}");
        let ended_on_paper = PipelineSettings { print_paper: Some(STANDARD), ..ended };
        assert_ne!(conversion::stage47(&ended_on_paper), conversion::stage47(&on_paper), "{mode:?}");
    }
    // A slide is projected, and one shot on a reversal film too: neither shows the inversion, with or without a paper named.
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    let reversal = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::SLIDE);
    for s in [slide, reversal] {
        assert!(!s.takes_paper() && !s.shows_inversion() && shown(&s) == Shown::Curve);
        assert!(shown(&PipelineSettings { print_paper: Some(STANDARD), ..s }) == Shown::Curve);
    }
    // A paper no file defines prints on no paper until its file is back, its key kept.
    let missing = PipelineSettings { print_paper: Some(PaperStock::of("a-paper-no-file-defines")), ..prepared() };
    assert!(missing.paper().is_none() && missing.shows_inversion() && shown(&missing) == Shown::Inversion);
    assert_eq!(conversion::stage47(&missing), conversion::stage47(&prepared()));
}

#[test]
fn lighting_moves_the_inversion_and_exposure_holds_prepares_black_and_white() {
    let rest = prepared();
    let mut lit = rest.clone();
    lit.print.exposure = 1.0;
    let (k, moved) = (conversion::stage47(&rest), conversion::stage47(&lit));
    let at = |k: &oni_render::Stage47, share: f64| cpu::stage47(scanned(&rest, [share; 3]), k)[1];
    assert!(at(&moved, 0.5) > at(&k, 0.5) + 0.02, "{} {}", at(&moved, 0.5), at(&k, 0.5));
    assert!(at(&moved, 0.0).abs() < 1e-4 && (at(&moved, 1.0) - 1.0).abs() < 1e-3, "{} {}", at(&moved, 0.0), at(&moved, 1.0));
    // Twin: at rest Lighting lays nothing.
    assert!(k.negative.is_none() && moved.negative.is_some());
}
