//! `look_lab_strip <raw dir> <out dir> <frame>[@name=x,y,w,h]…`: each frame on the Frontier through five
//! cubes side by side, its regions measured in Oklab; `ONI_SWIFT_FRONTIER` adds Swift's render to compare.

use oni_graph::Graph;
use oni_pipeline::look_lab::measure::{lab_of, lch, median};
use oni_pipeline::look_lab::{FITTED, LabParams, SEED_NEUTRAL, bake};
use oni_pipeline::scanner::{self, LIFT, MACHINE};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_render::{ColorCube, ScannerEncode, ScannerOutput, cpu};
use oni_settings::PipelineSettings;
use oni_settings::modes::{Face, ScannerMachine};
use std::path::Path;

/// Each panel's longest side.
const SIDE: usize = 640;
/// The 240°, 270° and 300° sectors.
const BLUES: [usize; 3] = [8, 9, 10];

type Region = [f64; 4];

struct Picture {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

fn numbers(text: &str) -> Region {
    let v: Vec<f64> = text.split(',').map(|n| n.parse().expect("a fraction")).collect();
    [v[0], v[1], v[2], v[3]]
}

/// `name@sky=…@skin=…` into the frame's name and its named regions.
fn frame(arg: &str) -> (String, Vec<(String, Region)>) {
    let mut parts = arg.split('@');
    let name = parts.next().unwrap().to_owned();
    let regions = parts.map(|p| p.split_once('=').map(|(k, v)| (k.to_owned(), numbers(v))).expect("region=x,y,w,h")).collect();
    (name, regions)
}

/// The frame reduced by a whole factor, each pixel the mean of its block.
fn reduced(full: &oni_decode::Linear, side: usize) -> Picture {
    let f = full.width.max(full.height).div_ceil(side).max(1);
    let (w, h) = (full.width / f, full.height / f);
    let mut px = vec![[0f32, 0.0, 0.0, 1.0]; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0f64; 3];
            for dy in 0..f {
                for dx in 0..f {
                    let p = full.pixels[(y * f + dy) * full.width + x * f + dx];
                    (0..3).for_each(|c| sum[c] += f64::from(p[c]));
                }
            }
            (0..3).for_each(|c| px[y * w + x][c] = (sum[c] / (f * f) as f64) as f32);
        }
    }
    Picture { w, h, px }
}

/// The frame's own sidecar on the Frontier, its neighbourhood stages left out as `--render` does.
fn on_frontier(original: &Path) -> PipelineSettings {
    let mut s = oni_settings::sidecar::read(original, true, false).expect("the sidecar reads").settings.per_pixel_only();
    s.face = Face::Scanner;
    s.scanner_machine = ScannerMachine::Frontier;
    s
}

/// The Frontier's stages on the CPU through `cube`, as `Pipeline::frontier` runs them, in working space.
fn machine(p: &Picture, s: &PipelineSettings, cube: &ColorCube) -> Vec<[f32; 4]> {
    let k = scanner::density(s);
    let out = ScannerOutput { warmth_share: 0.0, lift: LIFT, contrast: s.scanner_finish.contrast };
    let lift = (-s.capture_stops).exp2();
    p.px.iter()
        .map(|v| {
            let code = cpu::color_cube(cpu::scanner_density([v[0] * lift, v[1] * lift, v[2] * lift, 1.0], &k), cube);
            cpu::output_clip(cpu::scanner_output(code, code, &out))
        })
        .collect()
}

/// Working space to display sRGB code.
fn display(v: [f32; 4]) -> [f64; 3] {
    let c = cpu::scanner_encode(cpu::output_clip(v), &ScannerEncode { root: 1.0 });
    [c[0], c[1], c[2]].map(f64::from)
}

/// Median lightness, chroma and hue over a region.
fn measured(px: &[[f32; 4]], w: usize, h: usize, r: &Region) -> [f64; 3] {
    let (x0, y0) = ((r[0] * w as f64) as usize, (r[1] * h as f64) as usize);
    let (x1, y1) = (((r[0] + r[2]) * w as f64) as usize, ((r[1] + r[3]) * h as f64) as usize);
    let lch: Vec<[f64; 3]> = (y0..y1.min(h)).flat_map(|y| (x0..x1.min(w)).map(move |x| y * w + x)).map(|i| lch(lab_of(display(px[i])))).collect();
    [0, 1, 2].map(|k| median(&lch.iter().map(|v| v[k]).collect::<Vec<_>>()))
}

fn write_png(path: &Path, w: usize, h: usize, panels: &[Vec<[f32; 4]>]) {
    let gap = 8;
    let width = panels.len() * w + (panels.len() - 1) * gap;
    let mut bytes = vec![255u8; width * h * 3];
    for (k, panel) in panels.iter().enumerate() {
        for y in 0..h {
            for x in 0..w {
                let c = display(panel[y * w + x]);
                let at = (y * width + k * (w + gap) + x) * 3;
                (0..3).for_each(|i| bytes[at + i] = (c[i].clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    let file = std::fs::File::create(path).expect("the strip");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&bytes).unwrap();
}

fn dump(path: &Path) -> Option<Picture> {
    let b = std::fs::read(path).ok()?;
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let f: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    Some(Picture { w: int(0), h: int(4), px: f.chunks_exact(4).map(|c| [c[0], c[1], c[2], 1.0]).collect() })
}

/// Mean and 99th centile of the worst channel's gap on the common top-left rows and columns.
fn gap(a: &Picture, b: &Picture) -> (f64, f64) {
    let (w, h) = (a.w.min(b.w), a.h.min(b.h));
    let mut e: Vec<f64> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (y, x)))
        .map(|(y, x)| {
            let (p, q) = (a.px[y * a.w + x], b.px[y * b.w + x]);
            (0..3).map(|c| f64::from((p[c].clamp(0.0, 1.0) - q[c].clamp(0.0, 1.0)).abs())).fold(0.0, f64::max)
        })
        .collect();
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(f64::total_cmp);
    (mean, e[e.len() * 99 / 100])
}

/// The app's own pipeline on the reduced frame against the CPU chain's measured-table panel.
fn against_app(g: &mut Graph, p: &Picture, s: &PipelineSettings, panel: &[[f32; 4]]) {
    let source = g.upload(p.w as u32, p.h as u32, &p.px).expect("upload");
    let image = Pipeline::new().image(g, &source, s, &Request::default(), &Inputs::default()).expect("recipe");
    let e = image.extent();
    let app = Picture { w: e.w as usize, h: e.h as usize, px: g.render(&image, e, None).expect("render") };
    let (mean, p99) = gap(&app, &Picture { w: p.w, h: p.h, px: panel.to_vec() });
    println!("  the strip's measured-table panel against the app's pipeline: mean {mean:.5}, p99 {p99:.5}");
}

/// The Rust pipeline on LibRaw's whole frame against Swift's render of the same frame on the Frontier.
fn against_swift(g: &mut Graph, swift: &Picture, full: &oni_decode::Linear, s: &PipelineSettings, regions: &[(String, Region)]) {
    let source = g.upload(full.width as u32, full.height as u32, &full.pixels).expect("upload");
    let scale = swift.w.max(swift.h) as f64 / full.width.max(full.height) as f64;
    let image = Pipeline::new().image(g, &source, s, &Request { scale, ..Request::default() }, &Inputs::default()).expect("recipe");
    let e = image.extent();
    let ours = Picture { w: e.w as usize, h: e.h as usize, px: g.render(&image, e, None).expect("render") };
    let (mean, p99) = gap(&ours, swift);
    println!("  Rust (LibRaw) against Swift on the Frontier, {}x{}: mean {mean:.4}, p99 {p99:.4}", ours.w, ours.h);
    for (name, r) in regions {
        let (a, b) = (measured(&ours.px, ours.w, ours.h, r), measured(&swift.px, swift.w, swift.h, r));
        println!("    {name}: Rust L {:.3} C {:.3} h {:.1}°, Swift L {:.3} C {:.3} h {:.1}°", a[0], a[1], a[2], b[0], b[1], b[2]);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (raws, out) = (Path::new(&args[0]), Path::new(&args[1]));
    std::fs::create_dir_all(out).expect("the output folder");
    let lab = &*FITTED;
    let mut moved: LabParams = lab.frontier.clone();
    moved.matrix[2] = lab.neutral.matrix[2];
    for s in BLUES {
        (moved.hue[s], moved.twist[s], moved.chroma[s]) = (lab.neutral.hue[s], lab.neutral.twist[s], lab.neutral.chroma[s]);
    }
    let mut seeded = moved.clone();
    seeded.matrix[2] = SEED_NEUTRAL.matrix[2];
    // The table, the Lab at Frontier bare, the Lab neutral, then two moves of skin and blue from Frontier.
    let cubes = [
        ("measured table", MACHINE.render.clone()),
        ("Lab Frontier, character 0", bake(&lab.frontier.with_character(0.0), 27)),
        ("Lab neutral", bake(&lab.neutral, 27)),
        ("Lab Frontier, G←R and 240–300° neutral", bake(&moved, 27)),
        ("idem, G←R at the seed's neutral", bake(&seeded, 27)),
    ];
    let swift = std::env::var_os("ONI_SWIFT_FRONTIER");
    let mut graph = Graph::new().map_err(|e| println!("no GPU ({e}): the strips are not checked against the app")).ok();
    for arg in &args[2..] {
        let (name, regions) = frame(arg);
        let original = raws.join(&name);
        let s = on_frontier(&original);
        let full = oni_decode::decode(&original, 1.0).expect("LibRaw decodes the frame");
        let picture = reduced(&full, SIDE);
        let panels: Vec<Vec<[f32; 4]>> = cubes.iter().map(|(_, cube)| machine(&picture, &s, cube)).collect();
        let path = out.join(format!("{}.png", name.rsplit_once('.').map_or(name.as_str(), |n| n.0)));
        write_png(&path, picture.w, picture.h, &panels);
        println!("{name} → {}", path.display());
        for (region, r) in &regions {
            for ((what, _), panel) in cubes.iter().zip(&panels) {
                let m = measured(panel, picture.w, picture.h, r);
                println!("  {region:5} {what:40} L {:.3}  C {:.3}  h {:6.1}°", m[0], m[1], m[2]);
            }
        }
        let Some(g) = graph.as_mut() else { continue };
        against_app(g, &picture, &s, &panels[0]);
        if let Some(swift) = swift.as_ref().and_then(|d| dump(&Path::new(d).join(format!("{name}.f32")))) {
            against_swift(g, &swift, &full, &s, &regions);
        }
        g.purge();
    }
}
