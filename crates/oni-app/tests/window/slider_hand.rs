//! A slider's drag ends with its owners: a walk under the hand lands the drag on the frame it was on,
//! and the button lifted ends a drag whose slider was taken away, which Slint tells nothing.

use oni_app::PanelTab;
use oni_app::app::Shell;
use oni_app::fields::Field;
use oni_app::{Library, Menus, PanelState};
use oni_settings::sidecar;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::path::Path;
use std::time::Duration;

use crate::common;

fn exposure(path: &Path) -> f32 {
    Field::Exposure.knob().get(&sidecar::read(path, false).unwrap().settings)
}

/// A and B in one folder, A open in the editor on the tab holding Exposure, and the rail's point.
fn editor(name: &str) -> Option<(common::App, (f32, f32))> {
    let root = common::Root::new(name);
    let folder = root.scans();
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    // Tall enough for Lighting under the Colour tab's Colour and Paper sections.
    let app = common::launch(root).size(1280, 1000).show()?;
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    app.read(&folder);
    let lib = shell.window.global::<Library>();
    lib.invoke_tile_opened(at(shell, "A.tif"));
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let w = &shell.window;
    w.global::<PanelState>().set_tab(PanelTab::Color);
    w.global::<PanelState>().invoke_tab_picked(PanelTab::Color);
    rig.settle();
    // Exposure's rail, the knob's row at the slider's foot, right of its mid-track rest so no drag meets its detent.
    let slider = common::rail(w, "Exposure");
    assert!(slider.y + slider.height < 1000.0, "Exposure's rail in the window: {slider:?}");
    let (x, y) = (slider.x + slider.width * 0.6, slider.y + slider.height - 13.0);
    moved(rig, x, y);
    Some((app, (x, y)))
}

/// The place of the photo named `name` in the order a tile opens by.
fn at(shell: &Shell, name: &str) -> i32 {
    shell.ctl.state.borrow().order().iter().position(|p| p.ends_with(name)).unwrap() as i32
}

fn moved(rig: &common::Rig, x: f32, y: f32) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.pump();
}

fn button(rig: &common::Rig, (x, y): (f32, f32), down: bool) {
    let (position, button) = (LogicalPosition::new(x, y), PointerEventButton::Left);
    rig.window.dispatch_event(if down { WindowEvent::PointerPressed { position, button } } else { WindowEvent::PointerReleased { position, button } });
    rig.pump();
}

pub(crate) fn a_walk_under_a_drag_leaves_the_drag_on_its_frame() {
    let Some((app, (x, y))) = editor("slider-walk") else { return };
    let common::App { rig, shell, root, .. } = &app;
    let ctl = shell.ctl.clone();
    let open = || oni_app::store::get(ctl.state.borrow().store(), Field::Exposure);
    let (a, b) = (root.join("Scans/A.tif"), root.join("Scans/B.tif"));
    let a0 = open();
    button(rig, (x, y), true);
    moved(rig, x + 40.0, y);
    assert!(ctl.state.borrow().dragging() && ctl.state.borrow().hands() == 1, "the press holds a hand");

    // B opened under the hand: the drag lands on A, recorded and written as A leaves.
    shell.window.global::<Library>().invoke_tile_opened(at(shell, "B.tif"));
    rig.pump();
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("B.tif")));
    assert!((exposure(&a) - a0).abs() > 1e-3, "A's sidecar holds the drag: {} against {a0}", exposure(&a));
    assert_eq!((ctl.state.borrow().dragging(), ctl.state.borrow().hands()), (false, 0), "the walk lifts the hand");
    let b0 = open();
    moved(rig, x + 60.0, y);
    button(rig, (x + 60.0, y), false);
    assert_eq!(open(), b0, "the old hand's travel reaches nothing of B");
    std::thread::sleep(oni_app::app::WRITE_DELAY + Duration::from_millis(100));
    rig.settle();
    assert_eq!(exposure(&b), b0, "nor its sidecar");

    // Twin: the same press and travel with no walk moves the open frame, so the late drag was live.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    button(rig, (x, y), true);
    moved(rig, x + 40.0, y);
    moved(rig, x + 60.0, y);
    button(rig, (x + 60.0, y), false);
    assert!((open() - b0).abs() > 1e-3, "twin: the drag moves B's exposure");
    assert_eq!((ctl.state.borrow().dragging(), ctl.state.borrow().hands()), (false, 0));
}

pub(crate) fn a_slider_taken_away_under_the_hand_lets_go_at_the_button() {
    let Some((app, (x, y))) = editor("slider-taken") else { return };
    let common::App { rig, shell, root, .. } = &app;
    let ctl = shell.ctl.clone();
    let a = root.join("Scans/A.tif");
    let a0 = oni_app::store::get(ctl.state.borrow().store(), Field::Exposure);
    button(rig, (x, y), true);
    moved(rig, x + 40.0, y);
    // ⌥⌘] takes the panel and the slider under the hand away; the button then rises over the canvas.
    ctl.act(oni_app::Act::HidePanel);
    rig.pump();
    button(rig, (x + 40.0, y), false);
    // Twin: Slint told no one, so the drag and its hand stand until the window's hook lifts them.
    assert_eq!((ctl.state.borrow().dragging(), ctl.state.borrow().hands()), (true, 1), "the destroyed slider sent no release");
    ctl.lifted();
    assert_eq!((ctl.state.borrow().dragging(), ctl.state.borrow().hands()), (false, 0), "the lifted button ends the drag");
    let a1 = oni_app::store::get(ctl.state.borrow().store(), Field::Exposure);
    assert!((a1 - a0).abs() > 1e-3, "the hand's value landed: {a1} against {a0}");
    assert_eq!(shell.window.global::<Menus>().get_titles().undo.as_str(), "Undo Exposure", "one step, recorded");
    rig.until("A's sidecar written", Duration::from_secs(5), || (exposure(&a) - a1).abs() < 1e-5);
}
