//! `FilterBalance.selfCheck` on the graph: a cast frame with Lighting and the colour density moved,
//! balanced through the kernel on Temperature and Tint alone; each claim beside its twin.

use oni_core::balance::{Outcome, applied_outcome};
use oni_graph::{Graph, Source};
use oni_measure::filter_balance::{filters_to_axes, finite, printable, unlit};
use oni_measure::key;
use oni_measure::reader::{Copy, Laying, Reader};
use oni_measure::truncate::Stage;
use oni_pipeline::Pipeline;
use oni_settings::frame::RollReference;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::settings::roll_settings::dosed;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Id, LegacyPrint, PaperStock, PipelineSettings};

const W: u32 = 96;
const H: u32 = 64;

fn graph(check: &str) -> Option<Graph> {
    if !oni_fixtures::heavy(check) {
        return None;
    }
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

/// Columns of colour across the frame, each `(x0, x1, rgb)`, later ones over earlier ones.
fn scene(graph: &mut Graph, columns: &[(u32, u32, [f32; 3])], speck: Option<(u32, u32, u32, u32, [f32; 3])>) -> Source {
    let mut pixels = vec![[0.0f32, 0.0, 0.0, 1.0]; (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            let mut c = [0.0; 3];
            for (x0, x1, rgb) in columns {
                if (*x0..*x1).contains(&x) {
                    c = *rgb;
                }
            }
            if let Some((sx, sy, sw, sh, rgb)) = speck
                && (sx..sx + sw).contains(&x)
                && (sy..sy + sh).contains(&y)
            {
                c = rgb;
            }
            pixels[(y * W + x) as usize] = [c[0], c[1], c[2], 1.0];
        }
    }
    graph.upload(W, H, &pixels).unwrap()
}

fn settings(white: f32) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    let window = Levels { black: 0.1, white, mid: 0.5, ..Levels::default() };
    s.levels = LevelsSet { red: window, green: window, blue: window };
    s.print.exposure = 0.7;
    s.density = 0.37;
    s
}

fn parted(v: [f32; 3]) -> f32 {
    (v[0].max(v[1]).max(v[2]) - v[0].min(v[1]).min(v[2])) / v[1].max(1e-6)
}

fn stopped(filters: [f32; 3]) -> bool {
    let (t, n) = filters_to_axes(filters);
    [Print::TEMPERATURE_RANGE.0, Print::TEMPERATURE_RANGE.1].iter().any(|s| (t - s).abs() < 1e-5)
        || [Print::TINT_RANGE.0, Print::TINT_RANGE.1].iter().any(|s| (n - s).abs() < 1e-5)
}

#[test]
fn the_press_lands_the_three_means_together_through_the_kernel_and_the_dose_writes_its_share() {
    let check = "filter_balance::press";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    // A negative printing warm: a dark corner, a deeper one and a bright wall, all leaning the same way.
    let source = scene(&mut graph, &[(0, 96, [0.088, 0.11, 0.147]), (32, 64, [0.0071, 0.0091, 0.0123]), (0, 32, [0.027, 0.034, 0.046])], None);
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
    let s = settings(2.3);
    let found = key::filters(&mut r, copy, &s, printable).expect("the cast frame reads");
    let balanced = key::shades(&mut r, copy, &unlit(&s), found, printable).unwrap();
    let bare = key::shades(&mut r, copy, &unlit(&s), [0.0; 3], printable).unwrap();
    let (t, n) = filters_to_axes(found);
    println!("Temperature {t:+.3}, Tint {n:+.3}, the means part by {:.1e}", parted(balanced));
    assert!(parted(balanced) < 1e-3 && (found[0] + found[1] + found[2]).abs() < 1e-6, "{found:?} parts by {}", parted(balanced));
    // Twin: left at zero they part.
    assert!(parted(bare) > 0.02, "{}", parted(bare));

    // Read before Lighting: the light dialled moves nothing, where the same filters read lit part.
    let mut brighter = s.clone();
    brighter.print.exposure = -1.3;
    brighter.print.contrast = 0.31;
    assert_eq!(key::filters(&mut r, copy, &brighter, printable), Some(found));
    let lit = key::shades(&mut r, copy, &brighter, found, printable).unwrap();
    assert!(parted(lit) > 2e-3, "read lit the same filters part by {}", parted(lit));

    let reference = RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    let mut referenced = s.clone();
    referenced.roll_reference = Some(reference.clone());
    referenced.balance_strength = 0.5;
    let pressed = applied_outcome(&Outcome { reference: Some(reference), filters: found }, &referenced);
    let moved = dosed(&pressed, 0.8);
    assert!((pressed.print.red.exposure - found[0] / 2.0).abs() < 1e-6 && (pressed.print.blue.exposure - found[2] / 2.0).abs() < 1e-6);
    assert_eq!(pressed.balance_filters, Some(found));
    assert!((moved.print.red.exposure - found[0] * 0.8).abs() < 1e-6 && (moved.print.blue.exposure - found[2] * 0.8).abs() < 1e-6);
    let mut unpressed = s.clone();
    unpressed.print.red.exposure = 0.31;
    assert_eq!(dosed(&unpressed, 0.2).print.red.exposure, 0.31, "a frame no key balanced keeps its filters");

    let mut mono = s.clone();
    mono.mode = ConversionMode::Monochrome;
    assert_eq!(key::filters(&mut r, copy, &mono, printable), Some([0.0; 3]));
    assert_ne!(found, [0.0; 3]);
}

#[test]
fn a_dust_speck_past_white_is_left_out_and_read_it_pulls_the_filters_away() {
    let check = "filter_balance::dust";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    let source = scene(&mut graph, &[(0, 96, [0.12, 0.14, 0.17])], Some((44, 28, 8, 8, [0.012, 0.025, 0.04])));
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
    // On no paper and on one: the dust lies past the window's white, which the stage holds nowhere and a paper prints on
    // its own white end.
    let narrow = settings(1.2);
    let on_paper = PipelineSettings { print_paper: Some(PaperStock::default()), ..narrow.clone() };
    for (label, s) in [("no paper", &narrow), ("a paper", &on_paper)] {
        let kept = key::filters(&mut r, copy, s, printable).unwrap();
        let blinded = key::filters(&mut r, copy, s, finite).unwrap();
        let means = key::shades(&mut r, copy, &unlit(s), kept, printable).unwrap();
        println!("{label}: kept {:?}, blinded {:?}", filters_to_axes(kept), filters_to_axes(blinded));
        assert!(parted(means) < 1e-3 && !stopped(kept), "{label}: the rest parts by {} at {:?}", parted(means), filters_to_axes(kept));
        // Twin: read, the dust pulls the filters a tenth of an axis away.
        let ((t, n), (bt, bn)) = (filters_to_axes(kept), filters_to_axes(blinded));
        assert!((t - bt).hypot(n - bn) > 0.05, "{label}: {:?}", filters_to_axes(blinded));
    }
}

#[test]
fn the_balance_reads_onis_print_under_a_legacy_print_which_renders_its_own() {
    let check = "filter_balance::face";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    let source = scene(&mut graph, &[(0, 96, [0.088, 0.11, 0.147]), (0, 32, [0.027, 0.034, 0.046])], None);
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
    let pro = settings(2.3);
    let scanner = PipelineSettings { legacy_print: Some(LegacyPrint { d: 2, ..LegacyPrint::default() }), ..pro.clone() };
    let found = key::filters(&mut r, copy, &pro, printable).expect("the cast frame reads");
    assert_eq!(key::filters(&mut r, copy, &scanner, printable), Some(found), "the filters are ONI's print's under either");
    assert_eq!(key::roll_measured(&mut r, copy, "", &scanner), key::roll_measured(&mut r, copy, "", &pro), "the roll's measure reads ONI's windows");
    // Twin: in front of the curves the legacy print renders its own picture, which a balance would misread.
    let mut read = |s: &PipelineSettings| r.plane(copy, s, Stage::ManualCurves).expect("the frame renders").pixels;
    let (own, pro_s) = (read(&scanner), read(&pro));
    let apart = own.iter().zip(&pro_s).map(|(a, b)| (a[1] - b[1]).abs()).fold(0.0, f32::max);
    assert!(apart > 1e-2, "the legacy print's look parts from ONI's print by {apart}");
}

/// A digital photo's key balances its print as a negative's, on its film's own reference: filters that print
/// its light grey, no roll measured or written; twin: unfiltered, the cast frame parts.
#[test]
fn a_digital_photo_s_key_balances_its_print_on_its_films_reference() {
    let check = "filter_balance::digital";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    // A scene lit warm: a wall, a shade and a bright patch, all leaning red.
    let source = scene(&mut graph, &[(0, 96, [0.30, 0.22, 0.14]), (32, 64, [0.05, 0.035, 0.022]), (0, 32, [0.55, 0.42, 0.27])], None);
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
    let s = PipelineSettings::rest(ConversionMode::Digital, false);
    let found = key::balance_outcome(&mut r, copy, "digital", &s).expect("the cast photo reads");
    assert!(found.reference.is_none() && found.filters != [0.0; 3], "filters on no roll: {found:?}");
    let means = |r: &mut Reader, f: [f32; 3]| {
        let mut laid = oni_measure::filter_balance::unlit(&s);
        oni_measure::filter_balance::filtered(&mut laid, f);
        let pixels: Vec<[f32; 3]> = r.balance_pixels(copy, &laid).unwrap().into_iter().filter(|p| printable(*p)).collect();
        oni_measure::filter_balance::minkowski(&pixels).unwrap()
    };
    assert!(parted(means(&mut r, found.filters)) < 1e-2, "{found:?} parts by {}", parted(means(&mut r, found.filters)));
    assert!(parted(means(&mut r, [0.0; 3])) > 0.05, "twin: unfiltered the cast stays");
    let pressed = applied_outcome(&found, &s);
    assert_eq!((pressed.gains, pressed.levels, pressed.roll_reference.is_none()), (s.gains, s.levels, true), "the film's reference kept");
}

/// On a paper the key reads the paper's print, its Temperature and Tint the enlarger's filtration: the filters it finds land
/// the paper's means together; twin: no filter leaves them parted.
#[test]
fn on_a_paper_the_key_balances_the_papers_own_print() {
    let check = "filter_balance::paper";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    let source = scene(&mut graph, &[(0, 96, [0.30, 0.22, 0.14]), (32, 64, [0.05, 0.035, 0.022]), (0, 32, [0.55, 0.42, 0.27])], None);
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
    let paper = PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Digital, false) };
    assert!(paper.paper().is_some(), "a digital photo on the paper it names");
    let found = key::filters(&mut r, copy, &paper, printable).expect("the cast photo reads");
    let balanced = parted(key::shades(&mut r, copy, &unlit(&paper), found, printable).unwrap());
    let unfiltered = parted(key::shades(&mut r, copy, &unlit(&paper), [0.0; 3], printable).unwrap());
    println!("on the paper {:?}; the paper's means part by {balanced:.1e}, by {unfiltered:.1e} unfiltered", filters_to_axes(found));
    assert!(balanced < 1e-3, "{found:?} parts by {balanced}");
    assert!(unfiltered > 3e-3, "twin: unfiltered the paper's means part by {unfiltered}");
}
