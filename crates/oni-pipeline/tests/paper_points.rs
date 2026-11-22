//! Lighting's tones on the paper, in density on the negative before the enlarger: Black point and White point move the
//! density read as bare film and as the densest, Shadows and Highlights lay density by tone, each channel's mids and
//! contrast its own; ONI's print under the points to the bit. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, noise};
use frames::*;
use oni_pipeline::paper::GRADE_BASE;
use oni_pipeline::{conversion, film};
use oni_render::{Paper, Stage47, cpu};
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

/// The negative on a measured roll: its reference's ends as roll #8's.
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

fn moved(s: &PipelineSettings, f: impl Fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper.expect("a paper")
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs() / (1e-3 + b[c].abs())).fold(0.0, f32::max)
}

#[test]
fn the_points_move_the_density_read_as_bare_film_and_as_the_densest() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let span = rest.span;
        let (base, densest) = (rest.printed([0.0; 3]), rest.printed(span));
        for (label, f) in moves() {
            let p = paper_of(&moved(&s, f));
            let (black, white) = (p.points_black, p.points_white);
            // Each channel's density at its black point prints as the bare film printed at rest, its white point's as the densest.
            let at_black: [f32; 3] = std::array::from_fn(|c| black[c] * span[c]);
            let at_white: [f32; 3] = std::array::from_fn(|c| (1.0 - white[c]) * span[c]);
            let (b, w) = (apart(p.printed(at_black), base), apart(p.printed(at_white), densest));
            // The paper's light is read where it is exposed: the densest prints near the paper's white, its log exposure apart.
            let exposure = |q: &Paper, a: [f32; 3]| q.exposed(q.toned(a));
            let gap = |x: [f32; 3], y: [f32; 3]| (0..3).map(|c| (x[c] - y[c]).abs()).fold(0.0f32, f32::max);
            let (eb, ew) = (gap(exposure(&p, at_black), rest.exposed([0.0; 3])), gap(exposure(&p, at_white), rest.exposed(span)));
            println!("{frame}, {label}: points {black:?} {white:?}; their densities print within {b:.1e} and {w:.1e} of the rest's ends, exposed within {eb:.1e} and {ew:.1e}");
            assert!(b < 1e-5 && w < 1e-5 && eb < 1e-5 && ew < 1e-5, "{frame}, {label}");
            // Twin: the rest's paper exposes the same densities elsewhere.
            assert!(gap(rest.exposed(at_black), rest.exposed([0.0; 3])).max(gap(rest.exposed(at_white), rest.exposed(span))) > 1e-2, "{frame}, {label}");
        }
        // A global pair moves the three channels alike; a channel's own its own alone.
        let red = paper_of(&moved(&s, |p| p.channel_white[0] += 0.05));
        assert!(red.points_white[0] > 0.04 && red.points_white[1] == 0.0 && red.points_white[2] == 0.0, "{frame}");
    }
}

#[test]
fn shadows_and_highlights_lay_density_by_tone_the_ends_held() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let along = |p: &Paper, u: f32| p.printed(rest.span.map(|a| a * u));
        let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
        for (label, zones) in [("Shadows +0.2", [0.2, 0.0]), ("Highlights +0.2", [0.0, 0.2]), ("Shadows -0.25", [-0.25, 0.0]), ("Highlights -0.4", [0.0, -0.4])] {
            let p = paper_of(&moved(&s, |q| (q.shadows, q.highlights) = (zones[0], zones[1])));
            let ends = apart(along(&p, 0.0), along(&rest, 0.0)).max(apart(along(&p, 1.0), along(&rest, 1.0)));
            // The density laid, as a share of the range, at a quarter and at three quarters of it.
            let laid = |u: f32| p.toned(rest.span.map(|a| a * u))[1] / rest.span[1] - u;
            let (low, high) = (laid(0.25), laid(0.75));
            let printed = |u: f32| luma(along(&p, u)) / luma(along(&rest, u));
            println!("{frame}, {label}: ends within {ends:.1e}; density laid {low:+.4} at a quarter, {high:+.4} at three quarters; the print x{:.3}, x{:.3}", printed(0.25), printed(0.75));
            assert!(ends < 1e-6, "{frame}, {label}");
            let (near, far, at) = if zones[0] != 0.0 { (low, high, 0.25) } else { (high, low, 0.75) };
            let sense = zones[0] + zones[1];
            assert!(near * sense > 0.0 && near.abs() > 2.0 * far.abs() && (printed(at) - 1.0) * sense > 0.0, "{frame}, {label}: {near} {far}");
            // One density by the mean tone: a coloured amount moves along the negative's range, each dye by the same share.
            let u = [0.2f32, 0.3, 0.5];
            let toned = p.toned(std::array::from_fn(|c| rest.span[c] * u[c]));
            let shares: Vec<f32> = (0..3).map(|c| toned[c] / rest.span[c] - u[c]).collect();
            assert!(shares.iter().all(|v| (v - shares[0]).abs() < 1e-6 && v.abs() > 1e-3), "{frame}, {label}: {shares:?}");
            // Twin: each channel toned by its own tone moves the three apart, a colour laid by the zones.
            let own: Vec<f32> = (0..3).map(|c| p.toned(rest.span.map(|a| a * u[c]))[c] / rest.span[c] - u[c]).collect();
            assert!((own[0] - own[2]).abs() > 1e-3, "{frame}, {label}: {own:?}");
        }
    }
}

#[test]
fn each_channels_mids_and_contrast_are_its_own_in_density() {
    for (frame, s) in frames() {
        let rest = paper_of(&s);
        let p = paper_of(&moved(&s, |q| q.channel_mid = [0.2, 0.0, 0.0]));
        let mid = rest.span.map(|a| a * 0.5);
        let (lit, was) = (p.printed(mid), rest.printed(mid));
        println!("{frame}, red mid +0.2: the middle {lit:?} from {was:?}");
        assert!(lit[0] > was[0] * 1.05 && apart(p.printed([0.0; 3]), rest.printed([0.0; 3])) < 1e-6 && apart(p.printed(rest.span), rest.printed(rest.span)) < 1e-6, "{frame}");
        // Twin: the same mid on green moves green, not red.
        let green = paper_of(&moved(&s, |q| q.channel_mid = [0.0, 0.2, 0.0])).toned(mid);
        assert!((green[0] - mid[0]).abs() < 1e-7 && green[1] > mid[1], "{frame}");
        let c = paper_of(&moved(&s, |q| (q.contrast, q.channel_contrast) = (0.1, [0.2, 0.0, -0.1])));
        let expected = [0.3f32, 0.1, 0.0].map(|t| GRADE_BASE.powf(t));
        println!("{frame}, contrast 0.1 and red +0.2, blue -0.1: slopes {:?}", c.slope);
        assert!((0..3).all(|i| (c.slope[i] - expected[i]).abs() < 1e-6), "{frame}");
        // Twin: Contrast alone grades the three layers alike.
        assert!(paper_of(&moved(&s, |q| q.contrast = 0.1)).slope.iter().all(|v| (v - GRADE_BASE.powf(0.1)).abs() < 1e-6));
    }
    // Black and white reads no channel's own.
    let m = paper_of(&moved(&mono(), |q| (q.channel_mid, q.channel_contrast) = ([0.2; 3], [0.2; 3])));
    assert!(m.mids == [0.0; 3] && m.slope == [1.0; 3]);
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
    // ONI's print's outputs on these frames and moves on the data-sheet dyes, which the paper's points in density leave alone.
    const BEFORE: u64 = 0x5966_a93e_0cdf_615f;
    let bits = onis_print_bits(|k| k);
    println!("ONI's print folds to {bits:#018x}");
    assert_eq!(bits, BEFORE);
    // Twin: ONI's decompression a twentieth lower moves it.
    assert_ne!(onis_print_bits(|k| Stage47 { decompression: k.decompression * 0.95, ..k }), BEFORE);
}
