//! `KeycapPlate` against a spaced row of keys: on a plate the keys touch, wall to wall, on a socket as
//! tall as they are and rounded as their outer corner; off it they stand Swift's `DS.Space.sm` apart.

use oni_app::GroupProbe;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// Swift's `DS.Space.sm` between the keys of a spaced row.
const ROW_GAP: f32 = 8.0;
/// `KeycapStyle.cornerRadius` 16 at the small and mini variants' factors, `contourWidth` 0.
const PLATE_RADIUS: f32 = 16.0 * 0.525;
const MINI_RADIUS: f32 = 16.0 * 0.38;
/// `DS.Control.keycapPlateHeight`.
const PLATE_KEY: f32 = 52.0;

pub(crate) fn a_plate_s_keys_touch_and_a_spaced_row_s_keep_swift_s_gap() {
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

    assert!(near(probe.get_plate_gap(), 0.0), "a plate's keys share their walls: gap {}", probe.get_plate_gap());
    assert!(near(probe.get_mini_gap(), 0.0), "a mini plate's keys share their walls: gap {}", probe.get_mini_gap());
    assert!(near(probe.get_row_gap(), ROW_GAP), "a spaced row's keys stand {ROW_GAP} apart: gap {}", probe.get_row_gap());
    // Twins: the flush measure sees the row's gap, the spaced one the plate's lack of it.
    assert!(!near(probe.get_row_gap(), 0.0), "the gap measure reads a row's room");
    assert!(!near(probe.get_plate_gap(), ROW_GAP), "a plate is no spaced row");

    assert!(near(probe.get_plate_key_height(), PLATE_KEY), "the .plate key is {PLATE_KEY} tall: {}", probe.get_plate_key_height());
    assert!(near(probe.get_plate_height(), probe.get_plate_key_height()), "the socket is as tall as its keys, no inset");
    assert!(near(probe.get_plate_radius(), PLATE_RADIUS), "the socket follows the small cap's corner: {}", probe.get_plate_radius());
    assert!(near(probe.get_mini_radius(), MINI_RADIUS), "the mini socket follows the mini cap's corner: {}", probe.get_mini_radius());
    assert!(near(probe.get_row_radius(), 0.0), "a spaced row draws no socket");
    // Twins: each variant's radius tells the other's apart.
    assert!(!near(probe.get_mini_radius(), PLATE_RADIUS), "a mini plate is not rounded as a small one");
    assert!(!near(probe.get_plate_radius(), MINI_RADIUS), "a small plate is not rounded as a mini one");
}
