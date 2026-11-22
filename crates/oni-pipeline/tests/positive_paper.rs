//! A slide on the reversal paper: offered it alone, its enlarger at rest where its reference grey prints a plain RAW's
//! grey, neutral, its densest read near the paper's black and its bare film near its white; a neutral scale neutral;
//! Lighting moving it as it moves a negative; ONI's own render of a slide unmoved. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::V4;
use frames::*;
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::conversion;
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};

const RADIANCE: PaperStock = PaperStock::KodakEktachromeRadianceIII;

/// A slide on the reversal paper, Paper blacks on so it prints the paper's own black, where its enlarger rests.
fn slide() -> PipelineSettings {
    let mut s = PipelineSettings { print_paper: Some(RADIANCE), ..PipelineSettings::rest(ConversionMode::Positive, true) };
    s.print.paper_blacks = true;
    s
}

/// A slide its white pick balanced and a hand's windows narrowed.
fn prepared_slide() -> PipelineSettings {
    let mut s = slide();
    s.gains.stops = [0.35, 0.0, -0.45];
    let windows = [(1.05, 2.95, 0.6), (1.0, 3.0, 0.67), (0.95, 2.9, 0.72)];
    for (w, (black, white, mid)) in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue].into_iter().zip(windows) {
        (w.black, w.white, w.mid) = (black, white, mid);
    }
    s
}

fn slides() -> [(&'static str, PipelineSettings); 2] {
    [("slide at rest", slide()), ("prepared slide", prepared_slide())]
}

fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

fn luma(v: [f32; 3]) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

/// The scans of the slide's densest read, its bare film and its reference grey.
fn ends_scans(s: &PipelineSettings) -> [V4; 3] {
    let (bare, densest) = paper::film_ends(s);
    [scan_at(s, densest), scan_at(s, bare), grey_scan(s)]
}

fn printed(s: &PipelineSettings, q: V4) -> [f32; 3] {
    rgb(cpu::stage47(q, &conversion::stage47(s)))
}

fn moved(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t);
    t
}

#[test]
fn a_slide_prints_on_the_reversal_paper_alone_and_a_negative_never_does() {
    let k = conversion::stage47(&slide());
    assert!(k.paper.is_some_and(|p| p.reversal) && k.print_on == 0.0, "a slide on Radiance III");
    let on_negative_paper = PipelineSettings { print_paper: Some(PaperStock::default()), ..slide() };
    assert_eq!((on_negative_paper.paper(), on_negative_paper.fitting_paper()), (None, RADIANCE));
    // A slide on no paper, or on a negative one, renders on ONI's print as every slide did before, to the bit.
    let off = PipelineSettings { print_paper: None, ..slide() };
    assert_eq!(conversion::stage47(&on_negative_paper), conversion::stage47(&off));
    assert!(conversion::stage47(&off).paper.is_none() && conversion::stage47(&off).print_on == 1.0);
    let negative = PipelineSettings { print_paper: Some(RADIANCE), ..prepared_negative() };
    assert_eq!((negative.paper(), negative.fitting_paper()), (None, PaperStock::default()));
    let inverted = PipelineSettings { inverted_source: true, mode: ConversionMode::Negative, ..slide() };
    assert!(inverted.paper().is_none(), "a positive read as its own negative passes no enlarger");
    // Twin: the negative on its own paper prints, on a negative paper.
    assert!(conversion::stage47(&prepared_negative()).paper.is_some_and(|p| !p.reversal));
}

#[test]
fn the_slides_enlarger_rests_where_its_grey_prints_a_plain_raws_neutral_its_ends_the_papers() {
    for (label, s) in slides() {
        let k = conversion::stage47(&s);
        let p = k.paper.expect(label);
        let [densest, bare, grey] = ends_scans(&s).map(|q| rgb(cpu::stage47(q, &k)));
        let dmax = p.viewed(p.curves[0]);
        let filtration = conversion::printing(&s).unwrap().filtration;
        println!("{label}: filtration {filtration:?} CC, grey {grey:?} (a plain RAW's {:.5}), densest {densest:?} (Dmax {dmax:?}), bare {bare:?} (paper {:?})", *GREY_PRINT, p.white);
        assert!((0..3).all(|c| (grey[c] - *GREY_PRINT).abs() < 2e-6), "{label}: {grey:?}");
        // The densest read lands near the paper's Dmax, the bare film near its white; neither on 0 nor 1.
        assert!((0..3).all(|c| densest[c] >= dmax[c] - 1e-6 && densest[c] < 4.0 * dmax[c] && densest[c] > 1e-3), "{label}: {densest:?}");
        assert!((0..3).all(|c| bare[c] <= p.white[c] + 1e-6 && bare[c] > 0.6 * p.white[c] && bare[c] < 0.95), "{label}: {bare:?}");
        // Twin: the same enlarger through no filtration prints the grey off neutral.
        let unfiltered = rgb(cpu::stage47(grey_scan(&s), &Stage47 { paper: Some(Paper { filters: [1.0; 3], ..p }), ..k }));
        assert!((unfiltered[0] - unfiltered[2]).abs() > 1e-2, "{label}: {unfiltered:?}");
        // Twin: the ends not swapped, as a negative paper would print them: the bare film far the lighter.
        assert!(luma(bare) > 10.0 * luma(densest), "{label}");
    }
}

#[test]
fn a_slides_neutral_scale_prints_neutral_from_its_densest_to_its_bare_film() {
    for (label, s) in slides() {
        let (bare, densest) = paper::film_ends(&s);
        let k = conversion::stage47(&s);
        let mut worst = 0.0f32;
        let mut scale = Vec::new();
        // A neutral wedge on the slide: the frame's grey, which its windows balance, plus one density on every channel,
        // from its bare film's to its densest read's.
        let grey = paper::reference_grey(&s);
        let mean = |v: [f32; 3]| v.iter().sum::<f32>() / 3.0;
        let (from, to) = (mean(bare) - mean(grey), mean(densest) - mean(grey));
        for i in 0..=20 {
            let step = from + (to - from) * i as f32 / 20.0;
            let d = grey.map(|g| g + step);
            let v = rgb(cpu::stage47(scan_at(&s, d), &k));
            let cast = (v[0] / v[1]).ln().abs().max((v[2] / v[1]).ln().abs());
            worst = worst.max(cast);
            scale.push((step, luma(v), cast));
        }
        println!("{label}: a neutral scale's worst cast {worst:.4} (natural log of a channel over green); {scale:.4?}");
        // Kodak's own R, G and B curves part by up to 0.125 in this log between densities 0.15 and 2 (radiance_iii.csv).
        assert!(worst < 0.125, "{label}: {worst}");
        // The scale falls from the bare film to the densest, as a reversal print does.
        assert!(scale.windows(2).all(|w| w[1].1 < w[0].1), "{label}");
        // Twin: the rest's filtration taken away casts the same scale.
        let p = k.paper.unwrap();
        let bare_k = Stage47 { paper: Some(Paper { filters: [1.0; 3], ..p }), ..k };
        let mid = scan_at(&s, [0.5 * (bare[1] + densest[1]); 3]);
        let v = rgb(cpu::stage47(mid, &bare_k));
        assert!((v[0] / v[1]).ln().abs().max((v[2] / v[1]).ln().abs()) > 0.1, "{label}: {v:?}");
    }
}

#[test]
fn lighting_moves_a_slide_on_its_paper_as_it_moves_a_negative() {
    let s = prepared_slide();
    let [densest, _, grey] = ends_scans(&s);
    let rest = printed(&s, grey);
    // Exposure: a stop brighter is more light on a reversal paper.
    let over = moved(&s, |t| t.print.exposure += 1.0);
    let (k0, k1) = (conversion::stage47(&s).paper.unwrap(), conversion::stage47(&over).paper.unwrap());
    assert!(luma(printed(&over, grey)) > 1.3 * luma(rest) && (k1.scale / k0.scale - 2.0).abs() < 1e-4, "{:?}", printed(&over, grey));
    let under = moved(&s, |t| t.print.exposure -= 1.0);
    assert!(luma(printed(&under, grey)) < 0.8 * luma(rest));
    // A channel's filter: red brightened is a redder grey, the enlarger's cyan taken away.
    let red = moved(&s, |t| t.print.set_filters([0.5, 0.0, 0.0]));
    let (r, f0, f1) = (printed(&red, grey), conversion::printing(&s).unwrap().filtration, conversion::printing(&red).unwrap().filtration);
    println!("red: {r:?} from {rest:?}, filtration {f1:?} from {f0:?}");
    assert!(r[0] / r[1] > 1.1 * rest[0] / rest[1] && (luma(r) / luma(rest) - 1.0).abs() < 0.02, "{r:?}");
    // Contrast: the grade turns about the grey, the densest read darker.
    let hard = moved(&s, |t| t.print.contrast = 0.5);
    let near = scan_at(&s, paper::reference_grey(&s).map(|d| d + 0.3));
    let (g, dark) = (printed(&hard, grey), printed(&hard, near));
    assert!((luma(g) / luma(rest) - 1.0).abs() < 0.01 && luma(dark) < 0.85 * luma(printed(&s, near)), "{g:?} {dark:?}");
    // Shadows: the slide's dense tones lifted, its grey barely moved.
    let shadows = moved(&s, |t| t.print.shadows = 0.5);
    assert!(luma(printed(&shadows, near)) > 1.05 * luma(printed(&s, near)));
    // Paper blacks off: the densest read laid on pure black.
    let black = moved(&s, |t| t.print.paper_blacks = false);
    assert!(printed(&black, densest).iter().all(|v| v.abs() < 1e-5), "{:?}", printed(&black, densest));
    // Twin: a negative paper's sense would add cyan for red; here magenta and yellow rise against it.
    assert!(f1[0] == 0.0 && f1[1] - f0[1] > 10.0 && f1[2] - f0[2] > 10.0, "{f1:?} {f0:?}");
    assert!(luma(printed(&s, densest)) > 1e-4, "at rest the densest stays where the rest prints it");
}

#[test]
fn the_reversal_paper_is_kodaks_its_curves_falling_and_its_white_its_least_density() {
    let p = conversion::stage47(&slide()).paper.unwrap();
    let (top, bottom) = (p.curves[0], p.curves[PAPER_CURVE_NODES - 1]);
    println!("Radiance III: {top:?} unexposed, {bottom:?} fully exposed, {:?} over the base", p.print[0][3]);
    // Kodak's Dmax near 2.4 above the base, its Dmin within a few hundredths of it.
    assert!(top.iter().all(|d| (1.3..2.6).contains(d)) && bottom.iter().all(|d| (0.0..0.05).contains(d)), "{top:?} {bottom:?}");
    assert!((1..PAPER_CURVE_NODES).all(|i| (0..3).all(|c| p.curves[i][c] <= p.curves[i - 1][c] + 1e-6)), "falling with the light");
    let lit = |exposure: f32| luma(p.viewed(p.developed([exposure; 3])));
    assert!((luma(p.white) - lit(10.0)).abs() < 1e-6 && lit(10.0) > 50.0 * lit(-10.0));
    // Twin: a negative paper's white is its unexposed end.
    let negative = conversion::stage47(&prepared_negative()).paper.unwrap();
    assert!(negative.curves[0][1] < negative.curves[PAPER_CURVE_NODES - 1][1]);
}

/// Oklab's chroma of a linear Rec. 2020 light.
fn chroma(v: [f32; 3]) -> f32 {
    let m1 = [[0.616_645, 0.360_243, 0.023_013], [0.265_164, 0.635_856, 0.099_08], [0.100_1, 0.203_874, 0.695_253]];
    let lms: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| m1[r][c] * v[c]).sum::<f32>().max(0.0).cbrt());
    let a = 1.977_998_5 * lms[0] - 2.428_592_2 * lms[1] + 0.450_593_7 * lms[2];
    let b = 0.025_904_037 * lms[0] + 0.782_771_77 * lms[1] - 0.808_675_77 * lms[2];
    a.hypot(b)
}

#[test]
fn the_reversal_papers_primaries_hold_about_two_thirds_of_portra_enduras_chroma() {
    // Each paper's two-dye primaries at half its densest layers', the third dye absent: the gamut its dyes span.
    let primaries = |p: &Paper| {
        let top: [f32; 3] = std::array::from_fn(|c| 0.5 * p.curves[0][c].max(p.curves[PAPER_CURVE_NODES - 1][c]));
        [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]].map(|k: [f32; 3]| chroma(p.viewed(std::array::from_fn(|c| k[c] * top[c]))))
    };
    let radiance = primaries(&conversion::stage47(&slide()).paper.unwrap());
    let endura = primaries(&conversion::stage47(&prepared_negative()).paper.unwrap());
    println!("Oklab chroma of red, green and blue at half Dmax: Radiance III {radiance:.3?}, Portra Endura {endura:.3?}");
    // Kodak's R-3 dyes absorb far more outside their bands than spektrafilm's RA-4 ones: 0.6 to 0.7 of its chroma.
    assert!((0..3).all(|c| (0.5..0.8).contains(&(radiance[c] / endura[c]))), "{radiance:?} {endura:?}");
    // Twin: a neutral of every dye holds no chroma to speak of.
    let p = conversion::stage47(&slide()).paper.unwrap();
    assert!(chroma(p.viewed([0.8; 3])) < 0.5 * radiance.iter().copied().fold(f32::INFINITY, f32::min));
}

/// The slide's neutral from its pick, the densest film (0), through its reference grey (1), as dye amounts.
fn neutral(s: &PipelineSettings, t: f32) -> [f32; 3] {
    let [pick, _, grey] = paper::anchors(s, conversion::stage47(s).dyes.as_ref());
    std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]))
}

fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

#[test]
fn brightness_on_the_reversal_paper_holds_the_densest_film_and_moves_the_mids_as_exposure_does() {
    let mut followed = 0;
    for (label, s) in slides() {
        let rest = conversion::stage47(&s).paper.unwrap();
        for brightness in [-3.0f32, -1.0, -0.5, 0.5, 1.0, 3.0] {
            let p = conversion::stage47(&moved(&s, |u| u.print.brightness = brightness)).paper.unwrap();
            let at_rest = Paper { brightness: 0.0, ..p };
            let toward = |a: [f32; 3]| p.toward_light(p.pick_log, p.exposed(p.toned(a)));
            // The reach lies over the pick, where the neutral prints L* 25 above it on the print's own range, at rest.
            let (mut low, mut high) = (0.0f32, 8.0f32);
            for _ in 0..60 {
                let mid = 0.5 * (low + high);
                if toward(neutral(&s, mid)) < p.reach { low = mid } else { high = mid }
            }
            let (k, w) = (luma(p.pick), luma(p.white));
            let light = (luma(at_rest.coloured(at_rest.printed(neutral(&s, low)))) - k) / (w - k) * w;
            assert!(p.reach > 0.0 && (light / paper::BRIGHTNESS_REACH_LIGHT - 1.0).abs() < 1e-3, "{label}: reach {} at {light}", p.reach);
            // The pick and every neutral tone on its dark side print as at rest, to the bit.
            let mut beyond = 0;
            for i in 0..=400 {
                let a = neutral(&s, -0.5 + 0.6 * i as f32 / 400.0);
                if toward(a) <= 0.0 {
                    beyond += 1;
                    assert_eq!(p.shown(a), at_rest.shown(a), "{label}, {brightness}");
                }
            }
            assert!(beyond > 20, "{label}, {brightness}: {beyond}");
            // Past the fade and the lift's fold, the light Exposure gives, the ends kept: more on this paper.
            let exposure = conversion::stage47(&moved(&s, |u| u.print.exposure += brightness)).paper.unwrap();
            let exposed = Paper { scale: exposure.scale, brightness: 0.0, ..p };
            assert!((exposure.scale > rest.scale) == (brightness > 0.0), "{label}: a lighter print takes more light");
            let lift = brightness * std::f32::consts::LOG10_2;
            let whole = oni_render::fade(lift, p.reach) + lift.abs();
            for i in 0..=40 {
                let a = neutral(&s, 0.5 + 1.5 * i as f32 / 40.0);
                if toward(a) < whole {
                    continue;
                }
                followed += 1;
                let (now, want) = (p.shown(a), exposed.shown(a));
                assert!(worst(now, want) < 2e-5 * want.iter().copied().fold(0.0, f32::max) + 1e-7, "{label}, {brightness}: {now:?} {want:?}");
                // Twin: the negative paper's sense, the light taken the other way, darkens where it should lighten.
                let wrong = Paper { scale: rest.scale * (rest.scale / exposure.scale), brightness: 0.0, ..p };
                assert!(worst(now, wrong.shown(a)) > 1e-4 || worst(now, p.white) < 1e-3 || now[1] < 1e-3, "{label}, {brightness}");
            }
            // A tone a tenth of the reach over the pick moves under 3 % of the lift's log exposure; Exposure moves it all.
            let (mut deep_low, mut deep_high) = (0.0f32, 1.0f32);
            for _ in 0..60 {
                let mid = 0.5 * (deep_low + deep_high);
                if toward(neutral(&s, mid)) < 0.1 * p.reach { deep_low = mid } else { deep_high = mid }
            }
            let deep = neutral(&s, deep_low);
            let moved_by = p.toward_light(p.pick_log, p.brightened(p.exposed(p.toned(deep)))) - toward(deep);
            let by_exposure = exposure.toward_light(p.pick_log, exposure.exposed(exposure.toned(deep))) - toward(deep);
            assert!(moved_by.abs() <= 0.03 * lift.abs() + 1e-6 && (by_exposure - lift).abs() < 1e-3, "{label}, {brightness}: {moved_by} {by_exposure}");
            // Brightening lightens the grey, darkening darkens it.
            let (grey, was) = (luma(p.shown(neutral(&s, 1.0))), luma(at_rest.shown(neutral(&s, 1.0))));
            assert!((grey > was) == (brightness > 0.0), "{label}, {brightness}: {grey} {was}");
        }
    }
    println!("{followed} tones past the reach checked against Exposure");
    assert!(followed > 40, "{followed}");
}

#[test]
fn the_slide_s_print_stays_monotone_from_its_densest_to_its_bare_film_at_any_brightness() {
    for (label, s) in slides() {
        for brightness in [-4.0f32, -1.0, 1.0, 4.0] {
            let p = conversion::stage47(&moved(&s, |u| u.print.brightness = brightness)).paper.unwrap();
            // From past the densest film (u = 0 here, its pick) to past the bare film, the print only lightens.
            let tones: Vec<[f32; 3]> = (0..=3000).map(|i| p.shown(neutral(&s, -0.5 + 2.5 * i as f32 / 3000.0))).collect();
            let fall = tones.windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0f32, f32::max);
            println!("{label}, Brightness {brightness:+}: largest fall {fall:.1e}");
            assert!(fall < 1e-6, "{label}, {brightness}: {fall}");
            // Twin: a fall of a hundredth past the middle is refused.
            let dipped = tones.iter().enumerate().map(|(i, v)| if i > 1500 { v.map(|x| x - 0.01) } else { *v }).collect::<Vec<_>>();
            assert!(dipped.windows(2).flat_map(|w| (0..3).map(move |c| w[0][c] - w[1][c])).fold(0.0f32, f32::max) > 1e-3);
        }
    }
}
