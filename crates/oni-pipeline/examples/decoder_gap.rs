//! `decoder_gap <dumps> <sidecars> <r,g,b stops> <3x3 rows>`: each frame's two decodes rendered through its
//! sidecar, LibRaw's raw, gained, matrixed and with the gains in the sidecar, each against Apple's render.

use oni_graph::Graph;
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::PipelineSettings;
use std::path::Path;

type Px = Vec<[f32; 4]>;

fn dump(path: &Path) -> (u32, u32, Px) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let f: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    (int(0), int(4), f.chunks_exact(4).map(|c| [c[0], c[1], c[2], 1.0]).collect())
}

fn numbers(arg: &str) -> Vec<f32> {
    arg.split(',').map(|v| v.trim().parse().expect("a number")).collect()
}

/// Mean signed gap per channel after the output clip, and the mean of the worst channel's absolute gap.
fn gap(ours: &[[f32; 4]], apple: &[[f32; 4]]) -> ([f64; 3], f64) {
    let clip = |x: f32| f64::from(x.clamp(0.0, 1.0));
    let n = ours.len() as f64;
    let mut signed = [0f64; 3];
    let mut worst = 0f64;
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
    let gains = numbers(&args[2]);
    let m = numbers(&args[3]);
    let mut names: Vec<String> = std::fs::read_dir(dumps)
        .expect("dumps")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".apple.f32").map(str::to_owned))
        .filter(|n| sidecars.join(format!("{n}.json")).is_file())
        .collect();
    names.sort();
    let mut g = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    let arms = ["LibRaw raw", "gains at decode", "matrix at decode", "gains in sidecar"];
    let mut totals = vec![([0f64; 3], 0f64); arms.len()];
    let mut level = [0f64; 3];
    for name in &names {
        let settings = oni_settings::sidecar::read(&sidecars.join(name), true, false).expect("sidecar").settings.per_pixel_only();
        let (w, h, apple) = dump(&dumps.join(format!("{name}.apple.f32")));
        let (lw, lh, libraw) = dump(&dumps.join(format!("{name}.libraw.f32")));
        assert_eq!((w, h), (lw, lh), "{name}: the two dumps share one grid");
        let request = Request { full_width: Some(f64::from(w) * 8.0), ..Request::default() };
        let mut render = |pixels: &Px, s: &PipelineSettings| -> Px {
            let source = g.upload(w, h, pixels).expect("upload");
            let image = pipeline.image(&mut g, &source, s, &request, &Inputs::default()).expect("recipe");
            g.render(&image, image.extent(), None).expect("render")
        };
        let reference = render(&apple, &settings);
        let n = (reference.len() * names.len()) as f64;
        reference.iter().for_each(|p| (0..3).for_each(|c| level[c] += f64::from(p[c].clamp(0.0, 1.0)) / n));
        let gained: Px = libraw.iter().map(|p| [0, 1, 2, 3].map(|c| if c < 3 { p[c] * gains[c].exp2() } else { 1.0 })).collect();
        let matrixed: Px = libraw
            .iter()
            .map(|p| [0, 1, 2, 3].map(|r| if r < 3 { (0..3).map(|c| m[3 * r + c] * p[c]).sum() } else { 1.0 }))
            .collect();
        let mut migrated = settings.clone();
        (0..3).for_each(|c| migrated.gains.stops[c] += gains[c]);
        let outputs = [
            render(&libraw, &settings),
            render(&gained, &settings),
            render(&matrixed, &settings),
            render(&libraw, &migrated),
        ];
        print!("{name}");
        for (i, out) in outputs.iter().enumerate() {
            let (signed, worst) = gap(out, &reference);
            print!(" | {}: {:+.4} {:+.4} {:+.4} ({:.4})", arms[i], signed[0], signed[1], signed[2], worst);
            (0..3).for_each(|c| totals[i].0[c] += signed[c] / names.len() as f64);
            totals[i].1 += worst / names.len() as f64;
        }
        println!();
        g.purge();
    }
    println!("Apple's render, mean output R {:.4} G {:.4} B {:.4}", level[0], level[1], level[2]);
    println!("mean of {} frames, output minus Apple's render per channel (mean worst-channel gap):", names.len());
    for (arm, (signed, worst)) in arms.iter().zip(&totals) {
        println!("  {arm:18} R {:+.4}  G {:+.4}  B {:+.4}  ({worst:.4})", signed[0], signed[1], signed[2]);
    }
}
