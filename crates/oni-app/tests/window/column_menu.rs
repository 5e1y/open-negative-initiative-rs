//! A hidden column slid over the card by the pointer stays while a menu it opened holds the pointer,
//! and slides away once the pointer meets the card beside it, as it does when no menu ever opened.

use oni_app::{Library, MainWindow};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, VecModel};
use std::rc::Rc;

use crate::common;
use crate::panel_bindings::opened;

fn moved(rig: &common::Rig, x: f32, y: f32) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.pump();
}

/// A press and its release as winit hands them over: the window's pointer hook first, then Slint.
fn pressed(rig: &common::Rig, w: &MainWindow, x: f32, y: f32, button: PointerEventButton) {
    let position = LogicalPosition::new(x, y);
    moved(rig, x, y);
    oni_app::outside::pressed(w);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button });
    rig.pump();
}

/// The library slid in from the card's edge and the pointer laid on it.
fn revealed(rig: &common::Rig, w: &MainWindow) {
    let edge = (0..24).map(|x| x as f32).find(|&x| {
        moved(rig, x, 300.0);
        w.get_revealed_library()
    });
    assert!(edge.is_some(), "the card's edge reveals the hidden library");
    rig.settle();
    moved(rig, 120.0, 300.0);
}

pub(crate) fn a_revealed_column_stays_while_its_menu_holds_the_pointer() {
    let Some((rig, shell, root)) = opened("column-menu") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    ctl.act(oni_app::Act::HideLibrary);
    rig.settle();

    // Twin first: with no press in it, the column slides away as soon as the pointer meets the card.
    revealed(&rig, w);
    assert!(w.get_revealed_library());
    moved(&rig, 700.0, 400.0);
    assert!(!w.get_revealed_library(), "left for the card, the column hides");

    // A right click on a row opens its menu at the pointer; the pointer then moves onto the menu.
    revealed(&rig, w);
    let row = (60..400).map(|i| i as f32 * 2.0).find(|&y| {
        lib.set_menu_lines(ModelRc::new(Rc::new(VecModel::default())));
        pressed(&rig, w, 120.0, y, PointerEventButton::Right);
        lib.get_menu_lines().row_count() > 0
    });
    let y = row.expect("a row of the revealed library offers a menu");
    moved(&rig, 140.0, y + 14.0);
    assert!(w.get_revealed_library(), "the column stays while its menu holds the pointer");
    rig.settle();
    assert!(w.get_revealed_library(), "and stays put");

    // A click on the card closes the menu; the pointer met on the card, the column slides away.
    pressed(&rig, w, 700.0, 400.0, PointerEventButton::Left);
    moved(&rig, 702.0, 402.0);
    assert!(!w.get_revealed_library(), "the menu closed, the pointer on the card hides the column");

    // Twin: a menu closed with the pointer back on the column keeps it, until the pointer leaves it.
    revealed(&rig, w);
    lib.set_menu_lines(ModelRc::new(Rc::new(VecModel::default())));
    pressed(&rig, w, 120.0, y, PointerEventButton::Right);
    assert!(lib.get_menu_lines().row_count() > 0, "the row's menu again");
    moved(&rig, 140.0, y + 14.0);
    // Left of the menu, which opens rightward from the pointer, and still on the column.
    pressed(&rig, w, 24.0, y, PointerEventButton::Left);
    moved(&rig, 26.0, y + 2.0);
    assert!(w.get_revealed_library(), "the pointer back on the column keeps it");
    moved(&rig, 700.0, 400.0);
    assert!(!w.get_revealed_library(), "left for the card, it hides");
    let _ = std::fs::remove_dir_all(&root);
}
