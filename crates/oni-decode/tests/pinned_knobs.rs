//! Every automatic knob of LibRaw is pinned in `csrc/pinned.c`, so no frame decodes by its own content.

const PINNED: &str = include_str!("../csrc/pinned.c");

/// The knobs that would let LibRaw decide from the frame, each with the value that stops it.
const KNOBS: [&str; 6] = [
    "p->use_camera_wb = 0;",
    "p->use_auto_wb = 0;",
    "p->no_auto_bright = 1;",
    "p->adjust_maximum_thr = 0.0f;",
    "p->med_passes = 0;",
    "p->fbdd_noiserd = 0;",
];

fn unpinned(source: &str) -> Vec<&'static str> {
    KNOBS.iter().copied().filter(|k| !source.contains(k)).collect()
}

#[test]
fn no_automatic_knob_is_left_to_libraw() {
    assert_eq!(unpinned(PINNED), Vec::<&str>::new());
    // Twin: a copy that drops the white-level pin is caught.
    let loose = PINNED.replace("p->adjust_maximum_thr = 0.0f;", "");
    assert_eq!(unpinned(&loose), vec!["p->adjust_maximum_thr = 0.0f;"]);
}
