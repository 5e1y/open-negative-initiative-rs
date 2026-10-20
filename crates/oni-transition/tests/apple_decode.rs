//! Apple's decode in process on roll #8's DSC_6440: the 3x3 fitted on it brings LibRaw's render, through
//! the real pipeline and the frame's own sidecar, onto the render of Apple's decode, the Swift render.

#[cfg(target_os = "macos")]
#[test]
fn one_frame_s_matrix_lands_libraw_on_the_swift_render() {
    use oni_graph::Graph;
    use oni_pipeline::{Inputs, Pipeline, Request};
    use oni_transition::apple::{AppleDecode, CoreImage};
    use oni_transition::measure::{self, BLOCK, Grid, Plane};
    let check = "one_frame_s_matrix_lands_libraw_on_the_swift_render";
    let Some(dir) = oni_fixtures::raw_dir(check) else { return };
    let raw = dir.join("DSC_6440.NEF");
    if !raw.is_file() || !oni_fixtures::heavy(check) {
        return println!("SKIPPED {check}: no DSC_6440.NEF in {}, or heavy checks off", dir.display());
    }
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED {check}: no GPU") };
    let apple = CoreImage.blocks(&raw).expect("Apple's decode");
    let d = oni_decode::decode_native(&raw, 1.0).expect("LibRaw's decode");
    let plane = Plane { width: d.width, height: d.height, pixels: &d.pixels };
    let shift = measure::fit(&apple, plane).expect("a fit");
    assert_eq!(shift.offset, (0, 0));
    let m = measure::matrix(&shift.pairs).expect("a matrix");
    let r = measure::residual(&shift.pairs, &m);
    assert!(r[0] < 0.002 && r[1] < 0.002 && r[2] < 0.006, "density residual {r:?}");

    let libraw = Grid::of(plane);
    let settings = oni_settings::sidecar::read(&raw, true, false).expect("its sidecar").settings.per_pixel_only();
    let request = Request { full_width: Some((apple.width * BLOCK) as f64), ..Request::default() };
    let mut pipeline = Pipeline::new();
    let mut render = |means: &[[f32; 3]]| -> Vec<[f32; 4]> {
        let px: Vec<[f32; 4]> = means.iter().map(|v| [v[0], v[1], v[2], 1.0]).collect();
        let source = g.upload(apple.width as u32, apple.height as u32, &px).expect("upload");
        let image = pipeline.image(&mut g, &source, &settings, &request, &Inputs::default()).expect("recipe");
        g.render(&image, image.extent(), None).expect("render")
    };
    let swift = render(&apple.means);
    let gap = |ours: &[[f32; 4]]| -> f64 {
        let clip = |x: f32| f64::from(x.clamp(0.0, 1.0));
        ours.iter().zip(&swift).map(|(o, s)| (0..3).map(|c| (clip(o[c]) - clip(s[c])).abs()).fold(0.0, f64::max)).sum::<f64>() / swift.len() as f64
    };
    let matched: Vec<[f32; 3]> = libraw.means.iter().map(|v| {
        let o = measure::apply(&m, v.map(f64::from));
        [o[0] as f32, o[1] as f32, o[2] as f32]
    }).collect();
    let (with, without) = (gap(&render(&matched)), gap(&render(&libraw.means)));
    println!("mean worst-channel gap to the Swift render: {without:.4} as LibRaw reads it, {with:.4} through the matrix");
    assert!(with < 0.0025, "{with}");
    // Twin: LibRaw's own colours stand visibly off the Swift render.
    assert!(without > 0.01, "{without}");
}
