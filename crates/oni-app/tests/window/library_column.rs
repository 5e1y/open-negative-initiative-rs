//! The library column beside a photo: centred when the screen or the photo opened elsewhere changes, never on an
//! edit or a click in it; a resize keeps rows and scroll; a redrawn thumbnail reaches its row; a double click acts once.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, Store as StoreGlobal, TreeRow};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Image, Model, ModelRc};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

fn rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_rows().iter().collect()
}

fn photo_row(lib: &Library, title: &str) -> usize {
    rows(lib).iter().position(|r| r.kind == 2 && r.title == title).unwrap_or_else(|| panic!("{title} has a row"))
}

fn picture(lib: &Library, title: &str) -> Image {
    rows(lib)[photo_row(lib, title)].picture.clone()
}

/// Lets the system's double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn the_column_follows_the_open_photo_and_keeps_the_wheel_s_scroll() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the library column is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-library-column-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..30 {
        common::plain_tiff(&folder.join(format!("P{i:02}.tif")), 96, 64, 0.3 + i as f32 / 60.0);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("P00 rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.shot("column-beside-p00");
    rig.until("every row's thumbnail", Duration::from_secs(60), || rows(&lib).iter().all(|r| r.kind != 2 || r.picture.size().width > 0));
    rig.settle();

    // A row clicked opens its photo and leaves the column where the hand is; the key centres it.
    let before = lib.get_tree_viewport_y();
    lib.invoke_row_clicked(photo_row(&lib, "P20") as i32, false, false);
    rig.pump();
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("P20.tif"));
    assert_eq!(lib.get_tree_viewport_y(), before, "a click in the column never scrolls it");
    lib.invoke_tree_centre();
    let centred = lib.get_tree_viewport_y();
    assert!(centred < 0.0, "the column is taller than its view, so P20 sits below its top: {centred}");

    // The wheel's scroll stays through every notch on the open photo.
    let scrolled = centred + 30.0;
    lib.set_tree_viewport_y(scrolled);
    let p20 = picture(&lib, "P20");
    let p05 = picture(&lib, "P05");
    for _ in 0..3 {
        w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
        rig.pump();
    }
    assert_eq!(lib.get_tree_viewport_y(), scrolled, "an edit never moves the column");
    lib.invoke_tree_centre();
    assert_eq!(lib.get_tree_viewport_y(), centred, "twin: the centre key brings the photo back to the middle");

    // The edited photo's thumbnail, drawn again, reaches its row; the others keep theirs.
    rig.until("P20's row redrawn", Duration::from_secs(30), || picture(&lib, "P20") != p20);
    assert!(picture(&lib, "P05") == p05, "twin: a photo not edited keeps its picture");

    // Back from the gallery, the column centres on the photo again.
    lib.set_tree_viewport_y(scrolled);
    ctl.act("toggle-mode");
    rig.shot("column-gallery");
    ctl.act("toggle-mode");
    rig.shot("column-back");
    assert_eq!(lib.get_tree_viewport_y(), centred, "a screen change centres the column");

    // A double click on a photo row opens it once; a first click after the interval opens again.
    rest(&rig);
    let opens = ctl.state.borrow().opens();
    let p10 = photo_row(&lib, "P10") as i32;
    lib.invoke_row_clicked(p10, false, false);
    lib.invoke_row_clicked(p10, false, false);
    lib.invoke_row_double_clicked(p10);
    rig.pump();
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("P10.tif"));
    assert_eq!(ctl.state.borrow().opens(), opens + 1, "two clicks and a double click open the photo once");
    rest(&rig);
    lib.invoke_row_clicked(photo_row(&lib, "P12") as i32, false, false);
    assert_eq!(ctl.state.borrow().opens(), opens + 2, "twin: a single click opens");

    // A resize lays the same rows in place and keeps the scroll within the content.
    let model: ModelRc<TreeRow> = lib.get_rows();
    let before = rows(&lib)[photo_row(&lib, "P05")].clone();
    lib.set_tree_viewport_y(-10.0);
    lib.invoke_tree_resized(320.0, 500.0);
    rig.pump();
    assert!(lib.get_rows() == model, "the rows are relaid, not rebuilt");
    let after = rows(&lib)[photo_row(&lib, "P05")].clone();
    assert!(after.width != before.width && after.picture == before.picture, "the row takes the new width and keeps its picture");
    assert_eq!(lib.get_tree_viewport_y(), -10.0, "a resize keeps the scroll");
    lib.set_tree_viewport_y(-100_000.0);
    lib.invoke_tree_resized(320.0, 520.0);
    let floor = -(lib.get_tree_content_height() - 520.0);
    assert!((lib.get_tree_viewport_y() - floor).abs() < 0.5, "the scroll clamped to the content: {} against {floor}", lib.get_tree_viewport_y());
    lib.invoke_tree_layout_next();
    assert!(lib.get_rows() != model, "twin: a new layout lays new rows");

    // The chevron folds a group; a double click on its name types over it, the folder's own name.
    let group = rows(&lib).iter().position(|r| r.kind != 2 && r.expandable).expect("a group row") as i32;
    let unfolded = rows(&lib)[group as usize].expanded;
    lib.invoke_row_folded(group);
    assert_eq!(rows(&lib)[group as usize].expanded, !unfolded, "the chevron folds the group");
    lib.invoke_row_folded(group);
    assert_eq!(rows(&lib)[group as usize].expanded, unfolded, "twin: a second click unfolds it");
    rest(&rig);
    lib.invoke_row_clicked(group, false, false);
    lib.invoke_row_clicked(group, false, false);
    lib.invoke_row_double_clicked(group);
    rig.pump();
    let renamed = rows(&lib)[group as usize].clone();
    assert!(renamed.renaming && renamed.title == "Scans", "a double click types over the folder's name: {}", renamed.title);
    assert_eq!(renamed.expanded, unfolded, "and folds nothing");
    lib.invoke_row_rename_cancelled();
    assert!(!rows(&lib)[group as usize].renaming, "twin: Escape leaves the name as it was");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
