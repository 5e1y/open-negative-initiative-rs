use super::*;
use crate::preset_parameters::pasted_rows;

/// The open frame copied with `carries` once moved, then moved off and pasted back on itself: its
/// vignette and exposure after the paste, and the board's text.
fn pasted_back(store: &mut EditorStore, frame: &str, carries: &BTreeSet<String>) -> (f32, f32, String) {
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.apply_at(Command::Set { knob: Knob::Exposure, value: 0.4 }, 1.0);
    store.copy_settings(None, carries);
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.0 }, 2.0);
    store.apply_at(Command::Set { knob: Knob::Exposure, value: 0.0 }, 3.0);
    store.paste_settings(&[frame.to_string()]);
    let s = store.settings();
    (Knob::Vignette.get(s), Knob::Exposure.get(s), store.board.settings.clone().unwrap_or_default())
}

#[test]
fn a_copy_narrowed_to_some_boxes_pastes_those_alone() {
    let (mut store, dir) = fresh("narrowed");
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
    let (vignette, exposure, text) = pasted_back(&mut store, &frame, &BTreeSet::from(["vignette".to_string()]));
    assert!((vignette - 0.5).abs() < 1e-6, "the vignette ticked comes back: {vignette}");
    assert!(exposure.abs() < 1e-6, "the exposure left out stays the frame's: {exposure}");
    assert!(text.contains("\"carries\""), "the board names what the copy carries");
    // Twin: the whole look brings the exposure back too, on a board the codec alone reads.
    let (vignette, exposure, text) = pasted_back(&mut store, &frame, &pasted_rows());
    assert!((vignette - 0.5).abs() < 1e-6 && (exposure - 0.4).abs() < 1e-6, "{vignette} {exposure}");
    assert!(!text.contains("\"carries\""));
    let _ = std::fs::remove_dir_all(&dir);
}
