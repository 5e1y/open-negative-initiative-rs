//! The prewarm compiles what a busy notch on a preview needs, Texture's blurs included.
use super::*;

/// A busy edit on a 2000 px preview: Texture runs, as it rests only under a 750 px side.
fn busy_notch(engine: &mut Engine) {
    notch(engine, &engine::prewarm::busy());
}

fn notch(engine: &mut Engine, s: &PipelineSettings) {
    let (w, h) = (2000usize, 16usize);
    let pixels = (0..w * h).map(|i| [0.2 + (i % 13) as f32 * 0.05, 0.4, 0.6, 1.0]).collect();
    let decoded = Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None, seed: [0, 0] };
    let negative = engine.prepare(decoded).unwrap();
    engine.resident(&negative, s, None).unwrap();
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
    engine.prewarm_at(64, 64, &|_| true);
    let compiled = engine.compiled();
    busy_notch(&mut engine);
    assert!(engine.compiled() > compiled, "the small frame alone misses Texture");
}

#[test]
fn a_prewarmed_engine_compiles_nothing_on_a_digital_photo_s_first_busy_notch() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the digital prewarm is not exercised");
        return;
    };
    let warm = Engine::shared(&device);
    let mut engine = warm.lock().unwrap();
    engine.prewarm();
    let compiled = engine.compiled();
    notch(&mut engine, &engine::prewarm::digital_busy());
    assert_eq!(engine.compiled(), compiled, "a pipeline compiled on a digital photo's first notch");
    drop(engine);
    // Twin: warmed on the film modes alone, the virtual film, halation and grain wait for the first notch.
    let film = Engine::shared(&Device::open().unwrap());
    let mut engine = film.lock().unwrap();
    engine.prewarm_where(&|s| !s.mode.is_digital());
    let compiled = engine.compiled();
    notch(&mut engine, &engine::prewarm::digital_busy());
    assert!(engine.compiled() > compiled, "the film modes alone warm the digital chain");
}

#[test]
fn a_prewarmed_engine_compiles_nothing_on_a_digital_photo_s_first_busy_notch_on_a_legacy_print() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the digital legacy print's prewarm is not exercised");
        return;
    };
    let warm = Engine::shared(&device);
    let mut engine = warm.lock().unwrap();
    engine.prewarm();
    let compiled = engine.compiled();
    notch(&mut engine, &engine::prewarm::digital_legacy_busy());
    assert_eq!(engine.compiled(), compiled, "a pipeline compiled on a digital photo's first notch on a legacy print");
    drop(engine);
    // Twin: warmed on the film modes alone, the digital machine's film passes wait.
    let pro = Engine::shared(&Device::open().unwrap());
    let mut engine = pro.lock().unwrap();
    engine.prewarm_where(&|s| !s.mode.is_digital());
    let compiled = engine.compiled();
    notch(&mut engine, &engine::prewarm::digital_legacy_busy());
    assert!(engine.compiled() > compiled, "the film modes alone warm the digital legacy print's");
}
