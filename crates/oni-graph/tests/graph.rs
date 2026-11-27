//! The graph's promises on synthetic noise: regions, cache, tiles, pool, and each kernel against its reference.

mod common;
#[path = "graph/grids.rs"]
mod grids;

use common::{LyingBlur, LyingLanczos, Shifted, TOLERANCE, cut, error, noise};
use oni_graph::cpu::{self, Plane};
use oni_graph::stages::{Axis, BoxBlur3, Crop, Gain, Lanczos};
use oni_graph::{Cache, Graph, Image, Rect, Source};

const W: u32 = 300;
const H: u32 = 200;

/// The default adapter, or `None` after a skip line on a machine that has none.
fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

fn source(g: &mut Graph, seed: u64) -> (Source, Vec<[f32; 4]>) {
    let pixels = noise(W, H, seed);
    (g.upload(W, H, &pixels).expect("upload"), pixels)
}

/// Every test stage, a crop that leaves the origin, and a Lanczos reduction that changes the grid.
fn chain(source: &Source) -> Image {
    Image::new(source)
        .then(Gain([1.3, 0.8, 1.1, 1.0]))
        .then(BoxBlur3)
        .then(Crop(Rect::new(17, 9, 251, 177)))
        .then(BoxBlur3)
        .lanczos(0.37)
}

/// Corners, edges, the interior and a single pixel: every way a region meets the extent.
fn regions(extent: Rect) -> Vec<Rect> {
    let (x, y, w, h) = (extent.x, extent.y, extent.w, extent.h);
    vec![
        Rect::new(x, y, 13, 11),
        Rect::new(x + w as i32 - 9, y + h as i32 - 7, 9, 7),
        Rect::new(x + 21, y, 30, 5),
        Rect::new(x, y + 20, 4, 25),
        Rect::new(x + 31, y + 17, 40, 29),
        Rect::new(x + 50, y + 33, 1, 1),
    ]
}

#[test]
fn a_region_equals_the_full_render_cropped() {
    let Some(mut g) = graph("a_region_equals_the_full_render_cropped") else { return };
    let (s, _) = source(&mut g, 1);
    let image = chain(&s);
    let extent = image.extent();
    assert_eq!(extent, Rect::new(0, 0, 93, 65), "251 x 0.37 and 177 x 0.37, rounded");
    let full = g.render(&image, extent, None).expect("full render");
    for region in regions(extent) {
        let part = g.render(&image, region, None).expect("region render");
        assert_eq!(part, cut(&full, extent, region), "{region:?} differs from the full render");
    }

    // Twin: a blur declaring no margin reads its region's own edge instead of its neighbours.
    let liar = Image::new(&s).then(Gain([1.3, 0.8, 1.1, 1.0])).then(LyingBlur);
    let full = g.render(&liar, liar.extent(), None).expect("full render");
    let region = Rect::new(40, 30, 50, 40);
    let part = g.render(&liar, region, None).expect("region render");
    let e = error(&part, &cut(&full, liar.extent(), region));
    assert!(e > 1e-3, "a stage under-declaring its margin went unseen: error {e}");

    // Twin: a reduction declaring two pixels less reach than its taps, behind a stage that is not the source.
    let e0 = s.extent();
    let short = Lanczos { axis: Axis::X, origin: e0.x, from: e0.w, to: 111 };
    let liar = Image::new(&s).then(Gain([1.3, 0.8, 1.1, 1.0])).then(LyingLanczos(short));
    let full = g.render(&liar, liar.extent(), None).expect("full render");
    let region = Rect::new(40, 30, 50, 40);
    let part = g.render(&liar, region, None).expect("region render");
    let e = error(&part, &cut(&full, liar.extent(), region));
    assert!(e > 1e-6, "a Lanczos under-declaring its reach went unseen: error {e}");
    let honest = Image::new(&s).then(Gain([1.3, 0.8, 1.1, 1.0])).then(short);
    let full = g.render(&honest, honest.extent(), None).expect("full render");
    assert_eq!(g.render(&honest, region, None).expect("region"), cut(&full, honest.extent(), region));
}

#[test]
fn an_auxiliary_input_is_read_where_declared_and_its_shared_prefix_once() {
    let Some(mut g) = graph("an_auxiliary_input_is_read_where_declared_and_its_shared_prefix_once") else { return };
    let (s, pixels) = source(&mut g, 11);
    let gain = [1.2, 0.9, 1.1, 1.0];
    let base = Image::new(&s).then(Gain(gain)).then(BoxBlur3);
    let image = |by: i32, honest: bool| base.clone().then_reading(Shifted { by, honest }, &[base.clone().then(BoxBlur3)]);
    let honest = image(3, true);
    let extent = honest.extent();
    let before = g.stats().dispatches;
    let full = g.render(&honest, extent, None).expect("full render");
    assert_eq!(g.stats().dispatches - before, 4, "the gain and the first blur run once for both readers");

    let plane = cpu::box_blur_3(&cpu::gain(&Plane::new(s.extent(), pixels), gain));
    let blurred = cpu::box_blur_3(&plane);
    let reference: Vec<[f32; 4]> = (0..H as i32)
        .flat_map(|y| (0..W as i32).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (a, b) = (plane.load(x, y), blurred.load(x + 3, y));
            std::array::from_fn(|c| a[c] + b[c])
        })
        .collect();
    let e = error(&full, &reference);
    assert!(e < TOLERANCE, "against the reference: error {e}");
    for region in regions(extent) {
        assert_eq!(g.render(&honest, region, None).expect("region"), cut(&full, extent, region), "{region:?}");
    }
    let mut cache = Cache::default();
    g.render(&honest, extent, Some(&mut cache)).expect("cached");
    let before = g.stats().dispatches;
    g.render(&image(4, true), extent, Some(&mut cache)).expect("late change");
    assert_eq!(g.stats().dispatches - before, 1, "a change on the reader alone reruns it alone");

    // Twins: a reader under-declaring its auxiliary reach disagrees with its own full render at a
    // region's edge, and an auxiliary chain on another gain is a second prefix the counter sees.
    let liar = image(3, false);
    let full = g.render(&liar, extent, None).expect("full render");
    let region = Rect::new(40, 30, 50, 40);
    let e = error(&g.render(&liar, region, None).expect("region"), &cut(&full, extent, region));
    assert!(e > 1e-3, "an under-declared auxiliary region went unseen: error {e}");
    let apart = base.clone().then_reading(Shifted { by: 3, honest: true }, &[Image::new(&s).then(Gain([1.3; 4])).then(BoxBlur3).then(BoxBlur3)]);
    let before = g.stats().dispatches;
    g.render(&apart, extent, None).expect("render");
    assert_eq!(g.stats().dispatches - before, 6, "two prefixes, each run");
}

#[test]
fn the_judge_sees_nan_and_infinity() {
    let reference = noise(8, 8, 9);
    assert_eq!(error(&reference, &reference), 0.0);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut broken = reference.clone();
        broken[13][1] = bad;
        assert!(error(&broken, &reference) > 1.0, "{bad} scored as a match");
        assert!(error(&reference, &broken) > 1.0, "{bad} in the reference scored as a match");
    }
}

#[test]
fn each_kernel_matches_its_reference() {
    let Some(mut g) = graph("each_kernel_matches_its_reference") else { return };
    let (s, pixels) = source(&mut g, 2);
    let plane = Plane::new(s.extent(), pixels);
    let gain = [1.3, 0.8, 1.1, 1.0];
    let crop = Rect::new(17, 9, 251, 177);
    let reference = {
        let a = cpu::box_blur_3(&cpu::gain(&plane, gain));
        let b = cpu::box_blur_3(&cpu::crop(&a, crop));
        let x = cpu::lanczos(&b, Axis::X, 93);
        cpu::lanczos(&x, Axis::Y, 65)
    };
    let image = chain(&s);
    let gpu = g.render(&image, image.extent(), None).expect("render");
    assert_eq!(reference.extent, image.extent());
    let e = error(&gpu, &reference.pixels);
    println!("chain against its reference: {e:.2e}");
    assert!(e < TOLERANCE, "chain error {e}");

    // Twins: the same judge fails a reference whose gain is off by a percent, or that skips a blur.
    let nudged = {
        let a = cpu::box_blur_3(&cpu::gain(&plane, [1.313, 0.8, 1.1, 1.0]));
        let b = cpu::box_blur_3(&cpu::crop(&a, crop));
        cpu::lanczos(&cpu::lanczos(&b, Axis::X, 93), Axis::Y, 65)
    };
    let e = error(&gpu, &nudged.pixels);
    assert!(e > TOLERANCE, "a 1 % gain error went unseen: error {e}");
    let unblurred = {
        let a = cpu::gain(&plane, gain);
        let b = cpu::box_blur_3(&cpu::crop(&a, crop));
        cpu::lanczos(&cpu::lanczos(&b, Axis::X, 93), Axis::Y, 65)
    };
    let e = error(&gpu, &unblurred.pixels);
    assert!(e > 1e-3, "a missing blur went unseen: error {e}");
    let blur = g.render(&Image::new(&s).then(BoxBlur3), s.extent(), None).expect("render");
    let e = error(&blur, &cpu::gain(&plane, [1.0; 4]).pixels);
    assert!(e > 1e-2, "a blur against the identity went unseen: error {e}");
}

#[test]
fn a_late_change_reruns_only_what_follows() {
    let Some(mut g) = graph("a_late_change_reruns_only_what_follows") else { return };
    let (s, _) = source(&mut g, 3);
    let image = |a: f32, b: f32| Image::new(&s).then(Gain([a; 4])).then(BoxBlur3).then(Gain([b; 4]));
    let extent = s.extent();
    let region = Rect::new(40, 30, 120, 90);
    let mut cache = Cache::default();
    let rendered = |g: &mut Graph, img: &Image, region: Rect, cache: &mut Cache| {
        let before = g.stats().dispatches;
        let out = g.render(img, region, Some(cache)).expect("cached render");
        let fresh = g.render(img, region, None).expect("uncached render");
        assert_eq!(out, fresh, "a cached render differs from a fresh one");
        (g.stats().dispatches - before - img.stages() as u64, out)
    };
    assert_eq!(rendered(&mut g, &image(1.0, 1.0), region, &mut cache).0, 3, "first render runs every stage");
    assert_eq!(rendered(&mut g, &image(1.0, 1.0), region, &mut cache).0, 0, "nothing changed");
    let (late, last) = rendered(&mut g, &image(1.0, 1.5), region, &mut cache);
    assert_eq!(late, 1, "the last gain alone");
    assert_eq!(rendered(&mut g, &image(1.2, 1.5), region, &mut cache).0, 3, "the first gain, so everything");
    let inner = Rect::new(60, 50, 30, 30);
    assert_eq!(rendered(&mut g, &image(1.2, 1.5), inner, &mut cache).0, 0, "a region inside the cached one");
    let wider = Rect::new(0, 0, extent.w, extent.h);
    assert_eq!(rendered(&mut g, &image(1.2, 1.5), wider, &mut cache).0, 3, "a region past the cached one");
    assert_eq!(cache.len(), 3);
    let (other, _) = source(&mut g, 33);
    let elsewhere = Image::new(&other).then(Gain([1.2; 4])).then(BoxBlur3).then(Gain([1.5; 4]));
    assert_eq!(rendered(&mut g, &elsewhere, wider, &mut cache).0, 3, "another source, same stages");

    // Twins: without a cache the counter sees all three stages, and the picture a stale cache
    // would return fails the comparison the cached renders pass.
    let before = g.stats().dispatches;
    g.render(&image(1.2, 1.5), region, None).expect("render");
    assert_eq!(g.stats().dispatches - before, 3, "the counter misses dispatches");
    let fresh = g.render(&image(1.2, 1.5), region, None).expect("render");
    assert_ne!(last, fresh, "a stale picture went unseen");
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn a_tiled_source_equals_an_untiled_one() {
    let Some(mut whole) = graph("a_tiled_source_equals_an_untiled_one") else { return };
    let mut tiled = Graph::with_max_side(64).expect("a second graph on the adapter the first found");
    let pixels = noise(W, H, 4);
    let a = whole.upload(W, H, &pixels).expect("upload");
    let b = tiled.upload(W, H, &pixels).expect("upload");
    assert_eq!((a.tiles(), b.tiles()), (1, 20), "300 x 200 in 64 px tiles is 5 x 4");
    let full = Rect::new(0, 0, W, H);
    assert_eq!(tiled.render(&Image::new(&b), full, None).expect("assembled"), pixels, "tiles laid back");
    let (ia, ib) = (chain(&a), chain(&b));
    let left = whole.render(&ia, ia.extent(), None).expect("untiled");
    let right = tiled.render(&ib, ib.extent(), None).expect("tiled");
    assert!(tiled.stats().copies > 0, "the tiles were never assembled");
    assert_eq!(left, right, "tiles show at their seams");
    let blurred = |g: &mut Graph, s: &Source| g.render(&Image::new(s).then(BoxBlur3), full, None).expect("blur");
    let reference = blurred(&mut whole, &a);
    assert_eq!(reference, blurred(&mut tiled, &b), "a blur across tile seams");

    // Twin: each tile blurred alone, as a source cut at the seams would be, shows the seams.
    let mut stitched = vec![[0.0f32; 4]; pixels.len()];
    for ty in (0..H).step_by(64) {
        for tx in (0..W).step_by(64) {
            let r = Rect::new(tx as i32, ty as i32, 64.min(W - tx), 64.min(H - ty));
            let alone = whole.upload(r.w, r.h, &cut(&pixels, full, r)).expect("tile");
            let out = whole.render(&Image::new(&alone).then(BoxBlur3), alone.extent(), None).expect("tile blur");
            for y in 0..r.h as usize {
                let at = (r.y as usize + y) * W as usize + r.x as usize;
                stitched[at..at + r.w as usize].copy_from_slice(&out[y * r.w as usize..(y + 1) * r.w as usize]);
            }
        }
    }
    let e = error(&stitched, &reference);
    assert!(e > 1e-3, "seams cut between tiles went unseen: error {e}");
}

#[test]
fn a_materialised_image_renders_as_its_recipe() {
    let Some(mut g) = graph("a_materialised_image_renders_as_its_recipe") else { return };
    let (s, _) = source(&mut g, 6);
    let image = chain(&s);
    let recipe = g.render(&image, image.extent(), None).expect("recipe");
    let held = g.materialise(&image).expect("materialise");
    assert_eq!(held.extent(), image.extent());
    let before = g.stats().dispatches;
    let pixels = g.render(&Image::new(&held), held.extent(), None).expect("held");
    assert_eq!(g.stats().dispatches, before, "a held source runs no stage");
    assert_eq!(pixels, recipe);

    // Past the texture side, the image is rendered in pieces and held in tiles.
    let mut tiled = Graph::with_max_side(64).expect("a second graph on the adapter the first found");
    let t = tiled.upload(W, H, &noise(W, H, 6)).expect("upload");
    let whole = tiled.materialise(&chain(&t)).expect("materialise past the texture side");
    assert!(whole.tiles() > 1, "a materialised image past the side is tiled");
    assert_eq!(tiled.render(&Image::new(&whole), whole.extent(), None).expect("held"), recipe, "tiled and held");

    // Twin: materialised one stage short, it no longer matches.
    let short = Image::new(&t).then(Gain([1.3, 0.8, 1.1, 1.0])).then(BoxBlur3).then(Crop(Rect::new(17, 9, 251, 177))).lanczos(0.37);
    let held = tiled.materialise(&short).expect("materialise past the texture side");
    let pixels = tiled.render(&Image::new(&held), held.extent(), None).expect("held");
    assert_eq!(pixels.len(), recipe.len());
    assert!(error(&pixels, &recipe) > 1e-3, "a missing stage went unseen");
}

#[test]
fn a_request_outside_the_extent_is_refused() {
    let Some(mut g) = graph("a_request_outside_the_extent_is_refused") else { return };
    let (s, _) = source(&mut g, 7);
    let image = Image::new(&s).then(Crop(Rect::new(10, 10, 50, 50)));
    assert!(g.render(&image, Rect::new(10, 10, 50, 50), None).is_ok());
    assert!(g.render(&image, Rect::new(0, 0, 50, 50), None).is_err(), "a region past the crop");
    assert!(g.render(&image, Rect::new(10, 10, 0, 5), None).is_err(), "an empty region");
}
