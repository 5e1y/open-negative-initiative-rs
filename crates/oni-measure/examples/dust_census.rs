//! `dust_census <frame>…`: each frame's dust at five sensitivities: the shapes the veil shows, counted by size, how
//! many a ring holds, and of the strokes its sidecar holds, the sensitivity at which the veil shows each and a ring
//! falls on it; `ONI_DUST_OUT` also writes each frame's view, its map of what shows from where, and its rings.

use oni_graph::{Graph, Image};
use oni_measure::dust::{self, Limits, Speck};
use oni_pipeline::{Inputs, Pipeline};
use oni_settings::frame::Correction;
use oni_settings::sidecar;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

const UPPER: [u32; 8] = [2, 8, 32, 128, 512, 2048, 8192, u32::MAX];
const NAMES: [&str; 8] = ["1-2", "3-8", "9-32", "33-128", "129-512", "513-2k", "2k-8k", ">8k"];
/// The sensitivities read, and the inset each shows: the reach of 0.8 density in four steps.
const AT: [(f32, f32); 5] = [(0.0, 0.0), (0.25, 0.2), (0.5, 0.4), (0.75, 0.6), (1.0, 0.8)];
/// A stroke's size by its brush's radius, in pixels.
const RADII: [(f64, &str); 4] = [(12.0, "r<12"), (20.0, "r12-20"), (40.0, "r20-40"), (f64::MAX, "r>=40")];

fn cells(s: &Speck, width: usize) -> impl Iterator<Item = usize> + '_ {
    s.runs.iter().flat_map(move |r| (r.start..r.end).map(move |x| r.row as usize * width + x as usize))
}

/// The pixels a stroke's brush covers.
fn covered(c: &Correction, width: usize, height: usize) -> HashSet<usize> {
    let points: Vec<(f64, f64)> = c.stroke.iter().map(|p| (p.x * width as f64, p.y * height as f64)).collect();
    let near = |x: f64, y: f64| {
        points.windows(2).map(|w| segment(x, y, w[0], w[1])).chain(points.first().map(|p| (x - p.0).hypot(y - p.1))).fold(f64::MAX, f64::min) <= c.radius
    };
    let (x0, x1) = points.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = points.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let span = |lo: f64, hi: f64, end: usize| ((lo - c.radius).floor().max(0.0) as usize)..((hi + c.radius).ceil().min(end as f64 - 1.0) as usize + 1);
    span(y0, y1, height).flat_map(|y| span(x0, x1, width).map(move |x| (x, y))).filter(|&(x, y)| near(x as f64 + 0.5, y as f64 + 0.5)).map(|(x, y)| y * width + x).collect()
}

fn segment(x: f64, y: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l <= 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
    (x - (a.0 + t * dx)).hypot(y - (a.1 + t * dy))
}

fn main() {
    let out = std::env::var_os("ONI_DUST_OUT").map(PathBuf::from);
    let mut graph = Graph::new().expect("a GPU");
    let mut pipeline = Pipeline::new();
    for name in std::env::args().skip(1) {
        let path = Path::new(&name);
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let mut settings = sidecar::read(path, true).expect("a sidecar").settings;
        let painted = std::mem::take(&mut settings.corrections);
        let (d, _) = oni_decode::decode_for(path, 1.0, 0).expect("a decode");
        let source = graph.upload_half(d.width as u32, d.height as u32, &d.pixels).unwrap();
        let scan = (d.width, d.height);
        drop(d);
        let framed = pipeline.framed(&mut graph, Image::new(&source), &settings, 1.0, &Inputs::default()).unwrap();
        let e = framed.extent();
        let pixels = graph.render(&framed, e, None).unwrap();
        graph.purge();
        let (width, height) = (e.w as usize, e.h as usize);
        // A stroke is kept on the scan; the census reads it on the frame.
        let placing = settings.geometry.placing((scan.0 as f64, scan.1 as f64));
        let painted: Vec<Correction> = painted.into_iter().map(|c| Correction { stroke: c.stroke.iter().map(|p| placing.on_frame(*p)).collect(), ..c }).collect();
        let limits = Limits::of(&settings);
        let started = std::time::Instant::now();
        let reading = dust::read(&pixels, width, &limits, 3072);
        println!("{stem}: {width} x {height}, read in {:.0} ms, {} px past the window", started.elapsed().as_secs_f64() * 1e3, reading.past);
        let ringed: Vec<HashSet<usize>> = AT.iter().map(|&(s, _)| reading.at(s).iter().flat_map(|r| cells(r, width)).collect()).collect();
        let shown: Vec<Vec<Speck>> = AT.iter().map(|&(_, inset)| dust::shapes(&pixels, width, &limits.closed_in(inset))).collect();
        for (k, (s, _)) in AT.iter().enumerate() {
            let mut counts = [[0u32; 2]; 8];
            for shape in &shown[k] {
                let b = UPPER.iter().position(|&u| shape.area <= u).unwrap();
                counts[b][0] += 1;
                counts[b][1] += u32::from(cells(shape, width).any(|i| ringed[k].contains(&i)));
            }
            let total = counts.iter().fold([0, 0], |t, c| [t[0] + c[0], t[1] + c[1]]);
            let sizes: Vec<String> = NAMES.iter().enumerate().filter(|(b, _)| counts[*b][0] > 0).map(|(b, n)| format!("{n} {}/{}", counts[b][1], counts[b][0])).collect();
            println!("  {:>3.0}%: {} rings; shapes shown held by one {}/{}: {}", s * 100.0, reading.at(*s).len(), total[1], total[0], if total[0] > 2000 { "(too many to list)".to_string() } else { sizes.join("  ") });
        }
        let mut first = [0u32; 6];
        for shape in &shown[0] {
            first[ringed.iter().position(|held| cells(shape, width).any(|i| held.contains(&i))).unwrap_or(5)] += 1;
        }
        println!("  shown at rest, first held at 0/25/50/75/100%: {:?}, never {}", &first[..5], first[5]);
        if !painted.is_empty() {
            // Each stroke: where the veil first shows a pixel under it, and where a ring first falls on it.
            let past: Vec<HashSet<usize>> = shown.iter().map(|shapes| shapes.iter().flat_map(|s| cells(s, width)).collect()).collect();
            let unders: Vec<HashSet<usize>> = painted.iter().map(|c| covered(c, width, height)).collect();
            let anywhere: HashSet<usize> = unders.iter().flatten().copied().collect();
            let mut table = vec![[[0u32; 6]; 6]; RADII.len()];
            for (c, under) in painted.iter().zip(&unders) {
                let seen = past.iter().position(|p| under.iter().any(|i| p.contains(i))).unwrap_or(5);
                let found = ringed.iter().position(|r| under.iter().any(|i| r.contains(i))).unwrap_or(5);
                table[RADII.iter().position(|&(r, _)| c.radius < r).unwrap()][seen][found] += 1;
            }
            let rings_on: Vec<usize> = AT.iter().map(|&(s, _)| reading.at(s).iter().filter(|r| cells(r, width).any(|i| anywhere.contains(&i))).count()).collect();
            println!("  {} strokes painted; rings on a stroke at 0/25/50/75/100%: {:?} of {:?}", painted.len(), rings_on, AT.iter().map(|&(s, _)| reading.at(s).len()).collect::<Vec<_>>());
            for (b, (_, name)) in RADII.iter().enumerate() {
                for seen in 0..6 {
                    let row = table[b][seen];
                    if row.iter().sum::<u32>() > 0 {
                        println!("    {name:>7}, shown from {:>5}: ringed from 0/25/50/75/100% {:?}, never {}", ["0%", "25%", "50%", "75%", "100%", "never"][seen], &row[..5], row[5]);
                    }
                }
            }
        }
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            let on = settings.roll_reference.as_ref().map(|r| oni_core::balance::written(r, &settings));
            let on = on.as_ref().unwrap_or(&settings);
            let windows = [on.levels.red, on.levels.green, on.levels.blue];
            let view: Vec<u8> = pixels.iter().flat_map(|p| windows.iter().enumerate().map(|(c, w)| ((((limits.density(p, c) - w.black) / (w.white - w.black)).clamp(0.0, 1.0)).powf(1.0 / 2.2) * 255.0).round() as u8).collect::<Vec<_>>()).collect();
            let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{stem}-view.ppm"))).unwrap());
            write!(f, "P6\n{width} {height}\n255\n").unwrap();
            f.write_all(&view).unwrap();
            let mut map = vec![255u8; width * height];
            for (k, shapes) in shown.iter().enumerate().rev() {
                for i in shapes.iter().flat_map(|s| cells(s, width)) {
                    map[i] = k as u8;
                }
            }
            let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{stem}-past.pgm"))).unwrap());
            write!(f, "P5\n{width} {height}\n255\n").unwrap();
            f.write_all(&map).unwrap();
            let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{stem}-rings.csv"))).unwrap());
            writeln!(f, "s,area,x0,y0,x1,y1").unwrap();
            for (s, _) in AT {
                for r in reading.at(s) {
                    writeln!(f, "{s},{},{},{},{},{}", r.area, r.bounds[0], r.bounds[1], r.bounds[2], r.bounds[3]).unwrap();
                }
            }
        }
    }
}
