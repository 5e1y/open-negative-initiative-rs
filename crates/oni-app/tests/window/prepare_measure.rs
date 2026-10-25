//! The measure's screen: each frame's histogram drawn as it lands and morphed into the next one's,
//! the well empty until the first lands, a measure stopped and started again included.

use oni_app::app::{Options, Shell};
use oni_app::{Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const FRAMES: usize = 8;

fn edges(p: &Preparing) -> String {
    p.get_trace_curves().iter().map(|c| c.edge.to_string()).collect()
}

pub(crate) fn each_frame_measured_morphs_the_histogram_and_a_measure_opens_on_an_empty_well() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the measure's screen is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-measure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..FRAMES {
        common::graded_tiff(&folder.join(format!("F{i}.tif")), 1500, 1000, 0.2 + 0.7 * i as f32 / FRAMES as f32);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p) = (w.global::<Library>(), w.global::<Preparing>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let first = oni_app::app::tiles(w).iter().map(|t| t.index).min().unwrap();
    lib.invoke_tile_clicked(first, false, false);
    rig.pump();
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == FRAMES);
    p.invoke_step_picked(4);
    rig.until("every frame placed", Duration::from_secs(20), || p.get_action_title() == format!("MEASURE {FRAMES} FRAMES"));
    rig.settle();

    // Started: the well is empty until a frame lands, which only the event loop brings.
    p.invoke_advance();
    assert!(p.get_measuring());
    assert!(!p.get_traced() && p.get_trace_curves().row_count() == 0 && p.get_trace_trail().row_count() == 0, "the first measure opens on an empty well");
    // Drawn once, so the well is laid and tells its size.
    let _ = rig.drawn();
    let (mut names, mut drawn) = (HashSet::new(), HashSet::new());
    let start = Instant::now();
    while p.get_measuring() && names.len() < 3 {
        assert!(start.elapsed() < Duration::from_secs(120), "timed out waiting for the frames' histograms");
        rig.pump();
        if p.get_traced() {
            assert_eq!(p.get_trace_curves().row_count(), 4, "luminance, red, green and blue");
            assert!(p.get_trace_name().starts_with('F') && p.get_trace_count().contains(&format!("/ {FRAMES}")));
            names.insert(p.get_trace_name().to_string());
            let e = edges(&p);
            assert!(e.starts_with("M 0 "), "the curves are laid in the well's points: {e:.40}");
            drawn.insert(e);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("shot {}", rig.shot("prepare-measuring").display());
    // Twin: the histogram the checks below find gone was there, and moved between landings.
    assert!(names.len() >= 2, "the histogram follows the frames as they land: {names:?}");
    assert!(drawn.len() > names.len(), "a landing morphs over several frames, never jumps: {} drawn for {} frames", drawn.len(), names.len());
    assert!(p.get_trace_trail().row_count() >= 1, "the frames before trail behind");

    // Stopped, then started again: no histogram of the measure before shows.
    p.invoke_cancel();
    rig.until("the measure stopped", Duration::from_secs(60), || !p.get_measuring());
    assert!(p.get_shown(), "a stopped measure keeps the roll open");
    assert!(!p.get_traced() && p.get_trace_curves().row_count() == 0);
    p.invoke_advance();
    assert!(p.get_measuring());
    assert!(!p.get_traced() && p.get_trace_curves().row_count() == 0 && p.get_trace_trail().row_count() == 0, "the measure again opens on an empty well, not a stale one");
    rig.until("the measure landed", Duration::from_secs(180), || !p.get_shown() || w.global::<oni_app::Advice>().get_shown());
    assert!(!p.get_traced(), "a landed measure leaves nothing drawn for the next one");
    let _ = std::fs::remove_dir_all(&root);
}
