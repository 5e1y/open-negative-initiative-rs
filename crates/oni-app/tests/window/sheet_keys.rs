//! A sheet hands the keyboard to its default key, named, so Return presses it and an assistive tool
//! lands on it; the window's bar behind the sheet reads out of reach until it closes.

use oni_app::app::{Options, Shell};
use oni_app::{Advice, AppSettings, Confirm, Library, Presets, Told};
use oni_core::{Archive, EditorStore};
use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementRoot};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, SharedString};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The keys so named, the bar's or the sheet's by where they stand, with whether they are in reach.
fn keys(w: &oni_app::MainWindow, label: &str, in_bar: bool) -> Vec<bool> {
    ElementHandle::find_by_accessible_label(w, label)
        .filter(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .filter(|e| (e.absolute_position().y < 60.0) == in_bar)
        .map(|e| e.accessible_enabled().unwrap_or(true))
        .collect()
}

/// The bar's keys in reach, each found once.
fn bar_live(w: &oni_app::MainWindow, labels: &[&str]) -> Vec<bool> {
    labels.iter().map(|l| {
        let found = keys(w, l, true);
        assert_eq!(found.len(), 1, "one bar key named {l:?}");
        found[0]
    }).collect()
}

/// Every key shown in the window carries a name an assistive tool reads.
fn unnamed(w: &oni_app::MainWindow) -> usize {
    w.root_element().query_descendants().match_descendants().match_accessible_role(AccessibleRole::Button).find_all()
        .iter()
        .filter(|e| e.accessible_label().is_none_or(|l| l.trim().is_empty()))
        .count()
}

pub(crate) fn a_sheets_default_key_holds_the_keyboard_and_the_bar_waits_behind_it() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the sheets' keys are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sheet-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    common::linear_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    // The evaluated window, wide enough for the bar's words.
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let presets = w.global::<Presets>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    // Return as the window hands it over, Slint's own key.
    let ret = SharedString::from(Key::Return);
    let bar = ["Crop", "Compare", "Copy", "De-dust", "Settings", "EXPORT"];
    // Twin: no sheet, the window's monitor holds the keyboard and the bar is in reach.
    assert_eq!(common::focused_label(w), "", "the photos hold the keyboard");
    assert!(bar_live(w, &bar).iter().all(|l| *l), "with no sheet, the bar's keys are live");

    // Copy Settings, the window's keyboard moved before the sheet is built, as a frame drawn builds it:
    // its COPY holds the keyboard, every key named, the bar out of reach behind it.
    ctl.act(oni_app::Act::Copy);
    rig.pump();
    rig.drawn();
    assert!(presets.get_copying());
    assert_eq!(common::focused_label(w), "Copy", "the sheet's default key holds the keyboard");
    assert_eq!((keys(w, "Copy", false), keys(w, "Cancel", false)), (vec![true], vec![true]), "the sheet's keys, named");
    assert_eq!(unnamed(w), 0, "every key shown is named");
    assert!(bar_live(w, &bar).iter().all(|l| !*l), "behind the sheet, the bar is out of reach");
    println!("shot {}", rig.shot("sheet-keys-copy").display());
    // An assistive tool's press on a bar key behind the sheet reaches nothing.
    let settings = || ElementHandle::find_by_accessible_label(w, "Settings").find(|e| e.accessible_role() == Some(AccessibleRole::Button)).unwrap();
    settings().invoke_accessible_default_action();
    rig.pump();
    assert!(!w.global::<AppSettings>().get_shown(), "Settings stays shut behind the sheet");
    rig.key(&ret);
    assert!(!presets.get_saving(), "Return presses COPY");
    rig.settle();
    assert_eq!(common::focused_label(w), "", "closed, the photos hold the keyboard again");
    assert!(bar_live(w, &bar).iter().all(|l| *l), "twin: the sheet gone, the bar is live again");

    // The sheet built before the window's keyboard moves: its key still ends holding it; Escape leaves.
    ctl.act(oni_app::Act::Copy);
    rig.drawn();
    rig.pump();
    assert_eq!(common::focused_label(w), "Copy", "the key built first still holds the keyboard");
    rig.key(&SharedString::from(Key::Escape));
    assert!(!presets.get_saving(), "Escape from the default key closes the sheet");
    rig.settle();

    // A notice's OK holds the keyboard and Return closes it.
    let advice = w.global::<Advice>();
    advice.set_title("Nothing to paste".into());
    advice.set_message("Copy a photo's settings first.".into());
    advice.set_shown(true);
    rig.settle();
    rig.drawn();
    assert_eq!(common::focused_label(w), "OK", "a notice's key holds the keyboard");
    rig.key(&ret);
    assert!(!advice.get_shown(), "Return presses OK");
    rig.settle();

    // A question whose key removes has no default: Return takes nothing, Escape cancels.
    let confirm = w.global::<Confirm>();
    confirm.set_title("Remove?".into());
    confirm.set_key("Remove".into());
    confirm.set_destructive(true);
    confirm.set_shown(true);
    rig.settle();
    rig.drawn();
    assert_eq!(common::focused_label(w), "", "no key of a removing question takes the keyboard");
    rig.key(&ret);
    assert!(confirm.get_shown(), "Return removes nothing");
    rig.key(&SharedString::from(Key::Escape));
    assert!(!confirm.get_shown(), "Escape cancels");
    rig.settle();

    // A passing notice leaves the keyboard to the photos and names its keys.
    let told = w.global::<Told>();
    told.set_title("Paste to Roll".into());
    told.set_message("Crop pasted on 23 photos of the roll.".into());
    told.set_action("Undo".into());
    told.set_shown(true);
    rig.settle();
    rig.drawn();
    assert_eq!((common::focused_label(w), keys(w, "Dismiss", false), keys(w, "Undo", false)), (String::new(), vec![true], vec![true]));
    assert!(bar_live(w, &bar).iter().all(|l| *l), "a passing notice covers nothing");

    // Twin: the sheet gone, the bar's Settings opens to an assistive tool's press.
    settings().invoke_accessible_default_action();
    rig.pump();
    assert!(w.global::<AppSettings>().get_shown(), "twin: with no sheet, Settings opens");
    let _ = std::fs::remove_dir_all(&root);
}
