//! `brightness_strips <out> <raw>...` (`SIDE`, `TAG`): each photo on its paper (the default where it has none) at Exposure
//! and Brightness −1 to +1 as PNG grids, its neutral's transfer plotted and measured; originals and sidecars only read.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_pipeline::{conversion, paper};
use oni_settings::{PipelineSettings, sidecar};
use std::path::Path;

type V3 = [f32; 3];

const STEPS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

/// CIE L*, a*, b* (D65) of linear Rec. 2020.
fn lab(v: V3) -> V3 {
    let m = [[0.636958, 0.144617, 0.168881], [0.262700, 0.677998, 0.059302], [0.0, 0.028073, 1.060985]];
    let xyz: V3 = std::array::from_fn(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2]);
    let white = [0.95047, 1.0, 1.08883];
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let [fx, fy, fz]: V3 = std::array::from_fn(|c| f(xyz[c].max(0.0) / white[c]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn srgb(p: [f32; 4]) -> [u8; 3] {
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    std::array::from_fn(|r| encode(m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]))
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

impl Canvas {
    fn new(w: usize, h: usize, fill: u8) -> Canvas {
        Canvas { w, h, px: vec![[fill; 3]; w * h] }
    }

    fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    fn tile(&mut self, l: &Linear, x0: usize, y0: usize) {
        for y in 0..l.height {
            for x in 0..l.width {
                self.put((x0 + x) as i64, (y0 + y) as i64, srgb(l.pixels[y * l.width + x]));
            }
        }
    }

    fn save(&self, path: &Path) {
        let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let mut enc = png::Encoder::new(file, self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&self.px.concat()).unwrap();
    }
}

/// The print of the neutral `t` of the way from the pick to the grey, under `s`.
fn neutral_print(s: &PipelineSettings, t: f32) -> V3 {
    let k = conversion::stage47(s);
    let [pick, _, grey] = paper::anchors(s, k.dyes.as_ref());
    let a: V3 = std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]));
    k.paper.unwrap().shown(a)
}

fn with(s: &PipelineSettings, exposure: f32, brightness: f32) -> PipelineSettings {
    let mut s = s.clone();
    s.print.exposure += exposure;
    s.print.brightness = brightness;
    s
}

/// The neutral ramp's L* from the pick to past the grey under `s`, on `n` steps of `t` up to `reach`.
fn ramp(s: &PipelineSettings, n: usize, reach: f32) -> Vec<V3> {
    let k = conversion::stage47(s);
    let p = k.paper.unwrap();
    let [pick, _, grey] = paper::anchors(s, k.dyes.as_ref());
    (0..=n).map(|i| {
        let t = reach * i as f32 / n as f32;
        lab(p.shown(std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]))))
    }).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    let side: usize = std::env::var("SIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(700);
    let tag = std::env::var("TAG").unwrap_or_else(|_| "now".into());
    std::fs::create_dir_all(out).unwrap();
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    for raw in args[1..].iter().map(Path::new) {
        let name = raw.file_stem().unwrap().to_string_lossy().to_string();
        let mut read = sidecar::read(raw, true).expect("a sidecar").settings;
        read.print_paper = read.print_paper.or(Some(Default::default()));
        read.print.brightness = 0.0;
        // The ramp's reach: the neutral up to L* 95 at rest.
        let reach = {
            let (mut lo, mut hi) = (0.0f32, 16.0f32);
            for _ in 0..40 {
                let m = 0.5 * (lo + hi);
                if lab(neutral_print(&read, m))[0] < 95.0 { lo = m } else { hi = m }
            }
            lo
        };
        let n = 2000;
        let rest = ramp(&read, n, reach);
        let k = conversion::stage47(&read);
        let p = k.paper.unwrap();
        let [pick, _, grey] = paper::anchors(&read, k.dyes.as_ref());
        let x_of = |i: usize| {
            let t = reach * i as f32 / n as f32;
            p.toward_light(p.pick_log, p.exposed(std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]))))
        };
        let xs: Vec<String> = [1.0f32, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0, 50.0, 70.0, 90.0].iter()
            .map(|m| format!("{m}:{:.3}", x_of(rest.iter().position(|l| l[0] >= *m).unwrap_or(n)))).collect();
        println!("{name}: paper {:?}, Print lift {}, pick L* {:.2}, reach {:.3}; log exposure from the pick by L* {}", read.print_paper, read.print.print_lift, rest[0][0], p.reach, xs.join(" "));
        // Measures on the neutral: each Brightness against Exposure at the same value.
        let values = [-4.0f32, -2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 4.0];
        let marks = [3.0f32, 5.0, 8.0, 12.0, 20.0, 30.0, 50.0, 70.0];
        let at_mark = |m: f32| rest.iter().position(|l| l[0] >= m).unwrap_or(n);
        let mut curves = Vec::new();
        for b in values {
            let bright = ramp(&with(&read, 0.0, b), n, reach);
            let expo = ramp(&with(&read, b, 0.0), n, reach);
            let moves: Vec<String> = marks.iter().map(|m| {
                let i = at_mark(*m);
                format!("{m}:{:+.1}/{:+.1}", bright[i][0] - rest[i][0], expo[i][0] - rest[i][0])
            }).collect();
            // The slope of L* out against L* at rest, over the ramp where the rest climbs from L* 1 to 90.
            let span: Vec<usize> = (1..n).filter(|i| rest[*i][0] > 1.0 && rest[*i + 1][0] < 90.0).collect();
            let slope = |c: &[V3], i: usize| (c[i + 1][0] - c[i][0]) / (rest[i + 1][0] - rest[i][0]).max(1e-6);
            let (lo, hi) = span.iter().fold((f32::MAX, f32::MIN), |(l, h), i| (l.min(slope(&bright, *i)), h.max(slope(&bright, *i))));
            let (elo, ehi) = span.iter().fold((f32::MAX, f32::MIN), |(l, h), i| (l.min(slope(&expo, *i)), h.max(slope(&expo, *i))));
            // The largest jump of that slope from one step to the next, a band's mark.
            let jump = span.windows(2).map(|w| (slope(&bright, w[1]) - slope(&bright, w[0])).abs()).fold(0.0f32, f32::max);
            let ejump = span.windows(2).map(|w| (slope(&expo, w[1]) - slope(&expo, w[0])).abs()).fold(0.0f32, f32::max);
            let fall = bright.windows(2).map(|w| w[0][0] - w[1][0]).fold(0.0f32, f32::max);
            let chroma = |c: &[V3]| c.iter().map(|v| v[1].hypot(v[2])).fold(0.0f32, f32::max);
            println!(
                "  {b:+}: L* moves Brightness/Exposure by L* at rest {}\n        slope {lo:.2}..{hi:.2} (Exposure {elo:.2}..{ehi:.2}), slope step {jump:.3} (Exposure {ejump:.3}), \
                 fall {fall:.2e}, neutral C* max {:.2} (Exposure {:.2}, rest {:.2})",
                moves.join(" "), chroma(&bright), chroma(&expo), chroma(&rest)
            );
            curves.push((b, bright, expo));
        }
        // The transfer: L* out against L* at rest, Brightness solid, Exposure faint; the slope below it.
        let (pw, ph) = (900usize, 900usize);
        let mut plot = Canvas::new(pw, ph + 300, 250);
        for g in 0..=10 {
            for k in 0..pw {
                plot.put(k as i64, (ph - g * ph / 10) as i64, [215; 3]);
                plot.put((g * pw / 10) as i64, k as i64, [215; 3]);
            }
        }
        let colour = |b: f32| -> [u8; 3] {
            let t = (b.abs() / 4.0).sqrt();
            if b > 0.0 { [(200.0 * (1.0 - t)) as u8, (120.0 * (1.0 - t)) as u8, 255] } else { [255, (120.0 * (1.0 - t)) as u8, (60.0 * (1.0 - t)) as u8] }
        };
        let faint = |c: [u8; 3]| c.map(|v| ((v as u16 + 2 * 250) / 3) as u8);
        for (b, bright, expo) in &curves {
            for i in 0..n {
                let x = (rest[i][0] / 100.0 * pw as f32) as i64;
                let ye = ph as i64 - (expo[i][0] / 100.0 * ph as f32) as i64;
                let yb = ph as i64 - (bright[i][0] / 100.0 * ph as f32) as i64;
                plot.put(x, ye, faint(colour(*b)));
                plot.put(x, yb, colour(*b));
                plot.put(x, yb + 1, colour(*b));
                // Slope panel: dL*/dL*rest from 0 to 6, Brightness only.
                let d = (rest[i + 1][0] - rest[i][0]).max(1e-6);
                let s = (bright[i + 1][0] - bright[i][0]) / d;
                if rest[i][0] > 0.5 {
                    plot.put(x, (ph + 300) as i64 - (s / 6.0 * 300.0) as i64, colour(*b));
                }
            }
        }
        for i in 0..n {
            let x = (rest[i][0] / 100.0 * pw as f32) as i64;
            plot.put(x, ph as i64 - (rest[i][0] / 100.0 * ph as f32) as i64, [0; 3]);
            plot.put(x, (ph + 300) as i64 - 50, [0; 3]);
        }
        plot.save(&out.join(format!("{name}-{tag}-curves.png")));
        // The renders: Exposure on the first row, Brightness on the second, −1 to +1.
        let negative = e.prepare(render::decode(raw, Reading::of(&read)).expect("a decode")).unwrap();
        let tiles: Vec<Vec<Linear>> = [true, false].iter().map(|expo| {
            STEPS.iter().map(|v| {
                let s = if *expo { with(&read, *v, 0.0) } else { with(&read, 0.0, *v) };
                let l = e.render(&negative, &s, Some(side)).unwrap();
                let lum = |p: &[f32; 4]| 0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2];
                let mean = l.pixels.iter().map(|p| lum(p).max(1e-5).ln()).sum::<f32>() / l.pixels.len() as f32;
                println!("  render {} {v:+}: log mean light {mean:.3}", if *expo { "Exposure" } else { "Brightness" });
                l
            }).collect()
        }).collect();
        let (tw, th) = (tiles[0][0].width, tiles[0][0].height);
        let gap = 8;
        let mut grid = Canvas::new(5 * tw + 4 * gap, 2 * th + gap, 255);
        for (r, row) in tiles.iter().enumerate() {
            for (c, l) in row.iter().enumerate() {
                grid.tile(l, c * (tw + gap), r * (th + gap));
            }
        }
        grid.save(&out.join(format!("{name}-{tag}-grid.png")));
        // The darkest third of the frame still holding some light, twice as large: rest, Brightness +1, Exposure +1,
        // Brightness −1, Exposure −1.
        let shown = [&tiles[1][2], &tiles[1][4], &tiles[0][4], &tiles[1][0], &tiles[0][0]];
        let (cw, ch) = (2 * (tw / 3), 2 * (th / 3));
        let mut zoom = Canvas::new(5 * cw + 4 * gap, ch, 255);
        // The same region in each: found once on the rest, then cut from each at that place.
        let (ox, oy) = {
            let l = &tiles[1][2];
            let (w3, h3) = (l.width / 3, l.height / 3);
            let lum = |p: &[f32; 4]| 0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2];
            let mut best = (f32::MAX, 0, 0);
            for oy in (0..l.height - h3).step_by(h3 / 4) {
                for ox in (0..l.width - w3).step_by(w3 / 4) {
                    let mut sum = 0.0;
                    for y in (oy..oy + h3).step_by(4) {
                        for x in (ox..ox + w3).step_by(4) {
                            sum += lum(&l.pixels[y * l.width + x]).max(1e-4).ln();
                        }
                    }
                    let score = (sum / ((h3 / 4) * (w3 / 4)) as f32 - (0.02f32).ln()).abs();
                    if score < best.0 { best = (score, ox, oy) }
                }
            }
            (best.1, best.2)
        };
        for (i, l) in shown.iter().enumerate() {
            let mut pixels = Vec::with_capacity(cw * ch);
            for y in 0..ch {
                for x in 0..cw {
                    pixels.push(l.pixels[(oy + y / 2) * l.width + ox + x / 2]);
                }
            }
            zoom.tile(&Linear { width: cw, height: ch, pixels }, i * (cw + gap), 0);
        }
        zoom.save(&out.join(format!("{name}-{tag}-shadows.png")));
    }
}
