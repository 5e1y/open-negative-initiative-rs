//! Each photo's grain through the chain: its seed rearranges the grain and keeps its character, a scan
//! pixel keeps its dissolved grain under a region and a crop, and a print notch reruns neither the grain
//! nor its blur; each verdict with a twin the check refuses.

mod common;

use common::{V4, graph, noise, whole};
use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request, grain};
use oni_settings::{ConversionMode, PipelineSettings};

/// A coarse grain over the whole frame, its blur well past its floor on a scan 1500 pixels wide.
fn coarse() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, false).per_pixel_only();
    (s.grain.amount, s.grain.size) = (0.8, 1.0);
    s
}

const SEEDS: [[u32; 2]; 2] = [[0x2545_f491, 0x9e37_79b9], [7, 0xdead_beef]];

/// Each pixel's light in stops over its own, the grain's alone on a flat grey.
fn stops(out: &[V4]) -> Vec<f64> {
    out.iter().map(|p| f64::from(p[1] / 0.18).log2()).collect()
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let (ma, mb) = (mean(a), mean(b));
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    cov / (a.iter().map(|x| (x - ma).powi(2)).sum::<f64>() * b.iter().map(|y| (y - mb).powi(2)).sum::<f64>()).sqrt()
}

fn spread(v: &[f64]) -> f64 {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
}

#[test]
fn a_photo_s_seed_rearranges_its_grain_and_keeps_its_character() {
    let Some(mut g) = graph("a_photo_s_seed_rearranges_its_grain_and_keeps_its_character") else { return };
    let (w, h) = (600u32, 400u32);
    let source = g.upload(w, h, &vec![[0.18, 0.18, 0.18, 1.0]; (w * h) as usize]).unwrap();
    let mut pipeline = Pipeline::new();
    let s = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.6, ..coarse().grain }, ..coarse() };
    let mut laid = |g: &mut Graph, seed: [u32; 2]| {
        let image = pipeline.image(g, &source, &s, &Request::default(), &Inputs { seed, ..Inputs::default() }).unwrap();
        // The print bends every seed's grain alike, so its output compares their arrangements.
        stops(&whole(g, &image))
    };
    let (none, a, b) = (laid(&mut g, [0, 0]), laid(&mut g, SEEDS[0]), laid(&mut g, SEEDS[1]));
    for (x, y, name) in [(&none, &a, "none and the first"), (&a, &b, "the first and the second")] {
        let c = correlation(x, y);
        println!("{name}: correlation {c:.4}, spreads {:.4} and {:.4}", spread(x), spread(y));
        assert!(c.abs() < 0.05, "{name} correlate at {c}");
        assert!((spread(x) / spread(y) - 1.0).abs() < 0.05, "{name}: the grain's strength moves");
    }
    // Twin: one seed again lays one arrangement.
    assert!(correlation(&a, &laid(&mut g, SEEDS[0])) > 0.999);
}

#[test]
fn a_scan_pixel_keeps_its_dissolved_grain_under_a_region_and_a_crop() {
    let Some(mut g) = graph("a_scan_pixel_keeps_its_dissolved_grain_under_a_region_and_a_crop") else { return };
    let (w, h) = (1500u32, 1000u32);
    let source = g.upload(w, h, &noise(w, h, 3, 0.05, 0.6)).unwrap();
    let s = coarse();
    assert!(grain::dissolve(&s, Rect::new(0, 0, w, h)) > 0.3, "the blur acts");
    let mut pipeline = Pipeline::new();
    let inputs = Inputs { seed: SEEDS[0], ..Inputs::default() };
    let mut render = |g: &mut Graph, s: &PipelineSettings, region: Option<Rect>| -> (Rect, Vec<V4>) {
        let image = pipeline.image(g, &source, s, &Request::default(), &inputs).unwrap();
        let region = region.unwrap_or(image.extent());
        (region, g.render(&image, region, None).unwrap())
    };
    let (_, all) = render(&mut g, &s, None);
    let at = |x: i32, y: i32| all[(y as u32 * w + x as u32) as usize];
    let cut = |r: Rect| -> Vec<V4> { (r.y..r.bottom()).flat_map(|y| (r.x..r.right()).map(move |x| (x, y))).map(|(x, y)| at(x, y)).collect() };
    let region = Rect::new(301, 207, 128, 96);
    let (_, part) = render(&mut g, &s, Some(region));
    assert_eq!(part, cut(region), "a region's dissolved grain is the whole's");
    let mut cropped = s.clone();
    cropped.geometry.crop = oni_settings::cg::Rect::new(0.2, 0.25, 0.5, 0.5);
    let (frame, inside) = render(&mut g, &cropped, None);
    // A crop's edge repeats its own pixels into the blur: inside its reach the two renders may part.
    let reach = Rect::new(frame.x + 8, frame.y + 8, frame.w - 16, frame.h - 16);
    let kept: Vec<V4> = (reach.y..reach.bottom()).flat_map(|y| (reach.x..reach.right()).map(move |x| ((y - frame.y) as u32 * frame.w + (x - frame.x) as u32) as usize)).map(|i| inside[i]).collect();
    assert_eq!(kept, cut(reach), "a crop moves the dissolved grain");
    // Twin: a region a pixel over.
    assert_ne!(part, cut(Rect::new(302, 207, 128, 96)));
}

#[test]
fn a_print_notch_reruns_neither_the_grain_nor_its_blur() {
    let Some(mut g) = graph("a_print_notch_reruns_neither_the_grain_nor_its_blur") else { return };
    let (w, h) = (1200u32, 800u32);
    let source = g.upload(w, h, &noise(w, h, 9, 0.05, 0.9)).unwrap();
    let mut pipeline = Pipeline::new();
    let rest = PipelineSettings::rest(ConversionMode::Digital, false);
    let grained = PipelineSettings { grain: coarse().grain, ..rest.clone() };
    let mut image = |g: &mut Graph, s: &PipelineSettings| pipeline.image(g, &source, s, &Request::default(), &Default::default()).unwrap();
    assert_eq!(image(&mut g, &grained).stages(), image(&mut g, &rest).stages() + 1, "the grain is one pass of the chain, its blur read beside it");
    let mut notches = |g: &mut Graph, s: &PipelineSettings, notch: &dyn Fn(&mut PipelineSettings, f32)| -> (u64, u64) {
        let mut cache = oni_graph::Cache::frontier();
        let mut ran = (0, 0);
        for step in 0..3 {
            let mut t = s.clone();
            notch(&mut t, step as f32 * 0.01);
            let image = image(g, &t);
            let before = g.stats();
            g.materialise_cached(&image, image.extent(), Some(&mut cache)).unwrap();
            let after = g.stats();
            ran = (after.dispatches - before.dispatches, after.pixels - before.pixels);
        }
        ran
    };
    let exposure = |t: &mut PipelineSettings, d: f32| t.print.exposure += d;
    let plain = notches(&mut g, &rest, &exposure);
    assert_eq!(notches(&mut g, &grained, &exposure), plain, "an exposure notch reruns the grain or its blur");
    // An amount notch reruns the grain's pass, the film's that reads it and the dye separation's that reads the
    // film: the blur reads only Size. Twin: a size notch blurs again.
    let amount = |t: &mut PipelineSettings, d: f32| t.grain.amount -= d;
    assert_eq!(notches(&mut g, &grained, &amount).0, plain.0 + 3, "an amount notch blurs again");
    let size = |t: &mut PipelineSettings, d: f32| t.grain.size -= d;
    assert_eq!(notches(&mut g, &grained, &size).0, plain.0 + 5, "the twin reruns {:?}", notches(&mut g, &grained, &size));
}
