//! A key's tooltip goes as AppKit's does: a press hides it, and a key whose help changed under the
//! hand hides the tip it showed when left, so no old tooltip stays floating over the window.

use oni_app::{App, Tip};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};

use crate::common;
use crate::panel_bindings::opened;

/// Whether the tooltip shown, if any, is the one whose text starts so.
fn showing(tip: &Tip, start: &str) -> bool {
    tip.get_shown() && tip.get_text().starts_with(start)
}

fn moved(rig: &common::Rig, x: f32, y: f32) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.pump();
}

pub(crate) fn a_tooltip_hides_on_a_press_and_with_the_help_it_showed() {
    let Some((rig, shell, root)) = opened("tooltip") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let tip = w.global::<Tip>();
    // The mode key, found along the bar under the title bar by the tooltip it shows beside a photo.
    let y = 60.0;
    let x = (0..400).map(|i| i as f32 * 2.0).find(|&x| {
        moved(&rig, x, y);
        showing(&tip, "Gallery (")
    });
    let x = x.expect("the mode key's tooltip along the bar");

    // A click hides the tooltip; the help the key now shows is not the one it showed.
    let position = LogicalPosition::new(x, y);
    oni_app::outside::pressed(w);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.settle();
    assert!(w.global::<App>().get_gallery(), "the key went to the gallery");
    assert!(!tip.get_shown(), "the press hides the tooltip");
    moved(&rig, x, 400.0);
    assert!(!showing(&tip, "Gallery (") && !showing(&tip, "Editor ("), "left, the key leaves no tooltip behind");
    // Twin: hovered again, the key shows its new help.
    moved(&rig, x, y);
    assert!(showing(&tip, "Editor ("), "{}", tip.get_text());

    // The help changed under the hand without a press, by the menu's ⌘G: the old tip goes.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    assert!(!tip.get_shown(), "a key whose help changed hides the tip it showed");
    moved(&rig, x, 400.0);
    moved(&rig, x, y);
    assert!(showing(&tip, "Gallery ("), "twin: {}", tip.get_text());
    moved(&rig, x, 400.0);
    assert!(!showing(&tip, "Gallery ("), "twin: left, the key hides the tip it showed");
    let _ = std::fs::remove_dir_all(&root);
}
