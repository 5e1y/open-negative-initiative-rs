//! De-dust's corrections as a grid of their numbers: a click on the picture lays a dot, a tile under the
//! hand shows its correction alone with the ring of its source, and a click on the tile removes it.

use oni_app::app::{Options, Shell};
use oni_app::{Canvas, Dedust, Library};
use oni_core::{Archive, EditorStore};
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

fn numbers(w: &oni_app::MainWindow) -> Vec<i32> {
    w.global::<Dedust>().get_rows().iter().map(|r| r.number).collect()
}

fn healed(w: &oni_app::MainWindow) -> bool {
    w.global::<Dedust>().get_rows().iter().all(|r| !r.pending)
}

fn marked(c: &Canvas) -> Vec<i32> {
    c.get_marks().iter().map(|m| m.number).collect()
}

pub(crate) fn a_click_lays_a_dot_and_a_tile_shows_its_correction_alone_with_its_source() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the corrections' grid is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-dedust-grid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 320, 240, 0.5);
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    settings.corrections = (0..3)
        .map(|k| {
            let x = 0.2 + 0.25 * k as f64;
            Correction { id: Id::new(), stroke: vec![Point { x, y: 0.6 }, Point { x: x + 0.04, y: 0.6 }], radius: 4.0 }
        })
        .collect();
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    // SAFETY: set before any thread of this process reads it; the patches land in the check's own folder.
    unsafe { std::env::set_var("ONI_CACHES", &caches) };
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
    let (lib, c, d) = (w.global::<Library>(), w.global::<Canvas>(), w.global::<Dedust>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1 && c.get_frame().size().width > 0);
    ctl.act(oni_app::Act::Correction);
    rig.until("A's patches", Duration::from_secs(60), || healed(w) && c.get_marks().row_count() == 3);
    assert_eq!(numbers(w), [1, 2, 3]);

    // A click on the picture, no drag, lays a dot of the brush's size.
    let (fx, fy, fw, fh) = (c.get_picture_x(), c.get_picture_y(), c.get_picture_width(), c.get_picture_height());
    c.invoke_paint_start(fx + fw * 0.5, fy + fh * 0.25);
    c.invoke_paint_end();
    assert_eq!(numbers(w), [1, 2, 3, 4], "a click lays a correction");
    let dot = ctl.state.borrow().store().settings().corrections.last().cloned().unwrap();
    assert_eq!(dot.stroke.len(), 2, "a dot holds two points, as a stroke does");
    assert!((dot.stroke[1].x - dot.stroke[0].x).abs() < 1e-3 && dot.stroke[1].y == dot.stroke[0].y, "{:?}", dot.stroke);
    // Twin: a press off the picture lays nothing.
    c.invoke_paint_start(fx - 40.0, fy - 40.0);
    c.invoke_paint_end();
    assert_eq!(numbers(w).len(), 4, "a press off the picture lays nothing");
    rig.until("the dot's patch", Duration::from_secs(60), || healed(w) && c.get_marks().row_count() == 4);

    // At rest every marker shows and no source covers the picture.
    assert_eq!(marked(&c), [1, 2, 3, 4]);
    assert_eq!((c.get_source_marks().row_count(), c.get_source_lines().as_str()), (0, ""));
    // The hand on the second tile: its marker alone, the ring of its source and the line to it.
    d.set_hovered_row(1);
    d.invoke_shown(1);
    assert_eq!(marked(&c), [2]);
    assert_eq!(c.get_source_marks().row_count(), 1, "the ring where its patch was copied from");
    assert!(!c.get_source_lines().is_empty());
    rig.settle();
    rig.shot("dedust_grid_hovered");
    // Twin: the hand gone, every marker again and no source.
    d.set_hovered_row(-1);
    d.invoke_shown(-1);
    assert_eq!(marked(&c), [1, 2, 3, 4]);
    assert_eq!(c.get_source_marks().row_count(), 0);

    // The hand on the third, then a click on it: it goes, the others keep their numbers, none held.
    d.set_hovered_row(2);
    d.invoke_shown(2);
    assert_eq!(marked(&c), [3]);
    d.invoke_remove(2);
    assert_eq!(numbers(w), [1, 2, 4], "the others keep their numbers");
    assert_eq!(d.get_hovered_row(), -1, "the removed one held no longer");
    assert_eq!(marked(&c), [1, 2, 4]);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
