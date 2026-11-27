use super::*;
use oni_graph::Rect;

#[test]
fn a_gesture_keeps_a_whole_reduction_only_as_large_as_the_canvas_it_renders() {
    let (frame, canvas) = ((8000.0, 5000.0), (3920.0, 2640.0));
    // The whole frame reduced to the side a view renders at `scale`, lighter under a hand.
    let kept = |scale: f64, coarse: bool, budget: &dyn Fn(f64, f64) -> bool| {
        let l = lighter(coarse, canvas);
        budget(frame.0 * frame.1 * (scale * l).min(1.0).powi(2), l)
    };
    let ours = |whole: f64, l: f64| kept_whole(whole, canvas, l);
    let fit = (canvas.0 / frame.0).min(canvas.1 / frame.1);
    assert!(!kept(1.0, true, &ours), "a gesture at 100 % reduces the whole frame each step");
    assert!(kept(fit, true, &ours) && kept(0.6, false, &ours), "a pan at fit and a sharp render at 60 % keep it");
    // Twin: the canvas's own budget keeps the gesture's whole 10 MP at 100 %, never reused at the next step.
    assert!(kept(1.0, true, &|whole, _| whole <= KEPT_WHOLE * canvas.0 * canvas.1));
}

#[test]
fn a_reduction_through_the_halvings_is_the_direct_one() {
    let Ok(device) = crate::render::Device::open() else { return };
    let mut engine = Engine::on(&device);
    let (w, h) = (1600usize, 1000usize);
    let wave = |x: f32, y: f32| [0.4 + 0.3 * (x * 0.05).sin() * (y * 0.031).cos(), 0.3 + 0.2 * x / w as f32, 0.5, 1.0];
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| wave((i % w) as f32, (i / w) as f32)).collect();
    let full = engine.graph.upload(w as u32, h as u32, &pixels).unwrap();
    let (level, reduce) = engine.level(&full, 300, w).unwrap();
    assert!(level.extent().w < w as u32 && reduce > 0.5, "{:?} {reduce}", level.extent());
    let through = Image::new(&level).lanczos(reduce);
    let read = |e: &mut Engine, image: &Image| e.graph.render(image, image.extent(), None).unwrap();
    let direct = Image::new(&full).lanczos(300.0 / w as f64);
    assert_eq!(through.extent(), direct.extent());
    let (ours, theirs) = (read(&mut engine, &through), read(&mut engine, &direct));
    let gap = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>()).sum::<f32>() / a.len() as f32;
    assert!(gap(&ours, &theirs) < 2e-3, "{}", gap(&ours, &theirs));
    // Twin: the frame a few pixels over reduces to another picture, which the gap sees.
    let moved = Image::new(&full).then(oni_graph::stages::Crop(Rect::new(6, 0, w as u32 - 6, h as u32))).lanczos(300.0 / w as f64);
    let other = read(&mut engine, &moved);
    assert!(gap(&ours[..other.len().min(ours.len())], &other) > 1e-2);
}
