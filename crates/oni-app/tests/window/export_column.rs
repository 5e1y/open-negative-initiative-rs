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
    // The colour space's tabs inside the narrower column in every language: the last as far in as the first.
    for language in [oni_i18n::Language::English, oni_i18n::Language::French, oni_i18n::Language::Spanish] {
        oni_app::language::speak(Some(language));
        rig.settle();
        let bar = one(w, "SpaceTabs");
        let mut tabs: Vec<Placed> = common::placed_all(w, Role::Tab).into_iter().filter(|t| t.y >= bar.y && t.y + t.height <= bar.y + bar.height + 0.5).collect();
        tabs.sort_by(|a, b| a.x.total_cmp(&b.x));
        tabs.dedup_by(|a, b| (a.x - b.x).abs() < 0.5);
        let (first, last) = (tabs[0], tabs[tabs.len() - 1]);
        let (left, right) = (first.x - bar.x, bar.x + bar.width - (last.x + last.width));
        println!("{language:?}: the space's tabs {left} pt in at the left, {right} at the right");
        assert_eq!(tabs.len(), 5, "{language:?}: five tabs: {tabs:?}");
        assert!((left - right).abs() < 0.5 && right > 0.0, "{language:?}: the last tab {right} pt in, the first {left}");
        // Twin: measured from the column's edge, the last tab would read off the first's inset.
        assert!(((b.x + b.width) - (last.x + last.width) - left).abs() > 0.5, "twin: the column's edge reads as the tabs'");
    }
    oni_app::language::speak(Some(oni_i18n::Language::English));
    // Twin: the editor's panel hidden, its card reaches past where the export's column stands.
    ctl.leave_export();
    rig.settle();
    ctl.act(oni_app::Act::HidePanel);
    rig.settle();
    let hidden = one(w, "CanvasCard");
    assert!(hidden.x + hidden.width > b.x + 1.0, "twin: the card reached the column's place: {hidden:?} {b:?}");
}
