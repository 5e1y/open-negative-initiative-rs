//! The Film Effects block through the window's pointer: a press on Grain's rail lays the grain, its
//! block's Reset shows and the frame renders again; shots at fit and at 100 % show it on the photo.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// The photo shown and whether it is a positive: the first photo of `ONI_RAW_DIR`, a RAW read as a
/// negative and any other file as a positive, else a graded TIFF.
fn photo(folder: &Path) -> (String, bool) {
    std::fs::create_dir_all(folder).unwrap();
    if let Some(dir) = std::env::var_os("ONI_RAW_DIR") {
        let kind = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mut photos: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| ["nef", "rw2", "jpg", "tif"].contains(&kind(p).as_str())).collect();
        photos.sort();
        if let Some(photo) = photos.first() {
            let name = photo.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::copy(photo, folder.join(&name)).unwrap();
            return (name, !["nef", "rw2"].contains(&kind(photo).as_str()));
        }
    }
    common::graded_tiff(&folder.join("A.tif"), 2400, 1600, 0.7);
    ("A.tif".into(), false)
}

/// A press on the rail of the slider named `label`, at `share` of its travel.
fn press_rail(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, share: f32) {
    let slider = common::the(w, AccessibleRole::Slider, label);
    let at = LogicalPosition::new(slider.x + 10.0 + share * (slider.width - 20.0), slider.y + slider.height - 13.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn a_press_on_grains_rail_lays_the_grain_and_renders_it() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Film Effects block is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-film-effects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    let (name, positive) = photo(&folder);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    w.global::<Library>().invoke_tile_opened(0);
    rig.until("the photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    w.global::<oni_app::Notice>().invoke_close();
    rig.settle();
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with(&name)));
    if positive {
        let n = ctl.state.borrow().renders();
        w.global::<PanelState>().invoke_film_type_picked(1);
        rig.until("the positive rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
        rig.settle();
    }

    let grain = || ctl.state.borrow().store().settings().grain;
    let g = w.global::<Store>();
    assert!(grain().amount == 0.0 && !g.get_film_effects_modified(), "the photo opens with no grain");
    let renders = ctl.state.borrow().renders();
    press_rail(&rig, w, "Grain", 0.8);
    rig.until("the grain rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let laid = grain();
    assert!((laid.amount - 0.8).abs() < 0.03, "the press lays Grain at its place on the rail: {laid:?}");
    assert!(g.get_film_effects_modified(), "the block's Reset shows");
    // Twin: the press reached Grain alone, its size at rest.
    assert_eq!(laid.size, 0.5, "Grain size moved: {laid:?}");
    println!("shot {}", rig.shot("film-effects-grain-fit").display());

    // At 100 %, where the grain's cells are a pixel or two.
    for _ in 0..oni_app::viewport::ZOOM_STEPS.len() {
        if ctl.state.borrow().viewport().scale == Some(1.0) {
            break;
        }
        let n = ctl.state.borrow().renders();
        ctl.act(oni_app::Act::Larger);
        rig.until("a rung's render", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
    }
    rig.settle();
    assert_eq!(ctl.state.borrow().viewport().scale, Some(1.0), "the view at 100 %");
    println!("shot {}", rig.shot("film-effects-grain-100").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
