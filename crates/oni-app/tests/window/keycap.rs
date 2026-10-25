//! `KeycapPress` on the headless window: a tap holds the cap down `KeycapStyle.minHold`, 0.14 s,
//! however short, and a press held longer lifts at its release.

use oni_app::KeyProbe;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn a_tap_holds_the_cap_down_its_floor_and_a_long_press_lifts_at_once() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = KeyProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(200, 120));
    probe.show().unwrap();
    rig.settle();
    rig.shot("keycap");
    let at = LogicalPosition::new(probe.get_key_x(), probe.get_key_y());
    let press = || {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
        rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
        rig.pump();
    };
    let release = || {
        rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
        rig.pump();
    };
    assert!(!probe.get_key_down());
    press();
    assert!(probe.get_key_down(), "the press takes the cap down");
    release();
    assert!(probe.get_key_down(), "a tap leaves it down");
    std::thread::sleep(Duration::from_millis(80));
    rig.pump();
    assert!(probe.get_key_down(), "still inside the floor");
    std::thread::sleep(Duration::from_millis(120));
    rig.pump();
    assert!(!probe.get_key_down(), "lifted once 0.14 s have passed");
    // Twin: a press held past the floor lifts at its release.
    press();
    std::thread::sleep(Duration::from_millis(250));
    rig.pump();
    release();
    assert!(!probe.get_key_down());
}
