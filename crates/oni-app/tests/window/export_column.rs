//! The export's column lies over the editor's: the card flush with the column, the column as wide and as high, its
//! sections' content as far in; the export changes what the column holds, never where it stands.

use i_slint_backend_testing::{AccessibleRole as Role, ElementRoot};
use slint::ComponentHandle;

use crate::common::{self, Placed};
use crate::panel_bindings::opened;

/// The one shown element of the component `kind`, where the layout put it.
fn one(w: &oni_app::MainWindow, kind: &str) -> Placed {
    let found: Vec<Placed> = w
        .root_element()
        .query_descendants()
        .match_inherits(kind)
        .find_all()
        .into_iter()
        .map(|e| Placed { x: e.absolute_position().x, y: e.absolute_position().y, width: e.size().width, height: e.size().height })
        .filter(|p| p.width > 0.0 && p.height > 0.0)
        .collect();
    assert_eq!(found.len(), 1, "one {kind} shown: {found:?}");
    found[0]
}

/// The card's right edge, the column's place, and how far in a slider of the column starts.
fn laid(w: &oni_app::MainWindow, card: &str, column: &str, slider: &str) -> (f32, Placed, f32) {
    let (card, column) = (one(w, card), one(w, column));
    (card.x + card.width, column, common::the(w, Role::Slider, slider).x - column.x)
}

pub(crate) fn the_export_column_lies_over_the_editors() {
    let Some(app) = opened("export-column") else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    // The Mac's traffic lights, under which the bar's room stays as the export opens.
    w.global::<oni_app::Bar>().set_traffic_lights(100.0);
    rig.window.set_size(slint::PhysicalSize::new(1280, 900));
    rig.settle();
    let editor = laid(w, "CanvasCard", "Panel", "Exposure");
    ctl.act(oni_app::Act::Export);
    rig.settle();
    w.global::<oni_app::Exporting>().set_format(1);
    w.global::<oni_app::Exporting>().invoke_format_picked(1);
    rig.settle();
    let export = laid(w, "ExportCard", "ExportPanel", "Quality");
    println!("card's edge, column, content's inset: the editor {editor:?}, the export {export:?}");
    assert!((editor.0 - editor.1.x).abs() < 0.5 && (export.0 - export.1.x).abs() < 0.5, "each card flush with its column: {editor:?} {export:?}");
    let (a, b) = (editor.1, export.1);
    assert!((a.x - b.x).abs() < 0.5 && (a.width - b.width).abs() < 0.5 && (a.y - b.y).abs() < 0.5, "the column in the same place: {a:?} {b:?}");
    assert!((editor.2 - export.2).abs() < 0.5, "the content as far in: {} against {}", editor.2, export.2);
    println!("shot {}", rig.shot("export-column").display());
    // Twin: the editor's panel hidden, its card reaches past where the export's column stands.
    ctl.leave_export();
    rig.settle();
    ctl.act(oni_app::Act::HidePanel);
    rig.settle();
    let hidden = one(w, "CanvasCard");
    assert!(hidden.x + hidden.width > b.x + 1.0, "twin: the card reached the column's place: {hidden:?} {b:?}");
}
