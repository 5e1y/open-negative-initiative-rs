//! A paper is a look baked in its file (`scripts/paper-looks.py`), the app solving nothing: every data sheet's paper prints
//! every frame as the app printed it when it laid the paper itself (`paper_look/before.json`), within ΔE 0.5 (Oklab × 100).
//! Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, noise};
use frames::*;
use oni_pipeline::conversion;
use oni_render::cpu;
use oni_settings::{PaperStock, PipelineSettings};
use std::collections::BTreeMap;
use std::path::PathBuf;

type Prints = BTreeMap<String, BTreeMap<String, Vec<[f32; 3]>>>;

/// The frames each paper prints: a negative, a digital photo and a black and white one at rest, a negative under
/// Lighting, and one brightened.
fn frames() -> Vec<(&'static str, PipelineSettings)> {
    let mut negative = prepared_negative();
    negative.print.print_lift = false;
    let mut lit = negative.clone();
    (lit.print.exposure, lit.print.contrast, lit.print.shadows) = (0.3, 0.2, -0.1);
    let mut bright = negative.clone();
    bright.print.brightness = 0.5;
    vec![("negative", negative), ("digital", digital()), ("black and white", mono()), ("Lighting", lit), ("Brightness", bright)]
}

/// Scan pixels over each frame's windows, a little past both ends.
fn scans(s: &PipelineSettings) -> Vec<V4> {
    let w = s.levels.windows();
    noise(8, 8, 7, -0.1, 1.1).iter().map(|u| scan_at(s, std::array::from_fn(|c| w[c].black + u[c] * (w[c].white - w[c].black)))).collect()
}

fn printed(paper: PaperStock, s: &PipelineSettings) -> Vec<[f32; 3]> {
    let on = PipelineSettings { print_paper: Some(paper), ..s.clone() };
    let k = conversion::stage47(&on);
    scans(s).iter().map(|q| cpu::stage47(*q, &k)).map(|v| [v[0], v[1], v[2]]).collect()
}

fn sheets() -> Vec<PaperStock> {
    PaperStock::all().into_iter().filter(|p| p.key() != "fujifilm-frontier-sp-3000").collect()
}

fn prints() -> Prints {
    sheets().into_iter().map(|p| (p.key().to_string(), frames().into_iter().map(|(f, s)| (f.to_string(), printed(p, &s))).collect())).collect()
}

/// Oklab of a linear Rec. 2020 light, through linear sRGB unclipped.
fn oklab(v: [f32; 3]) -> [f64; 3] {
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let l: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| m[r][c] * f64::from(v[c])).sum::<f64>().max(0.0));
    let lms = [
        0.4122214708 * l[0] + 0.5363325363 * l[1] + 0.0514459929 * l[2],
        0.2119034982 * l[0] + 0.6806995451 * l[1] + 0.1073969566 * l[2],
        0.0883024619 * l[0] + 0.2817188376 * l[1] + 0.6299787005 * l[2],
    ]
    .map(f64::cbrt);
    [
        0.2104542553 * lms[0] + 0.7936177850 * lms[1] - 0.0040720468 * lms[2],
        1.9779984951 * lms[0] - 2.4285922050 * lms[1] + 0.4505937099 * lms[2],
        0.0259040371 * lms[0] + 0.7827717662 * lms[1] - 0.8086757660 * lms[2],
    ]
}

fn delta(a: [f32; 3], b: [f32; 3]) -> f64 {
    let (p, q) = (oklab(a), oklab(b));
    100.0 * (0..3).map(|c| (p[c] - q[c]).powi(2)).sum::<f64>().sqrt()
}

fn worst(a: &[[f32; 3]], b: &[[f32; 3]]) -> f64 {
    a.iter().zip(b).map(|(x, y)| delta(*x, *y)).fold(0.0, f64::max)
}

fn kept() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/paper_look/before.json")
}

#[test]
fn every_data_sheet_paper_prints_every_frame_as_the_app_laid_it() {
    let now = prints();
    if std::env::var_os("ONI_PAPER_LOOK_KEEP").is_some() {
        std::fs::create_dir_all(kept().parent().unwrap()).unwrap();
        std::fs::write(kept(), serde_json::to_vec(&now).unwrap()).unwrap();
        return println!("kept {}", kept().display());
    }
    let before: Prints = serde_json::from_slice(&std::fs::read(kept()).unwrap()).unwrap();
    assert_eq!(before.keys().collect::<Vec<_>>(), now.keys().collect::<Vec<_>>(), "every data sheet's paper");
    for (paper, frames) in &now {
        for (frame, printed) in frames {
            let off = worst(printed, &before[paper][frame]);
            println!("{paper} · {frame}: within ΔE {off:.3}");
            assert!(off < 0.5, "{paper} · {frame}: ΔE {off}");
        }
    }
    // Twin: another paper's print of the same frames is told apart.
    let (endura, royal) = (&now["kodak-endura-premier"]["negative"], &before["kodak-royal-digital"]["negative"]);
    assert!(worst(endura, royal) > 1.0, "{}", worst(endura, royal));
}
