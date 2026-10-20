//! `render_pairs <dumps> <swift sidecars> <rust sidecars> <out> [dr,db,dc,dy]`: Apple's decode through the
//! Swift sidecar and LibRaw's through the Rust one, with probes of the red and blue gains and C and Y clicks.

use oni_graph::Graph;
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::PipelineSettings;
use oni_settings::modes::Face;
use std::path::Path;

type Px = Vec<[f32; 4]>;

/// The step of each probe: a gain in stops, a click in half-steps of density.
const GAIN_STEP: f32 = 0.05;
const CLICK_STEP: i64 = 2;

fn dump(path: &Path) -> (u32, u32, Px) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let f: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    (int(0), int(4), f.chunks_exact(4).map(|c| [c[0], c[1], c[2], 1.0]).collect())
}

fn write(path: &Path, w: u32, h: u32, px: &Px) {
    let mut bytes = Vec::with_capacity(8 + px.len() * 16);
    bytes.extend_from_slice(&(w as i32).to_le_bytes());
    bytes.extend_from_slice(&(h as i32).to_le_bytes());
    px.iter().flatten().for_each(|v| bytes.extend_from_slice(&v.to_le_bytes()));
    std::fs::write(path, bytes).expect("render written");
}

/// The fit's gains or matrix, as `decoder_fit` prints them, from the environment.
fn numbers(key: &str) -> Option<Vec<f32>> {
    std::env::var(key).ok().map(|v| v.split(',').map(|n| n.trim().parse().expect("a number")).collect())
}

fn settings(folder: &Path, name: &str) -> PipelineSettings {
    oni_settings::sidecar::read(&folder.join(name), true, false).expect("sidecar").settings.per_pixel_only()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dumps, swift, rust, out) = (Path::new(&args[0]), Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]));
    let fix: Option<Vec<f32>> = args.get(4).map(|a| a.split(',').map(|v| v.parse().expect("dr,db,dc,dy")).collect());
    std::fs::create_dir_all(out).expect("out folder");
    let mut names: Vec<String> = std::fs::read_dir(dumps)
        .expect("dumps")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".apple.f32").map(str::to_owned))
        .filter(|n| swift.join(format!("{n}.json")).is_file() && rust.join(format!("{n}.json")).is_file())
        .collect();
    names.sort();
    let mut g = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    for name in &names {
        let (s, r) = (settings(swift, name), settings(rust, name));
        let (w, h, apple) = dump(&dumps.join(format!("{name}.apple.f32")));
        let (lw, lh, libraw) = dump(&dumps.join(format!("{name}.libraw.f32")));
        assert_eq!((w, h), (lw, lh), "{name}: the two dumps share one grid");
        let request = Request { full_width: Some(f64::from(w) * 8.0), ..Request::default() };
        let mut render = |pixels: &Px, s: &PipelineSettings| -> Px {
            let source = g.upload(w, h, pixels).expect("upload");
            let image = pipeline.image(&mut g, &source, s, &request, &Inputs::default()).expect("recipe");
            g.render(&image, image.extent(), None).expect("render")
        };
        let mut arms: Vec<(&str, Px)> = Vec::new();
        if let Some(f) = &fix {
            let mut moved = r.clone();
            moved.gains.stops[0] += f[0];
            moved.gains.stops[2] += f[1];
            moved.scanner.c += f[2] as i64;
            moved.scanner.y += f[3] as i64;
            arms.push(("fix", render(&libraw, &moved)));
        } else {
            arms.push(("swift", render(&apple, &s)));
            arms.push(("rust", render(&libraw, &r)));
            arms.push(("rust-swiftsidecar", render(&libraw, &s)));
            if let Some(gains) = numbers("DECODER_GAINS") {
                let gained: Px = libraw.iter().map(|p| [0, 1, 2, 3].map(|c| if c < 3 { p[c] * gains[c].exp2() } else { 1.0 })).collect();
                arms.push(("rust-gains", render(&gained, &s)));
            }
            if let Some(m) = numbers("DECODER_MATRIX") {
                let matrixed: Px = libraw
                    .iter()
                    .map(|p| [0, 1, 2, 3].map(|r| if r < 3 { (0..3).map(|c| m[3 * r + c] * p[c]).sum() } else { 1.0 }))
                    .collect();
                arms.push(("rust-matrix", render(&matrixed, &s)));
            }
            for (arm, channel) in [("dr", 0), ("db", 2)] {
                let mut moved = r.clone();
                moved.gains.stops[channel] += GAIN_STEP;
                arms.push((arm, render(&libraw, &moved)));
            }
            if r.face == Face::Scanner {
                let mut moved = r.clone();
                moved.scanner.c += CLICK_STEP;
                arms.push(("dc", render(&libraw, &moved)));
                let mut moved = r.clone();
                moved.scanner.y += CLICK_STEP;
                arms.push(("dy", render(&libraw, &moved)));
            }
        }
        for (arm, px) in &arms {
            write(&out.join(format!("{name}.{arm}.f32")), w, h, px);
        }
        println!("{name}: {w}x{h}, {:?} face, {} arms", r.face, arms.len());
        g.purge();
    }
}
