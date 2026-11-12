//! The editor's filmstrip numbers each frame on its roll beside its name; the status bar says the open
//! frame's place on the roll and, once its sidecar holds what it shows, that its settings are saved.

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementRoot};
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, MainWindow, Store};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The status bar's words as drawn: the one text shown that starts with the open frame's name.
fn bar(w: &MainWindow, name: &str) -> String {
    let lines = lines(w, name);
    assert_eq!(lines.len(), 1, "one status line for {name}: {lines:?}");
    lines[0].clone()
}

/// Every text shown that starts with the frame's name, as the status bar draws its line.
fn lines(w: &MainWindow, name: &str) -> Vec<String> {
    w
        .root_element()
        .query_descendants()
        .match_accessible_role(AccessibleRole::Text)
        .find_all()
        .into_iter()
        .filter_map(|e: ElementHandle| e.accessible_label().map(|l| l.to_string()))
        .filter(|l| l.starts_with(&format!("{name} · ")))
        .collect()
}

/// Whether `inner` lies within `outer`'s width and below its middle, as a caption under a picture.
fn under(inner: &common::Placed, outer: &common::Placed) -> bool {
    inner.x >= outer.x && inner.x + inner.width <= outer.x + outer.width + 0.5 && inner.y > outer.y + outer.height / 2.0 && inner.y < outer.y + outer.height
}

pub(crate) fn the_filmstrip_numbers_its_frames_and_the_status_bar_says_the_place_and_the_save() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the filmstrip is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-filmstrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..3 {
        common::plain_tiff(&folder.join(format!("P{i:02}.tif")), 96, 64, 0.3 + i as f32 / 10.0);
    }
    // A sidecar from a newer format: P02 opens read-only.
    std::fs::write(oni_settings::sidecar::url_for(&folder.join("P02.tif")), r#"{"version": 999}"#).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, canvas) = (w.global::<Library>(), w.global::<Canvas>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let second = oni_app::app::tiles(w).iter().find(|t| t.caption == "P01").map(|t| t.index).expect("P01's tile");
    lib.invoke_tile_opened(second);
    rig.until("P01 rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    println!("shot {}", rig.shot("filmstrip-p01").display());

    // Each photo row carries its place on the roll, the folders none.
    let rows: Vec<_> = lib.get_rows().iter().collect();
    let numbered: Vec<(String, String)> = rows.iter().filter(|r| r.kind == 2).map(|r| (r.title.to_string(), r.number.to_string())).collect();
    assert_eq!(numbered, [("P00", "1"), ("P01", "2"), ("P02", "3")].map(|(t, n)| (t.to_string(), n.to_string())));
    assert!(rows.iter().filter(|r| r.kind != 2).all(|r| r.number.is_empty()), "a folder's row has no number");
    // Drawn: the number sits under the frame's picture, before its name.
    let tile = |name: &str| common::placed(w, AccessibleRole::ListItem, name).into_iter().next().unwrap_or_else(|| panic!("{name}'s tile shown"));
    let (p00, p01) = (tile("P00"), tile("P01"));
    let two = common::placed(w, AccessibleRole::Text, "2").into_iter().find(|t| under(t, &p01)).expect("2 drawn under P01's picture");
    let name = common::placed(w, AccessibleRole::Text, "P01").into_iter().find(|t| under(t, &p01)).expect("P01 drawn under its picture");
    assert!(two.x + two.width <= name.x && name.level_with(&two), "the number reads before the name: {two:?} {name:?}");
    // Twin: P00's caption carries its own number, not P01's.
    assert!(!common::placed(w, AccessibleRole::Text, "2").iter().any(|t| under(t, &p00)));

    // The status bar says the frame's place, and that its settings are saved once the opening's write landed.
    assert!(canvas.get_status().starts_with("P01 · Frame 2 of 3 · "), "{}", canvas.get_status());
    let sidecar = oni_settings::sidecar::url_for(&folder.join("P01.tif"));
    rig.until("the opening written", Duration::from_secs(10), || canvas.get_saved());
    assert!(sidecar.exists(), "said saved, the sidecar is on the disk");
    rig.until("the line drawn again", Duration::from_secs(30), || bar(w, "P01").ends_with(" · Settings saved"));

    // An edit hides it until its own write lands.
    w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
    rig.pump();
    assert!(!canvas.get_saved() && !bar(w, "P01").contains("Settings saved"), "an edit not written yet is not said saved");
    let edited = oni_settings::sidecar::read(&folder.join("P01.tif"), false, true).unwrap().settings;
    rig.until("the edit written", Duration::from_secs(10), || canvas.get_saved());
    let written = oni_settings::sidecar::read(&folder.join("P01.tif"), false, true).unwrap().settings;
    assert_ne!(written, edited, "said saved, the disk holds the edit");
    rig.until("the line drawn again", Duration::from_secs(30), || bar(w, "P01").ends_with(" · Settings saved"));
    println!("shot {}", rig.shot("filmstrip-saved").display());

    // A read-only frame says its own place and never that its settings are saved.
    let third = oni_app::app::tiles(w).iter().find(|t| t.caption == "P02").map(|t| t.index).expect("P02's tile");
    let renders = ctl.state.borrow().renders();
    lib.invoke_tile_opened(third);
    rig.until("P02 rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert!(canvas.get_status().starts_with("P02 · Frame 3 of 3 · "), "{}", canvas.get_status());
    assert!(!canvas.get_status().contains("Frame 2"), "twin: the place follows the open frame");
    assert!(canvas.get_read_only() && !canvas.get_saved(), "a sidecar from a newer format is never said saved");
    // Read as it is drawn: the bar gives its room to work under way, as a redrawn thumbnail, and then the line again.
    let line = std::cell::RefCell::new(String::new());
    rig.until("P02's line drawn", Duration::from_secs(30), || lines(w, "P02").first().map(|l| *line.borrow_mut() = l.clone()).is_some());
    assert!(!line.borrow().contains("Settings saved"), "{}", line.borrow());
    let _ = std::fs::remove_dir_all(&root);
}
