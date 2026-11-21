//! Lighting's Black point and White point on the paper: the colour they stretch held as on ONI's print, on a measured roll
//! and on none, the ends black and white; ONI's print to the bit; the GPU beside its reference. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::{conversion, film, paper, stages};
use oni_render::{Stage47, cpu};
use oni_settings::frame::RollReference;
use oni_settings::tone::Print;
use oni_settings::{Id, PipelineSettings, VirtualStock};

/// The prepared negative with its windows' mids at rest, as roll #8's frames have them.
fn negative() -> PipelineSettings {
    let mut s = prepared_negative();
    for w in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue] {
        w.mid = 0.5;
    }
    s
}

/// The negative on a measured roll: its reference's ends as roll #8's, so the colour factor reads each reach.
fn on_roll() -> PipelineSettings {
    let mut s = negative();
    let (black, white) = ([0.362, 0.348, 0.319], [0.716, 0.754, 0.850]);
    s.roll_reference = Some(RollReference { id: Id::new(), stops: s.gains.stops, black, white, base: None, median: None });
    s
}

/// The frames, a measured roll and none: the negative on its roll, the same off a roll and a digital photo.
fn frames() -> [(&'static str, PipelineSettings); 3] {
    [("on its roll", on_roll()), ("off a roll", negative()), ("digital", digital())]
}

/// Lighting's points the checks move: the pair, each alone, one channel's own white and another's own black.
fn moves() -> Vec<(&'static str, fn(&mut Print))> {
    vec![
        ("Black point +0.05", |p| p.black_point += 0.05),
        ("White point +0.05", |p| p.white_point += 0.05),
        ("both +0.05", |p| (p.black_point, p.white_point) = (p.black_point + 0.05, p.white_point + 0.05)),
        ("both +0.15", |p| (p.black_point, p.white_point) = (p.black_point + 0.15, p.white_point + 0.15)),
        ("red's white +0.05", |p| p.channel_white[0] += 0.05),
        ("blue's black +0.05", |p| p.channel_black[2] += 0.05),
    ]
}

/// The scan densities of the frame's neutral that `k` prints at luma `y`.
fn neutral_at(s: &PipelineSettings, k: &Stage47, y: f32) -> [f32; 3] {
    let grey = paper::reference_grey(s);
    let luma = |t: f32| {
        let v = cpu::stage47(scan_at(s, grey.map(|d| d + t)), k);
        0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]
    };
    let (mut low, mut high) = (-1.5f32, 1.5f32);
    let rising = luma(high) > luma(low);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if (luma(mid) < y) == rising { low = mid } else { high = mid }
    }
    grey.map(|d| d + 0.5 * (low + high))
}

/// Coloured patches and their neutrals as scan pixels: three tints on the neutrals `k` prints at four lumas, so the
/// paper and ONI's print are read at the same printed tones.
fn patches(s: &PipelineSettings, k: &Stage47) -> Vec<(V4, V4)> {
    let mut out = Vec::new();
    for y in [0.08, 0.15, 0.3, 0.5, 0.7] {
        let neutral = neutral_at(s, k, y);
        for tint in [[0.06, 0.0, -0.06], [-0.05, 0.05, 0.0], [0.0, -0.04, 0.06]] {
            out.push((scan_at(s, std::array::from_fn(|c| neutral[c] + tint[c])), scan_at(s, neutral)));
        }
    }
    out
}

/// Stage 4-7 stopped at its windows, where the points act: no print curve, no colour density.
fn window_stage(k: Stage47) -> Stage47 {
    Stage47 { print_on: 0.0, density: 1.0, ..k }
}

/// The patch's colour against its neutral through `k`: the spread of their channels' difference about its mean.
fn chroma(k: &Stage47, (patch, neutral): (V4, V4)) -> f32 {
    let (p, n) = (cpu::stage47(patch, k), cpu::stage47(neutral, k));
    let d: [f32; 3] = std::array::from_fn(|c| p[c] - n[c]);
    let mean = (d[0] + d[1] + d[2]) / 3.0;
    d.iter().map(|x| (x - mean).powi(2)).sum::<f32>().sqrt()
}

/// The same spread on the print's log light, where the eye reads saturation.
fn log_chroma(k: &Stage47, (patch, neutral): (V4, V4)) -> f32 {
    let (p, n) = (cpu::stage47(patch, k), cpu::stage47(neutral, k));
    let d: [f32; 3] = std::array::from_fn(|c| (p[c].max(1e-6) / n[c].max(1e-6)).ln());
    let mean = (d[0] + d[1] + d[2]) / 3.0;
    d.iter().map(|x| (x - mean).powi(2)).sum::<f32>().sqrt()
}

fn moved(s: &PipelineSettings, f: fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

/// Each patch's colour under the move over its colour at rest, read by `measure` through `k`'s variant `twin`.
fn ratios(s: &PipelineSettings, f: fn(&mut Print), twin: impl Fn(Stage47) -> Stage47, measure: fn(&Stage47, (V4, V4)) -> f32) -> Vec<f32> {
    let rest = conversion::stage47(s);
    let (at_rest, lit) = (twin(rest), twin(conversion::stage47(&moved(s, f))));
    patches(s, &rest).into_iter().map(|q| measure(&lit, q) / measure(&at_rest, q)).collect()
}

/// Which patches ONI's moved windows lay on their black: their colour there is clipped, not held.
fn clipped(s: &PipelineSettings, f: fn(&mut Print)) -> Vec<bool> {
    let onis = PipelineSettings { print_paper: None, ..s.clone() };
    let lit = window_stage(conversion::stage47(&moved(&onis, f)));
    let low = |q: V4| cpu::stage47(q, &lit)[..3].iter().any(|v| *v < 0.05);
    patches(&onis, &conversion::stage47(&onis)).into_iter().map(|(p, n)| low(p) || low(n)).collect()
}

/// The paper's points as plain windows: no colour held.
fn unheld(k: Stage47) -> Stage47 {
    Stage47 { decompression: 1.0, film_share: 0.0, ..k }
}

fn mean(a: &[f32]) -> f32 {
    a.iter().sum::<f32>() / a.len() as f32
}

#[test]
fn the_points_stretch_the_colour_on_the_paper_by_onis_ratio() {
    let (mut worst, mut twin_least) = (0.0f32, f32::INFINITY);
    for (frame, s) in frames() {
        let onis = PipelineSettings { print_paper: None, ..s.clone() };
        for (label, f) in moves() {
            let at = |t: &PipelineSettings, twin: fn(Stage47) -> Stage47| ratios(t, f, move |k| window_stage(twin(k)), chroma);
            let (oni, paper, plain) = (at(&onis, |k| k), at(&s, |k| k), at(&s, unheld));
            let kept = clipped(&s, f);
            let gap = |a: &[f32]| a.iter().zip(&oni).zip(&kept).filter(|(_, c)| !**c).map(|((x, o), _)| (x - o).abs()).fold(0.0, f32::max);
            let k = conversion::stage47(&moved(&s, f));
            let o = conversion::stage47(&moved(&onis, f));
            let seen = |t: &PipelineSettings, twin: fn(Stage47) -> Stage47| mean(&ratios(t, f, twin, log_chroma));
            println!(
                "{frame}, {label}: at the window x{:.4} on ONI's print ({:.4}, {:.4}), x{:.4} on the paper ({:.4}, {:.4}), x{:.4} unheld; {}",
                format!("worst gap {:.4}, unheld {:.4}; printed, by log chroma, x{:.4}, x{:.4}, x{:.4}", gap(&paper), gap(&plain), seen(&onis, |k| k), seen(&s, |k| k), seen(&s, unheld)),
                mean(&oni), o.decompression, o.film_share, mean(&paper), k.decompression, k.film_share, mean(&plain)
            );
            // Measured: within 0.014 on every patch ONI's window leaves off its black, a global move within 0.01.
            assert!(gap(&paper) < 0.02, "{frame}, {label}: {paper:?} against {oni:?}");
            worst = worst.max(gap(&paper));
            if !label.contains("'s") {
                // Twin: the plain windows, no colour held, part from ONI's ratio on every global move.
                assert!(gap(&plain) > 0.025, "{frame}, {label}: unheld {plain:?} against {oni:?}");
                twin_least = twin_least.min(gap(&plain));
            }
        }
    }
    println!("the paper within {worst:.4} of ONI's ratio, the unheld paper {twin_least:.4} off at least");
}

#[test]
fn the_ends_stay_black_and_white_under_the_points() {
    for (frame, s) in frames() {
        for (label, f) in moves() {
            let k = conversion::stage47(&moved(&s, f));
            let ends = |k: &Stage47| anchor_scans(&s).map(|q| cpu::stage47(q, k));
            let apart = |a: [V4; 3], b: [V4; 3]| (0..2).flat_map(|i| (0..3).map(move |c| (a[i][c] - b[i][c]).abs())).fold(0.0, f32::max);
            let ([black, white, _], plain) = (ends(&k), ends(&unheld(k)));
            println!("{frame}, {label}: black {black:?}, white {white:?}, {:.1e} from the plain windows'", apart(ends(&k), plain));
            // The hold moves neither end off the plain windows': black on black, white at or past white, a global move neutral.
            assert!(apart(ends(&k), plain) < 1e-5, "{frame}, {label}");
            assert!((0..3).all(|c| black[c].abs() < 1e-6 && white[c] >= 1.0 - 1e-5), "{frame}, {label}");
            if !label.contains("'s") {
                assert!((0..3).all(|c| (white[c] - white[0]).abs() < 1e-5), "{frame}, {label}: {white:?}");
            }
            // Twin: the colour held against ONI's density windows in place of the paper's own tints both ends.
            let onis = conversion::stage47(&PipelineSettings { print_paper: None, ..moved(&s, f) });
            let wrong = Stage47 { balance_low: onis.balance_low, balance_span: onis.balance_span, ..k };
            assert!(apart(ends(&wrong), plain) > 1e-3, "{frame}, {label}: {:?}", ends(&wrong));
        }
    }
}

#[test]
fn the_gpu_holds_the_papers_colour_as_its_reference() {
    let Some(mut g) = graph("the_gpu_holds_the_papers_colour_as_its_reference") else { return };
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 11, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 4] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    for (frame, s) in [("lit negative on its roll", on_roll()), ("lit negative off a roll", prepared_negative())] {
        let mut lit = s;
        (lit.print.exposure, lit.print.contrast, lit.print.shadows) = (lit.print.exposure + 0.5, 0.2, 0.1);
        (lit.print.black_point, lit.print.white_point, lit.print.channel_white, lit.print.channel_black) = (0.05, 0.05, [0.04, 0.0, 0.0], [0.0, 0.0, 0.03]);
        let k = conversion::stage47(&lit);
        assert!(k.paper.is_some() && (k.decompression, k.film_share) != (1.0, 0.0), "{frame}: the paper holds its colour");
        let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        judge(&format!("paper's points, {frame}"), &out, &reference, &mapped(&pixels, |p| cpu::stage47(p, &unheld(k))));
    }
}

/// ONI's print of every frame under every move, its outputs' bits folded by FNV-1a.
fn onis_print_bits(twin: impl Fn(Stage47) -> Stage47) -> u64 {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let mut pixels: Vec<V4> = noise(17, 13, 7, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(noise(17, 13, 8, 0.0, 1.0));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (_, s) in frames() {
        let onis = PipelineSettings { print_paper: None, ..s };
        for f in std::iter::once((|_: &mut Print| {}) as fn(&mut Print)).chain(moves().into_iter().map(|(_, f)| f)) {
            let k = twin(conversion::stage47(&moved(&onis, f)));
            assert!(k.paper.is_none());
            for v in pixels.iter().flat_map(|p| cpu::stage47(*p, &k)) {
                for b in v.to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    }
    hash
}

#[test]
fn onis_print_under_the_points_is_unchanged_to_the_bit() {
    // ONI's print's outputs on these frames and moves before the paper held its colour.
    const BEFORE: u64 = 0x9a8b_d567_adf2_dadb;
    let bits = onis_print_bits(|k| k);
    println!("ONI's print folds to {bits:#018x}");
    assert_eq!(bits, BEFORE);
    // Twin: ONI's decompression a twentieth lower moves it.
    assert_ne!(onis_print_bits(|k| Stage47 { decompression: k.decompression * 0.95, ..k }), BEFORE);
}
