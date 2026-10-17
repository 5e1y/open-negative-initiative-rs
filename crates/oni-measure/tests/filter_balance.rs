//! `FilterBalance.selfCheck` on the graph: a cast frame with Lighting and the colour density moved,
//! balanced through the kernel on Temperature and Tint alone; each claim beside its twin.

use oni_core::balance::{Outcome, applied_outcome};
use oni_graph::{Graph, Source};
use oni_measure::filter_balance::{filters_to_axes, finite, printable, unlit};
use oni_measure::key;
use oni_measure::reader::{Copy, Reader};
use oni_pipeline::Pipeline;
use oni_settings::frame::RollReference;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::settings::roll_settings::dosed;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Id, PipelineSettings};

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
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline };
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
    let pressed = applied_outcome(&Outcome { reference, filters: found }, &referenced);
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
fn a_dust_speck_past_white_is_left_out_and_read_it_sends_an_axis_to_its_stop() {
    let check = "filter_balance::dust";
    let Some(mut graph) = graph(check) else { return };
    let mut pipeline = Pipeline::new();
    let source = scene(&mut graph, &[(0, 96, [0.12, 0.14, 0.17])], Some((44, 28, 8, 8, [0.012, 0.025, 0.04])));
    let copy = Copy { source: &source, full_width: None };
    let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline };
    let narrow = settings(1.2);
    let kept = key::filters(&mut r, copy, &narrow, printable).unwrap();
    let blinded = key::filters(&mut r, copy, &narrow, finite).unwrap();
    let means = key::shades(&mut r, copy, &unlit(&narrow), kept, printable).unwrap();
    println!("kept {:?}, blinded {:?}", filters_to_axes(kept), filters_to_axes(blinded));
    assert!(parted(means) < 1e-3 && !stopped(kept), "the rest parts by {} at {:?}", parted(means), filters_to_axes(kept));
    // Twin: read, the dust sends an axis to its stop.
    assert!(stopped(blinded), "{:?}", filters_to_axes(blinded));
}
