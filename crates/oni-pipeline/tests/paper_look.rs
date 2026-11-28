//! A data sheet's paper reads its dyes through a broad band (`scripts/paper-looks.py`): the 3 × 3 that best stands for the
//! print through its dyes' spectra, its curves unmixed through it, so a grey prints as through the sheet's Status A
//! crosstalk while a saturated colour gives more of its chroma to the dyes' side absorptions. Each verdict has a twin.

use oni_pipeline::conversion;
use oni_pipeline::film::bake;
use oni_render::{PAPER_CURVE_NODES, Paper};
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};
use serde::Deserialize;

#[derive(Deserialize)]
struct Profile {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    crosstalk: [[f64; 3]; 3],
}

fn on(p: PaperStock) -> Paper {
    let s = PipelineSettings { print_paper: Some(p), ..PipelineSettings::rest(ConversionMode::Negative, true) };
    conversion::stage47(&s).paper().expect("a paper")
}

/// The same paper through its sheet's Status A crosstalk, its curves unmixed through it so its greys are the look's.
fn through_status_a(k: &Paper, p: PaperStock) -> Paper {
    let x = serde_json::from_str::<Profile>(&oni_pipeline::paper::source(p).expect("a data sheet's paper")).unwrap().data.crosstalk;
    let unmix = bake::inverse(&x);
    let curves: [[f32; 3]; PAPER_CURVE_NODES] = std::array::from_fn(|i| {
        let d = k.dyed(k.amounts([k.curve_low + k.curve_step * i as f32; 3]));
        bake::apply(&unmix, std::array::from_fn(|c| f64::from(d[c] - k.floor[c]))).map(|v| v as f32)
    });
    let q = Paper { curves, crosstalk: x.map(|r| r.map(|v| v as f32)), ..*k };
    Paper { pick: q.printed([0.0; 3]), white: q.printed([1.0; 3]), ..q }
}

/// CIELAB chroma of a linear Rec. 2020 light on D65.
fn chroma(v: [f32; 3]) -> f64 {
    let xyz = bake::apply(&bake::rec2020_to_xyz(), v.map(f64::from));
    let white = bake::apply(&bake::rec2020_to_xyz(), [1.0; 3]);
    let f = |t: f64| if t > (6.0f64 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [x, y, z] = std::array::from_fn(|c| f(xyz[c] / white[c]));
    (500.0 * (x - y)).hypot(200.0 * (y - z))
}

/// Six saturated colours in the window's middle: each channel, then each pair, three tenths off the others.
const SATURATED: [[f32; 3]; 6] = [[0.8, 0.5, 0.5], [0.5, 0.8, 0.5], [0.5, 0.5, 0.8], [0.2, 0.5, 0.5], [0.5, 0.2, 0.5], [0.5, 0.5, 0.2]];

#[test]
fn a_data_sheets_broad_band_prints_its_greys_as_status_a_and_flattens_its_saturated_colours_more() {
    for p in PaperStock::all().into_iter().filter(|p| !p.is_machine()) {
        let (k, a) = (on(p), through_status_a(&on(p), p));
        let grey = (0..=40).map(|i| i as f32 / 40.0).map(|t| (0..3).map(|c| (k.shown([t; 3])[c] - a.shown([t; 3])[c]).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
        let kept: Vec<f64> = SATURATED.iter().map(|s| chroma(k.shown(*s)) / chroma(a.shown(*s))).collect();
        let mean = kept.iter().sum::<f64>() / kept.len() as f64;
        println!("{}: a grey within {grey:.1e} of Status A's; saturated colours keep {mean:.3} of their Status A chroma ({kept:.3?})", p.name());
        assert!(grey < 1e-4 && mean < 0.97, "{p:?}: {grey} {mean}");
        // Twin: the Status A crosstalk read as the look keeps every colour's chroma.
        let same = SATURATED.iter().map(|s| chroma(a.shown(*s)) / chroma(through_status_a(&a, p).shown(*s))).fold(0.0f64, |m, v| m.max((v - 1.0).abs()));
        assert!(same < 1e-3 && k.crosstalk != a.crosstalk, "{p:?}: {same}");
    }
}
