//! The virtual enlarger with Print on paper off: the neutral paper rests its grey on a plain RAW's, its pick on pure black
//! whatever Print lift says, each layer printing its own dye alone on ONI's print curve, the same on the three, so a frame
//! whose windows agree prints as on ONI's print; Lighting's grade turns about the grey;
//! the GPU on every branch. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::paper::{self, GREY_PRINT, neutral};
use oni_pipeline::{conversion, film, stages};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};

fn paperless(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { paperless: true, ..s }
}

/// A negative whose three windows agree: ONI's print and the neutral paper see the same tones.
fn even_negative() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    for w in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue] {
        (w.black, w.white, w.mid) = (0.3, 1.6, 0.5);
    }
    s.print_paper = Some(PaperStock::default());
    s
}

#[test]
fn the_neutral_paper_rests_its_grey_on_a_plain_raws_and_its_pick_on_pure_black() {
    for (label, s) in [("digital", digital()), ("negative", prepared_negative()), ("black and white", mono())] {
        assert!(s.print.print_lift, "{label}: Print lift on, so its silence shows");
        let s = paperless(s);
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let [black, _, grey] = anchor_scans(&s).map(|q| cpu::stage47(q, &k));
        let rested = p.printed(paper::shares_of(&s, &k, paper::anchors(&s)[2]));
        println!("{label}: grey {rested:?} at rest, {grey:?} shown (a plain RAW's {:.5}), black {black:?}, pick {:?}", *GREY_PRINT, p.pick);
        // The rest prints the grey neutral at a plain RAW's, shown through the pick laid on pure black by as little as the
        // pick's light off.
        assert!((0..3).all(|c| (rested[c] - *GREY_PRINT).abs() < 2e-4), "{label}: {rested:?}");
        assert!((0..3).all(|c| (grey[c] - rested[c]).abs() <= p.pick[c] + 2e-6), "{label}: {grey:?}");
        assert!(black[..3].iter().all(|v| v.abs() < 1e-6) && p.anchor == [0.0; 3], "{label}: the pick on pure black, {black:?}");
        assert!(p.white.iter().all(|w| (w - 1.0).abs() < 1e-6), "{label}: no paper's white, {:?}", p.white);
        // Twin: the same frame on its paper lays the pick where the paper's rest prints it.
        let on_paper = conversion::stage47(&PipelineSettings { paperless: false, ..s.clone() }).paper.expect(label);
        assert!(on_paper.anchor.iter().all(|a| *a > 1e-3), "{label}: {:?}", on_paper.anchor);
    }
    // Twin: the prepared negative through ten CC of yellow prints its grey off neutral.
    let s = paperless(prepared_negative());
    let k = conversion::stage47(&s);
    let bare = Stage47 { paper: k.paper.map(|p| Paper { filters: [1.0, 1.0, 10f32.powf(-0.1)], ..p }), ..k };
    let unfiltered = cpu::stage47(grey_scan(&s), &bare);
    assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2, "{unfiltered:?}");
}

#[test]
fn each_layer_of_the_neutral_paper_prints_its_own_dye_alone() {
    let s = prepared_negative();
    let k = conversion::stage47(&s);
    let grey = paper::shares_of(&s, &k, paper::anchors(&s)[2]);
    let moved = |p: &Paper, c: usize| {
        let (at, mut more) = (p.printed(grey), grey);
        more[c] += 0.2;
        let after = p.printed(more);
        std::array::from_fn::<f32, 3, _>(|i| after[i] - at[i])
    };
    let neutral = conversion::stage47(&paperless(s.clone())).paper.unwrap();
    let on_paper = k.paper.unwrap();
    for c in 0..3 {
        let own = moved(&neutral, c);
        println!("dye {c}: the neutral paper moves {own:?}, the paper {:?}", moved(&on_paper, c));
        assert!(own[c] > 1e-3 && (0..3).filter(|i| *i != c).all(|i| own[i] == 0.0), "dye {c}: {own:?}");
        // Twin: on the paper each dye's light reaches the other layers too.
        let crossed = moved(&on_paper, c);
        assert!((0..3).filter(|i| *i != c).any(|i| crossed[i].abs() > 1e-4), "dye {c}: {crossed:?}");
    }
}

#[test]
fn the_neutral_papers_curve_is_onis_print_at_rest_on_each_layer() {
    let sheet = neutral::sheet();
    let rest = Print { exposure: oni_pipeline::conversion::NEGATIVE_PRINT_STOPS, ..Print::default() };
    let last = (PAPER_CURVE_NODES - 1) as f32;
    for (i, d) in sheet.curves.iter().enumerate() {
        let expected = -rest.output(1.0 - i as f32 / last, LevelsChannel::Linked).max(neutral::NEUTRAL_DEEPEST).log10();
        assert!(d.iter().all(|v| (v - expected).abs() < 1e-5), "node {i}: {d:?} for {expected}");
    }
    assert!(sheet.print.len() == 3 && sheet.view.len() == 3);
    // Twin: a paper's own layers each run their own curve.
    let portra = paper::sheet(PaperStock::of("kodak-endura-premier"));
    assert!(portra.curves.iter().any(|d| (d[0] - d[1]).abs() > 0.05 || (d[1] - d[2]).abs() > 0.05));
}

#[test]
fn a_frame_whose_windows_agree_prints_on_the_neutral_paper_as_on_onis_print() {
    let s = even_negative();
    let (oni, neutral, on_paper) =
        (conversion::stage47(&PipelineSettings { print_paper: None, ..s.clone() }), conversion::stage47(&paperless(s.clone())), conversion::stage47(&s));
    let (mut worst, mut away) = (0.0f32, 0.0f32);
    for i in 0..=40 {
        let d = 0.3 + 1.3 * i as f32 / 40.0;
        let q = scan_at(&s, [d; 3]);
        let (a, b, c) = (cpu::stage47(q, &oni), cpu::stage47(q, &neutral), cpu::stage47(q, &on_paper));
        worst = worst.max((0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max));
        away = away.max((0..3).map(|k| (a[k] - c[k]).abs()).fold(0.0, f32::max));
    }
    println!("the neutral paper within {worst:.2e} of ONI's print, the paper {away:.3}");
    assert!(worst < 1e-3, "{worst}");
    // Twin: the paper's own curves print the same tones elsewhere.
    assert!(away > 0.02, "{away}");
}

#[test]
fn lightings_grade_turns_the_neutral_paper_about_its_grey() {
    let s = paperless(prepared_negative());
    let [black, white, grey] = lit(&s, |t| t.print.contrast = 0.3);
    let [_, rest_white, _] = lit(&s, |_| {});
    assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 1e-2), "{grey:?}");
    assert!(black[..3].iter().all(|v| v.abs() < 1e-6), "{black:?}");
    // Twin: the white moves with the grade.
    assert!((0..3).any(|c| (white[c] - rest_white[c]).abs() > 1e-2), "{white:?} {rest_white:?}");
    // Exposure moves the grey, Brightness the light above its knee, both on the neutral paper.
    let [_, _, darker] = lit(&s, |t| t.print.exposure -= 1.0);
    let [_, _, brighter] = lit(&s, |t| t.print.brightness = 1.0);
    assert!(darker[1] < grey[1] * 0.8 && brighter[1] > *GREY_PRINT * 1.2, "{darker:?} {brighter:?}");
}

#[test]
fn the_gpu_prints_on_the_neutral_paper_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_prints_on_the_neutral_paper_as_its_reference_does") else { return };
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 11, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 5] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0], [1e-6, 0.4, 0.002, 0.5]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    let mut lit_negative = paperless(prepared_negative());
    (lit_negative.print.exposure, lit_negative.print.contrast, lit_negative.print.shadows, lit_negative.print.highlights) = (2.0, 0.2, 0.1, -0.1);
    lit_negative.print.set_filters([0.2, -0.1, 0.0]);
    (lit_negative.print.black_point, lit_negative.print.white_point, lit_negative.print.channel_mid, lit_negative.print.brightness) = (0.03, 0.05, [0.0, 0.1, 0.0], 0.5);
    lit_negative.density = 0.3;
    let mut lit_mono = paperless(mono());
    (lit_mono.print.contrast, lit_mono.print.highlights, lit_mono.print.brightness) = (0.3, -0.2, -0.5);
    let frames = [
        ("digital", paperless(digital())),
        ("lit negative", lit_negative),
        ("frontier chosen", paperless(PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..prepared_negative() })),
        ("black and white", lit_mono),
    ];
    for (label, s) in frames {
        let k = conversion::stage47(&s);
        assert!(k.paper.is_some_and(|p| p.mix == paper::OWN_LAYERS && p.print.len() == 3), "{label}: on the neutral paper");
        let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        let twin = Stage47 { paper: k.paper.map(|p| Paper { scale: p.scale * 1.1, ..p }), ..k };
        judge(&format!("neutral paper, {label}"), &out, &reference, &mapped(&pixels, |p| cpu::stage47(p, &twin)));
    }
}
