//! A slide roll: Prepare the Roll places one window and lays the gains of the white picked on every
//! slide, its level held; none picked, none laid; never a roll reference nor a contact sheet.

use oni_app::app::{Options, Shell};
use oni_app::{Framing, Library, PanelState, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

const FRAMES: usize = 3;

/// Frames with a warm cast, red brightest and blue darkest, as a slide's own colour.
fn cast(folder: &Path) {
    for i in 0..FRAMES {
        let tone = 0.5 + 0.2 * i as f32;
        common::pixels_tiff(&folder.join(format!("F{i}.tif")), 300, 200, |x, y| {
            let v = 0.1 + 0.8 * tone * (x + y) as f32 / 500.0;
            [v, 0.8 * v, 0.6 * v]
        });
    }
}

fn alike(s: &PipelineSettings) -> bool {
    s.levels.red == s.levels.green && s.levels.green == s.levels.blue
}

pub(crate) fn a_slide_roll_takes_the_gains_of_the_white_picked_through_one_window() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the slides are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-slides-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (slides, negatives) = (root.join("Slides"), root.join("Negatives"));
    cast(&slides);
    cast(&negatives);
    let canonical = |folder: &Path, i: usize| PathBuf::from(oni_library::paths::canonical(&folder.join(format!("F{i}.tif"))));
    // The negatives stand on a roll already, so they open in the editor.
    let mut measured = PipelineSettings::rest(ConversionMode::Negative, false);
    measured.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    // The slides are unprepared positives, so one opens in the editor before any roll.
    let slide = PipelineSettings::rest(ConversionMode::Positive, false);
    for i in 0..FRAMES {
        sidecar::write(&measured, &canonical(&negatives, i), WriteOptions::default()).unwrap();
        sidecar::write(&slide, &canonical(&slides, i), WriteOptions::default()).unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p, panel) = (w.global::<Library>(), w.global::<Preparing>(), w.global::<PanelState>());
    let first = || oni_app::app::tiles(w).iter().map(|t| t.index).min().unwrap();
    let settings = || ctl.state.borrow().store().settings().clone();

    shell.open_folder(&slides);
    rig.until("the slides read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    // A slide never prepared opens in the editor, no roll asked.
    let opened = oni_app::app::tiles(w).iter().map(|t| t.index).max().unwrap();
    lib.invoke_tile_opened(opened);
    rig.until("a slide open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("F2.tif")));
    rig.settle();
    assert!(!p.get_shown(), "a slide asks for no roll");
    assert!(!w.global::<Framing>().get_has_window(), "nor opens on a window");

    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();

    // A slide once measured as its own negative is measured again on its gains alone.
    let mut once = sidecar::read(&canonical(&slides, 1), false, true).unwrap().settings;
    (once.mode, once.inverted_source) = (ConversionMode::Negative, true);
    sidecar::write(&once, &canonical(&slides, 1), WriteOptions::default()).unwrap();
    lib.invoke_tile_clicked(first(), false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == FRAMES);
    let skipped = || (0..5).map(|i| p.get_skipped().row_data(i).unwrap()).collect::<Vec<bool>>();
    // Twin first: a negative scanner roll walks past the flat-field to its base, flags and measure.
    p.invoke_film_picked(0);
    rig.pump();
    assert_eq!(skipped(), [false, true, false, false, false]);
    assert_eq!(p.get_action_title().as_str(), "NEXT");
    p.invoke_film_picked(1);
    rig.pump();
    assert_eq!(skipped(), [false, true, false, true, false], "a slide roll picks a white on its base step and has no flags");
    p.invoke_advance();
    rig.pump();
    assert_eq!(p.get_step(), 2, "the film leads to the white");
    assert!(p.get_skippable() && !p.get_action_enabled(), "no white yet: Skip offered, NEXT waiting");
    // A white picked on a slide's warm ramp: its colour alone goes to grey on every slide.
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    // Drawn first, so the base step's area has its size and the scan its place in it.
    let _ = rig.drawn();
    rig.pump();
    p.invoke_base_clicked(p.get_scan_x() + p.get_scan_width() * 0.5, p.get_scan_y() + p.get_scan_height() * 0.5);
    rig.pump();
    assert_eq!((p.get_step(), p.get_action_title().as_str(), p.get_action_enabled(), p.get_skippable()), (2, "NEXT", true, false), "the white picked stays shown until NEXT");
    p.invoke_advance();
    rig.pump();
    assert_eq!(p.get_step(), 4, "the white leads to the window");
    assert_eq!(p.get_readouts().row_data(4).unwrap().as_str(), "1 / 3", "every slide measured");
    assert!(p.get_window_shown());
    let disk = |i: usize| sidecar::read(&canonical(&slides, i), false, true).unwrap().settings;
    let rest = disk(1);
    p.invoke_advance();
    rig.until("the slides levelled", Duration::from_secs(60), || !p.get_shown());
    rig.settle();
    rig.until("a slide open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some());
    rig.settle();
    // The open frame landed in memory, the others on disk; each its gains moved, its windows at rest.
    let open = ctl.state.borrow().open_path().map(str::to_string).unwrap();
    let laid = |i: usize| if open.ends_with(&format!("F{i}.tif")) { settings() } else { disk(i) };
    let frames: Vec<PipelineSettings> = (0..FRAMES).map(laid).collect();
    let window = frames[0].geometry.measure_window;
    for (i, s) in frames.iter().enumerate() {
        assert_eq!((s.mode, s.roll_reference.is_none(), s.inverted_source), (ConversionMode::Positive, true, false), "F{i}");
        let g = s.gains.stops;
        assert!(alike(s) && g[0] < g[1] && g[1] < g[2] && g.iter().sum::<f32>().abs() < 1e-4, "F{i}: red down, blue up, the level held: {:?} {:?}", s.levels, s.gains);
        assert!(window.is_some() && s.geometry.measure_window == window, "F{i}: one window for the roll");
    }
    assert!(panel.get_slide(), "the Roll tab drops the measure's warning, the dyes and the ceiling");
    assert!(!w.global::<oni_app::Store>().get_source_balance_modified(), "the measured gains are the slide's rest: no reset offered");
    assert!(frames.iter().all(|s| s.measured_gains == Some(s.gains.stops) && s.gains == frames[0].gains), "one roll's gains on every slide, kept as its rest");
    assert!(w.global::<Framing>().get_has_window() && !w.global::<Framing>().get_armed(), "the Crop tab shows the window, unarmed");
    // The measure lays no contact sheet beside the slides.
    let sheets = || {
        let mut found: Vec<PathBuf> = std::fs::read_dir(&slides).unwrap().filter_map(|e| Some(e.ok()?.path())).filter(|p| p.extension().is_some_and(|x| x == "tif") && p.to_string_lossy().contains("contact sheet")).collect();
        found.sort();
        found
    };
    assert!(sheets().is_empty(), "{:?}", sheets());
    assert_eq!(ctl.state.borrow().picked().len(), 1, "the measure lands on one frame alone, never the whole roll unasked");
    // A gallery undo takes back every frame of the measure, a redo lays them again.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    rig.until("every slide back", Duration::from_secs(10), || (0..FRAMES).all(|i| disk(i).gains == rest.gains && disk(i).geometry.measure_window.is_none()));
    ctl.act(oni_app::Act::Redo);
    rig.pump();
    rig.until("every slide balanced again", Duration::from_secs(10), || (0..FRAMES).all(|i| disk(i).gains != rest.gains));

    // Twin: the white skipped, the slides keep the camera's 5500 K, their gains at rest.
    lib.invoke_tile_clicked(first(), false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll again", Duration::from_secs(10), || p.get_shown());
    p.invoke_film_picked(1);
    p.invoke_step_picked(2);
    rig.pump();
    if p.get_has_base() {
        p.invoke_clear_base();
        rig.pump();
    }
    assert!(p.get_skippable());
    p.invoke_advance();
    rig.pump();
    p.invoke_advance();
    rig.until("the slides measured unpicked", Duration::from_secs(60), || !p.get_shown());
    rig.settle();
    rig.until("every slide at rest", Duration::from_secs(10), || (0..FRAMES).all(|i| disk(i).gains.stops == [0.0; 3] && disk(i).measured_gains.is_none()));

    let _ = std::fs::remove_dir_all(&root);
}
