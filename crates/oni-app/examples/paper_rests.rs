//! `paper_rests <raw>...`: each frame's enlarger at rest on the default paper, side by side: (a) the shipped rest, the time
//! and filtration that print the reference grey at a plain RAW's grey, neutral; (b) a printer's minimum time for maximum
//! black, the shortest time that prints the bare film at a share of every layer's density range (0.98, and 0.95 and 0.90
//! to weigh the threshold), filtered so the grey prints neutral. For each, the filtration, the time against (a)'s, and the
//! print of the base (black), the densest read (white) and the grey: light, luma and each layer's density against Dmax.

use oni_pipeline::{conversion, paper};
use oni_render::{PAPER_CURVE_NODES, Paper};
use oni_settings::tone::Print;
use oni_settings::{PipelineSettings, sidecar};
use std::path::Path;

type V3 = [f32; 3];

fn luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

fn filters(cc: V3) -> V3 {
    cc.map(|v| 10f32.powf(-v / paper::CC_PER_DENSITY))
}

/// (b): Newton on the log time, magenta and yellow so the bare film's least reached layer prints at `share` of its range
/// and the grey neutral; black and white by the time alone.
fn base_rest(p: &Paper, grey: V3, mono: bool, share: f32, from: V3) -> V3 {
    let at = |x: V3| Paper { scale: x[0].exp(), filters: filters([0.0, x[1], x[2]]), ..*p };
    let base = |x: V3| paper::layer_shares(&at(x), [0.0; 3]).into_iter().fold(f32::INFINITY, f32::min) - share;
    let miss = |x: V3| {
        let light = at(x).printed(grey).map(|v| v.max(1e-12).ln());
        [base(x), light[1] - light[0], light[2] - light[0]]
    };
    let mut x = from;
    for _ in 0..80 {
        let r = miss(x);
        if mono {
            let slope = (base([x[0] + 0.01, 0.0, 0.0]) - base(x)) / 0.01;
            x[0] -= (base(x) / slope).clamp(-1.0, 1.0);
            continue;
        }
        let probe = [0.01, 0.5, 0.5];
        let col: [V3; 3] = std::array::from_fn(|j| {
            let mut m = x;
            m[j] += probe[j];
            let v = miss(m);
            std::array::from_fn(|c| (v[c] - r[c]) / probe[j])
        });
        let j: [[f64; 3]; 3] = std::array::from_fn(|c| std::array::from_fn(|k| f64::from(col[k][c])));
        let step = solve(j, r.map(f64::from));
        x = std::array::from_fn(|k| x[k] - (step[k] as f32).clamp(-[1.0, 20.0, 20.0][k], [1.0, 20.0, 20.0][k]));
    }
    x
}

fn solve(a: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    std::array::from_fn(|k| {
        let mut m = a;
        (0..3).for_each(|r| m[r][k] = b[r]);
        det(m) / d
    })
}

fn report(tag: &str, p: &Paper, x: V3, anchors: &[V3; 3], rest_a: f32) {
    let at = Paper { scale: x[0].exp(), filters: filters([0.0, x[1], x[2]]), ..*p };
    let top = at.curves[PAPER_CURVE_NODES - 1];
    let dmax_light = at.viewed(top);
    let white_light = at.viewed(at.curves[0]);
    println!(
        "  {tag}: filtration C 0 M {:.2} Y {:.2}, time {:+.3} stops against (a); paper Dmax light {:.4} {:.4} {:.4} (luma {:.4}), white {:.4} {:.4} {:.4} (luma {:.4})",
        x[1], x[2], (x[0] - rest_a) / std::f32::consts::LN_2, dmax_light[0], dmax_light[1], dmax_light[2], luma(dmax_light), white_light[0], white_light[1], white_light[2], luma(white_light)
    );
    for (name, a) in [("black (base)", anchors[0]), ("white (densest read)", anchors[1]), ("grey", anchors[2])] {
        let light = at.printed(a);
        let density = at.developed(at.exposed(a));
        let shares = paper::layer_shares(&at, a);
        println!(
            "    {name:<20} light {:.4} {:.4} {:.4} luma {:.4} | layers D {:.3} {:.3} {:.3} of Dmax {:.3} {:.3} {:.3} (shares {:.3} {:.3} {:.3})",
            light[0], light[1], light[2], luma(light), density[0], density[1], density[2], top[0], top[1], top[2], shares[0], shares[1], shares[2]
        );
    }
}

fn main() {
    for raw in std::env::args().skip(1) {
        let raw = Path::new(&raw);
        let mut s: PipelineSettings = sidecar::read(raw, true).expect("a sidecar").settings;
        s.print_paper = Some(Default::default());
        s.print = Print { exposure: Print::resting_exposure(s.mode), ..Print::default() };
        let k = conversion::stage47(&s);
        let p = conversion::printing(&s).expect("a paper").paper;
        let bare = Paper { scale: 1.0, filters: [1.0; 3], ..p };
        let mono = s.mode.is_monochrome();
        let separation = oni_pipeline::conversion::stock_amounts(&s);
        let anchors = paper::anchors(&s, separation.as_ref());
        let (time, pack) = paper::resting(&bare, anchors[2], mono);
        let a = [time, pack[1], pack[2]];
        println!("{} ({:?}, dyes {:?}, separation {}, unmixed {})", raw.file_name().unwrap().to_string_lossy(), s.mode, s.dye_set, s.separation, k.dyes.is_some());
        println!("  anchors (dye amounts above the base): white {:.3} {:.3} {:.3}, grey {:.3} {:.3} {:.3}", anchors[1][0], anchors[1][1], anchors[1][2], anchors[2][0], anchors[2][1], anchors[2][2]);
        report("(a) grey at a plain RAW's grey", &bare, a, &anchors, a[0]);
        for share in [0.90, 0.95, 0.98] {
            let b = base_rest(&bare, anchors[2], mono, share, [a[0] + 1.0, a[1], a[2]]);
            report(&format!("(b) base at {share} of Dmax"), &bare, b, &anchors, a[0]);
        }
    }
}
