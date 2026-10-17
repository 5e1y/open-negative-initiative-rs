//! Roll #8's DSC_6440 against the Swift app's own dump of it (`--render` and `--decode` at a 1000-pixel
//! side): the canvas mapping fed the Swift decode must land on the Swift render.

use oni_ui_poc::frame::{self, Linear, as_set, converted};
use oni_render::Stage47;
use serde_json::Value;

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, linear Rec. 2020.
fn dump(path: &std::path::Path) -> Linear {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let (width, height) = (int(0), int(4));
    let floats: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(floats.len(), width * height * 4, "{}: truncated", path.display());
    Linear { width, height, pixels: floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() }
}

/// Mean and 99th centile of the worst channel's gap, after the output clip both sides share.
fn gap(a: &Linear, b: &Linear) -> (f64, f64) {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let clip = |x: f32| x.clamp(0.0, 1.0);
    let mut e: Vec<f64> = a.pixels.iter().zip(&b.pixels).map(|(x, y)| (0..3).map(|c| (clip(x[c]) - clip(y[c])).abs() as f64).fold(0.0, f64::max)).collect();
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(|x, y| x.total_cmp(y));
    (mean, e[e.len() * 99 / 100])
}

fn luma(image: &Linear) -> f64 {
    image.pixels.iter().map(|p| (0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2]) as f64).sum::<f64>() / image.pixels.len() as f64
}

#[test]
fn the_canvas_mapping_lands_on_the_swift_render() {
    let Some(dir) = oni_fixtures::reference_dir("the_canvas_mapping_lands_on_the_swift_render") else { return };
    let Some(roll) = oni_fixtures::raw_dir("the_canvas_mapping_lands_on_the_swift_render") else { return };
    let list = std::fs::read_to_string(dir.join("list.txt")).expect("list.txt");
    let index = list.lines().position(|l| l.ends_with(frame::FRAME)).expect("the frame is in the dump's list");
    let name = format!("{index:05}.f32");
    let decode = dump(&dir.join("decode").join(&name));
    let render = dump(&dir.join("render").join(&name));
    let sidecar: Value = serde_json::from_slice(&std::fs::read(roll.join(format!("{}.json", frame::FRAME))).unwrap()).unwrap();
    let set = as_set(&sidecar).unwrap();

    let ours = converted(&decode, set.capture_stops, &set.stage47);
    let (mean, p99) = gap(&ours, &render);
    println!("against the Swift render: mean {mean:.5}, 99th centile {p99:.5}; luma {:.4} for {:.4}", luma(&ours), luma(&render));
    assert!(mean < 1e-3 && p99 < 5e-3, "the mapping departs from the Swift render: mean {mean}, p99 {p99}");

    // Twins: Hunt, the separation and the roll reference each move the result past a threshold.
    let hunt_off = Stage47 { hunt: 0.0, ..set.stage47 };
    let separation_off = Stage47 { sep_r: [1.0, 0.0, 0.0], sep_g: [0.0, 1.0, 0.0], sep_b: [0.0, 0.0, 1.0], ..set.stage47 };
    // The frame stands on its roll's reference, whose span ratio is 1 here: read without it, its own dose would blend the film's colour.
    let unreferenced = Stage47 { decompression: 0.5, film_share: 0.5, ..set.stage47 };
    for (what, k) in [("Hunt off", hunt_off), ("separation off", separation_off), ("roll reference ignored", unreferenced)] {
        let (m, p) = gap(&converted(&decode, set.capture_stops, &k), &render);
        println!("twin, {what}: mean {m:.5}, 99th centile {p:.5}");
        assert!(m >= 1e-3 || p >= 5e-3, "twin: {what} passes, so the check cannot see it");
    }
}

/// A measurement, not a check: how far LibRaw's pinned decode sits from `CIRAWFilter`'s on the same file,
/// the one thing the canvas cannot inherit from the Swift app.
#[test]
fn the_decoders_differ_by_a_measured_exposure() {
    let Some(dir) = oni_fixtures::reference_dir("the_decoders_differ_by_a_measured_exposure") else { return };
    let Some(roll) = oni_fixtures::raw_dir("the_decoders_differ_by_a_measured_exposure") else { return };
    let list = std::fs::read_to_string(dir.join("list.txt")).expect("list.txt");
    let index = list.lines().position(|l| l.ends_with(frame::FRAME)).unwrap();
    let swift = dump(&dir.join("decode").join(format!("{index:05}.f32")));
    let ours = frame::Frame::open(&roll.join(frame::FRAME)).unwrap().canvas;
    let mean = |image: &Linear, c: usize| image.pixels.iter().map(|p| p[c] as f64).sum::<f64>() / image.pixels.len() as f64;
    let ev: Vec<f64> = (0..3).map(|c| (mean(&ours, c) / mean(&swift, c)).log2()).collect();
    println!("LibRaw against CIRAWFilter, per channel: {:+.2} {:+.2} {:+.2} EV", ev[0], ev[1], ev[2]);
    assert!(ev.iter().all(|e| e.is_finite()), "both decodes carry signal");
}
