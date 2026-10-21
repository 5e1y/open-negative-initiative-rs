//! Tiles rendered together read back as each one rendered alone, in their order.
use super::*;

#[test]
fn tiles_rendered_together_equal_each_render_alone() {
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
    let together = engine.render_tiles(&[(&a, &sa), (&b, &sb)], 300);
    let alone = [engine.render(&a, &sa, Some(300)).unwrap(), engine.render(&b, &sb, Some(300)).unwrap()];
    for (t, a) in together.iter().zip(&alone) {
        let t = t.as_ref().unwrap();
        assert_eq!((t.width, t.height), (a.width, a.height));
        assert_eq!(t.pixels, a.pixels);
    }
    // Twin: the two tiles differ, so a readback swapping them would fail above.
    assert_ne!((alone[0].width, alone[0].height), (alone[1].width, alone[1].height));
}
