//! `brightness_knee <out> <knee L*> <raw>...`: each photo on its paper, its tones in L*, Brightness's candidate shapes
//! against Exposure on a ramp and a chart, and with `RENDERS` set renders as sRGB PPM; originals and sidecars only read.

use oni_app::render::{self, Device, Engine, Linear, Reading};
use oni_pipeline::{conversion, paper};
use oni_render::Paper;
use oni_settings::{PipelineSettings, sidecar};
use std::io::Write;
use std::path::Path;

type V3 = [f32; 3];

const GAMMA: f32 = 0.6;
const LOG2: f32 = std::f32::consts::LOG10_2;

fn mean(v: V3) -> f32 {
    (v[0] + v[1] + v[2]) / 3.0
}

fn luma(v: V3) -> f32 {
    0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]
}

/// CIE L*, a*, b* (D65) of linear Rec. 2020.
fn lab(v: V3) -> V3 {
    let m = [[0.636958, 0.144617, 0.168881], [0.262700, 0.677998, 0.059302], [0.0, 0.028073, 1.060985]];
    let xyz: V3 = std::array::from_fn(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2]);
    let white = [0.95047, 1.0, 1.08883];
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let [fx, fy, fz]: V3 = std::array::from_fn(|c| f(xyz[c].max(0.0) / white[c]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn chroma(v: V3) -> f32 {
    let l = lab(v);
    l[1].hypot(l[2])
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Candidate C as the paper kernel lays it: the mean log exposure under the pick moved by `d` above the knee `k`.
fn knee(x: f32, d: f32, k: f32) -> f32 {
    oni_render::kneed(x, d, k)
}

struct Frame {
    s: PipelineSettings,
    p: Paper,
    grey: V3,
    lp: f32,
    pick_logs: V3,
}

impl Frame {
    fn new(s: PipelineSettings) -> Frame {
        let k = conversion::stage47(&s);
        let p = k.paper.expect("a paper");
        let [pick, _, grey] = paper::anchors(&s, k.dyes.as_ref());
        let untoned = Paper { points_black: [0.0; 3], points_white: [0.0; 3], zones: [0.0; 2], mids: [0.0; 3], ..p };
        Frame { lp: mean(untoned.exposed(pick)), pick_logs: untoned.exposed(pick), s, p, grey }
    }

    /// The neutral of scene stops `st` from the grey, a negative of `GAMMA`.
    fn neutral(&self, st: f32) -> V3 {
        self.grey.map(|g| g + GAMMA * LOG2 * st)
    }

    fn x(&self, a: V3) -> f32 {
        self.lp - mean(self.p.exposed(self.p.toned(a)))
    }

    fn rest(&self, a: V3) -> V3 {
        self.p.shown(a)
    }

    fn exposure(&self, b: f32, a: V3) -> V3 {
        let mut s = self.s.clone();
        s.print.exposure += b;
        conversion::stage47(&s).paper.unwrap().shown(a)
    }

    /// Each channel of the print at rest read on u from the pick to the white, moved by `f`, laid back on its ends.
    fn on_u(&self, a: V3, f: impl Fn(f32) -> f32) -> V3 {
        let p = &self.p;
        let raw = p.coloured(p.printed(a));
        std::array::from_fn(|c| {
            let (k, an, w) = (p.pick[c], p.anchor[c], p.white[c]);
            let v = raw[c];
            if v <= k {
                return v * an / k;
            }
            an + (w - an) * f((v - k) / (w - k))
        })
    }

    fn odds(&self, g: f32, a: V3) -> V3 {
        self.on_u(a, |u| {
            let t = u.min(1.0);
            g * t / (1.0 + (g - 1.0) * t) + (u - t)
        })
    }

    /// A: a gain on the light above a knee `uk` of the range: u = τ + m, m a C1 ramp, τ + g·m.
    fn gain_above(&self, g: f32, uk: f32, a: V3) -> V3 {
        self.on_u(a, |u| {
            let m = if u <= 0.0 { 0.0 } else if u < uk { u * u / (2.0 * uk) } else { u - uk / 2.0 };
            u - m + g * m
        })
    }

    /// B: the odds gain faded to none under `uk` by a smoothstep on u.
    fn odds_faded(&self, g: f32, uk: f32, a: V3) -> V3 {
        self.on_u(a, |u| {
            let t = u.min(1.0);
            let o = g * t / (1.0 + (g - 1.0) * t) + (u - t);
            u + smooth(u / uk) * (o - u)
        })
    }

    /// C: Exposure's shift of the log exposure, faded to none over `xk` above the pick.
    fn shifted(&self, b: f32, xk: f32, a: V3) -> V3 {
        let p = &self.p;
        let log = p.exposed(p.toned(a));
        let x = self.lp - mean(log);
        let moved = knee(x, b * LOG2, xk) - x;
        let light = p.viewed(p.developed(p.graded(log.map(|v| v - moved))));
        let light = if p.luma { [luma(light); 3] } else { light };
        Paper { brightness: 0.0, ..*p }.ended(p.coloured(light))
    }

    /// C per channel, each layer its own knee: where the neutral's knee tone lies under the pick in that layer.
    fn shifted_knees(&self, b: f32, ks: V3, a: V3) -> V3 {
        let p = &self.p;
        let log = p.exposed(p.toned(a));
        let own = self.pick_logs;
        let moved: V3 = std::array::from_fn(|c| own[c] - knee(own[c] - log[c], b * LOG2, ks[c]));
        let light = p.viewed(p.developed(p.graded(moved)));
        let light = if p.luma { [luma(light); 3] } else { light };
        Paper { brightness: 0.0, ..*p }.ended(p.coloured(light))
    }

    /// C per channel: each layer's own distance under the pick's own, the same knee on each.
    fn shifted_own(&self, b: f32, xk: f32, a: V3) -> V3 {
        let p = &self.p;
        let log = p.exposed(p.toned(a));
        let own = self.pick_logs;
        let moved: V3 = std::array::from_fn(|c| own[c] - knee(own[c] - log[c], b * LOG2, xk));
        let light = p.viewed(p.developed(p.graded(moved)));
        let light = if p.luma { [luma(light); 3] } else { light };
        Paper { brightness: 0.0, ..*p }.ended(p.coloured(light))
    }
}

/// The 18 chromatic ColorChecker patches in linear Rec. 2020.
fn chart() -> Vec<V3> {
    let code: [[f32; 3]; 18] = [
        [115.0, 82.0, 68.0], [194.0, 150.0, 130.0], [98.0, 122.0, 157.0], [87.0, 108.0, 67.0], [133.0, 128.0, 177.0], [103.0, 189.0, 170.0],
        [214.0, 126.0, 44.0], [80.0, 91.0, 166.0], [193.0, 90.0, 99.0], [94.0, 60.0, 108.0], [157.0, 188.0, 64.0], [224.0, 163.0, 46.0],
        [56.0, 61.0, 150.0], [70.0, 148.0, 73.0], [175.0, 54.0, 60.0], [231.0, 199.0, 31.0], [187.0, 86.0, 149.0], [8.0, 133.0, 161.0],
    ];
    let m = [[0.6274, 0.3293, 0.0433], [0.0691, 0.9195, 0.0114], [0.0164, 0.0880, 0.8956]];
    let decode = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    code.iter().map(|c| {
        let l = c.map(decode);
        std::array::from_fn(|r| m[r][0] * l[0] + m[r][1] * l[1] + m[r][2] * l[2])
    }).collect()
}

/// The gain whose shape `f` prints the grey at the L* `target`, by bisection on its log.
fn calibrated(target: f32, f: impl Fn(f32) -> f32) -> f32 {
    let (mut lo, mut hi) = (-12.0f32, 12.0f32);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if f(mid.exp2()) < target { lo = mid } else { hi = mid }
    }
    (0.5 * (lo + hi)).exp2()
}

fn ppm(out: &Path, name: &str, l: &Linear) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.ppm"))).unwrap());
    write!(f, "P6\n{} {}\n255\n", l.width, l.height).unwrap();
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    for p in &l.pixels {
        let s: [u8; 3] = std::array::from_fn(|r| encode(m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]));
        f.write_all(&s).unwrap();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = Path::new(&args[0]);
    let knee_l: f32 = args[1].parse().unwrap();
    let renders = std::env::var("RENDERS").is_ok();
    std::fs::create_dir_all(out).unwrap();
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    for (index, raw) in args[2..].iter().map(Path::new).enumerate() {
        let name = raw.file_stem().unwrap().to_string_lossy().to_string();
        let mut read = sidecar::read(raw, true).expect("a sidecar").settings;
        read.print_paper = read.print_paper.or(Some(Default::default()));
        (read.print.brightness, read.print.true_blacks) = (0.0, false);
        let f = Frame::new(read.clone());
        let pick_l = lab(f.rest(paper::anchors(&f.s, conversion::stage47(&f.s).dyes.as_ref())[0]))[0];
        println!("{name}: paper {:?}, pick L* {pick_l:.2}, grey amounts {:.3?}, the pick's mean log exposure {:.3}", f.s.print_paper, f.grey, f.lp);
        // The neutral ramp: log exposure under the pick against L* at rest.
        let mut row = String::new();
        for i in 0..=24 {
            let a = f.grey.map(|g| g * i as f32 / 12.0);
            row += &format!(" x{:.3}:L{:.1}", f.x(a), lab(f.rest(a))[0]);
        }
        println!("  ramp (amounts 0 to twice the grey):{row}");
        // Where the frame's tones lie at rest.
        let negative = e.prepare(render::decode(raw, Reading::of(&read)).expect("a decode")).unwrap();
        let side = Some(1200);
        let rest = e.render(&negative, &read, side).unwrap();
        let (ya, yw) = (luma(f.p.anchor), luma(f.p.white));
        let on_black = |p: &[f32; 4]| lab([((luma([p[0], p[1], p[2]]) - ya) / (yw - ya) * yw).max(0.0); 3])[0];
        let mut ls: Vec<f32> = rest.pixels.iter().map(on_black).collect();
        ls.sort_by(f32::total_cmp);
        let at = |q: f32| ls[((ls.len() - 1) as f32 * q) as usize];
        println!(
            "  L* at rest, its pick on black: 0.5% {:.1}, 1% {:.1}, 2% {:.1}, 5% {:.1}, 10% {:.1}, 25% {:.1}, 50% {:.1}, 90% {:.1}, 99% {:.1}",
            at(0.005), at(0.01), at(0.02), at(0.05), at(0.1), at(0.25), at(0.5), at(0.9), at(0.99)
        );
        let above = |d: f32| ls.iter().filter(|l| **l <= d).count() as f32 / ls.len() as f32 * 100.0;
        println!("  share of pixels under L* 5 / 10 / 15 / 20 / 30 on black: {:.1}% / {:.1}% / {:.1}% / {:.1}% / {:.1}%", above(5.0), above(10.0), above(15.0), above(20.0), above(30.0));
        // The candidates against Exposure on the ramp and the chart.
        let stops = [-3.0f32, -2.0, -1.0, 0.0, 2.0];
        let chart_amounts: Vec<V3> = chart().iter().map(|c| std::array::from_fn(|ch| f.grey[ch] + GAMMA * (c[ch] / 0.18).log10())).collect();
        let grey_l = |print: &dyn Fn(V3) -> V3| lab(print(f.neutral(0.0)))[0];
        // The knee: the neutral whose print, its pick on black and its white the paper's, reaches L* `knee_l`.
        let y_white = luma(f.p.white);
        let uk = (((knee_l + 16.0) / 116.0).powi(3)) / y_white;
        let share = |t: f32| (luma(f.p.coloured(f.p.printed(f.grey.map(|g| g * t)))) - luma(f.p.pick)) / (y_white - luma(f.p.pick));
        let (mut lo, mut hi) = (0.0f32, 8.0f32);
        for _ in 0..50 {
            let m = 0.5 * (lo + hi);
            if share(m) < uk { lo = m } else { hi = m }
        }
        let xk = f.x(f.grey.map(|g| g * lo));
        let knee_logs = f.p.exposed(f.p.toned(f.grey.map(|g| g * lo)));
        let ks: V3 = std::array::from_fn(|c| f.pick_logs[c] - knee_logs[c]);
        println!("  per-layer knees {ks:.3?}");
        for b in [-4.0f32, -1.0, -0.5, 0.5, 1.0, 4.0] {
            let drift = |print: &dyn Fn(V3) -> V3| (0..=300).map(|i| {
                let a = f.grey.map(|g| g * 1.5 * i as f32 / 300.0);
                (chroma(print(a)) - chroma(f.rest(a))).abs()
            }).fold(0.0f32, f32::max);
            let mut bright = f.s.clone();
            bright.print.exposure += 2.0;
            let e2 = Frame::new(bright);
            let span = e2.p.span;
            let fall = |print: &dyn Fn(V3) -> V3| {
                let tones: Vec<V3> = (0..=6000).map(|i| print(span.map(|v| v * (-0.5 + 3.0 * i as f32 / 6000.0)))).collect();
                tones.windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0f32, f32::max)
            };
            let e2_logs = e2.p.exposed(e2.p.toned(f.grey.map(|g| g * lo)));
            let e2_ks: V3 = std::array::from_fn(|c| e2.pick_logs[c] - e2_logs[c]);
            let e2_xk = e2.x(f.grey.map(|g| g * lo));
            println!(
                "  Brightness {b:+}: neutral C* drift: mean {:.2}, own knees {:.2}, Exposure {:.2}; span-ramp fall at Exposure +2: mean {:.1e}, own knees {:.1e}",
                drift(&|a| f.shifted(b, xk, a)), drift(&|a| f.shifted_knees(b, ks, a)), drift(&|a| f.exposure(b, a)),
                fall(&|a| e2.shifted(b, e2_xk, a)), fall(&|a| e2.shifted_knees(b, e2_ks, a)),
            );
        }
        let stops_under = (lo - 1.0) * mean(f.grey) / (GAMMA * LOG2);
        println!("  knee: L* {knee_l} on black, u {uk:.4} of the range, log exposure {xk:.3} under the pick, {stops_under:.2} stops from the grey");
        let report = |label: &str, print: &dyn Fn(V3) -> V3| {
            let l: Vec<String> = stops.iter().map(|st| format!("{:5.1}", lab(print(f.neutral(*st)))[0])).collect();
            let hue: Vec<f32> = stops.iter().map(|st| chroma(print(f.neutral(*st)))).collect();
            let c = chart_amounts.iter().map(|a| chroma(print(*a))).sum::<f32>() / chart_amounts.len() as f32;
            let lstar: Vec<f32> = chart_amounts.iter().map(|a| lab(print(*a))[0]).collect();
            let low = lstar.iter().copied().fold(f32::INFINITY, f32::min);
            println!("    {label:<26} L* at -3/-2/-1/0/+2 stops {}  neutral C* max {:.2}  chart C* {c:.1} (darkest patch L* {low:.1})", l.join(" "), hue.iter().copied().fold(0.0, f32::max));
        };
        report("rest", &|a| f.rest(a));
        for b in [-1.0f32, -0.5, 0.5, 1.0] {
            println!("  Brightness {b:+}");
            let target = grey_l(&|a| f.exposure(b, a));
            report("Exposure (target)", &|a| f.exposure(b, a));
            let g = (2.0 * b).exp2();
            report("today's odds gain", &|a| f.odds(g, a));
            let ga = calibrated(target, |g| grey_l(&|a| f.gain_above(g, uk, a)));
            report(&format!("A gain above knee g{ga:.2}"), &|a| f.gain_above(ga, uk, a));
            let gb = calibrated(target, |g| grey_l(&|a| f.odds_faded(g, uk, a)));
            report(&format!("B odds faded g{gb:.2}"), &|a| f.odds_faded(gb, uk, a));
            report("C log H shift, knee", &|a| f.shifted(b, xk, a));
            report("C per channel", &|a| f.shifted_own(b, xk, a));
            // Monotony of each shape over the ramp from past the pick to past the white.
            let fall = |print: &dyn Fn(V3) -> V3| {
                let tones: Vec<V3> = (0..=3000).map(|i| print(f.grey.map(|g| g * (-0.5 + 4.5 * i as f32 / 3000.0)))).collect();
                tones.windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0f32, f32::max)
            };
            println!("    largest fall: A {:.1e}, B {:.1e}, C {:.1e}", fall(&|a| f.gain_above(ga, uk, a)), fall(&|a| f.odds_faded(gb, uk, a)), fall(&|a| f.shifted(b, xk, a)));
        }
        // The toe: each tone of the ramp by its L* on black at rest, moved by Brightness ±1 under knees of L* 10 to 25.
        let on_black_of = |v: V3| lab([((luma(v) - luma(f.p.anchor)) / (y_white - luma(f.p.anchor)) * y_white).max(0.0); 3])[0];
        let tone_at = |target: f32| {
            let (mut lo, mut hi) = (0.0f32, 8.0f32);
            for _ in 0..50 {
                let m = 0.5 * (lo + hi);
                if on_black_of(f.rest(f.grey.map(|g| g * m))) < target { lo = m } else { hi = m }
            }
            f.grey.map(|g| g * lo)
        };
        let marks = [1.0f32, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0, 50.0];
        for kl in [3.0f32, 5.0, 8.0] {
            let uk = (((kl + 16.0) / 116.0).powi(3)) / y_white;
            let (mut lo, mut hi) = (0.0f32, 8.0f32);
            for _ in 0..50 {
                let m = 0.5 * (lo + hi);
                if share(m) < uk { lo = m } else { hi = m }
            }
            let xk = f.x(f.grey.map(|g| g * lo));
            for b in [-1.0f32, 1.0] {
                let row: Vec<String> = marks.iter().map(|m| {
                    let a = tone_at(*m);
                    format!("{m}:{:+.1}/{:+.1}", on_black_of(f.shifted(b, xk, a)) - m, on_black_of(f.exposure(b, a)) - m)
                }).collect();
                println!("  toe, knee L* {kl} (log {xk:.3}), Brightness {b:+}: L* on black at rest: knee's move / Exposure's: {}", row.join(" "));
            }
        }
        let grey_logs = f.p.exposed(f.grey);
        println!("  the grey's log exposure under the pick per layer {:.3?}", std::array::from_fn::<f32, 3, _>(|c| f.pick_logs[c] - grey_logs[c]));
        for b in [-4.0f32, -2.0, 2.0, 4.0] {
            let fall = |print: &dyn Fn(V3) -> V3| {
                let tones: Vec<V3> = (0..=6000).map(|i| print(f.grey.map(|g| g * (-0.5 + 4.5 * i as f32 / 6000.0)))).collect();
                tones.windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0f32, f32::max)
            };
            let tone = |print: &dyn Fn(V3) -> V3| stops.map(|st| lab(print(f.neutral(st)))[0]);
            let toe = |print: &dyn Fn(V3) -> V3| (1..=8).map(|i| chroma(print(f.grey.map(|g| g * i as f32 / 16.0)))).fold(0.0f32, f32::max);
            println!(
                "  Brightness {b:+}: C L* {:.1?} fall {:.1e} toe C* {:.2}; C per channel L* {:.1?} fall {:.1e} toe C* {:.2}; Exposure L* {:.1?} toe C* {:.2}; rest toe C* {:.2}",
                tone(&|a| f.shifted(b, xk, a)), fall(&|a| f.shifted(b, xk, a)), toe(&|a| f.shifted(b, xk, a)),
                tone(&|a| f.shifted_own(b, xk, a)), fall(&|a| f.shifted_own(b, xk, a)), toe(&|a| f.shifted_own(b, xk, a)),
                tone(&|a| f.exposure(b, a)), toe(&|a| f.exposure(b, a)), toe(&|a| f.rest(a)),
            );
        }
        if renders && index < 2 {
            for (label, change) in [("brightness-1", (-1.0f32, 0.0f32)), ("brightness+0", (0.0, 0.0)), ("brightness+1", (1.0, 0.0)), ("exposure-1", (0.0, -1.0)), ("exposure+1", (0.0, 1.0))] {
                let mut s = read.clone();
                s.print.brightness += change.0;
                s.print.exposure += change.1;
                ppm(out, &format!("{name}-{label}"), &e.render(&negative, &s, side).unwrap());
            }
        }
    }
}
