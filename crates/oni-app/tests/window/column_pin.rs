//! The editor's column with its pictures two to a row: the roll held at its top covers what scrolls under
//! it, its ground and the box's clip on whole device pixels, so no edge pixel is left half to each.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Library;
use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// Device pixels to a point: a fraction, where a box the column lays can start between two.
const SCALE: f32 = 1.5;
/// `Space.s`: the photos' header padding over the tree's box.
const PAD: f32 = 8.0;

pub(crate) fn the_roll_held_at_the_column_s_top_covers_what_scrolls_under_it() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the column's held roll is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-column-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Archive"), root.join("Support"), root.join("Caches"));
    for r in 1..=4 {
        let roll = folder.join("2026").join(format!("2026-08-18 {r}"));
        for i in 0..12 {
            common::pixels_tiff(&roll.join(format!("R{r}P{i:02}.tif")), 96, 64, |x, y| { let v = 0.05 + 0.9 * ((x + y) % 32) as f32 / 31.0; [v, 1.0 - v, 0.5] });
        }
        oni_library::folders::set_roll(Some(&oni_settings::Film::new(&format!("2026-08-18 {r}"))), &oni_library::paths::canonical(&roll)).unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    // A point at 1.5 device pixels, where the column lays the tree's box at a fractional one.
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(slint::PhysicalSize::new(1920, 1200));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("the photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    lib.invoke_tree_layout_picked(1);
    rig.settle();
    common::scroll_tree(&lib, -200.0);
    rig.until("the pictures in view", Duration::from_secs(60), || lib.get_rows().iter().filter(|r| r.kind == 2 && r.y + r.height > -lib.get_tree_viewport_y() && r.y < -lib.get_tree_viewport_y() + 600.0).all(|r| r.picture.size().width > 0));
    rig.settle();
    let pin = lib.get_tree_pin();
    assert!(pin.shown && pin.push == 0.0, "a roll held at the top, unpushed: {:?}", (pin.shown, pin.push));

    // The held row's ground, and the clip over what scrolls under it, start on a whole device pixel; twin:
    // the box the column lays starts between two.
    let held = common::placed(w, Role::ListItem, &pin.row.title).into_iter().map(|p| p.y).fold(f32::MIN, f32::max);
    let picker = common::the(w, Role::Button, "Two per row");
    let laid = picker.y + picker.height + PAD;
    let whole = |y: f32| ((y * SCALE) - (y * SCALE).round()).abs() < 0.01;
    assert!(whole(held), "the held row on a whole device pixel: {held} at {SCALE}");
    assert!(!whole(laid) && held > laid && held - laid < 1.0 / SCALE + 0.01, "twin: the box laid between two: {laid}, held at {held}");
    println!("shot {}", rig.shot("column-pin").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
