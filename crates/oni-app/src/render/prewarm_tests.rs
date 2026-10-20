//! The prewarm compiles what a busy notch on a preview needs, Texture's blurs included.
use super::*;

/// A busy edit on a 2000 px preview: Texture runs, as it rests only under a 750 px side.
fn busy_notch(engine: &mut Engine) {
    let (w, h) = (2000usize, 16usize);
    let pixels = (0..w * h).map(|i| [0.2 + (i % 13) as f32 * 0.05, 0.4, 0.6, 1.0]).collect();
    let decoded = Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None };
    let negative = engine.prepare(decoded).unwrap();
    engine.resident(&negative, &engine::prewarm::busy(), None).unwrap();
}

#[test]
fn a_prewarmed_engine_compiles_nothing_on_its_first_busy_notch() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the prewarm is not exercised");
        return;
    };
    let warm = Engine::shared(&device);
    let mut engine = warm.lock().unwrap();
    engine.prewarm();
    let compiled = engine.compiled();
    busy_notch(&mut engine);
    assert_eq!(engine.compiled(), compiled, "a pipeline compiled on the first notch");
    drop(engine);
    // Twin: warmed on the small frame alone, Texture's shaders wait for the first notch.
    let small = Engine::shared(&Device::open().unwrap());
    let mut engine = small.lock().unwrap();
    engine.prewarm_at(64, 64);
    let compiled = engine.compiled();
    busy_notch(&mut engine);
    assert!(engine.compiled() > compiled, "the small frame alone misses Texture");
}
