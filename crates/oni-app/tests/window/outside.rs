//! A press beside a menu or a field, as macOS takes it: a menu closes and the press reaches nothing
//! under it, unless it lands on another picker, which opens; a field ends its editing, a rename
//! commits, and the keys go back to the window's monitor.

use oni_app::OutsideProbe;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// A click as winit hands it over: the window's pointer hook first, then Slint.
fn click(rig: &common::Rig, probe: &OutsideProbe, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    oni_app::outside::pressed(probe);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// Past the platform's double-click interval, so the next click counts as a first one.
fn pause(rig: &common::Rig) {
    std::thread::sleep(Duration::from_millis(600));
    rig.pump();
}

fn typed(rig: &common::Rig, text: &str) {
    for c in text.chars() {
        rig.key(&c.to_string());
    }
}

fn probe() -> (common::Rig, OutsideProbe) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = OutsideProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 400));
    probe.show().unwrap();
    rig.drawn();
    (rig, probe)
}

pub(crate) fn a_menu_closes_on_a_click_beside_it_which_reaches_nothing_under_it() {
    let (rig, p) = probe();
    let (key, first, second, row) = ((p.get_key_x(), p.get_key_y()), p.get_first_x(), p.get_second_x(), p.get_pickers_y());

    // Twin of the swallow: with no menu down the key takes the click.
    click(&rig, &p, key.0, key.1);
    assert_eq!(p.get_clicks(), 1, "the key takes a click while no menu is down");

    // A click beside the menu closes it and reaches nothing under it.
    pause(&rig);
    click(&rig, &p, first, row);
    assert!(p.get_first_open(), "a click on the picker drops its menu");
    click(&rig, &p, key.0, key.1);
    assert!(!p.get_first_open(), "a click beside the menu closes it");
    assert_eq!(p.get_clicks(), 1, "the click that closed the menu reaches nothing under it");

    // Twin: a click inside picks the line under it.
    pause(&rig);
    click(&rig, &p, first, row);
    click(&rig, &p, p.get_line_x(), p.get_line_y());
    assert_eq!(p.get_first_chosen(), 1, "a click inside the menu picks its line");
    assert!(!p.get_first_open(), "the line picked closes the menu");

    // A click on another picker closes the menu and opens that picker's.
    pause(&rig);
    click(&rig, &p, first, row);
    click(&rig, &p, second, row);
    assert!(!p.get_first_open(), "the other picker's click closes the first menu");
    assert!(p.get_second_open(), "and opens its own, as macOS does");

    // Twin: a click on the picker whose menu is down closes it and opens nothing.
    click(&rig, &p, second, row);
    assert!(!p.get_second_open(), "a click on the open picker closes its menu");
    assert!(!p.get_first_open(), "and opens no other");
    assert_eq!(p.get_clicks(), 1, "no click of these reached the key");
    rig.shot("outside-menu");
}

pub(crate) fn a_field_ends_on_a_click_beside_it_and_a_rename_commits() {
    let (rig, p) = probe();
    let (key, x, y) = ((p.get_key_x(), p.get_key_y()), p.get_text_x() + 4.0, p.get_field_y());

    // A click beside a focused field ends its editing; the keys go back to the window's monitor.
    click(&rig, &p, x, y);
    typed(&rig, "abc");
    assert!(p.get_editing(), "a click enters the field");
    assert_eq!(p.get_text(), "abc");
    click(&rig, &p, key.0, key.1);
    assert!(!p.get_editing(), "a click beside the field ends its editing");
    assert!(p.get_monitor_focused(), "the window's monitor holds the keyboard again");
    assert_eq!(p.get_clicks(), 1, "the click beside a field still reaches what it lands on");
    rig.key("x");
    assert_eq!(p.get_heard(), "x", "a key goes to the window's shortcuts");
    assert_eq!(p.get_text(), "abc", "and no longer into the field");

    // Twin: a click inside keeps the focus and moves the caret, so a key goes in beside the words.
    pause(&rig);
    click(&rig, &p, x, y);
    pause(&rig);
    click(&rig, &p, x + 2.0, y);
    assert!(p.get_editing(), "a click inside the field keeps its editing");
    rig.key("Z");
    let text = p.get_text().to_string();
    assert_eq!(text.len(), 4, "the caret placed, the key goes in beside the words: {text}");
    assert_eq!(p.get_heard(), "x", "the monitor heard none of it");

    // A rename commits on a click beside it, as the Finder's does.
    p.set_name("Roll".into());
    p.set_renaming(true);
    rig.pump();
    typed(&rig, "Roll 9");
    click(&rig, &p, key.0, key.1);
    assert!(!p.get_renaming(), "a click beside the name ends the rename");
    assert_eq!(p.get_committed(), "Roll 9", "the name typed is committed");
    assert!(!p.get_cancelled());
    assert!(p.get_monitor_focused(), "the monitor holds the keyboard after the rename");

    // Twin: a click inside keeps the rename, and Escape still gives the old name back.
    p.set_renaming(true);
    rig.pump();
    typed(&rig, "Other");
    pause(&rig);
    click(&rig, &p, x, p.get_rename_y());
    assert!(p.get_renaming(), "a click inside the name keeps the rename");
    rig.key(&slint::SharedString::from(Key::Escape));
    assert!(!p.get_renaming() && p.get_cancelled(), "Escape cancels the rename");
    assert_eq!(p.get_committed(), "Roll 9", "the cancelled name is not committed");
    rig.shot("outside-field");
}

pub(crate) fn a_click_from_one_date_box_to_the_next_keeps_the_date_in_editing() {
    let (rig, p) = probe();
    let (y, month, day, key) = (p.get_date_y(), p.get_month_x(), p.get_day_x(), (p.get_key_x(), p.get_key_y()));

    // February typed over the month, then a click on the day: the three boxes keep one editing, so
    // nothing is committed, as Swift's single focus does.
    click(&rig, &p, month, y);
    assert!(p.get_date_editing(), "a click enters the month");
    typed(&rig, "4");
    pause(&rig);
    click(&rig, &p, day, y);
    assert!(p.get_date_editing(), "the day takes the editing over from the month");
    assert_eq!(p.get_commits(), 0, "moving to the day commits nothing half typed");
    typed(&rig, "15");

    // Twin: a click beside the boxes ends the editing and commits the date once, whole.
    pause(&rig);
    click(&rig, &p, key.0, key.1);
    assert!(!p.get_date_editing(), "a click beside the date ends its editing");
    assert_eq!((p.get_commits(), p.get_date_committed().as_str()), (1, "4/15/1999"));
    rig.shot("outside-date");
}
