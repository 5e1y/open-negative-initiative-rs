//! A digital photo shot on a reversal film and projected: the GPU's projection against its scalar reference, the bare
//! film projected as the screen's white, a slide's grey held on its sheet through its couplers, and the editor's chain
//! laying the slide on the screen, never on a paper. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_pipeline::film::{self, GREY, bake::COUPLERS, projection};
use oni_pipeline::{Pipeline, Request, conversion, projector, stages};
use oni_render::{Projection, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

const SLIDE: VirtualStock = VirtualStock::of("fujifilm-velvia-50");

#[test]
fn the_gpu_projects_a_slide_as_its_reference_does() {
    let p = projection::projected(SLIDE);
    // The bare film is the screen's white, the veil over it; a dense slide is the veil alone.
    let white = cpu::projection([1.0; 4], &p.projection, &p.table);
    assert!((0..3).all(|c| (white[c] - 1.0 - projection::FLARE).abs() < 1e-3), "{white:?}");
    let black = cpu::projection([1e-4, 1e-4, 1e-4, 1.0], &p.projection, &p.table);
    assert!((0..3).all(|c| black[c] < 2.0 * projection::FLARE), "{black:?}");
    let Some(mut g) = graph("the_gpu_projects_a_slide_as_its_reference_does") else { return };
    let (w, h) = (31, 13);
    let pixels: Vec<V4> = noise(w, h, 9, 0.0, 1.0).iter().map(|q| q.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let table = g.upload(p.projection.bands as u32, 2, &p.table).unwrap();
    let out = whole(&mut g, &oni_graph::Image::new(&source).then_reading(stages::projection(&p.projection), &[oni_graph::Image::new(&table)]));
    let reference = mapped(&pixels, |q| cpu::projection(q, &p.projection, &p.table));
    // Twin: the room's veil left out.
    let bare = Projection { flare: 0.0, ..p.projection.clone() };
    judge("a slide projected", &out, &reference, &mapped(&pixels, |q| cpu::projection(q, &bare, &p.table)));
}

#[test]
fn a_slides_couplers_move_its_colours_and_leave_its_grey_on_the_sheet() {
    let entry = SLIDE.entry();
    assert!(entry.reversal && !VirtualStock::of("kodak-portra-800").entry().reversal);
    // A quarter of a negative's common matrix: a slide's curves are steep enough that the whole one folds its axis back.
    let quarter = COUPLERS.map(|r| r.map(|v| v / 4.0));
    let (plain, coupled) = (film::bake_through(&entry, [[0.0; 3]; 3]).unwrap(), film::bake_through(&entry, quarter).unwrap());
    let pass = |s: &film::Stock, p: V4| cpu::virtual_negative(p, &s.kernel_at(0.0), &s.table);
    // A slide falls with exposure: a brighter grey passes more light.
    let grey = [GREY, GREY, GREY, 1.0];
    assert!(pass(&plain, [GREY * 4.0, GREY * 4.0, GREY * 4.0, 1.0])[1] > 2.0 * pass(&plain, grey)[1]);
    let apart = |p: V4| (0..3).map(|c| (pass(&coupled, p)[c] / pass(&plain, p)[c] - 1.0).abs()).fold(0.0f32, f32::max);
    println!("grey {:.2e}, red {:.2e}", apart(grey), apart([0.4, 0.08, 0.05, 1.0]));
    assert!(apart(grey) < 2e-2, "the grey moved {}", apart(grey));
    // Twin: a strong colour moves.
    assert!(apart([0.4, 0.08, 0.05, 1.0]) > 5e-2);
}

#[test]
fn a_photo_on_a_slide_film_is_projected_on_the_screen_and_never_printed() {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(SLIDE);
    assert!(s.print_paper.is_none() && s.projector() == Some(SLIDE), "a slide is projected, not printed");
    // A negative taken after it prints on the default paper again.
    assert!(s.on_stock(VirtualStock::of("kodak-portra-400")).print_paper.is_some());
    // A paper named on it is never printed on: the slide takes no paper.
    let named = PipelineSettings { print_paper: Some(Default::default()), ..s.clone() };
    assert!(named.paper().is_none() && !named.takes_paper() && conversion::stage47(&named).paper.is_none());
    let Some(mut g) = graph("a_photo_on_a_slide_film_is_projected_on_the_screen_and_never_printed") else { return };
    let (w, h) = (23, 11);
    let pixels: Vec<V4> = noise(w, h, 4, 0.0, 1.2).iter().map(|q| q.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let mut s = s.per_pixel_only();
    s.grain.amount = 0.0;
    let mut pipeline = Pipeline::new();
    let render = |p: &mut Pipeline, g: &mut oni_graph::Graph, s: &PipelineSettings| p.render(g, &source, s, &Request::default(), &Default::default(), None).unwrap().1;
    let out = render(&mut pipeline, &mut g, &s);
    let stock = film::stock(SLIDE);
    let light = conversion::scene_white(&s).unwrap();
    let developed = |q: V4| cpu::virtual_negative(cpu::color_matrix(q, &light), &stock.kernel_at(0.0), &stock.table);
    let clipped = |q: V4| q.map(|v| v.clamp(0.0, 1.0));
    let expected = mapped(&pixels, |q| clipped(projector::shown(developed(q), &s, SLIDE)));
    // Twin: the scene unlit at the film's daylight.
    judge("a photo on a slide, projected", &out, &expected, &mapped(&pixels, |q| clipped(projector::shown(cpu::virtual_negative(q, &stock.kernel_at(0.0), &stock.table), &s, SLIDE))));
    // Projected blacks keep the screen's own ends, the projection as it is.
    let veiled = PipelineSettings { print: oni_settings::tone::Print { projected_blacks: true, ..s.print }, ..s.clone() };
    let p = projection::projected(SLIDE);
    let shown = mapped(&pixels, |q| clipped(cpu::projection(developed(q), &p.projection, &p.table)));
    judge("a photo on a slide, Projected blacks", &render(&mut pipeline, &mut g, &veiled), &shown, &expected);
}

const PROVIA: VirtualStock = VirtualStock::of("fujifilm-provia-100f");

/// Where Provia 100F's sheet stops tracing green and blue, in log H over its first point (-3.38): their rest lies under
/// red's stroke, coincident within line width.
const TRACED: [(usize, f64); 2] = [(1, -1.50 + 3.38), (2, -2.16 + 3.38)];

/// CIELAB of a screen's linear Rec. 2020 light, its white D65's.
fn cielab(v: V4) -> [f32; 3] {
    const XYZ: [[f32; 3]; 3] = [[0.636_958, 0.144_617, 0.168_881], [0.262_700, 0.677_998, 0.059_302], [0.0, 0.028_073, 1.060_985]];
    const WHITE: [f32; 3] = [0.950_47, 1.0, 1.088_83];
    let f = |t: f32| if t > (6.0f32 / 29.0).powi(3) { t.cbrt() } else { t / (3.0 * (6.0f32 / 29.0).powi(2)) + 4.0 / 29.0 };
    let [x, y, z]: [f32; 3] = std::array::from_fn(|r| f((0..3).map(|c| XYZ[r][c] * v[c].max(0.0)).sum::<f32>() / WHITE[r]));
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}

#[test]
fn provias_profile_reads_falls_with_exposure_and_projects_its_bare_film_as_the_screens_white() {
    let entry = PROVIA.entry();
    assert!(PROVIA.is_known() && entry.reversal && entry.offered() && VirtualStock::all().contains(&PROVIA));
    let profile = film::profile::Profile::read(entry.profile.as_bytes()).unwrap();
    let stock = film::stock(PROVIA);
    let pass = |v: f32| cpu::virtual_negative([v, v, v, 1.0], &stock.kernel_at(0.0), &stock.table);
    let (grey, brighter) = (pass(GREY), pass(GREY * 4.0));
    assert!((0..3).all(|c| brighter[c] > 2.0 * grey[c]), "{grey:?} {brighter:?}");
    let p = projection::projected(PROVIA);
    let white = cpu::projection([1.0; 4], &p.projection, &p.table);
    assert!((0..3).all(|c| (white[c] - 1.0 - projection::FLARE).abs() < 1e-3), "{white:?}");
    for slide in [PROVIA, VirtualStock::of("fujifilm-velvia-100"), VirtualStock::SLIDE] {
        let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(slide);
        let light = conversion::scene_white(&s).unwrap();
        let st = film::stock(slide);
        let shown = projector::shown(cpu::virtual_negative(cpu::color_matrix([GREY, GREY, GREY, 1.0], &light), &st.kernel_at(0.0), &st.table), &s, slide);
        println!("{}: a scene's 18 % grey projected at L*a*b* {:?}", slide.key(), cielab(shown));
    }
    // The sheet's green passes an 18 % grey inside the range the filled curves span, over four log H.
    let first = profile.log_exposure[0];
    let span = profile.log_exposure.last().unwrap() - first;
    let least = |c: usize, to: f64| profile.log_exposure.iter().zip(&profile.sheet_density_curves).filter(|(le, _)| **le <= to).map(|(_, d)| d[c]).fold(f64::INFINITY, f64::min);
    let grey_density = -(0.18f64).log10();
    assert!(span > 4.0 && least(1, f64::INFINITY) < grey_density, "{span} {}", least(1, f64::INFINITY));
    // Twin: green and blue as the sheet traces them, unfilled, end before any 18 % grey, too short a range to lay it.
    for (c, traced) in TRACED {
        assert!(least(c, first + traced) > grey_density, "channel {c}: {}", least(c, first + traced));
    }
}
