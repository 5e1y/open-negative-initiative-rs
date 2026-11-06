//! A plate against a group of keys: a plate is a socket of keycaps, set flush on one face as tall as
//! they are and rounded as a key is; a group's flat keys stand Swift's `DS.Space.sm` apart, each on its own
//! face inside its own edge, told apart at rest. Capitals are the keycap's alone.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::GroupProbe;
use slint::platform::WindowEvent;
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// Swift's `DS.Space.sm` between the keys of a group.
const ROW_GAP: f32 = 8.0;
/// A key's corner, `DS.Radius.badge`.
const PLATE_RADIUS: f32 = 8.0;
/// A keycap's height, `DS.Control.keycapHeight`, and a flat key's, `DS.Control.fieldHeight`.
const CAP: f32 = 40.0;
const FLAT: f32 = 32.0;
/// How much lighter a flat key's edge is than its face, summed over the three channels: the white at
/// 10 % the edge lays over the face lifts each by about 20.
const EDGE_LIFT: u32 = 30;

/// The sum of a 565 pixel's three channels, each scaled to 8 bits.
fn light(p: Rgb565Pixel) -> u32 {
    let v = p.0 as u32;
    ((v >> 11) & 0x1f) * 255 / 31 + ((v >> 5) & 0x3f) * 255 / 63 + (v & 0x1f) * 255 / 31
}

pub(crate) fn a_plate_s_keycaps_touch_and_a_group_s_flat_keys_stand_apart_in_their_edges() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = GroupProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 260));
    probe.show().unwrap();
    rig.settle();
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;

    assert!(near(probe.get_plate_gap(), 0.0), "a plate's keycaps share their walls: gap {}", probe.get_plate_gap());
    assert!(near(probe.get_row_gap(), ROW_GAP), "a group's keys stand {ROW_GAP} apart: gap {}", probe.get_row_gap());
    // Twins: the flush measure sees the group's gap, the spaced one the plate's lack of it.
    assert!(!near(probe.get_row_gap(), 0.0), "the gap measure reads a group's room");
    assert!(!near(probe.get_plate_gap(), ROW_GAP), "a plate is no spaced group");

    assert!(near(probe.get_plate_key_height(), CAP), "the plate holds keycaps, {CAP} tall: {}", probe.get_plate_key_height());
    assert!(near(probe.get_plate_height(), probe.get_plate_key_height()), "the socket is as tall as its keycaps, no inset");
    assert!(near(probe.get_plate_radius(), PLATE_RADIUS), "the plate follows a key's corner: {}", probe.get_plate_radius());
    assert!(near(probe.get_row_radius(), 0.0), "a group draws no socket");
    // Twin: a flat key's height is no keycap's, so the height read tells which the plate holds.
    assert!(!near(probe.get_plate_key_height(), FLAT), "the plate's keys are no flat keys");

    // At rest, with no hand near: each flat key's edge lighter than its face, its face apart from the
    // ground, and the ground showing between the group's two keys.
    let (w, px) = rig.drawn();
    let (a, b) = (common::the(&probe, Role::Button, "Website"), common::the(&probe, Role::Button, "Feedback"));
    let at = |x: f32, y: f32| light(px[y as usize * w + x as usize]);
    let row = a.y + a.height / 2.0;
    let (ground, edge, face) = (at(a.x - 3.0, row), at(a.x, row), at(a.x + 3.0, row));
    rig.shot("group-probe");
    assert!(edge > face + EDGE_LIFT, "a flat key's edge shows at rest: edge {edge}, face {face}");
    assert!(face > ground, "a flat key's face stands off the ground: face {face}, ground {ground}");
    assert_eq!(at(a.x + a.width + ROW_GAP / 2.0, row), ground, "the ground between the group's two keys");
    assert!(at(b.x, row) > face + EDGE_LIFT, "the second key's own edge, a gap after the first's");
    // Twin: two points of one face show no edge between them, so the lift read is the edge's.
    assert!(at(a.x + 5.0, row) <= face + EDGE_LIFT, "the face alone reads as no edge");
}

/// A flat key and a block's title show their words as written; a keycap alone shows its word in capitals.
pub(crate) fn capitals_are_the_keycap_s_alone() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = GroupProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 360));
    probe.show().unwrap();
    rig.settle();
    let drawn = |word: &str| !common::placed(&probe, Role::Text, word).is_empty();

    assert!(drawn("Website") && drawn("Feedback"), "a flat key shows its word as written");
    assert!(!drawn("WEBSITE") && !drawn("FEEDBACK"), "no flat key shows its word in capitals");
    assert!(drawn("Block title") && !drawn("BLOCK TITLE"), "a block's title shows as written");
    assert!(drawn("DONE") && drawn("DETECT"), "a keycap shows its word in capitals, on a plate or not");
    // Twin: the search reads the legend drawn, not the key's name, which stays "Done".
    assert!(!drawn("Done"), "the keycap's legend is never its word as written");
}
