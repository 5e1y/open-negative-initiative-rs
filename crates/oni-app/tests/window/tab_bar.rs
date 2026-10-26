//! The tab bar on the headless window: a click only asks, the owner writes the choice, so a bar bound
//! one way to its owner's value, as Settings' language, still follows it after a click; a choice
//! beyond its tabs shows no pill.

use oni_app::FieldProbe;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn a_tab_bar_bound_one_way_follows_its_owner_after_a_click() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = FieldProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 400));
    probe.show().unwrap();
    rig.drawn();
    assert_eq!(probe.get_shown(), 0);

    // A click on the third tab asks for it and moves nothing: the owner has not written it.
    let at = LogicalPosition::new(probe.get_left() + probe.get_bar_width() * 5.0 / 6.0, probe.get_bar_y() + 10.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
    assert_eq!(probe.get_picked(), 2, "the click asks for the third tab");
    assert_eq!(probe.get_shown(), 0, "the bar shows the owner's value, not the click's");

    // The owner writes another value: the binding still holds, so the bar follows it.
    probe.set_choice(1);
    rig.pump();
    assert_eq!(probe.get_shown(), 1, "after a click the bar still follows its owner");
    // Twin: the owner writing the asked tab moves the bar there.
    probe.set_choice(2);
    rig.pump();
    assert_eq!(probe.get_shown(), 2);

    // A selection beyond the tabs, as De-dust's, shows no pill, which waits where it stood.
    let at = probe.get_pill_x();
    assert!(probe.get_pill_shown());
    probe.set_choice(3);
    rig.settle();
    assert!(!probe.get_pill_shown(), "no tab is selected, so no pill shows");
    assert_eq!(probe.get_pill_x(), at, "the pill does not slide past the last tab");
    // Twin: back on a tab, the pill shows under it.
    probe.set_choice(0);
    rig.settle();
    assert!(probe.get_pill_shown() && probe.get_pill_x() < at, "the pill comes back under the first tab");
}
