//! A slide on a colour paper, printed as a lab printed one: copied on a virtual internegative, its reference grey at the
//! film's grey and its densest read at the copy's black, then printed as that negative; Lighting moving it as it moves a
//! negative; its colours kept; off the paper, ONI's own render unmoved; the GPU as its reference. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::paper::internegative::{self, STOCK};
use oni_pipeline::paper::{self, GREY_PRINT};
use oni_pipeline::film::{self, GREY};
use oni_pipeline::{Pipeline, conversion, stages};
use oni_render::{Paper, Stage47, cpu};
use oni_settings::axis::density_axis;
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};

/// A slide at rest on the default paper.
fn slide() -> PipelineSettings {
    PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Positive, true) }
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

fn moved(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t);
    t
}

/// The slide's scan pixel holding densities `d`, through its own gains and pedestal.
fn scan_at(s: &PipelineSettings, d: [f32; 3]) -> V4 {
    let pedestal = density_axis::pedestal(conversion::rendered_ceiling(s));
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / s.gains.stops[c].exp2() - pedestal);
    [light[0], light[1], light[2], 1.0]
}

/// The slide's scan through its copy's light and film, as the chain lays them before stage 4.
fn negative(s: &PipelineSettings, q: V4) -> V4 {
    let copy = internegative::copy(s).expect("a slide on a paper");
    let baked = film::stock(STOCK);
    cpu::virtual_negative(cpu::color_matrix(q, &copy.light), &baked.kernel, &baked.table)
}

/// The slide's scan printed through its internegative on its paper.
fn printed(s: &PipelineSettings, q: V4) -> [f32; 3] {
    rgb(cpu::stage47(negative(s, q), &conversion::stage47(s)))
}

/// The slide's scan on ONI's own print.
fn on_onis_print(s: &PipelineSettings, q: V4) -> [f32; 3] {
    rgb(cpu::stage47(q, &conversion::stage47(&PipelineSettings { print_paper: None, ..s.clone() })))
}

#[test]
fn a_slide_on_a_paper_prints_through_its_internegative_and_off_it_renders_on_onis_print() {
    let s = prepared_slide();
    let copy = internegative::copy(&s).expect("a slide on a paper is copied");
    assert_eq!((copy.settings.mode, copy.settings.film().map(|f| f.stock), copy.settings.print_paper), (ConversionMode::Digital, Some(STOCK), s.print_paper));
    assert_eq!(conversion::stage47(&s), conversion::stage47(&copy.settings));
    assert!(conversion::stage47(&s).paper.is_some() && conversion::printing(&s).is_some());
    // Exposure keeps its distance from the rest, every other Lighting value its value.
    let lit = moved(&s, |t| (t.print.exposure, t.print.contrast) = (0.7, 0.2));
    let copied = internegative::copy(&lit).unwrap().settings.print;
    assert!((copied.exposure - Print::resting_exposure(ConversionMode::Digital) - 0.7).abs() < 1e-6);
    assert_eq!(Print { exposure: 0.7, ..copied }, lit.print);
    // Off the paper, bypassed, a negative or a positive read as its own negative: no copy.
    let off = PipelineSettings { print_paper: None, ..s.clone() };
    assert!(internegative::copy(&off).is_none() && conversion::stage47(&off).paper.is_none() && conversion::stage47(&off).print_on == 1.0);
    assert!(internegative::copy(&moved(&s, |t| t.print.bypassed = true)).is_none());
    assert!(internegative::copy(&PipelineSettings { mode: ConversionMode::Negative, ..s.clone() }).is_none());
    assert!(internegative::copy(&PipelineSettings { mode: ConversionMode::Negative, inverted_source: true, ..s.clone() }).is_none());
    // Twin: the slide off the paper reads its own windows, not the copy's.
    assert_ne!(conversion::stage47(&off), conversion::stage47(&s));
}

#[test]
fn the_slides_grey_prints_a_plain_raws_neutral_its_densest_read_near_black_its_bare_film_near_the_papers_white() {
    for (label, s) in slides() {
        let (bare, densest) = paper::film_ends(&s);
        let [grey, dense, clear] = [paper::reference_grey(&s), densest, bare].map(|d| printed(&s, scan_at(&s, d)));
        let white = conversion::stage47(&s).paper.unwrap().white;
        println!("{label}: grey {grey:?} (a plain RAW's {:.5}), densest {dense:?}, bare film {clear:?} (paper {white:?})", *GREY_PRINT);
        // Within a hundredth: the copy unmixes its dyes, which its stock's reference was placed without.
        assert!((0..3).all(|c| (grey[c] / *GREY_PRINT - 1.0).abs() < 1e-2), "{label}: {grey:?}");
        // The densest read lies on the film's toe, a breath over its base, the print's black: under L* 3.
        assert!(luma(dense) > 0.0 && luma(dense) < 3.3e-3, "{label}: {dense:?}");
        assert!(luma(clear) > 0.6 * luma(white) && luma(clear) <= luma(white) + 1e-6, "{label}: {clear:?}");
        // Paper blacks: the base where the enlarger's rest prints it, the densest read lighter, under the grey.
        let held = printed(&moved(&s, |t| t.print.paper_blacks = true), scan_at(&s, densest));
        assert!(luma(held) > luma(dense) + 1e-3 && luma(held) < 0.1 * luma(grey), "{label}: {held:?}");
        // Twin: the scan's light taken through the gains alone, its grey not brought to the film's, prints another grey.
        let copy = internegative::copy(&s).unwrap();
        let off = cpu::color_matrix(scan_at(&s, paper::reference_grey(&s)), &copy.light).map(|v| v / GREY * 0.5);
        let baked = film::stock(STOCK);
        let wrong = rgb(cpu::stage47(cpu::virtual_negative([off[0], off[1], off[2], 1.0], &baked.kernel, &baked.table), &conversion::stage47(&s)));
        assert!((wrong[1] / *GREY_PRINT - 1.0).abs() > 0.05, "{label}: {wrong:?}");
    }
}

#[test]
fn a_slides_neutral_scale_prints_neutral_from_its_densest_read_to_its_bare_film() {
    for (label, s) in slides() {
        let (bare, densest) = paper::film_ends(&s);
        let grey = paper::reference_grey(&s);
        let mean = |v: [f32; 3]| v.iter().sum::<f32>() / 3.0;
        let (from, to) = (mean(densest) - mean(grey), mean(bare) - mean(grey));
        let mut worst = 0.0f32;
        let mut scale = Vec::new();
        // A wedge on the slide, its grey plus one density on every channel, from its densest read to its bare film.
        for i in 1..=20 {
            let step = from + (to - from) * i as f32 / 20.0;
            let v = printed(&s, scan_at(&s, grey.map(|g| g + step)));
            let cast = (v[0] / v[1]).ln().abs().max((v[2] / v[1]).ln().abs());
            // Under a hundredth of the light, past L* 9, no cast is seen.
            if luma(v) > 0.01 {
                worst = worst.max(cast);
            }
            scale.push((step, luma(v), cast));
        }
        println!("{label}: a neutral scale's worst cast {worst:.4} (natural log of a channel over green); {scale:.4?}");
        assert!(worst < 0.08, "{label}: {worst}");
        assert!(scale.windows(2).all(|w| w[1].1 > w[0].1), "{label}: lighter toward the bare film");
    }
    // Twin: the copy exposed without the slide's gains casts the same scale.
    let s = prepared_slide();
    let ungained = PipelineSettings { gains: Default::default(), ..s.clone() };
    let v = printed(&s, scan_at(&ungained, paper::reference_grey(&s)));
    assert!((v[0] / v[1]).ln().abs().max((v[2] / v[1]).ln().abs()) > 0.1, "{v:?}");
}

/// Oklab's chroma of a linear Rec. 2020 light.
fn chroma(v: [f32; 3]) -> f32 {
    let m1 = [[0.616_645, 0.360_243, 0.023_013], [0.265_164, 0.635_856, 0.099_08], [0.100_1, 0.203_874, 0.695_253]];
    let lms: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| m1[r][c] * v[c]).sum::<f32>().max(0.0).cbrt());
    let a = 1.977_998_5 * lms[0] - 2.428_592_2 * lms[1] + 0.450_593_7 * lms[2];
    let b = 0.025_904_037 * lms[0] + 0.782_771_77 * lms[1] - 0.808_675_77 * lms[2];
    a.hypot(b)
}

/// Each colour of a slide, its grey moved by `depth` of density on one channel or two, through ONI's print of `s` and
/// the copy of `on_paper`: the copy's chroma over ONI's.
fn chroma_kept(s: &PipelineSettings, on_paper: &PipelineSettings, depth: f32) -> Vec<f32> {
    let grey = paper::reference_grey(s);
    let tints: [[f32; 3]; 6] = [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    tints
        .iter()
        .map(|t| {
            let q = scan_at(s, std::array::from_fn(|c| grey[c] + depth * (t[c] - 0.5)));
            chroma(printed(on_paper, q)) / chroma(on_onis_print(s, q))
        })
        .collect()
}

#[test]
fn the_copy_keeps_a_slides_colours_as_onis_print_shows_them() {
    // Its windows at rest, as Prepare leaves a slide: narrowed by hand, ONI's print steepens, the paper keeps its grade.
    let balanced = PipelineSettings { gains: prepared_slide().gains, ..slide() };
    for (label, s) in [("slide at rest", slide()), ("balanced slide", balanced)] {
        let kept = chroma_kept(&s, &s, 0.6);
        println!("{label}: chroma kept, red, green, blue, cyan, magenta, yellow: {kept:.3?}");
        // The reversal paper this copy replaced kept 0.49 to 0.63 of it on the owner's slides.
        assert!(kept.iter().all(|k| (0.8..1.6).contains(k)), "{label}: {kept:?}");
        // Twin: the print alone at a colour density as low as that paper held, which the check refuses.
        let washed = chroma_kept(&s, &moved(&s, |t| t.density = -0.5), 0.6);
        println!("{label}, Saturation -0.5: {washed:.3?}");
        assert!(washed.iter().any(|k| *k < 0.8), "{label}: {washed:?}");
    }
}

#[test]
fn lighting_moves_a_slide_on_its_paper_as_it_moves_a_negative() {
    let s = prepared_slide();
    let grey_scan = scan_at(&s, paper::reference_grey(&s));
    let near = scan_at(&s, paper::reference_grey(&s).map(|d| d + 0.4));
    let rest = printed(&s, grey_scan);
    // Exposure: a stop brighter is less light on the negative paper.
    let over = moved(&s, |t| t.print.exposure += 1.0);
    let (k0, k1) = (conversion::stage47(&s).paper.unwrap(), conversion::stage47(&over).paper.unwrap());
    assert!(luma(printed(&over, grey_scan)) > 1.3 * luma(rest) && (k0.scale / k1.scale - 2.0).abs() < 1e-4);
    // A channel's filter: red brightened is a redder grey of the same light, the enlarger's cyan held at none.
    let red = moved(&s, |t| t.print.set_filters([0.5, 0.0, 0.0]));
    let r = printed(&red, grey_scan);
    let (f0, f1) = (conversion::printing(&s).unwrap().filtration, conversion::printing(&red).unwrap().filtration);
    println!("red: {r:?} from {rest:?}, filtration {f1:?} from {f0:?}");
    assert!(r[0] / r[1] > 1.1 * rest[0] / rest[1] && (luma(r) / luma(rest) - 1.0).abs() < 0.02, "{r:?}");
    // Contrast: the grade turns about the grey, a denser tone darker.
    let hard = moved(&s, |t| t.print.contrast = 0.5);
    assert!((luma(printed(&hard, grey_scan)) / luma(rest) - 1.0).abs() < 0.01 && luma(printed(&hard, near)) < 0.9 * luma(printed(&s, near)));
    // Shadows lift the slide's dense tones.
    assert!(luma(printed(&moved(&s, |t| t.print.shadows = 0.5), near)) > 1.05 * luma(printed(&s, near)));
    // Twin: a reversal paper's sense, a stop brighter as more light, darkens the grey instead.
    let wrong = Stage47 { paper: Some(Paper { scale: k0.scale * 2.0, ..k1 }), ..conversion::stage47(&over) };
    assert!(luma(rgb(cpu::stage47(negative(&s, grey_scan), &wrong))) < luma(rest));
}

#[test]
fn the_gpu_prints_a_slide_through_its_internegative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_prints_a_slide_through_its_internegative_as_its_reference_does") else { return };
    let (w, h) = (29, 17);
    let s = prepared_slide();
    let mut lit = moved(&s, |t| (t.print.exposure, t.print.contrast, t.print.shadows, t.print.brightness) = (0.4, 0.2, 0.1, 0.5));
    lit.print.set_filters([0.1, 0.0, -0.2]);
    let mut pixels: Vec<V4> = noise(w, h, 5, 0.0, 1.0).iter().map(|p| p.map(|v| v * v)).collect();
    pixels[0] = scan_at(&s, paper::film_ends(&s).1);
    pixels[1] = scan_at(&s, paper::film_ends(&s).0);
    let source = g.upload(w, h, &pixels).unwrap();
    let mut pipeline = Pipeline::new();
    for (label, s) in [("slide", s.clone()), ("lit slide", lit)] {
        let copy = internegative::copy(&s).unwrap();
        let k = conversion::stage47(&s);
        let filmed = pipeline.filmed(&mut g, Image::new(&source).then(stages::color_matrix(&copy.light)), &copy.settings).unwrap();
        let out = whole(&mut g, &stages::converted(filmed, &k));
        let reference = mapped(&pixels, |q| cpu::stage47(negative(&s, q), &k));
        let twin = Stage47 { paper: k.paper.map(|p| Paper { scale: p.scale * 1.1, ..p }), ..k };
        judge(&format!("a {label} through its internegative"), &out, &reference, &mapped(&pixels, |q| cpu::stage47(negative(&s, q), &twin)));
        // The editor's chain lays the same, its output clipped.
        let request = oni_pipeline::Request::default();
        let chain = pipeline.render(&mut g, &source, &s.per_pixel_only(), &request, &Default::default(), None).unwrap().1;
        let clipped = mapped(&reference, |p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), p[3]]);
        let onis = mapped(&pixels, |q| cpu::stage47(q, &conversion::stage47(&PipelineSettings { print_paper: None, ..s.clone() })));
        judge(&format!("the editor's chain, a {label}"), &chain, &clipped, &onis);
    }
}

#[test]
fn brightness_on_a_slide_holds_its_densest_read_and_moves_its_grey_as_exposure_does() {
    for (label, s) in slides() {
        let densest = scan_at(&s, paper::film_ends(&s).1);
        let grey = scan_at(&s, paper::reference_grey(&s));
        for brightness in [-2.0f32, -1.0, 1.0, 2.0] {
            let lit = moved(&s, |t| t.print.brightness = brightness);
            let p = conversion::stage47(&lit).paper.unwrap();
            let k = conversion::stage47(&lit);
            // What the paper reads of the copy's scan: its dyes unmixed, then its amounts and Lighting's tones.
            let read = |q: V4| k.dyes.as_ref().map_or(negative(&s, q), |d| cpu::dye_separation(negative(&s, q), &k, d));
            let amounts = |q: V4| p.toned(p.amounts(read(q)));
            let toward = |q: V4| p.toward_light(p.pick_log, p.exposed(amounts(q)));
            let moved_by = |q: V4| p.toward_light(p.pick_log, p.brightened(p.exposed(amounts(q)))) - toward(q);
            let lift = brightness * std::f32::consts::LOG10_2;
            println!("{label}, Brightness {brightness:+}: densest {:.4} over the pick (reach {:.4}) moved {:.5}; grey {:.4} moved {:.5}", toward(densest), p.reach, moved_by(densest), toward(grey), moved_by(grey));
            // The slide's densest read lies in the copy's toe, a quarter of the reach over its base: under a quarter of the lift.
            assert!(toward(densest) < 0.3 * p.reach && moved_by(densest).abs() <= 0.25 * lift.abs(), "{label}, {brightness}");
            // Its grey moves the whole lift once past the fade, as Exposure moves it; within it, part of it, the same way.
            if toward(grey) > oni_render::fade(lift, p.reach) + lift.abs() {
                assert!((moved_by(grey) - lift).abs() < 1e-3, "{label}, {brightness}");
            } else {
                assert!(moved_by(grey) / lift > 0.5 && moved_by(grey) / lift <= 1.0 + 1e-4, "{label}, {brightness}");
            }
            // Twin: Exposure's stops move the densest read as much as the grey.
            let exposure = conversion::stage47(&moved(&s, |t| t.print.exposure += brightness)).paper.unwrap();
            let by_exposure = exposure.toward_light(p.pick_log, exposure.exposed(amounts(densest))) - toward(densest);
            assert!((by_exposure - lift).abs() < 1e-3, "{label}, {brightness}: {by_exposure}");
        }
    }
}
