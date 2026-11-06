//! A help mark's card opens beside its mark and never over the photo: under the mark, or over it
//! where more room stands, over its block's whole column, its demo shortened so the card keeps to the
//! window; a note elsewhere too.

use oni_app::app::{Options, Shell};
use oni_app::{Tip, TipBox, Tips};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The margin `Tips.place` keeps from the window's edges, `Space.s`.
const MARGIN: f32 = 8.0;
/// A block's inset in the panel, `SectionStyle.inset`, and the card's own width, `TipStyle.card-width`.
const INSET: f32 = 16.0;
const OWN_WIDTH: f32 = 300.0;

/// A card over its block's column, edge to edge with the column's inset: no word of the block peeks out.
fn over_its_column(b: &TipBox, panel_x: f32, right: f32) -> bool {
    (b.x - (panel_x + INSET)).abs() < 0.5 && (b.x + b.width - (right - INSET)).abs() < 0.5
}

/// A card under its mark, from the mark's foot, or over it, down to the mark's top.
fn by_its_mark(b: &TipBox, foot: f32, top: f32) -> bool {
    (b.y - foot).abs() < 0.5 || (b.y + b.height - top).abs() < 0.5
}

/// Each help card met down the panel, with where its mark stands and where it opened.
fn cards_down(rig: &common::Rig, tip: &Tip, w: &oni_app::MainWindow, left: f32, from: f32, to: f32) -> Vec<(String, f32, f32, TipBox)> {
    let mut met: Vec<(String, f32, f32, TipBox)> = Vec::new();
    let mut y = from;
    while y < to && met.len() < 2 {
        let mut x = left;
        while x < left + 280.0 {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && met.iter().all(|m| m.0 != tip.get_text().as_str()) {
                met.push((tip.get_text().to_string(), tip.get_y(), tip.get_above(), w.get_tip_box()));
                break;
            }
            x += 3.0;
        }
        y += 3.0;
    }
    met
}

pub(crate) fn a_help_card_opens_by_its_mark_never_over_the_photo() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the help card is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-card-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, tip, tips) = (w.global::<oni_app::Library>(), w.global::<Tip>(), w.global::<Tips>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    let (panel_x, foot) = (w.get_panel_x(), w.get_tabs_foot());
    assert_eq!(panel_x, 1280.0 - 340.0, "the panel's left edge");

    // Two marks at different heights each open their card by their own mark, over their block's column.
    let met = cards_down(&rig, &tip, w, panel_x + 4.0, foot + 2.0, 790.0);
    assert_eq!(met.len(), 2, "two help marks down the panel: {met:?}", met = met.iter().map(|m| (&m.0, m.1)).collect::<Vec<_>>());
    assert!(met[1].1 - met[0].1 > 20.0, "the marks sit at different heights");
    for (note, mark_foot, mark_top, b) in &met {
        assert!(by_its_mark(b, *mark_foot, *mark_top), "{note:?} opened at {b:?}, its mark at {mark_top}..{mark_foot}");
        assert!(over_its_column(b, panel_x, 1280.0), "{note:?} covers its block's column edge to edge: {b:?}");
    }
    rig.shot("help-card-panel");
    // Twins: the card's place left of the panel under its tab bar is neither by the mark nor off the
    // photo; at its own width from its mark, held a margin in, the block's first letters peek out.
    let (_, mark_foot, mark_top, b) = &met[1];
    let old = TipBox { x: panel_x - MARGIN - b.width, y: foot, ..b.clone() };
    assert!(!by_its_mark(&old, *mark_foot, *mark_top) && old.x < panel_x);
    let own = TipBox { x: 1280.0 - MARGIN - OWN_WIDTH, width: OWN_WIDTH, ..b.clone() };
    assert!(!over_its_column(&own, panel_x, 1280.0) && own.x > panel_x + INSET, "the sliver: {own:?}");
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();

    // A mark low in the window opens its card over it, whole.
    tip.set_demo("sharpening".into());
    tips.invoke_note(true, "a note".into(), panel_x + 30.0, 700.0, 682.0, 0.0);
    rig.pump();
    let low = w.get_tip_box();
    assert!((low.y + low.height - tip.get_above()).abs() < 0.5, "a low mark's card stands over it: {low:?}");
    assert!(low.height > low.width, "twin: whole, its square demo over the note: {low:?}");
    tips.invoke_note(false, "a note".into(), panel_x + 30.0, 700.0, 682.0, 0.0);

    // Short rooms on both sides: the card keeps to the larger, under its mark, its demo shorter.
    rig.window.set_size(slint::PhysicalSize::new(1280, 500));
    rig.settle();
    // The room every note is held in, read off a note far below the window's foot.
    tips.invoke_note(true, "far".into(), 400.0, 5000.0, 4982.0, 0.0);
    rig.pump();
    let floor = w.get_tip_box();
    let room = floor.y + floor.height + MARGIN;
    tips.invoke_note(false, "far".into(), 400.0, 5000.0, 4982.0, 0.0);
    assert!(room < 560.0, "the window is short: {room}");
    let (top, mark_foot) = (room / 2.0 - 20.0, room / 2.0 - 2.0);
    tips.invoke_note(true, "a note".into(), panel_x + 30.0, mark_foot, top, 0.0);
    rig.pump();
    let held = w.get_tip_box();
    assert!((held.y - tip.get_y()).abs() < 0.5, "under its mark, where more room stands: {held:?} in {room}");
    assert!((held.y + held.height - (room - MARGIN)).abs() < 0.5, "the card ends a margin in: {held:?} in {room}");
    // Twin: at its whole height, the low mark's, it would have left the window.
    assert!(held.y + low.height > room - MARGIN, "the room was short: {held:?}, whole {}", low.height);
    tips.invoke_note(false, "a note".into(), panel_x + 30.0, mark_foot, top, 0.0);
    tip.set_demo("".into());
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.settle();

    // A note over the canvas opens under its own mark too.
    tips.invoke_note(true, "a note".into(), 400.0, 300.0, 282.0, 0.0);
    rig.pump();
    let own = w.get_tip_box();
    assert_eq!((own.x, own.y), (400.0, 300.0 + 4.0), "a mark off the panel keeps its place");
    tips.invoke_note(false, "a note".into(), 400.0, 300.0, 282.0, 0.0);
    let _ = std::fs::remove_dir_all(&root);
}
