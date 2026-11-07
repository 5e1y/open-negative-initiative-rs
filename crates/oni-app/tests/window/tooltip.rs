//! A key's tooltip goes as AppKit's does: a press hides it, a slider's stays down through its drag, a
//! key whose help changed under the hand hides the tip it showed when left, and a new screen or a sheet
//! takes it down with the key it replaced or covered, so no old tooltip stays floating. A slider's covers
//! none of its neighbours.

use i_slint_backend_testing::AccessibleRole as Role;
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
    let y = 60.0;
    let x = mode_key(&rig, &tip, y);

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

/// A press on a slider's rail as winit hands it over, the window's pointer hook first unless `hooked` is
/// false, as a tool's synthetic click; then a drag along the rail and off it, the tooltip read at each step.
fn dragged(rig: &common::Rig, w: &oni_app::MainWindow, (x, y): (f32, f32), hooked: bool) -> Vec<bool> {
    let tip = w.global::<Tip>();
    if hooked {
        oni_app::outside::pressed(w);
    }
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: LogicalPosition::new(x, y), button: PointerEventButton::Left });
    rig.pump();
    let mut seen = vec![tip.get_shown()];
    for (dx, dy) in [(10.0, 0.0), (20.0, 0.0), (30.0, -40.0), (40.0, 40.0), (50.0, 0.0)] {
        moved(rig, x + dx, y + dy);
        seen.push(tip.get_shown());
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(x + 50.0, y), button: PointerEventButton::Left });
    rig.pump();
    seen.push(tip.get_shown());
    seen
}

pub(crate) fn a_slider_s_tooltip_stays_down_through_its_drag() {
    let Some((rig, shell, root)) = opened("tooltip-drag") else { return };
    let w = &shell.window;
    let tip = w.global::<Tip>();
    w.global::<oni_app::PanelState>().set_tab(2);
    w.global::<oni_app::PanelState>().invoke_tab_picked(2);
    rig.settle();
    // Exposure's rail, found down the slider by the tooltip it shows.
    let slider = common::the(w, i_slint_backend_testing::AccessibleRole::Slider, "Exposure");
    let x = slider.x + slider.width / 3.0;
    let rail = (0..slider.height as i32).map(|k| slider.y + k as f32).find(|&y| {
        moved(&rig, x, y);
        showing(&tip, "Double-click to bring it back to rest")
    });
    let y = rail.expect("the rail's tooltip");

    // The real event path: the press takes the tooltip down, and no step of the drag brings it back.
    let seen = dragged(&rig, w, (x, y), true);
    assert!(seen.iter().all(|s| !s), "the tooltip stays down through the drag: {seen:?}");
    // Twin: a click that skips the window's hook, as the evaluation tool's, leaves it standing.
    moved(&rig, x, slider.y - 60.0);
    moved(&rig, x, y);
    assert!(showing(&tip, "Double-click"), "hovered again, the rail tells its help");
    let seen = dragged(&rig, w, (x, y), false);
    assert!(seen.iter().any(|s| *s), "without the hook the tooltip stood: {seen:?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// The mode key along the bar under the title bar, found at `y` by the tooltip it shows beside a photo.
fn mode_key(rig: &common::Rig, tip: &Tip, y: f32) -> f32 {
    let x = (0..400).map(|i| i as f32 * 2.0).find(|&x| {
        moved(rig, x, y);
        showing(tip, "Gallery (")
    });
    x.expect("the mode key's tooltip along the bar")
}

pub(crate) fn a_tooltip_goes_with_the_screen_or_the_sheet_over_its_key() {
    let Some((rig, shell, root)) = opened("tooltip-scene") else { return };
    let w = &shell.window;
    let (tip, app) = (w.global::<Tip>(), w.global::<App>());
    let y = 60.0;
    let x = mode_key(&rig, &tip, y);

    // The menu's Export under the resting hand: the bar leaves with the editor, its tooltip with it,
    // and the hand's next move over the export brings none back.
    app.invoke_menu(oni_app::Act::Export);
    rig.settle();
    assert!(w.global::<oni_app::Exporting>().get_shown(), "the export shows");
    assert!(!tip.get_shown(), "a new screen takes the tooltip down: {}", tip.get_text());
    moved(&rig, x + 300.0, 400.0);
    assert!(!tip.get_shown(), "moved over the export, no tooltip of the bar: {}", tip.get_text());
    // Twin: back beside the photo, the key hovered again tells its help.
    shell.ctl.leave_export();
    rig.settle();
    moved(&rig, x, y);
    assert!(showing(&tip, "Gallery ("), "twin: {}", tip.get_text());

    // A sheet opened over the resting hand covers the key and takes its tooltip down; closed, the
    // hand left and back tells it again.
    app.invoke_menu(oni_app::Act::Settings);
    rig.settle();
    assert!(w.global::<oni_app::AppSettings>().get_shown(), "Settings shows");
    assert!(!tip.get_shown(), "a sheet takes the tooltip down: {}", tip.get_text());
    app.invoke_menu(oni_app::Act::CloseSettings);
    rig.settle();
    assert!(!w.global::<oni_app::AppSettings>().get_shown(), "Settings closed");
    moved(&rig, x, 400.0);
    moved(&rig, x, y);
    assert!(showing(&tip, "Gallery ("), "twin: {}", tip.get_text());
    let _ = std::fs::remove_dir_all(&root);
}

/// Whether two boxes share any point.
fn overlap(a: &common::Placed, b: &common::Placed) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// The tooltip's box in the window's points, where the layer lays it; the headless window draws its menu
/// bar inside, which shifts what it draws, never this box.
fn note(w: &oni_app::MainWindow) -> common::Placed {
    let b = w.get_tip_box();
    common::Placed { x: b.x, y: b.y, width: b.width, height: b.height }
}

/// The hand on `(x, y)`: the tooltip shown, its box on `own` alone and clear of `above` and `below`;
/// twin: the same box hung under the control at `foot`, as it was, lies over the next slider.
fn clear(rig: &common::Rig, w: &oni_app::MainWindow, (x, y): (f32, f32), help: &str, foot: f32, [above, own, below]: [common::Placed; 3]) {
    moved(rig, x, y);
    assert!(showing(&w.global::<Tip>(), help), "{help}: {}", w.global::<Tip>().get_text());
    let at = note(w);
    assert!(overlap(&at, &own), "{help}: on its own slider {own:?}: {at:?}");
    assert!(!overlap(&at, &above) && !overlap(&at, &below), "{help}: {at:?} clear of {above:?} and {below:?}");
    let under = common::Placed { y: foot + 4.0, ..at };
    assert!(overlap(&under, &below), "twin: {under:?} over {below:?}");
}

pub(crate) fn a_slider_s_tooltip_covers_no_neighbour() {
    let Some((rig, shell, root)) = opened("tooltip-neighbour") else { return };
    let w = &shell.window;
    let tip = w.global::<Tip>();
    w.global::<oni_app::PanelState>().set_tab(2);
    w.global::<oni_app::PanelState>().invoke_tab_picked(2);
    rig.settle();
    let sliders = |names: [&str; 3]| names.map(|name| common::the(w, Role::Slider, name));

    // Tint's rail, between Temperature and Saturation, found up the slider by its tooltip: its name
    // above tells the same help.
    let three = sliders(["Temperature", "Tint", "Saturation"]);
    let own = three[1];
    let x = own.x + own.width / 3.0;
    let y = (0..own.height as i32).rev().map(|k| own.y + k as f32).find(|&y| {
        moved(&rig, x, y);
        showing(&tip, "Double-click to bring it back to rest")
    });
    let y = y.expect("Tint's rail tooltip");
    clear(&rig, w, (x, y), "Double-click to bring it back to rest", own.y + own.height, three);

    // The Scanner face's step keys, between C and Y: M's minus and plus.
    w.global::<oni_app::Bar>().invoke_act(oni_app::Act::FaceScanner);
    rig.settle();
    let three = sliders(["C", "M", "Y"]);
    for help in ["M −: toward green", "M +: toward magenta"] {
        moved(&rig, 0.0, 0.0);
        let key = common::the(w, Role::Button, help);
        clear(&rig, w, key.middle(), help, key.y + key.height, three);
    }
    let _ = std::fs::remove_dir_all(&root);
}
