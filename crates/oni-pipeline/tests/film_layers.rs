//! A film's exposure and its development as passes of their own, and its emulsion's blur between them: each against its
//! reference, the two composing the negative, the blur on the film at every resolution; each verdict with its twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::{Image, Rect};
use oni_pipeline::{Pipeline, film, halation, layers, stages};
use oni_render::cpu::{self, Grid};
use oni_render::{Gaussian, VirtualNegative};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// A digital photo on Portra 800 with no grain, no halation and no neighbourhood stage, its film's blur `microns` wide.
fn soft(microns: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).on_stock(VirtualStock::of("kodak-portra-800")).per_pixel_only();
    (s.grain.amount, s.halation.amount, s.bloom.size) = (0.0, 0.0, microns);
    s
}

#[test]
fn a_films_exposure_and_development_match_their_references_and_compose_its_negative() {
    let Some(mut g) = graph("a_films_exposure_and_development_match_their_references_and_compose_its_negative") else { return };
    let (w, h) = (37, 29);
    let mut pixels = noise(w, h, 11, 0.0, 1.5);
    // Lights past the curves' ends, a colour off the table's square and a transparent pixel.
    let edges = [[0.0, 0.0, 0.0, 1.0], [40.0, 40.0, 40.0, 1.0], [0.3, -0.05, -0.05, 1.0], [-0.05, -0.05, 0.3, 0.5], [1e-6, 0.0, 0.0, 1.0]];
    pixels[..edges.len()].copy_from_slice(&edges);
    let input = g.upload(w, h, &pixels).unwrap();
    for key in ["kodak-portra-800", "kodak-ektachrome-e100", "foma-fomapan-100"] {
        let stock = film::stock(VirtualStock::of(key));
        let (tw, th) = stock.kernel.table_size();
        let table = [Image::new(&g.upload(tw as u32, th as u32, &stock.table).unwrap())];
        let k = stock.kernel_at(0.4);
        let exposed = mapped(&pixels, |p| cpu::virtual_exposure(p, &k, &stock.table));
        let out = whole(&mut g, &Image::new(&input).then_reading(stages::virtual_exposure(&k), &table));
        // Twin: a film a hundredth faster.
        let brighter = VirtualNegative { exposure: k.exposure * 1.01, ..k.clone() };
        judge(&format!("{key}'s exposure"), &out, &exposed, &mapped(&pixels, |p| cpu::virtual_exposure(p, &brighter, &stock.table)));
        let layers = g.upload(w, h, &exposed).unwrap();
        let developed = mapped(&exposed, |e| cpu::virtual_development(e, &k, &stock.table));
        let out = whole(&mut g, &Image::new(&layers).then_reading(stages::virtual_development(&k), &table));
        // Twin: the curves a twentieth of a stop along their axis.
        let slid = VirtualNegative { log_low: k.log_low + 0.015, ..k.clone() };
        judge(&format!("{key}'s development"), &out, &developed, &mapped(&exposed, |e| cpu::virtual_development(e, &slid, &stock.table)));
        // The two compose the negative, to the bit on the reference and within the passes' own rounding on the GPU.
        let whole_negative = mapped(&pixels, |p| cpu::virtual_negative(p, &k, &stock.table));
        assert!(developed.iter().zip(&whole_negative).all(|(a, b)| a.map(f32::to_bits) == b.map(f32::to_bits)), "{key}: the reference's two steps are its one");
        let two = Image::new(&input).then_reading(stages::virtual_exposure(&k), &table).then_reading(stages::virtual_development(&k), &table);
        let out = whole(&mut g, &two);
        // Twin: the scene's own channels developed as if they were the layers' exposures.
        judge(&format!("{key}'s two passes"), &out, &whole_negative, &mapped(&pixels, |p| cpu::virtual_development(p, &k, &stock.table)));
    }
}

#[test]
fn a_films_blur_lies_on_its_layers_exposures_before_their_curve() {
    let Some(mut g) = graph("a_films_blur_lies_on_its_layers_exposures_before_their_curve") else { return };
    let side = 240u32;
    let c = side as f32 / 2.0;
    let pixels: Vec<V4> = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f32 + 0.5 - c, (i / side) as f32 + 0.5 - c);
            if x * x + y * y < 400.0 { [4.0, 3.0, 1.0, 1.0] } else { [0.02, 0.05, 0.1, 1.0] }
        })
        .collect();
    let source = g.upload(side, side, &pixels).unwrap();
    let scan = Rect::new(0, 0, side, side);
    // Two pixels of blur on this small frame, far past any film's own.
    let s = soft(300.0);
    let mut pipeline = Pipeline::new();
    let filmed = pipeline.filmed(&mut g, Image::new(&source), &s, Some(scan)).unwrap();
    let out = whole(&mut g, &filmed);
    let stock = film::stock(s.film().unwrap().stock);
    let k = stock.kernel_at(0.0);
    let sigma = layers::bloom(&s, scan) as f32;
    assert!((sigma - 2.0).abs() < 1e-6, "{sigma}");
    let blurred = |of: &[V4]| {
        let gaussian = |vertical| Gaussian { sigma, vertical, transparent: false };
        cpu::gaussian(&cpu::gaussian(&Grid::new(0, 0, side as usize, side as usize, of.to_vec()), &gaussian(false)), &gaussian(true)).pixels
    };
    let exposed = mapped(&pixels, |p| cpu::virtual_exposure(p, &k, &stock.table));
    let reference = mapped(&blurred(&exposed), |e| stock.scanned(cpu::virtual_development(e, &k, &stock.table)));
    // Twin: the blur laid on the developed negative, past its curve.
    let on_negative = mapped(&blurred(&mapped(&exposed, |e| cpu::virtual_development(e, &k, &stock.table))), |n| stock.scanned(n));
    judge("a film's blur", &out, &reference, &on_negative);

    // At rest, or finer than a pixel holds, the film stays one pass; the halation alone makes it three.
    let mut key = |s: &PipelineSettings, effects: Option<Rect>| pipeline.filmed(&mut g, Image::new(&source), s, effects).unwrap().key();
    let one = key(&soft(0.0), None);
    assert_eq!(key(&soft(0.0), Some(scan)), one, "a film at rest is one pass");
    assert_eq!(key(&soft(10.0), Some(scan)), one, "a blur under a pixel's floor lays no stage");
    assert_eq!(key(&s, None), one, "a measure reads no blur");
    assert_ne!(key(&s, Some(scan)), one, "twin: a blur is a stage");
    let haloed = PipelineSettings { halation: oni_settings::blocks::Halation { amount: 0.01, size: 125.0 }, ..soft(0.0) };
    assert!(halation::lays(&haloed) && key(&haloed, Some(scan)) != one, "twin: a halation is a stage");
}

#[test]
fn a_films_effects_keep_their_width_on_the_film_at_half_size_as_at_full() {
    let mut s = soft(3.3);
    s.halation.amount = 0.01;
    // A 24 Mpx frame decoded whole, at LibRaw's half size, and a portrait: the same 36 mm along the long side.
    let (full, half, portrait) = (Rect::new(0, 0, 6048, 4032), Rect::new(0, 0, 3024, 2016), Rect::new(0, 0, 4032, 6048));
    for width in [layers::bloom, halation::deviation] {
        let (a, b) = (width(&s, full), width(&s, half));
        assert!((a / b - 2.0).abs() < 1e-9 && width(&s, portrait) == a, "{a} {b}");
    }
    assert!((layers::bloom(&s, full) - 3.3 * 6048.0 / 36_000.0).abs() < 1e-6, "3.3 µm of a 36 mm frame, in pixels");
    assert!((halation::deviation(&s, full) - 125.0 * 6048.0 / 36_000.0).abs() < 1e-6);
    // Twin: a width in pixels whatever the scan is twice as wide on the half-size picture.
    let fixed = |scan: Rect| layers::bloom(&s, full) / f64::from(scan.w.max(scan.h));
    assert!((fixed(half) / fixed(full) - 2.0).abs() < 1e-9);
}
