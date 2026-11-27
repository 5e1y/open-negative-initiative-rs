//! The graph's promises past one render on one grid: an enlargement, a shared device, the anchored reduction, a sheet.

use super::*;

#[test]
fn an_enlargement_keeps_regions_and_reference() {
    let Some(mut g) = graph("an_enlargement_keeps_regions_and_reference") else { return };
    let (s, pixels) = source(&mut g, 8);
    let image = Image::new(&s).then(Crop(Rect::new(100, 50, 60, 40))).lanczos(1.7);
    assert_eq!(image.extent(), Rect::new(0, 0, 102, 68));
    let full = g.render(&image, image.extent(), None).expect("render");
    for region in regions(image.extent()) {
        assert_eq!(g.render(&image, region, None).expect("region"), cut(&full, image.extent(), region), "{region:?}");
    }
    let plane = cpu::crop(&Plane::new(s.extent(), pixels), Rect::new(100, 50, 60, 40));
    let reference = cpu::lanczos(&cpu::lanczos(&plane, Axis::X, 102), Axis::Y, 68);
    let e = error(&full, &reference.pixels);
    assert!(e < TOLERANCE, "enlargement error {e}");

    // Twin: the reference enlarged from a crop one pixel over.
    let shifted = cpu::crop(&Plane::new(s.extent(), noise(W, H, 8)), Rect::new(101, 50, 60, 40));
    let wrong = cpu::lanczos(&cpu::lanczos(&shifted, Axis::X, 102), Axis::Y, 68);
    let e = error(&full, &wrong.pixels);
    assert!(e > 1e-2, "a one-pixel shift went unseen: error {e}");
}

#[test]
fn graphs_on_a_shared_device_render_as_their_own() {
    let Some(mut own) = graph("graphs_on_a_shared_device_render_as_their_own") else { return };
    let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
        println!("SKIPPED graphs_on_a_shared_device_render_as_their_own: no device");
        return;
    };
    let limit = device.limits().max_texture_dimension_2d;
    let mut a = Graph::from_device(device.clone(), queue.clone(), "shared".into(), u32::MAX);
    let mut b = Graph::from_device(device.clone(), queue, "shared".into(), 1024);
    // The graph follows the device it was handed, under the side its caller asked for.
    assert_eq!((a.max_side(), b.max_side()), (limit, limit.min(1024)));
    let pixels = noise(W, H, 9);
    let reference = {
        let s = own.upload(W, H, &pixels).expect("upload");
        own.render(&chain(&s), chain(&s).extent(), None).expect("own render")
    };
    for g in [&mut a, &mut b] {
        let s = g.upload(W, H, &pixels).expect("upload");
        assert_eq!(g.render(&chain(&s), chain(&s).extent(), None).expect("shared render"), reference);
    }
    // Twin: another gain on the shared device is told apart.
    let s = a.upload(W, H, &pixels).expect("upload");
    let other = Image::new(&s).then(Gain([1.31, 0.8, 1.1, 1.0])).then(BoxBlur3).then(Crop(Rect::new(17, 9, 251, 177))).then(BoxBlur3).lanczos(0.37);
    assert_ne!(a.render(&other, other.extent(), None).expect("twin render"), reference);
}

#[test]
fn the_anchored_reduction_is_core_image_s_and_its_reference_s() {
    let Some(mut g) = graph("the_anchored_reduction_is_core_image_s_and_its_reference_s") else { return };
    let (s, pixels) = source(&mut g, 3);
    let plane = Plane::new(s.extent(), pixels);
    let scale = 0.1653;
    let image = Image::new(&s).lanczos_anchored(scale);
    // 300 × 0.1653 = 49.59 and 200 × 0.1653 = 33.06: the partial pixel is kept, not rounded away.
    assert_eq!(image.extent(), Rect::new(0, 0, 50, 34));
    let gpu = g.render(&image, image.extent(), None).expect("render");
    let reference = cpu::lanczos_anchored(&cpu::lanczos_anchored(&plane, Axis::X, scale), Axis::Y, scale);
    let e = error(&gpu, &reference.pixels);
    println!("anchored Lanczos against its reference: {e:.2e}");
    assert!(e < TOLERANCE, "anchored Lanczos error {e}");
    for region in regions(image.extent()).into_iter().filter(|r| image.extent().contains(r)) {
        let part = g.render(&image, region, None).expect("region render");
        assert_eq!(part, cut(&gpu, image.extent(), region), "{region:?} differs from the full render");
    }
    // The top row holds a sixth of a pixel of picture: it fades towards clear, as Core Image's does.
    let ones = g.upload(W, H, &vec![[1.0; 4]; (W * H) as usize]).expect("upload");
    let flat = g.render(&Image::new(&ones).lanczos_anchored(scale), image.extent(), None).expect("render");
    assert!(flat[10][3] < 0.2 && (flat[16 * 50 + 10][3] - 1.0).abs() < 1e-4, "{:?} {:?}", flat[10], flat[16 * 50 + 10]);
    // Twin: anchored at the top instead, the rows land a fraction of a pixel away.
    let flipped = Plane::new(plane.extent, (0..200).rev().flat_map(|y| plane.pixels[y * 300..(y + 1) * 300].to_vec()).collect());
    let top = cpu::lanczos_anchored(&cpu::lanczos_anchored(&flipped, Axis::X, scale), Axis::Y, scale);
    let top: Vec<[f32; 4]> = (0..34).rev().flat_map(|y| top.pixels[y * 50..(y + 1) * 50].to_vec()).collect();
    assert!(error(&gpu, &top) > 1e-2, "the anchor went unseen");
    // Shifted under a pixel, the kernel still follows its reference, and the shift is seen.
    let shifted = Image::new(&s).lanczos_shifted(scale, (0.37, 0.81));
    assert_eq!(shifted.extent(), Rect::new(0, 0, 50, 34));
    let moved = g.render(&shifted, shifted.extent(), None).expect("render");
    let reference = cpu::lanczos_shifted(&cpu::lanczos_shifted(&plane, Axis::X, scale, 0.37), Axis::Y, scale, 0.81);
    assert!(error(&moved, &reference.pixels) < TOLERANCE, "shifted anchored Lanczos error {}", error(&moved, &reference.pixels));
    assert!(error(&moved, &gpu) > 1e-2, "twin: the shift went unseen");
}

#[test]
fn images_laid_out_on_one_sheet_equal_each_render_at_its_place_and_leave_the_rest_clear() {
    let Some(mut g) = graph("images_laid_out_on_one_sheet_equal_each_render_at_its_place_and_leave_the_rest_clear") else { return };
    let (a, _) = source(&mut g, 3);
    let (b, _) = source(&mut g, 4);
    let (ia, ib) = (chain(&a), Image::new(&b).then(Gain([0.5, 1.2, 0.9, 1.0])));
    let rendered = [g.render(&ia, ia.extent(), None).unwrap(), g.render(&ib, ib.extent(), None).unwrap()];
    let (pa, pb) = ((5, 7), (110, 2));
    let size = (420, 210);
    let (copies, sheet) = (g.stats().copies, g.laid_out(&[(&ia, pa), (&ib, pb)], size).unwrap());
    assert_eq!(g.stats().copies, copies, "each image's last stage writes on the sheet, with no copy");
    let read = g.read_back(&[&sheet]).expect("read back").remove(0);
    let sheet_rect = Rect::new(0, 0, size.0, size.1);
    for ((image, at), pixels) in [(&ia, pa), (&ib, pb)].into_iter().zip(&rendered) {
        let e = image.extent();
        assert_eq!(cut(&read, sheet_rect, Rect::new(at.0, at.1, e.w, e.h)), *pixels);
    }
    assert_eq!(read[0], [0.0; 4], "the sheet is clear where nothing lies");
    // Twin: a bare source has no stage to write on the sheet, so it is copied there.
    g.laid_out(&[(&Image::new(&a), pa)], size).unwrap();
    assert!(g.stats().copies > copies);
    // Twin: a place past the sheet's edge is refused rather than cut.
    assert!(g.laid_out(&[(&ib, (400, 0))], size).is_err());
    // Twin: the second sheet reuses nothing of the first under its clear corner.
    let again = g.laid_out(&[(&ib, pb)], size).unwrap();
    assert_eq!(g.read_back(&[&again]).expect("read back").remove(0)[size.0 as usize * 5 + 10], [0.0; 4]);
}
