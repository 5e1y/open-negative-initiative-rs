//! The decode must stay linear: one stop more doubles every channel's mean.

mod common;

/// The bound of the Swift check: |ratio - 2| <= 2 * 0.01 on every channel.
fn linear(ratio: [f64; 3]) -> bool {
    ratio.iter().all(|r| (r - 2.0).abs() <= 0.02)
}

fn ratio(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [b[0] / a[0], b[1] / a[1], b[2] / a[2]]
}

#[test]
fn one_stop_doubles_every_channel() {
    let Some(files) = common::frames("linearity", 1) else {
        return;
    };
    for f in files.iter().take(3) {
        let base = oni_decode::decode(f, 1.0).expect("decode");
        let up = oni_decode::decode(f, 2.0).expect("decode +1 EV");
        let r = ratio(common::means(&base.pixels), common::means(&up.pixels));
        println!(
            "{}: {}x{} ratio {r:.4?}",
            f.file_name().unwrap().to_string_lossy(),
            base.width,
            base.height
        );
        assert!(linear(r), "not linear: {r:?}");
        // Twin: the same pair through a display gamma must fail the bound.
        let encoded = |px: &[[f32; 4]]| {
            px.iter()
                .map(|p| p.map(|v| v.max(0.0).powf(1.0 / 2.2)))
                .collect::<Vec<_>>()
        };
        let g = ratio(
            common::means(&encoded(&base.pixels)),
            common::means(&encoded(&up.pixels)),
        );
        assert!(!linear(g), "the bound cannot see a gamma: {g:?}");
    }
}
