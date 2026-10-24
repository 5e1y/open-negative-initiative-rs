//! The text fields as AppKit's on the headless window: a field entered by a click or by Tab selects its
//! whole value, so a key typed replaces it; once in, a click places the caret, a double click selects a
//! word and a triple click the line.

use oni_app::FieldProbe;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

const VALUE: &str = "alpha beta gamma";

/// Clicks `count` times in a row at one spot, as quick as a hand's double or triple click.
fn clicks(rig: &common::Rig, x: f32, y: f32, count: usize) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    for _ in 0..count {
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
        rig.pump();
    }
}

/// Past the platform's double-click interval, so the next click counts as a first one.
fn pause(rig: &common::Rig) {
    std::thread::sleep(Duration::from_millis(600));
    rig.pump();
}

pub(crate) fn a_field_entered_selects_its_value_and_a_double_click_a_word() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = FieldProbe::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 300));
    probe.show().unwrap();
    rig.drawn();
    // At the first word's second letter, inside the field's padding plus a few points.
    let x = probe.get_left() + 14.0;
    let one = probe.get_one_y() + probe.get_field_height() / 2.0;
    probe.set_first(VALUE.into());

    // A click into the unfocused field selects all of it: the key typed replaces the whole value.
    clicks(&rig, x, one, 1);
    rig.key("X");
    assert_eq!(probe.get_first(), "X", "a click entering the field selects its whole value");

    // Twin: once in, a click places the caret, and the key typed goes in beside the value.
    probe.set_first(VALUE.into());
    pause(&rig);
    clicks(&rig, x, one, 1);
    rig.key("Y");
    let typed = probe.get_first().to_string();
    assert_eq!(typed.len(), VALUE.len() + 1, "a click in a focused field selects nothing: {typed}");
    assert!(typed.contains("lpha beta gamma") || typed.starts_with('Y'), "the value stays around the caret: {typed}");

    // A double click selects the word under it alone.
    probe.set_first(VALUE.into());
    pause(&rig);
    clicks(&rig, x, one, 2);
    rig.key("Z");
    assert_eq!(probe.get_first(), "Z beta gamma", "a double click selects the word");

    // A triple click selects the line.
    probe.set_first(VALUE.into());
    pause(&rig);
    clicks(&rig, x, one, 3);
    rig.key("W");
    assert_eq!(probe.get_first(), "W", "a triple click selects the line");

    // Tab enters the next field with its whole value selected, as a click does.
    probe.set_second("one two".into());
    rig.key(&slint::SharedString::from(Key::Tab));
    rig.key("V");
    assert_eq!(probe.get_second(), "V", "Tab into a field selects its whole value");
    assert_eq!(probe.get_first(), "W", "the field left keeps its value");

    // The multiline field too, on a click.
    probe.set_lines("one line\nanother".into());
    pause(&rig);
    clicks(&rig, x, probe.get_three_y() + 12.0, 1);
    rig.key("U");
    assert_eq!(probe.get_lines(), "U", "a click entering the multiline field selects all of it");
    rig.shot("text-fields");
}
