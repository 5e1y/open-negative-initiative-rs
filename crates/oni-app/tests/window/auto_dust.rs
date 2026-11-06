//! Auto De-dust through the tab's two keys on a frame holding one speck past its window: nothing written
//! by the reading, one undoable correction by the removal, its patch, and a second reading over it.

use oni_app::app::{Options, Shell};
use oni_app::{Act, Canvas, Dedust};
use oni_core::{Archive, EditorStore};
use oni_settings::levels::Levels;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// A hand's drag at `y`, rightwards by `by`, through the window's own pointer events.
fn dragged(rig: &common::Rig, x: f32, y: f32, by: f32) {
    let at = |x: f32| LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(x) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at(x), button: PointerEventButton::Left });
    rig.pump();
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(x + by) });
    rig.pump();
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at(x + by), button: PointerEventButton::Left });
    rig.pump();
}

/// The window's pixels the lit colour holds, and their mean brightness, each channel at 0 to 1.
fn on_screen(rig: &common::Rig) -> (usize, f64) {
    let (_, pixels) = rig.drawn();
    let channels = |v: u16| (f64::from(v >> 11) / 31.0, f64::from((v >> 5) & 63) / 63.0, f64::from(v & 31) / 31.0);
    let lit = pixels.iter().filter(|p| matches!(channels(p.0), (r, g, b) if r > 0.9 && (0.4..0.8).contains(&g) && b < 0.25)).count();
    let sum: f64 = pixels.iter().map(|p| { let (r, g, b) = channels(p.0); r + g + b }).sum();
    (lit, sum / 3.0 / pixels.len().max(1) as f64)
}

/// A key pressed while the frame decodes, its thumbnail kept: the opening itself brings the full resolution, as
/// the Settings' whole decode does for a RAW, so no later build announces it.
pub(crate) fn a_detect_pressed_before_the_frame_landed_reads_once_it_lands() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: auto De-dust is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-auto-dust-early-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Scans"), root.join("Support"));
    common::pixels_tiff(&folder.join("A.tif"), 1600, 1200, |x, y| {
        let speck = (800..806).contains(&x) && (600..606).contains(&y);
        let v = if speck { 0.0 } else { 0.8 + 0.1 * ((x as f32 * 0.13).sin() * (y as f32 * 0.17).cos()) };
        [v, v, v]
    });
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    let window = Levels::with_ends(-0.5, 1.0);
    (settings.levels.red, settings.levels.green, settings.levels.blue) = (window, window, window);
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches: oni_app::patches::caches(), picker: Box::new(|| None), ..Options::default() };
    let engine = oni_app::render::Engine::shared(&gpu);
    engine.lock().unwrap().caches = Some(oni_app::patches::caches());
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, d) = (w.global::<oni_app::Library>(), w.global::<Dedust>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    // As a second launch finds a roll: its thumbnail drawn, no preview in memory, so the opening reads the frame.
    rig.until("the thumbnail drawn", Duration::from_secs(60), || {
        rig.pump();
        lib.get_tiles().row_data(0).is_some_and(|t| t.picture.size().width > 0)
    });
    ctl.state.borrow().forget_previews();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    // Before anything landed: the decode is still on its thread and no event has been pumped.
    assert_eq!(ctl.state.borrow().renders(), 0, "twin: the frame has not landed");
    ctl.act(Act::Correction);
    d.invoke_detect();
    assert!(d.get_detecting(), "the key is taken");
    rig.until("the reading asked early", Duration::from_secs(15), || {
        rig.pump();
        d.get_found_shown()
    });
    assert!(!d.get_detecting(), "the note says it no longer reads");
    assert_eq!(d.get_found(), 1, "and it reads what a press after the landing reads");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_reading_writes_nothing_and_its_removal_one_patch_per_speck() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: auto De-dust is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-auto-dust-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Scans"), root.join("Support"));
    // A varied ground for the patch's search to read, and an opaque speck of six pixels across.
    common::pixels_tiff(&folder.join("A.tif"), 1600, 1200, |x, y| {
        let speck = (800..806).contains(&x) && (600..606).contains(&y);
        let v = if speck { 0.0 } else { 0.8 + 0.1 * ((x as f32 * 0.13).sin() * (y as f32 * 0.17).cos()) };
        [v, v, v]
    });
    // The window the frame was balanced on: its ground inside, the speck far past its white.
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    let window = Levels::with_ends(-0.5, 1.0);
    (settings.levels.red, settings.levels.green, settings.levels.blue) = (window, window, window);
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches: oni_app::patches::caches(), picker: Box::new(|| None), ..Options::default() };
    let engine = oni_app::render::Engine::shared(&gpu);
    engine.lock().unwrap().caches = Some(oni_app::patches::caches());
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, d, canvas) = (w.global::<oni_app::Library>(), w.global::<Dedust>(), w.global::<Canvas>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("the frame rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let corrections = || ctl.state.borrow().store().settings().corrections.clone();
    ctl.act(Act::Correction);
    rig.pump();
    assert!(canvas.get_painting());
    assert!(!canvas.get_dust_shown(), "nothing shows until the key is pressed");
    rig.settle();
    let bare = on_screen(&rig);

    d.invoke_detect();
    rig.until("the reading", Duration::from_secs(60), || {
        rig.pump();
        d.get_found_shown()
    });
    assert_eq!(d.get_found(), 1, "one speck");
    assert!(canvas.get_dust_shown());
    assert_eq!(canvas.get_dust_marks().row_count(), 1);
    assert_eq!((canvas.get_dust_mask().size().width, canvas.get_dust_mask().size().height), (1600, 1200));
    assert!(corrections().is_empty(), "a reading writes nothing");
    // Along the slider and back: what a key writes is what shows at the rest it is left on.
    d.invoke_sensitivity_press(false);
    d.invoke_sensitivity_drag(300.0, 300.0, false);
    d.invoke_sensitivity_release();
    rig.pump();
    assert_eq!(d.get_sensitivity_text().as_str(), "100%");
    assert!(d.get_found() >= 1, "a speck once found stays found");
    d.invoke_sensitivity_press(false);
    d.invoke_sensitivity_drag(-300.0, 300.0, false);
    d.invoke_sensitivity_release();
    rig.pump();
    assert_eq!((d.get_sensitivity_text().as_str(), d.get_found()), ("0%", 1), "twin: back at rest, the one speck");
    // The same through the window's pointer: down the panel until a drag moves the slider, the tab's first control.
    let x = 1280.0 - w.global::<oni_app::DetailLoupe>().get_band_width() / 2.0 - 30.0;
    // Past the double click the keys above made, which would read the press as a reset.
    let past_a_double = oni_app::system::double_click() + Duration::from_millis(50);
    std::thread::sleep(past_a_double);
    let mut y = 0.0;
    while d.get_sensitivity() == 0.0 && y < 800.0 {
        dragged(&rig, x, y, 40.0);
        y += 4.0;
    }
    assert!(d.get_sensitivity() > 0.0, "a hand's drag moves the slider");
    assert!(d.get_found_shown(), "and leaves the reading up");
    let moved = d.get_sensitivity();
    // A press off the knob lands it under the pointer; the drag then carries it by its share of the track.
    std::thread::sleep(past_a_double);
    dragged(&rig, x, y - 4.0, 0.0);
    let landed = d.get_sensitivity();
    assert!((moved - landed - 40.0 / 308.0).abs() < 0.02, "by the drag's share of the track: {moved} from {landed}");
    std::thread::sleep(past_a_double);
    dragged(&rig, x, y - 4.0, -20.0);
    let back = d.get_sensitivity();
    assert!(back > 0.0 && back < moved, "twin: dragged back, it comes back by as much, no reset: {back} from {moved}");
    std::thread::sleep(past_a_double);
    d.invoke_sensitivity_press(false);
    d.invoke_sensitivity_drag(-300.0, 300.0, false);
    d.invoke_sensitivity_release();
    rig.pump();
    rig.settle();
    let veiled = on_screen(&rig);
    assert!(veiled.0 > bare.0, "the speck lit on the canvas: {veiled:?} against {bare:?}");
    assert!(veiled.1 < bare.1, "the picture darker under the veil: {veiled:?} against {bare:?}");

    d.invoke_remove_found();
    rig.pump();
    let written = corrections();
    assert_eq!(written.len(), 1, "one correction per speck");
    assert!(!canvas.get_dust_shown(), "the veil goes with the reading");
    // Across the speck's six pixels, and the margin past them.
    assert!(written[0].radius >= 8.0, "{}", written[0].radius);
    let (x, y) = (written[0].stroke[0].x * 1600.0, written[0].stroke[0].y * 1200.0);
    assert!((798.0..808.0).contains(&x) && (598.0..608.0).contains(&y), "{x},{y}");
    let patch = oni_app::patches::file(&oni_app::patches::caches(), written[0].id);
    rig.until("the patch", Duration::from_secs(60), || {
        rig.pump();
        patch.exists()
    });

    d.invoke_detect();
    rig.until("the second reading", Duration::from_secs(60), || {
        rig.pump();
        d.get_found_shown()
    });
    assert_eq!(d.get_found(), 0, "the patch healed what the first reading showed");
    assert!(canvas.get_dust_shown(), "the veil shows even with no dust");
    d.invoke_detect();
    rig.pump();
    assert!(!canvas.get_dust_shown(), "twin: the key takes it away");
    rig.settle();
    assert!(on_screen(&rig).1 > veiled.1, "twin: the veil gone, the picture is back");

    ctl.act(Act::Undo);
    rig.pump();
    assert!(corrections().is_empty(), "the removal is one undo step");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
