//! `GainPanel.selfCheck` and `TemperatureTintPanel.selfCheck`: a slider's label and track name the
//! colour the hand gets, so each is measured through the kernel and read off the panel it is drawn on.

use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::axis::density_axis;
use oni_settings::levels::LevelsSet;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PipelineSettings};

type V3 = [f32; 3];

/// Each gain row's hue and complement, as the panel names them.
const NAMES: [(&str, &str); 3] = [("Red", "Cyan"), ("Green", "Magenta"), ("Blue", "Yellow")];

fn panel(file: &str) -> String {
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui/panel").join(file);
    std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()))
}

/// A neutral 0.2 patch through each window's own arithmetic; `lit` keeps Lighting on.
fn render(g: &mut Graph, mode: ConversionMode, lit: bool, build: impl FnOnce(&mut PipelineSettings)) -> V3 {
    let mut s = PipelineSettings { mode, ..PipelineSettings::default() };
    s.levels = LevelsSet::neutral(mode, density_axis::base());
    build(&mut s);
    let mut m = s.per_pixel_only();
    m.print.bypassed = !lit;
    m.chroma_decompression = 1.0;
    m.roll_reference = None;
    let source = g.upload(2, 2, &[[0.2, 0.2, 0.2, 1.0]; 4]).expect("the patch uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: 1, h: 1 }), ..Request::default() };
    let (_, px) = Pipeline::new().render(g, &source, &m, &request, &Inputs::default(), None).expect("the patch renders");
    [px[0][0], px[0][1], px[0][2]]
}

/// How far the render stands towards a channel's complement, against the mean of the other two.
fn towards_complement(channel: usize, px: V3) -> f32 {
    let others: Vec<f32> = (0..3).filter(|&c| c != channel).map(|c| px[c]).collect();
    (others[0] + others[1]) / 2.0 - px[channel]
}

/// The right-hand end each gain row names, inverted then not: `label: inverted ? "A ↔ B" : "C ↔ D"`.
fn right_ends(slint: &str) -> Vec<(String, String)> {
    slint
        .lines()
        .filter(|l| l.contains("label: PanelState.gains-inverted ?"))
        .map(|l| {
            let quoted: Vec<&str> = l.split('"').skip(1).step_by(2).take(2).collect();
            let right = |label: &str| label.split(" ↔ ").nth(1).unwrap_or_default().to_string();
            (right(quoted[0]), right(quoted[1]))
        })
        .collect()
}

#[test]
fn each_gain_row_names_the_end_the_hand_reaches_in_both_senses() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let ends = right_ends(&panel("roll_tab.slint"));
    assert_eq!(ends.len(), 3, "three gain rows on the Roll tab: {ends:?}");
    for mode in [ConversionMode::Negative, ConversionMode::Positive] {
        let rest = render(&mut g, mode, false, |_| {});
        for (channel, (hue, complement)) in NAMES.iter().enumerate() {
            let moved = render(&mut g, mode, false, |s| s.gains.stops[channel] = 1.0);
            let gap = towards_complement(channel, moved) - towards_complement(channel, rest);
            let right = if mode.inverts_gain_sense() { &ends[channel].0 } else { &ends[channel].1 };
            let reached = if gap > 0.0 { complement } else { hue };
            assert!(gap.abs() > 1e-4 && right == reached, "{mode:?} {hue}: +1 stop moves {gap:+.5} towards the complement, the track ends on {right}");
        }
    }
    // Twin: every row's two ends swap with the sense, so a label read in the wrong mode names the opposite.
    assert!(ends.iter().all(|(inverted, plain)| inverted != plain), "{ends:?}");
    let wiring = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-app/src/app/roll_tab.rs")).unwrap();
    assert!(wiring.contains("set_gains_inverted(settings.mode.inverts_gain_sense())"), "the panel takes the frame's measured sense");
    let flat = render(&mut g, ConversionMode::Monochrome, false, |s| s.gains.stops[1] = 1.0);
    let grey = flat.iter().copied().fold(f32::MIN, f32::max) - flat.iter().copied().fold(f32::MAX, f32::min);
    assert!(grey < 1e-6, "the rows in monochrome land grey: {grey:.2e} of spread");
}

#[test]
fn temperature_and_tint_reach_the_colours_their_tracks_end_on() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let colour = panel("color_tab.slint");
    let track = |label: &str| {
        let lines: Vec<&str> = colour.lines().collect();
        let at = lines.iter().position(|l| l.contains(&format!("label: \"{label}\""))).expect("the slider");
        lines[at + 1].split("high: ").nth(1).unwrap_or_default().trim_end_matches(';').to_string()
    };
    assert_eq!((track("Temperature").as_str(), track("Tint").as_str()), ("Colour.channel-yellow", "Colour.channel-magenta"));
    let lay = |s: &mut PipelineSettings, axis: V3| {
        let now = s.print.filters();
        s.print.set_filters(std::array::from_fn(|c| now[c] + axis[c]));
    };
    let warmth = |v: V3| v[0] - v[2];
    let magenta = |v: V3| (v[0] + v[2]) / 2.0 - v[1];
    for mode in [ConversionMode::Negative, ConversionMode::Positive] {
        let rest = render(&mut g, mode, true, |_| {});
        let warmed = render(&mut g, mode, true, |s| lay(s, Print::TEMPERATURE_AXIS));
        let tinted = render(&mut g, mode, true, |s| lay(s, Print::TINT_AXIS));
        let (warm, tint) = (warmth(warmed) - warmth(rest), magenta(tinted) - magenta(rest));
        assert!(warm > 1e-3 && tint > 1e-3, "{mode:?}: Temperature right warms by {warm:.4}, Tint right adds magenta by {tint:.4}");
        // Twin: Temperature left cools.
        let cooled = render(&mut g, mode, true, |s| lay(s, Print::TEMPERATURE_AXIS.map(|a| -a)));
        assert!(warmth(cooled) < warmth(rest) - 1e-3, "{mode:?}: Temperature left does not cool");
    }
}
