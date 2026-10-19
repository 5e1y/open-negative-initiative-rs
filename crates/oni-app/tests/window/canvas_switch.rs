//! A photo switched to on the GPU canvas: nothing of the last photo's picture nor of its before,
//! its own render shown on the emptied canvas, its corrections numbered from one; the De-dust key
//! and brush; a heal that failed is not started again on every notch.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Bar, Canvas, Dedust, Library, PanelState, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

/// A sidecar holding `n` short strokes across the middle of the frame.
fn with_corrections(path: &Path, n: usize) {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.corrections = (0..n)
        .map(|k| {
            let x = 0.3 + 0.2 * k as f64;
            Correction { id: Id::new(), stroke: vec![Point { x, y: 0.5 }, Point { x: x + 0.05, y: 0.5 }], radius: 3.0 }
        })
        .collect();
    sidecar::write(&s, path, WriteOptions::default()).unwrap();
}

fn rows(w: &oni_app::MainWindow) -> Vec<(String, bool)> {
    w.global::<Dedust>().get_rows().iter().map(|r| (r.label.to_string(), r.pending)).collect()
}

fn labels(w: &oni_app::MainWindow) -> Vec<String> {
    rows(w).into_iter().map(|(l, _)| l).collect()
}

fn healed(w: &oni_app::MainWindow) -> bool {
    rows(w).iter().all(|(_, pending)| !pending)
}

pub(crate) fn a_photo_switched_to_shows_nothing_of_the_last_and_numbers_its_corrections_from_one() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the canvas switch is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-canvas-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    with_corrections(&folder.join("A.tif"), 1);
    with_corrections(&folder.join("B.tif"), 2);
    // SAFETY: set before any thread of this process reads it; the patches land in the check's own folder.
    unsafe { std::env::set_var("ONI_CACHES", &caches) };
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let canvas = Some((gpu.device.clone(), gpu.queue.clone()));
    let options = Options { support, caches, canvas, picker: Box::new(|| None), ..Options::default() };
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
    rig.until("A's patch", Duration::from_secs(60), || healed(w));
    assert_eq!(labels(w), ["Correction 1"]);
    ctl.act("compare");
    rig.until("A's before", Duration::from_secs(60), || c.get_before().size().width > 0);

    // B has no thumbnail yet: the canvas empties, A's before with it, until B's render lands.
    ctl.state.borrow_mut().forget_tiles();
    let renders = ctl.state.borrow().renders();
    let b = lib.get_rows().iter().position(|r| r.kind == 2 && r.title == "B").expect("B's row");
    lib.invoke_row_clicked(b as i32);
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.tif"));
    assert_eq!(c.get_frame().size().width, 0, "nothing of A's picture stands under B's name");
    assert_eq!(c.get_before().size().width, 0, "nor A's before");
    rig.until("B's own render on the emptied canvas", Duration::from_secs(30), || ctl.state.borrow().renders() > renders && c.get_frame().size().width > 0);
    rig.until("B's patches", Duration::from_secs(60), || healed(w));
    assert_eq!(labels(w), ["Correction 1", "Correction 2"], "B's corrections number from one, as A's did");

    // The De-dust key goes dark when the tab bar leaves De-dust.
    ctl.act("correction");
    assert!(w.global::<Bar>().get_dedust(), "the key lights on De-dust");
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    assert!(!w.global::<Bar>().get_dedust(), "twin: the tab bar leaving De-dust darkens it");
    ctl.act("correction");

    // A double press on the brush resets its size at once, before the release.
    let rested = format!("{} px", Correction::DEFAULT_RADIUS);
    assert!(!d.invoke_press(false));
    d.invoke_drag(80.0, 200.0, false);
    d.invoke_release();
    assert_ne!(d.get_readout().as_str(), rested, "twin: the drag moved the brush");
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!d.invoke_press(false));
    assert!(d.invoke_press(false), "the second press is a double press");
    assert_eq!(d.get_readout().as_str(), rested, "the reset shows while the press is held");
    d.invoke_release();

    // A notch leaves the corrections' list as it was; a correction painted renews it.
    let list = d.get_rows();
    w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
    assert!(d.get_rows() == list, "a notch rebuilds no correction row");

    // A correction whose patch cannot be generated, its frame's file gone, is not tried again.
    std::fs::remove_file(folder.join("B.tif")).unwrap();
    let (fx, fy, fw, fh) = (c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height());
    c.invoke_paint_start(fx + fw * 0.5, fy + fh * 0.3);
    c.invoke_paint_move(fx + fw * 0.6, fy + fh * 0.3);
    c.invoke_paint_end();
    assert!(d.get_rows() != list, "twin: a painted correction renews the list");
    assert_eq!(rows(w).last(), Some(&("Correction 3".to_string(), true)), "its patch is being generated");
    rig.until("the generation's failure", Duration::from_secs(60), || healed(w));
    w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
    assert!(healed(w), "a notch starts no generation again");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
