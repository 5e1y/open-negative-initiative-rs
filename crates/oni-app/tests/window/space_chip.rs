//! The canvas's colour-space chip by the pointer: its popover opens named as the export's field and
//! shared with it, and each tab inside it takes the click, Custom and sRGB from Display P3 included.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Exporting;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};

use crate::common;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn the_space_chips_tabs_take_the_pointer() {
    let root = common::Root::new("space-chip");
    let folder = root.scans();
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let e = w.global::<Exporting>();
    app.edit_first();
    rig.settle();
    assert_eq!(e.get_chip_title(), "Display P3");
    // The chip where the canvas's band lays it under the bar, found by the words it shows.
    let chip = || {
        let (title, hint) = (e.get_chip_title(), e.get_chip_hint());
        let label = if hint.is_empty() { title.to_string() } else { format!("{title} · {hint}") };
        common::the(w, Role::Button, &label).middle()
    };
    // At rest the chip is its words alone on the band; under the hand its flat face shows. Read inside
    // its padding, off the words, against the band just beside it.
    let label = || {
        let (title, hint) = (e.get_chip_title(), e.get_chip_hint());
        if hint.is_empty() { title.to_string() } else { format!("{title} · {hint}") }
    };
    let bare = || {
        let placed = common::the(w, Role::Button, &label());
        let (width, px) = rig.drawn();
        let y = (placed.y + placed.height / 2.0) as usize;
        px[y * width + (placed.x + 2.0) as usize].0 == px[y * width + (placed.x - 4.0) as usize].0
    };
    assert!(bare(), "the chip bare at rest");
    let (x, y) = chip();
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.settle();
    // Twin: the face under the hand is seen by the same reading.
    assert!(!bare(), "the chip's face under the hand");

    // The chip's title opens the popover, which stays open while its tabs are picked.
    click(rig, x, y);
    rig.settle();
    // The export panel's name, and the choice said to be the export's; twin: the former name is gone.
    common::in_popup(w, Role::Text, "Colour space");
    common::in_popup(w, Role::Text, "Shared by the canvas and the export.");
    assert!(common::placed(w, Role::Text, "Working Space").is_empty());
    rig.shot("space-chip");
    // Each tab where the popover lays it, found anew: a press on one leaves the popover open.
    let tab = |rig: &common::Rig, name: &str| {
        let (x, y) = common::in_popup(w, Role::Tab, name).middle();
        click(rig, x, y);
        rig.settle();
        (x, y)
    };
    tab(rig, "sRGB");
    assert_eq!(e.get_chip_title(), "sRGB", "sRGB from Display P3");
    tab(rig, "P3");
    assert_eq!(e.get_chip_title(), "Display P3");
    // Last: a longer title widens the chip into its room, which the popover under it follows.
    tab(rig, "Custom");
    assert_eq!(e.get_space_tab(), 4, "Custom from Display P3");
    // Twin: a click beside the popover closes it and picks nothing.
    let (x, y) = common::in_popup(w, Role::Tab, "sRGB").middle();
    let at = chip();
    click(rig, at.0, at.1);
    rig.settle();
    click(rig, 600.0, 600.0);
    rig.settle();
    click(rig, x, y);
    assert_eq!(e.get_space_tab(), 4, "the popover closed, the tabs under it gone");
}
