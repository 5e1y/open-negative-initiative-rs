//! The lens warp: on the GPU against its scalar reference through every branch, its region exact so a
//! region renders the whole's cut, the corrected picture filling its frame, and every render without
//! a lens left on the turns' and the angle's own stages.

mod common;

use common::{V4, graph, judge, noise, whole};
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::stages::Warped;
use oni_pipeline::{Inputs, Pipeline, Request, geometry};
use oni_render::Warp;
use oni_render::cpu::{self, Grid};
use oni_settings::cg;
use oni_settings::frame::Geometry;
use oni_settings::lens::{LensCorrection, Radial};
use oni_settings::{ConversionMode, PipelineSettings};

fn lens(distortion: f32, red: f32, blue: f32) -> LensCorrection {
    LensCorrection { distortion, red, blue, vignetting: 0.0 }
}

fn out_of(r: Rect) -> (i32, i32, usize, usize) {
    (r.x, r.y, r.w as usize, r.h as usize)
}

#[test]
fn the_warp_matches_its_reference_in_every_branch() {
    let Some(mut g) = graph("the_warp_matches_its_reference_in_every_branch") else { return };
    let input = Rect::new(0, 0, 61, 41);
    let pixels = noise(input.w, input.h, 21, 0.0, 1.0);
    let source = g.upload_at(input, &pixels).unwrap();
    let grid = Grid::new(0, 0, input.w as usize, input.h as usize, pixels);
    let size = (f64::from(input.w), f64::from(input.h));
    let mirrored = Geometry { quarter_turns: 1, mirrored: true, ..Geometry::default() };
    let tilted = Geometry { angle: 7.0, ..Geometry::default() };
    let cases = [
        ("barrel corrected", Geometry::default(), lens(80.0, 0.0, 0.0)),
        ("pincushion corrected", Geometry::default(), lens(-90.0, 0.0, 0.0)),
        ("fringes alone", Geometry::default(), lens(0.0, 70.0, -60.0)),
        ("both", Geometry::default(), lens(-50.0, -100.0, 100.0)),
        ("a turn with a mirror", mirrored, lens(40.0, 50.0, 0.0)),
        ("seven degrees, transparent", tilted, lens(-60.0, 0.0, 80.0)),
    ];
    for (name, geometry, correction) in cases {
        let radial = correction.radial(size).unwrap();
        let w = geometry::warped(&geometry, &radial, input, 1.0);
        assert_eq!(w.k.transparent, geometry.angle != 0.0, "{name}");
        assert_eq!(w.k.fringed(), correction.red != 0.0 || correction.blue != 0.0, "{name}");
        let out = whole(&mut g, &Image::new(&source).then(w));
        let reference = cpu::warp(&grid, out_of(w.extent), &w.k).pixels;
        // Twin: red and blue read at green's place, or with no fringe a slightly other distortion.
        let twin = if w.k.fringed() {
            Warp { coefficients: [w.k.coefficients[1]; 3], ..w.k }
        } else {
            Warp { coefficients: w.k.coefficients.map(|c| [c[0], c[1] * 1.05, c[2], c[3]]), ..w.k }
        };
        judge(name, &out, &reference, &cpu::warp(&grid, out_of(w.extent), &twin).pixels);
    }
    // A region on the tilted frame's edge, reading partly past the input, read alone.
    let w = geometry::warped(&tilted, &lens(-60.0, 30.0, -30.0).radial(size).unwrap(), input, 1.0);
    let corner = Rect::new(w.extent.x, w.extent.y + w.extent.h as i32 / 2 - 6, 14, 12);
    let part = g.render(&Image::new(&source).then(w), corner, None).unwrap();
    let transparent = cpu::warp(&grid, out_of(corner), &w.k).pixels;
    let held = cpu::warp(&grid, out_of(corner), &Warp { transparent: false, ..w.k }).pixels;
    judge("a region straddling the extent", &part, &transparent, &held);
}

/// Every input pixel a bilinear tap of `w` reads for `region`, on the scalar reference's arithmetic.
fn taps(w: &Warp, region: Rect) -> Rect {
    let mut reached = Rect::new(0, 0, 0, 0);
    for y in region.y..region.bottom() {
        for x in region.x..region.right() {
            let c = [x as f32 + 0.5, y as f32 + 0.5];
            let s = [w.t[0][0] * c[0] + w.t[0][1] * c[1] + w.t[0][2], w.t[1][0] * c[0] + w.t[1][1] * c[1] + w.t[1][2]];
            let d = [s[0] - w.centre[0], s[1] - w.centre[1]];
            let u = (d[0] * d[0] + d[1] * d[1]) * w.inv_unit2;
            for channel in 0..3 {
                let f = w.factor(channel, u);
                let a = [w.centre[0] + f * d[0] - 0.5, w.centre[1] + f * d[1] - 0.5];
                let q = Rect::new(a[0].floor() as i32, a[1].floor() as i32, 2, 2);
                reached = if reached.is_empty() { q } else { reached.union(&q) };
            }
        }
    }
    reached
}

/// The bounding box of `region`'s corners mapped through `t` alone, outset by a pixel.
fn affine_only(t: &[[f32; 3]; 2], region: Rect) -> Rect {
    let corners = [(region.x, region.y), (region.right(), region.y), (region.x, region.bottom()), (region.right(), region.bottom())];
    let points = corners.map(|(x, y)| (t[0][0] * x as f32 + t[0][1] * y as f32 + t[0][2], t[1][0] * x as f32 + t[1][1] * y as f32 + t[1][2]));
    let (x0, x1) = points.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = points.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    Rect::from_corners(x0.floor() as i32 - 1, y0.floor() as i32 - 1, x1.ceil() as i32 + 1, y1.ceil() as i32 + 1)
}

#[test]
fn the_warps_region_holds_every_tap_its_pixels_read() {
    let input = Rect::new(0, 0, 1600, 1066);
    let size = (1600.0, 1066.0);
    for (geometry, correction) in [
        (Geometry { angle: 3.0, ..Geometry::default() }, lens(-80.0, 60.0, -60.0)),
        (Geometry { quarter_turns: 3, ..Geometry::default() }, lens(100.0, -100.0, 100.0)),
        (Geometry::default(), lens(-100.0, 0.0, 0.0)),
    ] {
        let w = geometry::warped(&geometry, &correction.radial(size).unwrap(), input, 1.0);
        let e = w.extent;
        let regions = [Rect::new(e.x, e.y, 90, 70), Rect::new(e.right() - 90, e.bottom() - 70, 90, 70), Rect::new(e.x + e.w as i32 / 2 - 40, e.y + 300, 80, 60)];
        for region in regions {
            let needed = taps(&w.k, region);
            assert!(w.input_region(region).contains(&needed), "{correction:?} {region:?}: {:?} misses {needed:?}", w.input_region(region));
        }
    }
    // Twin: at a pincushion correction's corner, the turns' map alone misses what the taps read.
    let w = geometry::warped(&Geometry::default(), &lens(-100.0, 0.0, 0.0).radial(size).unwrap(), input, 1.0);
    let corner = Rect::new(1500, 966, 100, 100);
    assert!(!affine_only(&w.k.t, corner).contains(&taps(&w.k, corner)), "the turns' map alone holds the corner's taps");
}

/// The digital family the region check reads: distortion, opposite fringes and an angle.
fn lensed() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.lens_correction = LensCorrection { distortion: -80.0, red: 60.0, blue: -60.0, vignetting: 50.0 };
    s.geometry = Geometry { angle: 3.0, crop: cg::Rect::new(0.05, 0.1, 0.85, 0.8), ..Geometry::default() };
    s
}

#[test]
fn a_region_of_a_lens_corrected_frame_is_the_cut_of_the_whole_to_the_bit() {
    let Some(mut g) = graph("a_region_of_a_lens_corrected_frame_is_the_cut_of_the_whole_to_the_bit") else { return };
    let (w, h) = (1600, 1066);
    let source = g.upload(w, h, &noise(w, h, 31, 0.02, 1.0)).unwrap();
    let mut pipeline = Pipeline::new();
    let request = Request { scale: 0.5, full_width: Some(f64::from(w)), ..Request::default() };
    let image = pipeline.image(&mut g, &source, &lensed(), &request, &Default::default()).unwrap();
    let e = image.extent();
    let all = whole(&mut g, &image);
    let cut = |r: Rect| -> Vec<V4> {
        (r.y..r.bottom()).flat_map(|y| (r.x..r.right()).map(move |x| ((y - e.y) as u32 * e.w + (x - e.x) as u32) as usize)).map(|i| all[i]).collect()
    };
    let (rw, rh) = (e.w / 3, e.h / 3);
    let regions = [
        Rect::new(e.x, e.y, rw, rh),
        Rect::new(e.right() - rw as i32, e.y, rw, rh),
        Rect::new(e.x, e.bottom() - rh as i32, rw, rh),
        Rect::new(e.x + 37, e.y + 29, e.w / 2, e.h / 2),
    ];
    for region in regions {
        let part = g.render(&image, region, None).unwrap();
        assert!(part == cut(region), "{region:?}: the region is not the whole's cut");
        // Twin: the cut a pixel over is another picture.
        assert!(part != cut(Rect::new(region.x + if region.x > e.x { -1 } else { 1 }, region.y, region.w, region.h)));
    }
}

/// `w` read transparent past its input, so a gap inside the frame shows as a clear pixel.
fn clear_pixels(g: &mut Graph, source: &Source, w: Warped) -> usize {
    let clear = Warped { k: Warp { transparent: true, ..w.k }, ..w };
    whole(g, &Image::new(source).then(clear)).iter().filter(|p| p[3] < 1.0).count()
}

#[test]
fn a_corrected_picture_fills_its_frame_and_a_barrel_correction_never_zooms() {
    let Some(mut g) = graph("a_corrected_picture_fills_its_frame_and_a_barrel_correction_never_zooms") else { return };
    let input = Rect::new(0, 0, 600, 400);
    let source = g.upload_at(input, &noise(600, 400, 4, 0.1, 0.9)).unwrap();
    let size = (600.0, 400.0);
    for correction in [lens(-100.0, 100.0, -100.0), lens(-100.0, -100.0, 100.0), lens(0.0, -100.0, -100.0), lens(100.0, -100.0, -100.0)] {
        let radial = correction.radial(size).unwrap();
        let w = geometry::warped(&Geometry::default(), &radial, input, 1.0);
        assert_eq!(clear_pixels(&mut g, &source, w), 0, "{correction:?}: the frame has gaps");
    }
    // A barrel correction keeps the frame's own scale.
    assert_eq!(lens(100.0, 0.0, 0.0).radial(size).unwrap().coefficients[1][0], 1.0);
    // Twin: the pincushion correction unzoomed leaves its corners clear.
    let zoomed = lens(-100.0, 100.0, -100.0).radial(size).unwrap();
    let unzoomed = Radial { coefficients: zoomed.coefficients.map(|k| [k[0] / zoomed.coefficients[1][0], 0.2 * k[0] / zoomed.coefficients[1][0], 0.0, 0.0]), ..zoomed };
    let w = geometry::warped(&Geometry::default(), &unzoomed, input, 1.0);
    assert!(clear_pixels(&mut g, &source, w) > 100, "the unzoomed correction fills the frame");
}

#[test]
fn without_a_lens_every_render_keeps_its_own_stages_and_pixels() {
    let Some(mut g) = graph("without_a_lens_every_render_keeps_its_own_stages_and_pixels") else { return };
    let (w, h) = (300, 200);
    let source = g.upload(w, h, &noise(w, h, 9, 0.05, 0.9)).unwrap();
    let mut pipeline = Pipeline::new();
    let mut image = |s: &PipelineSettings| pipeline.image(&mut g, &source, s, &Request::default(), &Inputs::default()).unwrap();
    let stored = LensCorrection { distortion: 30.0, red: 20.0, blue: -20.0, vignetting: 60.0 };
    let framing = Geometry { quarter_turns: 1, angle: 2.0, ..Geometry::default() };
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome] {
        let rest = PipelineSettings { geometry: framing, ..PipelineSettings::rest(mode, true) };
        let held = PipelineSettings { lens_correction: stored, ..rest.clone() };
        let (a, b) = (image(&rest), image(&held));
        assert_eq!((a.key(), a.stages()), (b.key(), b.stages()), "{mode:?}: a stored lens reached the render");
    }
    // Digital at rest, turned and tilted: the turns' and the angle's stages, as before the lens.
    let rest = PipelineSettings { geometry: framing, ..PipelineSettings::rest(ConversionMode::Digital, true) };
    let bare = Image::new(&source);
    let framed = pipeline.framed(&mut g, bare.clone(), &rest, 1.0, &Inputs::default()).unwrap();
    assert_eq!(framed.key(), geometry::apply(bare.clone(), &framing, None, 1.0).key());
    // Twin: Distortion 30 lays the warp in their place, one stage for two.
    let warped = PipelineSettings { lens_correction: lens(30.0, 0.0, 0.0), ..rest.clone() };
    let radial = warped.lens((300.0, 200.0)).unwrap();
    let framed = pipeline.framed(&mut g, bare.clone(), &warped, 1.0, &Inputs::default()).unwrap();
    assert_eq!(framed.key(), geometry::apply(bare.clone(), &framing, Some(&radial), 1.0).key());
    assert_eq!(framed.stages() + 1, geometry::apply(bare.clone(), &framing, None, 1.0).stages());
    // Vignetting alone lays its own stage on the scan, and no warp.
    let vignetted = PipelineSettings { lens_correction: LensCorrection { vignetting: 50.0, ..Default::default() }, ..rest.clone() };
    let healed = pipeline.healed(&mut g, bare.clone(), &vignetted, 1.0, &Inputs::default()).unwrap();
    assert_eq!(healed.stages(), 1);
    let framed = pipeline.framed(&mut g, bare.clone(), &vignetted, 1.0, &Inputs::default()).unwrap();
    assert_eq!(framed.stages(), 1 + geometry::apply(bare, &framing, None, 1.0).stages());
}
