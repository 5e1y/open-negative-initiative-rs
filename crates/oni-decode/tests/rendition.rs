//! A camera's rendition matrix reaches every pixel of its decodes and no other camera's. Alone in its
//! binary: the table is the process's.

mod common;

#[test]
fn an_installed_matrix_turns_each_pixel_of_its_camera_alone() {
    let Some(frames) = common::frames("an_installed_matrix_turns_each_pixel_of_its_camera_alone", 1) else { return };
    let raw = &frames[0];
    let camera = oni_decode::camera::identify(raw).expect("a camera");
    let native = oni_decode::decode_native(raw, 1.0).expect("decode");
    let m = [[1.12, -0.17, -0.01], [0.03, 0.95, -0.06], [0.01, -0.01, 0.9]];
    // Twin: a matrix for another model leaves the decode as LibRaw's own.
    oni_decode::rendition::install([(camera.make.clone(), format!("{} II", camera.model), m)]);
    assert_eq!(oni_decode::decode(raw, 1.0).expect("decode").pixels, native.pixels);
    oni_decode::rendition::install([(camera.make.clone(), camera.model.clone(), m)]);
    let matched = oni_decode::decode(raw, 1.0).expect("decode");
    assert_eq!((matched.width, matched.height), (native.width, native.height));
    for i in (0..native.pixels.len()).step_by(997) {
        let (n, g) = (native.pixels[i], matched.pixels[i]);
        for r in 0..3 {
            let want: f32 = (0..3).map(|c| m[r][c] * n[c]).sum();
            assert!((g[r] - want).abs() <= 1e-4 + 1e-4 * want.abs(), "pixel {i}: {g:?} against {want}");
        }
    }
    oni_decode::rendition::install([]);
    assert_eq!(oni_decode::decode(raw, 1.0).expect("decode").pixels, native.pixels, "an empty table is LibRaw's own");
}
