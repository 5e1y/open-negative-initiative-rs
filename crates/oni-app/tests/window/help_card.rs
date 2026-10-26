//! A help mark's card in the editor panel opens at one place, whatever the mark's height: its right
//! edge a gap left of the panel, its top under the tab bar; a mark elsewhere keeps its own place.

use oni_app::app::{Options, Shell};
use oni_app::{App, Tip, TipBox, Tips};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// `TipStyle.card-gap` and the margin `Tips.place` keeps from the window's edges, `Space.s`.
const GAP: f32 = 8.0;

/// A card left of the panel, under its tab bar.
fn beside_panel(b: &TipBox, panel_x: f32, tabs_foot: f32) -> bool {
    (b.x + b.width - (panel_x - GAP)).abs() < 0.5 && (b.y - tabs_foot).abs() < 0.5
}

/// Each help card met down the panel, with its mark's height and where it opened.
fn cards_down(rig: &common::Rig, tip: &Tip, w: &oni_app::MainWindow, left: f32, from: f32, to: f32) -> Vec<(String, f32, TipBox)> {
    let mut met: Vec<(String, f32, TipBox)> = Vec::new();
    let mut y = from;
    while y < to && met.len() < 2 {
        let mut x = left;
        while x < left + 280.0 {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && met.iter().all(|m| m.0 != tip.get_text().as_str()) {
                met.push((tip.get_text().to_string(), tip.get_y(), w.get_tip_box()));
                break;
            }
            x += 3.0;
        }
        y += 3.0;
    }
    met
}

pub(crate) fn a_panel_help_card_opens_left_of_the_panel_under_its_tab_bar() {
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

    // The panel lays its own anchor: its left edge, the tab bar's foot under the histogram.
    let (panel_x, foot) = (w.get_panel_x(), w.get_tabs_foot());
    assert_eq!(panel_x, 1280.0 - 340.0, "the anchor is the panel's left edge");
    assert!(foot > 200.0 && foot < 400.0, "the tab bar's foot sits under the histogram: {foot}");

    // Two marks at different heights open their card at the same place.
    let met = cards_down(&rig, &tip, w, panel_x + 4.0, foot + 2.0, 790.0);
    assert_eq!(met.len(), 2, "two help marks down the panel: {met:?}", met = met.iter().map(|m| (&m.0, m.1)).collect::<Vec<_>>());
    assert!(met[1].1 - met[0].1 > 20.0, "the marks sit at different heights");
    for (note, _, b) in &met {
        assert!(beside_panel(b, panel_x, foot), "{note:?} opened at {b:?}");
    }
    rig.shot("help-card-panel");
    // Twin: a card opened beside its mark, the old placement, is no panel card.
    let (_, mark_y, b) = &met[1];
    let by_mark = TipBox { x: panel_x + 20.0, y: *mark_y, ..b.clone() };
    assert!(!beside_panel(&by_mark, panel_x, foot));

    // A short window holds the card whole: its top rises, its foot stays a margin in.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(10.0, 10.0) });
    rig.pump();
    // A note with a demo, so its card is tall enough for a short window to cut.
    tip.set_demo("sharpening".into());
    tips.invoke_note(true, "a note".into(), panel_x + 30.0, foot + 40.0);
    rig.pump();
    let tall = w.get_tip_box();
    assert!(beside_panel(&tall, panel_x, foot), "a note in the panel opens beside it: {tall:?}");
    let short = (foot + tall.height / 2.0) as u32;
    rig.window.set_size(slint::PhysicalSize::new(1280, short));
    rig.settle();
    let held = w.get_tip_box();
    // The room every note is held in, read off a note far below the window's foot.
    tips.invoke_note(true, "far".into(), 400.0, 5000.0);
    rig.pump();
    let floor = w.get_tip_box();
    let room = floor.y + floor.height + GAP;
    tips.invoke_note(false, "far".into(), 400.0, 5000.0);
    assert!(room < 600.0, "the window is short: {room}");
    assert!(held.y < foot - 0.5, "a short window lifts the card's top: {held:?}");
    assert!((held.y + held.height - (room - GAP)).abs() < 0.5 && held.y >= GAP - 0.5, "the card leaves a short window: {held:?} in {room}");
    assert!((held.x + held.width - (panel_x - GAP)).abs() < 0.5, "a short window keeps the card left of the panel");
    tips.invoke_note(false, "a note".into(), panel_x + 30.0, foot + 40.0);
    tip.set_demo("".into());
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.settle();

    // Twin: a note over the canvas opens under its own mark.
    tips.invoke_note(true, "a note".into(), 400.0, 300.0);
    rig.pump();
    let own = w.get_tip_box();
    assert!(!beside_panel(&own, panel_x, foot), "a mark off the panel took the panel's place");
    assert_eq!((own.x, own.y), (400.0, 300.0 + 4.0), "a mark off the panel keeps its place");
    tips.invoke_note(false, "a note".into(), 400.0, 300.0);
    // Twin: in the gallery no panel stands, and a mark in its column keeps its place.
    w.global::<App>().set_gallery(true);
    rig.settle();
    tips.invoke_note(true, "a note".into(), panel_x + 30.0, foot + 40.0);
    rig.pump();
    let gallery = w.get_tip_box();
    assert!(!beside_panel(&gallery, panel_x, foot), "the gallery's column took the panel's place: {gallery:?}");
    tips.invoke_note(false, "a note".into(), panel_x + 30.0, foot + 40.0);
    let _ = std::fs::remove_dir_all(&root);
}
