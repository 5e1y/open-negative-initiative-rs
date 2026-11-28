//! The colour tools on the negative in every pipe: at their rest laid nowhere, Lighting moved or not; Saturation
//! spreading the layers about the neutral, its colours inside the paper's dyes at its track's ends; a hue range's move
//! touching that range alone; the saturation curve read on the print's own lightness; the GPU its reference. Each verdict
//! has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;
#[path = "colour_on_negative/scenes.rs"]
mod scenes;

use common::{V4, graph, judge, mapped, whole};
use frames::*;
use scenes::*;
use oni_graph::Image;
use oni_pipeline::{conversion, stages};
use oni_render::{LUMA, Negative, Paper, Stage47, cpu};
use oni_settings::cg::Point;
use oni_settings::curves::SaturationCurve;
use oni_settings::spectrogram::Band;
use oni_settings::tone::Print;
use oni_settings::{LegacyMachine, LegacyPrint, PaperStock, PipelineSettings};

/// Every pipe, each at the colour tools' rest, some with Lighting's other tools moved.
fn pipes() -> Vec<(&'static str, PipelineSettings)> {
    let mut uncompensated = endura(prepared_negative());
    uncompensated.print.colour_compensation = false;
    let mut legacy = unpapered(prepared_negative());
    legacy.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 1, c: 2, m: -1, y: 0, w: 3, laid: true });
    let mut bright = prepared_negative();
    (bright.print.brightness, bright.print.print_lift) = (0.7, false);
    let frontier = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..digital() };
    vec![
        ("Endura Premier, negative", endura(prepared_negative())),
        ("Endura Premier, uncompensated", uncompensated),
        ("legacy print, Frontier", legacy),
        ("Endura Premier, digital", endura(digital())),
        ("Endura Premier, black and white", endura(mono())),
        ("projector, slide", slide()),
        ("paper, negative", prepared_negative()),
        ("paper, negative, Brightness", bright),
        ("paper, digital", digital()),
        ("paper, black and white", mono()),
        ("paper, Frontier", frontier),
        ("no paper, negative", unpapered(prepared_negative())),
        ("no paper, digital", unpapered(digital())),
        ("no paper, black and white", unpapered(mono())),
    ]
}

/// Lighting at rest, then moved on the negative.
fn lightings() -> Vec<(&'static str, fn(&mut Print))> {
    vec![
        ("rest", |_| {}),
        ("Exposure +1, Contrast +0.3", |p| (p.exposure, p.contrast) = (p.exposure + 1.0, 0.3)),
        ("warmer, points, zones", |p| {
            p.set_filters([0.3, 0.0, -0.3]);
            (p.black_point, p.white_point, p.shadows, p.highlights) = (0.05, 0.1, 0.1, -0.1);
        }),
    ]
}

fn lit(s: &PipelineSettings, f: impl Fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

/// Every pipe under every Lighting, its colour tools as `colour` lays them.
fn kernels(colour: impl Fn(&mut PipelineSettings)) -> Vec<(String, Stage47)> {
    let mut out = Vec::new();
    for (label, s) in pipes() {
        for (lighting, f) in lightings() {
            out.push((format!("{label}, {lighting}"), conversion::stage47(&with(&lit(&s, f), &colour))));
        }
    }
    out
}

#[test]
fn every_pipe_at_the_colour_tools_rest_lays_them_nowhere() {
    // At rest the negative carries no colour tool, under Lighting at rest or moved: its kernel then keeps every layer's
    // log exposure to the bit (`saturation_spreads_the_scenes_chroma_about_its_neutral_and_holds_its_luminance`).
    for (label, k) in kernels(|_| {}) {
        assert!(k.negative.is_none_or(|n| !n.colours()), "{label}: the colour tools at rest lay something on the negative");
    }
    for (label, s) in pipes() {
        assert!(conversion::stage47(&s).negative.is_none(), "{label}: the colour tools at rest lay nothing on the negative");
    }
    // Twin: each colour tool moved lays itself on the negative of every pipe in colour, Lighting moved or not.
    let moves: [(&str, fn(&mut PipelineSettings)); 3] = [
        ("Saturation", |u| u.density = 0.4),
        ("a hue's range", |u| u.spectrogram.set(Band::Blue, 15.0, 0.6, 0.3)),
        ("the saturation curve", |u| {
            let points = [(0.0, 0.0), (0.25, 0.2), (0.5, 0.5), (0.75, 0.5), (1.0, 0.5)];
            u.saturation = SaturationCurve { points: points.map(|(x, y)| Point::new(x, y)).to_vec() };
        }),
    ];
    for (tool, moved) in moves {
        for (label, k) in kernels(moved).into_iter().filter(|(label, _)| !label.contains("black and white")) {
            assert!(k.negative.is_some_and(|n| n.colours()), "{label}: {tool} moved lays nothing");
        }
    }
}

#[test]
fn saturation_spreads_the_layers_and_the_paper_bounds_every_colour() {
    let scene = scene();
    println!("mean Oklab chroma of the colour patches, Saturation on the negative (on the print, as it was):");
    let mut checked = 0;
    for (label, s) in colour_pipes() {
        let rest = printed(&s, &scene);
        let at = |t: f32| printed(&with(&s, |u| u.density = t), &scene);
        let travels = [-1.0f32, -0.5, 0.0, 0.5, 1.0];
        let now: Vec<f64> = travels.iter().map(|t| mean_chroma(&at(*t))).collect();
        let was: Vec<f64> = travels.iter().map(|t| mean_chroma(&rest.iter().map(|v| on_the_print(*v, *t)).collect::<Vec<_>>())).collect();
        let row = |v: &[f64]| v.iter().map(|c| format!("{c:.4}")).collect::<Vec<_>>().join(" ");
        println!("  {label}: {} ({})", row(&now), row(&was));
        // More colour along the track, the greys kept.
        assert!(now.windows(2).all(|w| w[1] > w[0]), "{label}: {now:?}");
        // The frame's own grey keeps its negative; the same spread about a neutral a tenth of a log redder moves it.
        let full = with(&s, |u| u.density = 1.0);
        let k = conversion::stage47(&full);
        let n = k.negative.unwrap();
        let grey = scan_of(&full, oni_pipeline::paper::reference_grey(&full));
        let read = n.read(grey);
        let off = |n: &Negative| (0..3).map(|c| (n.redeveloped(read)[c] - read[c]).abs()).fold(0.0f32, f32::max);
        let redder = Negative { pivot: [n.pivot[0] + 0.1, n.pivot[1], n.pivot[2]], ..n };
        // A grey is read within a thousandth of a log of neutral, which the spread doubles.
        assert!(off(&n) < 4e-3 && off(&redder) > 2e-2, "{label}: the grey's negative moved by {}, {}", off(&n), off(&redder));
        // On a paper whose print is its dyes' own light, its pick and white on their anchors, every colour at the track's ends lies
        // within what its dyes print; the former print-side Saturation pushed twice as far, the twin, laid colours past them.
        let dyed = |p: &Paper| !p.luma && p.anchor == p.pick && p.white_anchor == p.white && rest.iter().all(|v| past_the_dyes(p, *v) < 1e-3);
        // The Frontier prints its fitted table, not its dyes' own light.
        let fitted = s.paper().map(|p| p.is_machine());
        if let Some(p) = conversion::stage47(&s).paper().filter(dyed).filter(|_| fitted != Some(true)) {
            println!("    {label}: read on its paper's dyes");
            checked += 1;
            for t in [-1.0f32, 1.0] {
                let past = at(t).iter().map(|v| past_the_dyes(&p, *v)).fold(0.0, f64::max);
                assert!(past < 1e-3, "{label}, Saturation {t}: {past} of density past the paper's dyes");
            }
            let former = rest.iter().map(|v| past_the_dyes(&p, on_the_print(*v, 2.0))).fold(0.0, f64::max);
            println!("    at twice the track's end the former Saturation laid colours {former:.3} of density past the paper's dyes");
            assert!(former > 1e-2, "{label}: {former}");
        }
    }
    // The Frontier's print is its fitted table's, the other papers' without Print lift laid on pure black: three prints are read.
    assert_eq!(checked, 3);
}

#[test]
fn a_hue_ranges_move_touches_that_range_alone() {
    let scene = scene();
    // Blue owns 240° to 1.8 times its neighbours' distance either side: Aqua 60° under, Purple 30° over.
    let owned = |hue: f32| (132.0..294.0).contains(&hue);
    let moved = |band: Band| {
        move |u: &mut PipelineSettings| {
            u.spectrogram.set(band, 15.0, 0.6, 0.3);
        }
    };
    for (label, s) in colour_pipes() {
        let rest = printed(&s, &scene);
        let blue = with(&s, moved(Band::Blue));
        let k = conversion::stage47(&blue);
        let hues: Vec<Option<f32>> = scanned(&blue, &scene).iter().map(|q| scene_hue(&k, *q)).collect();
        let off = |print: &[[f32; 3]]| {
            let (mut inside, mut outside) = (0.0f32, 0.0f32);
            for ((now, was), hue) in print.iter().zip(&rest).zip(&hues) {
                let change = (0..3).map(|c| (now[c] - was[c]).abs() / was[c].abs().max(1e-3)).fold(0.0f32, f32::max);
                match hue {
                    Some(h) if owned(*h) => inside = inside.max(change),
                    _ => outside = outside.max(change),
                }
            }
            (inside, outside)
        };
        let (inside, outside) = off(&printed(&blue, &scene));
        println!("{label}: Blue moved, its range by {inside:.3}, every other colour and grey by {outside:.1e}");
        assert!(inside > 0.05 && outside < 1e-4, "{label}: {inside} {outside}");
        // Twin: Aqua's move reaches past Blue's range.
        let (_, past) = off(&printed(&with(&s, moved(Band::Aqua)), &scene));
        assert!(past > 1e-3, "{label}: {past}");
    }
}

#[test]
fn the_saturation_curve_reads_the_prints_own_lightness() {
    let scene = scene();
    // Greyer shadows, the highlights kept.
    let points = [(0.0, 0.0), (0.25, 0.2), (0.5, 0.5), (0.75, 0.5), (1.0, 0.5)].map(|(x, y)| Point::new(x, y)).to_vec();
    for (label, s) in colour_pipes() {
        let curved = with(&s, |u| u.saturation = SaturationCurve { points: points.clone() });
        let k = conversion::stage47(&curved);
        let n = k.negative.expect("the curve lays on the negative");
        // A grey's lightness on the curve's axis is the print's own: its luma on a display gamma.
        let rest = printed(&s, &scene);
        let x = |v: [f32; 3]| (0..3).map(|c| LUMA[c] * v[c]).sum::<f32>().max(0.0).powf(1.0 / 2.2).min(1.0);
        let lightness = |table: &[f32; oni_render::LIGHTNESS_NODES], q: V4| {
            let about = as_grey(&n, q);
            Negative { lightness: *table, ..n }.lightness_at((0..3).map(|c| LUMA[c] * about[c]).sum())
        };
        let scans = scanned(&curved, &scene);
        let off = |table: &[f32; oni_render::LIGHTNESS_NODES]| (0..GREYS).map(|i| (lightness(table, scans[i]) - x(rest[i])).abs()).fold(0.0f32, f32::max);
        let shifted: [f32; oni_render::LIGHTNESS_NODES] = std::array::from_fn(|i| n.lightness[(i + 2).min(oni_render::LIGHTNESS_NODES - 1)]);
        let (held, twin) = (off(&n.lightness), off(&shifted));
        println!("{label}: a grey's lightness on the curve within {held:.1e} of its print's; two nodes off, {twin:.1e}");
        // A digital photo's greys print up to C* 4.7 off neutral (`digital_scan`), so their luma strays from a grey's by
        // 1.3e-2.
        assert!(held < 1.5e-2 && twin > 4e-2, "{label}: {held} {twin}");
        // Monotone along the greys: a lighter grey prints lighter.
        // The Frontier's table prints its greys a hair darker past its white, by 4e-4.
        assert!(n.lightness.windows(2).all(|w| w[1] - w[0] >= -1e-3), "{label}: {:?}", n.lightness);
        // The shadows lose colour, the highlights keep theirs.
        let now = printed(&curved, &scene);
        let (mut dark, mut light) = (Vec::new(), Vec::new());
        for i in GREYS..scene.len() {
            let ratio = chroma(now[i]) / chroma(rest[i]).max(1e-6);
            if x(rest[i]) < 0.2 && chroma(rest[i]) > 0.02 {
                dark.push(ratio);
            } else if x(rest[i]) > 0.6 && chroma(rest[i]) > 0.02 {
                light.push(ratio);
            }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        println!("  the curve keeps {:.2} of the shadows' chroma ({} patches), {:.2} of the highlights' ({})", mean(&dark), dark.len(), mean(&light), light.len());
        assert!(dark.is_empty() || mean(&dark) < 0.75, "{label}: {dark:?}");
        assert!(light.is_empty() || (mean(&light) - 1.0).abs() < 0.05, "{label}: {light:?}");
        assert!(!dark.is_empty() || !light.is_empty(), "{label}");
    }
}

#[test]
fn a_black_and_white_print_takes_no_colour_tool() {
    let tools = |u: &mut PipelineSettings| {
        u.density = 0.8;
        u.spectrogram.set(Band::Red, 10.0, 0.5, 0.5);
        u.saturation = SaturationCurve { points: [0.0, 0.25, 0.5, 0.75, 1.0].map(|x| Point::new(x, 0.8)).to_vec() };
    };
    for (label, s) in [("Endura Premier", endura(mono())), ("paper", mono())] {
        assert!(conversion::stage47(&with(&s, tools)).negative.is_none(), "{label}");
    }
    // Twin: a colour negative takes them.
    let n = conversion::stage47(&with(&endura(prepared_negative()), tools)).negative.expect("colour tools on the negative");
    assert!(n.spread > 1.0 && n.mixer.is_some() && n.curve.is_some());
}

#[test]
fn every_print_stays_monotone_along_a_colours_ramp() {
    const TOP: f32 = 5.0;
    // Three strong hues, each from four stops under the grey to one over, short of the films' shoulders.
    let ramps: Vec<Vec<V4>> = [[1.0f32, 0.35, 0.2], [0.25, 0.9, 0.35], [0.2, 0.35, 1.0]]
        .iter()
        .map(|h| (0..=140).map(|i| 0.184 * 2f32.powf(-4.0 + TOP * i as f32 / 140.0)).map(|g| [h[0] * g, h[1] * g, h[2] * g, 1.0]).collect())
        .collect();
    let luma = |v: [f32; 3]| (0..3).map(|c| LUMA[c] * v[c]).sum::<f32>();
    for (label, s) in colour_pipes() {
        for (name, f) in tools() {
            let t = with(&s, f);
            for ramp in &ramps {
                let y: Vec<f32> = printed(&t, ramp).into_iter().map(luma).collect();
                let turned = turn(&y);
                // A curve turning faster than the paper prints, alone or with Lighting, may trade a little light for
                // the colour it takes: the paper's dyes darken a strong colour more than a grey of its luminance.
                let bound = if name.starts_with("curve at") || name.starts_with("all") { 3e-2 } else { 5e-3 };
                if turned > 1e-4 {
                    println!("{label}, {name}: a colour's ramp turns back by {turned:.1e}");
                }
                assert!(turned < bound && y.iter().all(|v| v.is_finite()), "{label}, {name}: the print turns back by {turned:.2e}");
            }
        }
    }
    // Twin: a slope under none on the negative turns the print over.
    let s = with(&endura(digital()), |u| u.density = 0.5);
    let k = conversion::stage47(&s);
    let over = Stage47 { negative: k.negative.map(|n| Negative { slope: [-1.0; 3], ..n }), ..k };
    let y: Vec<f32> = scanned(&s, &ramps[0]).iter().map(|q| luma(rgb(cpu::stage47(*q, &over)))).collect();
    assert!(turn(&y) > 5e-3, "{}", turn(&y));
}

#[test]
fn the_gpu_lays_the_colour_tools_on_the_negative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_lays_the_colour_tools_on_the_negative_as_its_reference_does") else { return };
    let pixels = pixels();
    let source = g.upload(17, 26, &pixels).unwrap();
    for (label, s) in colour_pipes() {
        for (name, f) in tools() {
            let k = conversion::stage47(&with(&s, f));
            let n = k.negative.expect("the colour tools lay on the negative");
            // Read as the screen shows it: past its white, a toe read back four stops up spreads f32's last bits.
            let shown = |v: V4| [v[0].clamp(0.0, 1.0), v[1].clamp(0.0, 1.0), v[2].clamp(0.0, 1.0), v[3]];
            let out = mapped(&whole(&mut g, &stages::converted(Image::new(&source), &k)), shown);
            let reference = mapped(&pixels, |q| shown(cpu::stage47(q, &k)));
            let twin = Stage47 { negative: Some(Negative { spread: n.spread * 1.05 + 0.05, ..n }), ..k };
            judge(&format!("{label}, {name}"), &out, &reference, &mapped(&pixels, |q| shown(cpu::stage47(q, &twin))));
        }
    }
}
