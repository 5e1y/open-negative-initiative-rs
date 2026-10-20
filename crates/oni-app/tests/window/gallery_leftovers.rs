//! The gallery's last gaps on the headless window: back from the editor it lands on the open photo,
//! a band's heading is renamed in place, and files dropped from the Finder open as ⌘O opens them.

use oni_app::app::{self, Options, Shell};
use oni_app::{App, Library};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn click(rig: &common::Rig, (x, y): (f32, f32)) {
    let position = LogicalPosition::new(x, y);
    for e in [
        WindowEvent::PointerMoved { position },
        WindowEvent::PointerPressed { position, button: PointerEventButton::Left },
        WindowEvent::PointerReleased { position, button: PointerEventButton::Left },
    ] {
        rig.window.dispatch_event(e);
        rig.pump();
    }
}

fn typed(rig: &common::Rig, text: &str) {
    for c in text.chars() {
        rig.key(&c.to_string());
    }
}

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

pub(crate) fn the_gallery_lands_on_the_open_photo_renames_its_headings_and_takes_a_drop() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery's leftovers are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gallery-leftovers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, elsewhere, support, caches) = (root.join("Scans"), root.join("Elsewhere"), root.join("Support"), root.join("Caches"));
    for i in 0..96 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 24, 16, 0.5);
    }
    for i in 0..4 {
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i}.tif")), 24, 16, 0.4);
    }
    common::plain_tiff(&elsewhere.join("E1.tif"), 24, 16, 0.3);
    common::plain_tiff(&elsewhere.join("E2.tif"), 24, 16, 0.6);
    std::fs::write(elsewhere.join("notes.txt"), b"not a photo").unwrap();
    let originals: Vec<(PathBuf, Vec<u8>)> = ["E1.tif", "E2.tif", "notes.txt"].iter().map(|n| (elsewhere.join(n), bytes(&elsewhere.join(n)))).collect();
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
    let (lib, app) = (w.global::<Library>(), w.global::<App>());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 100);
    rig.settle();
    let index = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(name)).unwrap_or_else(|| panic!("{name}"));
    let viewport = ctl.state.borrow().grid().1;
    let stop = lib.get_content_height() - viewport;
    assert!(stop > viewport, "the grid scrolls: {stop} past a viewport of {viewport}");

    // Back from the editor, the open photo lands in the viewport's middle; nothing moves before.
    lib.invoke_scrolled(-1e6);
    rig.pump();
    lib.invoke_tile_opened(index("A48.tif") as i32);
    rig.pump();
    assert!(!app.get_gallery());
    assert_eq!(lib.get_scroll(), 0.0, "twin: the grid hidden behind the editor keeps its place");
    ctl.act("toggle-mode");
    rig.pump();
    assert!(app.get_gallery());
    let tile = app::tiles(w).into_iter().find(|t| t.caption == "A48").expect("the open photo is laid");
    let centre = tile.y + tile.height / 2.0 - lib.get_scroll();
    assert!((centre - viewport / 2.0).abs() < 1.0, "centred: {centre} in {viewport}");
    // The first photo opened from the bottom lands back at the top, as far as the content allows.
    lib.invoke_scrolled(1e6);
    rig.pump();
    assert_eq!(lib.get_scroll(), stop);
    lib.invoke_tile_opened(0);
    rig.pump();
    ctl.act("toggle-mode");
    rig.pump();
    assert_eq!(lib.get_scroll(), 0.0, "the first photo clamps the landing to the top");

    // A heading's single click picks nothing; its double click types over it, never opening a photo.
    rig.settle();
    let heading = |title: &str| {
        let shown = lib.get_headings();
        (0..shown.row_count()).filter_map(|i| shown.row_data(i)).find(|h| h.title == title)
    };
    let first = lib.get_headings().row_data(0).expect("a heading at the top");
    let on_heading = (lib.get_grid_x() + 200.0, lib.get_grid_y() + first.y - lib.get_scroll() + 12.0);
    let picked = || app::tiles(w).iter().filter(|t| t.selected).count();
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, on_heading);
    assert_eq!(picked(), 1, "a click on a heading leaves the picks as they were");
    assert!(!heading(&first.title).unwrap().renaming, "twin: one click is no rename");
    click(&rig, on_heading);
    assert!(heading(&first.title).unwrap().renaming, "the double click types over the heading");
    assert!(app.get_gallery() && lib.get_heading_draft().is_empty(), "a folder no roll names opens an empty field");
    rig.pump();
    typed(&rig, "Holiday");
    rig.key(&slint::SharedString::from(Key::Return));
    let renamed = scans.join("Holiday");
    rig.until("the folder renamed", Duration::from_secs(10), || renamed.is_dir());
    assert!(!scans.join(first.title.as_str()).exists(), "the band's own folder took the name");
    rig.until("the heading named", Duration::from_secs(30), || heading("Holiday").is_some());
    assert!(!heading("Holiday").unwrap().renaming);

    // Escape puts the heading back; a click on a cell commits what was typed.
    let holiday = heading("Holiday").unwrap();
    let on_holiday = (lib.get_grid_x() + 200.0, lib.get_grid_y() + holiday.y - lib.get_scroll() + 12.0);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, on_holiday);
    click(&rig, on_holiday);
    typed(&rig, "X");
    rig.key(&slint::SharedString::from(Key::Escape));
    assert!(!heading("Holiday").unwrap().renaming);
    assert!(renamed.is_dir() && !scans.join("HolidayX").exists(), "twin: Escape renames nothing");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, on_holiday);
    click(&rig, on_holiday);
    typed(&rig, "Trip");
    let cell = app::tiles(w).into_iter().find(|t| t.y - lib.get_scroll() > 60.0 && t.y + t.height - lib.get_scroll() < viewport).expect("a cell in view");
    click(&rig, (lib.get_grid_x() + cell.x + cell.width / 2.0, lib.get_grid_y() + cell.y - lib.get_scroll() + cell.height / 2.0));
    rig.until("the folder renamed again", Duration::from_secs(10), || scans.join("Trip").is_dir());

    assert!(app.get_gallery(), "the click that commits picks its cell, opening nothing");

    // A project's heading renames the project, trimmed; an empty name keeps it.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    let t = app::tiles(w).into_iter().find(|t| t.index == 0).unwrap();
    assert!(lib.invoke_context(t.x + t.width / 2.0, t.y + t.height / 2.0));
    let lines = lib.get_menu_lines();
    let at = (0..lines.row_count()).find(|&i| lines.row_data(i).unwrap().text.starts_with("New Project from")).expect("New Project from");
    lib.invoke_menu_chosen(at as i32);
    rig.pump();
    lib.invoke_surface_picked(1);
    rig.pump();
    assert_eq!(lib.get_scroll(), 0.0, "the projects' grid lands afresh");
    let project = lib.get_headings().row_data(0).expect("the project's heading");
    let on_project = (lib.get_grid_x() + 200.0, lib.get_grid_y() + project.y - lib.get_scroll() + 12.0);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, on_project);
    click(&rig, on_project);
    assert_eq!(lib.get_heading_draft(), project.title, "a project's field opens on its name");
    lib.invoke_heading_committed("   ".into());
    rig.pump();
    assert!(heading(&project.title).is_some(), "twin: an empty name keeps the project's");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    click(&rig, on_project);
    click(&rig, on_project);
    lib.invoke_heading_committed("  Portraits ".into());
    rig.pump();
    assert!(heading("Portraits").is_some(), "{:?}", lib.get_headings().row_data(0).map(|h| h.title));
    lib.invoke_surface_picked(0);
    rig.pump();

    // A drop is refused beside a photo, then taken in the gallery as ⌘O takes it.
    lib.invoke_tile_opened(0);
    rig.pump();
    assert!(!ctl.dropped(&[elsewhere.join("E1.tif")]), "twin: the editor takes no drop");
    assert!(ctl.state.borrow().opened().is_some_and(|o| o.ends_with("Scans")));
    ctl.act("toggle-mode");
    rig.pump();
    assert!(ctl.dropped(&[elsewhere.join("notes.txt"), elsewhere.join("E2.tif")]));
    rig.until("the dropped photo open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("E2.tif")));
    assert!(ctl.state.borrow().opened().is_some_and(|o| o.ends_with("Elsewhere")), "the first item's folder opens");
    assert!(!app.get_gallery(), "the first photo among the items opens in the editor");
    ctl.act("toggle-mode");
    rig.pump();
    assert!(ctl.dropped(&[scans.join("Trip")]));
    rig.until("the dropped folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some_and(|o| o.ends_with("Trip")));
    rig.settle();
    assert!(app.get_gallery(), "twin: a folder dropped opens no photo");
    for (path, was) in &originals {
        assert_eq!(&bytes(path), was, "{} is untouched", path.display());
    }
    let _ = std::fs::remove_dir_all(&root);
}
