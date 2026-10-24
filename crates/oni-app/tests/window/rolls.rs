//! The roll gestures on the headless window: the Measured Roll block counts the roll and the folder's
//! frames it missed, Join and Roll of the N reach them, Reset to Default asks then reaches the whole
//! roll, Ungroup takes the roll off the targets, and the balance key asks Each or Average.

use oni_app::app::{Options, Shell};
use oni_app::{Bar, Choice, Confirm, Library, Menus, PanelState};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::frame::RollReference;
use oni_settings::{ConversionMode, Film, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn settings_of(path: &Path) -> PipelineSettings {
    sidecar::read(path, false, true).unwrap().settings
}

/// A sidecar naming roll `id` and carrying `film`, as Prepare the Roll leaves a frame.
fn measured(path: &Path, id: Id, film: &Film) {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.roll_reference = Some(RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    s.film = Some(film.clone());
    sidecar::write(&s, path, WriteOptions::default()).unwrap();
}

pub(crate) fn the_roll_gestures_reach_what_swift_reaches() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the roll gestures are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-rolls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
    let (id, film) = (Id::new(), Film::new("Roll 9"));
    measured(&canonical("A.tif"), id, &film);
    measured(&canonical("B.tif"), id, &film);
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
    let (lib, menus, panel) = (w.global::<Library>(), w.global::<Menus>(), w.global::<PanelState>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| {
        let shown: Vec<String> = oni_app::app::tiles(w).into_iter().map(|t| t.caption.to_string()).collect();
        let mut tiles = oni_app::app::tiles(w);
        tiles.sort_by_key(|t| t.index);
        tiles.iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name} in {shown:?}")).index
    };
    let rest = || std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));

    // Nothing picked, nothing to prepare; the contact sheet stays dark while the build lacks it.
    assert!(!menus.get_live().measure_roll && !w.global::<Bar>().get_can_prepare() && !w.global::<Bar>().get_can_sheet());

    // Ungroup names the photos it takes off their roll: A and B carry Roll 9, C does not.
    lib.invoke_tile_clicked(tile("A"), false, false);
    lib.invoke_tile_clicked(tile("C"), true, false);
    rig.pump();
    assert!(menus.get_live().ungroup);
    assert_eq!(menus.get_titles().ungroup.as_str(), "Ungroup", "one of the two carries a roll");
    lib.invoke_tile_clicked(tile("B"), true, false);
    rig.pump();
    assert_eq!(menus.get_titles().ungroup.as_str(), "Ungroup 2 Photos");
    lib.invoke_tile_clicked(tile("A"), true, false);
    lib.invoke_tile_clicked(tile("B"), true, false);
    rig.pump();
    assert!(!menus.get_live().ungroup, "twin: C alone carries no roll");
    rest();

    // A open: the block counts the roll's two and the folder's two it missed; Join takes them in.
    lib.invoke_tile_clicked(tile("A"), false, false);
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.until("A open", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());
    rig.pump();
    assert_eq!((panel.get_roll_state(), panel.get_roll_measured(), panel.get_roll_unmeasured()), (1, 2, 2));
    panel.invoke_join_roll();
    rig.pump();
    for name in ["C.tif", "D.tif"] {
        assert_eq!(settings_of(&canonical(name)).roll_reference.map(|r| r.id), Some(id), "{name} joined the roll");
    }
    assert_eq!((panel.get_roll_measured(), panel.get_roll_unmeasured()), (4, 0));
    rig.chord(slint::platform::Key::Control, "g");
    rest();
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert!(settings_of(&canonical("C.tif")).roll_reference.is_none(), "one undo takes the join back");

    // Reset to Default from B alone reaches the whole roll, once the key confirms it.
    lib.invoke_tile_clicked(tile("B"), false, false);
    rig.pump();
    assert!(menus.get_live().reset_to_default);
    ctl.act(oni_app::Act::ResetToDefault);
    rig.until("the reach", Duration::from_secs(10), || w.global::<Confirm>().get_shown());
    assert_eq!(w.global::<Confirm>().get_title().as_str(), "Reset 2 Photos to Default?");
    assert_eq!(w.global::<Confirm>().get_key().as_str(), "RESET 2 PHOTOS");
    rig.key("\u{1b}");
    assert!(settings_of(&canonical("A.tif")).roll_reference.is_some(), "twin: Cancel resets nothing");
    ctl.act(oni_app::Act::ResetToDefault);
    rig.until("the reach", Duration::from_secs(10), || w.global::<Confirm>().get_shown());
    rig.key("\r");
    assert!(w.global::<Confirm>().get_shown(), "Reset is destructive: Return does not reach it");
    w.global::<Confirm>().invoke_answered(true);
    rig.pump();
    // A, still open behind the gallery, is written once its edit rests, as every open frame is.
    for name in ["A.tif", "B.tif"] {
        rig.until(&format!("{name} reset"), Duration::from_secs(5), || settings_of(&canonical(name)).roll_reference.is_none());
    }
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    rig.until("the roll back", Duration::from_secs(5), || settings_of(&canonical("A.tif")).roll_reference.is_some());
    rest();

    // The balance key over two frames asks Each or Average first; Cancel runs nothing.
    lib.invoke_tile_clicked(tile("C"), false, false);
    lib.invoke_tile_clicked(tile("D"), true, false);
    rig.pump();
    assert_eq!(panel.get_balance_note().as_str(), "Acts on 2 photos.");
    let before = settings_of(&canonical("C.tif"));
    ctl.act(oni_app::Act::AutoBalance);
    rig.pump();
    let choice = w.global::<Choice>();
    assert!(choice.get_shown());
    assert_eq!(choice.get_title().as_str(), "Auto White Balance on 2 photos");
    let keys: Vec<String> = (0..choice.get_keys().row_count()).filter_map(|i| choice.get_keys().row_data(i)).map(|k| k.text.to_string()).collect();
    assert_eq!(keys, ["Balance Each of the 2", "Average the 2 Balances"]);
    rig.key("\u{1b}");
    assert!(!choice.get_shown() && !panel.get_balancing());
    assert_eq!(settings_of(&canonical("C.tif")), before, "twin: Cancel balances nothing");
    ctl.act(oni_app::Act::AutoBalance);
    rig.pump();
    choice.invoke_chosen(0);
    rig.until("the run", Duration::from_secs(120), || !panel.get_balancing());
    // Both frames read, the key back: a synthetic ramp has no grey to measure, so the run says so.
    assert_eq!(panel.get_balance_progress().as_str(), "Reading 2 of 2\u{2026}");
    assert!(w.global::<oni_app::Canvas>().get_status().contains("nothing to balance"), "{}", w.global::<oni_app::Canvas>().get_status());
    let _ = std::fs::remove_dir_all(&root);
}

/// `TreeRowChrome`: a roll's row draws its stock, its mode's generic one while none is picked.
pub(crate) fn a_rolls_row_draws_its_stock() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the tree is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-roll-stock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (rolled, loose) = (root.join("Rolled"), root.join("Loose"));
    for (folder, name) in [(&rolled, "A.tif"), (&rolled, "B.tif"), (&loose, "C.tif")] {
        common::linear_tiff(&folder.join(name), 96, 64, 0.5);
    }
    // A folder made a roll carries it in its marker, the stock never picked.
    oni_settings::film::roll_folder::set_roll(Some(&Film::new("Roll 9")), &rolled).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&root);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let rows = shell.window.global::<Library>().get_rows();
    let listed: Vec<(i32, String, bool)> = rows.iter().map(|r| (r.kind, r.title.to_string(), r.has_stock)).collect();
    assert!(rows.iter().any(|r| r.kind == 1 && r.title == "Roll 9" && r.has_stock), "the roll draws its stock: {listed:?}");
    // Twin: the folder holding no roll keeps its glyph.
    assert!(rows.iter().any(|r| r.kind == 0 && r.title == "Loose" && !r.has_stock), "{listed:?}");
    println!("shot {}", rig.shot("rolls-stock-row").display());
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
