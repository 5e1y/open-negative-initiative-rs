//! Format 46 keeps every look: each Scanner-face case and corpus sidecar renders to the bit as `legacy_print/fingerprints.json`
//! holds, format 45's look on format 66's unmixed data-sheet dyes, Lighting and Saturation at rest or moving the
//! negative: the whole chain on the GPU, stage 4-7 and the look on the CPU; a slide Pro rendered, on the GPU its
//! projector's since format 70.

use oni_graph::Graph;
use oni_pipeline::{Inputs, Pipeline, Request, conversion, frontier_look, oni_look, scanner};
use oni_render::{ScannerEncode, ScannerOutput, cpu};
use oni_settings::{LegacyMachine, PipelineSettings};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const W: u32 = 96;
const H: u32 = 24;

/// A negative's transmittances: a density ramp over its orange mask, in six tinted bands.
fn scene() -> Vec<[f32; 4]> {
    let tints = [[0.0, 0.0, 0.0], [0.25, 0.05, -0.1], [-0.15, 0.1, 0.3], [0.1, -0.2, 0.05], [0.3, 0.3, 0.3], [-0.1, -0.1, -0.2]];
    let base = [0.24f32, 0.62, 0.92];
    (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .map(|(x, y)| {
            let d = 2.2 * x as f32 / (W - 1) as f32;
            let t = tints[(y / 4) as usize];
            let c = |i: usize| 10f32.powf(-(base[i] + d * [0.62, 0.66, 0.7][i] + t[i]));
            [c(0), c(1), c(2), 1.0]
        })
        .collect()
}

/// FNV-1a over every output's bits.
fn fingerprint(px: &[[f32; 4]]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in px.iter().flatten() {
        for b in v.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// The scene through stage 4-7 and the legacy print's look on the CPU, as the Scanner face laid them.
fn cpu_print(px: &[[f32; 4]], s: &PipelineSettings) -> Vec<[f32; 4]> {
    let laid = scanner::on_legacy_print(s);
    let k = conversion::stage47(laid.as_ref().unwrap_or(s));
    let m = &*scanner::MACHINE;
    px.iter()
        .map(|p| {
            let out = cpu::stage47(*p, &k);
            let Some(legacy) = s.legacy() else { return out };
            let looked = if s.paper().is_some() {
                if legacy.w == 0 {
                    return out;
                }
                cpu::scanner_encode(out, &ScannerEncode { root: 1.0 })
            } else {
                let frontier = legacy.machine == LegacyMachine::Frontier;
                let code = cpu::scanner_encode(out, &ScannerEncode { root: if frontier { 1.0 } else { oni_look::ROOT } });
                cpu::color_cube(code, if frontier { &frontier_look::CUBE } else { &oni_look::CUBE })
            };
            let (warmth, share) = m.warmth(legacy.w).map_or((looked, 0.0), |(cube, share)| (cpu::color_cube(looked, cube), share));
            cpu::scanner_output(looked, warmth, &ScannerOutput { warmth_share: share, lift: scanner::LIFT, contrast: 0.0 })
        })
        .collect()
}

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Every document the fixture names, as stored, and its two fingerprints of format 45.
fn documents() -> Vec<(String, Vec<u8>, [String; 2])> {
    let text = std::fs::read(here().join("legacy_print/fingerprints.json")).unwrap();
    let kept: BTreeMap<String, [String; 2]> = serde_json::from_slice(&text).unwrap();
    let corpus = here().join("../../oni-settings/tests/corpus");
    kept.into_iter()
        .map(|(name, prints)| {
            let path = match name.strip_prefix("corpus/") {
                Some(rest) => corpus.join(rest),
                None => here().join("legacy_print").join(format!("{name}.json")),
            };
            (name, std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())), prints)
        })
        .collect()
}

fn read(bytes: &[u8]) -> PipelineSettings {
    oni_settings::migrate::document(bytes).unwrap().document.settings
}

/// The document as if it named `face`, every other field as stored.
fn on_face(bytes: &[u8], face: &str) -> PipelineSettings {
    let mut v: Value = serde_json::from_slice(bytes).unwrap();
    v["settings"]["face"] = Value::from(face);
    read(&serde_json::to_vec(&v).unwrap())
}

#[test]
fn every_photo_prints_its_machine_s_look_as_format_45_did_on_the_cpu() {
    let (mut scanned, mut pro) = (0, 0);
    for (name, bytes, [_, want]) in documents() {
        let s = read(&bytes);
        let print = |t: &PipelineSettings| fingerprint(&cpu_print(&scene(), t));
        assert_eq!(print(&s), want, "{name} moved");
        let stored: Value = serde_json::from_slice(&bytes).unwrap();
        if stored["settings"]["face"] == "scanner" {
            // Twins: a photo the Scanner face rendered, read as Pro's or migrated without its legacy print, moves.
            scanned += 1;
            assert_ne!(print(&on_face(&bytes, "pro")), want, "{name} read as Pro's");
            if s.legacy_print.is_some() {
                assert_ne!(print(&PipelineSettings { legacy_print: None, ..s.clone() }), want, "{name} without its legacy print");
            }
        } else {
            // Twin: a photo Pro rendered, its Scanner settings taken instead, moves.
            pro += 1;
            assert_ne!(print(&on_face(&bytes, "scanner")), want, "{name} read as the Scanner's");
        }
    }
    println!("{scanned} photos the Scanner face rendered and {pro} Pro rendered print to the bit");
    assert!(scanned >= 20 && pro >= 20, "{scanned} {pro}");
}

/// The whole chain on the GPU, bit for bit, where the fixture's bits were drawn: Metal on Apple silicon. A float's last
/// bit differs from one driver to the next, so elsewhere the CPU fold above stands alone.
#[test]
fn every_photo_renders_whole_as_format_45_did_on_the_gpu() {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return println!("SKIPPED every_photo_renders_whole_as_format_45_did_on_the_gpu: its bits are Metal's on Apple silicon");
    }
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED every_photo_renders_whole_as_format_45_did_on_the_gpu: no GPU") };
    let source = g.upload(W, H, &scene()).unwrap();
    let mut render = |s: &PipelineSettings| {
        let image = Pipeline::new().image(&mut g, &source, s, &Request::default(), &Inputs::default()).unwrap();
        fingerprint(&g.render(&image, image.extent(), None).unwrap())
    };
    for (name, bytes, [want, _]) in documents() {
        let s = read(&bytes);
        assert_eq!(render(&s), want, "{name} moved");
        // Twin: off its legacy print, laid no more, a Scanner photo moves.
        if let Some(l) = s.legacy_print {
            assert_ne!(render(&PipelineSettings { legacy_print: Some(oni_settings::LegacyPrint { laid: false, ..l }), ..s.clone() }), want, "{name}");
        }
    }
}
