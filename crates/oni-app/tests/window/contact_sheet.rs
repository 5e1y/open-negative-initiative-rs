//! The contact sheet on the headless window: the measure lays the roll's sheet beside its first frame,
//! New Contact Sheet composes the selection and opens it, the Details tab dims what a sheet skips, and
//! a second measure writes over the roll's former sheet rather than adding one.

use oni_app::app::{Options, Shell};
use oni_app::{Bar, Library, Menus, PanelState, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use oni_settings::{Id, PipelineSettings};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn settings(path: &Path) -> PipelineSettings {
    sidecar::read(path, false, true).unwrap().settings
}


/// The sheets the folder holds, by name.
fn sheets(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "tif") && p.to_string_lossy().contains("contact sheet"))
        .collect();
    found.sort();
    found
}

pub(crate) fn a_roll_lays_its_sheet_and_a_selection_composes_one_that_opens_dimming_what_a_sheet_skips() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the contact sheet is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sheet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 150, 100, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
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
    let (lib, menus, bar, p) = (w.global::<Library>(), w.global::<Menus>(), w.global::<Bar>(), w.global::<Preparing>());
    let advice = w.global::<oni_app::Advice>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| {
        let tiles = oni_app::app::tiles(w);
        tiles.iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index
    };

    // `canMakeSheet`: two photos at least, the menu and both keys on one rule and one title.
    assert!(!menus.get_live().contact_sheet && !bar.get_can_sheet());
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    assert!(!menus.get_live().contact_sheet, "twin: one photo is no sheet");
    assert_eq!(menus.get_titles().sheet.as_str(), "New Contact Sheet");
    lib.invoke_tile_clicked(tile("B"), true, false);
    rig.pump();
    assert!(menus.get_live().contact_sheet && bar.get_can_sheet());
    assert_eq!(menus.get_titles().sheet.as_str(), "New Contact Sheet from 2 Photos");
    assert_eq!(bar.get_sheet_title(), menus.get_titles().sheet);

    // A roll never measured has no window to carry: the gesture says so and writes nothing.
    ctl.act(oni_app::Act::ContactSheet);
    rig.pump();
    assert!(advice.get_shown());
    assert_eq!(advice.get_message().as_str(), "Prepare this roll first: a contact sheet carries the window its roll was measured on.");
    assert!(sheets(&folder).is_empty());
    advice.set_shown(false);

    // Prepare the Roll measures the four frames and lays their sheet beside the first, adopted unopened.
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(60), || p.get_shown());
    rig.until("every frame read", Duration::from_secs(60), || p.get_capture() == 0);
    p.invoke_step_picked(4);
    rig.until("the window step", Duration::from_secs(60), || p.get_action_title() == "MEASURE 4 FRAMES");
    p.invoke_advance();
    rig.pump();
    rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
    assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    rig.pump();
    let laid = sheets(&folder);
    assert_eq!(laid.len(), 1, "{laid:?}");
    let first = laid[0].clone();
    let day = first.file_name().unwrap().to_string_lossy().into_owned();
    assert!(day.ends_with(" — contact sheet.tif") && day.len() == "2026-05-07 — contact sheet.tif".len(), "{day}");
    let roll = settings(&folder.join("B.tif")).roll_reference.map(|r| r.id);
    let sheet = settings(&first);
    let grid = sheet.sheet.clone().expect("the sheet's sidecar carries its grid");
    assert_eq!((grid.columns, grid.rows, grid.frames, grid.windows.as_ref().map(Vec::len)), (6, 1, 4, Some(4)));
    assert_eq!(sheet.roll_reference.map(|r| r.id), roll, "the sheet stands on its roll's reference");
    assert!(sheet.flat_field.is_neutral(), "and never takes a flat-field of its own");
    assert!(ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")), "twin: the measure's sheet is adopted, not opened");
    rig.until("the sheet's tile", Duration::from_secs(60), || oni_app::app::tiles(w).iter().any(|t| t.caption == day.trim_end_matches(".tif")));
    let decoded = oni_app::render::decode_full(&first, oni_app::render::Reading::default()).unwrap().linear;
    let (gw, gh) = (decoded.width as f64, decoded.height as f64);
    let expected = (6.0 * grid.tile.width + 7.0 * grid.margin, grid.tile.height + 2.0 * grid.margin);
    assert_eq!((gw, gh), expected, "the file is exactly its grid");
    assert!(gw * gh <= oni_measure::sheet::MAX_PIXELS);
    // The first cell's centre holds the frame's picture; the margin beside it stays clear.
    let centre = decoded.pixels[(gh / 2.0) as usize * decoded.width + (grid.margin + grid.tile.width / 2.0) as usize];
    let gutter = decoded.pixels[(gh / 2.0) as usize * decoded.width + (grid.margin / 2.0) as usize];
    assert!(centre[3] > 0.99 && centre[0] > 0.01, "{centre:?}");
    assert!(gutter[3] < 1e-3, "{gutter:?}");

    // New Contact Sheet over three photos: counted per frame, written beside the first, then opened.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    lib.invoke_tile_clicked(tile("B"), false, false);
    lib.invoke_tile_clicked(tile("C"), true, false);
    lib.invoke_tile_clicked(tile("D"), true, false);
    rig.pump();
    assert_eq!(menus.get_titles().sheet.as_str(), "New Contact Sheet from 3 Photos");
    bar.invoke_act(oni_app::Act::ContactSheet);
    assert!(!menus.get_live().contact_sheet, "a run under way bars a second");
    assert_eq!(menus.get_titles().sheet.as_str(), "Making Contact Sheet…");
    let second = folder.join(format!("{} 2.tif", day.trim_end_matches(".tif")));
    rig.until("the sheet opened", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|o| Path::new(o) == second) || advice.get_shown());
    assert!(!advice.get_shown(), "{}", advice.get_message());
    assert!(first.exists() && second.exists(), "a second sheet is numbered, never written over the first");
    let made = settings(&second);
    assert_eq!(made.sheet.as_ref().map(|g| g.frames), Some(3));
    assert_eq!(made.roll_reference.map(|r| r.id), roll);
    assert!(menus.get_live().contact_sheet == false && !bar.get_can_sheet(), "the open sheet alone is selected");

    // On the sheet the Details tab dims sharpening and chroma and says why; a photograph does not.
    let panel = w.global::<PanelState>();
    rig.until("the sheet decoded", Duration::from_secs(30), || panel.get_sheet());
    panel.set_tab(3);
    rig.settle();
    println!("shot {}", rig.shot("sheet-details").display());
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    lib.invoke_tile_opened(tile("A"));
    rig.until("A open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")));
    assert!(!panel.get_sheet(), "twin: a photograph's details stay live");

    // A second measure of the roll writes over its former sheet instead of adding a third.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll again", Duration::from_secs(60), || p.get_shown());
    rig.until("every frame read", Duration::from_secs(60), || p.get_capture() == 0);
    p.invoke_step_picked(4);
    rig.until("the window step", Duration::from_secs(60), || p.get_action_title() == "MEASURE 4 FRAMES");
    p.invoke_advance();
    rig.pump();
    rig.until("the second measure", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
    assert!(!advice.get_shown(), "{}", advice.get_message());
    rig.pump();
    let now = settings(&folder.join("B.tif")).roll_reference.map(|r| r.id);
    assert_ne!(now, roll, "the roll stands on a new reference");
    assert_eq!(sheets(&folder).len(), 2, "{:?}", sheets(&folder));
    let on: Vec<Option<Id>> = sheets(&folder).iter().map(|s| settings(s).roll_reference.map(|r| r.id)).collect();
    assert_eq!(on.iter().filter(|r| **r == now).count(), 1, "one former sheet now stands on the new reference: {on:?}");
    let _ = std::fs::remove_dir_all(&root);
}
