//! Edit Together on the headless window: three frames selected open six to a row, a drag on one moves
//! all three alike as one step, a frame left out keeps its own.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::{ComponentHandle, Model};
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn exposure(path: &Path) -> f32 {
    sidecar::read(path, false).map(|l| l.settings.print.exposure).unwrap_or_default()
}

pub(crate) fn frames_opened_together_take_one_drag_as_one_step_and_one_left_out_keeps_its_own() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Edit Together is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-together-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 150, 100, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, canvas) = (w.global::<Library>(), w.global::<Canvas>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| oni_app::app::tiles(w).iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    let path = |name: &str| folder.join(format!("{name}.tif"));
    // Each frame's place on its roll: its rank in the folder's gallery order.
    let mut shown = oni_app::app::tiles(w);
    shown.sort_by_key(|t| t.index);
    let place = |name: &str| (shown.iter().position(|t| t.caption == name).expect("a tile") + 1).to_string();
    let numbers = || {
        let mut n: Vec<String> = canvas.get_numbers().iter().map(|n| n.text.to_string()).collect();
        n.sort();
        n
    };

    lib.invoke_tile_clicked(tile("A"), false, false);
    lib.invoke_tile_clicked(tile("B"), true, false);
    lib.invoke_tile_clicked(tile("C"), true, false);
    rig.pump();
    lib.invoke_tile_opened(tile("B"));
    // Each tile drawn once its source lands: the third picture's centre is a picture, not the black
    // the gap between two tiles shows.
    let at = |x: usize, y: usize| canvas.get_frame().to_rgba8().filter(|b| b.width() as usize > x).map(|b| b.as_slice()[y * b.width() as usize + x]);
    // Within the dither's step: two grounds differ by a code at most.
    let near = |a: Option<slint::Rgba8Pixel>, b: Option<slint::Rgba8Pixel>| a.zip(b).is_some_and(|(a, b)| [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b)].iter().all(|d| *d <= 2));
    let black = |p: Option<slint::Rgba8Pixel>| p.is_some_and(|p| p.r.max(p.g).max(p.b) <= 2);
    // The tiles' box is drawn at the share of its size the view shows, two pixels of slack around it:
    // 3:2 frames 1024 wide, 30 apart.
    let k = || (f64::from(canvas.get_frame().size().width) - 4.0) / f64::from(3 * 1024 + 2 * 30);
    let inside = |x: f64, y: f64| ((2.0 + x * k()) as usize, (2.0 + y * k()) as usize);
    let (third, gap) = (|| inside(2.0 * 1054.0 + 512.0, 341.0), || inside(1039.0, 341.0));
    rig.until("the grid", Duration::from_secs(60), || {
        canvas.get_status().as_str() == "3 frames together" && !near(at(third().0, third().1), at(gap().0, gap().1))
    });
    let size = canvas.get_frame().size();
    assert!((f64::from(size.width - 4) / f64::from(size.height - 4) - 3132.0 / 683.0).abs() < 0.05, "three tiles on one row: {size:?}");
    assert!(black(at(gap().0, gap().1)) && !black(at(third().0, third().1)), "the gaps are black, the pictures not: {:?}", at(gap().0, gap().1));
    // A contact sheet: the black under the frames with a margin, each frame's number on it.
    let sheet = canvas.get_sheet();
    let (fx, fy, fw, fh) = (canvas.get_frame_x(), canvas.get_frame_y(), canvas.get_frame_width(), canvas.get_frame_height());
    assert!(canvas.get_together() && sheet.x < fx && sheet.y < fy && sheet.x + sheet.width > fx + fw && sheet.y + sheet.height > fy + fh, "{fx} {fy} {fw} {fh}");
    // Under less than a row no logo: the row's numbers and a margin end the sheet.
    assert!(((sheet.width / sheet.height) as f64 - 3252.0 / (3.0 * 60.0 + 1024.0 * 2.0 / 3.0)).abs() < 0.02, "a margin twice the gap above, the numbers' band and a margin below: {} × {}", sheet.width, sheet.height);
    assert!(canvas.get_logo().size().width == 0 && canvas.get_logo_box().height == 0.0, "no logo under three frames");
    rig.until("the numbers", Duration::from_secs(10), || numbers().len() == 3);
    assert_eq!(numbers(), ["A", "B", "C"].map(place));
    let selected = |n: &str| oni_app::app::tiles(w).iter().any(|t| t.caption == n && t.selected);
    assert!(["A", "B", "C"].iter().all(|n| selected(n)), "the selection stays the frames shown");

    // Zoomed and panned as a contact sheet: a double click takes the sheet to its own pixels.
    let fitted = canvas.get_frame_width();
    // The sheet moves when its render lands, which a software GPU takes seconds to do.
    canvas.invoke_double_clicked();
    rig.until("the sheet zoomed", Duration::from_secs(60), || (canvas.get_frame_width() - fitted).abs() > 1.0);
    canvas.invoke_double_clicked();
    rig.until("twin: the sheet back to fitted", Duration::from_secs(60), || (canvas.get_frame_width() - fitted).abs() < 1.0);
    rig.settle();
    // The window resized: the sheet's ground and numbers follow its texture at once, before its render.
    let held = || {
        let s = canvas.get_sheet();
        let first = canvas.get_numbers().row_data(0).map_or(0.0, |n| n.y);
        (s.width, canvas.get_frame_width() / s.width, (canvas.get_frame_x() - s.x) / s.width, (first - s.y) / s.height)
    };
    let before = held();
    rig.window.set_size(slint::PhysicalSize::new(1000, 700));
    rig.drawn();
    rig.pump();
    let after = held();
    assert!(canvas.get_frame_width() < fitted - 1.0, "the texture follows the smaller window");
    assert!((after.0 - before.0).abs() > 1.0, "twin: the ground did move");
    assert!((after.1 - before.1).abs() < 0.01 && (after.2 - before.2).abs() < 0.01 && (after.3 - before.3).abs() < 0.01, "{before:?} {after:?}");
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.drawn();
    rig.settle();
    println!("shot {}", rig.shot("together-grid").display());

    let g = w.global::<Store>();
    let field = Field::Exposure.index();
    let left_out = exposure(&path("D"));
    g.invoke_press(field, false);
    for k in 1..=8 {
        g.invoke_drag(field, k as f32 * 5.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(field);
    rig.pump();
    let set = ctl.state.borrow().store().settings().print.exposure;
    rig.until("the others written", Duration::from_secs(10), || ["A", "C"].iter().all(|n| (exposure(&path(n)) - set).abs() < 1e-6));
    assert_eq!(exposure(&path("D")), left_out, "twin: a frame not selected keeps its own");
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    let back = ctl.state.borrow().store().settings().print.exposure;
    // Auto White Balance over frames together lays their average on all of them, without asking.
    let filters = |p: &Path| sidecar::read(p, false).ok().and_then(|l| l.settings.balance_filters);
    ctl.act(oni_app::Act::AutoBalance);
    rig.until("the balance landed", Duration::from_secs(60), || filters(&path("A")).is_some() && filters(&path("C")).is_some());
    let lead = ctl.state.borrow().store().settings().balance_filters;
    assert!(lead.is_some() && filters(&path("A")) == lead && filters(&path("C")) == lead, "{lead:?}, {:?}", filters(&path("A")));
    assert_eq!(filters(&path("D")), None, "twin: a frame left out keeps its own");
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert!((back - set).abs() > 1e-6, "the drag was undone");
    for n in ["A", "C"] {
        assert!((exposure(&path(n)) - back).abs() < 1e-6, "{n}: one undo takes every frame back");
    }

    // Back to the gallery and into one frame: the grid gives the canvas back to that frame alone.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    lib.invoke_tile_clicked(tile("D"), false, false);
    lib.invoke_tile_opened(tile("D"));
    rig.until("D alone", Duration::from_secs(60), || canvas.get_status().contains(" × "));
    assert!(ctl.state.borrow().store().together().is_empty());
    assert!(!canvas.get_together() && numbers().is_empty(), "twin: one frame alone on the canvas's own ground");
    // The full resolution is built as the frame opens, for every render, a stroke or a zoom.
    rig.until("D's full resolution", Duration::from_secs(10), || ctl.state.borrow().full_built());

    // Beside the photo, a ⌘-click in the column shows the two frames together at once, no double click.
    let row = |n: &str| lib.get_rows().iter().position(|r| r.kind == 2 && r.title == n).unwrap_or_else(|| panic!("{n}'s row")) as i32;
    lib.invoke_row_clicked(row("A"), true, false);
    rig.until("D and A together", Duration::from_secs(30), || canvas.get_status().as_str() == "2 frames together");
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("D.tif")), "the open frame leads");
    rig.until("their numbers", Duration::from_secs(30), || numbers().len() == 2);
    let mut roll = vec![place("A"), place("D")];
    roll.sort();
    assert_eq!(numbers(), roll, "twin: numbered by the roll, not by the selection");
    // Twin: A taken out again leaves D alone on the canvas.
    lib.invoke_row_clicked(row("A"), true, false);
    rig.until("D alone again", Duration::from_secs(30), || canvas.get_status().contains(" × "));
    let _ = std::fs::remove_dir_all(&root);
}
