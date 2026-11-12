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
                crate::patches::write(&caches, oni_pipeline::corrections::PatchKey::of(&PipelineSettings::rest(oni_settings::ConversionMode::Negative, true), &c), side, side, &vec![[0.5; 4]; (side * side) as usize]).unwrap();
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
    let key = |c: &Correction| oni_pipeline::corrections::PatchKey::of(&with(&[]), c);
    assert!(b.iter().all(|c| engine.keeps_patch(key(c))), "the render's own patches stay");
    assert!(a.iter().any(|c| !engine.keeps_patch(key(c))), "the older frame's let go");
    // Twin: a render naming more than the budget keeps every patch it names.
    let many = corrections(10);
    engine.render(&negative, &with(&many), Some(32)).unwrap();
    assert!(engine.inputs_kept() >= 10 * patch, "a patch the render names was let go");
    engine.trim();
    assert_eq!(engine.inputs_kept(), 0, "the editor's memory takes them with it");
    let _ = std::fs::remove_dir_all(&caches);
}

/// A search lays the patches its window holds, each at its own cost: a frame holding 150 patches
/// elsewhere costs what one holding ten does, and a second search uploads nothing again.
#[test]
fn a_search_costs_what_lies_in_its_window_however_many_patches_the_frame_holds() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the search's cost is not exercised");
        return;
    };
    let caches = std::env::temp_dir().join(format!("oni-search-cost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&caches);
    let mut engine = Engine::on(&device);
    engine.caches = Some(caches.clone());
    // Wider than a search's window, about 2 900 px about an 800 px tile.
    let (w, h) = (4800usize, 900usize);
    let pixels = (0..w * h).map(|i| (0.2 + 0.5 * (i % w) as f32 / w as f32, i)).map(|(v, i)| [v, v * 0.8, v * (0.5 + 0.1 * (i % 7) as f32), 1.0]).collect();
    let full = engine.full(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
    let dot = |x: f64, y: f64| {
        let point = |x: f64| oni_settings::cg::Point { x, y };
        let c = Correction { id: Id::new(), stroke: vec![point(x), point(x + 0.002)], radius: 12.0 };
        crate::patches::write(&caches, oni_pipeline::corrections::PatchKey::of(&PipelineSettings::rest(oni_settings::ConversionMode::Negative, true), &c), 40, 40, &vec![[0.9, 0.9, 0.9, 1.0]; 1600]).unwrap();
        c
    };
    let (searched, near) = (dot(0.05, 0.5), dot(0.1, 0.4));
    let far: Vec<Correction> = (0..150).map(|k| dot(0.75 + 0.2 * k as f64 / 150.0, 0.1 + 0.8 * (k * 7 % 150) as f64 / 150.0)).collect();
    let with = |cs: &[&[Correction]]| PipelineSettings { corrections: cs.concat(), ..PipelineSettings::rest(oni_settings::ConversionMode::Negative, true) };
    let (one, by) = (std::slice::from_ref(&searched), std::slice::from_ref(&near));
    let (few, many) = (with(&[one, by, &far[..10]]), with(&[one, by, &far]));
    let cost = |engine: &mut Engine, s: &PipelineSettings| {
        let before = engine.stats();
        let (region, _) = engine.search_region(&full, s, &searched).unwrap();
        let after = engine.stats();
        (after.pixels - before.pixels, after.textures_created - before.textures_created, region.pixels)
    };
    // The first searches read each patch and stroke once.
    cost(&mut engine, &few);
    cost(&mut engine, &many);
    let (a, _, seen_few) = cost(&mut engine, &few);
    let (b, made, seen_many) = cost(&mut engine, &many);
    assert_eq!(a, b, "150 patches away from the window cost what 10 do");
    assert_eq!(made, 0, "a second search uploaded strokes or patches again");
    assert!(seen_few == seen_many, "the far patches changed the window");
    // Twins: the patch lying in the window is laid there, and the counters see its cost and, once
    // another correction joins the frame, its patches and strokes gathered again.
    let (c, _, seen_bare) = cost(&mut engine, &with(&[one, &far[..10]]));
    assert!(a > c, "the patch in the window cost nothing: {a} against {c}");
    assert!(seen_few != seen_bare, "the search did not see the patch lying in its window");
    let joined = dot(0.6, 0.5);
    assert!(cost(&mut engine, &with(&[one, by, &far, std::slice::from_ref(&joined)])).1 >= 2, "the sheets gathered again went unseen");
    let _ = std::fs::remove_dir_all(&caches);
}
