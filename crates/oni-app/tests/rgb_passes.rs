//! RGB passes on the headless window: Prepare the Roll groups the files by threes, reads their order,
//! shifts by one file and registers each triplet; the gallery and status bar show each by its lead.

use oni_app::app::{Options, Shell};
use oni_app::{App, Library, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

const W: usize = 900;
const H: usize = 600;
/// The red passes' content lies this many pixels right of the green's.
const RED_SHIFT: f64 = 2.0;

/// Discs of many sizes and levels over a grey ground: edges in every direction for the registration.
fn texture(seed: u64) -> Vec<f32> {
    let mut values = vec![0.3f32; W * H];
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..420 {
        let level = (0.05 + 0.9 * next()) as f32;
        let (x, y, inner) = (next() * W as f64, next() * H as f64, 3.0 + 14.0 * next());
        let outer = inner + 0.6 + 6.0 * next();
        for row in ((y - outer).max(0.0) as usize)..((y + outer).ceil() as usize).min(H) {
            for col in ((x - outer).max(0.0) as usize)..((x + outer).ceil() as usize).min(W) {
                let r = ((col as f64 + 0.5 - x).powi(2) + (row as f64 + 0.5 - y).powi(2)).sqrt();
                let a = ((outer - r) / (outer - inner)).clamp(0.0, 1.0) as f32;
                values[row * W + col] = level * a + values[row * W + col] * (1.0 - a);
            }
        }
    }
    values
}

/// A pass under narrowband light: its band carries the frame, the other two a little of it.
fn pass(path: &Path, frame: &[f32], band: usize, shift: f64) {
    common::pixels_tiff(path, W, H, |x, y| {
        let x = (x as f64 - shift).round().clamp(0.0, (W - 1) as f64) as usize;
        let v = frame[y.min(H - 1) * W + x.min(W - 1)];
        let mut rgb = [0.12 * v; 3];
        rgb[band] = v;
        rgb
    });
}

fn loaded(path: &Path) -> sidecar::Loaded {
    sidecar::read(path, false, true).unwrap()
}

fn strings(model: slint::ModelRc<slint::SharedString>) -> Vec<String> {
    (0..model.row_count()).filter_map(|i| model.row_data(i)).map(|s| s.to_string()).collect()
}

#[test]
fn a_roll_of_passes_is_grouped_by_threes_ordered_by_its_light_and_registered_as_measured() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: RGB passes are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-rgb-passes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Passes"), root.join("Support"), root.join("Caches"));
    std::fs::create_dir_all(&folder).unwrap();
    // Two frames taken green, red, blue, and one stray file: P6.
    for (frame, first) in [(texture(0x9E37_79B9_7F4A_7C15), 0), (texture(0x2545_F491_4F6C_DD1D), 3)] {
        pass(&folder.join(format!("P{first}.tif")), &frame, 1, 0.0);
        pass(&folder.join(format!("P{}.tif", first + 1)), &frame, 0, RED_SHIFT);
        pass(&folder.join(format!("P{}.tif", first + 2)), &frame, 2, 0.0);
    }
    pass(&folder.join("P6.tif"), &texture(7), 1, 0.0);
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
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
    let (lib, p, app) = (w.global::<Library>(), w.global::<Preparing>(), w.global::<App>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| {
        let tiles = oni_app::app::tiles(w);
        tiles.iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).clone()
    };
    assert_eq!(oni_app::app::tiles(w).len(), 7);

    // One frame picked brings the folder; the files stand alone until RGB Passes is chosen.
    lib.invoke_tile_clicked(tile("P0").index, false, false);
    rig.pump();
    ctl.act("prepare-roll");
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == 7);
    rig.until("the first frame read", Duration::from_secs(10), || p.get_capture() == 0);
    rig.settle();
    assert!(!p.get_grouped(), "twin: no grouping before RGB Passes is chosen");
    p.invoke_capture_picked(2);
    rig.pump();
    assert!(p.get_grouped() && p.get_cells().row_count() == 2, "seven files make two frames");
    assert_eq!(p.get_grouping_note().as_str(), "2 frames from 7 files");
    assert!(p.get_grouping_remark().starts_with("1 file is left out"), "{}", p.get_grouping_remark());
    assert!(p.get_cells().row_data(0).unwrap().rgb, "a grouped frame wears its seal");
    // The order read off the first frame's light: its files lean green, red, blue.
    rig.until("the order read off the files", Duration::from_secs(30), || strings(p.get_pass_letters()) == ["G", "R", "B"]);
    assert_eq!(strings(p.get_pass_names()), ["P0.tif", "P1.tif", "P2.tif"]);
    assert!((0..3).all(|i| p.get_pass_pictures().row_data(i).is_some_and(|i| i.size().width > 0)), "each chip shows its file");
    rig.settle();
    println!("shot {}", rig.shot("rgb-passes-grouped").display());

    // A band dropped at the end slides the others aside; dropped back, the order is the light's again.
    p.invoke_pass_moved(0, 2);
    rig.pump();
    assert_eq!(strings(p.get_pass_letters()), ["R", "B", "G"]);
    p.invoke_pass_moved(2, 0);
    rig.pump();
    assert_eq!(strings(p.get_pass_letters()), ["G", "R", "B"]);

    // Shift by One File: the triplets start one file later and their order is read again.
    p.invoke_shift_grouping();
    rig.pump();
    assert_eq!(strings(p.get_pass_names()), ["P1.tif", "P2.tif", "P3.tif"]);
    assert!(p.get_grouping_remark().starts_with("1 file is left out"));
    rig.until("the shifted order read", Duration::from_secs(30), || strings(p.get_pass_letters()) == ["R", "B", "G"]);
    // Twin: three shifts come back to where the grouping started.
    p.invoke_shift_grouping();
    p.invoke_shift_grouping();
    rig.pump();
    assert_eq!(strings(p.get_pass_names()), ["P0.tif", "P1.tif", "P2.tif"]);
    rig.until("the order read again", Duration::from_secs(30), || strings(p.get_pass_letters()) == ["G", "R", "B"]);

    // Measured: each triplet registered, its passes joined to their lead, as one gallery step.
    p.invoke_step_picked(4);
    rig.pump();
    assert_eq!(p.get_action_title().as_str(), "MEASURE 2 FRAMES");
    p.invoke_advance();
    let advice = w.global::<oni_app::Advice>();
    rig.until("the measure landed", Duration::from_secs(300), || !p.get_shown() || advice.get_shown());
    assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    rig.pump();
    assert!(!app.get_gallery());
    let lead = loaded(&canonical("P3.tif")).settings;
    let passes = lead.rgb_passes.clone().expect("the lead names its passes");
    assert_eq!((passes.red.as_str(), passes.green.as_str(), passes.blue.as_str()), ("P4.tif", "P3.tif", "P5.tif"));
    let r = passes.registration.expect("the triplet is registered");
    assert!((r.red.dx * W as f64 + RED_SHIFT).abs() < 0.2 && (r.red.dy * W as f64).abs() < 0.2, "the red pass laid {RED_SHIFT} px back: {:?}", r.red);
    // Twin: the blue pass, taken where the green was, is laid where it lies.
    assert!((r.blue.dx * W as f64).abs() < 0.2 && (r.blue.dy * W as f64).abs() < 0.2 && (r.blue.scale - 1.0).abs() < 1e-3, "{:?}", r.blue);
    assert_eq!(loaded(&canonical("P4.tif")).settings.pass_of.as_deref(), Some("P3.tif"));
    assert_eq!(loaded(&canonical("P5.tif")).settings.pass_of.as_deref(), Some("P3.tif"));
    assert!(loaded(&canonical("P6.tif")).settings.rgb_passes.is_none(), "the stray file stays out of the roll");
    let open = ctl.state.borrow().open_path().map(str::to_string);
    assert!(open.as_deref().is_some_and(|o| o.ends_with("P0.tif")));
    assert!(ctl.state.borrow().store().settings().rgb_passes.as_ref().is_some_and(|p| p.registration.is_some()), "the open lead landed in memory");

    // The gallery shows each frame by its lead and the stray file; the seal names a lone lead.
    ctl.act("toggle-mode");
    rig.pump();
    // The measure also lays the roll's contact sheet, which joins the library beside the frames.
    let frames = || oni_app::app::tiles(w).into_iter().filter(|t| !t.caption.ends_with("\u{2014} contact sheet")).collect::<Vec<_>>();
    rig.until("the passes hidden", Duration::from_secs(10), || frames().len() == 3);
    let captions: Vec<String> = frames().iter().map(|t| t.caption.to_string()).collect();
    assert_eq!(captions, ["P0", "P3", "P6"]);
    assert_eq!(oni_app::app::tiles(w).len(), 4, "and the roll's contact sheet");
    assert!(tile("P0").rgb && tile("P3").rgb && !tile("P6").rgb);
    lib.invoke_tile_clicked(tile("P3").index, false, false);
    rig.pump();
    assert_eq!(lib.get_status_seal().as_str(), "RGB");
    lib.invoke_tile_clicked(tile("P6").index, false, false);
    rig.pump();
    assert_eq!(lib.get_status_seal().as_str(), "", "twin: a frame standing alone wears no seal");
    rig.settle();
    println!("shot {}", rig.shot("rgb-passes-gallery").display());

    // One undo takes the measure back, the passes' names of their lead with it.
    ctl.act("undo");
    rig.pump();
    assert!(loaded(&canonical("P4.tif")).settings.pass_of.is_none(), "one undo unjoins the passes");
    assert!(loaded(&canonical("P3.tif")).settings.rgb_passes.is_none());
    let _ = std::fs::remove_dir_all(&root);
}
