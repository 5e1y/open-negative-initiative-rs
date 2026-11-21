//! Lighting's colour on the paper: Temperature and Tint tone ONI's print curve as on ONI's print, the black and the white
//! held, the enlarger at its rest; Hunt follows Exposure and the zones, never Contrast; ONI's print to the bit. Each verdict
//! has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, noise};
use frames::*;
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::{conversion, film};
use oni_render::{Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{PipelineSettings, VirtualStock};

/// Temperature and Tint at half and full travel either way, as the filters they write.
fn warmths() -> Vec<(String, [f32; 3])> {
    let mut out = Vec::new();
    for (name, axis, range) in [("Temperature", Print::TEMPERATURE_AXIS, Print::TEMPERATURE_RANGE.1), ("Tint", Print::TINT_AXIS, Print::TINT_RANGE.1)] {
        for share in [-1.0, -0.5, 0.5, 1.0] {
            out.push((format!("{name} {share:+}"), axis.map(|a| a * share * range)));
        }
    }
    out
}

/// How far each end prints off: the white's furthest channel from white, the black's from black.
fn ends_off([black, white, _]: [V4; 3]) -> (f32, f32) {
    ((0..3).map(|c| (white[c] - 1.0).abs()).fold(0.0, f32::max), (0..3).map(|c| black[c].abs()).fold(0.0, f32::max))
}

/// The enlarger's former move: the filters as 30.1 CC a stop on its rest filtration, the least at none, the light holding
/// the windowed grey's luma; ONI's print curve left unfiltered.
fn filtration_moved(s: &PipelineSettings, k: &Stage47) -> (Stage47, [f32; 3]) {
    let rest = conversion::printing(s).unwrap();
    let cc: [f32; 3] = std::array::from_fn(|c| rest.filtration[c] + 100.0 * std::f32::consts::LOG10_2 * s.print.filters()[c]);
    let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
    let cc = cc.map(|v| v - least);
    let p = Paper { filters: cc.map(|v| 10f32.powf(-v / 100.0)), ..k.paper.unwrap() };
    let grey = paper::anchors(s, k.dyes.as_ref())[2];
    let luma = |v: [f32; 3]| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    let (mut low, mut high) = (p.scale.ln() - 10.0, p.scale.ln() + 10.0);
    for _ in 0..80 {
        let mid = 0.5 * (low + high);
        let q = Paper { scale: mid.exp(), ..p };
        if luma(q.windowed(q.printed(grey))) > *GREY_PRINT { low = mid } else { high = mid }
    }
    (Stage47 { paper: Some(Paper { scale: (0.5 * (low + high)).exp(), ..p }), own_stops: [0.0; 3], ..*k }, cc)
}

#[test]
fn temperature_and_tint_tone_the_print_as_on_onis_and_the_ends_stay_black_and_white() {
    let s = prepared_negative();
    let onis = PipelineSettings { print_paper: None, ..s.clone() };
    let rest = lit(&s, |_| {});
    let (rest_white, rest_black) = ends_off(rest);
    let luma = |v: V4| 0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2];
    for (label, f) in warmths() {
        let toned = |t: &mut PipelineSettings| t.print.set_filters(f);
        let on_paper = lit(&s, toned);
        let (white, black) = ends_off(on_paper);
        let (oni_white, oni_black) = ends_off(lit(&onis, toned));
        println!("{label}: the whites within {white:.1e} of white, the blacks within {black:.1e} (ONI's print {oni_white:.1e}, {oni_black:.1e}; at rest {rest_white:.1e}, {rest_black:.1e})");
        // Measured: every end exact on the paper, within 8e-5 on ONI's print (3.9e-5 at its rest): warmth moves neither.
        assert!(white <= oni_white.max(rest_white) + 1e-6 && black <= oni_black.max(rest_black) + 1e-6, "{label}: {on_paper:?}");
        let (grey, was) = (on_paper[2], rest[2]);
        println!("{label}: the grey {grey:?} from {was:?}");
        assert!((luma(grey) - luma(was)).abs() < 1e-5, "{label}: its luma held, {} {}", luma(grey), luma(was));
        // Toned the filters' way: warmer red against blue, a magenta tint green below red and blue.
        let axes = |v: V4| ((v[0] / v[2]).ln(), ((v[0] * v[2]).sqrt() / v[1]).ln());
        let ((warmth, tint), (was_warmth, was_tint)) = (axes(grey), axes(was));
        let (t, n) = (f[0] - f[2], (f[0] + f[2]) / 2.0 - f[1]);
        let (dw, dn) = (warmth - was_warmth, tint - was_tint);
        // The axis left at rest moves a tenth of the other at most (measured 0.07 on Temperature's full travel).
        let towards = |delta: f32, axis: f32, other: f32| if axis == 0.0 { delta.abs() < 0.1 * other.abs() } else { delta / axis > 0.05 };
        assert!(towards(dw, t, dn) && towards(dn, n, dw), "{label}: {grey:?}");
        let k = conversion::stage47(&{ let mut t = s.clone(); toned(&mut t); t });
        let (former, _) = filtration_moved(&{ let mut t = s.clone(); toned(&mut t); t }, &k);
        let (white, black) = ends_off(anchor_scans(&s).map(|q| cpu::stage47(q, &former)));
        println!("{label}: on the enlarger, as before, the whites within {white:.4} of white, the blacks within {black:.4}");
        // Twin: the enlarger's filters move the ends, 0.034 and 0.0075 at half the Temperature's travel, 0.12 at its end.
        assert!(white > 1e-2 || black > 2e-3, "{label}: {white} {black}");
    }
    // Warmer: red brightens, blue darkens; a magenta tint takes the green down.
    let [_, _, warm] = lit(&s, |t| t.print.set_filters(Print::TEMPERATURE_AXIS));
    assert!(warm[0] > rest[2][0] * 1.05 && warm[2] < rest[2][2] * 0.95, "{warm:?}");
    let [_, _, magenta] = lit(&s, |t| t.print.set_filters(Print::TINT_AXIS));
    assert!(magenta[1] < rest[2][1] * 0.97, "{magenta:?}");
}

#[test]
fn the_enlarger_stays_at_its_rest_filtration_whatever_temperature_and_tint_say() {
    let s = prepared_negative();
    let rest = conversion::printing(&s).unwrap();
    for (label, f) in warmths() {
        let mut t = s.clone();
        t.print.set_filters(f);
        let now = conversion::printing(&t).unwrap();
        assert_eq!((now.filtration, now.paper.filters, now.paper.scale), (rest.filtration, rest.paper.filters, rest.paper.scale), "{label}");
        let k = conversion::stage47(&t);
        assert_eq!(k.own_stops, f, "{label}: the filters on ONI's print curve");
        // Twin: the enlarger's former move dials the filtration by 30.1 CC a stop.
        let (_, cc) = filtration_moved(&t, &k);
        let apart = (0..3).map(|c| (cc[c] - rest.filtration[c]).abs()).fold(0.0, f32::max);
        println!("{label}: rest {:?}, the former move {cc:?}", rest.filtration);
        assert!(apart > 5.0, "{label}: {cc:?}");
    }
}

/// The scan pixel of the frame's grey leaning red against blue, so its print carries a colour density.
fn tinted(s: &PipelineSettings) -> V4 {
    let g = grey_scan(s);
    [g[0] * 1.15, g[1], g[2] * 0.87, 1.0]
}

/// The log chroma `k` prints `q` at, red against blue: the colour density scales it.
fn spread(q: V4, k: &Stage47) -> f32 {
    let v = cpu::stage47(q, k);
    (v[0] / v[2]).ln()
}

/// The log of the colour density Hunt lays at `t`: its print's log chroma against the same print with Hunt off.
fn hunted(q: V4, k: &Stage47) -> f32 {
    (spread(q, k) / spread(q, &Stage47 { hunt: 0.0, ..*k })).ln()
}

#[test]
fn exposure_and_the_zones_lower_the_colour_density_on_the_paper_as_on_onis_print() {
    let s = prepared_negative();
    let onis = PipelineSettings { print_paper: None, ..s.clone() };
    let q = tinted(&s);
    let at = |t: &PipelineSettings, f: &dyn Fn(&mut PipelineSettings)| {
        let mut u = t.clone();
        f(&mut u);
        conversion::stage47(&u)
    };
    let up = |u: &mut PipelineSettings| u.print.exposure += 1.0;
    let rest = hunted(q, &conversion::stage47(&s));
    let paper = hunted(q, &at(&s, &up)) - rest;
    let oni = hunted(q, &at(&onis, &up)) - hunted(q, &conversion::stage47(&onis));
    println!("Exposure +1: the colour density's log {paper:+.4} on the paper from {rest:+.1e} at rest, {oni:+.4} on ONI's print");
    let as_onis = |v: f32| v < -0.05 && (v - oni).abs() < 0.1 * oni.abs();
    assert!(as_onis(paper), "{paper} {oni}");
    // Twin: Hunt off on the paper leaves the colour density where it rests.
    let unhunted = hunted(q, &Stage47 { hunt: 0.0, ..at(&s, &up) }) - rest;
    assert!(!as_onis(unhunted), "{unhunted}");
    for (label, zones) in [("Shadows", [0.2, 0.0]), ("Highlights", [0.0, 0.2])] {
        let v = hunted(q, &at(&s, &|u| (u.print.shadows, u.print.highlights) = (zones[0], zones[1])));
        let o = hunted(q, &at(&onis, &|u| (u.print.shadows, u.print.highlights) = (zones[0], zones[1]))) - hunted(q, &conversion::stage47(&onis));
        println!("{label} +0.2: {v:+.4} on the paper, {o:+.4} on ONI's print");
        assert!(v < -1e-3 && o < -1e-3, "{label}: {v} {o}");
    }
    // Black and white carries no Hunt, on the paper as on ONI's print.
    assert_eq!(conversion::stage47(&mono()).hunt, 0.0);
}

#[test]
fn contrast_leaves_the_colour_density_on_the_paper() {
    let s = prepared_negative();
    let q = tinted(&s);
    for travel in [-0.3, 0.3] {
        let mut t = s.clone();
        t.print.contrast = travel;
        let k = conversion::stage47(&t);
        let v = hunted(q, &k);
        // Twin: Hunt reading the contrast, as on ONI's print.
        let reading = hunted(q, &Stage47 { hunt_contrast: 1.0, ..k });
        println!("Contrast {travel:+}: the colour density's log {v:+.1e}, read by Hunt {reading:+.4}");
        assert!(v.abs() < 1e-5, "{travel}: {v}");
        assert!(reading.abs() > 1e-2, "{travel}: {reading}");
    }
}

/// ONI's print on every setting the paper's checks move, Contrast at rest, its outputs' bits folded by FNV-1a.
fn onis_print_bits(twin: impl Fn(Stage47) -> Stage47) -> u64 {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let mut pixels: Vec<V4> = noise(23, 17, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(noise(23, 17, 6, 0.0, 1.0));
    let off = |s: PipelineSettings| PipelineSettings { print_paper: None, ..s };
    let mut lit = off(prepared_negative());
    (lit.print.exposure, lit.print.shadows, lit.print.highlights) = (2.0, 0.1, -0.1);
    lit.print.set_filters([0.2, -0.1, 0.0]);
    (lit.print.black_point, lit.print.white_point, lit.print.channel_contrast, lit.print.channel_mid) = (0.03, 0.05, [0.1, 0.0, -0.1], [0.0, 0.1, 0.0]);
    lit.density = 0.3;
    let mut settings = vec![off(prepared_negative()), off(digital()), off(mono()), lit];
    for stops in [-1.0, 1.0] {
        let mut t = off(prepared_negative());
        t.print.exposure += stops;
        settings.push(t);
    }
    let mut warmer = off(prepared_negative());
    warmer.print.set_filters(Print::TEMPERATURE_AXIS);
    settings.push(warmer);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for s in &settings {
        let k = twin(conversion::stage47(s));
        assert!(k.paper.is_none());
        for p in &pixels {
            for v in cpu::stage47(*p, &k) {
                for b in v.to_bits().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    }
    hash
}

#[test]
fn onis_print_is_unchanged_to_the_bit() {
    // ONI's print's outputs on these settings, Contrast at rest, whose colour the kernel never touches.
    const BEFORE: u64 = 0x3374_6d46_e618_f2e6;
    let bits = onis_print_bits(|k| k);
    println!("ONI's print folds to {bits:#018x}");
    assert_eq!(bits, BEFORE);
    // Twin: Contrast a notch off its rest moves it.
    assert_ne!(onis_print_bits(|k| Stage47 { print_contrast: k.print_contrast + 0.05, ..k }), BEFORE);
}
