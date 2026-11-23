//! Lighting on the negative in every pipe: at rest every print as before to the bit; Exposure a stop more light on the
//! film, the negative shot a stop over; on a digital photo the Film section's Exposure; colour kept as the print
//! brightens; Contrast about the grey; the filters tinting, the grey's weight held; every print finite and monotone past
//! the curves' ends; the GPU its reference. Each verdict has a twin.

mod common;
#[path = "paper/frames.rs"]
mod frames;

use common::{V4, graph, judge, mapped, noise, whole};
use frames::*;
use oni_graph::Image;
use oni_pipeline::look_lab::measure;
use oni_pipeline::{conversion, film, stages};
use oni_render::{Negative, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, LegacyMachine, LegacyPrint, PaperStock, PipelineSettings, VirtualStock};

fn onis(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { print_paper: None, ..s }
}

fn slide() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    s.gains.stops = [0.3, 0.0, -0.4];
    s
}

/// Every pipe Lighting reaches, each at Lighting's rest, some with the print's other tools moved, as a sidecar of format
/// 58 held them, read through this format's migration.
fn pipes() -> Vec<(&'static str, PipelineSettings)> {
    stored_pipes().into_iter().map(|(label, s)| (label, stored(&s))).collect()
}

fn stored_pipes() -> Vec<(&'static str, PipelineSettings)> {
    let mut uncompensated = onis(prepared_negative());
    uncompensated.print.colour_compensation = false;
    let mut legacy = onis(prepared_negative());
    legacy.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 1, c: 2, m: -1, y: 0, w: 3, laid: true });
    // At its machine's entry, as format 58 stored it.
    legacy.print.exposure = oni_settings::migrate::one_face::look_entry_exposure(legacy.mode) - oni_settings::migrate::departure::former_rest(legacy.mode);
    let mut bright = prepared_negative();
    (bright.print.brightness, bright.print.print_lift) = (0.7, false);
    let paperless = PipelineSettings { paperless: true, ..prepared_negative() };
    let frontier = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..digital() };
    vec![
        ("ONI's print, negative", onis(prepared_negative())),
        ("ONI's print, uncompensated", uncompensated),
        ("ONI's print, legacy Frontier at its entry", legacy),
        ("ONI's print, digital", onis(digital())),
        ("ONI's print, black and white", onis(mono())),
        ("ONI's print, slide", slide()),
        ("paper, negative", prepared_negative()),
        ("paper, negative, Brightness", bright),
        ("paper, digital", digital()),
        ("paper, black and white", mono()),
        ("paper, Frontier", frontier),
        ("neutral paper", paperless),
        ("paper, slide", PipelineSettings { print_paper: Some(PaperStock::default()), ..slide() }),
    ]
}

/// A scan noise and a virtual negative's scan.
fn pixels() -> Vec<V4> {
    let stock = film::stock(VirtualStock::of("kodak-portra-800"));
    let mut pixels: Vec<V4> = noise(17, 13, 7, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(noise(17, 13, 8, 0.0, 1.0));
    pixels
}

/// `s` as a sidecar of format 58 held it, read through this format's migration.
fn stored(s: &PipelineSettings) -> PipelineSettings {
    let mut value = serde_json::to_value(oni_settings::migrate::departure::stored_at(&oni_settings::document::Document::of(s.clone()), 58)).unwrap();
    value["version"] = 58.into();
    oni_settings::migrate::document(&serde_json::to_vec(&value).unwrap()).unwrap().document.settings
}

/// The bits of every pipe's print of `pixels`, folded by FNV-1a.
fn bits(twin: impl Fn(Stage47) -> Stage47) -> u64 {
    let pixels = pixels();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (_, s) in pipes() {
        let k = twin(conversion::stage47(&s));
        for v in pixels.iter().flat_map(|p| cpu::stage47(*p, &k)) {
            for b in v.to_bits().to_le_bytes() {
                hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    hash
}

#[test]
fn every_pipe_at_lightings_rest_prints_as_before_to_the_bit() {
    // The fold of every pipe's print at Lighting's rest, a legacy print at its machine's entry, on the unmixed sheets, a
    // paper's white on pure white without Print lift.
    const BEFORE: u64 = 0xb973_ba24_f517_1e4d;
    for (label, s) in pipes() {
        assert!(conversion::stage47(&s).negative.is_none(), "{label}: Lighting at rest lays nothing on the negative");
    }
    let folded = bits(|k| k);
    println!("every pipe at rest folds to {folded:#018x}");
    assert_eq!(folded, BEFORE);
    // Twin: a gain a thousandth off on red moves it.
    assert_ne!(bits(|k| Stage47 { gain: [k.gain[0] * 1.001, k.gain[1], k.gain[2]], ..k }), BEFORE);
}

/// Scene patches in linear Rec. 2020: greys over seven stops, then colours at three levels.
fn scene() -> Vec<V4> {
    let mut out: Vec<V4> = (0..15).map(|i| [0.184 * 2f32.powf(-4.0 + 0.5 * i as f32); 3]).map(|g| [g[0], g[1], g[2], 1.0]).collect();
    let hues = [[0.6, 0.15, 0.08], [0.12, 0.45, 0.1], [0.08, 0.12, 0.55], [0.5, 0.4, 0.08], [0.45, 0.12, 0.4], [0.1, 0.4, 0.45], [0.42, 0.24, 0.16]];
    for level in [0.25f32, 0.6, 1.4, 3.0] {
        out.extend(hues.iter().map(|h| [h[0] * level, h[1] * level, h[2] * level, 1.0]));
    }
    out
}

/// A scanned negative shot on `stock`'s profile, read as Prepare the Roll reads its own reference.
fn shot_on(stock: VirtualStock) -> PipelineSettings {
    let s = film::on_reference(stock, &stock.reference());
    PipelineSettings { mode: ConversionMode::Negative, ..s }
}

/// `scene` exposed `stops` over on `stock` and scanned.
fn scanned(stock: VirtualStock, stops: f32) -> Vec<V4> {
    let baked = film::stock(stock);
    let k = baked.kernel_at(stops);
    scene().iter().map(|p| cpu::virtual_negative(*p, &k, &baked.table)).collect()
}

fn lit(s: &PipelineSettings, f: impl Fn(&mut Print)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t.print);
    t
}

fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

/// Oklab of a print's linear Rec. 2020 light, clipped to the screen as the render's output is.
fn lab(v: [f32; 3]) -> [f64; 3] {
    let m = [[1.660_49, -0.587_641, -0.072_850], [-0.124_550, 1.132_9, -0.008_349], [-0.018_150, -0.100_579, 1.118_73]];
    let v = v.map(|x| f64::from(x.clamp(0.0, 1.0)));
    measure::oklab(std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum::<f64>().max(0.0)))
}

/// Oklab ΔE × 100 between two prints, the worst and the median over the patches.
fn apart(a: &[[f32; 3]], b: &[[f32; 3]]) -> (f64, f64) {
    let mut e: Vec<f64> = a.iter().zip(b).map(|(x, y)| {
        let (p, q) = (lab(*x), lab(*y));
        100.0 * ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    }).collect();
    e.sort_by(|x, y| x.total_cmp(y));
    (*e.last().unwrap(), e[e.len() / 2])
}

/// The scan pixel holding densities `d` through the frame's own gains, a slide's as its own reading takes them.
fn scan_of(s: &PipelineSettings, d: [f32; 3]) -> V4 {
    let pedestal = oni_settings::axis::density_axis::pedestal(conversion::rendered_ceiling(s));
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / s.gains.stops[c].exp2() - pedestal);
    [light[0], light[1], light[2], 1.0]
}

/// What stage 4 reads of a scan pixel: a slide on a paper, its internegative's scan.
fn as_read(s: &PipelineSettings, q: V4) -> V4 {
    match oni_pipeline::paper::internegative::copy(s) {
        Some(copy) => {
            let baked = film::stock(oni_pipeline::paper::internegative::STOCK);
            cpu::virtual_negative(cpu::color_matrix(q, &copy.light), &baked.kernel, &baked.table)
        }
        None => q,
    }
}

fn printed(pixels: &[V4], k: &Stage47) -> Vec<[f32; 3]> {
    pixels.iter().map(|p| rgb(cpu::stage47(*p, k))).collect()
}

#[test]
fn exposure_on_a_scanned_negative_is_that_negative_shot_a_stop_over() {
    for stock in [VirtualStock::of("kodak-portra-400"), VirtualStock::of("kodak-ektar-100"), VirtualStock::of("fujifilm-xtra-400")] {
        for (pipe, paper) in [("ONI's print", None), ("a paper", Some(PaperStock::default()))] {
            let s = PipelineSettings { print_paper: paper, ..shot_on(stock) };
            for stops in [-1.0f32, 1.0] {
                let over = printed(&scanned(stock, stops), &conversion::stage47(&s));
                let k = conversion::stage47(&lit(&s, |p| p.exposure += stops));
                let moved = printed(&scanned(stock, 0.0), &k);
                let (worst, median) = apart(&moved, &over);
                let (unmoved, _) = apart(&printed(&scanned(stock, 0.0), &conversion::stage47(&s)), &over);
                println!("{stock:?} on {pipe}, Exposure {stops:+}: ΔE to the negative shot {stops:+} stop, worst {worst:.2}, median {median:.2}; unmoved {unmoved:.1}");
                // The scan is read on its stock's mean layer, the shot negative developed on its three coupled ones.
                assert!(worst < 5.0 && median < 1.5, "{stock:?}, {pipe}, {stops}");
                // Twin: the enlarger's time a stop the other way, the print-side Exposure, misses the shot negative.
                if let Some(p) = k.paper {
                    let timed = Stage47 { negative: None, paper: Some(Paper { scale: p.scale * (-stops).exp2(), ..p }), ..k };
                    let (by_time, _) = apart(&printed(&scanned(stock, 0.0), &timed), &over);
                    println!("  the enlarger's time instead: worst {by_time:.2}");
                    assert!(by_time > 1.5 * worst, "{stock:?}, {stops}");
                }
                assert!(unmoved > 5.0, "{stock:?}, {pipe}: the twin, the scan unmoved");
            }
        }
    }
}

#[test]
fn on_a_digital_photo_lightings_exposure_is_the_films() {
    for stock in [VirtualStock::of("kodak-portra-800"), VirtualStock::of("kodak-portra-800-push-2"), VirtualStock::of("kodak-gold-200")] {
        for (pipe, paper) in [("ONI's print", None), ("a paper", Some(PaperStock::default()))] {
            let s = PipelineSettings { print_paper: paper, ..digital().on_stock(stock) };
            let baked = film::stock(stock);
            let shoot = |stops: f32| -> Vec<V4> { scene().iter().map(|p| cpu::virtual_negative(*p, &baked.kernel_at(stops), &baked.table)).collect() };
            let by_film = printed(&shoot(1.0), &conversion::stage47(&s));
            let by_lighting = printed(&shoot(0.0), &conversion::stage47(&lit(&s, |p| p.exposure += 1.0)));
            let (worst, median) = apart(&by_lighting, &by_film);
            println!("{stock:?} on {pipe}: Lighting's Exposure +1 from Film's +1, ΔE worst {worst:.2}, median {median:.2}");
            assert!(worst < 3.5 && median < 0.3, "{stock:?}, {pipe}");
            // Twin: half a stop on the film is another print.
            let (_, half) = apart(&printed(&shoot(0.5), &conversion::stage47(&s)), &by_film);
            assert!(half > 5.0 * median, "{stock:?}, {pipe}: {half}");
        }
    }
}

fn luma(v: [f32; 3]) -> f32 {
    0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]
}

/// The mean Oklab chroma of the colour patches printing lighter than `above`, and how many.
fn chroma(print: &[[f32; 3]], above: f64) -> (f64, usize) {
    let lit: Vec<f64> = print[15..].iter().map(|v| lab(*v)).filter(|l| l[0] > above).map(|l| l[1].hypot(l[2])).collect();
    (lit.iter().sum::<f64>() / lit.len().max(1) as f64, lit.len())
}

/// The enlarger's time, in stops, that prints `scan`'s grey as bright as `target`, from the frame's rest `k`.
fn timed_to(scan: &[V4], k: &Stage47, target: f32) -> Stage47 {
    let p = k.paper.unwrap();
    let timed = |t: f32| Stage47 { paper: Some(Paper { scale: p.scale * (-t).exp2(), ..p }), ..*k };
    let (mut low, mut high) = (-6.0f32, 6.0f32);
    for _ in 0..50 {
        let mid = 0.5 * (low + high);
        if luma(printed(scan, &timed(mid))[8]) < target { low = mid } else { high = mid }
    }
    timed(0.5 * (low + high))
}

#[test]
fn brightening_on_the_negative_keeps_the_films_colour() {
    let negative = PipelineSettings { print_paper: Some(PaperStock::default()), ..shot_on(VirtualStock::of("kodak-portra-400")) };
    let photo = PipelineSettings { print_paper: Some(PaperStock::default()), ..digital() };
    for (label, s, stock) in [("negative", negative, VirtualStock::of("kodak-portra-400")), ("digital", photo, VirtualStock::of("kodak-portra-800"))] {
        let scan = scanned(stock, 0.0);
        let rest = conversion::stage47(&s);
        for stops in [1.0f32, 2.0] {
            let on_negative = printed(&scan, &conversion::stage47(&lit(&s, |p| p.exposure += stops)));
            let shot = printed(&scanned(stock, stops), &rest);
            let by_time = printed(&scan, &timed_to(&scan, &rest, luma(on_negative[8])));
            let grey = lab(on_negative[8])[0];
            let ((kept, n), (film, _), (lost, _)) = (chroma(&on_negative, grey), chroma(&shot, grey), chroma(&by_time, grey));
            let (all, every) = (chroma(&on_negative, 0.0).0, chroma(&by_time, 0.0).0);
            let (gain, all_gain) = (100.0 * (kept / lost - 1.0), 100.0 * (all / every - 1.0));
            println!("{label}, Exposure {stops:+}: the {n} colours lighter than the grey keep chroma {kept:.4} on the negative,");
            println!("  {film:.4} shot so, {lost:.4} by the time ({gain:+.1} %); every colour {all:.4} and {every:.4} ({all_gain:+.1} %)");
            // A digital photo keeps its film's colour; a scan, read on its stock's mean layer, about the time's.
            let floor = if label == "digital" { film * 0.98 } else { lost * 0.85 };
            assert!(n >= 6 && kept > floor, "{label}, {stops}");
            // Twin: the time's print, its grey as bright, is another print.
            assert!(apart(&by_time, &on_negative).0 > 1.0, "{label}, {stops}");
        }
    }
}

#[test]
fn contrast_turns_about_the_grey_and_the_filters_tint_holding_its_weight() {
    for (label, s) in pipes() {
        let grey = as_read(&s, scan_of(&s, oni_pipeline::paper::reference_grey(&s)));
        let at_rest = rgb(cpu::stage47(grey, &conversion::stage47(&s)));
        for travel in [-0.5f32, 0.5] {
            let k = conversion::stage47(&lit(&s, |p| p.contrast = travel));
            let now = rgb(cpu::stage47(grey, &k));
            // The grey's negative unmoved; its print within the hundredth the black's anchoring lays on a paper.
            let n = k.negative.unwrap();
            let read = n.read(grey);
            let again = n.redeveloped(read);
            assert!((0..3).all(|c| (again[c] - read[c]).abs() < 2e-4), "{label}, Contrast {travel}: the grey's negative {again:?} from {read:?}");
            let off = (0..3).map(|c| (now[c] - at_rest[c]).abs() / at_rest[c].max(1e-3)).fold(0.0f32, f32::max);
            assert!(off < 1e-2, "{label}, Contrast {travel}: the grey {now:?} from {at_rest:?}");
            // Twin: the same slope about a tone a log under the grey moves the grey.
            let about_base = Stage47 { negative: k.negative.map(|n| Negative { pivot: n.pivot.map(|p| p - 1.0), ..n }), ..k };
            let twin = luma(rgb(cpu::stage47(grey, &about_base))) / luma(at_rest) - 1.0;
            assert!(twin * travel > 0.02, "{label}, {travel}: {twin}");
        }
        if s.is_monochrome() {
            continue;
        }
        let k = conversion::stage47(&lit(&s, |p| p.set_filters(Print::TEMPERATURE_AXIS)));
        let warm = rgb(cpu::stage47(grey, &k));
        let held = luma(warm) / luma(at_rest) - 1.0;
        println!("{label}, warmer: the grey {warm:?} from {at_rest:?}, its luma {:+.2} %", 100.0 * held);
        assert!(held.abs() < 0.05 && warm[0] / warm[2] > at_rest[0] / at_rest[2] * 1.1, "{label}");
        // Twin: unheld, the filters' neutral stop off, the grey's luma moves.
        let unheld = Stage47 { negative: k.negative.map(|n| Negative { shift: n.shift.map(|v| v - n.shift.iter().sum::<f32>() / 3.0 + 0.05), ..n }), ..k };
        assert!((luma(rgb(cpu::stage47(grey, &unheld))) / luma(at_rest) - 1.0).abs() > held.abs() + 0.005, "{label}");
    }
}

/// Lighting's sliders at their tracks' ends and together.
fn extremes() -> Vec<(&'static str, fn(&mut Print))> {
    vec![
        ("Exposure +4", |p| p.exposure += 4.0),
        ("Exposure -4", |p| p.exposure -= 4.0),
        ("Contrast +0.5", |p| p.contrast = 0.5),
        ("Contrast -0.5", |p| p.contrast = -0.5),
        ("Shadows +0.25, Highlights -0.25", |p| (p.shadows, p.highlights) = (0.25, -0.25)),
        ("points", |p| (p.black_point, p.white_point) = (0.2, 0.3)),
        ("warmest", |p| p.set_filters(Print::TEMPERATURE_AXIS.map(|a| a * Print::TEMPERATURE_RANGE.1))),
        ("all", |p| {
            (p.exposure, p.contrast, p.shadows, p.highlights, p.black_point) = (p.exposure + 2.0, 0.4, 0.2, -0.2, -0.1);
            p.set_filters([0.5, -0.3, 0.0]);
            p.channel_contrast = [0.3, 0.0, -0.3];
        }),
    ]
}

#[test]
fn every_print_stays_finite_and_monotone_past_the_curves_ends() {
    for (label, s) in pipes() {
        let positive = s.mode.reads_positive();
        let (bare, densest) = oni_pipeline::paper::film_ends(&s);
        for (name, f) in extremes() {
            let k = conversion::stage47(&lit(&s, f));
            let mut last = f32::NAN;
            let mut turned = 0.0f32;
            // From well under the bare film to well past the densest read: monotone from a fifth of the range either side
            // of it, finite everywhere; past it, the couplers of a shoulder read back may trade a hair between layers.
            for i in 0..=600 {
                let t = -0.6 + 2.2 * i as f32 / 600.0;
                let d: [f32; 3] = std::array::from_fn(|c| bare[c] + t * (densest[c] - bare[c]));
                let out = cpu::stage47(as_read(&s, scan_of(&s, d)), &k);
                assert!(out.iter().all(|v| v.is_finite()), "{label}, {name}: {d:?} {out:?}");
                let y = luma(rgb(out));
                if last.is_finite() && (-0.2..=1.2).contains(&t) {
                    // A negative prints lighter the denser it is, a positive darker; under a ten-thousandth of light,
                    // past the bare film, the couplers may trade a hair between layers.
                    let fall = if positive { y - last } else { last - y };
                    turned = turned.max((fall - 1e-4).max(0.0) / last.abs().max(1e-3));
                }
                last = y;
            }
            assert!(turned < 1e-3, "{label}, {name}: the print turns back by {turned:.2e}");
        }
        // Twin: a slope under none on the negative turns the print over.
        let k = conversion::stage47(&lit(&s, |p| p.contrast = 0.2));
        let over = Stage47 { negative: k.negative.map(|n| Negative { slope: [-1.0; 3], ..n }), ..k };
        let ends = [0.2f32, 0.8].map(|t| luma(rgb(cpu::stage47(as_read(&s, scan_of(&s, std::array::from_fn(|c| bare[c] + t * (densest[c] - bare[c])))), &over))));
        assert!((ends[1] < ends[0]) != positive, "{label}: {ends:?}");
    }
}

#[test]
fn the_gpu_lays_lighting_on_the_negative_as_its_reference_does() {
    let Some(mut g) = graph("the_gpu_lays_lighting_on_the_negative_as_its_reference_does") else { return };
    let pixels = pixels();
    let source = g.upload(17, 26, &pixels).unwrap();
    for (label, s) in pipes() {
        for (name, f) in extremes() {
            let k = conversion::stage47(&lit(&s, f));
            // Black and white reads no filter.
            let Some(n) = k.negative else { continue };
            // Read as the screen shows it: past its white, a toe read back four stops up spreads f32's last bits.
            let shown = |v: V4| [v[0].clamp(0.0, 1.0), v[1].clamp(0.0, 1.0), v[2].clamp(0.0, 1.0), v[3]];
            let out = mapped(&whole(&mut g, &stages::converted(Image::new(&source), &k)), shown);
            let reference = mapped(&pixels, |q| shown(cpu::stage47(q, &k)));
            let twin = Stage47 { negative: Some(Negative { shift: n.shift.map(|v| v + 0.02), ..n }), ..k };
            judge(&format!("{label}, {name}"), &out, &reference, &mapped(&pixels, |q| shown(cpu::stage47(q, &twin))));
        }
    }
}






