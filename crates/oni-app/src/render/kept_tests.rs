//! The masks and patches the engine reads stay under their budget, never losing one a render names,
//! and go with the editor's memory.
use super::*;
use oni_settings::frame::Correction;

#[test]
fn the_patches_kept_stay_under_their_budget_and_go_with_the_editor() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the kept inputs are not exercised");
        return;
    };
    let caches = std::env::temp_dir().join(format!("oni-kept-inputs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&caches);
    // Each patch 1000 px square in 32-bit floats, 16 MB on the GPU.
    let (side, patch) = (1000u32, 16_000_000u64);
    let corrections = |n: usize| -> Vec<Correction> {
        (0..n)
            .map(|_| {
                let c = Correction { id: Id::new(), stroke: Vec::new(), radius: 20.0 };
                crate::patches::write(&caches, c.id, side, side, &vec![[0.5; 4]; (side * side) as usize]).unwrap();
                c
            })
            .collect()
    };
    let mut engine = Engine::on(&device);
    engine.caches = Some(caches.clone());
    let pixels = vec![[0.4; 4]; 64 * 48];
    let negative = engine.prepare(Decoded { linear: Linear { width: 64, height: 48, pixels }, full_width: 64.0, undeclared: None }).unwrap();
    let with = |cs: &[Correction]| PipelineSettings { corrections: cs.to_vec(), ..PipelineSettings::rest(oni_settings::ConversionMode::Negative, true) };
    let (a, b) = (corrections(5), corrections(5));
    engine.render(&negative, &with(&a), Some(32)).unwrap();
    assert_eq!(engine.inputs_kept(), 5 * patch, "a frame's five patches kept");
    engine.render(&negative, &with(&b), Some(32)).unwrap();
    assert!(engine.inputs_kept() <= INPUTS_KEPT, "{} bytes kept past the budget", engine.inputs_kept());
    assert!(b.iter().all(|c| engine.keeps_patch(c.id)), "the render's own patches stay");
    assert!(a.iter().any(|c| !engine.keeps_patch(c.id)), "the older frame's let go");
    // Twin: a render naming more than the budget keeps every patch it names.
    let many = corrections(10);
    engine.render(&negative, &with(&many), Some(32)).unwrap();
    assert!(engine.inputs_kept() >= 10 * patch, "a patch the render names was let go");
    engine.trim();
    assert_eq!(engine.inputs_kept(), 0, "the editor's memory takes them with it");
    let _ = std::fs::remove_dir_all(&caches);
}
