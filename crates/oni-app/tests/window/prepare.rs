//! Prepare the Roll on the headless window, every step by the hand: left and resumed from its draft,
//! measured as one gallery step, asked for by a bare frame opened, and reached from the Roll tab.

use oni_app::app::{Options, Shell};
use oni_app::{App, Bar, Library, Menus, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn loaded(path: &Path) -> sidecar::Loaded {
    sidecar::read(path, false, true).unwrap()
}

fn click(rig: &common::Rig, x: f32, y: f32) {
    let at = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn a_roll_is_prepared_left_resumed_from_its_draft_and_measured_as_one_gallery_step() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
    // D stands on another roll: a frame of this folder the measure leaves out.
    let mut other = PipelineSettings::rest(ConversionMode::Negative, false);
    other.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    sidecar::write(&other, &canonical("D.tif"), WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, menus, p, app) = (w.global::<Library>(), w.global::<Menus>(), w.global::<Preparing>(), w.global::<App>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| {
        let mut tiles = oni_app::app::tiles(w);
        tiles.sort_by_key(|t| t.index);
        tiles.iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index
    };
    let names = || (0..p.get_cells().row_count()).count();
    let action = || (p.get_footer_x() + p.get_footer_width() - 12.0, p.get_footer_y() + p.get_footer_height() / 2.0);

    // Nothing picked, nothing to prepare; one frame picked lights the menu item and the bar's key.
    assert!(!menus.get_live().measure_roll && !w.global::<Bar>().get_can_prepare());
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    assert!(menus.get_live().measure_roll && w.global::<Bar>().get_can_prepare());
    w.global::<Bar>().invoke_act("prepare-roll".into());
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && names() == 3);
    rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
    assert_eq!(p.get_left_out().as_str(), "1 frame of this folder, measured on another roll, is left out.");
    assert_eq!(p.get_heading().as_str(), "Select the Film Type");
    assert!(p.get_skipped().row_data(1).unwrap(), "a scanner's roll skips the flat-field");
    assert!(!menus.get_live().toggle_mode && !menus.get_live().measure_roll, "⌘G and the item itself stay dark meanwhile");
    assert!(loaded(&canonical("B.tif")).roll_draft.is_none(), "twin: the film step writes no draft");
    rig.settle();
    println!("shot {}", rig.shot("prepare-film").display());

    // NEXT walks past the skipped flat-field to the base; the step reached rides in every sidecar.
    let (x, y) = action();
    click(&rig, x, y);
    assert_eq!(p.get_step(), 2);
    rig.until("the drafts written", Duration::from_secs(10), || ["A.tif", "B.tif", "C.tif"].iter().all(|n| loaded(&canonical(n)).roll_draft.is_some_and(|d| d.step == "base")));
    assert!(loaded(&canonical("D.tif")).roll_draft.is_none(), "the frame left out carries no draft");
    assert_eq!((p.get_base_note().as_str(), p.get_action_title().as_str()), ("No base picked yet.", "SKIP"));

    // A click on the scan picks the bare film there and moves on to the flags.
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    rig.settle();
    println!("shot {}", rig.shot("prepare-base").display());
    click(&rig, p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.05, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.1);
    assert_eq!(p.get_step(), 3, "a pick goes on to the flags");
    assert_eq!(p.get_base_note().as_str(), "On A.tif");

    // A click on the sheet flags C; Clear Flags takes every flag off; a second click flags it again.
    rig.settle();
    let cell = |i: usize| {
        let (pad, gap) = (24.0, 16.0);
        let width = (p.get_sheet_width() - 2.0 * pad - 5.0 * gap) / 6.0;
        (p.get_sheet_origin_x() + pad + i as f32 * (width + gap) + width / 2.0, p.get_sheet_origin_y() + pad + width / 3.0)
    };
    let (x, y) = cell(2);
    click(&rig, x, y);
    assert_eq!(p.get_flags_note().as_str(), "2 measured, 1 flagged");
    p.invoke_clear_flags();
    rig.pump();
    assert_eq!(p.get_flags_note().as_str(), "3 measured, 0 flagged", "twin: the flag is the click's, not the sheet's");
    click(&rig, x, y);
    assert!(p.get_cells().row_data(2).unwrap().flagged);
    rig.settle();
    println!("shot {}", rig.shot("prepare-flags").display());

    // The window step: two to measure, the arrows walk them, the body drags the window.
    p.invoke_step_picked(4);
    rig.pump();
    assert_eq!(p.get_action_title().as_str(), "MEASURE 2 FRAMES");
    assert_eq!(p.get_frame_name().as_str(), "A.tif");
    rig.key("\u{F703}");
    assert_eq!(p.get_frame_name().as_str(), "B.tif");
    rig.key("\u{F703}");
    assert_eq!(p.get_frame_name().as_str(), "B.tif", "twin: the last frame holds");
    rig.settle();
    let (wx, wy) = (p.get_window_x(), p.get_window_y());
    let (cx, cy) = (p.get_card_origin_x() + wx + p.get_window_width() / 2.0, p.get_card_origin_y() + wy + p.get_window_height() / 2.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(cx, cy) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: LogicalPosition::new(cx, cy), button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(cx - 10.0, cy - 6.0) });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(cx - 10.0, cy - 6.0), button: PointerEventButton::Left });
    rig.pump();
    assert!((p.get_window_x() - (wx - 10.0)).abs() < 1.0 && (p.get_window_y() - (wy - 6.0)).abs() < 1.0, "the body moves the window");
    println!("shot {}", rig.shot("prepare-window").display());

    // Escape leaves for the gallery, every frame's place kept in its sidecar.
    rig.key("\u{1b}");
    assert!(!p.get_shown() && app.get_gallery());
    rig.until("the drafts written", Duration::from_secs(10), || loaded(&canonical("C.tif")).roll_draft.is_some_and(|d| d.step == "window"));
    let c = loaded(&canonical("C.tif")).roll_draft.unwrap();
    assert!(c.flagged && c.ticked && c.base.as_ref().is_some_and(|b| b.path.ends_with("A.tif")));

    // B picked brings the same roll back where it stood.
    lib.invoke_tile_clicked(tile("B"), false, false);
    rig.pump();
    ctl.act("prepare-roll");
    rig.until("the roll resumed", Duration::from_secs(10), || p.get_shown() && p.get_step() == 4);
    rig.until("every frame placed", Duration::from_secs(10), || p.get_action_title() == "MEASURE 2 FRAMES");
    assert!(p.get_cells().row_data(2).unwrap().flagged);

    // MEASURE: off this thread, then the editor on the first frame; one gallery step lands it.
    let (x, y) = action();
    click(&rig, x, y);
    let advice = w.global::<oni_app::Advice>();
    rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
    assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    rig.pump();
    assert!(!app.get_gallery(), "the editor opens once measured");
    assert!(ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")));
    let ids: Vec<Option<Id>> = ["B.tif", "C.tif"].iter().map(|n| loaded(&canonical(n)).settings.roll_reference.map(|r| r.id)).collect();
    assert!(ids[0].is_some() && ids[0] == ids[1], "B and the flagged C stand on one roll: {ids:?}");
    assert!(["B.tif", "C.tif"].iter().all(|n| loaded(&canonical(n)).roll_draft.is_none()), "a measure clears the draft");
    assert_eq!(ctl.state.borrow().store().settings().roll_reference.as_ref().map(|r| r.id), ids[0], "A, open, landed in memory");
    assert!(loaded(&canonical("D.tif")).settings.roll_reference.map(|r| r.id) != ids[0], "twin: D keeps its own roll");
    ctl.act("toggle-mode");
    rig.pump();
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Measure the roll of 3 photos");
    ctl.act("undo");
    rig.pump();
    assert!(loaded(&canonical("B.tif")).settings.roll_reference.is_none(), "one undo takes the whole measure back");
    rig.until("A back at rest", Duration::from_secs(5), || loaded(&canonical("A.tif")).settings.roll_reference.is_none());

    // A frame never measured, opened in the editor, goes to Prepare the Roll; Escape leaves for the gallery.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(tile("C"), false, false);
    lib.invoke_tile_clicked(tile("C"), false, false);
    lib.invoke_tile_opened(tile("C"));
    rig.until("the roll asked for", Duration::from_secs(30), || p.get_shown());
    assert_eq!(names(), 3);
    rig.key("\u{1b}");
    assert!(app.get_gallery(), "Cancel leaves for the gallery, not the editor");
    // Twin: a frame on a roll opens in the editor and stays there.
    lib.invoke_tile_opened(tile("D"));
    rig.until("D open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("D.tif")));
    rig.settle();
    assert!(!p.get_shown() && !app.get_gallery());

    // The Roll tab's key prepares from the editor, D with the frames never measured; Escape goes back to it.
    let panel = w.global::<oni_app::PanelState>();
    assert!(panel.get_can_prepare());
    panel.invoke_prepare_roll();
    rig.until("the roll from the editor", Duration::from_secs(10), || p.get_shown() && names() == 4);
    // The measure's contact sheet still stands on the roll undone, so it counts as another roll's.
    assert_eq!(p.get_left_out().as_str(), "1 frame of this folder, measured on another roll, is left out.");
    rig.key("\u{1b}");
    assert!(!p.get_shown() && !app.get_gallery(), "Cancel goes back to the editor it came from");
    let _ = std::fs::remove_dir_all(&root);
}
