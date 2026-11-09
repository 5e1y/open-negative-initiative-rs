//! A help mark opens its note as a card left of the column holding it, nothing in the column moved or covered; its mark,
//! Escape or a press closes it, and it goes with its mark. A block folds by a chevron; its Reset shows once the block moves.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{Act, App, Bar, Canvas, Exporting, PanelState, Tip};
use slint::ComponentHandle;

use crate::common::{self, Placed};
use crate::help_demo::{mark, press_help};
use crate::panel_bindings::opened;

const LIGHTING: &str = "The negative back to light along a straight line: the black point on pure black, the white on pure white.\n\nEvery slider but Contrast brightens to the right; Contrast bends the print into an S about a middle grey, its ends held. Black point and White point move the ends of the scan itself.";
/// The card's padding, `Gap.m`, its step from the column, `TipStyle.card-gap`, the window's margin, and
/// the menu bar a check's window draws above the interface's coordinates.
const PAD: f32 = 16.0;
const GAP: f32 = 8.0;
const MARGIN: f32 = 8.0;
const MENU: f32 = 40.0;

/// The card's insets round what it holds: left, top, right, bottom.
fn insets(card: &Placed, first: &Placed, last: &Placed) -> [f32; 4] {
    [first.x - card.x, first.y - card.y, card.x + card.width - (last.x + last.width), card.y + card.height - (last.y + last.height)]
}

fn even(insets: [f32; 4]) -> bool {
    insets.iter().all(|i| (i - PAD).abs() < 0.5)
}

/// The block's Reset on the row of its "?", none while the block stands at its rest.
pub(crate) fn reset_of(w: &oni_app::MainWindow, subject: &str) -> Option<Placed> {
    let row = mark(w, subject);
    let found: Vec<Placed> = common::placed(w, Role::Button, "Reset").into_iter().filter(|p| p.level_with(&row)).collect();
    assert!(found.len() <= 1, "one Reset at most on {subject}'s row: {found:?}");
    found.first().copied()
}

/// The key an assistive tool reaches by its name: whether it folds anything, and whether that stands open.
fn told(w: &oni_app::MainWindow, label: &str) -> (Option<bool>, Option<bool>) {
    let key = ElementHandle::find_by_accessible_label(w, label).find(|e| e.accessible_role() == Some(Role::Button));
    let key = key.unwrap_or_else(|| panic!("a key named {label}"));
    (key.accessible_expandable(), key.accessible_expanded())
}

pub(crate) fn a_help_card_opens_left_of_the_panel_and_moves_nothing() {
    let Some((rig, shell, root)) = opened("help-card") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let tip = w.global::<Tip>();
    // Tall enough to hold the Lighting block's sliders on screen.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1400));
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    rig.settle();

    // Each mark is named with its block, none with what any block does.
    assert!(common::placed(w, Role::Button, "What this block does").is_empty(), "no mark is named for every block");
    let (lighting, colour) = (mark(w, "Lighting"), mark(w, "Colour"));
    assert!(colour.y < lighting.y, "twin: two marks, two names, the colour's first");
    let expanded = || {
        let found: Vec<_> = ElementHandle::find_by_accessible_label(w, "Help on Lighting").filter(|e| e.accessible_role() == Some(Role::Button)).collect();
        found[0].accessible_expanded()
    };
    assert_eq!(expanded(), Some(false), "the mark tells its card is closed");

    // At rest a block shows no Reset; its fold's chevron leads its title, the tree's disclosure.
    assert!(reset_of(w, "Lighting").is_none(), "nothing moved: no Reset");
    let fold = common::the(w, Role::Button, "Hide Lighting");
    let title = common::placed(w, Role::Text, "Lighting").into_iter().find(|t| t.level_with(&fold)).expect("the block's title");
    assert!(fold.x + fold.width / 2.0 < title.x, "the chevron before the title: {fold:?} {title:?}");
    assert_eq!(told(w, "Hide Lighting"), (Some(true), Some(true)), "the fold tells its block stands open");

    // Open, the card stands left of the panel under its tab bar; nothing in the panel moves.
    let before = common::the(w, Role::Slider, "Exposure");
    press_help(&rig, w, "Lighting");
    assert_eq!(tip.get_open(), LIGHTING);
    assert_eq!(expanded(), Some(true), "twin: and open");
    let card = common::the(w, Role::Groupbox, "Help on Lighting");
    let after = common::the(w, Role::Slider, "Exposure");
    let beside = |c: &Placed| (c.x + c.width - (w.get_panel_x() - GAP)).abs() < 0.5 && (c.y - MENU - w.get_tabs_foot()).abs() < 0.5;
    assert!(beside(&card), "left of the panel, under its tab bar: {card:?} {}", w.get_panel_x());
    assert!(!beside(&Placed { x: w.get_panel_x() + PAD, ..card }), "twin: a card over the panel's column is refused");
    assert_eq!((after.x, after.y), (before.x, before.y), "the block stays where it stood");

    // One padding round what it holds: the demo's picture on top, the note at the foot.
    let note = common::the(w, Role::Text, LIGHTING);
    let picture = common::placed_all(w, Role::Image).into_iter().find(|i| i.width > card.width / 2.0 && i.x >= card.x && i.x + i.width <= card.x + card.width && i.y >= card.y && i.y < note.y);
    let picture = picture.expect("the demo's picture on the card");
    let round = insets(&card, &picture, &note);
    assert!(even(round), "padded alike on four sides: {round:?}");
    assert!(!even([PAD, PAD, PAD, PAD / 2.0]), "twin: a foot half as deep is refused");
    rig.shot("help-card-open");

    // The slider in reach while the card plays: a press on its rail moves Exposure, the Reset wakes in place.
    assert!(after.x >= card.x + card.width, "the card leaves the slider uncovered: {after:?} {card:?}");
    let exposure = || ctl.state.borrow().store().settings().print.exposure;
    let rested = exposure();
    common::click(&rig, w, (after.x + after.width * 0.95, after.y + after.height - 8.0));
    rig.settle();
    assert!(exposure() > rested, "the press reached the slider: {rested} to {}", exposure());
    assert_eq!(tip.get_open(), LIGHTING, "the card stays open while the hand works");
    let moved = reset_of(w, "Lighting").expect("something moved: the Reset shows");
    assert_eq!(told(w, "Reset").0, Some(false), "twin: a key that opens nothing claims no fold");
    // At the column's far end, the whole title between it and the fold.
    let end = |reset: &Placed| (reset.x + reset.width - (1280.0 - PAD)).abs() < 0.5 && reset.x > title.x + title.width;
    assert!(end(&moved), "the Reset at the row's far end: {moved:?} {title:?}");
    assert!(!end(&Placed { x: fold.x + fold.width, ..moved }), "twin: a Reset beside the fold is refused");

    // Another block's mark takes its place at the same spot; its own mark again closes it.
    press_help(&rig, w, "Colour");
    assert!(common::placed(w, Role::Groupbox, "Help on Lighting").is_empty(), "one card at a time");
    assert_eq!(common::placed(w, Role::Groupbox, "Help on Colour").len(), 1);
    assert!(beside(&common::the(w, Role::Groupbox, "Help on Colour")), "the same place for every block");
    press_help(&rig, w, "Colour");
    assert!(tip.get_open().is_empty() && common::placed(w, Role::Groupbox, "Help on Colour").is_empty(), "its mark again closes it");

    // Folded, its fold tells it as plainly.
    let hide = ElementHandle::find_by_accessible_label(w, "Hide Lighting").find(|e| e.accessible_role() == Some(Role::Button));
    hide.expect("the fold").invoke_accessible_default_action();
    rig.settle();
    assert_eq!(told(w, "Show Lighting"), (Some(true), Some(false)), "the fold tells its block is folded");
    let _ = std::fs::remove_dir_all(&root);
}

/// Opens the Lighting block's card and says where it stands.
fn lighting_card(rig: &common::Rig, w: &oni_app::MainWindow) -> Placed {
    press_help(rig, w, "Lighting");
    common::the(w, Role::Groupbox, "Help on Lighting")
}

pub(crate) fn a_help_card_closes_by_hand_and_goes_with_its_mark() {
    let Some((rig, shell, root)) = opened("help-card-close") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (tip, app, bar) = (w.global::<Tip>(), w.global::<App>(), w.global::<Bar>());
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    rig.settle();
    let closed = |why: &str| {
        rig.settle();
        assert!(tip.get_open().is_empty() && !tip.get_has_clip(), "{why}: no card, no clip");
        assert!(common::placed(w, Role::Groupbox, "Help on Lighting").is_empty(), "{why}: no card on screen");
    };

    // Escape takes the card and nothing behind it; twin: the next one leaves the editor.
    lighting_card(&rig, w);
    rig.key("\u{1b}");
    closed("Escape");
    assert!(!app.get_gallery(), "Escape took the card, not the editor");

    // A press on the card closes it and reaches nothing under it; twin: there, with no card, the canvas hears it.
    let doubles = std::rc::Rc::new(std::cell::Cell::new(0));
    let heard = doubles.clone();
    w.global::<Canvas>().on_double_clicked(move || heard.set(heard.get() + 1));
    let spot = lighting_card(&rig, w).middle();
    common::click(&rig, w, spot);
    closed("a press on it");
    assert_eq!(doubles.get(), 0, "the press stayed on the card");
    common::click(&rig, w, spot);
    common::click(&rig, w, spot);
    assert_eq!(doubles.get(), 1, "twin: the canvas lies under that spot");
    rig.settle();

    // Whatever takes its mark away takes the card: the pane, the face, the panel hidden, a sheet, the gallery.
    lighting_card(&rig, w);
    w.global::<PanelState>().set_tab(3);
    w.global::<PanelState>().invoke_tab_picked(3);
    closed("another pane");
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    rig.settle();
    lighting_card(&rig, w);
    bar.invoke_act(Act::FaceScanner);
    closed("the other face");
    bar.invoke_act(Act::FacePro);
    rig.settle();
    lighting_card(&rig, w);
    ctl.act(Act::HidePanel);
    closed("the panel hidden");
    ctl.act(Act::HidePanel);
    rig.settle();
    lighting_card(&rig, w);
    w.global::<oni_app::Help>().set_report_shown(true);
    closed("a sheet over the window");
    w.global::<oni_app::Help>().set_report_shown(false);
    rig.settle();
    lighting_card(&rig, w);
    ctl.act(Act::ToggleMode);
    closed("the gallery");
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_help_card_stands_inside_a_short_window_and_beside_every_column() {
    let Some((rig, shell, root)) = opened("help-card-room") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    // The window at its smallest: the card lifted to keep to it, its top rising past the tab bar's foot.
    rig.window.set_size(slint::PhysicalSize::new(1100, 680));
    rig.settle();
    let card = lighting_card(&rig, w);
    assert!(card.y + card.height <= 680.0 - MARGIN + 0.5, "inside the window: {card:?}");
    assert!(MENU + w.get_tabs_foot() + card.height > 680.0 - MARGIN, "twin: under the tab bar it would leave the window: {card:?}");
    assert!((card.x + card.width - (w.get_panel_x() - GAP)).abs() < 0.5, "lifted, still left of the panel: {card:?}");
    press_help(&rig, w, "Lighting");

    // The export's column is wider and has no bar above: the card keeps its step from it, at the window's top.
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    ctl.act(Act::Export);
    rig.until("the export's column", std::time::Duration::from_secs(30), || w.global::<Exporting>().get_shown());
    rig.settle();
    let presets = mark(w, "Presets");
    assert!(presets.x > 1280.0 - 400.0, "the mark in the export's column: {presets:?}");
    press_help(&rig, w, "Presets");
    let card = common::the(w, Role::Groupbox, "Help on Presets");
    assert!((card.x + card.width - (1280.0 - 400.0 - GAP)).abs() < 0.5 && (card.y - MENU - MARGIN).abs() < 0.5, "left of the export's column: {card:?}");
    // Escape takes the card first; twin: the next one leaves the export.
    rig.key("\u{1b}");
    rig.settle();
    assert!(w.global::<Exporting>().get_shown() && common::placed(w, Role::Groupbox, "Help on Presets").is_empty(), "Escape took the card");
    rig.key("\u{1b}");
    rig.settle();
    assert!(!w.global::<Exporting>().get_shown(), "twin: the next Escape left the export");
    let _ = std::fs::remove_dir_all(&root);
}
