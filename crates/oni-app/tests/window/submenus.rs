//! A submenu as AppKit opens it: on the pointer resting on its line, at once on a click or →, closed by
//! another line or ←, kept open by a pointer heading for it, turned back at the window's edge.

use oni_app::SubmenuProbe;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

/// Longer than `MenuStyle.sub-delay`, the pointer's rest before a submenu opens or closes.
const REST: Duration = Duration::from_millis(400);

fn probe() -> (common::Rig, SubmenuProbe) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = SubmenuProbe::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(800, 500));
    probe.show().unwrap();
    rig.drawn();
    (rig, probe)
}

fn wait(rig: &common::Rig, span: Duration) {
    let start = Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn to(rig: &common::Rig, (x, y): (f32, f32)) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.pump();
}

fn click(rig: &common::Rig, (x, y): (f32, f32)) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// The middle of the menu's line `i`, left part.
fn line(p: &SubmenuProbe, i: usize) -> (f32, f32) {
    (p.get_line_x(), p.get_line_top() + (i as f32 + 0.5) * p.get_line_height())
}

fn drop(rig: &common::Rig, p: &SubmenuProbe, x: f32, y: f32) {
    if p.get_menu_open() {
        click(rig, (790.0, 490.0));
    }
    p.invoke_drop(x, y);
    rig.pump();
    assert!(p.get_menu_open());
}

/// Moves in `steps` from `from` to `to`, a step every `pace`, as a hand sweeps.
fn sweep(rig: &common::Rig, from: (f32, f32), to_: (f32, f32), steps: usize, pace: Duration) {
    for k in 1..=steps {
        let f = k as f32 / steps as f32;
        to(rig, (from.0 + (to_.0 - from.0) * f, from.1 + (to_.1 - from.1) * f));
        wait(rig, pace);
    }
}

pub(crate) fn a_submenu_opens_on_the_pointer_at_rest_and_closes_on_another_line() {
    let (rig, p) = probe();
    drop(&rig, &p, 40.0, 40.0);

    // Twin: a plain line under the resting pointer opens nothing.
    to(&rig, line(&p, 0));
    wait(&rig, REST);
    assert_eq!((p.get_open(), p.get_asked()), (-1, 0), "a plain line opens no submenu");
    assert_eq!(p.get_lit(), 0, "the line under the pointer is lit");

    // An opener's submenu opens once the pointer rests on it, without a click, and not before.
    to(&rig, line(&p, 1));
    assert_eq!(p.get_open(), -1, "the submenu waits for the pointer to rest");
    wait(&rig, REST);
    assert_eq!(p.get_open(), 1, "resting on an opener opens its submenu");
    assert!(p.get_sub_shown());
    assert!(!p.get_turned(), "room on the right: the submenu sits right of its menu");
    assert_eq!(p.get_sub_left(), 40.0 + p.get_span(), "beside the menu");
    assert!((p.get_sub_top() - (40.0 + p.get_line_height())).abs() < 1.0, "its first line level with its opener");
    assert_eq!(p.get_chosen(), -1, "opening chose nothing");

    // Another opener swaps it; a plain line closes it.
    to(&rig, line(&p, 4));
    wait(&rig, REST);
    assert_eq!(p.get_open(), 4, "the next opener's submenu replaces it");
    to(&rig, line(&p, 3));
    wait(&rig, REST);
    assert_eq!(p.get_open(), -1, "a plain line at rest closes the submenu");
    assert!(!p.get_sub_shown());
    assert!(p.get_menu_open(), "the menu stays");

    // A line of the submenu picks it and closes the menu.
    to(&rig, line(&p, 1));
    wait(&rig, REST);
    to(&rig, (p.get_sub_left() + 40.0, line(&p, 1).1));
    click(&rig, (p.get_sub_left() + 40.0, line(&p, 2).1));
    assert_eq!(p.get_sub_chosen(), 1, "a click in the submenu picks its line");
    assert!(!p.get_menu_open());
}

pub(crate) fn a_pointer_heading_for_the_submenu_keeps_it_open_across_other_lines() {
    let (rig, p) = probe();
    drop(&rig, &p, 40.0, 40.0);
    let pace = Duration::from_millis(40);
    // From the opener's right part, down across Middle, into the submenu's third line: 480 ms.
    let edge = 40.0 + p.get_span();
    let from = (edge - 60.0, line(&p, 1).1);
    let into = (edge + 20.0, line(&p, 3).1);
    let away = (edge - 140.0, line(&p, 3).1);
    let open = |p: &SubmenuProbe| {
        to(&rig, from);
        wait(&rig, REST);
        assert_eq!(p.get_open(), 1);
    };

    open(&p);
    sweep(&rig, from, into, 12, pace);
    assert_eq!(p.get_open(), 1, "a diagonal towards the submenu keeps it open over the lines it crosses");
    wait(&rig, REST);
    assert_eq!((p.get_open(), p.get_sub_lit()), (1, 2), "and the line it lands on in the submenu is lit");

    // Twin: the same sweep at the same pace, mirrored away from the submenu, closes it on the way.
    open(&p);
    sweep(&rig, from, away, 12, pace);
    assert_eq!(p.get_open(), -1, "a sweep away lets the delay close it over Middle");

    // Twin: the diagonal stopped on Middle closes it once the pointer rests there.
    open(&p);
    sweep(&rig, from, (from.0 + (into.0 - from.0) * 0.5, from.1 + (into.1 - from.1) * 0.5), 6, pace);
    assert_eq!(p.get_lit(), 1, "crossing lines towards the submenu leaves the opener lit");
    wait(&rig, REST);
    assert_eq!((p.get_open(), p.get_lit()), (-1, 2), "the diagonal stopped on Middle closes the submenu");
}

pub(crate) fn a_click_or_the_arrow_opens_a_submenu_at_once_and_it_turns_at_the_window_s_edge() {
    let (rig, p) = probe();
    drop(&rig, &p, 40.0, 40.0);

    // A click opens it at once.
    click(&rig, line(&p, 4));
    assert_eq!(p.get_open(), 4, "a click on an opener opens its submenu with no wait");
    assert_eq!(p.get_chosen(), -1, "and chooses nothing");
    // Twin: a click on a plain line chooses it, opening nothing.
    click(&rig, line(&p, 3));
    assert_eq!((p.get_chosen(), p.get_asked()), (3, 1), "a plain line's click chooses it");

    // → opens the lit opener's submenu and lights its first line; ← closes it; Return picks.
    drop(&rig, &p, 40.0, 40.0);
    let down: slint::SharedString = Key::DownArrow.into();
    let right: slint::SharedString = Key::RightArrow.into();
    let left: slint::SharedString = Key::LeftArrow.into();
    let ret: slint::SharedString = Key::Return.into();
    rig.key(&down);
    rig.key(&right);
    assert_eq!((p.get_lit(), p.get_open()), (0, -1), "twin: → on a plain line opens nothing");
    rig.key(&down);
    rig.key(&right);
    assert_eq!((p.get_open(), p.get_sub_lit()), (1, 0), "→ on an opener opens its submenu on its first line");
    rig.key(&down);
    assert_eq!(p.get_sub_lit(), 1, "the arrows walk the submenu once in it");
    rig.key(&left);
    assert_eq!((p.get_open(), p.get_lit()), (-1, 1), "← closes the submenu, its opener still lit");
    rig.key(&right);
    rig.key(&ret);
    assert_eq!(p.get_sub_chosen(), 0, "Return picks the submenu's lit line");

    // At the window's right edge the submenu turns to the menu's left, the menu staying put.
    let edge = 800.0 - p.get_span() - 20.0;
    drop(&rig, &p, edge, 40.0);
    click(&rig, line(&p, 1));
    assert!(p.get_turned(), "no room on the right: the submenu turns left");
    assert_eq!(p.get_sub_left(), edge - p.get_span(), "left of the menu");
    to(&rig, line(&p, 0));
    assert_eq!(p.get_lit(), 0, "the menu's lines stay where they were drawn");
    // Twin: in the window's middle it stays on the right.
    drop(&rig, &p, 200.0, 40.0);
    click(&rig, line(&p, 1));
    assert!(!p.get_turned());
    assert_eq!(p.get_sub_left(), 200.0 + p.get_span());
}
