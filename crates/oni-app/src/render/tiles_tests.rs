//! Tiles laid on one sheet read back as each one rendered alone, at its place.
use super::*;

#[test]
fn tiles_laid_on_a_sheet_equal_each_render_alone_at_its_place() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the tiles are not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let mut engine = shared.lock().unwrap();
    let mut frame = |w: usize, h: usize, tone: f32| {
        let pixels = (0..w * h).map(|i| [tone + (i % 7) as f32 * 0.02, 0.4, 0.6, 1.0]).collect();
        engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap()
    };
    let (a, b) = (frame(600, 400, 0.2), frame(300, 450, 0.5));
    let (sa, mut sb) = (PipelineSettings::default(), PipelineSettings::default());
    sb.vignette.stops = 0.6;
    let places = [(0, 0), (320, 10)];
    let sheet = engine.render_laid(&[Laid::Frame(&a, &sa), Laid::Frame(&b, &sb)], 300, &|i, _| places[i], (640, 320)).unwrap();
    let read = Linear { width: 640, height: 320, pixels: engine.graph.read_back(&[&sheet]).unwrap().remove(0) };
    let alone = [engine.render(&a, &sa, Some(300)).unwrap(), engine.render(&b, &sb, Some(300)).unwrap()];
    for (l, (x, y)) in alone.iter().zip(places) {
        for row in 0..l.height {
            let at = (y as usize + row) * read.width + x as usize;
            assert_eq!(read.pixels[at..at + l.width], l.pixels[row * l.width..(row + 1) * l.width], "row {row}");
        }
    }
    // Twin: the two tiles differ, so a sheet swapping their places would fail above.
    assert_ne!((alone[0].width, alone[0].height), (alone[1].width, alone[1].height));
}

#[test]
fn a_tile_from_a_held_preview_is_reduced_to_its_side_and_a_small_one_kept() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the tiles are not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let mut engine = shared.lock().unwrap();
    let mut frame = |w: usize, h: usize| {
        let pixels = (0..w * h).map(|i| [0.2 + (i % 7) as f32 * 0.02, 0.4, 0.6, 1.0]).collect();
        engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: 6000.0, undeclared: None }).unwrap()
    };
    let (big, small) = (frame(600, 400), frame(120, 80));
    let tile = engine.tile_of(&big, 300).unwrap();
    assert_eq!(tile.size(), (300, 200));
    assert_eq!(tile.full_width, 6000.0, "the frame's own width, which the dosed radii read");
    // Twin: a preview already under the side is laid as it is.
    assert_eq!(engine.tile_of(&small, 300).unwrap().size(), (120, 80));
}

#[test]
fn a_picture_standing_in_is_scaled_to_the_tile_side_at_its_place() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the tiles are not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let mut engine = shared.lock().unwrap();
    let grey = [0.25, 0.5, 0.75, 1.0];
    let picture = engine.picture(40, 20, &vec![grey; 800]).unwrap();
    let sheet = engine.render_laid(&[Laid::Shown(&picture)], 100, &|_, _| (10, 5), (200, 100)).unwrap();
    let read = engine.graph.read_back(&[&sheet]).unwrap().remove(0);
    let at = |x: usize, y: usize| read[y * 200 + x];
    // 40 × 20 at a side of 100 covers 100 × 50 from (10, 5): its middle and its far corner are the grey.
    let near = |p: [f32; 4]| p.iter().zip(grey).all(|(a, b)| (a - b).abs() < 2e-3);
    assert!(near(at(60, 30)) && near(at(108, 53)), "{:?} {:?}", at(60, 30), at(108, 53));
    // Twin: past the scaled picture the sheet stays clear.
    assert_eq!(at(112, 57), [0.0; 4]);
}


#[test]
fn a_cropped_frame_spans_the_tile_side_as_an_uncropped_one_does() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the tiles are not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let mut engine = shared.lock().unwrap();
    let pixels = (0..600 * 400).map(|i| [0.2 + (i % 7) as f32 * 0.02, 0.4, 0.6, 1.0]).collect();
    let frame = engine.prepare(Decoded { linear: Linear { width: 600, height: 400, pixels }, full_width: 600.0, undeclared: None }).unwrap();
    let mut cropped = PipelineSettings::default();
    (cropped.geometry.crop.width, cropped.geometry.crop.height) = (0.5, 0.5);
    let sizes = std::cell::RefCell::new(Vec::new());
    let place = |_, size| {
        sizes.borrow_mut().push(size);
        (0, 0)
    };
    let whole = PipelineSettings::default();
    engine.render_laid(&[Laid::Frame(&frame, &whole), Laid::Frame(&frame, &cropped)], 200, &place, (200, 200)).unwrap();
    let sizes = sizes.into_inner();
    assert_eq!(sizes[0].0, 200);
    assert_eq!(sizes[1].0, 200, "a crop to a quarter of the frame is as wide as the whole: {sizes:?}");
    // Twin: its proportions are its crop's, within the rounding of its edges.
    assert!(sizes[1].1.abs_diff(sizes[0].1) <= 1, "{sizes:?}");
}
