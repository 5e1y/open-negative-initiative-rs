//! Every branch's adversarial settings against the Swift app's own render of them: a roll #8 frame
//! and a Lumix frame, fed the Swift decode, each settings set rendered by both and compared after
//! the output clip; the settings are `adversarial/branches.json`, the Swift renders `ONI_ADVERSARIAL`.

use oni_graph::{Graph, Image};
use oni_pipeline::{Inputs, Pipeline, Request};
use std::path::{Path, PathBuf};

/// Fed the Swift decode, a branch the port follows lands within this of Swift's render.
const MEAN: f64 = 2e-3;
const P99: f64 = 1e-2;
/// A branch's settings move Swift's render's 99th centile this far from the frame at rest, or reach nothing.
const MOVES: f64 = 5e-3;

struct Dump {
    width: u32,
    height: u32,
    pixels: Vec<[f32; 4]>,
}

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, top row first.
fn dump(path: &Path) -> Dump {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let (width, height) = (int(0), int(4));
    let floats: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(floats.len(), (width * height * 4) as usize, "{}: truncated", path.display());
    Dump { width, height, pixels: floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() }
}

/// Mean, 99th centile and worst of the worst channel's gap, after the output clip both sides share.
fn gap(a: &[[f32; 4]], b: &[[f32; 4]]) -> (f64, f64, f64) {
    let clip = |x: f32| x.clamp(0.0, 1.0);
    let mut e: Vec<f64> = a.iter().zip(b).map(|(x, y)| (0..3).map(|c| f64::from((clip(x[c]) - clip(y[c])).abs())).fold(0.0, f64::max)).collect();
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(|x, y| x.total_cmp(y));
    (mean, e[e.len() * 99 / 100], *e.last().unwrap())
}

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

/// The Swift decode of a frame: its index in the reference's list, in the list's own folder.
fn decode(reference: &Path, list: &str, folder: &str, frame: &str) -> Dump {
    let names = std::fs::read_to_string(reference.join(list)).unwrap();
    let index = names.lines().position(|l| l.ends_with(frame)).unwrap_or_else(|| panic!("{frame} is not in {list}"));
    dump(&reference.join(folder).join(format!("{index:05}.f32")))
}

#[test]
fn every_branch_lands_on_the_swift_render_of_its_adversarial_settings() {
    let check = "every_branch_lands_on_the_swift_render_of_its_adversarial_settings";
    let Some(swift) = oni_fixtures::present(oni_fixtures::ADVERSARIAL_DIR, check) else { return };
    let Some(reference) = oni_fixtures::reference_dir(check) else { return };
    let (Some(roll), Some(lumix)) = (oni_fixtures::raw_dir(check), oni_fixtures::lumix_dir(check)) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(mut g) = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let mut pipeline = Pipeline::new();
    let branches: Vec<serde_json::Value> = serde_json::from_str(include_str!("adversarial/branches.json")).unwrap();
    let manifest: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(swift.join("manifest.json")).unwrap()).unwrap();
    let scratch = std::env::temp_dir().join(format!("oni-adversarial-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let frames: [(PathBuf, Dump); 2] = [
        (roll.join("DSC_6440.NEF"), decode(&reference, "list.txt", "decode", "DSC_6440.NEF")),
        (lumix.join("P1072392.RW2"), decode(&reference, "negs-list.txt", "negs-decode", "P1072392.RW2")),
    ];
    let mut failures = Vec::new();
    for (original, decoded) in &frames {
        let frame = original.file_name().unwrap().to_string_lossy().into_owned();
        let source = g.upload(decoded.width, decoded.height, &decoded.pixels).unwrap();
        let rest = serde_json::json!({"name": "rest", "branch": "the frame's own sidecar", "settings": {}});
        let rest_whole = serde_json::json!({"name": "rest-whole", "branch": "the frame's own sidecar", "whole": true, "settings": {}});
        let (per_pixel, whole): (Vec<&serde_json::Value>, Vec<&serde_json::Value>) = branches.iter().partition(|b| !b["whole"].as_bool().unwrap_or(false));
        let mut at_rest: [Option<Dump>; 2] = [None, None];
        for b in std::iter::once(&rest).chain(per_pixel).chain(std::iter::once(&rest_whole)).chain(whole) {
            let name = b["name"].as_str().unwrap();
            let whole = b["whole"].as_bool().unwrap_or(false);
            let entry = manifest.iter().find(|m| m["frame"] == frame.as_str() && m["name"] == name).unwrap_or_else(|| panic!("{frame} {name}: not dumped"));
            let folder = if whole { "render-whole" } else { "render" };
            let swift_render = dump(&swift.join(folder).join(format!("{:05}.f32", entry["index"].as_u64().unwrap())));
            let s = settings(original, &b["settings"], &scratch);
            let s = if whole { s } else { s.per_pixel_only() };
            // Swift's own grid for the crop: fed Swift's decode, the frame quantises where Swift's did.
            let request = Request { full_width: entry["fullWidth"].as_f64(), ..Request::default() };
            let image: Image = pipeline.image(&mut g, &source, &s, &request, &Inputs::default()).expect("the recipe builds");
            let e = image.extent();
            if (e.w, e.h) != (swift_render.width, swift_render.height) {
                println!("{frame} {name:18} {}: {}x{} for Swift's {}x{}", b["branch"].as_str().unwrap(), e.w, e.h, swift_render.width, swift_render.height);
                failures.push(format!("{frame} {name}: size"));
                continue;
            }
            let ours = g.render(&image, e, None).expect("the frame renders");
            let (mean, p99, worst) = gap(&ours, &swift_render.pixels);
            let ok = mean < MEAN && p99 < P99;
            // Twin: Swift's render against Swift's frame at rest, so settings that reach nothing are seen.
            let moves = match &at_rest[usize::from(whole)] {
                Some(r) if (r.width, r.height) == (swift_render.width, swift_render.height) => gap(&swift_render.pixels, &r.pixels).1,
                Some(_) => f64::INFINITY,
                None => f64::NAN,
            };
            let reached = moves.is_nan() || moves > MOVES;
            let verdict = if !ok { "DEPARTS" } else if !reached { "REACHES NOTHING" } else { "ok" };
            println!("{frame} {name:18} mean {mean:.5}  p99 {p99:.5}  max {worst:.4}  moves {moves:.4}  {verdict}  ({})", b["branch"].as_str().unwrap());
            if verdict != "ok" {
                failures.push(format!("{frame} {name}: {verdict}, mean {mean:.5}, p99 {p99:.5}, moves {moves:.5}"));
            }
            if at_rest[usize::from(whole)].is_none() {
                at_rest[usize::from(whole)] = Some(swift_render);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(failures.is_empty(), "{} branches depart from Swift:\n{}", failures.len(), failures.join("\n"));
}
