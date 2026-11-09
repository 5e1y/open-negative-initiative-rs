//! The keyboard's one model: Tab walks every control in reading order, each answering its own keys; a
//! press gives the keyboard back to the photos, a holder gone hands it on, a sheet keeps Tab inside.

use oni_app::KeyboardProbe;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, SharedString};
use std::path::PathBuf;

use crate::common;

fn probe() -> (common::Rig, KeyboardProbe) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = KeyboardProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    oni_app::slider_track::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 400));
    probe.show().unwrap();
    rig.drawn();
    (rig, probe)
}

/// A key as winit hands it over: the window's key hook first, then Slint.
fn key(rig: &common::Rig, p: &KeyboardProbe, text: &str) {
    oni_app::outside::keyed(p.window(), || p.invoke_refocus());
    rig.key(text);
}

fn backtab(rig: &common::Rig, p: &KeyboardProbe) {
    oni_app::outside::keyed(p.window(), || p.invoke_refocus());
    rig.chord(Key::Shift, "\t");
}

/// A press as winit hands it over, the window's pointer hook first when `hooked`.
fn press(rig: &common::Rig, p: &KeyboardProbe, x: f32, y: f32, hooked: bool) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    if hooked {
        oni_app::outside::pressed(p);
    }
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// Whether a focus ring is drawn anywhere in the probe.
fn ringed(p: &KeyboardProbe) -> bool {
    i_slint_backend_testing::ElementHandle::find_by_element_type_name(p, "FocusRing").next().is_some()
}

pub(crate) fn tab_walks_every_control_in_reading_order_each_answering_its_own_keys() {
    let (rig, p) = probe();
    let right: SharedString = Key::RightArrow.into();
    let left: SharedString = Key::LeftArrow.into();
    let up: SharedString = Key::UpArrow.into();
    assert!(p.get_monitor_focused() && p.get_held(), "the photos hold the keyboard first");
    // Twin of the key's own Space: held by the monitor, Space reaches the photos, not the key.
    key(&rig, &p, " ");
    assert_eq!((p.get_heard().as_str(), p.get_clicks()), (" ", 0));

    key(&rig, &p, "\t");
    assert!(!p.get_monitor_focused() && p.get_held(), "Tab passes over the monitor to the first key");
    key(&rig, &p, " ");
    assert_eq!(p.get_clicks(), 1, "Space presses the key holding the keyboard");
    key(&rig, &p, "x");
    assert_eq!(p.get_heard().as_str(), "x", "a key the key does not answer reaches the photos");
    rig.shot("keyboard-key");

    key(&rig, &p, "\t");
    key(&rig, &p, &right);
    key(&rig, &p, &right);
    key(&rig, &p, &left);
    assert_eq!(p.get_tab(), 1, "the arrows pick the tab beside");
    assert_eq!(p.get_clicks(), 1, "the key no longer holds the keyboard");

    key(&rig, &p, "\t");
    assert!(ringed(&p), "Tab rings the slider it gives the keyboard to");
    key(&rig, &p, &right);
    key(&rig, &p, &up);
    rig.chord(Key::Shift, &right);
    key(&rig, &p, &left);
    assert_eq!(p.get_steps(), 11, "an arrow is a step, ⇧ ten");
    assert_eq!(p.get_tab(), 1, "the tabs no longer hold the keyboard");
    rig.shot("keyboard-slider");

    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert!(p.get_on(), "Space flips the switch");

    key(&rig, &p, "\t");
    key(&rig, &p, "a");
    assert!(p.get_editing() && !p.get_held(), "the field holds the keyboard, its keys its own");
    assert_eq!(p.get_text().as_str(), "a");

    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert_eq!(p.get_clicks(), 2, "Tab wraps round to the first key, over the monitor");
    assert!(p.get_held(), "a key holding the keyboard lets the bare keys through");
    backtab(&rig, &p);
    key(&rig, &p, "b");
    assert_eq!(p.get_text().as_str(), "ab", "⇧Tab walks back to the field");
}

pub(crate) fn a_press_gives_the_keyboard_back_to_the_photos_and_a_key_whose_holder_is_gone_reaches_them() {
    let (rig, p) = probe();
    let (x, y) = (p.get_tabs_x(), p.get_tabs_y());
    // Twin: a press Slint alone sees leaves the keyboard with the key, as no hook told the window.
    key(&rig, &p, "\t");
    press(&rig, &p, x, y, false);
    key(&rig, &p, " ");
    assert_eq!(p.get_clicks(), 1, "unhooked, the key still holds the keyboard");
    // Through the window's pointer hook, a press on any control gives the keyboard back to the photos.
    press(&rig, &p, x, y, true);
    assert!(p.get_monitor_focused(), "the photos hold the keyboard again");
    key(&rig, &p, " ");
    assert_eq!((p.get_clicks(), p.get_heard().as_str()), (1, " "), "Space reaches the photos, not the key");

    // The key holding the keyboard taken away: Slint would drop the next key; the hook hands it on.
    key(&rig, &p, "\t");
    p.set_keyed(false);
    rig.pump();
    rig.key("y");
    assert_ne!(p.get_heard().as_str(), "y", "twin: unhooked, the key went nowhere");
    key(&rig, &p, "z");
    assert!(p.get_monitor_focused() && p.get_heard() == "z", "the monitor takes the key whose holder is gone");
}

pub(crate) fn a_sheet_keeps_tab_inside_it_while_it_covers_the_window() {
    let (rig, p) = probe();
    p.set_covered(true);
    rig.pump();
    // Tab from the window enters the sheet at its first key, never a control behind it.
    key(&rig, &p, "\t");
    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert_eq!(p.get_answered(), 1, "Tab enters the sheet at its first key");
    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert_eq!(p.get_answered(), 2);
    // Past its last key, Tab meets the window's wall and comes back to its first.
    key(&rig, &p, "\t");
    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert_eq!(p.get_answered(), 1, "Tab comes round to the sheet's first key");
    assert_eq!(p.get_clicks(), 0, "no key behind the sheet took the keyboard");
    // ⇧Tab from its first key goes round to its last, through the window's other wall.
    backtab(&rig, &p);
    backtab(&rig, &p);
    key(&rig, &p, " ");
    assert_eq!(p.get_answered(), 2, "⇧Tab comes round to the sheet's last key");
    rig.shot("keyboard-sheet");
    key(&rig, &p, &SharedString::from(Key::Escape));
    assert!(!p.get_covered(), "Escape from a key of the sheet closes it");
    // Twin: uncovered, Tab from the photos reaches the window's key.
    p.invoke_refocus();
    key(&rig, &p, "\t");
    key(&rig, &p, " ");
    assert_eq!(p.get_clicks(), 1, "uncovered, the window's key takes Tab");
}

pub(crate) fn a_slider_worked_by_hand_leaves_the_arrows_to_the_photos() {
    let (rig, p) = probe();
    let right: SharedString = Key::RightArrow.into();
    let escape: SharedString = Key::Escape.into();
    let (x, y) = (p.get_knob_x(), p.get_knob_y());
    // A drag Slint alone sees, no hook handing the keyboard on: the slider takes none, the photos hear the arrow.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: LogicalPosition::new(x, y), button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x + 30.0, y) });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(x + 30.0, y), button: PointerEventButton::Left });
    rig.pump();
    let dragged = p.get_steps();
    assert!(p.get_monitor_focused() && !ringed(&p), "the photos keep the keyboard, nothing ringed");
    key(&rig, &p, &right);
    assert_eq!((p.get_steps(), p.get_heard()), (dragged, right.clone()), "the arrow walks the photos");
    // Twin: Tab gives the slider the keyboard, ringed, and its arrows step it.
    for _ in 0..3 {
        key(&rig, &p, "\t");
    }
    assert!(!p.get_monitor_focused() && ringed(&p), "Tab rings the slider holding the keyboard");
    key(&rig, &p, &right);
    assert_eq!(p.get_steps(), dragged + 1, "its arrows step it");
    rig.shot("keyboard-slider-tabbed");
    // Escape hands the keyboard back to the photos, which hear the next arrow.
    key(&rig, &p, &escape);
    assert!(p.get_monitor_focused(), "Escape gives the keyboard back to the photos");
    key(&rig, &p, &right);
    assert_eq!(p.get_steps(), dragged + 1);
    // A press beside the slider holding it does as well, through the window's pointer hook.
    for _ in 0..3 {
        key(&rig, &p, "\t");
    }
    press(&rig, &p, x, p.get_bare_y(), true);
    assert!(p.get_monitor_focused(), "a press beside the slider gives the keyboard back");
}

pub(crate) fn a_number_typed_over_a_slider_readout_steps_it_there() {
    let (rig, p) = probe();
    let right: SharedString = Key::RightArrow.into();
    let (x, y) = (p.get_value_x(), p.get_value_y());
    // A click on the value opens it selected: the number typed replaces it, Return takes it.
    press(&rig, &p, x, y, true);
    assert!(p.get_entering(), "the value opens to a number typed over it");
    rig.key("2");
    rig.key("0");
    rig.shot("keyboard-slider-typed");
    rig.key("\n");
    assert_eq!(p.get_steps(), 20, "Return takes the number typed");
    assert!(!p.get_entering());
    // Return gives the photos the keyboard back, even a key Slint alone sees: the arrow walks them.
    rig.key(&right);
    assert!(p.get_monitor_focused() && p.get_heard() == right, "the photos hear the arrow after Return");
    // Twin: Tab gives the slider the keyboard, whose arrows step on from the number typed.
    for _ in 0..3 {
        key(&rig, &p, "\t");
    }
    key(&rig, &p, &right);
    assert_eq!(p.get_steps(), 21);
    // Escape hands the value back unchanged.
    press(&rig, &p, x, y, true);
    rig.key("5");
    rig.key(&SharedString::from(Key::Escape));
    assert_eq!(p.get_steps(), 21, "Escape leaves the value as it was");
    // A press elsewhere takes the number typed, the photos holding the keyboard after it.
    press(&rig, &p, x, y, true);
    rig.key("\u{2212}");
    rig.key("3");
    press(&rig, &p, x, p.get_bare_y(), true);
    assert_eq!(p.get_steps(), -3, "a press elsewhere takes the number typed");
    assert!(p.get_monitor_focused(), "the press gave the photos the keyboard");
    // Twin: words where a number goes move nothing.
    press(&rig, &p, x, y, true);
    rig.key("x");
    rig.key("\n");
    assert_eq!(p.get_steps(), -3);
    // The target is the typed number's box, a small key tall: a click just above the number's line opens it.
    press(&rig, &p, x, y - 16.0, true);
    assert!(!p.get_entering(), "twin: a click past the box opens nothing");
    press(&rig, &p, x, y - 6.0, true);
    assert!(p.get_entering(), "a click above the number, within its box, opens it");
}

pub(crate) fn a_slider_tells_its_double_click_on_its_name_and_its_typed_value_and_nothing_on_its_rail() {
    let (rig, p) = probe();
    let tip = p.global::<oni_app::Tip>();
    let hover = |x: f32, y: f32| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
        rig.pump();
    };
    // The name tells the double click that brings it back to rest, the value the click that types it.
    hover(p.get_name_x(), p.get_value_y());
    assert!(tip.get_shown() && tip.get_text() == "Double-click to bring it back to rest", "name: {}", tip.get_text());
    // The rail under the hand tells nothing, so no note covers the slider while it is worked.
    hover(p.get_knob_x(), p.get_knob_y());
    assert!(!tip.get_shown(), "rail: {}", tip.get_text());
    // The value's face opens under the hand; its figure tells the click at the hand's next move.
    hover(p.get_value_x(), p.get_value_y());
    hover(p.get_value_x() + 1.0, p.get_value_y());
    assert!(tip.get_shown() && tip.get_text() == "Click to type a value", "value: {}", tip.get_text());
    // Twin: beside the slider, nothing is told.
    hover(p.get_value_x(), p.get_bare_y());
    assert!(!tip.get_shown(), "beside: {}", tip.get_text());
}
