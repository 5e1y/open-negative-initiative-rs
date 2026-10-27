//! A slide roll is a plain RAW editor's: Prepare the Roll asks no measure and writes no roll, an
//! unmeasured slide opens in the editor, and Auto levels lays one RGB black and white on a click.

use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState, Preparing};
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

pub(crate) fn a_slide_roll_is_prepared_unmeasured_and_levelled_on_a_click() {
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
    for i in 0..FRAMES {
        sidecar::write(&measured, &canonical(&negatives, i), WriteOptions::default()).unwrap();
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
    assert_eq!(skipped(), [false, true, true, true, true], "a slide roll has no base, flags or window");
    assert_eq!(p.get_action_title().as_str(), "PREPARE 3 FRAMES", "no measure to run");
    p.invoke_advance();
    rig.until("the slides written", Duration::from_secs(30), || !p.get_shown());
    rig.settle();
    for i in 0..FRAMES {
        let s = sidecar::read(&canonical(&slides, i), false, true).unwrap().settings;
        assert_eq!((s.mode, s.roll_reference.is_none(), s.gains.stops), (ConversionMode::Positive, true, [0.0; 3]), "F{i}");
    }
    rig.until("a slide open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some());
    rig.settle();
    assert!(panel.get_slide(), "the Roll tab drops the measure's warning, the dyes and the ceiling");

    // Another slide opened, never measured, stays in the editor.
    let opened = oni_app::app::tiles(w).iter().map(|t| t.index).max().unwrap();
    lib.invoke_tile_opened(opened);
    rig.until("the other slide open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("F2.tif")));
    rig.settle();
    assert!(!p.get_shown(), "a slide asks for no roll");

    // Auto levels: one black and white on every channel, the gains at rest, one undo.
    let rest = settings();
    panel.invoke_auto_levels();
    rig.until("the slide levelled", Duration::from_secs(60), || settings().levels != rest.levels);
    let s = settings();
    assert!(alike(&s) && s.gains.stops == [0.0; 3] && s.roll_reference.is_none(), "{:?} {:?}", s.levels, s.gains);
    assert!(s.levels.red.black > rest.levels.red.black || s.levels.red.white < rest.levels.red.white);
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert_eq!(settings().levels, rest.levels, "one undo takes the key back");

    // Twin: a negative on its roll levels each channel on its own, the gains aligned.
    shell.open_folder(&negatives);
    rig.until("the negatives read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    lib.invoke_tile_opened(first());
    rig.until("a negative open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.contains("Negatives")));
    rig.settle();
    assert!(!panel.get_slide() && !p.get_shown());
    let before = settings();
    panel.invoke_auto_levels();
    rig.until("the negative levelled", Duration::from_secs(60), || settings().levels != before.levels);
    let n = settings();
    assert!(!alike(&n) && n.gains.stops != [0.0; 3], "{:?} {:?}", n.levels, n.gains);
    let _ = std::fs::remove_dir_all(&root);
}
