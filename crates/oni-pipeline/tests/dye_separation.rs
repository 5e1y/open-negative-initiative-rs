//! The dye separation on the stocks' measured dyes: the virtual negative's scan separates into the dye amounts it
//! developed, every set's steps converge on a negative's mixtures, the windows open on the bare film, the GPU
//! runs its reference, and only a colour negative at a dose unmixes so. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::conversion;
use oni_pipeline::film::{self, GREY, dyes};
use oni_pipeline::stages;
use oni_render::{DyeSeparation, Stage47, cpu};
use oni_settings::frame::RgbPasses;
use oni_settings::{ConversionMode, DyeSet, PipelineSettings, VirtualStock};

/// The ColorChecker's patches as linear Rec. 2020 on D65, from their published sRGB values.
fn chart() -> Vec<[f32; 3]> {
    let srgb: [[f32; 3]; 24] = [
        [115.0, 82.0, 68.0], [194.0, 150.0, 130.0], [98.0, 122.0, 157.0], [87.0, 108.0, 67.0], [133.0, 128.0, 177.0], [103.0, 189.0, 170.0],
        [214.0, 126.0, 44.0], [80.0, 91.0, 166.0], [193.0, 90.0, 99.0], [94.0, 60.0, 108.0], [157.0, 188.0, 64.0], [224.0, 163.0, 46.0],
        [56.0, 61.0, 150.0], [70.0, 148.0, 73.0], [175.0, 54.0, 60.0], [231.0, 199.0, 31.0], [187.0, 86.0, 149.0], [8.0, 133.0, 161.0],
        [243.0, 243.0, 242.0], [200.0, 200.0, 200.0], [160.0, 160.0, 160.0], [122.0, 122.0, 121.0], [85.0, 85.0, 85.0], [52.0, 52.0, 52.0],
    ];
    let linear = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let to_2020 = [[0.627_403_9, 0.329_283_04, 0.043_313_07], [0.069_097_29, 0.919_540_4, 0.011_362_32], [0.016_391_44, 0.088_013_31, 0.895_595_2]];
    srgb.iter().map(|p| { let l = p.map(linear); to_2020.map(|r| r[0] * l[0] + r[1] * l[1] + r[2] * l[2]) }).collect()
}

/// The chart from four stops under to three over, and a grey ramp over the film's whole latitude.
fn scenes() -> Vec<V4> {
    let mut out = Vec::new();
    for stops in -4..=3 {
        out.extend(chart().iter().map(|p| { let k = 2f32.powi(stops); [p[0] * k, p[1] * k, p[2] * k, 1.0] }));
    }
    out.extend((-8..=6).map(|s| { let g = GREY * 2f32.powi(s); [g, g, g, 1.0] }));
    out
}

/// Stage 4 reading the camera's light whole: unit gains, no pedestal, the bare film the camera's own.
fn reading(set: DyeSet, dose: f32) -> (Stage47, DyeSeparation) {
    let d = dyes::dyes(set);
    let k = Stage47 { gain: [1.0; 3], pedestal: 0.0, log_guard: 1e-12, ..Stage47::rest() };
    (k, d.separation(d.bare_density(), dose))
}

fn amounts(light: V4) -> [f32; 3] {
    [light[0], light[1], light[2]].map(|v| -v.log10())
}

#[test]
fn the_virtual_negatives_scan_separates_into_the_dye_amounts_it_developed() {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (k, d) = reading(DyeSet::KodakPortra800, 1.0);
    let (mut worst, mut read_as_scanned, mut other_set) = (0.0f32, 0.0f32, 0.0f32);
    let (gold_k, gold) = reading(DyeSet::KodakGold200, 1.0);
    for scene in scenes() {
        let developed = cpu::virtual_dyes(scene, &stock.kernel, &stock.table);
        let scan = cpu::virtual_negative(scene, &stock.kernel, &stock.table);
        let separated = amounts(cpu::dye_separation(scan, &k, &d));
        let as_scanned = amounts(cpu::dye_separation(scan, &k, &DyeSeparation { dose: 0.0, ..d }));
        let on_gold = amounts(cpu::dye_separation(scan, &gold_k, &gold));
        for c in 0..3 {
            worst = worst.max((separated[c] - developed[c]).abs());
            read_as_scanned = read_as_scanned.max((as_scanned[c] - developed[c]).abs());
            other_set = other_set.max((on_gold[c] - developed[c]).abs());
        }
    }
    println!("dye amounts recovered within {worst:.2e}; read as scanned {read_as_scanned:.3}; on Gold 200's dyes {other_set:.3}");
    assert!(worst < 2e-4, "the separation misses the developed dyes by {worst}");
    // Twins: the scan's own densities, and another stock's dyes, are not the dyes this negative holds.
    assert!(read_as_scanned > 0.1 && other_set > 0.02, "{read_as_scanned} {other_set}");
}

#[test]
fn every_set_converges_on_a_negatives_mixtures_where_its_matrix_alone_misses() {
    let cloud: Vec<[f32; 3]> = noise(40, 40, 11, 0.0, 1.0)
        .iter()
        .map(|p| { let grey = 2.2 * p[0]; [grey + 0.6 * (p[1] - 0.5), grey, grey + 0.6 * (p[2] - 0.5)].map(|v| v.max(-0.1)) })
        .collect();
    for set in DyeSet::ALL {
        let d = dyes::dyes(set);
        let measured = dyes::measured(set);
        let (scan, camera) = (film::bake::scan_weights(&film::cie::D50), film::bake::camera_weights(&film::cie::D50));
        let bare = d.bare_density();
        let (mut worst, mut first) = (0.0f32, 0.0f32);
        for c in &cloud {
            let (density, _) = dyes::read(&measured, &scan, c.map(f64::from));
            let above: [f32; 3] = std::array::from_fn(|i| density[i] as f32 - bare[i]);
            let back = d.separation([0.0; 3], 1.0).separated(above);
            let (own, _) = dyes::read(&measured, &camera, c.map(f64::from));
            let guess: [f32; 3] = std::array::from_fn(|i| (0..3).map(|j| d.matrix[i][j] * (own[j] as f32 - d.base[j])).sum());
            for i in 0..3 {
                worst = worst.max((back[i] - c[i]).abs());
                first = first.max((guess[i] - c[i]).abs());
            }
        }
        println!("{set:?}: within {worst:.2e}, the matrix alone {first:.3}, matrix {:?}", d.matrix);
        assert!(worst < 2e-4, "{set:?} misses by {worst}");
        // Twin: the matrix's straight line is not the dyes: the steps carry the separation.
        assert!(first > 0.01, "{set:?}: {first}");
    }
}

#[test]
fn the_generic_set_is_the_c41_profiles_mean_and_none_of_them() {
    let generic = dyes::measured(DyeSet::GenericC41);
    let portra = dyes::measured(DyeSet::KodakPortra400);
    assert!(generic.len() >= portra.len() - 4, "the generic set spans the band most profiles measure: {}", generic.len());
    let mean_at = |l: usize| {
        let profile = |n: &str| film::profile::Profile::read(dyes::source(n).unwrap()).unwrap();
        let at: Vec<[f64; 4]> = dyes::C41.iter().filter_map(|n| film::bake::scanned(&profile(n)).into_iter().find(|(w, _)| *w == l).map(|(_, v)| v)).collect();
        std::array::from_fn::<f64, 4, _>(|k| at.iter().map(|v| v[k]).sum::<f64>() / at.len() as f64)
    };
    for (l, v) in &generic {
        let m = mean_at(*l);
        assert!((0..4).all(|k| (v[k] - m[k]).abs() < 1e-12), "{l}");
    }
    // Twin: no profile's own dyes are the mean.
    for set in DyeSet::ALL.into_iter().filter(|s| !s.is_generic()) {
        assert_ne!(dyes::dyes(set).matrix, dyes::dyes(DyeSet::GenericC41).matrix, "{set:?}");
    }
}

/// A digital photo at rest, its neighbourhood stages off so a pixel prints alone.
/// A digital photo on ONI's print, whose windows read the dyes' tones.
fn filmed() -> PipelineSettings {
    PipelineSettings { print_paper: None, ..PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only() }
}

#[test]
fn a_digital_photo_unmixes_its_stocks_own_dyes_from_its_bare_film() {
    let s = filmed();
    let k = conversion::stage47(&s);
    let d = k.dyes.expect("a digital photo unmixes its stock's measured dyes");
    assert_eq!(s.dye_set, DyeSet::KodakPortra800);
    assert_eq!((d.matrix, d.dose), (dyes::dyes(DyeSet::KodakPortra800).matrix, s.separation));
    // The bare film opens every window: its dyes are none.
    let stock = film::stock(VirtualStock::KodakPortra800);
    let bare = cpu::virtual_negative([0.0, 0.0, 0.0, 1.0], &stock.kernel, &stock.table);
    let windowed = Stage47 { print_on: 0.0, hunt: 0.0, density: 1.0, ..k };
    let open = cpu::stage47(bare, &windowed);
    assert!(open[..3].iter().all(|v| v.abs() < 1e-4), "{open:?}");
    let after = cpu::dye_separation(bare, &k, &d);
    assert!((0..3).all(|c| (-after[c].log10() - d.black[c]).abs() < 1e-3), "the bare film reads its own density: {after:?}");
    // Twin: at no dose the photo reads its scan's own densities, which lie above the bare film too.
    let plain = conversion::stage47(&PipelineSettings { separation: 0.0, ..s.clone() });
    assert!(plain.dyes.is_none() && cpu::stage47(bare, &Stage47 { print_on: 0.0, hunt: 0.0, density: 1.0, ..plain })[..3].iter().all(|v| v.abs() < 1e-4));
}

#[test]
fn only_a_colour_negative_at_a_dose_unmixes_measured_dyes() {
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(conversion::stage47(&negative).dyes.map(|d| d.dose), Some(1.0), "the whole unmix at rest");
    let off = [
        PipelineSettings { separation: 0.0, ..negative.clone() },
        PipelineSettings { inverted_source: true, ..negative.clone() },
        PipelineSettings { rgb_passes: Some(RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None }), ..negative.clone() },
        PipelineSettings::rest(ConversionMode::Positive, true),
        PipelineSettings::rest(ConversionMode::Monochrome, true),
    ];
    for s in &off {
        let k = conversion::stage47(s);
        assert!(k.dyes.is_none(), "{:?}", (s.mode, s.separation));
    }
    // Twin: the windows stay the frame's own on the dyes, read through their tones.
    let k = conversion::stage47(&negative);
    assert_eq!((k.red[0], k.red[1]), (negative.levels.red.black, negative.levels.red.white));
}

#[test]
fn the_gpu_separates_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_separates_as_its_reference_does") else { return };
    let stock = film::stock(VirtualStock::KodakPortra800);
    let (w, h) = (37, 23);
    let mut pixels: Vec<V4> = noise(w, h, 5, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    let edges: [V4; 5] = [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0], [-0.1, 0.2, 0.3, 1.0], [3.0, 2.0, 1.0, 1.0], [1e-6, 0.4, 0.002, 0.5]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let source = g.upload(w, h, &pixels).unwrap();
    let k = conversion::stage47(&filmed());
    let out = whole(&mut g, &stages::converted(Image::new(&source), &k));
    let reference = mapped(&pixels, |p| cpu::stage47(p, &k));
    let twin_k = Stage47 { dyes: k.dyes.map(|d| DyeSeparation { dose: d.dose * 0.9, ..d }), ..k };
    judge("dye separation then stage 4-7", &out, &reference, &mapped(&pixels, |p| cpu::stage47(p, &twin_k)));
    let alone = whole(&mut g, &Image::new(&source).then(stages::dye_separation(&k, &k.dyes.unwrap())));
    judge("dye separation", &alone, &mapped(&pixels, |p| cpu::dye_separation(p, &k, &k.dyes.unwrap())), &pixels);
}

#[test]
fn the_dyes_tones_keep_every_grey_the_windows_print_neutral_and_unmix_the_colours() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-0.8, 0.0, 1.2];
    let windows = [(0.55, 1.6, 0.45), (0.54, 1.75, 0.5), (0.52, 1.9, 0.58)];
    for (w, (black, white, mid)) in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue].into_iter().zip(windows) {
        (w.black, w.white, w.mid) = (black, white, mid);
    }
    let family = PipelineSettings { separation: 0.0, ..s.clone() };
    let (on, off) = (conversion::stage47(&s), conversion::stage47(&family));
    assert!(on.dyes.is_some());
    // A grey the windows print neutral: each channel's share of its window, through its own gamma.
    let grey = |level: f32| -> V4 {
        let w = s.levels.windows();
        let d: [f32; 3] = std::array::from_fn(|c| w[c].black + level.powf(1.0 / w[c].gamma()) * (w[c].white - w[c].black));
        let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / on.gain[c] - on.pedestal);
        [light[0], light[1], light[2], 1.0]
    };
    let mut worst = 0.0f32;
    for i in 0..=30 {
        let p = grey(0.05 + 1.2 * i as f32 / 30.0);
        let (a, b) = (cpu::stage47(p, &on), cpu::stage47(p, &off));
        worst = (0..3).map(|c| (a[c] - b[c]).abs()).fold(worst, f32::max);
    }
    println!("a neutral grey prints within {worst:.1e} of the unmixed scan");
    assert!(worst < 1e-3, "{worst}");
    // Twin: a colour is unmixed, more saturated than the scan reads it.
    let colour = { let g = grey(0.5); [g[0] * 1.6, g[1], g[2] * 0.7, 1.0] };
    let (a, b) = (cpu::stage47(colour, &on), cpu::stage47(colour, &off));
    let spread = |p: V4| p[..3].iter().cloned().fold(f32::MIN, f32::max) - p[..3].iter().cloned().fold(f32::MAX, f32::min);
    println!("a colour's spread: unmixed {:.3}, scanned {:.3}", spread(a), spread(b));
    assert!(spread(a) > spread(b) + 0.02);
}
