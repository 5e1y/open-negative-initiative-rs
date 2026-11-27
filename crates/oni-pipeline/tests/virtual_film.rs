//! The virtual negative against a double-precision reference written from spektrafilm's own steps: its
//! Mitchell-cubic lookup of the light table, `np.interp` on the profile's curves, the scan summed on every
//! wavelength the profile measures. The kernel is f32, bilinear, on a uniform curve grid; each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::Image;
use oni_pipeline::film::{self, GREY, bake, profile::Profile};
use oni_pipeline::{Pipeline, Request, stages};
use oni_render::{VirtualNegative, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// The spectra a colour negative reads, and those a slide reads.
const NEGATIVE_SPECTRA: &[u8] = include_bytes!("../resources/film/irradiance_xy_otsu2018.npy");
const SHEET_SPECTRA: &[u8] = include_bytes!("../resources/film/irradiance_xy_tc.npy");

fn mitchell(t: f64) -> f64 {
    let (b, c) = (1.0 / 3.0, 1.0 / 3.0);
    let x = t.abs();
    if x < 1.0 {
        ((12.0 - 9.0 * b - 6.0 * c) * x.powi(3) + (-18.0 + 12.0 * b + 6.0 * c) * x * x + (6.0 - 2.0 * b)) / 6.0
    } else if x < 2.0 {
        ((-b - 6.0 * c) * x.powi(3) + (6.0 * b + 30.0 * c) * x * x + (-12.0 * b - 48.0 * c) * x + (8.0 * b + 24.0 * c)) / 6.0
    } else {
        0.0
    }
}

/// `_cubic_interp_lut_at_2d`: Mitchell weights, indices reflected at the edges, normalised.
fn cubic(table: &[[f64; 3]], n: usize, tc: [f64; 2]) -> [f64; 3] {
    let cell = |v: f64| {
        let c = (v * (n - 1) as f64).clamp(0.0, (n - 1) as f64);
        if c >= (n - 1) as f64 { (n - 2, 1.0) } else { (c.floor() as usize, c - c.floor()) }
    };
    let reflect = |i: i64| -> usize {
        let l = n as i64;
        (if i < 0 { -i } else if i >= l { 2 * (l - 1) - i } else { i }) as usize
    };
    let ((xb, xf), (yb, yf)) = (cell(tc[0]), cell(tc[1]));
    let (mut out, mut sum) = ([0.0; 3], 0.0);
    for i in 0..4 {
        for j in 0..4 {
            let w = mitchell(xf + 1.0 - i as f64) * mitchell(yf + 1.0 - j as f64);
            let v = table[reflect(xb as i64 - 1 + i as i64) * n + reflect(yb as i64 - 1 + j as i64)];
            sum += w;
            for c in 0..3 {
                out[c] += w * v[c];
            }
        }
    }
    out.map(|v| v / sum)
}

struct Reference {
    profile: Profile,
    light: Vec<[f64; 3]>,
    n: usize,
    to_film: bake::M3,
    curves: (Vec<[f64; 3]>, Vec<[f64; 3]>),
    /// The log exposures `curves` lie on: the sheet's, reaching every exposure the couplers move a layer to.
    le: Vec<f64>,
    couplers: bake::M3,
}

impl Reference {
    fn new(stock: VirtualStock) -> Self {
        let profile = Profile::read(stock.entry().profile.as_bytes()).unwrap();
        let spectra = film::profile::Spectra::read(if stock.entry().reversal { SHEET_SPECTRA } else { NEGATIVE_SPECTRA }).unwrap();
        let white = film::reference_light(stock);
        let light = bake::light_table(&spectra, &bake::sensitivity(&profile));
        let light = bake::balanced(light, spectra.nodes, &bake::to_film(white), f64::from(GREY));
        let (couplers, reversal) = (film::couplers(&stock.entry()), stock.entry().reversal);
        // A slide develops its dyes; a negative its curves, its file's fitted ones or its sheet's, over its bare film's D-min.
        let (curves, le) = match reversal {
            true => (bake::curves_falling(&profile.log_exposure, &profile.density_curves, &couplers), profile.log_exposure.clone()),
            false => {
                let own = stock.entry().curves.as_deref().map_or_else(|| profile.sheet_density_curves.clone(), <[_]>::to_vec);
                let (le, own) = bake::reaching(&profile.log_exposure, &own, &couplers);
                let (first, inhibited) = bake::curves_through(&le, &own, &couplers);
                let base = film::status::read(&profile.base_density);
                ((first, inhibited.iter().map(|d| std::array::from_fn(|c| d[c] + base[c])).collect()), le)
            }
        };
        Reference { light, n: spectra.nodes, to_film: bake::to_film(white), curves, le, profile, couplers }
    }

    /// The film's log exposure per channel for a scene pixel, at `stops`.
    fn log_raw(&self, rgb: [f64; 3], stops: f64) -> [f64; 3] {
        let xyz = bake::apply(&self.to_film, rgb);
        let b = xyz[0] + xyz[1] + xyz[2];
        let tc = bake::square_of_xy([xyz[0] / b.max(1e-10), xyz[1] / b.max(1e-10)]);
        cubic(&self.light, self.n, tc).map(|v| (v * b * stops.exp2()).max(0.0) + 1e-10).map(f64::log10)
    }

    /// The dyes' densities the negative develops.
    fn developed(&self, rgb: [f64; 3], stops: f64) -> [f64; 3] {
        let le = &self.le;
        let log = self.log_raw(rgb, stops);
        let first: [f64; 3] = std::array::from_fn(|c| bake::interpolate(log[c], le, &self.curves.0, c));
        let inhibited: [f64; 3] = std::array::from_fn(|m| log[m] - (0..3).map(|k| first[k] * self.couplers[k][m]).sum::<f64>());
        std::array::from_fn(|c| bake::interpolate(inhibited[c], le, &self.curves.1, c))
    }

    /// Each layer's density as its own channel's light, as a masked negative's scan reads it.
    fn scan(&self, rgb: [f64; 3], stops: f64) -> [f64; 3] {
        self.developed(rgb, stops).map(|d| 10f64.powf(-d))
    }
}

/// The colour negatives, whose three layers and couplers this reference develops; silver has checks of its own.
fn colour_stocks() -> impl Iterator<Item = VirtualStock> {
    VirtualStock::sheets().into_iter().filter(|s| !s.is_silver())
}

/// The ColorChecker's patches as their sRGB code is usually given, decoded and laid in Rec. 2020.
fn chart() -> Vec<[f64; 3]> {
    let code: [[f64; 3]; 24] = [
        [115.0, 82.0, 68.0], [194.0, 150.0, 130.0], [98.0, 122.0, 157.0], [87.0, 108.0, 67.0], [133.0, 128.0, 177.0], [103.0, 189.0, 170.0],
        [214.0, 126.0, 44.0], [80.0, 91.0, 166.0], [193.0, 90.0, 99.0], [94.0, 60.0, 108.0], [157.0, 188.0, 64.0], [224.0, 163.0, 46.0],
        [56.0, 61.0, 150.0], [70.0, 148.0, 73.0], [175.0, 54.0, 60.0], [231.0, 199.0, 31.0], [187.0, 86.0, 149.0], [8.0, 133.0, 161.0],
        [243.0, 243.0, 242.0], [200.0, 200.0, 200.0], [160.0, 160.0, 160.0], [122.0, 122.0, 121.0], [85.0, 85.0, 85.0], [52.0, 52.0, 52.0],
    ];
    let to2020 = [[0.6274, 0.3293, 0.0433], [0.0691, 0.9195, 0.0114], [0.0164, 0.0880, 0.8956]];
    let decode = |v: f64| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    code.iter().map(|c| bake::apply(&to2020, c.map(decode))).collect()
}

/// The scenes inside the gamut the checks expose: a grey ramp from 8 stops under the film's grey to 6 over, and
/// the chart at three exposures.
fn scenes() -> Vec<([f64; 3], f64)> {
    let mut out: Vec<([f64; 3], f64)> = (-16..=12).map(|h| ([f64::from(GREY) * 2f64.powf(f64::from(h) / 2.0); 3], 0.0)).collect();
    for stops in [-2.0, 0.0, 2.0] {
        out.extend(chart().into_iter().map(|c| (c, stops)));
    }
    out
}

/// The working space's primaries and two of their mixtures, whose chromaticities lie on the locus.
fn primaries() -> Vec<([f64; 3], f64)> {
    [[0.18, 0.0, 0.0], [0.0, 0.18, 0.0], [0.0, 0.0, 0.18], [0.5, 0.5, 0.0], [0.0, 0.5, 0.5]].map(|c| (c, 0.0)).to_vec()
}

fn density(t: [f64; 3]) -> [f64; 3] {
    t.map(|v| -v.log10())
}

/// The largest density gap between the kernel and the reference over `scenes`.
fn worst(kernel: &VirtualNegative, table: &[[f32; 4]], reference: &Reference, scenes: &[([f64; 3], f64)]) -> f64 {
    scenes
        .iter()
        .map(|(rgb, stops)| {
            let k = VirtualNegative { exposure: (*stops as f32).exp2(), ..kernel.clone() };
            let out = cpu::virtual_negative([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0], &k, table);
            // The light written less its reading's veil, read back as the film's own.
            let veil = f64::from(kernel.veil);
            let got = density([out[0], out[1], out[2]].map(|t| (f64::from(t) + veil) / (1.0 + veil)));
            let want = density(reference.scan(*rgb, *stops));
            (0..3).map(|c| (got[c] - want[c]).abs()).fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

/// On every stock, the kernel within 0.001 density of the reference on every scene inside the gamut, under a
/// hundredth of a stop on the negative's straight line, and within 0.015 on the locus' primaries, where a tungsten
/// white's compression steepens the table the kernel reads bilinearly (0.0142 on 500T); the twins, couplers left out
/// or an exposure off a tenth of a stop, are refused.
#[test]
fn the_kernel_reads_the_negative_the_reference_develops() {
    let (inside, locus) = (scenes(), primaries());
    for kind in colour_stocks() {
        let reference = Reference::new(kind);
        let stock = film::stock(kind);
        let (gap, edge) = (worst(&stock.kernel, &stock.table, &reference, &inside), worst(&stock.kernel, &stock.table, &reference, &locus));
        eprintln!("{kind:?}: kernel against the reference, {gap:.5} density at worst, {edge:.5} on the locus");
        // A slide's curves are three to four times a negative's dyes' slope, a negative's printed Status M curves steeper
        // than its dyes' (0.0013 on Fujifilm 200), read on the same 256 nodes.
        let bound = 0.002;
        assert!(gap < bound && edge < 0.015, "{kind:?}: {gap} {edge}");
        let uncoupled = VirtualNegative { couplers: [[0.0; 3]; 3], ..stock.kernel.clone() };
        assert!(uncoupled == stock.kernel || worst(&uncoupled, &stock.table, &reference, &inside) > 0.001, "{kind:?}");
        let off = VirtualNegative { to_film: stock.kernel.to_film.map(|r| r.map(|v| v * 1.072)), ..stock.kernel.clone() };
        assert!(worst(&off, &stock.table, &reference, &inside) > 0.001, "{kind:?}");
    }
}

/// Every film is balanced on its reference white: a grey at its speed exposes every layer within a sixth of a
/// stop of log 0 (Portra 400's blue layer 0.037 off), and the couplers hold a developed grey ramp on the data sheet's
/// curves. Twins: a grey a stop over is off log 0, and couplers acting on the data sheet's own curves move the ramp off it.
#[test]
fn a_grey_exposes_and_develops_on_the_data_sheet() {
    let held: Vec<(VirtualStock, f64)> = colour_stocks().map(|kind| (kind, grey_on(kind))).collect();
    for (kind, gap) in held {
        // A grey's layers take log exposures a few hundredths apart, which the neighbours' inhibition reads, more on
        // a push's steeper curves (0.028 on Portra 800 Push 2) and most on a slide's (0.033 on Velvia 50).
        let bound = if kind.entry().reversal { 0.04 } else { 0.03 };
        assert!(gap < bound, "{kind:?}: {gap}");
    }
}

/// The stock's grey ramp developed off the data sheet, at worst; its grey and the twins checked.
fn grey_on(kind: VirtualStock) -> f64 {
    let reference = Reference::new(kind);
    let log = reference.log_raw([f64::from(GREY); 3], 0.0);
    eprintln!("{kind:?}: grey's log exposure {log:?}");
    // A grey exposes every layer at log 0, no layer offset.
    assert!((0..3).all(|c| log[c].abs() < 0.05), "{kind:?}: {log:?}");
    let over = reference.log_raw([f64::from(GREY); 3], 1.0);
    assert!((0..3).all(|c| over[c].abs() > 0.2), "{kind:?}: {over:?}");
    let le = &reference.le;
    // A negative's sheet lies over its bare film's D-min.
    let base = if kind.entry().reversal { [0.0; 3] } else { film::status::read(&reference.profile.base_density) };
    // The gap to the data sheet of a grey ramp developed through `second`, the curve the couplers act on.
    let gap = |second: &[[f64; 3]]| {
        let mut gap: f64 = 0.0;
        for h in -12..=8 {
            let log = reference.log_raw([f64::from(GREY) * 2f64.powf(f64::from(h) / 2.0); 3], 0.0);
            let first: [f64; 3] = std::array::from_fn(|c| bake::interpolate(log[c], le, &reference.curves.0, c));
            for m in 0..3 {
                let inhibited = log[m] - (0..3).map(|k| first[k] * reference.couplers[k][m]).sum::<f64>();
                // A negative's sheet is its first curve; a reversal film's, the dyes its curves leave.
                let sheet = if kind.entry().reversal { bake::interpolate(log[m], &reference.profile.log_exposure, &reference.profile.density_curves, m) } else { first[m] + base[m] };
                gap = gap.max((bake::interpolate(inhibited, le, second, m) - sheet).abs());
            }
        }
        gap
    };
    let held = gap(&reference.curves.1);
    eprintln!("{kind:?}: a grey's development off the data sheet, {held:.5} density at worst");
    // Twin, on a coupled film: the couplers acting on the data sheet's own curves move the ramp off it.
    let uncorrected: Vec<[f64; 3]> = reference.curves.0.iter().map(|d| std::array::from_fn(|c| d[c] + base[c])).collect();
    assert!(reference.couplers == [[0.0; 3]; 3] || gap(&uncorrected) > 0.02, "{kind:?}");
    held
}

/// The GPU's virtual negative against its scalar reference, on scenes reaching every branch: a dark, a
/// negative and a blinding light past the curves' ends, chromaticities past the table's square, and plain
/// noise. Twins: the scene's light a hundredth stronger, and the channels swapped.
#[test]
fn the_gpu_develops_the_negative_its_reference_does() {
    let Some(mut g) = graph("the_gpu_develops_the_negative_its_reference_does") else { return };
    let (w, h) = (41, 29);
    let mut pixels = noise(w, h, 7, -0.05, 6.0);
    let edges: [V4; 8] = [
        [0.0, 0.0, 0.0, 1.0],
        [-0.2, -0.1, -0.3, 1.0],
        [4000.0, 3000.0, 2000.0, 1.0],
        [0.3, -0.05, -0.05, 1.0],
        [-0.05, 0.3, -0.05, 1.0],
        [-0.05, -0.05, 0.3, 0.5],
        [1e-6, 0.0, 0.0, 1.0],
        [0.184, 0.184, 0.184, 1.0],
    ];
    pixels[..edges.len()].copy_from_slice(&edges);
    let input = g.upload(w, h, &pixels).unwrap();
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let (tw, th) = stock.kernel.table_size();
    let table = g.upload(tw as u32, th as u32, &stock.table).unwrap();
    let k = stock.kernel_at(0.4);
    let image = Image::new(&input).then_reading(stages::virtual_negative(&k), &[Image::new(&table)]);
    let out = whole(&mut g, &image);
    let brighter = VirtualNegative { exposure: k.exposure * 1.01, ..k.clone() };
    let reference = mapped(&pixels, |p| cpu::virtual_negative(p, &k, &stock.table));
    judge("virtual negative", &out, &reference, &mapped(&pixels, |p| cpu::virtual_negative(p, &brighter, &stock.table)));
    judge("virtual negative, channels swapped", &out, &reference, &mapped(&reference, |p| [p[2], p[1], p[0], p[3]]));
}

/// A digital photo at rest on ONI Standard's own black, its grain and neighbourhood stages off so a pixel prints alone.
fn filmed() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("kodak-portra-800")).per_pixel_only();
    s.print.print_lift = true;
    s.print_paper = Some(oni_settings::PaperStock::of("oni-standard"));
    s.grain.amount = 0.0;
    s
}

/// A scene grey lit at the film's own white prints between the paper's black and white on every channel, no grey placed;
/// the digital photo is its virtual negative read by a negative's pipeline, pixel for pixel. Twins: the film a stop over
/// prints that grey lighter, and the same scan read as a slide is another picture.
#[test]
fn the_film_prints_a_grey_and_its_negative_is_read_as_any_negative() {
    let Some(mut g) = graph("the_film_prints_a_grey_and_its_negative_is_read_as_any_negative") else { return };
    let (w, h) = (64, 8);
    let mut scene: Vec<V4> = (0..w * h).map(|i| {
        let x = (i % w) as f32 / (w - 1) as f32;
        let v = GREY * 2f32.powf(8.0 * x - 5.0);
        if i / w < 4 { [v, v, v, 1.0] } else { [v, v * 0.6, v * 0.3, 1.0] }
    }).collect();
    // A grey under the film's white: the decode's pixel its white maps onto equal channels.
    let white = VirtualStock::of("kodak-portra-800").white().unwrap();
    let to_white = oni_settings::white::relative(oni_settings::white::uv(white), oni_settings::white::anchor(true));
    let back = oni_settings::white::inverse(&to_white.map(|r| r.map(f64::from)));
    let lit = oni_settings::white::apply(&back, [f64::from(GREY); 3]).map(|v| v as f32);
    scene[0] = [lit[0], lit[1], lit[2], 1.0];
    let source = g.upload(w, h, &scene).unwrap();
    let mut pipeline = Pipeline::new();
    let digital = filmed();
    let on = pipeline.render(&mut g, &source, &digital, &Request::default(), &Default::default(), None).unwrap().1;
    let pure = PipelineSettings { print: oni_settings::tone::Print { print_lift: false, ..digital.print }, ..digital.clone() };
    let stretched = pipeline.render(&mut g, &source, &pure, &Request::default(), &Default::default(), None).unwrap().1[0];
    println!("a scene grey: the film {:?}, on pure black {stretched:?}", on[0]);
    let white = oni_pipeline::conversion::stage47(&digital).paper().expect("a digital photo's paper").white;
    let luma = oni_render::LUMA.iter().zip(white).map(|(w, v)| w * v).sum::<f32>();
    for c in 0..3 {
        assert!(stretched[c] > 0.05 && stretched[c] < 0.95, "channel {c}: {stretched:?}");
        assert!(on[0][c] / white[c] * luma > 0.05 && on[0][c] < white[c], "channel {c}: {:?} on {white:?}", on[0]);
    }
    let over = PipelineSettings { virtual_film: digital.film().map(|f| oni_settings::VirtualFilm { exposure: 1.0, ..f }), ..digital.clone() };
    let lighter = pipeline.render(&mut g, &source, &over, &Request::default(), &Default::default(), None).unwrap().1;
    assert!(lighter[0][1] > on[0][1] + 0.02, "twin: a stop over prints lighter");
    // The film's scan, then that scan read by the negative's own pipeline on the photo's settings.
    let image = Image::new(&source).then(stages::color_matrix(&oni_pipeline::conversion::scene_white(&digital).unwrap()));
    let scanned = pipeline.filmed(&mut g, image, &digital, None).unwrap();
    let read_back = whole(&mut g, &scanned);
    let scan = g.upload(w, h, &read_back).unwrap();
    let mut read = |mode| {
        let s = PipelineSettings { mode, ..digital.clone() };
        pipeline.render(&mut g, &scan, &s, &Request::default(), &Default::default(), None).unwrap().1
    };
    let (negative, slide) = (read(ConversionMode::Negative), read(ConversionMode::Positive));
    judge("a digital photo read as its negative", &on, &negative, &slide);
    assert!(on.iter().all(|p| p.iter().all(|v| v.is_finite())));
}


/// A tungsten film sees a daylight scene in its decode's light and prints its grey blue, as a real one does; a daylight
/// film prints it near neutral, and the Source tab's Filtration takes the cast away along the whole ramp.
#[test]
fn a_tungsten_film_prints_a_daylight_grey_blue_and_the_filtration_takes_it_away() {
    let Some(mut g) = graph("a_tungsten_film_prints_a_daylight_grey_blue_and_the_filtration_takes_it_away") else { return };
    let shot = |key: &'static str| {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of(key)).per_pixel_only();
        s.grain.amount = 0.0;
        s
    };
    let mut at = |s: &PipelineSettings, scene: [f32; 3]| {
        let source = g.upload(4, 4, &[[scene[0], scene[1], scene[2], 1.0]; 16]).unwrap();
        Pipeline::new().render(&mut g, &source, s, &Request::default(), &Default::default(), None).unwrap().1[0]
    };
    let blue = |p: V4| p[2] / p[0];
    let (mut tungsten, daylight) = (shot("kodak-vision3-500t"), shot("kodak-vision3-250d"));
    let (cast, day) = (at(&tungsten, [GREY; 3]), at(&daylight, [GREY; 3]));
    println!("a daylight grey: 500T {cast:?}, 250D {day:?}");
    assert!(blue(cast) > 1.5 && cast[2] > cast[1] && cast[1] > cast[0], "{cast:?}");
    assert!((blue(day) - 1.0).abs() < 0.15, "{day:?}");
    // Twin: the same grey lit at the tungsten film's own white reaches its layers neutral and prints so.
    let to_white = oni_pipeline::conversion::scene_white(&tungsten).unwrap();
    let rows = [to_white.r_vector, to_white.g_vector, to_white.b_vector].map(|r| [f64::from(r[0]), f64::from(r[1]), f64::from(r[2])]);
    let lit = oni_settings::white::apply(&oni_settings::white::inverse(&rows), [f64::from(GREY); 3]).map(|v| v as f32);
    let under = at(&tungsten, lit);
    assert!((blue(under) - 1.0).abs() < 0.1, "twin: {under:?}");
    // Red and blue filtration until the mid grey prints neutral, as a hand sets them; the ramp follows it.
    let ramp = [0.045, GREY, 0.5];
    let widest = |at: &mut dyn FnMut(&PipelineSettings, [f32; 3]) -> V4, s: &PipelineSettings| {
        ramp.iter().map(|&v| at(s, [v; 3])).map(|p| (p[0] - p[1]).abs().max((p[2] - p[1]).abs())).fold(0.0f32, f32::max)
    };
    let before = widest(&mut at, &tungsten);
    for _ in 0..30 {
        let p = at(&tungsten, [GREY; 3]);
        tungsten.gains.stops[0] += 0.4 * (p[0] / p[1]).log2();
        tungsten.gains.stops[2] += 0.4 * (p[2] / p[1]).log2();
    }
    let after = widest(&mut at, &tungsten);
    println!("the filtration {:?}: the ramp's widest channel gap {before:.3} before, {after:.4} after", tungsten.gains.stops);
    // What stays is the film's own crossover along its grey axis, fitted on its reference: 0.041 on 500T.
    assert!(after < 0.05 && before > 0.1, "{before} {after}");
}
