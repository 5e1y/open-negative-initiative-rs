//! Every branch's adversarial settings reach the render: a roll #8 frame and a Lumix frame, each
//! settings set of `adversarial/branches.json` rendered on LibRaw's decode and compared with the
//! frame's own render after the output clip.

mod common;

use common::gap;
use oni_graph::{Graph, Image};
use oni_pipeline::{Inputs, Pipeline, Request};
use std::path::Path;

/// A branch's settings move the render's 99th centile this far from the frame at rest, or reach nothing.
const MOVES: f64 = 5e-3;
/// The side the decode is reduced to.
const SIDE: usize = 1000;

fn merge(base: &mut serde_json::Value, over: &serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

/// The frame's sidecar with a branch's settings laid over it, read as the app reads a sidecar.
fn settings(original: &Path, over: &serde_json::Value, scratch: &Path) -> oni_settings::PipelineSettings {
    let mut doc: serde_json::Value = serde_json::from_slice(&std::fs::read(oni_settings::sidecar::url_for(original)).unwrap()).unwrap();
    merge(&mut doc["settings"], over);
    let copy = scratch.join(original.file_name().unwrap());
    std::fs::write(oni_settings::sidecar::url_for(&copy), serde_json::to_vec(&doc).unwrap()).unwrap();
    oni_settings::sidecar::read(&copy, true, false).expect("the sidecar reads").settings
}

/// The 99th-centile gap of `settings` from the frame at rest, on a LibRaw decode of `original`.
fn moves(g: &mut Graph, pipeline: &mut Pipeline, source: &oni_graph::Source, full_width: f64, rest: &[[f32; 4]], s: &oni_settings::PipelineSettings, whole: bool) -> f64 {
    let s = if whole { s.clone() } else { s.per_pixel_only() };
    let request = Request { full_width: Some(full_width), ..Request::default() };
    let image: Image = pipeline.image(g, source, &s, &request, &Inputs::default()).expect("the recipe builds");
    let e = image.extent();
    let rendered = g.render(&image, e, None).expect("the frame renders");
    // A branch that reframes the output reaches it as surely as one that moves its pixels.
    if rendered.len() == rest.len() { gap(&rendered, rest).1 } else { f64::INFINITY }
}

#[test]
fn every_branch_reaches_the_render_of_its_adversarial_settings() {
    let check = "every_branch_reaches_the_render_of_its_adversarial_settings";
    let Some(roll) = oni_fixtures::raw_dir(check) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(mut g) = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let mut pipeline = Pipeline::new();
    let branches: Vec<serde_json::Value> = serde_json::from_str(include_str!("adversarial/branches.json")).unwrap();
    let scratch = std::env::temp_dir().join(format!("oni-adversarial-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let mut frames = vec![roll.join("DSC_6440.NEF")];
    frames.extend(oni_fixtures::lumix_dir(check).map(|d| d.join("P1072392.RW2")));
    let mut failures = Vec::new();
    for original in &frames {
        let frame = original.file_name().unwrap().to_string_lossy().into_owned();
        let (decoded, factor) = oni_decode::decode_for(original, 1.0, SIDE).expect("LibRaw decodes the frame");
        let source = g.upload(decoded.width as u32, decoded.height as u32, &decoded.pixels).unwrap();
        let full_width = (decoded.width * factor) as f64;
        for whole in [false, true] {
            let at_rest_settings = settings(original, &serde_json::json!({}), &scratch);
            let s = if whole { at_rest_settings.clone() } else { at_rest_settings.per_pixel_only() };
            let request = Request { full_width: Some(full_width), ..Request::default() };
            let image: Image = pipeline.image(&mut g, &source, &s, &request, &Inputs::default()).expect("the recipe builds");
            let e = image.extent();
            let rest = g.render(&image, e, None).expect("the frame renders");
            // Twin: the frame's own settings move nothing from the frame at rest, so the bound sees a branch that reaches nothing.
            let none = moves(&mut g, &mut pipeline, &source, full_width, &rest, &at_rest_settings, whole);
            assert!(none <= MOVES, "{frame}: the frame at rest moves {none:.5} from itself");
            for b in branches.iter().filter(|b| b["whole"].as_bool().unwrap_or(false) == whole) {
                let name = b["name"].as_str().unwrap();
                let moved = moves(&mut g, &mut pipeline, &source, full_width, &rest, &settings(original, &b["settings"], &scratch), whole);
                println!("{frame} {name:18} moves {moved:.4}  ({})", b["branch"].as_str().unwrap());
                if moved <= MOVES {
                    failures.push(format!("{frame} {name}: reaches nothing, moves {moved:.5}"));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(failures.is_empty(), "{} branches reach nothing:\n{}", failures.len(), failures.join("\n"));
}
