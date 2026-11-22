//! Lighting's colour compensation switched off on ONI's print: no Hunt, no points' or Contrast's saturation and the
//! points' colour unheld, the luma as with it, the roll's factor and the film's share kept, the GPU beside its
//! reference; on, the print as before to the bit; on a paper nothing to cut. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{TOLERANCE, V4, error, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::{Graph, Source};
use oni_pipeline::{Pipeline, Request, conversion, film, finishing, stages};
use oni_render::{Stage47, cpu};
use oni_settings::axis::pipeline::luma;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::frame::RollReference;
use oni_settings::tone::Print;
use oni_settings::{Id, PipelineSettings, VirtualStock};

/// The prepared negative with its windows' mids at rest.
fn negative() -> PipelineSettings {
    let mut s = prepared_negative();
    for w in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue] {
        w.mid = 0.5;
    }
    s
}

/// The negative on a measured roll whose ends are roll #8's, so the colour factor departs from 1.
fn on_roll() -> PipelineSettings {
    let mut s = negative();
    let (black, white) = ([0.362, 0.348, 0.319], [0.716, 0.754, 0.850]);
    s.roll_reference = Some(RollReference { id: Id::new(), stops: s.gains.stops, black, white, base: None, median: None });
    s
}

/// Each frame on ONI's print; a paper lays no compensation for the switch to cut.
fn prints() -> Vec<(String, PipelineSettings)> {
    let frames = [("on its roll", on_roll()), ("off a roll", negative()), ("digital", digital())];
    frames.into_iter().map(|(frame, s)| (format!("{frame}, ONI's print"), PipelineSettings { print_paper: None, ..s })).collect()
}

/// Lighting's moves the checks make.
fn moves() -> Vec<(&'static str, fn(&mut Print))> {
    vec![
        ("Exposure +1", |p| p.exposure += 1.0),
        ("Exposure -1", |p| p.exposure -= 1.0),
        ("Highlights +0.5", |p| p.highlights = 0.5),
        ("Highlights -0.5", |p| p.highlights = -0.5),
        ("Shadows +0.5", |p| p.shadows = 0.5),
        ("Shadows -0.5", |p| p.shadows = -0.5),
        ("Contrast +0.3", |p| p.contrast = 0.3),
        ("Contrast -0.3", |p| p.contrast = -0.3),
        ("Black point +0.05", |p| p.black_point += 0.05),
        ("White point +0.05", |p| p.white_point += 0.05),
    ]
}

fn with(s: &PipelineSettings, on: bool, f: fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    t.print.colour_compensation = on;
    f(&mut t.print);
    t
}

/// The colour `shown` lays on a pixel, each curve twice its value at the pixel's tone.
fn laid(p: V4, shown: &[SaturationCurve]) -> V4 {
    let y = luma([p[0], p[1], p[2]]);
    let x = f64::from(y.max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0));
    let f = shown.iter().map(|c| (2.0 * Curve { points: c.points.clone() }.evaluated().value(x)).max(0.0)).product::<f64>() as f32;
    [y + f * (p[0] - y), y + f * (p[1] - y), y + f * (p[2] - y), p[3]]
}

/// Stage 4-7's reference, then the print's own saturation and the clip: the render where every other stage rests.
fn render(s: &PipelineSettings, q: V4) -> V4 {
    cpu::output_clip(laid(cpu::stage47(q, &conversion::stage47(s)), &finishing::print_saturation(s)))
}

/// Tinted scans across the frame's windows, each with its neutral of the same densities.
fn pairs(s: &PipelineSettings) -> Vec<(V4, V4)> {
    let w = s.levels.windows();
    let mut out = vec![];
    for i in 0..9 {
        let t = 0.1 + 0.8 * i as f32 / 8.0;
        let base: [f32; 3] = std::array::from_fn(|c| w[c].black + t * (w[c].white - w[c].black));
        for tint in [[0.06, 0.0, -0.06], [-0.05, 0.05, 0.0], [0.0, -0.04, 0.06], [0.08, -0.03, -0.04]] {
            out.push((scan_at(s, std::array::from_fn(|c| base[c] + tint[c])), scan_at(s, base)));
        }
    }
    out
}

/// A patch's colour around its neutral: the spread of their channels' log ratio, where the eye reads saturation.
fn colour(s: &PipelineSettings, (patch, neutral): (V4, V4)) -> f32 {
    let (p, n) = (render(s, patch), render(s, neutral));
    let d: [f32; 3] = std::array::from_fn(|c| (p[c].max(1e-6) / n[c].max(1e-6)).ln());
    let mean = (d[0] + d[1] + d[2]) / 3.0;
    d.iter().map(|x| (x - mean).powi(2)).sum::<f32>().sqrt()
}

/// The patches printed between 2 % and 90 % of light, clear of the clip and of the black.
fn readable(s: &PipelineSettings, q: &(V4, V4)) -> bool {
    let y = luma(render(s, q.1)[..3].try_into().unwrap());
    (0.02..0.9).contains(&y)
}

/// The median over the readable patches of their colour under `lit` over their colour at `rest`.
fn colour_ratio(rest: &PipelineSettings, lit: &PipelineSettings) -> f32 {
    let mut r: Vec<f32> = pairs(rest).into_iter().filter(|q| readable(rest, q) && readable(lit, q)).map(|q| colour(lit, q) / colour(rest, q)).collect();
    r.sort_by(f32::total_cmp);
    r[r.len() / 2]
}

#[test]
fn without_compensation_the_print_lays_no_colour_in_answer_to_lighting() {
    for (print, s) in prints() {
        for (label, f) in moves() {
            let (on, off) = (with(&s, true, f), with(&s, false, f));
            let (k_on, k_off) = (conversion::stage47(&on), conversion::stage47(&off));
            assert_eq!(k_off.hunt, 0.0, "{print}, {label}: no Hunt");
            assert_eq!(finishing::print_saturation(&off), [SaturationCurve::default(), SaturationCurve::default()], "{print}, {label}");
            // Twin: on, Hunt reads the move and Contrast and the points lay their curves.
            assert_eq!(k_on.hunt, 1.0, "{print}, {label}");
            let curves = finishing::print_saturation(&on).iter().filter(|c| !c.is_neutral()).count();
            assert_eq!(curves > 0, label.starts_with("Contrast") || label.contains("point"), "{print}, {label}: {curves}");
        }
    }
}

#[test]
fn without_compensation_the_luma_moves_as_with_it_and_the_colour_as_the_curves_give_it() {
    let mut worst = 0.0f32;
    for (print, s) in prints() {
        let (rest_on, rest_off) = (with(&s, true, |_| {}), with(&s, false, |_| {}));
        for (label, f) in moves() {
            let (on, off) = (with(&s, true, f), with(&s, false, f));
            // A neutral's luma moves as with the compensation, which lays colour about it; a patch's within the colour
            // density's power mean, which Hunt lays on its luma.
            let y = |t: &PipelineSettings, p: V4| luma(render(t, p)[..3].try_into().unwrap());
            let apart = |p: V4| (y(&off, p) - y(&on, p)).abs() / y(&on, p).max(0.02);
            let (mut grey, mut moved, mut tinted) = (0.0f32, 0.0f32, Vec::new());
            for (patch, neutral) in pairs(&s) {
                grey = grey.max(apart(neutral));
                moved = moved.max((y(&on, neutral) - y(&rest_on, neutral)).abs() / y(&rest_on, neutral).max(0.02));
                tinted.push(apart(patch));
            }
            tinted.sort_by(f32::total_cmp);
            let (typical, gap) = (tinted[tinted.len() / 2], tinted[tinted.len() - 1]);
            worst = worst.max(grey);
            let (c_on, c_off) = (colour_ratio(&rest_on, &on), colour_ratio(&rest_off, &off));
            println!("{print}, {label}: a neutral's luma {grey:.1e} apart, a patch's {typical:.1e} (worst {gap:.1e}), the move {moved:.2}; colour x{c_on:.3} on, x{c_off:.3} off");
            assert!(grey < 1e-3, "{print}, {label}: a neutral's luma parts by {grey}");
            assert!(typical < 1e-2, "{print}, {label}: a patch's luma parts by {typical}");
            // Twin: the move itself moves the luma far past that bound, so the measure sees a move.
            assert!(moved > 0.05, "{print}, {label}: {moved}");
            // What the compensation laid is gone: Hunt's lower density as the print brightens, Contrast's and the
            // points' hold, each the colour its own way from the compensation's.
            let parted = match label {
                "Exposure +1" | "Highlights +0.5" | "Shadows +0.5" => c_off - c_on,
                "Exposure -1" | "Highlights -0.5" | "Shadows -0.5" => c_on - c_off,
                "Contrast +0.3" => c_off - c_on,
                "Contrast -0.3" => c_on - c_off,
                _ => (c_off - c_on).abs(),
            };
            let least = if label.contains("point") { 0.004 } else { 0.01 };
            assert!(parted > least, "{print}, {label}: the colour x{c_on} on, x{c_off} off");
        }
    }
    println!("a neutral's luma with the compensation off at most {worst:.1e} from it on");
}

#[test]
fn without_compensation_the_rolls_factor_and_the_films_share_still_act() {
    for (print, s) in prints() {
        let rest = with(&s, false, |_| {});
        let k = conversion::stage47(&rest);
        let at_rest = (conversion::colour_factor(&with(&s, true, |_| {})), k.decompression, k.film_share);
        // The points' share alone is cut: under Black point +0.05 the weights keep their rest's colour factor, the
        // film's share stretched by the points' reach as the windows' colour is.
        let pointed = conversion::stage47(&with(&s, false, |p| p.black_point += 0.05));
        if s.roll_reference.is_some() {
            assert!((at_rest.0 - 1.0).abs() > 1e-3 && at_rest.1 == at_rest.0 && at_rest.2 == 0.0, "{print}: {at_rest:?}");
            assert_eq!((pointed.decompression, pointed.film_share), (at_rest.1, 0.0), "{print}");
        } else {
            assert!(at_rest.1 > 0.0 && at_rest.2 > 0.0, "{print}: {at_rest:?}");
            assert_eq!(pointed.decompression, at_rest.1, "{print}");
            assert!((pointed.film_share - at_rest.2 / 0.95).abs() < 1e-6, "{print}: {}", pointed.film_share);
        }
        // Twin: cutting the whole decompression moves the frame at rest.
        let cut = Stage47 { decompression: 1.0, film_share: 0.0, ..k };
        let scans: Vec<V4> = pairs(&s).into_iter().map(|q| q.0).collect();
        let e = error(&mapped(&scans, |p| cpu::stage47(p, &k)), &mapped(&scans, |p| cpu::stage47(p, &cut)));
        println!("{print}: weights at rest {at_rest:?}; cutting them moves the frame {e:.2e}");
        assert!(e > 1e-3, "{print}: {e}");
    }
}

#[test]
fn without_compensation_the_gpu_is_its_reference() {
    let Some(mut g) = graph("without_compensation_the_gpu_is_its_reference") else { return };
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (w, h) = (31, 19);
    let mut pixels: Vec<V4> = noise(w, h, 13, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 4] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    for (print, s) in prints() {
        let lit = |on: bool| {
            with(&s, on, |p| {
                (p.exposure, p.contrast, p.shadows, p.highlights) = (p.exposure + 1.0, 0.3, 0.2, -0.3);
                (p.black_point, p.white_point, p.channel_white) = (0.05, 0.05, [0.04, 0.0, 0.0]);
            })
        };
        let (k, twin) = (conversion::stage47(&lit(false)), conversion::stage47(&lit(true)));
        let out = whole(&mut g, &stages::converted(oni_graph::Image::new(&source), &k));
        let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
        let twin = mapped(&pixels, |p| cpu::stage47(p, &twin));
        judge(&format!("compensation off, {print}"), &out, &reference, &twin);
        assert!(error(&out, &twin) > 4e-2, "{print}: the compensation's twin is {:.2e} away", error(&out, &twin));
    }
}

/// Flat patches, so the spatial stages leave their centres as they are.
const PATCH: u32 = 9;

/// The whole render's patch centres for `scan`, every other stage at rest.
fn centres(g: &mut Graph, pipeline: &mut Pipeline, source: &Source, count: u32, s: &PipelineSettings) -> Vec<V4> {
    let request = Request { full_width: Some(f64::from(count * PATCH)), ..Request::default() };
    let image = pipeline.image(g, source, s, &request, &Default::default()).unwrap();
    let out = whole(g, &image);
    (0..count).map(|i| out[((PATCH / 2) * count * PATCH + i * PATCH + PATCH / 2) as usize]).collect()
}

#[test]
fn without_compensation_the_whole_render_lays_no_saturation_curve() {
    let Some(mut g) = graph("without_compensation_the_whole_render_lays_no_saturation_curve") else { return };
    let mut pipeline = Pipeline::new();
    // A digital photo's scene reaches stage 4-7 through its virtual negative, which this reference does not lay.
    for (print, s) in prints().into_iter().filter(|(p, _)| !p.starts_with("digital")) {
        let scans: Vec<V4> = pairs(&s).into_iter().map(|q| q.0).collect();
        let count = scans.len() as u32;
        let pixels: Vec<V4> = (0..count * PATCH * PATCH).map(|i| scans[((i % (count * PATCH)) / PATCH) as usize]).collect();
        let source = g.upload(count * PATCH, PATCH, &pixels).unwrap();
        let settle = |on: bool| {
            let mut t = with(&s, on, |p| (p.contrast, p.black_point, p.white_point) = (0.3, p.black_point + 0.05, 0.05));
            (t.sharpen.amount, t.chroma.force, t.texture.amount, t.secret_sauce) = (0.0, 0.0, 0.0, 0.0);
            t
        };
        let (off, on) = (settle(false), settle(true));
        let rendered = centres(&mut g, &mut pipeline, &source, count, &off);
        let bare = |t: &PipelineSettings| mapped(&scans, |p| cpu::output_clip(cpu::stage47(p, &conversion::stage47(t))));
        let e = error(&rendered, &bare(&off));
        // Twin: the curves the compensation would lay on the same print.
        let twin = error(&rendered, &mapped(&scans, |p| cpu::output_clip(laid(cpu::stage47(p, &conversion::stage47(&off)), &finishing::print_saturation(&on)))));
        println!("{print}: the render {e:.2e} off the print with no curve; {twin:.2e} off the curves laid");
        assert!(e < TOLERANCE, "{print}: {e}");
        assert!(twin > 1e-2, "{print}: {twin}");
    }
}

/// The CPU render of every print under every move, its outputs' bits folded by FNV-1a.
fn bits(on: Option<bool>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (_, s) in prints() {
        for f in std::iter::once((|_: &mut Print| {}) as fn(&mut Print)).chain(moves().into_iter().map(|(_, f)| f)) {
            let t = match on {
                Some(on) => with(&s, on, f),
                None => {
                    let mut t = s.clone();
                    f(&mut t.print);
                    t
                }
            };
            for v in pairs(&s).into_iter().flat_map(|(p, n)| [render(&t, p), render(&t, n)]).flatten() {
                for b in v.to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    }
    hash
}

#[test]
fn with_compensation_the_print_is_unchanged_to_the_bit() {
    // The fold of these renders on ONI's print before the switch existed, as the paper's density change left it, on the
    // data-sheet dyes.
    const BEFORE: u64 = 0x711b_f17a_c732_db9a;
    let (stored, on) = (bits(None), bits(Some(true)));
    println!("the renders fold to {stored:#018x}");
    assert_eq!((stored, on), (BEFORE, BEFORE));
    // Twin: off, the fold moves.
    assert_ne!(bits(Some(false)), BEFORE);
}

#[test]
fn on_a_paper_the_switch_has_nothing_to_cut() {
    for (frame, s) in [("on its roll", on_roll()), ("off a roll", negative()), ("digital", digital())] {
        for (label, f) in moves() {
            let (on, off) = (with(&s, true, f), with(&s, false, f));
            let printed = |t: &PipelineSettings| pairs(&s).into_iter().map(|(p, n)| [render(t, p), render(t, n)]).collect::<Vec<_>>();
            assert_eq!(printed(&on), printed(&off), "{frame}, {label}");
            assert_eq!(finishing::print_saturation(&on), finishing::print_saturation(&off), "{frame}, {label}");
            // Twin: on ONI's print the switch cuts Hunt.
            let onis = |t: &PipelineSettings| conversion::stage47(&PipelineSettings { print_paper: None, ..t.clone() }).hunt;
            assert_ne!(onis(&on), onis(&off), "{frame}, {label}");
        }
    }
}
