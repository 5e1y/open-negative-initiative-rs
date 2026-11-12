//! The tab bar on the headless window: a click only asks, the owner writes the choice, so a bar bound
//! one way to its owner's value, as Settings' language, still follows it after a click; a choice
//! beyond its tabs shows no pill; every label keeps the same room either side, a long one included.

use oni_app::FieldProbe;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn a_tab_bar_bound_one_way_follows_its_owner_after_a_click() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
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

    // A selection beyond the tabs shows no pill, which waits where it stood.
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

    // A tab is its label and an equal share of the rest: One and Three keep the same room either side.
    let (one, one_room) = (probe.get_pill_width(), probe.get_pill_room());
    probe.set_choice(2);
    rig.settle();
    let (three, three_room) = (probe.get_pill_width(), probe.get_pill_room());
    assert!((three_room - one_room).abs() < 1.0, "every label keeps the same room ({one_room} against {three_room})");
    assert!(one_room >= 12.0, "a bar with room gives each label at least 12 pt: {one_room}");
    // Twin: an even share of the bar leaves the longer label less room, which the measure sees.
    let even = (probe.get_bar_width() - 2.0 * 4.0 - 2.0 * 1.0) / 3.0;
    let (even_one, even_three) = ((even - (one - 2.0 * one_room)) / 2.0, (even - (three - 2.0 * three_room)) / 2.0);
    assert!((even_one - even_three).abs() > 1.0, "an even share's rooms differ ({even_one} against {even_three})");

    // Export's spaces, tight: Adobe RGB's tab outgrows an even share, sRGB's gives room, and both labels
    // keep the same room, past the floor of 8 pt.
    let share = (probe.get_space_width() - 2.0 * 4.0 - 4.0 * 1.0) / 5.0;
    probe.set_space(2);
    rig.settle();
    let (long, long_room) = (probe.get_space_pill_width(), probe.get_space_room());
    assert!(long > share + 1.0, "Adobe RGB's tab is wider than an even share ({long} against {share})");
    assert!(long_room > 9.0, "the long label keeps more than the floor: {long_room}");
    // Twin: sRGB gives room, so its tab is narrower than an even share.
    probe.set_space(0);
    rig.settle();
    assert!(probe.get_space_pill_width() < share - 1.0, "sRGB's tab gives room to the long label");
    assert!((probe.get_space_room() - long_room).abs() < 1.0, "sRGB keeps Adobe RGB's room ({} against {long_room})", probe.get_space_room());
}
