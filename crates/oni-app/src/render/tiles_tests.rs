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
    let sheet = engine.render_laid(&[(&a, &sa), (&b, &sb)], 300, &|i, _| places[i], (640, 320)).unwrap();
    let read = engine.read_sheet(&sheet);
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
