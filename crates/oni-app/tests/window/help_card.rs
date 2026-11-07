//! A help mark, named with its block, opens its note as a card in the block's flow under the title: the
//! sliders pushed down, never covered, and still in reach; the card padded alike on its four sides. Its
//! mark again closes it, another block's takes its place. A block folds by a chevron before its title; its
//! Reset shows only once the block moves, at the row's far end, the title between it and the fold.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{PanelState, Tip};
use slint::ComponentHandle;

use crate::common::{self, Placed};
use crate::help_demo::{mark, press_help};
use crate::panel_bindings::opened;

const LIGHTING: &str = "The negative back to light along a straight line: the black point on pure black, the white on pure white.\n\nEvery slider but Contrast brightens to the right; Contrast bends the print into an S about a middle grey, its ends held. Black point and White point move the ends of the scan itself.";
/// The card's padding, `Gap.m`, and the step under the title row, `SectionStyle.gap`.
const PAD: f32 = 16.0;
const GAP: f32 = 8.0;

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

pub(crate) fn a_help_card_opens_in_its_block_and_pushes_it_down() {
    let Some((rig, shell, root)) = opened("help-card") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let tip = w.global::<Tip>();
    // Tall enough for the open card and the sliders it pushes down.
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

    // Open, the card lies under its mark across the block's column, the sliders pushed down below it.
    let before = common::the(w, Role::Slider, "Exposure");
    press_help(&rig, w, "Lighting");
    assert_eq!(tip.get_open(), LIGHTING);
    assert_eq!(expanded(), Some(true), "twin: and open");
    let card = common::the(w, Role::Groupbox, "Help on Lighting");
    let after = common::the(w, Role::Slider, "Exposure");
    let mark_now = mark(w, "Lighting");
    assert!(card.y >= mark_now.y + mark_now.height, "under its mark: {card:?} {mark_now:?}");
    assert!((card.x - (w.get_panel_x() + PAD)).abs() < 0.5 && (card.x + card.width - (1280.0 - PAD)).abs() < 0.5, "across the column: {card:?}");
    assert!(after.y >= card.y + card.height, "the slider stands under the card: {after:?} {card:?}");
    assert!((after.y - before.y - (card.height + GAP)).abs() < 0.5, "pushed down by the card: {before:?} {after:?} {card:?}");
    assert!(before.y < card.y + card.height, "twin: where it stood, the card would cover it");

    // One padding round what it holds: the demo's picture on top, the note at the foot.
    let note = common::the(w, Role::Text, LIGHTING);
    let picture = common::placed_all(w, Role::Image).into_iter().find(|i| i.width > card.width / 2.0 && i.x >= card.x && i.y >= card.y && i.y < note.y);
    let picture = picture.expect("the demo's picture on the card");
    let round = insets(&card, &picture, &note);
    assert!(even(round), "padded alike on four sides: {round:?}");
    assert!(!even([PAD, PAD, PAD, PAD / 2.0]), "twin: a foot half as deep is refused");
    rig.shot("help-card-open");

    // The slider in reach while the card plays: a press on its rail moves Exposure, the Reset wakes in place.
    let exposure = || ctl.state.borrow().store().settings().print.exposure;
    let rested = exposure();
    common::click(&rig, w, (after.x + after.width * 0.95, after.y + after.height - 8.0));
    rig.settle();
    assert!(exposure() > rested, "the press reached the slider: {rested} to {}", exposure());
    assert_eq!(tip.get_open(), LIGHTING, "the card stays open while the hand works");
    let moved = reset_of(w, "Lighting").expect("something moved: the Reset shows");
    // At the column's far end, the whole title between it and the fold.
    let end = |reset: &Placed| (reset.x + reset.width - (1280.0 - PAD)).abs() < 0.5 && reset.x > title.x + title.width;
    assert!(end(&moved), "the Reset at the row's far end: {moved:?} {title:?}");
    assert!(!end(&Placed { x: fold.x + fold.width, ..moved }), "twin: a Reset beside the fold is refused");

    // Another block's mark takes its place; its own mark again closes it, the sliders back where they stood.
    press_help(&rig, w, "Colour");
    assert!(common::placed(w, Role::Groupbox, "Help on Lighting").is_empty(), "one card at a time");
    assert_eq!(common::placed(w, Role::Groupbox, "Help on Colour").len(), 1);
    press_help(&rig, w, "Colour");
    assert!(tip.get_open().is_empty() && common::placed(w, Role::Groupbox, "Help on Colour").is_empty(), "its mark again closes it");
    let back = common::the(w, Role::Slider, "Exposure");
    assert!((back.y - before.y).abs() < 0.5, "the block back in its place: {back:?} {before:?}");
    let _ = std::fs::remove_dir_all(&root);
}
