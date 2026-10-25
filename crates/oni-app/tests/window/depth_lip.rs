//! A well's lit lip lies under its bounds: a section ending on a field draws it whole, as the field
//! alone does, and only a folded section clips what it holds.

use oni_app::DepthProbe;
use slint::platform::WindowEvent;
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// The sum of a 565 pixel's three channels, each scaled to 8 bits.
fn light(p: Rgb565Pixel) -> u32 {
    let v = p.0 as u32;
    ((v >> 11) & 0x1f) * 255 / 31 + ((v >> 5) & 0x3f) * 255 / 63 + (v & 0x1f) * 255 / 31
}

/// The two rows of the lip under a field whose top is at `y`, at mid width, then the ground well below.
fn lip(w: usize, px: &[Rgb565Pixel], x: f32, y: f32, h: f32) -> (u32, u32, u32) {
    let at = |row: f32| light(px[row as usize * w + x as usize]);
    (at(y + h), at(y + h + 1.0), at(y + h + 6.0))
}

pub(crate) fn a_section_ending_on_a_field_draws_its_lip_whole() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = DepthProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 400));
    probe.show().unwrap();
    let (w, px) = rig.drawn();
    let x = probe.get_left() + probe.get_field_width() / 2.0;
    let h = probe.get_field_height();

    // Alone on the ground, the field's lip lights the two rows under it.
    let free = lip(w, &px, x, probe.get_free_y(), h);
    assert!(free.0 > free.2 && free.1 > free.2, "the lip shows under a field alone: {free:?}");
    // Twin: a box clipped at the field's bounds cuts the lip, down to the ground.
    let cut = lip(w, &px, x, probe.get_cut_y(), h);
    assert_eq!((cut.0, cut.1), (cut.2, cut.2), "a clip at the bounds cuts the lip: {cut:?}");
    // A section ending on the field draws its lip as the field alone does.
    let ending = lip(w, &px, x, probe.get_ending_y(), h);
    assert_eq!(ending, free, "a section ending on a field keeps its lip");

    // Folded, the section still clips what it holds: no well where the field stood.
    let well = |px: &[Rgb565Pixel]| light(px[(probe.get_ending_y() + h / 2.0) as usize * w + x as usize]);
    assert_ne!(well(&px), ending.2, "unfolded, the field's well shows");
    probe.set_folded(true);
    rig.pump();
    let (_, folded) = rig.drawn();
    assert_eq!(well(&folded), ending.2, "folded, the section shows nothing of its field");
}
