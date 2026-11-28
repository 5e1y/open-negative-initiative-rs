//! Picking Projects beside a photo, the archive's lower half rises to the column's top, its layout the projects' tree's;
//! picking Archive sends it back down, and a press while it moves turns it from where it stands. In the gallery, with no
//! lower half, the projects rise from the column's foot.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Library;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

/// `Motion.travel`, the rise's whole time.
const TRAVEL: Duration = Duration::from_millis(420);

/// The projects' head while they show: their view's picker at their top, the archive's own clipped under them.
fn foot(w: &oni_app::MainWindow) -> Option<f32> {
    ["Names only", "Two per row", "One per row"].iter().flat_map(|l| common::placed(w, Role::Button, l)).map(|k| k.y).reduce(f32::max)
}

fn pick(rig: &common::Rig, lib: &Library, surface: i32) {
    lib.set_surface(surface);
    lib.invoke_surface_picked(surface);
    rig.pump();
}

/// How far below its rest the projects' head stands a moment after the Projects tab is picked, the clock held from the
/// press, and that rest.
fn rise(rig: &common::Rig, w: &oni_app::MainWindow) -> (f32, f32) {
    let lib = w.global::<Library>();
    pick(rig, &lib, 1);
    rig.settle();
    let rest = foot(w).expect("the projects at rest");
    pick(rig, &lib, 0);
    rig.settle();
    assert!(common::placed(w, Role::Button, "New Project").is_empty(), "the archive alone at rest");
    rig.freeze();
    pick(rig, &lib, 1);
    rig.advance(MOMENT);
    rig.pump();
    let start = foot(w).expect("the projects laid as they rise");
    (start - rest, rest)
}

/// A moment after a press, the same in both modes.
const MOMENT: Duration = Duration::from_millis(30);

pub(crate) fn the_projects_rise_from_the_archives_lower_half_and_a_press_turns_them_back() {
    let Some(app) = crate::panel_bindings::opened("library-rise") else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    // Tall enough for the photos' half under the rows.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1100));
    rig.settle();
    let rows = w.global::<oni_app::Navigator>().get_height();
    let (beside, rest) = rise(rig, w);
    rig.advance(TRAVEL / 2);
    rig.pump();
    let mid = foot(w).unwrap() - rest;
    println!("beside a photo, rows {rows} pt tall, the projects {beside} pt under their rest at first, {mid} half way");
    assert!(beside > 0.5 * rows && beside <= rows + 0.5 && mid < beside - 1.0 && mid > 0.5, "rising from the lower half: {beside} {mid}");
    // A press while it moves turns it back from where it stands, the tab taken at once.
    pick(rig, &lib, 0);
    rig.advance(Duration::from_millis(40));
    rig.pump();
    let turned = foot(w).unwrap() - rest;
    assert!(lib.get_surface() == 0 && turned > mid && turned <= rows + 0.5, "turned back from {mid}: {turned}");
    rig.advance(TRAVEL);
    rig.pump();
    assert!(common::placed(w, Role::Button, "New Project").is_empty(), "sunk back to the archive");
    rig.thaw();
    rig.settle();
    // Twin: in the gallery, with no lower half, the projects rise from the column's foot, further at the same moment.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let (gallery, _) = rise(rig, w);
    rig.thaw();
    rig.settle();
    println!("in the gallery the projects {gallery} pt under their rest at first");
    assert!(gallery > beside + 1.0, "the gallery's rise from the foot, {gallery}, against the lower half's {beside}");
}
