//! `residual <dumps> <sidecars> <r,g,b stops>`: each frame rendered through the real pipeline from
//! `decoder_gap`'s block dumps: Apple's decode with the Swift app's sidecar, the reference; LibRaw's
//! with that sidecar; LibRaw's with the sidecar the transition corrected, written and read back.

use oni_graph::Graph;
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::{PipelineSettings, sidecar};
use std::path::Path;

type Px = Vec<[f32; 4]>;

fn dump(path: &Path) -> (u32, u32, Px) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let f: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    (int(0), int(4), f.chunks_exact(4).map(|c| [c[0], c[1], c[2], 1.0]).collect())
}

/// Mean signed gap per channel after the output clip, and the mean of each pixel's worst channel.
fn gap(ours: &[[f32; 4]], apple: &[[f32; 4]]) -> ([f64; 3], f64) {
    let clip = |x: f32| f64::from(x.clamp(0.0, 1.0));
    let n = ours.len() as f64;
    let (mut signed, mut worst) = ([0f64; 3], 0f64);
    for (o, a) in ours.iter().zip(apple) {
        let d = [0, 1, 2].map(|c| clip(o[c]) - clip(a[c]));
        (0..3).for_each(|c| signed[c] += d[c] / n);
        worst += d.iter().map(|v| v.abs()).fold(0.0, f64::max) / n;
    }
    (signed, worst)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dumps, sidecars) = (Path::new(&args[0]), Path::new(&args[1]));
    let stops: Vec<f64> = args[2].split(',').map(|v| v.trim().parse().expect("a number")).collect();
    let stops = [stops[0], stops[1], stops[2]];
    let mut names: Vec<String> = std::fs::read_dir(dumps)
        .expect("dumps")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".apple.f32").map(str::to_owned))
        .filter(|n| sidecars.join(format!("{n}.json")).is_file())
        .collect();
    names.sort();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let (support, roll) = (std::env::temp_dir().join(format!("oni-residual-support-{stamp}")), std::env::temp_dir().join(format!("oni-residual-roll-{stamp}")));
    std::fs::create_dir_all(&roll).expect("roll copy");
    let mut g = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    let mut totals = [([0f64; 3], 0f64); 2];
    for name in &names {
        let raw = roll.join(name);
        std::fs::write(&raw, b"stands for the RAW").expect("stand-in");
        std::fs::copy(sidecars.join(format!("{name}.json")), sidecar::url_for(&raw)).expect("sidecar copy");
        let swift = sidecar::read(&raw, true, false).expect("Swift's sidecar").settings.per_pixel_only();
        let entry = oni_transition::correct::correct(&support, &raw, stops);
        assert_eq!(entry.outcome, oni_transition::records::Outcome::Corrected, "{name}: {entry:?}");
        let corrected = sidecar::read(&raw, true, false).expect("corrected sidecar").settings.per_pixel_only();
        let (w, h, apple) = dump(&dumps.join(format!("{name}.apple.f32")));
        let (_, _, libraw) = dump(&dumps.join(format!("{name}.libraw.f32")));
        let request = Request { full_width: Some(f64::from(w) * 8.0), ..Request::default() };
        let mut render = |pixels: &Px, s: &PipelineSettings| -> Px {
            let source = g.upload(w, h, pixels).expect("upload");
            let image = pipeline.image(&mut g, &source, s, &request, &Inputs::default()).expect("recipe");
            g.render(&image, image.extent(), None).expect("render")
        };
        let reference = render(&apple, &swift);
        let outputs = [render(&libraw, &swift), render(&libraw, &corrected)];
        print!("{name}");
        for (i, out) in outputs.iter().enumerate() {
            let (signed, worst) = gap(out, &reference);
            print!(" | {}: {:+.4} {:+.4} {:+.4} ({worst:.4})", ["before", "after"][i], signed[0], signed[1], signed[2]);
            (0..3).for_each(|c| totals[i].0[c] += signed[c] / names.len() as f64);
            totals[i].1 += worst / names.len() as f64;
        }
        println!();
        g.purge();
    }
    println!("mean of {} frames, LibRaw's render minus the Swift render per channel (mean worst-channel gap):", names.len());
    for (arm, (signed, worst)) in ["before the transition", "after the transition"].iter().zip(&totals) {
        println!("  {arm:22} R {:+.4}  G {:+.4}  B {:+.4}  ({worst:.4})", signed[0], signed[1], signed[2]);
    }
    let _ = std::fs::remove_dir_all(&roll);
    let _ = std::fs::remove_dir_all(&support);
}
