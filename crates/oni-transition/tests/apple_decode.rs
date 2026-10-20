//! Apple's decode in-process against `scripts/decoder-gap` on roll #8: DSC_6440 reads +0.063,
//! -0.025, -0.125 stop in `scripts/decoder-gap`, the roll's median +0.063, -0.026, -0.126.

#[cfg(target_os = "macos")]
#[test]
fn core_image_reads_the_z_f_as_the_study_measured() {
    use oni_transition::apple::{AppleDecode, CoreImage, Simulated};
    use oni_transition::measure::{self, Plane};
    let check = "core_image_reads_the_z_f_as_the_study_measured";
    let Some(dir) = oni_fixtures::raw_dir(check) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let raw = dir.join("DSC_6440.NEF");
    if !raw.is_file() {
        println!("SKIPPED {check}: no DSC_6440.NEF in {}", dir.display());
        return;
    }
    let d = oni_decode::decode(&raw, 1.0).expect("LibRaw's decode");
    let plane = Plane { width: d.width, height: d.height, pixels: &d.pixels };
    let apple = measure::fit(&CoreImage.blocks(&raw).expect("Apple's decode"), plane).expect("a fit");
    let study = [0.063, -0.025, -0.125];
    for c in 0..3 {
        assert!((apple.stops[c] - study[c]).abs() < 0.006, "{:?}", apple.stops);
    }
    assert_eq!(apple.offset, (0, 0));
    assert!(apple.correlation > 0.999, "{apple:?}");
    // Twin: LibRaw against itself reads no shift, so the gap measured is Core Image's own.
    let itself = measure::fit(&Simulated { stops: [0.0; 3] }.blocks(&raw).expect("LibRaw's blocks"), plane).expect("a fit");
    assert!(itself.stops.iter().all(|s| s.abs() < 1e-3), "{itself:?}");
}
